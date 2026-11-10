//! Read-model helpers shared by `summary` and `serve`.
//!
//! Everything here is observation: leases from the claim store, file
//! modification times, and clock. Derivation itself lives in
//! `tatrs_core::lifecycle` so the CLI layers stay thin.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use tatrs_core::claims::ClaimStore;
use tatrs_core::{Claim, Repo, Task};

/// All readable leases, live and expired, keyed by task id. Corrupt files are
/// ignored here; `check` is the command that reports them.
pub fn load_claims(repo: &Repo) -> BTreeMap<String, Claim> {
    ClaimStore::locate(repo)
        .list()
        .into_iter()
        .filter_map(|entry| entry.claim)
        .map(|claim| (claim.task.clone(), claim))
        .collect()
}

/// Last filesystem-visible change per task, in epoch seconds.
pub fn activity(tasks: &[Task]) -> BTreeMap<String, i64> {
    tasks
        .iter()
        .filter_map(|task| mtime(&task.path).map(|secs| (task.id.to_string(), secs)))
        .collect()
}

/// Epoch seconds of a file's modification time, when the platform reports one.
pub fn mtime(path: &Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let secs = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(secs).ok()
}
