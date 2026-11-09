# Ship release binaries and CI

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: release,m3,agent
- OWNER: dsh

## Context

The binary works locally, but distribution is missing: no CI, no release artifacts, no install path other than a checkout. Agents need a pinned binary they can install in a container.

## Acceptance

- [x] GitHub Actions CI runs `fmt --check`, `clippy -D warnings`, and tests on ubuntu-latest, macos-latest, and windows-latest
- [x] A tag triggers release builds for `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`, uploaded with SHA-256 checksums
- [x] `cargo install --path crates/tatrs-cli` and prebuilt-binary installs are documented
- [x] a container image suitable for agent sandboxes is published or buildable from the repo
- [x] release builds report the commit in `tatrs version`

## Progress

2026-11-09:

- Repository: https://github.com/iivanic/tatrs (public, MIT).
- CI run [42661035854](https://github.com/iivanic/tatrs/actions/runs/42661035854) and
  [42661514428](https://github.com/iivanic/tatrs/actions/runs/42661514428) green on ubuntu, macos,
  and windows; badge in the README.
- Release [v0.1.0](https://github.com/iivanic/tatrs/releases/tag/v0.1.0) published with all three
  zips and `.sha256` files. The first attempt failed because `macos-14` is no longer a supported
  runner label; switching to `macos-latest` and re-tagging fixed it.
- Downloaded the Windows asset, matched its SHA-256, extracted it, and ran it: `tatrs 0.1.0`,
  `commit f334066`, `ls` works against the repo.
- Docker image `tatrs:dev` built and exercised: `version`, `ls`, and `summary` against the mounted
  repository as the unprivileged `tatrs` user.
- `docs/RELEASING.md` documents the flow and the remaining gaps (no signing, no `aarch64-linux`, no
  registry publication).
