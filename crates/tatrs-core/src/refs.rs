//! In-process reference search. Replaces upstream's `grep` shell-out so the
//! command behaves identically on Windows, cannot be influenced by the user's
//! grep configuration, and honours git's ignore rules.
//!
//! Ignore handling is delegated to the `ignore` crate: gitignore semantics
//! (nested files, negation, `**`, directory-only patterns) are subtle enough
//! that a hand-rolled matcher would be a liability. Global user excludes are
//! **not** applied — only the repository's own `.gitignore` files and
//! `.git/info/exclude` — so results are reproducible across machines.

use std::fs;

use ignore::WalkBuilder;
use serde::Serialize;

use crate::huid::Huid;
use crate::store::Repo;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reference {
    /// Repository-relative, forward-slash separated.
    pub path: String,
    pub line: usize,
    pub text: String,
}

/// Find references, respecting the repository's ignore files.
pub fn find_references(repo: &Repo, id: &Huid) -> Vec<Reference> {
    find_references_with(repo, id, true)
}

/// `respect_ignore = false` scans ignored files too (`--no-ignore`).
pub fn find_references_with(repo: &Repo, id: &Huid, respect_ignore: bool) -> Vec<Reference> {
    let mut references = Vec::new();

    let mut builder = WalkBuilder::new(repo.root());
    builder
        .hidden(false)
        .parents(true)
        .ignore(false)
        .git_global(false)
        .git_ignore(respect_ignore)
        .git_exclude(respect_ignore)
        .require_git(false)
        .follow_links(false)
        .same_file_system(false);

    builder.filter_entry(|entry| {
        !matches!(
            entry.file_name().to_string_lossy().as_ref(),
            ".git" | ".hg" | ".svn"
        )
    });

    for entry in builder.build().flatten() {
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        let path = entry.path();

        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        if bytes.contains(&0) {
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        if !text.contains(id.as_str()) {
            continue;
        }

        for (index, line) in text.lines().enumerate() {
            if line.contains(id.as_str()) {
                references.push(Reference {
                    path: repo.rel_path(path),
                    line: index + 1,
                    text: line.to_owned(),
                });
            }
        }
    }

    references.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
    });
    references
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn scratch() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-scratch");
        fs::create_dir_all(&base).unwrap();
        tempfile::Builder::new()
            .prefix("refs-")
            .tempdir_in(&base)
            .unwrap()
    }

    fn setup() -> (tempfile::TempDir, Repo, Huid) {
        let tmp = scratch();
        crate::store::init(tmp.path()).unwrap();
        let repo = Repo::open(tmp.path()).unwrap();
        let id = Huid::parse("20261109-120000").unwrap();
        (tmp, repo, id)
    }

    fn paths(references: &[Reference]) -> Vec<&str> {
        references.iter().map(|r| r.path.as_str()).collect()
    }

    #[test]
    fn finds_source_and_task_references_but_skips_git_and_binaries() {
        let (tmp, repo, id) = setup();

        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(
            tmp.path().join("src/main.c"),
            format!("// TODO: fix\n// TASK({id}): later\n"),
        )
        .unwrap();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".git/HEAD"), format!("{id}\n")).unwrap();
        fs::write(tmp.path().join("blob.bin"), b"\0\0").unwrap();
        fs::write(tmp.path().join(".hidden.c"), format!("{id}\n")).unwrap();

        let references = find_references(&repo, &id);
        assert_eq!(paths(&references), vec![".hidden.c", "src/main.c"]);
        assert_eq!(references[1].line, 2);
    }

    #[test]
    fn respects_gitignore_and_negation() {
        let (tmp, repo, id) = setup();

        fs::write(tmp.path().join(".gitignore"), "build/\n*.log\n!keep.log\n").unwrap();
        fs::create_dir_all(tmp.path().join("build")).unwrap();
        fs::write(tmp.path().join("build/gen.c"), format!("{id}\n")).unwrap();
        fs::write(tmp.path().join("debug.log"), format!("{id}\n")).unwrap();
        fs::write(tmp.path().join("keep.log"), format!("{id}\n")).unwrap();
        fs::write(tmp.path().join("src.c"), format!("{id}\n")).unwrap();

        assert_eq!(
            paths(&find_references(&repo, &id)),
            vec!["keep.log", "src.c"]
        );
        assert_eq!(
            paths(&find_references_with(&repo, &id, false)),
            vec!["build/gen.c", "debug.log", "keep.log", "src.c"]
        );
    }

    #[test]
    fn nested_gitignore_applies_to_its_subtree_only() {
        let (tmp, repo, id) = setup();

        fs::create_dir_all(tmp.path().join("src/generated")).unwrap();
        fs::create_dir_all(tmp.path().join("other/generated")).unwrap();
        fs::create_dir_all(tmp.path().join("src/nested")).unwrap();
        fs::write(tmp.path().join("src/.gitignore"), "generated/\n").unwrap();
        fs::write(tmp.path().join("src/generated/a.c"), format!("{id}\n")).unwrap();
        fs::write(tmp.path().join("other/generated/b.c"), format!("{id}\n")).unwrap();
        fs::write(tmp.path().join("src/nested/c.c"), format!("{id}\n")).unwrap();

        assert_eq!(
            paths(&find_references(&repo, &id)),
            vec!["other/generated/b.c", "src/nested/c.c"]
        );
    }
}
