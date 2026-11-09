# Ship release binaries and CI

- STATUS: OPEN
- PRIORITY: 100
- TAGS: release,m3,agent
- OWNER: dsh

## Context

The binary works locally, but distribution is missing: no CI, no release artifacts, no install path other than a checkout. Agents need a pinned binary they can install in a container.

## Acceptance

- [ ] GitHub Actions CI runs `fmt --check`, `clippy -D warnings`, and tests on ubuntu-latest, macos-latest, and windows-latest
- [ ] A tag triggers release builds for `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`, uploaded with SHA-256 checksums
- [x] `cargo install --path crates/tatrs-cli` and prebuilt-binary installs are documented
- [ ] a container image suitable for agent sandboxes is published or buildable from the repo
- [x] release builds report the commit in `tatrs version`

## Progress

Started 2026-11-09:

- `.github/workflows/ci.yml`: matrix over ubuntu/macos/windows; fmt, clippy `-D warnings`,
  `cargo test --locked`, release build. YAML validated with PyYAML; not yet executed.
- `.github/workflows/release.yml`: tag `v*` builds the three targets, packages with
  `scripts/package-release.ps1`, uploads artifacts, publishes a release with checksums.
- `scripts/package-release.ps1` exercised on Windows: produces the zip (binary, README, LICENSE) and
  a matching `.sha256`.
- `Dockerfile` (multi-stage, non-root, `/work`) and `.dockerignore`. Not built locally: the Docker
  daemon is not running on this machine.
- `docs/RELEASING.md` documents the tag flow, manual packaging, verification, and known gaps.
- Release build (`cargo build --release`) prints `commit 7c3e27c` from `tatrs version`.
- Docs cannot be closed from a local checkout: CI and the release workflow need a first push/tag.
