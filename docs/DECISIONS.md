# Decisions

Architecture decision records for questions the code cannot answer on its own. Each entry states the
options, the choice, the exact behaviour that follows, and what would make us revisit it.

| ID | Decision | Status | Date |
|-|-|-|-|
| D-001 | Acceptance enforcement level | accepted | 2026-11-09 |
| D-002 | Claim storage and lifecycle | accepted | 2026-11-09 |

---

## D-001: how hard should `## Acceptance` be enforced?

### Context

`tatrs check` currently reports `W0301` for any open task without a `## Acceptance` section, and
`--strict` promotes warnings to a failure. `tatrs ready`/`claim` (M2) will dispatch work to agents;
an agent with no verifiable stopping condition will either loop or declare victory arbitrarily.

The question is not "are acceptance criteria good" but **where the guarantee should bind**: at task
creation, at commit, or at dispatch. Those have different failure modes.

### Options

| Option | Guarantee | Friction | Failure mode |
|-|-|-|-|
| A. Warning always (today) | none; opt-in via `--strict` | none | teams never run `--strict`; no guarantee survives |
| B. Error always for open tasks | strong | high | legacy trees turn red; quick capture is blocked; people write ritual `- [x] done` to silence it |
| C. Error only for tasks tagged `agent` | exactly where machines act | none for human backlog | depends on the tag being the dispatch contract (it is, see below) |
| D. No `check` enforcement; gate only at `ready`/`claim` | strong at dispatch | none at authoring | bad tasks merge silently; nothing fails until someone tries to run them |
| E. C + hard gate at dispatch | strongest practical | proportional | two rules to keep aligned |

Option B is the tempting one and the wrong one. `tatrs` sells itself as drop-in compatible with
existing `tasks/` trees; the upstream corpus alone has 27 open tasks with no acceptance section, and
any team adopting the tool on a live repo would get a wall of red for work that is already fine.
More importantly, a mandatory section manufactures compliance: the cheapest way to pass is
`- [ ] it works`, which is worse than an honest absence because it looks verifiable. Lint that
cannot distinguish a real criterion from a ritual one should not be the thing that blocks a merge.

Option D is also tempting — keep `check` purely advisory and make dispatch the gate — but it lets a
PR add an unrunnable agent task and merge green. The dispatch failure then surfaces later, in a
different session, with less context.

### Decision

**Adopt E.** Severity is a function of the promise the task makes:

| Task state | No `## Acceptance` | Section present, no criteria | Criteria present |
|-|-|-|-|
| `CLOSED` | nothing | nothing | nothing |
| `OPEN`, no `agent` tag | `W0301` warning | `W0304` warning | ok |
| `OPEN`, `agent` tag | `E0301` **error** | `E0304` **error** | ok |
| any, `ACCEPTANCE: waived` | `I0301` info | `I0301` info | ok |

And a second, absolute layer: `ready` and `claim` refuse any task that fails the `agent` column,
regardless of `check` settings. Authoring stays cheap; dispatch is strict.

### Exact behaviour

1. **What counts as acceptance.** An H2 heading whose text starts with `Acceptance`
   (case-insensitive), containing at least one checkbox item with non-empty text:
   `^\s*[-*]\s+\[[ xX]\]\s+\S`. Prose alone does not count; neither does `- [ ]` with nothing after
   it. This is what makes the `--acceptance` template intentionally invalid until filled in.
2. **`agent` tag** is the dispatch contract: `ready` only serves tasks carrying it, so tagging is
   the moment a task promises to be machine-executable. Severity follows that promise.
3. **Waiver.** `ACCEPTANCE: waived` (single-token key, valid per `docs/FORMAT.md`) downgrades to
   info. A waiver is greppable and reviewable; silence is not. No reason field — the reason belongs
   in the body, where a reviewer sees it.
4. **CLOSED tasks are never linted.** Retro-linting history punishes work that was done before the
   rule existed and makes `check` unusable on old trees.
5. **`new` warns, never blocks.** Creating an `agent` task without criteria prints a one-line
   stderr warning: it will not appear in `ready`. Creation stays a cheap capture operation.
6. **`ready` filter** (M2): `OPEN`, tag `agent`, valid acceptance or waiver, not `wip`/`blocked`,
   all `DEPENDS` CLOSED, no unexpired claim.
7. **`claim` on a non-ready task** exits `3` (invalid data) with a diagnostic naming the failed
   condition, so an orchestrator can branch without parsing prose.
8. **JSON:** task objects gain an additive `acceptance` object —
   `{"present": bool, "criteria": n, "waived": bool, "ready": bool}`. `schema` stays `1`.
9. **CI:** `tatrs check --strict` remains the recommended command; it now catches `E0301` for free
   and `W0301` for repos that want the stricter human rule too.

### Consequences

**Positive.** Adoption on existing trees costs nothing (warnings only). The guarantee binds exactly
where a machine can cause damage. Dispatch remains the single place that must be correct, so a
future orchestrator cannot bypass it by not running `check`. The rule is explainable in one sentence
to a contributor: "if you tag it `agent`, it must say how it ends."

**Negative.** A task can be tagged `agent` and fail CI, which is the point but is still friction;
the fix is editing the task, not disabling the lint. The rule couples acceptance to a naming
convention, so tooling must treat `agent` as reserved. Teams that never use agents get only warnings
and may never act on them — acceptable, since they are not exposed to the risk.

**Mitigations.** `new -t agent` warns immediately; `ready --json` explains why a task is excluded
(`"blocked_by": ["acceptance"]`); `--lenient` still forces exit 0 for migration.

### Revisit if

- Tasks show ritual criteria (`- [ ] done`, `- [ ] works`) at a measurable rate; then presence checks
  are not buying anything and the `agent`-tag rule should be replaced by a human review gate.
- Most `agent` tasks are waived; then the tag is not a real contract and dispatch should require an
  explicit `--accept-any` flag instead.
- `ready` stops being the dispatch path; then the hard gate has to move to `claim` only.

---

## D-002: where do claims live?

### Context

Parallel agents need mutual exclusion: two workers must not pick the same task. A claim is a
**lease** — owner, acquisition time, expiry — and it is **ephemeral**: it matters for minutes to
hours, then it is meaningless. `TASK.md` is the opposite: durable, reviewed, merged, and part of
history.

The topology matters more than the storage format:

- N agents in one clone, each in its own `git worktree` (the documented model). Worktrees share
  `.git` but not a working tree.
- N agents in one working tree (discouraged, but possible).
- Agents on different machines, coordinating through a remote.

### Options

| Option | Visible across worktrees | Merge behaviour | History noise | Crash recovery | Complexity |
|-|-|-|-|-|-|
| A. Properties in `TASK.md` | only after commit + pull | conflicts with body edits and other claims | one commit per claim/refresh/release | stale claim until someone commits a cleanup | low |
| B. Per-task file `tasks/<HUID>/CLAIM` | no (per worktree) | none if ignored, conflicts if committed | none | file expires, needs GC | low |
| C. One map file `tasks/.claims.json` | no (per worktree) | none if ignored | none | same | medium; single-writer contention |
| D. Store under the **git common dir** | **yes, within one clone** | never tracked | none | expires; `check --fix` GCs | medium |
| E. Branch/ref or git notes | yes after push | ref updates, no conflicts | commit/ref noise | manual | high |
| F. External service | yes | n/a | none | server-side | out of scope |

Option A is the intuitive choice and the one the original plan assumed. It fails on its own terms:
a claim written in worktree A is invisible to agent B until A commits and B pulls, so the two agents
race exactly when claims are supposed to prevent racing. It also puts churn in the file humans
review, and forces every claim to dirty `git status` — breaking the "agents leave the tree clean"
expectation and colliding with any concurrent body edit.

Option B fixes the noise but not the visibility: each worktree has its own `tasks/`, so claims are
still per-worktree. Option C does not fix it either, and adds a single hot file.

Option D is the sweet spot for the actual deployment: **all worktrees of one clone share
`.git`**, so a store there is visible to every agent in the clone, never tracked, never merged, and
survives `git clean` and branch switches.

### Decision

**Adopt D, with B as a fallback for non-git directories.** Claims are runtime state and never enter
`TASK.md`. Durable accountability is a separate concept and stays in the task as `OWNER`.

- **Location:** `<git-common-dir>/tatrs/claims/<HUID>.json`. Resolve `.git` by walking up; if it is a
  file (linked worktree), read `gitdir:` and take its parent's parent as the common dir. Without git:
  `<repo>/tasks/.tatrs/claims/<HUID>.json`, ignored by a generated `tasks/.gitignore`.
- **One file per task**, created atomically (temp + rename), so no global lock is needed.

### Claim file

```json
{
  "task": "20261109-123637",
  "owner": "agent-3",
  "token": "5f0c1b9e2a",
  "acquired_at": "2026-11-09T13:41:02Z",
  "expires_at": "2026-11-09T15:41:02Z",
  "host": "desk",
  "pid": 41232,
  "run": "dsh-run-42"
}
```

`token` is a random hex string generated at claim time. It is the agent's proof of ownership and is
returned once by `claim`; `release` and `refresh` must present it. Timestamps are UTC ISO-8601.

### Protocol

- **claim** — validate readiness first (see D-001). If a live claim exists for another owner, exit
  `4` with `{"claimed": false, "owner": "...", "expires_at": "..."}`. Otherwise write the file and
  re-read it to confirm the token: two simultaneous claimers both rename, the loser sees the
  winner's token and exits `4`. Same owner re-claiming is a refresh.
- **refresh** — rewrite only if the stored token matches; extend `expires_at`. This is the
  heartbeat, expected every `ttl/2`.
- **release** — delete only if the token matches. A stale agent cannot release the new owner's
  claim (compare-and-delete closes the ABA window).
- **steal** — implicit: if the stored claim is expired, the next `claim` overwrites it. No
  `--steal` flag; expiry is the only permission needed.
- **TTL** — default `2h`, `--ttl 30m` overrides. Expiry is evaluated on read; there is no daemon.
- **GC** — `check --fix` removes expired claims for closed or missing tasks; `check` reports
  dangling claims as warnings only, never errors.

### `OWNER` versus the lease

`OWNER` in `TASK.md` means "this person or agent is accountable for the task" — durable, committed,
reviewable, and part of the task's story. The lease means "this process is executing right now" —
ephemeral, local, and invisible to reviewers. Conflating them makes git history noisy and makes
accountability expire in two hours. If a human assigns a task, they may write `OWNER`; when an agent
picks it up, only the lease changes.

### Consequences

**Positive.** Worktree-parallel agents coordinate for free. Claims never appear in diffs, never
conflict, and never need a commit. A crashed agent blocks a task for at most the TTL, and its
`RUN`/`host`/`pid` remain for post-mortem. The working tree stays clean, which keeps agent git
workflows (commit exactly what you changed) honest.

**Negative.** Claims do not cross machines or clones: a fresh clone thinks everything is free. This
is a real limitation and must be documented, not papered over — coordinating across machines needs a
queue or the remote tracker, not a local file. Claims also disappear if `.git` is deleted, and they
are advisory: a process that ignores the protocol can still work on a claimed task.

**Mitigations.** `ready` never lists tasks with a live claim, so cooperative agents never collide.
`--read-only` and MCP write-gating keep accidental claims rare. The limitation is stated in
`docs/PLAN.md` and in `tatrs claim --help`.

### Rejected alternatives

- **Committed claim properties** — visibility after push is too late, history noise, and conflicts
  with the file's real content.
- **Git notes / orphan branch** — atomicity and push semantics are worse, and agents would need git
  plumbing rather than a file write.
- **Single central JSON map** — a hot file, a custom lock, and a corruption blast radius, for no gain
  over one file per task.
- **File locks as the claim itself** — an OS lock dies with the process and is invisible to
  inspection; a TTL lease survives crashes and can be explained in `bytes`.

### Revisit if

- Agents commonly run across several machines or clones; then add an opt-in commit-based
  "announcement" (`tatrs claim --publish` writes `OWNER` for humans) or a queue integration.
- Claim churn exceeds TTL accuracy (very long tasks); then heartbeat wiring belongs in the
  orchestrator, not in the CLI.
- Windows path resolution of `.git` files proves unreliable in linked worktrees; then fall back to
  `--claims-dir` configuration.
