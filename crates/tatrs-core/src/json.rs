//! Versioned JSON views. The schema is documented in `docs/JSON.md`; changes
//! must be additive and bump [`SCHEMA_VERSION`] only for breaking changes.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};

use crate::task::Task;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)]
struct AcceptanceView {
    present: bool,
    criteria: usize,
    waived: bool,
    ready: bool,
}

#[derive(Serialize)]
struct TaskView<'a> {
    schema: u32,
    id: &'a str,
    path: &'a str,
    title: &'a str,
    status: &'a str,
    priority: i64,
    tags: &'a [String],
    closed: bool,
    acceptance: AcceptanceView,
    props: BTreeMap<String, String>,
    body: &'a str,
}

/// One task, including the body. `rel_path` is `tasks/<id>/TASK.md`.
pub fn task_json(task: &Task, rel_path: &str) -> Value {
    let props: BTreeMap<String, String> = task
        .props
        .iter()
        .map(|property| (property.key.clone(), property.value.clone()))
        .collect();

    let acceptance = task.acceptance();
    let view = TaskView {
        schema: SCHEMA_VERSION,
        id: task.id.as_str(),
        path: rel_path,
        title: &task.title,
        status: &task.status,
        priority: task.priority,
        tags: &task.tags,
        closed: task.is_closed(),
        acceptance: AcceptanceView {
            present: acceptance.present,
            criteria: acceptance.criteria,
            waived: acceptance.waived,
            ready: task.status == "OPEN" && task.is_agent_task() && acceptance.satisfied(),
        },
        props,
        body: &task.body,
    };

    serde_json::to_value(view).unwrap_or(Value::Null)
}

/// A list payload: `{"schema":1,"tasks":[...]}`.
pub fn tasks_json<'a, I>(tasks: I) -> Value
where
    I: IntoIterator<Item = (&'a Task, String)>,
{
    let tasks: Vec<Value> = tasks
        .into_iter()
        .map(|(task, rel_path)| task_json(task, &rel_path))
        .collect();
    json!({ "schema": SCHEMA_VERSION, "tasks": tasks })
}

/// A check payload: `{"schema":1,"ok":bool,"tasks":n,"diagnostics":[...]}`.
pub fn check_json(ok: bool, task_count: usize, diagnostics: &[Value]) -> Value {
    json!({
        "schema": SCHEMA_VERSION,
        "ok": ok,
        "tasks": task_count,
        "errors": diagnostics.iter().filter(|d| d["severity"] == "error").count(),
        "warnings": diagnostics.iter().filter(|d| d["severity"] == "warning").count(),
        "diagnostics": diagnostics,
    })
}
