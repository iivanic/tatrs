# Add ready, claim and release with leases

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agents,storage
- OWNER: dsh

## Context

Parallel agents need a lease, not a status change. `STATUS` stays binary. Per `docs/DECISIONS.md`
D-002, leases are ephemeral runtime state and never enter `TASK.md`: they live one-file-per-task
under `<git-common-dir>/tatrs/claims/`, so every worktree of one clone sees them and nothing is
committed. `OWNER` stays the durable assignment field. Per D-003, visibility comes from the
read-only `claim --status` view; there is no `--publish`.

## Acceptance

- [x] `ready` returns `OPEN`, `agent`-tagged tasks with valid acceptance, all `DEPENDS` CLOSED, no `wip`/`blocked`, and no live claim
- [x] `claim <HUID> --owner agent-3 --ttl 2h --run <id>` writes `<git-common-dir>/tatrs/claims/<HUID>.json` atomically and returns a token
- [x] two concurrent claims on one task: exactly one succeeds, the other exits 4 with the owner and expiry
- [x] `release <HUID> --token <t>` is compare-and-delete; a stale token exits 4 and does not remove the new owner's claim
- [x] `refresh` extends expiry only for the matching token; expired claims are stealable without a `--steal` flag
- [x] `claim --status` lists live claims first, soonest expiry first, then expired ones; columns are task, owner, state, expiry, host, run
- [x] `claim --status --task <HUID>` filters and `--fail-if-empty` exits 5; `--json` matches the D-003 schema
- [x] `claim --status` works under `--read-only` and never writes; nothing about a lease is ever written to `TASK.md`
- [x] non-git repositories fall back to `tasks/.tatrs/claims/`, which `init` keeps out of git
- [x] `check` warns (never errors) on dangling or long-expired claim files; `check --fix` removes them
- [x] `tatrs claim --help` states the cross-machine limitation in one sentence

## Progress

Done 2026-11-09:

- `tatrs-core::claims` stores one JSON per task under the git common dir (linked worktrees
  supported via `gitdir:`/`commondir`), with a `tasks/.tatrs/claims` fallback.
- Claim/refresh/release are serialized by a per-task `O_EXCL` lock (stale after 5 s) and confirmed
  by token, so concurrent claimers have exactly one winner (`concurrent_claims_have_exactly_one_winner`).
- `ready` and `claim` share `tatrs-core::ready`, which enforces D-001 acceptance, tags, dependencies,
  and live leases.
- `check` reports orphaned/expired/corrupt claim files; `check --fix` removes them.
- `scripts/agent-flow.ps1` runs the whole JSON-only loop.
