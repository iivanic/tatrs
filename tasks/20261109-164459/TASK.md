# Ship release binaries and CI

- STATUS: OPEN
- PRIORITY: 100
- TAGS: release,m3,agent
- OWNER: dsh

## Context

The binary works locally, but distribution is missing: no CI, no release artifacts, no install path other than a checkout. Agents need a pinned binary they can install in a container.

## Acceptance

- [x] GitHub Actions CI runs `fmt --check`, `clippy -D warnings`, and tests on ubuntu-latest, macos-latest, and windows-latest
- [ ] A tag triggers release builds for `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`, uploaded with SHA-256 checksums
- [x] `cargo install --path crates/tatrs-cli` and prebuilt-binary installs are documented
- [ ] a container image suitable for agent sandboxes is published or buildable from the repo
- [x] release builds report the commit in `tatrs version`

## Progress

2026-11-09:

- Repository published at https://github.com/iivanic/tatrs (public, MIT, topics set).
- First CI run [42661035854](https://github.com/iivanic/tatrs/actions/runs/42661035854) passed on
  ubuntu-latest, macos-latest, and windows-latest: fmt, clippy `-D warnings`, tests, release build.
  Badge added to the README; checkout action bumped to v5 for the Node 24 runner.
- `.github/workflows/release.yml`: tag `v*` builds the three targets, packages with
  `scripts/package-release.ps1`, uploads artifacts, publishes a release with checksums. Not yet
  triggered — the first tag is the remaining verification.
- `scripts/package-release.ps1` exercised on Windows: produces the zip (binary, README, LICENSE) and
  a matching `.sha256`.
- `Dockerfile` (multi-stage, non-root, `/work`) and `.dockerignore`. Not built locally: the Docker
  daemon is not running on this machine; the image has not been published.
- `docs/RELEASING.md` documents the tag flow, manual packaging, verification, and known gaps.
- Release build (`cargo build --release`) prints `commit 7c3e27c` from `tatrs version`.
