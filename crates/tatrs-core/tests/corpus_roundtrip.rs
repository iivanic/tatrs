//! Compatibility check: every task in the upstream corpus must parse and render
//! back byte-identically (after CRLF normalization). Skipped when the reference
//! checkout is absent, so CI without it still passes.

use std::fs;
use std::path::{Path, PathBuf};

use tatrs_core::{Huid, Task};

fn corpus_dir() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/tatr/tasks");
    dir.is_dir().then_some(dir)
}

fn first_difference(expected: &str, actual: &str) -> String {
    let mut expected_lines = expected.lines();
    let mut actual_lines = actual.lines();
    let mut line = 1;
    loop {
        match (expected_lines.next(), actual_lines.next()) {
            (Some(left), Some(right)) if left == right => line += 1,
            (left, right) => {
                return format!("line {line}:\n    expected: {left:?}\n    actual:   {right:?}");
            }
        }
    }
}

#[test]
fn upstream_corpus_round_trips() {
    let Some(dir) = corpus_dir() else {
        eprintln!("skipping: reference/tatr is not present");
        return;
    };

    let mut checked = 0usize;
    let mut failures = Vec::new();

    for entry in fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(id) = Huid::parse(&name) else {
            continue;
        };
        let task_md = path.join("TASK.md");
        if !task_md.is_file() {
            continue;
        }

        let content = fs::read_to_string(&task_md).unwrap();
        let expected = content.replace("\r\n", "\n");
        let task = Task::parse(id, &content, &task_md);
        let actual = task.render();
        if actual != expected {
            failures.push(format!("{name}: {}", first_difference(&expected, &actual)));
        }
        checked += 1;
    }

    assert!(checked > 0, "no tasks found in {}", dir.display());
    assert!(
        failures.is_empty(),
        "{} of {checked} tasks did not round-trip:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
