# Releasing `tatrs`

## Cutting a release

1. Bump `version` in the workspace `Cargo.toml` and run the full checklist
   (`cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`).
2. Commit, then tag on `main`:

   ```console
   git tag v0.1.0
   git push origin v0.1.0
   ```

3. `.github/workflows/release.yml` builds and publishes:

   | Target | Runner |
   |-|-|
   | `x86_64-unknown-linux-musl` | ubuntu-latest |
   | `aarch64-apple-darwin` | macos-14 |
   | `x86_64-pc-windows-msvc` | windows-latest |

   Each target produces `tatrs-<version>-<target>.zip` (binary, README, LICENSE)
   and a `.sha256` checksum, attached to the GitHub release with generated
   notes.

## Doing it by hand

```console
cargo build --release --locked --target <target> --bin tatrs
pwsh -File scripts/package-release.ps1 `
  -Target <target> `
  -Binary target/<target>/release/tatrs[.exe] `
  -Version v0.1.0 `
  -OutDir dist
```

The script is platform-neutral on purpose: it runs on every GitHub runner via
`pwsh`, so the workflow has no packaging branch per OS. It was exercised on
Windows during development; the first tagged run is the cross-platform proof.

## Verifying an artifact

```console
sha256sum -c tatrs-0.1.0-x86_64-unknown-linux-musl.zip.sha256
unzip tatrs-0.1.0-x86_64-unknown-linux-musl.zip -d ~/.local/bin
tatrs version        # prints the commit stamped at build time
```

## Container image

```console
docker build -t tatrs:dev .
docker run --rm -v "$PWD:/work" tatrs:dev ls
```

The image is `debian:bookworm-slim`, runs as an unprivileged user, and defaults
to `/work`. It was built and exercised locally on 2026-11-09 (`version`, `ls`,
`summary` against a mounted repository). It is not published to a registry yet.

## Known gaps

- No code signing or notarization (macOS Gatekeeper, Windows SmartScreen).
- `aarch64-unknown-linux-musl` is not built; add it to the matrix when needed.
- Release artifacts are not reproducible bit-for-bit; the commit stamp and
  lockfile are the provenance.
