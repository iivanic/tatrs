# Release 0.2.1

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: release,agent

Ship the board fixes as 0.2.1: the event stream now recovers from a dropped or rejected
connection, and the serve token is per clone, so an open board survives a restart and a rebuild.
Both changes landed after the v0.2.0 tag.

## Acceptance

- [x] workspace version is 0.2.1 and `tatrs version` reports it
- [x] `v0.2.1` release publishes musl, macOS arm64, and Windows zips with SHA-256 checksums
- [x] one downloaded asset verifies and runs
- [x] README, `docs/JSON.md`, and `docs/RELEASING.md` examples point at 0.2.1

## Progress

2026-11-25:

- Tagged `v0.2.1` on `0d8832a`; the release run
  [45597829294](https://github.com/iivanic/tatrs/actions/runs/45597829294) built all three targets and
  published them, with `.sha256` files, at
  [v0.2.1](https://github.com/iivanic/tatrs/releases/tag/v0.2.1).
- Downloaded `tatrs-0.2.1-x86_64-unknown-linux-musl.zip`: SHA-256 matched
  `7853d9d2…8d5d`, and the extracted binary reports `0.2.1` / `commit 0d8832a`.
- The zip stores `tatrs` as `0644`, so the documented `unzip` needs `chmod +x`; that packaging bug is
  `20261125-104833`, and the install snippets now carry the step.
