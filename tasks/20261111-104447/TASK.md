# Release 0.2.0

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: release,agent
- OWNER: dsh

## Context

Ship the lifecycle/board/agent-kit work as 0.2.0. The v0.1.0 tag predates those commits.

## Acceptance

- [x] workspace version is 0.2.0 and `tatrs version` reports it
- [x] `v0.2.0` release publishes musl, macOS arm64, and Windows zips with SHA-256 checksums
- [x] one downloaded asset verifies and runs
- [x] README install examples point at 0.2.0

## Progress

2026-11-11:

- Workspace bumped to 0.2.0; `tatrs version` reports `0.2.0` with commit `442bb76`.
- Release [v0.2.0](https://github.com/iivanic/tatrs/releases/tag/v0.2.0) published all three zips
  and checksums.
- Windows asset downloaded: SHA-256 matched, extracted binary reports `0.2.0` / `commit 442bb76`.
- README and `docs/RELEASING.md` examples point at 0.2.0.
