# File view: return to task when opened from a task

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui,dashboard
- OWNER: dsh

## Context

Opening an attachment from a task navigates to `#/file/...`, whose back link is hardcoded to `#/files`. It should return to the task it was opened from; only files opened from the browser should go back to `#/files`.

## Acceptance

- [x] a file opened from a task shows `← task` and returns to that task
- [x] a file opened from the files tree still shows `← files`
- [x] a direct file URL with no history falls back to `← files`
- [x] the serve test asserts both back-link variants exist in the page

## Progress

2026-11-11:

- `route()` remembers the previous hash; `showFile(path, from)` uses it to pick the back link and
  target (`#/task/<id>` vs `#/files`).
- A direct `#/file/...` load has no previous route and falls back to `← files`.
- Test asserts the page carries the helper and both back-link labels; docs updated.
