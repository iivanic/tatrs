//! In-process reference search. Replaces upstream's `grep` shell-out so the
//! command behaves identically on Windows and cannot be influenced by the
//! user's grep configuration.

use std::fs;
use std::path::Path;

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

/// Find every line mentioning `id`, skipping `.git` and binary files.
/// Best-effort: unreadable files and directories are ignored.
pub fn find_references(repo: &Repo, id: &Huid) -> Vec<Reference> {
    let mut references = Vec::new();
    walk(repo, repo.root(), id, &mut references);
    references
}

fn walk(repo: &Repo, dir: &Path, id: &Huid, out: &mut Vec<Reference>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            walk(repo, &path, id, out);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let Ok(bytes) = fs::read(&path) else {
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
                out.push(Reference {
                    path: repo.rel_path(&path),
                    line: index + 1,
                    text: line.to_owned(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-scratch");
        fs::create_dir_all(&base).unwrap();
        tempfile::Builder::new()
            .prefix("refs-")
            .tempdir_in(&base)
            .unwrap()
    }

    #[test]
    fn finds_source_and_task_references_but_skips_git_and_binaries() {
        let tmp = scratch();
        crate::store::init(tmp.path()).unwrap();
        let repo = Repo::open(tmp.path()).unwrap();
        let id = Huid::parse("20261109-120000").unwrap();

        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(
            tmp.path().join("src/main.c"),
            format!("// TODO: fix\n// TASK({id}): later\n"),
        )
        .unwrap();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".git/HEAD"), format!("{id}\n")).unwrap();
        fs::write(tmp.path().join("blob.bin"), b"\0\0").unwrap();

        let references = find_references(&repo, &id);
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].path, "src/main.c");
        assert_eq!(references[0].line, 2);
    }
}
