# Release 0.2.1

- STATUS: OPEN
- PRIORITY: 100
- TAGS: release,agent

Ship the board fixes as 0.2.1: the event stream now recovers from a dropped or rejected
connection, and the serve token is per clone, so an open board survives a restart and a rebuild.
Both changes landed after the v0.2.0 tag.

## Acceptance

- [x] workspace version is 0.2.1 and `tatrs version` reports it
- [ ] `v0.2.1` release publishes musl, macOS arm64, and Windows zips with SHA-256 checksums
- [ ] one downloaded asset verifies and runs
- [x] README, `docs/JSON.md`, and `docs/RELEASING.md` examples point at 0.2.1
