# Add ready, claim and release with advisory locks

- STATUS: OPEN
- PRIORITY: 100
- TAGS: agents,storage

## Context

Parallel agents need a lease, not a status change. `STATUS` stays binary; claims live in reserved
properties (`OWNER`, `CLAIMED`, `EXPIRES`) so upstream `tatr` still reads the task.

## Acceptance

- [ ] `ready` returns tasks whose `DEPENDS` are all CLOSED, excluding `wip`, `blocked`, and unexpired claims
- [ ] `claim --owner agent-3 --ttl 2h` writes the three properties atomically and survives concurrent claims
- [ ] `release <HUID>` clears the claim; claiming an already-claimed task exits 4 with the current owner
- [ ] claimed tasks disappear from `ready` until expiry, then reappear
- [ ] `check` reports dangling `DEPENDS` references
