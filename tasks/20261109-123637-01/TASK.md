# Add find, id, ref, summary and todo

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: cli,m1
- OWNER: dsh

## Context

Remaining upstream commands, implemented in-process: no `grep`, no shelling out, works on Windows.

## Acceptance

- [x] `find <HUID>` locates closed tasks too; `--path-only` prints `tasks/<id>/TASK.md`
- [x] `id` prints the current task HUID, or exits 2 outside a task directory
- [x] `ref <HUID>` walks the worktree and finds references in source and tasks
- [x] `summary --json` reports counts by status, tag, and priority
- [x] `todo <FILE...>` converts TODO comments without prompting when stdin is not a TTY

## Progress

Done 2026-11-09:

- `find`, `id`, and `ref` are thin adapters over core; `ref` walks the tree in process, skipping
  `.git` and binaries.
- `summary` mirrors upstream's human layout and adds `--json` with tag descriptions from
  `tasks/tags` plus a priority histogram.
- `todo` never prompts: no `--yes` means preview, `--yes` creates tasks and rewrites the markers,
  preserving prefixes, suffixes, and CRLF. Empty titles fall back to `TODO in <file>:<line>`.
- Core gained `refs`, `summary`, and `todo` modules plus `Repo::current_task` and `write_atomic`.
- Corpus round-trip test: all 94 upstream tasks parse and render byte-identically.
- 73 tests (49 core, 23 CLI, 1 corpus), clippy clean.
