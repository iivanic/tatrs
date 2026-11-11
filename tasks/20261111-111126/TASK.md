# Fix files view when serving --no-browse

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui,dashboard
- OWNER: dsh

## Context

With `serve --no-browse`, the rail still linked to `#/files`, which fetched `/api/tree` and showed "Error: not found". The board did not know browsing was disabled.

## Acceptance

- [x] the snapshot carries `browse`, mirroring `--no-browse`
- [x] `--no-browse` hides the files link and shows "file browser disabled" instead of a 404
- [x] default `tatrs serve` still serves the tree and file views
- [x] tests cover both modes

## Progress

2026-11-11:

- `Watcher` carries `browse`, and the snapshot exposes it.
- The dashboard hides the `files` link when `browse === false`, and `#/files`/`#/file/...` show
  "file browser disabled (serve --no-browse)" instead of a 404. A direct `#/files` load waits for
  the first snapshot rather than racing `/api/tree`.
- Verified default mode (`/api/tree` 200, `browse: true`) and added a `--no-browse` test asserting
  `404` on the tree route, `browse: false`, and the disabled-state marker in the page.
