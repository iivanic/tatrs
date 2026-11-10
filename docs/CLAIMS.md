# Claims and readiness

Parallel agents must not pick the same task. `tatrs` uses short-lived **leases**: a claim names an
owner, carries a random token, and expires. Leases are runtime state, never part of `TASK.md`
(`docs/DECISIONS.md` D-002 and D-003).

## Where leases live

| Project | Location |
|-|-|
| git repository | `<git-common-dir>/tatrs/claims/<HUID>.json` — shared by every worktree of the clone |
| plain directory | `tasks/.tatrs/claims/<HUID>.json` — kept out of git by the generated `tasks/.gitignore` |

Nothing is committed, so claims never show up in diffs, reviews, or history. The flip side is that
claims are **per clone**: a fresh clone on another machine considers every task free. Cross-machine
coordination needs a queue, not a local file.

## The flow

```console
$ tatrs ready --json                 # what may start now
$ tatrs claim 20261109-133012 --owner agent-3 --run run-42 --json
{"schema":1,"claimed":true,...,"token":"000000006af1d5910000fb7c","expires_at":"..."}

$ tatrs claim 20261109-133012 --status --json    # who holds what
$ tatrs claim 20261109-133012 --token <token> --ttl 2h   # heartbeat
$ tatrs release 20261109-133012 --token <token>
```

- `claim` refuses a task that is not dispatchable (exit `3`) and a live claim held by someone else
  (exit `4`).
- `--ttl` defaults to `2h`; refresh at `ttl/2`.
- Expiry is the only permission needed to steal: a crashed agent blocks a task for at most the TTL.
- `release` is compare-and-delete. A stale token cannot delete the new owner's claim.
- Claims disappear from `ready` while live; expired claims are shown by `claim --status` so a human
  can see what died.

## What makes a task ready

`ready` (and `claim`) require all of:

1. `STATUS: OPEN`;
2. the `agent` tag (the dispatch contract);
3. a satisfied acceptance section — a non-empty `- [ ]` criterion, or `ACCEPTANCE: waived`;
4. no `wip`, `blocked`, or `paused` tag;
5. every `DEPENDS` entry closed and present;
6. no live claim.

`ready --explain` lists the excluded tasks with the exact reason.

## Tokens and trust

The token is generated from the clock, process id, and a counter. It proves ownership in
`refresh`/`release`; it is not a security boundary — anyone with local write access can remove a
claim file. Leases stop cooperating agents from colliding, not adversaries.

## Exit codes

| Code | Meaning |
|-|-|
| 0 | claimed, refreshed, released, or listed |
| 3 | task is not dispatchable (readiness blockers printed to stderr) |
| 4 | lease held by another owner, or token mismatch |
| 5 | task or claim not found (`--fail-if-empty` for status) |
