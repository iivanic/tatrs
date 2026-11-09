# Ship release binaries and CI

- STATUS: OPEN
- PRIORITY: 100
- TAGS: release,m3,agent

## Context

The binary works locally, but distribution is missing: no CI, no release artifacts, no install path other than a checkout. Agents need a pinned binary they can install in a container.

## Acceptance

- [ ] GitHub Actions CI runs `fmt --check`, `clippy -D warnings`, and tests on ubuntu-latest, macos-latest, and windows-latest
- [ ] A tag triggers release builds for `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`, uploaded with SHA-256 checksums
- [ ] `cargo install --path crates/tatrs-cli` and prebuilt-binary installs are documented
- [ ] a container image suitable for agent sandboxes is published or buildable from the repo
- [ ] release builds report the commit in `tatrs version`
