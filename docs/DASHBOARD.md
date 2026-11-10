# Derived lifecycle and the live board

How `tatrs` turns a tree of `OPEN`/`CLOSED` files into something a human can supervise. This file is
normative for the mapping; `docs/DECISIONS.md` D-004 explains why the mapping is derived instead of
stored, and D-005 explains the transport.

## 1. Why not extend `STATUS`

`STATUS` is part of the compatibility contract: upstream `tatr` knows two values, `E0102` rejects
anything else, and every query (`status eq OPEN`, `ls --closed`, `ready`) is written against it.
Adding `PAUSED`, `WAITING`, or `REFUSED` would fork the dialect, change the meaning of existing
filters, and put a second, staler copy of runtime state into git.

The states a supervisor wants are either **observable** (a lease exists, a dependency is open) or
**declared** (someone decided to pause or refuse). Observing costs nothing and cannot go stale;
declaring needs one durable, greppable marker. So:

- observed states come from data that already exists (status, leases, `DEPENDS`, file activity);
- declared states use reserved tags (`docs/FORMAT.md` §6), which round-trip through upstream
  unchanged and survive every rewrite.

Nothing is written when a task changes state. There is no transition command, no migration, and no
way for the board to disagree with `TASK.md`.

## 2. States

| State | Meaning for a human | Evidence |
| - | - | - |
| `received` | the ticket exists, nothing has started | `OPEN` and none of the signals below |
| `in_progress` | someone is on it | live (unexpired) lease, or the `wip` tag |
| `paused` | deliberately stopped, usually starved of resources | `paused` tag (declared) |
| `waiting` | cannot proceed yet | an open or missing `DEPENDS` entry (observed), or the `blocked` tag (declared) |
| `completed` | done | `CLOSED` without `refused` |
| `refused` | closed without being done; the reason is in the body | `CLOSED` with `refused` (declared) |

### Precedence

Evaluated top to bottom; the first match wins:

1. `STATUS: CLOSED` → `refused` if tagged `refused`, otherwise `completed`.
2. A live lease (or, without one, the `wip` tag) → `in_progress`.
3. The `paused` tag → `paused`.
4. An open/missing dependency, or the `blocked` tag → `waiting`.
5. Otherwise → `received`.

A live lease beats a declared pause, and an open task can be `in_progress` even when a dependency is
open (a stale dependency). Both are reported as flags rather than silently resolved — see §3. A
closed task with `paused` is `completed`: closing wins, because the task's history is already
decided.

`declared` is `true` when the state rests only on a tag (`paused`, `blocked`, `wip`, `refused`) and
`false` when it rests on observed evidence (a lease, a dependency edge, `CLOSED` without a refusal).
A UI should render the two differently: "in progress" from a lease is a fact, from `wip` it is a
claim.

### One state, several conditions

Precedence is total, so a task always has **exactly one** `state`; the six counts therefore partition
the tree and always add up to `total`.

Several conditions can still hold at once. A paused task whose `DEPENDS` entry is still open is both
`paused` and `waiting`; a task tagged `wip` whose dependency is open is `waiting` but has already
started. Those losing conditions are not discarded: `Lifecycle.also` (JSON `also`, ordered by board
rank) lists them, and the dashboard renders them as `also waiting` next to the primary pill. The
rule is: **count every task once, show every condition that holds.**

## 3. Flags

Flags are orthogonal to the state. They are the part a human actually acts on.

| Flag | Condition | Why it matters |
| - | - | - |
| `unverified` | `CLOSED`, with at least one acceptance criterion still `- [ ]` | the task says done, its criteria say otherwise |
| `notify-owner` | `CLOSED` and tagged `notify` | work is finished but the owner has not been told |
| `stalled` | live lease, no file change for longer than `--stale` (default 2h) | the agent may be looping or dead while still holding the task |
| `expired-claim` | a lease exists but has expired, on an open task | a crashed worker; the task is stealable again |
| `needs-human` | tagged `needs-human` | a decision is required, not more time |
| `paused-but-claimed` | `paused` tag and a live lease | two conflicting signals; clean one of them up |
| `claim-on-closed-task` | a lease exists for a `CLOSED` task | stale lease, or a worker that closed without releasing |

`ready` is not a flag but is computed per task: it is exactly `docs/DECISIONS.md` D-001's dispatch
rule (`ready::evaluate`), so the board and `tatrs ready` can never disagree.

## 4. Counting

`lifecycle::Counts` is the board header:

| Field | Meaning |
| - | - |
| `total` | every task, open and closed |
| `received`, `in_progress`, `paused`, `waiting` | open work by state |
| `completed`, `refused` | closed work by outcome |
| `ready` | open tasks a dispatcher could start now (subset of `in_progress + received`) |
| `attention` | tasks with at least one flag |

`open()` is the sum of the four open states. "40 open" is not a plan; "3 ready, 37 blocked" is.

## 5. `tatrs serve`

Read-only HTTP on loopback. `docs/JSON.md` documents the endpoints and payloads.

```console
$ tatrs serve                      # http://127.0.0.1:7787/?t=<token>
$ tatrs serve --json               # print the URL object, then keep serving
$ tatrs serve --poll 500ms         # tighter sweep
$ tatrs serve --no-browse          # status only, no file browser
```

- **Change detection** is a sweep, not a filesystem watcher. Every `--poll` (default `1s`) the server
  `read_dir`s `tasks/`, `stat`s each `TASK.md` and each claim file, and re-parses **only** files
  whose `(len, mtime)` changed. Idle trees cost one `stat` per task per tick and no parsing.
  This also catches writes from other worktrees, which inotify/FSEvents may not report.
- **Push** is `text/event-stream`. Each change bumps `version` and broadcasts one `board` event
  carrying the whole snapshot; a 15-second keepalive comment keeps proxies from closing the stream.
  Clients use `EventSource`, which reconnects automatically after a restart.
- **The page never blocks the stream.** The status rail is a fixed DOM and updates in place; the
  content pane is routed by URL hash (`#/task/<id>`, `#/file/<path>`, `#/files`) and is only
  re-rendered when the route changes. Reading code while counts tick is the point. An open **task
  view** is live: when that task's entry changes (state, owner, flags, or the file's `rev`), it
  re-fetches and keeps the pane's scroll position. A **file view** is deliberately a snapshot —
  including `TASK.md` opened from the task — because a reader expects a document to stay put.
- **Browsing is constrained.** Only paths under the repository root are served; after
  `canonicalize`, a path that escapes the root is refused. `.git`, `target`, `node_modules`,
  `.venv`, `dist`, `build`, and `.tatrs` are never browsable. Files larger than `--max-file`
  (default 2048 KiB) are refused, and non-UTF-8 files return `415`. There are no mutating routes.
  The task view also lists the task folder itself: images become thumbnails served with their real
  content type, text files link into the content pane, and anything else links to the raw bytes.
- **Access is local.** The default bind is `127.0.0.1`; any other address requires `--allow-remote`.
  The session token is printed in the URL and set as an `HttpOnly` cookie by `/`. It keeps other
  local processes out; like a claim token, it is a capability, not a security boundary.

### Scale

| Tasks | Sweep | Re-parse on change | Snapshot | Notes |
| - | - | - | - | - |
| 100 | < 1 ms | 1 file | ~15 KB | trivial |
| 1 000 | ~2–5 ms | only changed files | ~150 KB | default settings are comfortable |
| 2 000 | ~5–10 ms | only changed files | ~300 KB | still fine; rebroadcast only on change |
| 10 000+ | ~30–50 ms | only changed files | ~1.5 MB | move to per-entry deltas before this point |

One `stat` per file per tick is the dominant idle cost; parsing and classification happen only when
something changed. Classification is `O(tasks × dependencies)` per rebuild because readiness resolves
each `DEPENDS` entry against the task list — with realistic dependency counts this stays in the low
milliseconds at 2 000 tasks. A full snapshot per change is wasteful only if many clients watch a very
large tree; that is the case that would justify deltas.

## 6. Non-goals

- **No writes.** The board never closes, claims, or retags. Mutations stay in the CLI/MCP surface
  where they are explicit and testable.
- **No history.** The board shows the present. `git log` is the history, and `COMMIT` records where
  a task ended.
- **No cross-clone view.** Leases are local (D-002), so the board is a view of this clone. A second
  machine sees its own claims.
- **No authentication beyond a local token.** Exposing the board to a network is a deliberate
  `--allow-remote` decision, not a supported deployment.
