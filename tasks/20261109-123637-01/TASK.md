# Add find, id, ref, summary and todo

- STATUS: OPEN
- PRIORITY: 100
- TAGS: cli,m1

## Context

Remaining upstream commands, implemented in-process: no `grep`, no shelling out, works on Windows.

## Acceptance

- [ ] `find <HUID>` locates closed tasks too; `--path-only` prints `tasks/<id>/TASK.md`
- [ ] `id` prints the current task HUID, or exits 2 outside a task directory
- [ ] `ref <HUID>` walks the worktree and finds references in source and tasks
- [ ] `summary --json` reports counts by status, tag, and priority
- [ ] `todo <FILE...>` converts TODO comments without prompting when stdin is not a TTY
