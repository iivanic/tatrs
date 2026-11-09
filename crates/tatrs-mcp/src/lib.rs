//! Minimal MCP (Model Context Protocol) server over stdio.
//!
//! Newline-delimited JSON-RPC 2.0, as used by MCP's stdio transport. Read-only
//! tools are always available; mutating tools appear only when the host passed
//! `--allow-write`. Every tool returns the same schema-1 JSON the CLI prints.

use std::io::{BufRead, Write};
use std::time::Duration;

use serde_json::{Value, json};
use tatrs_core::claims::ClaimStore;
use tatrs_core::json as core_json;
use tatrs_core::ready::evaluate;
use tatrs_core::task::{DEFAULT_PRIORITY, normalize_body};
use tatrs_core::{Huid, NewTask, Query, Repo, StoreError, Task, time};

/// Protocol revision advertised when the client does not ask for one.
const PROTOCOL_VERSION: &str = "2025-06-18";

const WRITE_TOOLS: [&str; 3] = ["create_task", "claim_task", "release_task"];

pub struct Server {
    repo: Repo,
    allow_write: bool,
}

impl Server {
    pub fn new(repo: Repo, allow_write: bool) -> Self {
        Self { repo, allow_write }
    }

    pub fn allow_write(&self) -> bool {
        self.allow_write
    }

    /// Handle one JSON-RPC message. Returns `None` for notifications.
    pub fn handle(&self, request: &Value) -> Option<Value> {
        let has_id = request.get("id").is_some();
        let id = request.get("id").cloned().unwrap_or(Value::Null);

        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return has_id.then(|| error(id, -32600, "invalid request: missing method"));
        };
        let params = request.get("params").cloned().unwrap_or(Value::Null);

        if !has_id {
            // Notifications (e.g. `notifications/initialized`) never get a reply.
            return None;
        }

        let response = match method {
            "initialize" => {
                let requested = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(PROTOCOL_VERSION);
                ok(
                    id,
                    json!({
                        "protocolVersion": requested,
                        "capabilities": {"tools": {"listChanged": false}},
                        "serverInfo": {"name": "tatrs", "version": env!("CARGO_PKG_VERSION")},
                        "instructions": "Tasks are directories under tasks/. Call claim_task before working and release_task when finished. Leases are local to this clone.",
                    }),
                )
            }
            "ping" => ok(id, json!({})),
            "tools/list" => ok(id, json!({ "tools": self.tool_definitions() })),
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return Some(error(id, -32602, "tools/call requires `name`"));
                };
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match self.call_tool(name, &arguments) {
                    Ok(payload) => ok(id, tool_result(&payload)),
                    Err(message) => ok(id, tool_failure(&message)),
                }
            }
            _ => error(id, -32601, format!("method `{method}` is not supported")),
        };

        Some(response)
    }

    /// Read newline-delimited JSON-RPC from `input`, writing responses to
    /// `output`. Returns when the input reaches EOF.
    pub fn serve<R: BufRead, W: Write>(&self, input: R, mut output: W) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let request: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(err) => {
                    let response = error(Value::Null, -32700, format!("parse error: {err}"));
                    writeln!(output, "{response}")?;
                    output.flush()?;
                    continue;
                }
            };

            if let Some(response) = self.handle(&request) {
                writeln!(output, "{response}")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    fn call_tool(&self, name: &str, args: &Value) -> Result<Value, String> {
        if WRITE_TOOLS.contains(&name) && !self.allow_write {
            return Err(format!(
                "tool `{name}` is disabled; start the server with `tatrs mcp --allow-write`"
            ));
        }

        match name {
            "list_tasks" => self.list_tasks(args),
            "get_task" => self.get_task(args),
            "create_task" => self.create_task(args),
            "claim_task" => self.claim_task(args),
            "release_task" => self.release_task(args),
            _ => Err(format!("unknown tool `{name}`")),
        }
    }

    fn list_tasks(&self, args: &Value) -> Result<Value, String> {
        let status = args.get("status").and_then(Value::as_str).unwrap_or("open");
        if !matches!(status, "open" | "closed" | "all") {
            return Err("`status` must be `open`, `closed`, or `all`".to_owned());
        }

        let tag = args.get("tag").and_then(Value::as_str);
        let query_source = args.get("query").and_then(Value::as_str);
        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .map(|limit| limit as usize);

        let query = match query_source {
            Some(source) => Query::compile(source).map_err(|err| err.message)?,
            None => Query::any(),
        };

        let loaded = self.repo.load();
        let mut tasks: Vec<&Task> = loaded
            .tasks
            .iter()
            .filter(|task| match status {
                "all" => true,
                "closed" => task.is_closed(),
                _ => !task.is_closed(),
            })
            .filter(|task| tag.is_none_or(|tag| task.tags.iter().any(|value| value == tag)))
            .filter(|task| query.matches(task))
            .collect();
        tasks.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        if let Some(limit) = limit {
            tasks.truncate(limit);
        }

        let mut payload = core_json::tasks_json(
            tasks
                .iter()
                .map(|task| (*task, self.repo.rel_task_file(&task.id))),
        );
        if !loaded.diagnostics.is_empty()
            && let Some(object) = payload.as_object_mut()
        {
            let diagnostics: Vec<Value> = loaded
                .diagnostics
                .iter()
                .map(|diagnostic| {
                    diagnostic.to_json(std::path::Path::new(&self.repo.rel_path(&diagnostic.path)))
                })
                .collect();
            object.insert("diagnostics".to_owned(), Value::Array(diagnostics));
        }

        Ok(payload)
    }

    fn get_task(&self, args: &Value) -> Result<Value, String> {
        let id = required_huid(args, "id")?;
        let task = self.repo.load_task(&id).map_err(|err| err.to_string())?;
        Ok(core_json::task_json(&task, &self.repo.rel_task_file(&id)))
    }

    fn create_task(&self, args: &Value) -> Result<Value, String> {
        let title = required_str(args, "title")?;
        let mut spec = NewTask::new(title);

        if let Some(tags) = args.get("tags") {
            let tags = tags
                .as_array()
                .ok_or_else(|| "`tags` must be an array of strings".to_owned())?;
            spec.tags = tags
                .iter()
                .map(|tag| {
                    tag.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "`tags` must be an array of strings".to_owned())
                })
                .collect::<Result<_, _>>()?;
        }

        spec.priority = args
            .get("priority")
            .and_then(Value::as_i64)
            .unwrap_or(DEFAULT_PRIORITY);
        spec.suffix = args
            .get("suffix")
            .and_then(Value::as_str)
            .map(str::to_owned);
        spec.body = normalize_body(args.get("body").and_then(Value::as_str).unwrap_or(""));
        if let Some(raw) = args.get("id").and_then(Value::as_str) {
            spec.id = Some(Huid::parse(raw).map_err(|err| err.to_string())?);
        }

        match self.repo.create(&spec) {
            Ok(task) => Ok(core_json::task_json(
                &task,
                &self.repo.rel_task_file(&task.id),
            )),
            // Deterministic ids are idempotency keys, like `tatrs new --id`.
            Err(StoreError::Exists(id)) if spec.id.is_some() => {
                let task = self.repo.load_task(&id).map_err(|err| err.to_string())?;
                Ok(core_json::task_json(&task, &self.repo.rel_task_file(&id)))
            }
            Err(err) => Err(err.to_string()),
        }
    }

    fn claim_task(&self, args: &Value) -> Result<Value, String> {
        let id = required_huid(args, "id")?;
        let owner = args.get("owner").and_then(Value::as_str).unwrap_or("mcp");
        let ttl_source = args.get("ttl").and_then(Value::as_str).unwrap_or("2h");
        let ttl = time::parse_duration(ttl_source)
            .ok_or_else(|| format!("invalid `ttl` `{ttl_source}`; use 30s, 15m, 2h, or 1d"))?;
        let run = args.get("run").and_then(Value::as_str).map(str::to_owned);

        let loaded = self.repo.load();
        let task = loaded
            .tasks
            .iter()
            .find(|task| task.id == id)
            .cloned()
            .ok_or_else(|| format!("task {id} was not found"))?;

        let readiness = evaluate(&task, &loaded.tasks, None, time::now_secs());
        if !readiness.ready {
            return Err(format!(
                "task {id} is not ready: {}",
                readiness.blockers.join("; ")
            ));
        }

        let store = ClaimStore::locate(&self.repo);
        let claim = store
            .claim(&id, owner, Duration::from_secs(ttl), run)
            .map_err(|err| err.to_string())?;

        Ok(json!({
            "schema": core_json::SCHEMA_VERSION,
            "claimed": true,
            "task": claim.task,
            "owner": claim.owner,
            "token": claim.token,
            "acquired_at": claim.acquired_at,
            "expires_at": claim.expires_at,
            "run": claim.run,
        }))
    }

    fn release_task(&self, args: &Value) -> Result<Value, String> {
        let id = required_huid(args, "id")?;
        let token = required_str(args, "token")?;
        let store = ClaimStore::locate(&self.repo);
        store.release(&id, token).map_err(|err| err.to_string())?;
        Ok(json!({
            "schema": core_json::SCHEMA_VERSION,
            "released": true,
            "task": id.as_str(),
        }))
    }

    fn tool_definitions(&self) -> Vec<Value> {
        let mut tools = vec![
            json!({
                "name": "list_tasks",
                "description": "List tasks, optionally filtered by status, tag, and a TQL query. Returns the schema-1 list payload.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "status": {"type": "string", "enum": ["open", "closed", "all"], "default": "open"},
                        "tag": {"type": "string"},
                        "query": {"type": "string", "description": "TQL expression, e.g. `:bug and priority lt 50`"},
                        "limit": {"type": "integer", "minimum": 1}
                    },
                    "additionalProperties": false
                }
            }),
            json!({
                "name": "get_task",
                "description": "Fetch one task, including its body and acceptance state.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"id": {"type": "string"}},
                    "required": ["id"],
                    "additionalProperties": false
                }
            }),
        ];

        if self.allow_write {
            tools.push(json!({
                "name": "create_task",
                "description": "Create a task. Pass `body` with an `## Acceptance` section for agent-ready work.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                        "tags": {"type": "array", "items": {"type": "string"}},
                        "priority": {"type": "integer"},
                        "body": {"type": "string"},
                        "suffix": {"type": "string"},
                        "id": {"type": "string", "description": "Deterministic HUID; repeats are a no-op"}
                    },
                    "required": ["title"],
                    "additionalProperties": false
                }
            }));
            tools.push(json!({
                "name": "claim_task",
                "description": "Acquire a lease on a ready task. Returns the token needed to release it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "owner": {"type": "string", "default": "mcp"},
                        "ttl": {"type": "string", "default": "2h"},
                        "run": {"type": "string"}
                    },
                    "required": ["id"],
                    "additionalProperties": false
                }
            }));
            tools.push(json!({
                "name": "release_task",
                "description": "Release a lease with the token returned by claim_task.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "token": {"type": "string"}
                    },
                    "required": ["id", "token"],
                    "additionalProperties": false
                }
            }));
        }

        tools
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message.into()},
    })
}

fn tool_result(payload: &Value) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(payload).unwrap_or_else(|_| payload.to_string()),
        }],
    })
}

fn tool_failure(message: &str) -> Value {
    json!({
        "content": [{"type": "text", "text": message}],
        "isError": true,
    })
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("`{key}` is required and must be a string"))
}

fn required_huid(args: &Value, key: &str) -> Result<Huid, String> {
    let raw = required_str(args, key)?;
    Huid::parse(raw).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> tempfile::TempDir {
        let base =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-scratch");
        std::fs::create_dir_all(&base).unwrap();
        tempfile::Builder::new()
            .prefix("mcp-")
            .tempdir_in(&base)
            .unwrap()
    }

    fn fresh_repo() -> (tempfile::TempDir, Repo) {
        let tmp = scratch();
        tatrs_core::store::init(tmp.path()).unwrap();
        let repo = Repo::open(tmp.path()).unwrap();
        (tmp, repo)
    }

    fn server(repo: Repo, write: bool) -> Server {
        Server::new(repo, write)
    }

    fn call(server: &Server, name: &str, arguments: Value) -> Value {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        });
        server.handle(&request).unwrap()
    }

    fn payload(response: &Value) -> Value {
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    fn create_agent_task(repo: &Repo) -> Huid {
        let mut spec = NewTask::new("Ready work");
        spec.tags = vec!["agent".to_owned()];
        spec.body = normalize_body("## Acceptance\n\n- [ ] done");
        repo.create(&spec).unwrap().id
    }

    #[test]
    fn initialize_reports_tools_capability() {
        let (_tmp, repo) = fresh_repo();
        let response = server(repo, false)
            .handle(&json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test"}},
            }))
            .unwrap();
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(response["result"]["serverInfo"]["name"], "tatrs");
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn write_tools_are_hidden_until_allowed() {
        let (_tmp, repo) = fresh_repo();
        let response = server(repo, false)
            .handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}))
            .unwrap();
        let names: Vec<&str> = response["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["list_tasks", "get_task"]);

        let (_tmp2, repo2) = fresh_repo();
        let response = server(repo2, true)
            .handle(&json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}))
            .unwrap();
        let names: Vec<&str> = response["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.len(), 2 + WRITE_TOOLS.len());
    }

    #[test]
    fn list_tasks_filters_by_status() {
        let (_tmp, repo) = fresh_repo();
        let mut closed = NewTask::new("Old");
        closed.tags = vec!["bug".to_owned()];
        let closed = repo.create(&closed).unwrap();
        let mut task = closed.clone();
        task.close();
        repo.save(&task).unwrap();
        repo.create(&NewTask::new("Fresh")).unwrap();

        let server = server(repo, false);
        let open = payload(&call(&server, "list_tasks", json!({})));
        assert_eq!(open["tasks"].as_array().unwrap().len(), 1);

        let all = payload(&call(&server, "list_tasks", json!({"status": "all"})));
        assert_eq!(all["tasks"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn write_tools_are_gated_and_idempotent() {
        let (_tmp, repo) = fresh_repo();
        let read_only = server(repo.clone(), false);
        let denied = call(
            &read_only,
            "create_task",
            json!({"title": "Gated", "id": "20261109-120000"}),
        );
        assert_eq!(denied["result"]["isError"], true);

        let writer = server(repo, true);
        let created = payload(&call(
            &writer,
            "create_task",
            json!({"title": "Gated", "body": "## Acceptance\n\n- [ ] x", "id": "20261109-120000"}),
        ));
        assert_eq!(created["title"], "Gated");

        let again = payload(&call(
            &writer,
            "create_task",
            json!({"title": "Gated", "id": "20261109-120000"}),
        ));
        assert_eq!(again["id"], created["id"]);

        let fetched = payload(&call(&writer, "get_task", json!({"id": "20261109-120000"})));
        assert_eq!(fetched["acceptance"]["criteria"], 1);
    }

    #[test]
    fn claim_and_release_round_trip() {
        let (_tmp, repo) = fresh_repo();
        let id = create_agent_task(&repo);
        let server = server(repo, true);

        let claimed = payload(&call(
            &server,
            "claim_task",
            json!({"id": id.as_str(), "owner": "agent-3", "run": "r1"}),
        ));
        assert_eq!(claimed["claimed"], true);
        let token = claimed["token"].as_str().unwrap().to_owned();

        let second = call(&server, "claim_task", json!({"id": id.as_str()}));
        assert_eq!(second["result"]["isError"], true);

        let released = payload(&call(
            &server,
            "release_task",
            json!({"id": id.as_str(), "token": token}),
        ));
        assert_eq!(released["released"], true);
    }

    #[test]
    fn unknown_methods_are_protocol_errors() {
        let (_tmp, repo) = fresh_repo();
        let response = server(repo, false)
            .handle(&json!({"jsonrpc": "2.0", "id": 9, "method": "nope"}))
            .unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn serve_handles_notifications_and_requests() {
        let (_tmp, repo) = fresh_repo();
        let server = server(repo, false);
        let input = concat!(
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n",
            "not json\n",
        );
        let mut output = Vec::new();
        server
            .serve(std::io::Cursor::new(input), &mut output)
            .unwrap();
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines.len(),
            2,
            "notification must not be answered: {lines:?}"
        );
        assert!(lines[0].contains("\"result\""));
        assert!(lines[1].contains("-32700"));
    }
}
