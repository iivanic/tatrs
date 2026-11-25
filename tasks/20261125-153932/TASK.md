# A fresh lease is reported as stalled

- STATUS: CLOSED
- PRIORITY: 90
- TAGS: agent,ui

The QMApp board put a task in **attention** with `stalled` the moment it was claimed. `20261125-094442` had `TASK.md` untouched since 09:47Z and was claimed at 15:37:54Z; the detail page showed `in progress`, `since 35s ago`, and `stalled` at the same time.

`lifecycle::classify` measures the quiet window from the task file mtime only, so any claim on a file older than `--stale` (default 2h) is flagged before the new owner has had a chance to write anything. Acquiring the lease is activity.

## Acceptance

- [x] a live lease acquired after the last file change is not stalled, and its `since` is the acquisition time
- [x] a lease that has been held past the stale window with no file change is still stalled
- [x] `docs/DASHBOARD.md` says which timestamp opens the quiet window

## Progress

2026-11-25:

- Reproduced from the live board: `/api/task/20261125-094442` returned `state=in_progress`,
  `since=15:37:54Z`, `flags=["stalled"]` 35s after the QMApp agent claimed it; TASK.md mtime was
  09:47Z.
- `classify` now takes `max(last_activity, acquired_at)` as the start of the quiet window, so a claim
  on an old file starts unstalled while a lease held past `--stale` without a write still flags.
- Same tree against the fixed build: `flags=[]`, `attention` 1 -> 0.
