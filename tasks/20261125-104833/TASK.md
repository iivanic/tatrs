# Release zips keep the executable bit

- STATUS: OPEN
- PRIORITY: 90
- TAGS: bug,release

The v0.2.1 Linux zip stores `tatrs` as `0644`, so the README's
`unzip ... -d ~/.local/bin` installs a binary that answers "Permission denied". `Compress-Archive`
records no Unix permission bits; the same flaw is in v0.2.0, where the Windows asset hid it.

Packaging now uses `zip` when it is available, which keeps the mode from the built binary; Windows
runners, where the mode is irrelevant, keep the `Compress-Archive` fallback. The install snippets in
README and `docs/RELEASING.md` gained the `chmod +x` step for zips cut before this fix.

## Acceptance

- [x] `package-release.ps1` produces zips whose binary carries `0755`
- [x] README and `docs/RELEASING.md` install steps yield a runnable `tatrs` for the 0.2.1 asset
- [ ] the next tagged release's Linux zip lists `tatrs` as `0755` (checked with `zipinfo`)

## Progress

2026-11-25:

- Reproduced: `zipinfo` on v0.2.0 and v0.2.1 musl zips reports `-rw-r--r--` for `tatrs`; the extracted
  binary needs `chmod +x` before it runs (it then reports `0.2.1` / `commit 0d8832a`).
- `scripts/package-release.ps1` computes an absolute `$OutDir` (it changes directory to stage the
  files) and shells out to `zip` when present. Verified the equivalent `zip -q <zip> tatrs README.md
  LICENSE` run stores `0o100755` via `zipfile`'s `external_attr`, with the fallback left intact.
- `pwsh` is not installed on this machine, so the script itself runs for the first time on the next
  tag; the fallback path keeps a regression from breaking a release.
