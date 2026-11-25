# Persist the serve token per clone

- STATUS: CLOSED
- PRIORITY: 85
- TAGS: bug,ui

The board token was per process, so an open page could not survive `tatrs serve` restarting: it
retried with the old token, got a 401, and `EventSource` gave up for good. The URL printed by one run
also stopped working in the next, which made a rebuild a page reload.

The token is now generated once per clone and stored `0600` in the per-clone state dir beside the
claims (`<git-common-dir>/tatrs/serve.token`, or `tasks/.tatrs/serve.token` without git), so both
sides of a restart agree. `Repo::state_dir` is the single place that answers where ephemeral state
lives; `ClaimStore::locate` now uses it too. `--read-only` keeps the token in memory and writes
nothing.

Because the token is now stable, a second clone served on another port can no longer be blamed for a
dead board: cookies for `127.0.0.1` are shared by every port, so every page request (fetches and
thumbnails) now carries the token from the page's own URL instead of relying on the cookie.

## Acceptance

- [x] the token is generated once per clone and stored `0600` in the state dir.
- [x] a restarted `tatrs serve` prints the same URL and an open board reconnects.
- [x] `--read-only` never writes the token file.
- [x] every page request carries the page's own token, so a foreign cookie cannot 401 the board.
- [x] tests cover persistence, read-only, and the token route.

## Progress

2026-11-25:

- `board_token()` reads `serve.token`, otherwise writes a fresh `session_token()` atomically and
  re-reads, so two servers racing agree on the file's value. Rotation is deleting the file.
- `Repo::state_dir()` moved the git-common-dir resolution out of `claims.rs`, which keeps claims and
  the token in one documented place.
- `dashboard.html` gained `api(path)`: the token from `location.search` is appended to `/events`,
  `/api/status`, `/api/task/…`, `/api/tree`, `/api/file`, `/api/action`, and thumbnail `src`s.
- Verified in Firefox: kill + restart keeps the page connected with the same URL, and with two clones
  served on 7801/7802 the cookie-only probe returns 401 while the page still renders task views.
