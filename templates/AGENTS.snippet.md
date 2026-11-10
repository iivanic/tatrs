<!-- Appended under an existing heading in a consumer repo's AGENTS.md. -->
<!-- markdownlint-disable MD041 -->

## Tasks

This repository tracks work with `tatrs`: the backlog is `tasks/<HUID>/TASK.md`, one task per
directory.

- Before writing a `TODO:` comment for durable work, create a task:
  `tatrs new "title" -t <tags> --json`.
- Never edit `STATUS` or `TAGS` by hand. Use `tatrs ls -u close|open|+tag|-tag <HUID>`.
- An `agent`-tagged task needs a `## Acceptance` section with at least one `- [ ]` criterion, or
  `ACCEPTANCE: waived`. `ready` and `claim` refuse tasks without one.
- Before starting: `tatrs ready --json`, then `tatrs claim <HUID> --owner <name> --json`. Refresh or
  release the lease when you stop, and never claim a task held by someone else.
- Close a task only when its acceptance section passes, and tick the boxes.
- Use `--json` in scripts; commands are non-interactive and never prompt.
- `tatrs check --strict` is the health check before committing.
