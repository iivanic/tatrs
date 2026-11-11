# Decisions

Architecture decision records for questions the code cannot answer on its own. Each entry states the
options, the choice, the exact behaviour that follows, and what would make us revisit it.

| ID | Decision | Status | Date |
| - | - | - | - |
| D-001 | Acceptance enforcement level | accepted | 2026-11-09 |
| D-002 | Claim storage and lifecycle | accepted | 2026-11-09 |
| D-003 | Claim visibility: status command, no published claims | accepted | 2026-11-09 |
| D-004 | Supervisor states: derive, do not extend `STATUS` | accepted | 2026-11-10 |
| D-005 | Live board transport: SSE + sweep, loopback only | accepted | 2026-11-10 |
| D-006 | Board recovery actions and tokenless lease release | accepted | 2026-11-11 |

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
| - | - | - | - |
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
| - | - | - | - |
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
6. **`ready` filter** (M2): `OPEN`, tag `agent`, valid acceptance or waiver, not `wip`/`blocked`
   (`paused` was added later, see D-004), all `DEPENDS` CLOSED, no unexpired claim.
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
| - | - | - | - | - | - |
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

- **Location:** `<git-common-dir>/tatrs/claims/<HUID>.json`. Resolve `.git` by walking up; if it
  is a file (linked worktree), read `gitdir:` and take its parent's parent as the common dir.
  Without git: `<repo>/tasks/.tatrs/claims/<HUID>.json`, ignored by a generated
  `tasks/.gitignore`.
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
- **serialization** — the read-modify-write is guarded by a per-task `<HUID>.lock` created with
  `O_EXCL` (stale after 5 s, wait 2 s), so claim/refresh/release cannot interleave with a steal.
  The post-write token check remains as a second guard.
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

- Agents commonly run across several machines or clones; then add a queue integration rather than
  publishing claims (see D-003).
- Claim churn exceeds TTL accuracy (very long tasks); then heartbeat wiring belongs in the
  orchestrator, not in the CLI.
- Windows path resolution of `.git` files proves unreliable in linked worktrees; then fall back to
  `--claims-dir` configuration.

---

## D-003: how do humans see active claims?

### Context

D-002 keeps leases out of git, which means a reviewer looking at a PR cannot see that an agent is
currently executing a task. The obvious remedy — an opt-in `tatrs claim --publish` that writes
`OWNER`/`CLAIMED`/`EXPIRES` into `TASK.md` — reintroduces exactly the problems D-002 rejected:
commit churn, conflicts with body edits, stale claims in history, and a dirty working tree for the
duration of a run.

### Decision

**No published claims.** The lease stays local to the clone, and `tatrs claim --status` is the
visibility mechanism. A reviewer who needs to know runs one command; nothing about the task changes.

### Exact behaviour

`tatrs claim --status` reads the claim store, never writes, and works under `--read-only`.

```console
$ tatrs claim --status
TASK                 OWNER     STATE    EXPIRES      HOST   RUN
20261109-123637      agent-3   live     in 1h 12m    desk   dsh-42
20261109-123637-01   agent-2   expired  3h 04m ago   desk   dsh-41
```

- **Ordering:** live claims first, soonest expiry first; then expired claims, most recently expired
  first. Expired rows are shown rather than hidden — they are the ones a human needs to clean up.
- **State:** `live` when `expires_at > now`, `expired` otherwise. Expiry is evaluated on read; there
  is no daemon and no background sweep.
- **Filters:** `--task <HUID>` (repeatable) restricts the listing. `--fail-if-empty` exits `5` when
  nothing is claimed, for scripts that wait on quiescence.
- **Exit codes:** `0` normally; `5` with `--fail-if-empty` and no claims; `1` when the claim store
  cannot be read. Corrupt claim files are reported to stderr and skipped, never fatal for `status`
  (they are `check`'s problem).
- **JSON:**

```json
{"schema": 1, "claims": [
  {"task": "20261109-123637", "owner": "agent-3", "state": "live",
   "acquired_at": "2026-11-09T13:41:02Z", "expires_at": "2026-11-09T15:41:02Z",
   "remaining_seconds": 4320, "host": "desk", "pid": 41232, "run": "dsh-42"}
]}
```

`remaining_seconds` is negative once expired, so a consumer needs no clock of its own.

- **Not part of status:** releasing, stealing, or extending a claim. Those require the token and stay
  with `claim`/`release`; `--status` is a read-only view.

### Consequences

**Positive.** No new file format, no commit churn, and no way for a stale lease to leak into
history. `status` answers both questions at once: "who is running what" and "what died and needs
cleanup". It is cheap enough to run before every manual task pick.

**Negative.** A PR reviewer with only a browser cannot see claims; they need a clone and the CLI, or
CI output that runs `tatrs claim --status --json`. Claims also remain invisible across machines
(D-002), so `status` on a different clone will show nothing — it is a local view, and the help text
must say so.

**Mitigation.** `check` continues to report dangling/expired claim files, so a repo that never runs
`status` still surfaces orphans in its normal health check.

### Rejected alternatives

- **`--publish` into `TASK.md`** — reintroduces commit churn, conflicts, and stale history; a lease
  that outlives a crash becomes a permanent lie in the task file.
- **Claim state in PR comments / CI artifacts** — external systems, extra auth, and it still goes
  stale.
- **A local web UI** — out of scope here; D-005 later added a read-only board that renders leases
  without publishing them, so this rejection covers only *writing* claim state into the task.

### Revisit if

- Reviewers routinely need claim state while reviewing; then emit `claim --status --json` as a CI
  artifact or PR check summary — not as committed task data.
- `status` listings grow beyond a screen; then add `--limit` and a `--since` filter rather than a
  new store.

---

## D-004: where do supervisor-facing states live?

### Context

A human supervising parallel agent work does not need `tasks/<HUID>/TASK.md`; they need to know
*what is happening*: received, in progress, paused, waiting, completed, refused. Today that has to be
reconstructed from `STATUS`, tags, leases, and `DEPENDS`.

Two of those six are **decisions** that no observation can recover — "we paused this" and "we refuse
to do this". The rest are observable: a task exists, a lease is held, a dependency is open, a task is
closed. `STATUS` is binary and is part of the compatibility contract (`docs/FORMAT.md` §3.2, §6):
upstream knows two values, `E0102` rejects anything else, and every consumer filters on `OPEN` /
`CLOSED`.

### Options

| Option | Where state lives | Upstream compatibility | Staleness | Cost |
| - | - | - | - | - |
| A. Extend `STATUS` to six values | one field in `TASK.md` | **breaks**: new enum, `E0102` rewrite, query semantics change | every transition is a commit | migration, doc rewrite, every filter and the MCP contract |
| B. Derive everything | nowhere | none | impossible | cannot express `paused` or `refused` |
| C. One `STATE` property | unknown property, preserved by upstream | medium: a second dialect; tools must learn it | two sources of truth beside `wip`/`blocked` | validation rules, precedence rules |
| D. **Derive observed, declare decisions with reserved tags** | tags, already free-form | none | impossible for observed states | a convention, documented once |

Option A is the intuitive answer and the wrong one. It makes `STATUS: PAUSED` a *durable fact about
a moment*, so a worker that pauses and disappears leaves a lie in git; it changes what `status eq
OPEN` means for every existing script; and it turns "started work" into a file write, which is
exactly the churn D-002 rejected for leases.

Option B is clean but incomplete: a pause is a human choice, not an absence of evidence. A task with
no lease is indistinguishable from one whose worker died.

Option C is the closest competitor. It has one advantage — a single validated field that can carry a
reason — but it introduces a second lifecycle vocabulary next to the tags the project already
reserved, and `STATE: WIP` would contradict the `wip` tag. Tags are where `docs/FORMAT.md` §6 already
put lifecycle, and unknown tags are already preserved by both tools.

### Decision

**Adopt D.** Nothing new is persisted beyond three reserved tags. `tatrs-core::lifecycle` is the only
place the mapping lives, and it is pure: the caller supplies the clock, the lease, and the last file
change.

### Exact behaviour

1. **Precedence** is normative in `docs/DASHBOARD.md` §2: `CLOSED` → `in_progress` (live lease, else
   `wip`) → `paused` → `waiting` → `received`. A live lease beats a declared pause; the conflict is
   reported as `paused-but-claimed`, never silently resolved.
2. **`STATUS` is unchanged.** `OPEN`/`CLOSED`, `E0102`, `render`, the upstream corpus, and every TQL
   query behave exactly as before.
3. **New reserved tags:** `paused`, `refused`, `notify` (`docs/FORMAT.md` §6). They are ordinary
   tags: preserved through rewrites, invisible to upstream, greppable, and reviewable.
4. **Dispatch excludes `paused`.** `ready`/`claim` already refused `wip` and `blocked`; a paused task
   is not dispatchable either, even if a stale `agent` tag and acceptance criteria are present.
5. **`refused` requires a reason.** The tag closes the task; the reason is prose in the body,
   where a reviewer sees it. `check` does not enforce a reason — the task is already closed and
   history should not be relitigated.
6. **Flags are data, not warnings.** `unverified`, `notify-owner`, `stalled`, `expired-claim`,
   `needs-human`, `paused-but-claimed`, `claim-on-closed-task` are computed per task and exposed in
   JSON. No new diagnostic code is emitted, so `check` output is unchanged.
7. **Additive JSON only.** `summary --json` gains `lifecycle`; the task object gains
   `acceptance.checked`. `schema` stays `1`.
8. **No transition command.** There is no `tatrs pause` in this decision. Pausing is `+paused` through
   the existing update language, which keeps one write path.

### Consequences

**Positive.** Adoption is free: an untouched tree gains a derived view without a single file change.
The board cannot disagree with the files, cannot show a state that was never written, and cannot
leave a stale `PAUSED` after a crash. Refusals and pauses remain reviewable in normal diffs.

**Negative.** Two of six states depend on people or agents remembering a tag. A worker that stops
without tagging `paused` looks `received` or `stalled`; the `stalled` flag exists to surface that, but
it needs a lease to compare against. `notify` is a convention with no enforcement, so "completed but
not notified" is only visible if the tag was applied.

**Mitigations.** `unverified` catches the most common false completion (criteria left unchecked);
`stalled` catches silent workers; `needs-human` is already reserved and now surfaces as a flag.

### Revisit if

- Tasks routinely need a *reason* attached to `paused`/`refused` in machine-readable form; then
  promote to a `STATE` property (option C) and treat the tags as aliases during migration.
- Most `refused` tasks end up lacking a reason, making the state meaningless; then require a
  `## Reason` heading for closed-by-refusal tasks and emit a diagnostic.
- A team needs states beyond these six; derive them as flags first (cheap, reversible) before adding
  any stored vocabulary.

---

## D-005: how does the live board stay current?

### Context

The board has one writer path (`tasks/` plus the local claim store) and one direction of data (server
to browser). It must stay responsive while the human reads code or Markdown in the same page, and it
must not add a dependency for a feature that is essentially a local file view.

### Options

| Concern | Options | Trade-off |
| - | - | - |
| Push transport | WebSocket | bidirectional, needs a dependency and frame/handshake parsing |
| | **SSE (`text/event-stream`)** | one direction, plain HTTP, automatic reconnect, ~50 lines on `std` |
| | Page polling | simplest, but every client polls and `tatrs` cannot signal change |
| Change detection | inotify/FSEvents (`notify`) | instant, but platform-specific runtime and event coalescing |
| | **`stat` sweep with incremental parse** | portable, catches other worktrees and NFS, idle cost is one `stat`/file/tick |
| Serving UI | external build step | asset pipeline and a second toolchain |
| | **`include_str!` single file** | no build step, no dependency, one request |

### Decision

**SSE + sweep + embedded HTML.** The watcher thread recomputes only when a `(len, mtime)` changed;
the result is serialized once, versioned, and published to every subscriber through a `Condvar`.

### Exact behaviour

- **Default bind** `127.0.0.1:7787` (`--port 0` picks a free port). Any non-loopback address is
  refused unless `--allow-remote` is passed.
- **Token.** A per-session capability token is printed in the URL (`/?t=…`); `/` sets it as an
  `HttpOnly; SameSite=Strict` cookie so `EventSource` and `fetch` authenticate. Requests without it
  get `401`.
- **Sweep** every `--poll` (default `1s`, `ms` suffix accepted). Per tick: `read_dir` + `stat` of each
  `TASK.md` and claim file; parse only changed files. Removed files are dropped from the cache.
- **Snapshot** is bumped on every rebuild and broadcast as one `board` event:
  `{"schema":1,"version":n,"generated_at":…,"counts":{…},"tasks":[…]}`. Bodies are excluded; task
  detail is fetched separately.
- **Keepalive** comment every 15 seconds so idle proxies do not close the stream.
- **Staleness** of a live lease is `--stale` (default `2h`) since the last file change, not since the
  claim began: an agent that keeps editing is not stalled.
- **Browsing** is off with `--no-browse`; otherwise `/api/tree` and `/api/file` serve only paths under
  the canonical root, never `.git`/`target`/`node_modules`/`.venv`/`dist`/`build`/`.tatrs`, never
  files above `--max-file` KiB, and never non-UTF-8 content.
- **Read-only.** There are no mutating routes, so `--read-only` is trivially satisfied.

### Scale

At 1 000 tasks a sweep is a few milliseconds and a change re-parses one file; snapshots are ~150 KB
and are sent only when something changed. At 2 000 tasks the same holds at roughly double the cost
(`docs/DASHBOARD.md` §5). The sweep is the floor: one `stat` per file per tick, whether or not
anything changed.

### Revisit if

- A tree passes ~10 000 tasks or several clients watch it: send per-entry deltas instead of full
  snapshots, and consider raising the default `--poll`.
- `stat` sweeps prove unreliable on a network filesystem: add a checksum of the task file, or fall
  back to `notify` behind a feature flag.
- The board needs more actions than D-006 allows: keep the allowlist small and route anything that
  edits prose, priority, or arbitrary tags through the CLI/MCP.

---

## D-006: board recovery actions and tokenless lease release

### Context

A read-only board can only *report* an orphaned lease, a paused task, or a closed task with an
unchecked criterion. Recovery then means switching to a terminal, finding the task, and typing the
right update. That is the exact context switch a supervisor should not need, especially when the
board already knows which flag is wrong.

The risky part is leases. `release` normally requires the token returned by `claim`; a board has no
token. Releasing someone else's lease is new authority and needs an explicit decision rather than an
incidental side effect of adding buttons.

### Decision

Add a single mutating route, `POST /api/action`, behind `tatrs serve --allow-actions` (off by
default). It accepts only a named intent for one task; there is no free-form edit.

| Intent | Effect |
| - | - |
| `release-lease` | force-release the task's lease (tokenless, see below) |
| `resume` | `-paused` |
| `unblock` | `-blocked` |
| `reopen` | `open` |
| `mark-notified` | `-notify` |
| `clear-needs-human` | `-needs-human` |

Each maps to `tatrs-core::update` or to `ClaimStore::force_release`, runs under
`tasks/.tatrs.lock`, saves atomically, and writes a `UPDATED` property so the change is visible in
`git diff`. Repeating an intent that no longer applies is a no-op (`changed: false`).

The UI renders a button only for a flag or state it resolves, and every button asks for
confirmation before the POST.

### Authority and blast radius

- The route is authorized by the same per-session token as the rest of the board (`HttpOnly`,
  `SameSite=Strict` cookie, or `?t=`), and the default bind is loopback. `--allow-remote` widens
  reach; it stays token-gated.
- **Tokenless release** lets any holder of the session token stop any lease on the clone. It cannot
  alter task text, priority, arbitrary tags, or close tasks. The worst case is coordination loss:
  two agents may work the same task; no data is deleted, and the next `check` still sees the lease.
- Releasing a lease does not notify the running agent. It may continue working while another agent
  can now claim the same task — the supervisor's action is advisory, not a kill signal.
- All actions are git-visible: the intent changes a tag or `STATUS`, and `UPDATED` records when.
  There is no hidden mutation and no server-side state to reconcile.
- The route is absent (404) unless `--allow-actions` is set; there is no read-only fallback that
  silently ignores a POST.

### Consequences

**Positive.** A supervisor can clear the four common stalls from the board, with a confirmation,
without a terminal; every change is reversible with the opposite intent or a normal update. The
authority is explicit, small, and audited.

**Negative.** The board becomes a write surface with a shared token; a leaked URL lets a third party
release leases (not edit prose). The intents encode lifecycle assumptions that must stay in sync
with `lifecycle.rs`; a new flag needs a matching intent or it has no button.

### Revisit if

- A new flag appears that has no safe, named recovery: add one intent and one test, not a generic
  tag editor.
- Actions prove too coarse for multi-agent supervision: move them to an authenticated MCP tool with
  the same allowlist, and retire the HTTP route.
- Tokenless release is abused in a shared clone: require an `OWNER` match or an explicit
  `--allow-release` separate from `--allow-actions`.
