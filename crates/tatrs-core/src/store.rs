//! Discovery, loading, and durable writes for a `tasks/` database.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::diagnostics::{Diagnostic, codes};
use crate::huid::{Huid, HuidError};
use crate::task::Task;

/// A project directory that contains a `tasks/` database.
#[derive(Debug, Clone)]
pub struct Repo {
    root: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct LoadResult {
    pub tasks: Vec<Task>,
    pub diagnostics: Vec<Diagnostic>,
}

impl LoadResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }
}

#[derive(Debug, Clone)]
pub struct NewTask {
    pub title: String,
    pub tags: Vec<String>,
    pub priority: i64,
    pub suffix: Option<String>,
    /// Deterministic ID. When set, creation is idempotent at the caller level.
    pub id: Option<Huid>,
    /// Body appended verbatim after the property block.
    pub body: String,
}

impl NewTask {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            tags: Vec::new(),
            priority: crate::task::DEFAULT_PRIORITY,
            suffix: None,
            id: None,
            body: String::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("no `tasks/` directory found in {} or any parent; run `tatrs init`", .start.display())]
    NoRepo { start: PathBuf },
    #[error("task {0} already exists")]
    Exists(Huid),
    #[error("task {0} was not found")]
    NotFound(String),
    #[error("could not allocate a task id after 100 attempts; check the system clock")]
    IdExhausted,
    #[error(
        "timed out after {timeout_ms} ms waiting for the task lock ({path} is held by another process)"
    )]
    Locked { path: PathBuf, timeout_ms: u64 },
    #[error(transparent)]
    Huid(#[from] HuidError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOutcome {
    pub created: bool,
    pub tasks_dir: PathBuf,
}

impl Repo {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        if root.join("tasks").is_dir() {
            Ok(Self { root })
        } else {
            Err(StoreError::NoRepo { start: root })
        }
    }

    /// Walk up from `start` looking for a project root with `tasks/`.
    pub fn discover(start: &Path) -> Result<Self, StoreError> {
        let mut current = Some(start);
        while let Some(dir) = current {
            if dir.join("tasks").is_dir() {
                return Ok(Self {
                    root: dir.to_path_buf(),
                });
            }
            current = dir.parent();
        }
        Err(StoreError::NoRepo {
            start: start.to_path_buf(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.root.join("tasks")
    }

    pub fn task_dir(&self, id: &Huid) -> PathBuf {
        self.tasks_dir().join(id.as_str())
    }

    pub fn task_file(&self, id: &Huid) -> PathBuf {
        self.task_dir(id).join("TASK.md")
    }

    /// `tasks/<id>/TASK.md`, forward-slash separated, for stable output.
    pub fn rel_task_file(&self, id: &Huid) -> String {
        format!("tasks/{}/TASK.md", id.as_str())
    }

    /// Path relative to the repository root, forward-slash separated.
    pub fn rel_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Load every task. Structural problems become diagnostics; unreadable task
    /// directories are skipped rather than aborting the listing.
    pub fn load(&self) -> LoadResult {
        let mut out = LoadResult::default();
        let dir = self.tasks_dir();

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) => {
                out.diagnostics.push(Diagnostic::error(
                    codes::READ_TASKS_DIR,
                    &dir,
                    None,
                    format!("cannot read tasks directory: {err}"),
                ));
                return out;
            }
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();

            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if matches!(name.as_str(), "tags" | "README.md" | "TAGS.md") {
                    continue;
                }
                if name.starts_with(".tmp-") {
                    out.diagnostics.push(Diagnostic::warning(
                        codes::LEFT_OVER_TEMP,
                        &path,
                        None,
                        "leftover temporary file; remove it or run `tatrs check --fix`",
                    ));
                } else {
                    out.diagnostics.push(Diagnostic::warning(
                        codes::STRAY_ENTRY,
                        &path,
                        None,
                        "unexpected file in tasks/; only task directories, `tags`, and `README.md` belong here",
                    ));
                }
                continue;
            }

            let id = match Huid::parse(&name) {
                Ok(id) => id,
                Err(err) => {
                    out.diagnostics.push(Diagnostic::error(
                        codes::INVALID_TASK_ID,
                        &path,
                        None,
                        format!("{err}; directory ignored"),
                    ));
                    continue;
                }
            };

            let task_path = self.task_file(&id);
            if !task_path.is_file() {
                out.diagnostics.push(Diagnostic::error(
                    codes::ORPHAN_TASK_DIR,
                    &path,
                    None,
                    "task directory has no TASK.md",
                ));
                continue;
            }

            let content = match fs::read_to_string(&task_path) {
                Ok(content) => content,
                Err(err) if err.kind() == ErrorKind::InvalidData => {
                    out.diagnostics.push(Diagnostic::error(
                        codes::INVALID_UTF8,
                        &task_path,
                        None,
                        "TASK.md is not valid UTF-8",
                    ));
                    continue;
                }
                Err(err) => {
                    out.diagnostics.push(Diagnostic::error(
                        codes::READ_ERROR,
                        &task_path,
                        None,
                        format!("cannot read TASK.md: {err}"),
                    ));
                    continue;
                }
            };

            let task = Task::parse(id, &content, &task_path);
            out.diagnostics.extend(task.diagnostics.iter().cloned());
            out.tasks.push(task);
        }

        out.tasks.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn load_task(&self, id: &Huid) -> Result<Task, StoreError> {
        let path = self.task_file(id);
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Err(StoreError::NotFound(id.to_string()));
            }
            Err(err) => return Err(StoreError::Io(err)),
        };
        Ok(Task::parse(id.clone(), &content, &path))
    }

    /// Create a task. The directory creation is the atomic reservation; the file
    /// is then written with a temp-file + rename.
    pub fn create(&self, spec: &NewTask) -> Result<Task, StoreError> {
        if let Some(id) = &spec.id {
            let dir = self.task_dir(id);
            match fs::create_dir(&dir) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                    return Err(StoreError::Exists(id.clone()));
                }
                Err(err) => return Err(StoreError::Io(err)),
            }
            let task = self.materialize(id.clone(), spec);
            if let Err(err) = self.save(&task) {
                let _ = fs::remove_dir_all(&dir);
                return Err(err);
            }
            return Ok(task);
        }

        let base = match &spec.suffix {
            Some(suffix) => Huid::now().with_suffix(suffix)?,
            None => Huid::now(),
        };

        for attempt in 0..100u32 {
            let id = if attempt == 0 {
                base.clone()
            } else {
                Huid::parse(format!("{}-{attempt:02}", base.as_str()))?
            };
            let dir = self.task_dir(&id);
            match fs::create_dir(&dir) {
                Ok(()) => {
                    let task = self.materialize(id, spec);
                    if let Err(err) = self.save(&task) {
                        let _ = fs::remove_dir_all(&dir);
                        return Err(err);
                    }
                    return Ok(task);
                }
                Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(StoreError::Io(err)),
            }
        }

        Err(StoreError::IdExhausted)
    }

    fn materialize(&self, id: Huid, spec: &NewTask) -> Task {
        let mut task = Task::new(
            id.clone(),
            spec.title.clone(),
            spec.tags.clone(),
            spec.priority,
        );
        task.body = spec.body.clone();
        task.path = self.task_file(&id);
        task
    }

    /// Durable, atomic write of `TASK.md`.
    pub fn save(&self, task: &Task) -> Result<(), StoreError> {
        let dir = self.task_dir(&task.id);
        fs::create_dir_all(&dir)?;
        write_atomic(&self.task_file(&task.id), &task.render())
    }

    /// The task whose directory contains `cwd`, if any. Only one level below
    /// `tasks/` counts, matching upstream's `tatrs id`.
    pub fn current_task(&self, cwd: &Path) -> Option<Huid> {
        let cwd = fs::canonicalize(cwd).ok()?;
        let tasks = fs::canonicalize(self.tasks_dir()).ok()?;
        if cwd.parent()? != tasks.as_path() {
            return None;
        }
        Huid::parse(cwd.file_name()?.to_string_lossy()).ok()
    }

    /// Acquire the exclusive batch-update lock, waiting up to `timeout`.
    ///
    /// The lock is an atomically created file; a lock older than
    /// [`LOCK_STALE_AFTER`] is considered abandoned and is reclaimed, so a
    /// crashed process cannot wedge the repository forever.
    pub fn try_lock(&self, timeout: Duration) -> Result<LockGuard, StoreError> {
        let path = self.tasks_dir().join(".tatrs.lock");
        let started = Instant::now();

        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let stamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|duration| duration.as_secs())
                        .unwrap_or(0);
                    let _ = writeln!(file, "pid={} at={stamp}", std::process::id());
                    return Ok(LockGuard { path });
                }
                Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                    if lock_is_stale(&path) {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    if started.elapsed() >= timeout {
                        return Err(StoreError::Locked {
                            path,
                            timeout_ms: timeout.as_millis() as u64,
                        });
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(err) => return Err(StoreError::Io(err)),
            }
        }
    }
}

/// How long a lock file may sit before another process may reclaim it.
pub const LOCK_STALE_AFTER: Duration = Duration::from_secs(30);

/// Held while a batch mutation runs; removes the lock file on drop.
#[derive(Debug)]
pub struct LockGuard {
    path: PathBuf,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn lock_is_stale(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .map(|age| age > LOCK_STALE_AFTER)
        .unwrap_or(false)
}

/// Write `contents` to `path` atomically: a temp file in the same directory is
/// renamed over the target, so readers never observe a truncated file.
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), StoreError> {
    let dir = path.parent().ok_or_else(|| {
        StoreError::Io(std::io::Error::new(
            ErrorKind::NotFound,
            "path has no parent directory",
        ))
    })?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(contents.as_bytes())?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|err| StoreError::Io(err.error))?;
    Ok(())
}

/// Create `tasks/` (and a short README) if it does not exist yet.
pub fn init(root: &Path) -> Result<InitOutcome, StoreError> {
    let tasks_dir = root.join("tasks");
    let created = !tasks_dir.is_dir();
    fs::create_dir_all(&tasks_dir)?;

    let readme = tasks_dir.join("README.md");
    if !readme.exists() {
        let body = "# Tasks\n\nCreated by `tatrs`. Each task is a directory named with a UTC HUID \
                    and contains a `TASK.md`. See `docs/FORMAT.md`.\n";
        fs::write(&readme, body)?;
    }

    Ok(InitOutcome { created, tasks_dir })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::DEFAULT_PRIORITY;

    /// Scratch space inside the workspace: some sandboxes deny writes to the
    /// system temp directory for spawned binaries.
    fn scratch() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-scratch");
        fs::create_dir_all(&base).unwrap();
        tempfile::Builder::new()
            .prefix("core-")
            .tempdir_in(&base)
            .unwrap()
    }

    fn repo() -> (tempfile::TempDir, Repo) {
        let tmp = scratch();
        init(tmp.path()).unwrap();
        let repo = Repo::open(tmp.path()).unwrap();
        (tmp, repo)
    }

    #[test]
    fn create_and_load_round_trip() {
        let (_tmp, repo) = repo();
        let task = repo.create(&NewTask::new("First task")).unwrap();
        assert_eq!(task.priority, DEFAULT_PRIORITY);

        let loaded = repo.load();
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].title, "First task");
        assert!(!loaded.has_errors());
    }

    #[test]
    fn explicit_id_is_rejected_when_present() {
        let (_tmp, repo) = repo();
        let id = Huid::parse("20261109-120000").unwrap();
        let mut spec = NewTask::new("Deterministic");
        spec.id = Some(id.clone());
        repo.create(&spec).unwrap();

        let err = repo.create(&spec).unwrap_err();
        assert!(matches!(err, StoreError::Exists(_)));
    }

    #[test]
    fn concurrent_creates_do_not_lose_tasks() {
        let tmp = scratch();
        init(tmp.path()).unwrap();
        let mut handles = Vec::new();
        for worker in 0..8 {
            let root = tmp.path().to_path_buf();
            handles.push(std::thread::spawn(move || {
                let repo = Repo::open(root).unwrap();
                let mut spec = NewTask::new(format!("task {worker}"));
                spec.suffix = Some(format!("w{worker}"));
                repo.create(&spec).unwrap();
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let repo = Repo::open(tmp.path()).unwrap();
        assert_eq!(repo.load().tasks.len(), 8);
    }

    #[test]
    fn lock_is_exclusive_and_released() {
        let (_tmp, repo) = repo();
        let guard = repo.try_lock(Duration::from_millis(100)).unwrap();
        let err = repo.try_lock(Duration::from_millis(100)).unwrap_err();
        assert!(matches!(err, StoreError::Locked { .. }));
        drop(guard);
        repo.try_lock(Duration::from_millis(100)).unwrap();
    }

    #[test]
    fn current_task_requires_exactly_one_level_below_tasks() {
        let (_tmp, repo) = repo();
        let id = Huid::parse("20261109-120000").unwrap();
        let dir = repo.task_dir(&id);
        fs::create_dir_all(dir.join("attachments")).unwrap();

        assert_eq!(repo.current_task(&dir), Some(id));
        assert_eq!(repo.current_task(&dir.join("attachments")), None);
        assert_eq!(repo.current_task(repo.root()), None);
    }

    #[test]
    fn orphan_and_invalid_directories_are_reported() {
        let (tmp, repo) = repo();
        fs::create_dir(tmp.path().join("tasks/20261109-120000")).unwrap();
        fs::create_dir(tmp.path().join("tasks/not-a-task")).unwrap();
        fs::write(tmp.path().join("tasks/stray.txt"), "x").unwrap();

        let loaded = repo.load();
        assert!(loaded.tasks.is_empty());
        let codes: Vec<_> = loaded.diagnostics.iter().map(|d| d.code).collect();
        assert!(codes.contains(&codes::ORPHAN_TASK_DIR));
        assert!(codes.contains(&codes::INVALID_TASK_ID));
        assert!(codes.contains(&codes::STRAY_ENTRY));
    }
}
