//! Human and porcelain rendering. JSON lives in `tatrs_core::json`.

use std::path::Path;

use tatrs_core::{Diagnostic, Repo, Task};

/// Compiler-style one-line report, matching upstream `tatr` so existing editor
/// integrations keep working:
/// `tasks/<id>/TASK.md:1: OPEN [PRIORITY:  10] [bug,agent] Title`
pub fn task_report(task: &Task, rel_path: &str) -> String {
    let mut out = format!(
        "{}:1: {} [PRIORITY: {:>3}]",
        rel_path, task.status, task.priority
    );
    if !task.tags.is_empty() {
        out.push_str(" [");
        out.push_str(&task.tags.join(","));
        out.push(']');
    }
    out.push(' ');
    out.push_str(&task.title);
    out
}

/// Stable tab-separated row: id, status, priority, tags, title, path.
pub fn porcelain_line(task: &Task, rel_path: &str) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        task.id,
        task.status,
        task.priority,
        task.tags.join(","),
        sanitize(&task.title),
        rel_path
    )
}

fn sanitize(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ")
}

pub fn diagnostic_line(diagnostic: &Diagnostic, repo: &Repo) -> String {
    diagnostic.render(Path::new(&repo.rel_path(&diagnostic.path)))
}

/// Diagnostics go to stderr so `--json` stdout stays machine-parseable.
pub fn emit_diagnostics(diagnostics: &[Diagnostic], repo: &Repo, quiet: bool) {
    if quiet {
        return;
    }
    for diagnostic in diagnostics {
        eprintln!("{}", diagnostic_line(diagnostic, repo));
    }
}
