# Make ref respect ignore files

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: cli,m2,agent

## Context

`ref` walked every file, including `target/` and other ignored build output. On a large checkout that is O(repo) in time and I/O, and it reports references from files git does not track.

## Acceptance

- [x] `.gitignore` files at any depth are honoured, including negation and `**`
- [x] `.git/info/exclude` is honoured; global user excludes are not applied
- [x] VCS metadata directories are always skipped
- [x] `--no-ignore` scans ignored files
- [x] hidden files are still scanned when not ignored
- [x] corpus round-trip and query baselines unchanged

## Progress

Done 2026-11-09 via the `ignore` crate (nested gitignore semantics are too subtle to hand-roll). Tests cover root and nested rules, negation, hidden files, `.git`, and the escape hatch.
