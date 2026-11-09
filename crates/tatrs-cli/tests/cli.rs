use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn tatrs() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tatrs"))
}

/// Scratch space inside the workspace: some sandboxes deny writes to the system
/// temp directory for spawned binaries.
fn scratch() -> tempfile::TempDir {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-scratch");
    std::fs::create_dir_all(&base).unwrap();
    tempfile::Builder::new()
        .prefix("cli-")
        .tempdir_in(&base)
        .unwrap()
}

fn run_in(dir: &Path, args: &[&str]) -> Output {
    tatrs().current_dir(dir).args(args).output().unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn json(output: &Output) -> Value {
    serde_json::from_str(&stdout(output)).unwrap_or_else(|err| {
        panic!(
            "invalid json: {err}\nstdout: {}\nstderr: {}",
            stdout(output),
            stderr(output)
        )
    })
}

fn init(dir: &Path) {
    let output = run_in(dir, &["init"]);
    assert!(output.status.success(), "init failed: {}", stderr(&output));
}

fn task_file(dir: &Path, rel: &str) -> PathBuf {
    dir.join(rel)
}

#[test]
fn init_new_ls_round_trip() {
    let tmp = scratch();
    init(tmp.path());

    let output = run_in(
        tmp.path(),
        &["new", "Fix parser", "-t", "bug,agent", "-p", "7", "--json"],
    );
    assert!(output.status.success(), "new failed: {}", stderr(&output));

    let task = json(&output);
    assert_eq!(task["schema"], 1);
    assert_eq!(task["title"], "Fix parser");
    assert_eq!(task["status"], "OPEN");
    assert_eq!(task["priority"], 7);
    assert_eq!(task["tags"], serde_json::json!(["bug", "agent"]));
    assert_eq!(task["closed"], false);

    let rel = task["path"].as_str().unwrap().to_owned();
    assert!(task_file(tmp.path(), &rel).is_file());

    let listed = run_in(tmp.path(), &["ls", "--json"]);
    assert!(listed.status.success());
    let payload = json(&listed);
    assert_eq!(payload["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(payload["tasks"][0]["id"], task["id"]);

    let human = run_in(tmp.path(), &["ls"]);
    assert!(stdout(&human).contains("OPEN [PRIORITY:   7] [bug,agent] Fix parser"));

    let porcelain = run_in(tmp.path(), &["ls", "--porcelain"]);
    assert!(stdout(&porcelain).starts_with(task["id"].as_str().unwrap()));
}

#[test]
fn show_returns_body_and_raw_bytes() {
    let tmp = scratch();
    init(tmp.path());

    let created = json(&run_in(
        tmp.path(),
        &["new", "With acceptance", "--acceptance", "--json"],
    ));
    let id = created["id"].as_str().unwrap();

    let shown = run_in(tmp.path(), &["show", id, "--json"]);
    assert!(shown.status.success(), "show failed: {}", stderr(&shown));
    let task = json(&shown);
    assert!(task["body"].as_str().unwrap().contains("## Acceptance"));

    let raw = run_in(tmp.path(), &["show", id, "--raw"]);
    let rel = created["path"].as_str().unwrap();
    let on_disk = std::fs::read_to_string(tmp.path().join(rel)).unwrap();
    assert_eq!(stdout(&raw), on_disk);

    let path_only = run_in(tmp.path(), &["show", id, "--path-only"]);
    assert_eq!(stdout(&path_only).trim(), rel);
}

#[test]
fn check_fails_on_invalid_task() {
    let tmp = scratch();
    init(tmp.path());

    let task_dir = tmp.path().join("tasks/20260101-000000");
    std::fs::create_dir(&task_dir).unwrap();
    std::fs::write(task_dir.join("TASK.md"), "no heading here\n").unwrap();

    let output = run_in(tmp.path(), &["check", "--json"]);
    assert_eq!(output.status.code(), Some(3), "stderr: {}", stderr(&output));
    let report = json(&output);
    assert_eq!(report["ok"], false);
    assert!(report["errors"].as_u64().unwrap() >= 1);
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E0101")
    );
}

#[test]
fn check_is_clean_for_a_fresh_task() {
    let tmp = scratch();
    init(tmp.path());
    run_in(tmp.path(), &["new", "Clean task", "--acceptance"]);
    let output = run_in(tmp.path(), &["check", "--strict"]);
    assert!(output.status.success(), "check failed: {}", stdout(&output));
}

#[test]
fn deterministic_ids_are_idempotent() {
    let tmp = scratch();
    init(tmp.path());

    let first = run_in(
        tmp.path(),
        &["new", "Deterministic", "--id", "20260101-000000", "--json"],
    );
    assert!(
        first.status.success(),
        "first create failed: {}",
        stderr(&first)
    );

    let second = run_in(
        tmp.path(),
        &["new", "Deterministic", "--id", "20260101-000000", "--json"],
    );
    assert!(
        second.status.success(),
        "idempotent retry failed: {}",
        stderr(&second)
    );
    assert_eq!(json(&first)["id"], json(&second)["id"]);

    let listed = json(&run_in(tmp.path(), &["ls", "--json"]));
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 1);
}

#[test]
fn empty_listing_can_fail() {
    let tmp = scratch();
    init(tmp.path());

    let ok = run_in(tmp.path(), &["ls"]);
    assert!(ok.status.success());

    let failed = run_in(tmp.path(), &["ls", "--fail-if-empty"]);
    assert_eq!(failed.status.code(), Some(5));
}

#[test]
fn missing_repository_is_a_runtime_error() {
    let outside = std::env::temp_dir();
    let output = run_in(&outside, &["ls"]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(stderr(&output).contains("tasks/"));
}

#[test]
fn read_only_blocks_mutation_but_allows_dry_run() {
    let tmp = scratch();
    init(tmp.path());

    let blocked = run_in(tmp.path(), &["new", "Nope", "--read-only"]);
    assert_eq!(blocked.status.code(), Some(2));
    assert!(stderr(&blocked).contains("--read-only"));

    let preview = run_in(tmp.path(), &["new", "Preview", "--read-only", "--dry-run"]);
    assert!(preview.status.success());
    assert!(stdout(&preview).contains("would create"));

    let listed = json(&run_in(tmp.path(), &["ls", "--json"]));
    assert!(listed["tasks"].as_array().unwrap().is_empty());
}

#[test]
fn body_flags_are_applied() {
    let tmp = scratch();
    init(tmp.path());

    let created = json(&run_in(
        tmp.path(),
        &[
            "new",
            "Body task",
            "--body",
            "# Context\n\nSome text",
            "--json",
        ],
    ));
    let id = created["id"].as_str().unwrap();
    let shown = json(&run_in(tmp.path(), &["show", id, "--json"]));
    assert_eq!(shown["body"], "\n# Context\n\nSome text\n");

    let body_path = tmp.path().join("body.md");
    std::fs::write(&body_path, "## Acceptance\n\n- [ ] x\n").unwrap();
    let from_file = json(&run_in(
        tmp.path(),
        &[
            "new",
            "From file",
            "--body-file",
            body_path.to_str().unwrap(),
            "--json",
        ],
    ));
    assert!(
        from_file["body"]
            .as_str()
            .unwrap()
            .starts_with("\n## Acceptance")
    );
}

#[test]
fn invalid_huid_argument_is_a_usage_error() {
    let tmp = scratch();
    init(tmp.path());

    let output = run_in(tmp.path(), &["show", "not-an-id"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("not a task id"));
}
