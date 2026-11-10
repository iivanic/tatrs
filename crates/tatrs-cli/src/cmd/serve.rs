//! `tatrs serve`: a read-only live board over HTTP.
//!
//! Design choices (see `docs/DASHBOARD.md`):
//!
//! - **Server-sent events, not WebSocket.** The board is one-way, so
//!   `text/event-stream` needs no framing protocol, no dependency, and the
//!   browser reconnects on its own.
//! - **Polling, not filesystem notifications.** A sweep stats every `TASK.md`
//!   and re-parses only changed files, which is portable, catches writes from
//!   other worktrees, and costs nothing when the tree is idle.
//!
//! The server binds loopback by default, requires a per-session capability
//! token, serves only files under the repository root, and never mutates.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};

use tatrs_core::claims::ClaimStore;
use tatrs_core::huid::Huid;
use tatrs_core::json as core_json;
use tatrs_core::lifecycle::{self, Context, Flag, Lifecycle, State};
use tatrs_core::{Claim, Repo, Task, time};

use crate::args::{Global, ServeArgs};
use crate::error::AppError;

const DASHBOARD: &str = include_str!("../../assets/dashboard.html");
const TOKEN_COOKIE: &str = "tatrs_token";
const MAX_TREE_ENTRIES: usize = 20_000;
/// Directories that are never browsable: build output and credential stores.
const BLOCKED: [&str; 7] = [
    ".git",
    "target",
    "node_modules",
    ".venv",
    "dist",
    "build",
    ".tatrs",
];

pub fn run(global: &Global, args: &ServeArgs) -> Result<(), AppError> {
    let repo = Repo::discover(&std::env::current_dir()?)?;

    let poll = poll_interval(&args.poll).ok_or_else(|| {
        AppError::Usage(format!(
            "--poll `{}` is not a duration of at least 100ms (try 500ms, 1s, 5s)",
            args.poll
        ))
    })?;
    let stale = time::parse_duration(&args.stale)
        .ok_or_else(|| AppError::Usage(format!("--stale `{}` is not a duration", args.stale)))?;
    let bind: IpAddr = args
        .bind
        .parse()
        .map_err(|_| AppError::Usage(format!("--bind `{}` must be an IP address", args.bind)))?;
    if !bind.is_loopback() && !args.allow_remote {
        return Err(AppError::Usage(format!(
            "refusing to bind {bind}: pass --allow-remote to expose the board beyond this machine"
        )));
    }

    let root = repo.root().canonicalize().map_err(|err| {
        AppError::Runtime(format!("cannot resolve {}: {err}", repo.root().display()))
    })?;
    let claims_dir = ClaimStore::locate(&repo).dir().to_path_buf();
    let stale_after = i64::try_from(stale).unwrap_or(i64::MAX);

    let mut watcher = Watcher::new(repo.clone(), claims_dir, stale_after);
    watcher.sweep();
    let (version, snapshot) = watcher.rebuild();

    let shared = Arc::new(Shared::new(version, snapshot));
    let watcher = Arc::new(Mutex::new(watcher));

    {
        let watcher = Arc::clone(&watcher);
        let shared = Arc::clone(&shared);
        thread::spawn(move || {
            loop {
                thread::sleep(poll);
                let rebuilt = match watcher.lock() {
                    Ok(mut watcher) => {
                        if !watcher.sweep() {
                            continue;
                        }
                        watcher.rebuild()
                    }
                    Err(_) => return,
                };
                shared.publish(rebuilt.0, rebuilt.1);
            }
        });
    }

    let listener = TcpListener::bind(SocketAddr::new(bind, args.port))
        .map_err(|err| AppError::Runtime(format!("cannot bind {bind}:{}: {err}", args.port)))?;
    let addr = listener
        .local_addr()
        .map_err(|err| AppError::Runtime(err.to_string()))?;
    let token = session_token();
    let url = format!("http://{addr}/?t={token}");

    if global.json {
        println!(
            "{}",
            json!({
                "schema": core_json::SCHEMA_VERSION,
                "url": url,
                "root": root.to_string_lossy(),
                "browse": !args.no_browse,
                "poll_ms": poll.as_millis(),
            })
        );
    } else if !global.quiet {
        println!("tatrs serve: {url}");
    }
    let _ = std::io::stdout().flush();

    let server = Arc::new(Server {
        repo,
        root,
        token,
        browse: !args.no_browse,
        max_file: args.max_file.saturating_mul(1024),
        watcher,
        shared,
    });

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let server = Arc::clone(&server);
        thread::spawn(move || handle(server, stream));
    }

    Ok(())
}

// ---------------------------------------------------------------- read model

/// Poll interval: `ms` suffix or the shared duration syntax, floored at 100ms.
fn poll_interval(text: &str) -> Option<Duration> {
    let text = text.trim();
    if let Some(millis) = text.strip_suffix("ms") {
        let amount: u64 = millis.trim().parse().ok()?;
        return (amount >= 100).then(|| Duration::from_millis(amount));
    }
    let seconds = time::parse_duration(text)?;
    (seconds >= 1).then(|| Duration::from_secs(seconds))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
}

fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    Some(Stamp {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn epoch_secs(modified: Option<SystemTime>) -> Option<i64> {
    let secs = modified?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(secs).ok()
}

/// Sub-second revision token for `TASK.md`; the UI refetches a task view when
/// this changes. Nanoseconds since the epoch fit in `u64` until the year 2554.
fn epoch_nanos(modified: Option<SystemTime>) -> Option<u64> {
    let nanos = modified?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    u64::try_from(nanos).ok()
}

/// One row of the board. Bodies are intentionally absent: they are fetched per
/// task so a snapshot stays small enough to broadcast.
#[derive(Serialize)]
struct Entry {
    id: String,
    title: String,
    state: State,
    also: Vec<State>,
    declared: bool,
    owner: Option<String>,
    run: Option<String>,
    since: Option<String>,
    /// `TASK.md` modification time in nanoseconds; changes on every edit, so a
    /// client can tell when its cached task view is stale.
    rev: Option<u64>,
    flags: Vec<Flag>,
    ready: bool,
    priority: i64,
    tags: Vec<String>,
    path: String,
}

impl Entry {
    fn new(repo: &Repo, task: &Task, lifecycle: &Lifecycle, rev: Option<u64>) -> Self {
        Self {
            id: task.id.as_str().to_owned(),
            title: task.title.clone(),
            state: lifecycle.state,
            also: lifecycle.also.clone(),
            declared: lifecycle.declared,
            owner: lifecycle.owner.clone(),
            run: lifecycle.run.clone(),
            since: lifecycle.since_iso(),
            rev,
            flags: lifecycle.flags.clone(),
            ready: lifecycle.ready,
            priority: task.priority,
            tags: task.tags.clone(),
            path: repo.rel_task_file(&task.id),
        }
    }
}

/// Incremental cache over `tasks/` and the claim store.
struct Watcher {
    repo: Repo,
    tasks_dir: PathBuf,
    claims_dir: PathBuf,
    stale_after: i64,
    tasks: Vec<Task>,
    task_stamps: BTreeMap<PathBuf, Stamp>,
    claim_stamps: BTreeMap<PathBuf, Stamp>,
    claims: BTreeMap<String, Claim>,
    activity: BTreeMap<String, i64>,
    revision: BTreeMap<String, u64>,
    version: u64,
}

impl Watcher {
    fn new(repo: Repo, claims_dir: PathBuf, stale_after: i64) -> Self {
        let tasks_dir = repo.tasks_dir();
        Self {
            repo,
            tasks_dir,
            claims_dir,
            stale_after,
            tasks: Vec::new(),
            task_stamps: BTreeMap::new(),
            claim_stamps: BTreeMap::new(),
            claims: BTreeMap::new(),
            activity: BTreeMap::new(),
            revision: BTreeMap::new(),
            version: 0,
        }
    }

    /// True when anything on disk changed since the previous sweep. Only then is
    /// the board recomputed, so an idle tree costs one `stat` per task per tick.
    fn sweep(&mut self) -> bool {
        let tasks = self.sweep_tasks();
        let claims = self.sweep_claims();
        tasks || claims
    }

    fn sweep_tasks(&mut self) -> bool {
        let mut changed = false;
        let mut seen: HashSet<String> = HashSet::new();
        let Ok(entries) = std::fs::read_dir(&self.tasks_dir) else {
            return false;
        };

        for entry in entries.flatten() {
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(id) = Huid::parse(&name) else { continue };
            let path = entry.path().join("TASK.md");
            let Some(file_stamp) = stamp(&path) else {
                continue;
            };
            seen.insert(name.clone());

            if self.task_stamps.get(&path) == Some(&file_stamp) {
                continue;
            }
            self.task_stamps.insert(path.clone(), file_stamp);

            let Ok(content) = std::fs::read_to_string(&path) else {
                // Do not cache the stamp: retry on the next sweep instead of
                // ignoring the file until it changes again.
                self.task_stamps.remove(&path);
                changed = true;
                continue;
            };
            let task = Task::parse(id.clone(), &content, &path);
            match self
                .tasks
                .binary_search_by(|candidate| candidate.id.cmp(&id))
            {
                Ok(index) => self.tasks[index] = task,
                Err(index) => self.tasks.insert(index, task),
            }
            if let Some(secs) = epoch_secs(file_stamp.modified) {
                self.activity.insert(name.clone(), secs);
            }
            if let Some(nanos) = epoch_nanos(file_stamp.modified) {
                self.revision.insert(name, nanos);
            }
            changed = true;
        }

        let before = self.tasks.len();
        self.tasks.retain(|task| seen.contains(task.id.as_str()));
        changed |= self.tasks.len() != before;

        let before = self.activity.len();
        self.activity.retain(|id, _| seen.contains(id));
        changed |= self.activity.len() != before;

        let before = self.revision.len();
        self.revision.retain(|id, _| seen.contains(id));
        changed |= self.revision.len() != before;

        let before = self.task_stamps.len();
        self.task_stamps.retain(|path, _| {
            path.parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .is_some_and(|id| seen.contains(id))
        });
        changed |= self.task_stamps.len() != before;
        changed
    }

    fn sweep_claims(&mut self) -> bool {
        let mut changed = false;
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let Ok(entries) = std::fs::read_dir(&self.claims_dir) else {
            let before = self.claims.len();
            self.claims.clear();
            self.claim_stamps.clear();
            return before != 0;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let Some(file_stamp) = stamp(&path) else {
                continue;
            };
            seen.insert(path.clone());
            if self.claim_stamps.get(&path) == Some(&file_stamp) {
                continue;
            }
            self.claim_stamps.insert(path.clone(), file_stamp);

            let parsed = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<Claim>(&text).ok());
            match parsed {
                Some(claim) => {
                    self.claims.insert(claim.task.clone(), claim);
                }
                None => {
                    if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                        self.claims.remove(stem);
                    }
                }
            }
            changed = true;
        }

        let before = self.claim_stamps.len();
        self.claim_stamps.retain(|path, _| seen.contains(path));
        changed |= self.claim_stamps.len() != before;

        let before = self.claims.len();
        self.claims
            .retain(|id, _| seen.contains(&self.claims_dir.join(format!("{id}.json"))));
        changed |= self.claims.len() != before;
        changed
    }

    /// Serialize a snapshot, bumping the version. Called only on change.
    fn rebuild(&mut self) -> (u64, String) {
        self.version += 1;
        let now = time::now_secs();
        let context = Context {
            now,
            last_activity: None,
            stale_after: self.stale_after,
        };
        let lifecycles =
            lifecycle::classify_all(&self.tasks, &self.claims, &self.activity, context);
        let counts = lifecycle::counts(&lifecycles);

        let mut entries: Vec<Entry> = self
            .tasks
            .iter()
            .zip(lifecycles.iter())
            .map(|(task, lifecycle)| {
                Entry::new(
                    &self.repo,
                    task,
                    lifecycle,
                    self.revision.get(task.id.as_str()).copied(),
                )
            })
            .collect();
        entries.sort_by(|left, right| {
            left.state
                .rank()
                .cmp(&right.state.rank())
                .then_with(|| right.priority.cmp(&left.priority))
                .then_with(|| left.id.cmp(&right.id))
        });

        let payload = json!({
            "schema": core_json::SCHEMA_VERSION,
            "version": self.version,
            "generated_at": time::format_iso(now),
            "counts": counts,
            "tasks": entries,
        });
        (self.version, payload.to_string())
    }

    /// One task plus its derivation, against the current dependency graph.
    fn detail(&self, id: &Huid) -> Option<(Task, Lifecycle)> {
        let index = self
            .tasks
            .binary_search_by(|candidate| candidate.id.cmp(id))
            .ok()?;
        let task = &self.tasks[index];
        let context = Context {
            now: time::now_secs(),
            last_activity: self.activity.get(id.as_str()).copied(),
            stale_after: self.stale_after,
        };
        let lifecycle =
            lifecycle::classify(task, &self.tasks, self.claims.get(id.as_str()), context);
        Some((task.clone(), lifecycle))
    }
}

// ------------------------------------------------------------ subscription

struct Published {
    version: u64,
    snapshot: Arc<String>,
}

struct Shared {
    state: Mutex<Published>,
    signal: Condvar,
}

impl Shared {
    fn new(version: u64, snapshot: String) -> Self {
        Self {
            state: Mutex::new(Published {
                version,
                snapshot: Arc::new(snapshot),
            }),
            signal: Condvar::new(),
        }
    }

    fn publish(&self, version: u64, snapshot: String) {
        match self.state.lock() {
            Ok(mut state) => {
                state.version = version;
                state.snapshot = Arc::new(snapshot);
            }
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.version = version;
                state.snapshot = Arc::new(snapshot);
            }
        }
        self.signal.notify_all();
    }

    fn current(&self) -> (u64, Arc<String>) {
        match self.state.lock() {
            Ok(state) => (state.version, Arc::clone(&state.snapshot)),
            Err(poisoned) => {
                let state = poisoned.into_inner();
                (state.version, Arc::clone(&state.snapshot))
            }
        }
    }
}

// ------------------------------------------------------------------- server

struct Server {
    repo: Repo,
    root: PathBuf,
    token: String,
    browse: bool,
    max_file: u64,
    watcher: Arc<Mutex<Watcher>>,
    shared: Arc<Shared>,
}

struct Request {
    method: String,
    path: String,
    query: String,
    cookie: Option<String>,
}

fn handle(server: Arc<Server>, mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Some(request) = read_request(&mut stream) else {
        return;
    };

    if request.method != "GET" {
        let _ = respond(
            &mut stream,
            405,
            &[("Allow", "GET")],
            "text/plain; charset=utf-8",
            b"method not allowed\n",
        );
        return;
    }

    if !authorized(&server, &request) {
        let _ = respond(
            &mut stream,
            401,
            &[],
            "text/plain; charset=utf-8",
            b"missing or invalid token; open the URL printed by `tatrs serve`\n",
        );
        return;
    }

    match request.path.as_str() {
        "/" | "/index.html" => {
            let cookie =
                if query_param(&request.query, "t").as_deref() == Some(server.token.as_str()) {
                    format!(
                        "{TOKEN_COOKIE}={}; HttpOnly; SameSite=Strict; Path=/",
                        server.token
                    )
                } else {
                    String::new()
                };
            let extra: Vec<(&str, &str)> = if cookie.is_empty() {
                Vec::new()
            } else {
                vec![("Set-Cookie", cookie.as_str())]
            };
            let _ = respond(
                &mut stream,
                200,
                &extra,
                "text/html; charset=utf-8",
                DASHBOARD.as_bytes(),
            );
        }
        "/events" => serve_events(stream, &server.shared),
        "/api/status" => {
            let (_, snapshot) = server.shared.current();
            let _ = respond(
                &mut stream,
                200,
                &[],
                "application/json",
                snapshot.as_bytes(),
            );
        }
        "/api/tree" if server.browse => respond_tree(&mut stream, &server),
        "/api/file" if server.browse => respond_file(&mut stream, &server, &request.query),
        "/favicon.ico" => {
            let _ = respond(&mut stream, 204, &[], "text/plain", b"");
        }
        path if path.starts_with("/api/task/") => {
            respond_task(&mut stream, &server, &path["/api/task/".len()..]);
        }
        _ => {
            let _ = respond(
                &mut stream,
                404,
                &[],
                "text/plain; charset=utf-8",
                b"not found\n",
            );
        }
    }
}

fn serve_events(mut stream: TcpStream, shared: &Shared) {
    let headers = "HTTP/1.1 200 OK\r\n\
        Content-Type: text/event-stream\r\n\
        Cache-Control: no-store\r\n\
        Connection: close\r\n\
        X-Accel-Buffering: no\r\n\r\n";
    if stream.write_all(headers.as_bytes()).is_err()
        || stream.write_all(b": connected\n\n").is_err()
    {
        return;
    }

    let mut seen = 0u64;
    loop {
        let (version, snapshot) = shared.current();
        if version != seen {
            seen = version;
            if write!(stream, "id: {version}\nevent: board\ndata: {snapshot}\n\n").is_err()
                || stream.flush().is_err()
            {
                return;
            }
            continue;
        }

        let state = match shared.state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        let (state, timeout) = match shared.signal.wait_timeout(state, Duration::from_secs(15)) {
            Ok(pair) => pair,
            Err(_) => return,
        };
        drop(state);

        if timeout.timed_out()
            && (stream.write_all(b": ping\n\n").is_err() || stream.flush().is_err())
        {
            return;
        }
    }
}

fn respond_task(stream: &mut TcpStream, server: &Server, raw_id: &str) {
    let Some(id) = percent_decode(raw_id).and_then(|id| Huid::parse(&id).ok()) else {
        let _ = respond(
            stream,
            400,
            &[],
            "text/plain; charset=utf-8",
            b"invalid task id\n",
        );
        return;
    };

    let detail = match server.watcher.lock() {
        Ok(watcher) => watcher.detail(&id),
        Err(_) => None,
    };
    let Some((task, lifecycle)) = detail else {
        let _ = respond(
            stream,
            404,
            &[],
            "text/plain; charset=utf-8",
            b"task not found\n",
        );
        return;
    };

    let mut payload = core_json::task_json(&task, &server.repo.rel_task_file(&id));
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "files".to_owned(),
            serde_json::to_value(task_files(&server.repo, &task)).unwrap_or(Value::Null),
        );
        object.insert(
            "lifecycle".to_owned(),
            json!({
                "state": lifecycle.state,
                "also": lifecycle.also,
                "declared": lifecycle.declared,
                "owner": lifecycle.owner,
                "run": lifecycle.run,
                "since": lifecycle.since_iso(),
                "flags": lifecycle.flags,
                "ready": lifecycle.ready,
            }),
        );
    }
    let body = payload.to_string();
    let _ = respond(stream, 200, &[], "application/json", body.as_bytes());
}

/// A file next to `TASK.md`: no content, just the metadata a link or thumbnail
/// needs.
#[derive(Debug, Serialize)]
struct TaskFile {
    name: String,
    path: String,
    size: u64,
    kind: &'static str,
}

/// Files in the task directory, excluding `TASK.md` and hidden entries. Read on
/// demand: attachments change rarely and keeping them out of the snapshot keeps
/// the broadcast small.
fn task_files(repo: &Repo, task: &Task) -> Vec<TaskFile> {
    let Some(dir) = task.path.parent() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<TaskFile> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "TASK.md" || name.starts_with('.') {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            Some(TaskFile {
                kind: file_kind(&name),
                path: repo.rel_path(&entry.path()),
                size: metadata.len(),
                name,
            })
        })
        .collect();
    files.sort_by(|left, right| left.name.cmp(&right.name));
    files
}

/// `image` for formats a browser renders inline, `text` for files the content
/// pane can show, `file` for everything else.
fn file_kind(name: &str) -> &'static str {
    match extension(name).as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" => "image",
        "md" | "markdown" | "txt" | "log" | "json" | "toml" | "yaml" | "yml" | "csv" => "text",
        _ => "file",
    }
}

/// Inline types only: SVG stays text, so a task attachment cannot execute
/// script in the board's origin.
fn image_content_type(name: &str) -> Option<&'static str> {
    match extension(name).as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default()
}

fn respond_tree(stream: &mut TcpStream, server: &Server) {
    let mut entries: Vec<Value> = Vec::new();
    let mut truncated = false;

    let walker = ignore::WalkBuilder::new(&server.root)
        .hidden(false)
        .git_global(false)
        .parents(false)
        .sort_by_file_name(|left, right| left.cmp(right))
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !BLOCKED
                .iter()
                .any(|blocked| name.eq_ignore_ascii_case(blocked))
        })
        .build();

    for item in walker {
        let Ok(item) = item else { continue };
        if item.path() == server.root {
            continue;
        }
        let Ok(relative) = item.path().strip_prefix(&server.root) else {
            continue;
        };
        if entries.len() >= MAX_TREE_ENTRIES {
            truncated = true;
            break;
        }
        let is_dir = item.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        entries.push(json!({
            "path": relative.to_string_lossy().replace('\\', "/"),
            "dir": is_dir,
        }));
    }

    let payload = json!({
        "schema": core_json::SCHEMA_VERSION,
        "truncated": truncated,
        "entries": entries,
    })
    .to_string();
    let _ = respond(stream, 200, &[], "application/json", payload.as_bytes());
}

enum FileError {
    Missing,
    Blocked,
    TooLarge,
}

fn respond_file(stream: &mut TcpStream, server: &Server, query: &str) {
    let Some(relative) = query_param(query, "path") else {
        let _ = respond(
            stream,
            400,
            &[],
            "text/plain; charset=utf-8",
            b"missing path\n",
        );
        return;
    };

    match read_repo_file(&server.root, &relative, server.max_file) {
        Ok(bytes) => {
            if let Some(content_type) = image_content_type(&relative) {
                let _ = respond(stream, 200, &[], content_type, &bytes);
                return;
            }
            match String::from_utf8(bytes) {
                Ok(text) => {
                    let _ = respond(
                        stream,
                        200,
                        &[],
                        "text/plain; charset=utf-8",
                        text.as_bytes(),
                    );
                }
                Err(_) => {
                    let _ = respond(
                        stream,
                        415,
                        &[],
                        "text/plain; charset=utf-8",
                        b"binary file\n",
                    );
                }
            }
        }
        Err(FileError::TooLarge) => {
            let _ = respond(
                stream,
                413,
                &[],
                "text/plain; charset=utf-8",
                b"file exceeds --max-file\n",
            );
        }
        Err(FileError::Blocked) => {
            let _ = respond(
                stream,
                403,
                &[],
                "text/plain; charset=utf-8",
                b"path is outside the browsable repository\n",
            );
        }
        Err(FileError::Missing) => {
            let _ = respond(
                stream,
                404,
                &[],
                "text/plain; charset=utf-8",
                b"file not found\n",
            );
        }
    }
}

fn read_repo_file(root: &Path, relative: &str, max: u64) -> Result<Vec<u8>, FileError> {
    let path = Path::new(relative);
    if relative.is_empty() || path.is_absolute() {
        return Err(FileError::Blocked);
    }
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                if BLOCKED
                    .iter()
                    .any(|blocked| name.eq_ignore_ascii_case(blocked))
                {
                    return Err(FileError::Blocked);
                }
            }
            _ => return Err(FileError::Blocked),
        }
    }

    // `canonicalize` resolves symlinks, so an escape is caught by the prefix
    // check rather than by the component scan alone.
    let canonical = root
        .join(path)
        .canonicalize()
        .map_err(|_| FileError::Missing)?;
    if !canonical.starts_with(root) {
        return Err(FileError::Blocked);
    }
    let metadata = std::fs::metadata(&canonical).map_err(|_| FileError::Missing)?;
    if !metadata.is_file() {
        return Err(FileError::Missing);
    }
    if metadata.len() > max {
        return Err(FileError::TooLarge);
    }
    std::fs::read(&canonical).map_err(|_| FileError::Missing)
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    extra: &[(&str, &str)],
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status} {}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Content-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; form-action 'none'\r\n",
        reason(status),
        body.len()
    );
    for (name, value) in extra {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");

    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        _ => "Error",
    }
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 2048];

    loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() > 16 * 1024 {
            return None;
        }
    }

    let head = String::from_utf8_lossy(&buffer);
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_owned();
    let target = request_line.next()?.to_owned();

    let mut cookie = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("cookie") {
            continue;
        }
        for pair in value.split(';') {
            if let Some((key, value)) = pair.trim().split_once('=')
                && key == TOKEN_COOKIE
            {
                cookie = Some(value.trim().to_owned());
            }
        }
    }

    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
    Some(Request {
        method,
        path: percent_decode(path)?,
        query: query.to_owned(),
        cookie,
    })
}

fn authorized(server: &Server, request: &Request) -> bool {
    if request.cookie.as_deref() == Some(server.token.as_str()) {
        return true;
    }
    query_param(&request.query, "t").as_deref() == Some(server.token.as_str())
}

fn query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if name == key {
            return percent_decode(value);
        }
    }
    None
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let hex = input.get(index + 1..index + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                index += 3;
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// Capability token for this session. It keeps other local processes out; it is
/// not a security boundary, exactly like a claim token (`docs/CLAIMS.md`).
fn session_token() -> String {
    use std::hash::{BuildHasher, Hasher};

    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    hasher.write_u128(nanos);
    hasher.write_u32(std::process::id());
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding_handles_escapes_and_plus() {
        assert_eq!(
            percent_decode("src%2Fmain.rs").as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(percent_decode("a+b").as_deref(), Some("a b"));
        assert_eq!(percent_decode("100%").as_deref(), None);
    }

    #[test]
    fn query_params_are_decoded() {
        assert_eq!(
            query_param("t=abc&path=src%2Fmain.rs", "path").as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(query_param("t=abc", "path"), None);
    }

    #[test]
    fn file_paths_cannot_escape_the_root() {
        let root = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        assert!(read_repo_file(&root, "../etc/passwd", 1024).is_err());
        assert!(read_repo_file(&root, "/etc/passwd", 1024).is_err());
        assert!(read_repo_file(&root, ".git/config", 1024).is_err());
        assert!(read_repo_file(&root, "", 1024).is_err());
    }

    #[test]
    fn sessions_get_distinct_tokens() {
        assert_ne!(session_token(), session_token());
        assert_eq!(session_token().len(), 16);
    }

    #[test]
    fn poll_intervals_accept_ms_and_seconds() {
        assert_eq!(poll_interval("500ms"), Some(Duration::from_millis(500)));
        assert_eq!(poll_interval("1s"), Some(Duration::from_secs(1)));
        assert_eq!(poll_interval("5"), Some(Duration::from_secs(5)));
        assert_eq!(poll_interval("50ms"), None);
        assert_eq!(poll_interval("soon"), None);
    }

    #[test]
    fn attachment_kinds_follow_the_extension() {
        assert_eq!(file_kind("screenshot.PNG"), "image");
        assert_eq!(file_kind("notes.md"), "text");
        assert_eq!(file_kind("dump.bin"), "file");
        assert_eq!(file_kind("Makefile"), "file");

        assert_eq!(image_content_type("a.jpeg"), Some("image/jpeg"));
        assert_eq!(image_content_type("a.webp"), Some("image/webp"));
        assert_eq!(image_content_type("a.svg"), None);
    }
}
