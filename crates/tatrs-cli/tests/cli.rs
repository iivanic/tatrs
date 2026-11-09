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
fn body_line_endings_are_normalized() {
    let tmp = scratch();
    init(tmp.path());

    let created = json(&run_in(
        tmp.path(),
        &[
            "new",
            "CRLF body",
            "--body",
            "line one\r\nline two",
            "--json",
        ],
    ));
    let rel = created["path"].as_str().unwrap();
    let raw = std::fs::read_to_string(tmp.path().join(rel)).unwrap();
    assert!(!raw.contains('\r'), "TASK.md must be LF-only: {raw:?}");
    assert!(raw.ends_with("line one\nline two\n"));
}

fn create(dir: &Path, title: &str, tags: &[&str], priority: i64) -> Value {
    let mut args: Vec<String> = vec!["new".into(), title.into(), "--json".into()];
    for tag in tags {
        args.push("-t".into());
        args.push((*tag).to_owned());
    }
    args.push("-p".into());
    args.push(priority.to_string());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = run_in(dir, &refs);
    assert!(output.status.success(), "{}", stderr(&output));
    json(&output)
}

#[test]
fn tql_filters_and_precedence() {
    let tmp = scratch();
    init(tmp.path());
    create(tmp.path(), "A", &["bug"], 10);
    create(tmp.path(), "B", &["bug"], 100);
    create(tmp.path(), "C", &["ui"], 5);

    let bugs = json(&run_in(tmp.path(), &["ls", ":bug", "--json"]));
    assert_eq!(bugs["tasks"].as_array().unwrap().len(), 2);

    let urgent = json(&run_in(
        tmp.path(),
        &["ls", ":bug and priority lt 50", "--json"],
    ));
    let titles: Vec<_> = urgent["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["A"]);

    let grouped = json(&run_in(
        tmp.path(),
        &["ls", "[:bug or :ui] and priority lt 50", "--json"],
    ));
    let grouped_titles: Vec<_> = grouped["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["title"].as_str().unwrap())
        .collect();
    assert_eq!(grouped_titles, vec!["A", "C"]);

    let not_bug = json(&run_in(tmp.path(), &["ls", ":ui and not :bug", "--json"]));
    assert_eq!(not_bug["tasks"].as_array().unwrap().len(), 1);

    let range = json(&run_in(
        tmp.path(),
        &["ls", "priority ge 10 and priority le 100", "--json"],
    ));
    assert_eq!(range["tasks"].as_array().unwrap().len(), 2);
}

#[test]
fn update_rewrites_only_matches_and_preserves_unknown_properties() {
    let tmp = scratch();
    init(tmp.path());
    let created = create(tmp.path(), "A", &["bug"], 10);
    create(tmp.path(), "B", &["ui"], 100);

    let rel = created["path"].as_str().unwrap();
    let path = tmp.path().join(rel);
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}- CUSTOM: keep\n")).unwrap();

    let output = run_in(
        tmp.path(),
        &["ls", "-u", "close,-bug,+agent", ":bug", "--json"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let payload = json(&output);
    assert_eq!(payload["updated"], 1);
    assert_eq!(payload["dry_run"], false);
    let task = &payload["tasks"][0];
    assert_eq!(task["status"], "CLOSED");
    assert_eq!(task["tags"], serde_json::json!(["agent"]));

    let rewritten = std::fs::read_to_string(&path).unwrap();
    assert!(rewritten.contains("- CUSTOM: keep"), "{rewritten}");
    assert!(rewritten.contains("- STATUS: CLOSED"), "{rewritten}");
    assert!(rewritten.contains("- TAGS: agent"), "{rewritten}");

    let untouched = json(&run_in(tmp.path(), &["ls", "--all", "--json"]));
    let ui = untouched["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["title"] == "B")
        .unwrap();
    assert_eq!(ui["status"], "OPEN");
}

#[test]
fn update_dry_run_writes_nothing() {
    let tmp = scratch();
    init(tmp.path());
    create(tmp.path(), "A", &["bug"], 10);

    let preview = json(&run_in(
        tmp.path(),
        &["ls", "-u", "close", ":bug", "--dry-run", "--json"],
    ));
    assert_eq!(preview["updated"], 1);
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["tasks"][0]["status"], "CLOSED");

    let after = json(&run_in(tmp.path(), &["ls", ":bug", "--json"]));
    assert_eq!(after["tasks"][0]["status"], "OPEN");
}

#[test]
fn reopening_needs_the_all_flag() {
    let tmp = scratch();
    init(tmp.path());
    create(tmp.path(), "A", &["bug"], 10);

    run_in(tmp.path(), &["ls", "-u", "close", ":bug"]);

    let skipped = json(&run_in(tmp.path(), &["ls", "-u", "open", ":bug", "--json"]));
    assert_eq!(skipped["tasks"].as_array().unwrap().len(), 0);

    let reopened = json(&run_in(
        tmp.path(),
        &["ls", "-a", "-u", "open", ":bug", "--json"],
    ));
    assert_eq!(reopened["updated"], 1);
    assert_eq!(reopened["tasks"][0]["status"], "OPEN");
}

#[test]
fn query_and_update_errors_exit_2_with_a_caret() {
    let tmp = scratch();
    init(tmp.path());
    create(tmp.path(), "A", &["bug"], 10);

    let bad_query = run_in(tmp.path(), &["ls", ":bug and"]);
    assert_eq!(bad_query.status.code(), Some(2));
    assert!(stderr(&bad_query).contains('^'), "{}", stderr(&bad_query));

    let bad_update = run_in(tmp.path(), &["ls", "-u", "nope", ":bug"]);
    assert_eq!(bad_update.status.code(), Some(2));
    assert!(stderr(&bad_update).contains("unknown action"));

    let missing_query = run_in(tmp.path(), &["ls", "-u", "close"]);
    assert_eq!(missing_query.status.code(), Some(2));
    assert!(stderr(&missing_query).contains("requires a query"));
}

#[test]
fn read_only_blocks_update_but_allows_dry_run() {
    let tmp = scratch();
    init(tmp.path());
    create(tmp.path(), "A", &["bug"], 10);

    let blocked = run_in(tmp.path(), &["ls", "-u", "close", ":bug", "--read-only"]);
    assert_eq!(blocked.status.code(), Some(2));

    let preview = run_in(
        tmp.path(),
        &["ls", "-u", "close", ":bug", "--dry-run", "--read-only"],
    );
    assert!(preview.status.success(), "{}", stderr(&preview));
}

#[test]
fn huid_and_negative_priority_queries() {
    let tmp = scratch();
    init(tmp.path());
    let created = create(tmp.path(), "A", &["bug"], -5);
    let id = created["id"].as_str().unwrap().to_owned();

    let by_id = json(&run_in(tmp.path(), &["ls", &id, "--json"]));
    assert_eq!(by_id["tasks"].as_array().unwrap().len(), 1);

    let negative = json(&run_in(tmp.path(), &["ls", "priority lt -1", "--json"]));
    assert_eq!(negative["tasks"].as_array().unwrap().len(), 1);

    let miss = json(&run_in(tmp.path(), &["ls", "20260101-000000", "--json"]));
    assert!(miss["tasks"].as_array().unwrap().is_empty());
}

#[test]
fn update_refuses_to_rewrite_a_malformed_task() {
    let tmp = scratch();
    init(tmp.path());

    let task_dir = tmp.path().join("tasks/20260101-000000");
    std::fs::create_dir(&task_dir).unwrap();
    let path = task_dir.join("TASK.md");
    std::fs::write(&path, "not a heading\n\nprecious body\n").unwrap();

    let output = run_in(tmp.path(), &["ls", "-u", "close", "any"]);
    assert_eq!(output.status.code(), Some(3), "{}", stderr(&output));

    let after = std::fs::read_to_string(&path).unwrap();
    assert!(
        after.contains("precious body"),
        "body was damaged: {after:?}"
    );
}

#[test]
fn find_and_id_work_on_closed_tasks() {
    let tmp = scratch();
    init(tmp.path());
    let created = create(tmp.path(), "Closed one", &["bug"], 10);
    let id = created["id"].as_str().unwrap().to_owned();
    let rel = created["path"].as_str().unwrap();

    run_in(tmp.path(), &["ls", "-u", "close", &id]);

    let found = json(&run_in(tmp.path(), &["find", &id, "--json"]));
    assert_eq!(found["status"], "CLOSED");

    let path_only = run_in(tmp.path(), &["find", &id, "--path-only"]);
    assert_eq!(stdout(&path_only).trim(), rel);

    let task_dir = tmp.path().join(rel).parent().unwrap().to_path_buf();
    let inside = run_in(&task_dir, &["id"]);
    assert_eq!(stdout(&inside).trim(), id);

    let outside = run_in(tmp.path(), &["id"]);
    assert_eq!(outside.status.code(), Some(2));
}

#[test]
fn ref_finds_references_and_can_fail_when_empty() {
    let tmp = scratch();
    init(tmp.path());
    let created = create(tmp.path(), "Referenced", &["bug"], 10);
    let id = created["id"].as_str().unwrap().to_owned();

    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(
        tmp.path().join("src/lib.rs"),
        format!("// see TASK({id}): details\n"),
    )
    .unwrap();

    let output = json(&run_in(tmp.path(), &["ref", &id, "--json"]));
    assert_eq!(output["huid"], id);
    let references = output["references"].as_array().unwrap();
    assert_eq!(references.len(), 1);
    assert_eq!(references[0]["path"], "src/lib.rs");
    assert_eq!(references[0]["line"], 1);

    let missing = run_in(tmp.path(), &["ref", "20260101-000000", "--fail-if-empty"]);
    assert_eq!(missing.status.code(), Some(5));
}

#[test]
fn summary_reports_open_then_closed() {
    let tmp = scratch();
    init(tmp.path());
    create(tmp.path(), "A", &["bug"], 10);
    let b = create(tmp.path(), "B", &["bug", "ui"], 20);
    run_in(
        tmp.path(),
        &["ls", "-u", "close", b["id"].as_str().unwrap()],
    );

    let open = json(&run_in(tmp.path(), &["summary", "--json"]));
    assert_eq!(open["status"], "OPEN");
    assert_eq!(open["total"], 1);
    assert_eq!(open["tags"][0]["tag"], "bug");
    assert_eq!(open["tags"][0]["count"], 1);

    let closed = json(&run_in(tmp.path(), &["summary", "-c", "--json"]));
    assert_eq!(closed["status"], "CLOSED");
    assert_eq!(closed["total"], 1);
    assert_eq!(closed["priorities"][0]["priority"], 20);
}

#[test]
fn todo_previews_then_converts_and_is_idempotent() {
    let tmp = scratch();
    init(tmp.path());
    let file = tmp.path().join("main.c");
    let original = "int main(void) {\n    // TODO: first\n    // TODO: second\n}\n";
    std::fs::write(&file, original).unwrap();

    let preview = run_in(tmp.path(), &["todo", "main.c"]);
    assert!(preview.status.success(), "{}", stderr(&preview));
    assert!(stdout(&preview).contains("would create 2 task(s)"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);

    let blocked = run_in(tmp.path(), &["todo", "main.c", "-y", "--read-only"]);
    assert_eq!(blocked.status.code(), Some(2));

    let written = json(&run_in(tmp.path(), &["todo", "main.c", "-y", "--json"]));
    assert_eq!(written["created"].as_array().unwrap().len(), 2);

    let rewritten = std::fs::read_to_string(&file).unwrap();
    assert!(rewritten.contains("TASK("), "{rewritten}");
    assert!(!rewritten.contains("TODO:"), "{rewritten}");

    let again = json(&run_in(tmp.path(), &["todo", "main.c", "-y", "--json"]));
    assert!(again["created"].as_array().unwrap().is_empty());
}

#[test]
fn invalid_huid_argument_is_a_usage_error() {
    let tmp = scratch();
    init(tmp.path());

    let output = run_in(tmp.path(), &["show", "not-an-id"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("not a task id"));
}
