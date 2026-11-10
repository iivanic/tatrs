# Live task view goes stale when the task changes

- STATUS: CLOSED
- PRIORITY: 10
- TAGS: bug,agent,ui

Repro: open a task from the **received** list on the board, then close it from a terminal. The status rail and the board table update over SSE, but the open task view keeps showing `received`. It only refreshes on a manual reload or re-navigation.

Expected: the task view tracks its own board entry and re-fetches when that entry changes, preserving scroll position. A raw file opened via the task's file link may stay stale: a document view is a snapshot, not a live object.

## Acceptance

- [x] an open task view re-fetches when its board entry changes and keeps the pane's scroll position
- [x] navigating to another view stops further refreshes for the previous task
- [x] a file opened from the task (`#/file/...`) still does not auto-refresh
- [x] closing a received task from a terminal flips its open view to completed without a manual reload
