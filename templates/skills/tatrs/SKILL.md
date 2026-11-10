---
name: tatrs
description: "Track work as `tasks/<HUID>/TASK.md` with the `tatrs` CLI. Use when the repository has a `tasks/` directory, when the user asks to create, list, claim, close, summarize, or check tasks, or when durable work needs a ticket instead of a `TODO:` comment."
---

# tatrs

`tatrs` is a git-native task tracker. Every task is `tasks/<HUID>/TASK.md`, where the HUID is a UTC
timestamp such as `20261109-133012`. There is no database and no daemon: the files are the state,
so a task is reviewable in a diff and readable in any editor.

## When to use

- The repository contains `tasks/` (check with `test -d tasks`).
- The user asks for a ticket, backlog item, or to "track this" / "close that".
- Work will outlive the current change and would otherwise become a `TODO:` comment.

Do not create tasks for trivial in-flight edits; a commit message is cheaper.

If `tatrs` is not installed, say so and offer `cargo install --path crates/tatrs-cli` (or a release
binary). Do not silently fall back to editing `tasks/` by hand.

## The loop

```console
$ tatrs ready --json                     # tasks an agent may start right now
$ tatrs claim 20261109-133012 --owner agent-3 --run run-42 --json
$ tatrs ls -u "+wip" 20261109-133012     # tags: +tag / -tag, close, open
$ tatrs ls -u close 20261109-133012      # done only when acceptance passes
$ tatrs release 20261109-133012 --token <token>
```

Create a task with a verifiable exit condition:

```console
$ tatrs new "Fix flaky parser test" -t bug,agent -p 10 --body "## Acceptance

- [ ] the flaky case runs 100 times without a failure" --json
```

Read it back with `tatrs show <HUID> --json` (includes the body) or `tatrs ls --json`. Summarize the
tree with `tatrs summary --json`; validate it with `tatrs check --strict`.

## Rules

- **Never edit `STATUS` or `TAGS` by hand.** Use `new` and the update language (`tatrs ls -u ...`);
  writes are atomic and preserve unknown properties.
- **`agent` is the dispatch contract.** An agent-tagged task must carry `## Acceptance` with at
  least one `- [ ]` criterion, or `ACCEPTANCE: waived`. `ready` and `claim` refuse anything else.
- **Claim before starting**, refresh with `claim <HUID> --token <token> --ttl 2h`, and release when
  you stop. A lease expires; expiry is the only permission needed to take over, so never claim a
  task held by someone else.
- **Close only when the criteria pass**, and tick the boxes in the body. Closing without doing so is
  reported as `unverified` on the live board.
- **Keep the body for notes**: logs, decisions, links. The property block is metadata.
- **No prompts.** Every command is non-interactive; use `--json` and branch on exit codes.

## Exit codes

| Code | Meaning |
| - | - |
| 0 | success |
| 1 | runtime or I/O error |
| 2 | usage error |
| 3 | invalid data, or the task is not dispatchable |
| 4 | lease held by another owner, or lock contention |
| 5 | not found |

## Beyond the CLI

- `tatrs serve` — read-only live board (derived states, attachments) on loopback.
- `tatrs mcp` — the same database over MCP; add `--allow-write` for create/claim/update.

Details: `docs/FORMAT.md` (grammar), `docs/QUERY.md` (TQL), `docs/CLAIMS.md` (leases and readiness),
`docs/JSON.md` (schemas), `docs/DASHBOARD.md` (live board).
