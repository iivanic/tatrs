//! Ephemeral task leases.
//!
//! Per `docs/DECISIONS.md` D-002, a claim is runtime state, never part of
//! `TASK.md`. One JSON file per task lives under the git common directory, so
//! every worktree of a clone sees the same leases and nothing is committed.
//! Without git, the fallback is `tasks/.tatrs/claims/`.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::huid::Huid;
use crate::store::{Repo, StoreError, write_atomic};
use crate::time;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub task: String,
    pub owner: String,
    pub token: String,
    pub acquired_at: String,
    pub expires_at: String,
    pub host: String,
    pub pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

impl Claim {
    pub fn is_live(&self, now: i64) -> bool {
        time::parse_iso(&self.expires_at).is_some_and(|expires| expires > now)
    }

    pub fn remaining_seconds(&self, now: i64) -> Option<i64> {
        time::parse_iso(&self.expires_at).map(|expires| expires - now)
    }
}

/// One entry of the claims directory; `claim` is `None` when the file is
/// unreadable or malformed, with the reason in `error`.
#[derive(Debug, Clone)]
pub struct ClaimEntry {
    pub path: PathBuf,
    pub claim: Option<Claim>,
    pub error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ClaimError {
    #[error("task {task} is claimed by {owner} until {expires_at}")]
    Held {
        task: String,
        owner: String,
        expires_at: String,
    },
    #[error("the claim token does not match the current owner of {0}")]
    NotHeld(String),
    #[error("no claim exists for {0}")]
    Missing(String),
    #[error("claim file {} is corrupt: {message}", .path.display())]
    Corrupt { path: PathBuf, message: String },
    #[error("timed out waiting for the claim lock {}", .0.display())]
    LockTimeout(PathBuf),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct ClaimStore {
    dir: PathBuf,
}

impl ClaimStore {
    /// Resolve the store for a repository: the git common dir when available,
    /// otherwise the fallback inside `tasks/`.
    pub fn locate(repo: &Repo) -> Self {
        match common_git_dir(repo.root()) {
            Some(git_dir) => Self {
                dir: git_dir.join("tatrs").join("claims"),
            },
            None => Self {
                dir: repo.tasks_dir().join(".tatrs").join("claims"),
            },
        }
    }

    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path_for(&self, id: &Huid) -> PathBuf {
        self.dir.join(format!("{}.json", id.as_str()))
    }

    pub fn read(&self, id: &Huid) -> Result<Option<Claim>, ClaimError> {
        let path = self.path_for(id);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(ClaimError::Io(err)),
        };
        let claim: Claim = serde_json::from_str(&text).map_err(|err| ClaimError::Corrupt {
            path,
            message: err.to_string(),
        })?;
        Ok(Some(claim))
    }

    /// All claim files, sorted by task id. Corrupt files are returned with an
    /// error rather than dropped, so `check` can report them.
    pub fn list(&self) -> Vec<ClaimEntry> {
        let mut entries = Vec::new();
        let Ok(dir) = fs::read_dir(&self.dir) else {
            return entries;
        };
        let mut paths: Vec<PathBuf> = dir
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();

        for path in paths {
            match fs::read_to_string(&path) {
                Ok(text) => match serde_json::from_str::<Claim>(&text) {
                    Ok(claim) => entries.push(ClaimEntry {
                        path,
                        claim: Some(claim),
                        error: None,
                    }),
                    Err(err) => entries.push(ClaimEntry {
                        path,
                        claim: None,
                        error: Some(err.to_string()),
                    }),
                },
                Err(err) => entries.push(ClaimEntry {
                    path,
                    claim: None,
                    error: Some(err.to_string()),
                }),
            }
        }

        entries
    }

    pub fn write(&self, claim: &Claim) -> Result<(), ClaimError> {
        fs::create_dir_all(&self.dir)?;
        let path = self.path_of(claim)?;
        let text = serde_json::to_string_pretty(claim).map_err(|err| ClaimError::Corrupt {
            path: path.clone(),
            message: err.to_string(),
        })?;
        write_atomic(&path, &format!("{text}\n"))?;
        Ok(())
    }

    fn path_of(&self, claim: &Claim) -> Result<PathBuf, ClaimError> {
        let id = Huid::parse(&claim.task).map_err(|err| ClaimError::Corrupt {
            path: self.dir.clone(),
            message: err.to_string(),
        })?;
        Ok(self.path_for(&id))
    }

    /// Serialize the read-modify-write of one claim. A stale lock (crashed
    /// process) is reclaimed after [`LOCK_STALE_AFTER`].
    fn lock_task(&self, id: &Huid) -> Result<TaskLock, ClaimError> {
        fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(format!("{}.lock", id.as_str()));
        let started = Instant::now();

        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let _ = writeln!(file, "pid={}", std::process::id());
                    return Ok(TaskLock { path });
                }
                Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .and_then(|modified| modified.elapsed().ok())
                        .is_some_and(|age| age > LOCK_STALE_AFTER);
                    if stale {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    if started.elapsed() > LOCK_TIMEOUT {
                        return Err(ClaimError::LockTimeout(path));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(err) => return Err(ClaimError::Io(err)),
            }
        }
    }

    /// Acquire or steal a claim. A live claim is never taken; an expired one is
    /// overwritten. The post-write read confirms the token, so two concurrent
    /// claimers cannot both believe they won.
    pub fn claim(
        &self,
        id: &Huid,
        owner: &str,
        ttl: Duration,
        run: Option<String>,
    ) -> Result<Claim, ClaimError> {
        let _lock = self.lock_task(id)?;
        let now = time::now_secs();

        if let Some(existing) = self.read(id).unwrap_or(None)
            && existing.is_live(now)
        {
            return Err(ClaimError::Held {
                task: id.to_string(),
                owner: existing.owner,
                expires_at: existing.expires_at,
            });
        }

        let claim = Claim {
            task: id.to_string(),
            owner: owner.to_owned(),
            token: new_token(),
            acquired_at: time::format_iso(now),
            expires_at: time::format_iso(now + ttl.as_secs() as i64),
            host: host_name(),
            pid: std::process::id(),
            run,
        };
        self.write(&claim)?;

        match self.read(id) {
            Ok(Some(current)) if current.token == claim.token => Ok(claim),
            Ok(Some(current)) => Err(ClaimError::Held {
                task: id.to_string(),
                owner: current.owner,
                expires_at: current.expires_at,
            }),
            Ok(None) => Err(ClaimError::Missing(id.to_string())),
            Err(err) => Err(err),
        }
    }

    /// Extend a claim only when the caller proves ownership with the token.
    pub fn refresh(&self, id: &Huid, token: &str, ttl: Duration) -> Result<Claim, ClaimError> {
        let _lock = self.lock_task(id)?;
        let now = time::now_secs();
        let mut claim = self
            .read(id)?
            .ok_or_else(|| ClaimError::Missing(id.to_string()))?;
        if claim.token != token {
            return Err(ClaimError::NotHeld(id.to_string()));
        }
        claim.expires_at = time::format_iso(now + ttl.as_secs() as i64);
        self.write(&claim)?;
        Ok(claim)
    }

    /// Compare-and-delete: a stale token cannot remove the new owner's claim.
    pub fn release(&self, id: &Huid, token: &str) -> Result<(), ClaimError> {
        let _lock = self.lock_task(id)?;
        let claim = self
            .read(id)?
            .ok_or_else(|| ClaimError::Missing(id.to_string()))?;
        if claim.token != token {
            return Err(ClaimError::NotHeld(id.to_string()));
        }
        match fs::remove_file(self.path_for(id)) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(ClaimError::Missing(id.to_string()))
            }
            Err(err) => Err(ClaimError::Io(err)),
        }
    }
}

const LOCK_STALE_AFTER: Duration = Duration::from_secs(5);
const LOCK_TIMEOUT: Duration = Duration::from_secs(2);

struct TaskLock {
    path: PathBuf,
}

impl Drop for TaskLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn host_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_owned())
}

fn new_token() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = time::now_secs();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{now:016x}{counter:04x}{:04x}", std::process::id() & 0xffff)
}

/// Resolve the git common directory, including linked worktrees.
fn common_git_dir(root: &Path) -> Option<PathBuf> {
    let dot_git = root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    if !dot_git.is_file() {
        return None;
    }

    let contents = fs::read_to_string(&dot_git).ok()?;
    let line = contents
        .lines()
        .find_map(|line| line.strip_prefix("gitdir:"))?
        .trim();
    let git_dir = fs::canonicalize(root.join(line)).ok()?;

    if let Ok(relative) = fs::read_to_string(git_dir.join("commondir"))
        && let Ok(common) = fs::canonicalize(git_dir.join(relative.trim()))
    {
        return Some(common);
    }

    git_dir.parent()?.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store;

    fn scratch() -> tempfile::TempDir {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-scratch");
        fs::create_dir_all(&base).unwrap();
        tempfile::Builder::new()
            .prefix("claims-")
            .tempdir_in(&base)
            .unwrap()
    }

    fn store_in(tmp: &tempfile::TempDir) -> ClaimStore {
        store::init(tmp.path()).unwrap();
        let repo = Repo::open(tmp.path()).unwrap();
        ClaimStore::locate(&repo)
    }

    #[test]
    fn claim_holds_and_releases() {
        let tmp = scratch();
        let store = store_in(&tmp);
        let id = Huid::parse("20261109-120000").unwrap();

        let claim = store
            .claim(&id, "agent-3", Duration::from_secs(7200), None)
            .unwrap();
        assert!(store.path_for(&id).is_file());

        let err = store
            .claim(&id, "agent-4", Duration::from_secs(7200), None)
            .unwrap_err();
        assert!(matches!(err, ClaimError::Held { .. }));

        let extended = store
            .refresh(&id, &claim.token, Duration::from_secs(10800))
            .unwrap();
        assert!(extended.remaining_seconds(time::now_secs()).unwrap() > 10_000);

        assert!(matches!(
            store.release(&id, "wrong"),
            Err(ClaimError::NotHeld(_))
        ));
        store.release(&id, &claim.token).unwrap();
        assert!(store.read(&id).unwrap().is_none());
    }

    #[test]
    fn expired_claims_are_stealable() {
        let tmp = scratch();
        let store = store_in(&tmp);
        let id = Huid::parse("20261109-120000").unwrap();

        store.claim(&id, "agent-3", Duration::ZERO, None).unwrap();
        let stolen = store
            .claim(&id, "agent-4", Duration::from_secs(60), None)
            .unwrap();
        assert_eq!(stolen.owner, "agent-4");
    }

    #[test]
    fn concurrent_claims_have_exactly_one_winner() {
        let tmp = scratch();
        let store = store_in(&tmp);
        let id = Huid::parse("20261109-120000").unwrap();

        let results: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|worker| {
                    let store = store.clone();
                    let id = id.clone();
                    scope.spawn(move || {
                        store.claim(
                            &id,
                            &format!("agent-{worker}"),
                            Duration::from_secs(60),
                            None,
                        )
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect()
        });

        assert_eq!(
            results.iter().filter(|result| result.is_ok()).count(),
            1,
            "exactly one claimer must win"
        );
        assert!(
            results
                .iter()
                .filter(|result| matches!(result, Err(ClaimError::Held { .. })))
                .count()
                >= 1
        );
    }

    #[test]
    fn locate_uses_git_dir_when_present() {
        let tmp = scratch();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        store::init(tmp.path()).unwrap();
        let repo = Repo::open(tmp.path()).unwrap();
        let store = ClaimStore::locate(&repo);
        assert_eq!(store.dir(), tmp.path().join(".git/tatrs/claims"));
    }

    #[test]
    fn locate_falls_back_without_git() {
        let tmp = scratch();
        let store = store_in(&tmp);
        assert_eq!(
            store.dir(),
            tmp.path().join("tasks/.tatrs/claims"),
            "fallback lives inside tasks/ and is a dot directory"
        );
    }

    #[test]
    fn list_reports_corrupt_files() {
        let tmp = scratch();
        let store = store_in(&tmp);
        fs::create_dir_all(store.dir()).unwrap();
        fs::write(store.dir().join("not-json.json"), "nope").unwrap();

        let entries = store.list();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].error.is_some());
    }
}
