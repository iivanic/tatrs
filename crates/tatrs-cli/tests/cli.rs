use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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
    run_in(
        tmp.path(),
        &["new", "Clean task", "--body", "## Acceptance\n\n- [ ] done"],
    );
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
fn ref_respects_gitignore_and_no_ignore_scans_everything() {
    let tmp = scratch();
    init(tmp.path());
    let created = create(tmp.path(), "Ignored reference", &["bug"], 10);
    let id = created["id"].as_str().unwrap().to_owned();

    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(
        tmp.path().join("src/generated.c"),
        format!("// TASK({id})\n"),
    )
    .unwrap();
    std::fs::write(tmp.path().join(".gitignore"), "src/\n").unwrap();

    let ignored = json(&run_in(tmp.path(), &["ref", &id, "--json"]));
    assert_eq!(ignored["references"].as_array().unwrap().len(), 0);

    let all = json(&run_in(tmp.path(), &["ref", &id, "--json", "--no-ignore"]));
    let references = all["references"].as_array().unwrap();
    assert_eq!(references.len(), 1);
    assert_eq!(references[0]["path"], "src/generated.c");
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

fn create_agent(dir: &Path, title: &str, acceptance: Option<&str>) -> Value {
    let mut args: Vec<String> = vec![
        "new".into(),
        title.into(),
        "-t".into(),
        "agent".into(),
        "--json".into(),
    ];
    if let Some(body) = acceptance {
        args.push("--body".into());
        args.push(body.to_owned());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = run_in(dir, &refs);
    assert!(output.status.success(), "{}", stderr(&output));
    json(&output)
}

fn claim_task(dir: &Path, id: &str, extra: &[&str]) -> Value {
    let mut args = vec!["claim", id, "--json"];
    if !extra.contains(&"--owner") {
        args.extend_from_slice(&["--owner", "agent-3"]);
    }
    args.extend_from_slice(extra);
    json(&run_in(dir, &args))
}

#[test]
fn ready_lists_only_dispatchable_tasks() {
    let tmp = scratch();
    init(tmp.path());
    let ready_id = create_agent(
        tmp.path(),
        "Agent ready",
        Some("## Acceptance\n\n- [ ] tests pass"),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    create_agent(tmp.path(), "Agent without criteria", None);
    create(tmp.path(), "Human task", &["bug"], 10);

    let ready = json(&run_in(tmp.path(), &["ready", "--json"]));
    let tasks = ready["ready"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], ready_id);
    assert_eq!(tasks[0]["acceptance"]["criteria"], 1);

    let explained = json(&run_in(tmp.path(), &["ready", "--json", "--explain"]));
    assert!(explained["excluded"].as_array().unwrap().len() >= 2);
}

#[test]
fn claim_ready_release_round_trip() {
    let tmp = scratch();
    init(tmp.path());
    let id = create_agent(tmp.path(), "Claim me", Some("## Acceptance\n\n- [ ] done"))["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let claim = claim_task(tmp.path(), &id, &["--run", "run-1"]);
    assert_eq!(claim["claimed"], true);
    assert_eq!(claim["owner"], "agent-3");
    assert_eq!(claim["run"], "run-1");
    let token = claim["token"].as_str().unwrap().to_owned();

    let second = run_in(tmp.path(), &["claim", &id, "--owner", "agent-4"]);
    assert_eq!(second.status.code(), Some(4), "{}", stderr(&second));

    let ready = json(&run_in(tmp.path(), &["ready", "--json"]));
    assert!(ready["ready"].as_array().unwrap().is_empty());

    let status = json(&run_in(tmp.path(), &["claim", "--status", "--json"]));
    assert_eq!(status["claims"][0]["state"], "live");
    assert_eq!(status["claims"][0]["owner"], "agent-3");

    let wrong = run_in(tmp.path(), &["release", &id, "--token", "wrong"]);
    assert_eq!(wrong.status.code(), Some(4));

    let released = run_in(tmp.path(), &["release", &id, "--token", &token]);
    assert!(released.status.success(), "{}", stderr(&released));

    let ready = json(&run_in(tmp.path(), &["ready", "--json"]));
    assert_eq!(ready["ready"].as_array().unwrap().len(), 1);
}

#[test]
fn expired_claims_are_visible_and_stealable() {
    let tmp = scratch();
    init(tmp.path());
    let id = create_agent(
        tmp.path(),
        "Short lease",
        Some("## Acceptance\n\n- [ ] done"),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let first = claim_task(tmp.path(), &id, &["--ttl", "0s"]);
    assert_eq!(first["claimed"], true);

    let status = json(&run_in(tmp.path(), &["claim", "--status", "--json"]));
    assert_eq!(status["claims"][0]["state"], "expired");

    let ready = json(&run_in(tmp.path(), &["ready", "--json"]));
    assert_eq!(ready["ready"].as_array().unwrap().len(), 1);

    let stolen = claim_task(tmp.path(), &id, &["--owner", "agent-4"]);
    assert_eq!(stolen["owner"], "agent-4");
}

#[test]
fn claim_refuses_tasks_that_are_not_ready() {
    let tmp = scratch();
    init(tmp.path());
    let human = create(tmp.path(), "Human task", &["bug"], 10);
    let output = run_in(tmp.path(), &["claim", human["id"].as_str().unwrap()]);
    assert_eq!(output.status.code(), Some(3), "{}", stderr(&output));
    assert!(stderr(&output).contains("agent"), "{}", stderr(&output));
}

#[test]
fn agent_tasks_without_criteria_fail_check() {
    let tmp = scratch();
    init(tmp.path());
    create_agent(tmp.path(), "No criteria", None);

    let output = run_in(tmp.path(), &["check", "--json"]);
    assert_eq!(output.status.code(), Some(3));
    let report = json(&output);
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "E0301")
    );

    create_agent(
        tmp.path(),
        "With criteria",
        Some("## Acceptance\n\n- [ ] tests pass"),
    );
    let clean = run_in(tmp.path(), &["check", "--json"]);
    assert_eq!(
        clean.status.code(),
        Some(3),
        "the first agent task still fails"
    );
}

#[test]
fn refresh_requires_the_token() {
    let tmp = scratch();
    init(tmp.path());
    let id = create_agent(tmp.path(), "Heartbeat", Some("## Acceptance\n\n- [ ] done"))["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let claim = claim_task(tmp.path(), &id, &[]);
    let token = claim["token"].as_str().unwrap().to_owned();

    let refreshed = json(&run_in(
        tmp.path(),
        &["claim", &id, "--token", &token, "--ttl", "3h", "--json"],
    ));
    assert_eq!(refreshed["token"], token);

    let wrong = run_in(
        tmp.path(),
        &["claim", &id, "--token", "wrong", "--ttl", "3h"],
    );
    assert_eq!(wrong.status.code(), Some(4));
}

#[test]
fn check_fix_removes_stale_and_corrupt_claims() {
    let tmp = scratch();
    init(tmp.path());
    let id = create_agent(tmp.path(), "Stale", Some("## Acceptance\n\n- [ ] done"))["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let claims_dir = tmp.path().join("tasks/.tatrs/claims");
    std::fs::create_dir_all(&claims_dir).unwrap();
    std::fs::write(
        claims_dir.join(format!("{id}.json")),
        serde_json::json!({
            "task": id,
            "owner": "agent-3",
            "token": "t",
            "acquired_at": "2020-01-01T00:00:00Z",
            "expires_at": "2020-01-02T00:00:00Z",
            "host": "desk",
            "pid": 1,
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(claims_dir.join("junk.json"), "not json").unwrap();

    let before = json(&run_in(tmp.path(), &["check", "--json"]));
    let codes: Vec<&str> = before["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|diagnostic| diagnostic["code"].as_str())
        .collect();
    assert!(codes.contains(&"W0402"), "{codes:?}");
    assert!(codes.contains(&"W0403"), "{codes:?}");

    let blocked = run_in(tmp.path(), &["check", "--fix", "--read-only"]);
    assert_eq!(blocked.status.code(), Some(2));

    let fixed = run_in(tmp.path(), &["check", "--fix"]);
    assert!(fixed.status.success(), "{}", stderr(&fixed));

    let leftover: Vec<_> = std::fs::read_dir(&claims_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(leftover.is_empty(), "{leftover:?} should have been removed");

    let after = json(&run_in(tmp.path(), &["check", "--json"]));
    let codes: Vec<&str> = after["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|diagnostic| diagnostic["code"].as_str())
        .collect();
    assert!(!codes.contains(&"W0402"), "{codes:?}");
    assert!(!codes.contains(&"W0403"), "{codes:?}");
}

#[test]
fn mcp_serves_json_rpc_over_stdio() {
    let tmp = scratch();
    init(tmp.path());
    let task = create_agent(tmp.path(), "MCP task", Some("## Acceptance\n\n- [ ] done"));
    let id = task["id"].as_str().unwrap().to_owned();

    let mut child = tatrs()
        .current_dir(tmp.path())
        .args(["mcp", "--allow-write"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        for message in [
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test"}},
            }),
            serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            serde_json::json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "list_tasks", "arguments": {"status": "all"}},
            }),
        ] {
            writeln!(stdin, "{message}").unwrap();
        }
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "server failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = String::from_utf8(output.stdout).unwrap();
    let responses: Vec<Value> = body
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 3, "{responses:?}");
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "tatrs");
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "create_task")
    );

    let listed: Value = serde_json::from_str(
        responses[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(listed["tasks"][0]["id"], id);
}

#[test]
fn completions_and_man_pages_are_generated() {
    let tmp = scratch();
    init(tmp.path());

    let bash = run_in(tmp.path(), &["completions", "bash"]);
    assert!(bash.status.success(), "{}", stderr(&bash));
    assert!(stdout(&bash).contains("tatrs"));

    let man = run_in(tmp.path(), &["man"]);
    assert!(man.status.success(), "{}", stderr(&man));
    assert!(stdout(&man).contains(".TH"), "{}", stdout(&man));
}

#[test]
fn invalid_huid_argument_is_a_usage_error() {
    let tmp = scratch();
    init(tmp.path());

    let output = run_in(tmp.path(), &["show", "not-an-id"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("not a task id"));
}

#[test]
fn summary_reports_derived_lifecycle() {
    let tmp = scratch();
    init(tmp.path());
    create(tmp.path(), "Received", &["bug"], 10);
    create(tmp.path(), "Paused", &["paused"], 10);
    create(tmp.path(), "Waiting", &["blocked"], 10);

    let done = create(tmp.path(), "Done", &[], 10);
    run_in(
        tmp.path(),
        &["ls", "-u", "close", done["id"].as_str().unwrap()],
    );
    let refused = create(tmp.path(), "Refused", &["refused"], 10);
    run_in(
        tmp.path(),
        &["ls", "-u", "close", refused["id"].as_str().unwrap()],
    );

    let summary = json(&run_in(tmp.path(), &["summary", "--json"]));
    let lifecycle = &summary["lifecycle"];
    assert_eq!(lifecycle["total"], 5);
    assert_eq!(lifecycle["received"], 1);
    assert_eq!(lifecycle["paused"], 1);
    assert_eq!(lifecycle["waiting"], 1);
    assert_eq!(lifecycle["completed"], 1);
    assert_eq!(lifecycle["refused"], 1);
    assert_eq!(lifecycle["ready"], 0);

    // Lifecycle is derived, so closing a task moves it without touching tags.
    let waiting = run_in(tmp.path(), &["ls", ":blocked", "--json"]);
    let waiting_id = json(&waiting)["tasks"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    run_in(tmp.path(), &["ls", "-u", "close", &waiting_id]);
    let after = json(&run_in(tmp.path(), &["summary", "--json"]));
    assert_eq!(after["lifecycle"]["waiting"], 0);
    assert_eq!(after["lifecycle"]["completed"], 2);
}

fn http_get(addr: &str, target: &str) -> String {
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    write!(
        stream,
        "GET {target} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    // Image bodies are not UTF-8; headers and text assertions still work.
    String::from_utf8_lossy(&response).into_owned()
}

fn http_json(response: &str) -> Value {
    let (head, body) = response.split_once("\r\n\r\n").expect("no headers");
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    serde_json::from_str(body).unwrap_or_else(|err| panic!("invalid json: {err}\n{body}"))
}

#[test]
fn serve_exposes_a_read_only_board() {
    let tmp = scratch();
    init(tmp.path());
    create(tmp.path(), "Paused task", &["paused"], 10);

    let mut child = tatrs()
        .current_dir(tmp.path())
        .args(["serve", "--port", "0", "--poll", "200ms", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    let announced: Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(announced["schema"], 1);
    let url = announced["url"].as_str().unwrap().to_owned();
    let addr = url
        .strip_prefix("http://")
        .unwrap()
        .split('/')
        .next()
        .unwrap()
        .to_owned();
    let token = url.split("t=").nth(1).unwrap().to_owned();

    let unauthorized = http_get(&addr, "/api/status");
    assert!(unauthorized.starts_with("HTTP/1.1 401"), "{unauthorized}");

    let status = http_json(&http_get(&addr, &format!("/api/status?t={token}")));
    assert_eq!(status["schema"], 1);
    assert_eq!(status["counts"]["paused"], 1);
    assert_eq!(status["counts"]["total"], 1);
    assert_eq!(status["tasks"][0]["state"], "paused");
    assert_eq!(status["tasks"][0]["declared"], true);
    assert_eq!(status["tasks"][0]["also"], serde_json::json!([]));

    // Attachments next to TASK.md are listed and served by kind.
    let task_id = status["tasks"][0]["id"].as_str().unwrap().to_owned();
    let task_path = tmp
        .path()
        .join(status["tasks"][0]["path"].as_str().unwrap());
    let task_dir = task_path.parent().unwrap();
    std::fs::write(task_dir.join("notes.md"), "# note\n").unwrap();
    std::fs::write(
        task_dir.join("shot.gif"),
        b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;",
    )
    .unwrap();
    std::fs::write(task_dir.join("blob.bin"), [0xff, 0xfe]).unwrap();

    let detail = http_json(&http_get(&addr, &format!("/api/task/{task_id}?t={token}")));
    let files = detail["files"].as_array().unwrap();
    let names: Vec<&str> = files
        .iter()
        .map(|file| file["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["blob.bin", "notes.md", "shot.gif"]);
    assert_eq!(files[1]["kind"], "text");
    assert_eq!(files[2]["kind"], "image");

    let encoded = format!("tasks/{task_id}/shot.gif").replace('/', "%2F");
    let image = http_get(&addr, &format!("/api/file?path={encoded}&t={token}"));
    assert!(image.starts_with("HTTP/1.1 200"), "{image}");
    assert!(image.contains("Content-Type: image/gif"), "{image}");

    let encoded = format!("tasks/{task_id}/blob.bin").replace('/', "%2F");
    let binary = http_get(&addr, &format!("/api/file?path={encoded}&t={token}"));
    assert!(binary.starts_with("HTTP/1.1 415"), "{binary}");

    let page = http_get(&addr, &format!("/?t={token}"));
    assert!(page.contains("live board"), "{page}");
    assert!(page.contains("id=\"board-search\""), "search box missing");
    assert!(page.contains("class=\"sort\""), "sortable headers missing");
    assert!(
        page.contains("[\"received\", \"ready\", \"in_progress\""),
        "ready must be the second state"
    );
    assert!(
        page.contains("href=\"#/\" title=\"back to the board\""),
        "brand must link to the board"
    );

    let escape = http_get(
        &addr,
        &format!("/api/file?path=..%2F..%2Fetc%2Fpasswd&t={token}"),
    );
    assert!(escape.starts_with("HTTP/1.1 403"), "{escape}");

    // A change on disk bumps the version the SSE stream would carry.
    let task_file = tmp
        .path()
        .join(status["tasks"][0]["path"].as_str().unwrap());
    let original = std::fs::read_to_string(&task_file).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(400));
    std::fs::write(&task_file, &original).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(600));
    let updated = http_json(&http_get(&addr, &format!("/api/status?t={token}")));
    assert!(updated["version"].as_u64() > status["version"].as_u64());

    child.kill().unwrap();
    let _ = child.wait();
}
