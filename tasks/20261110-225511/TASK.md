# Add allowlisted recovery actions to the live board

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui
- OWNER: dsh

The board is read-only, so an attention flag tells a human what is wrong and then sends them to a terminal. Supervision should not require a context switch for recovery.

Expose a deliberately small set of intents, each mapping to one existing core operation, behind `tatrs serve --allow-actions` (off by default):

| flag or state | button | operation |
| - | - | - |
| `expired-claim`, `stalled`, `claim-on-closed-task` | Release lease | `release` with a server-side admin override |
| `paused-but-claimed` | Release lease / Resume | `release` / `-paused` |
| `paused` | Resume | `-paused` |
| `waiting` via tag | Unblock | `-blocked` |
| `unverified` | Reopen | `open` |
| `notify-owner` | Mark notified | `-notify` |
| `needs-human` | Clear needs-human | `-needs-human` |

Constraints:

- No free-form edits: no body editor, no arbitrary tag input, no priority field. Only named intents, each idempotent and reversible.
- `POST /api/action` writes through `tatrs-core::update` under `tasks/.tatrs.lock`, atomically, and leaves the `<UPDATED: ...>` marker so the change shows up in `git diff`.
- `--allow-actions` is required; without it the server keeps no mutating route.
- Loopback plus the session token remains the boundary. Tokenless lease release is new authority and needs its own decision record (D-006).

## Acceptance

- [x] `POST /api/action` accepts only the allowlisted intents and returns 400 with a JSON error for anything else
- [x] without `--allow-actions` there is no mutating route
- [x] every intent is idempotent and leaves a `<UPDATED: ...>` marker
- [x] D-006 records tokenless lease release and its blast radius
- [x] buttons render only for flags they can resolve, with a confirm step

## Progress

2026-11-11:

- `POST /api/action` behind `--allow-actions`; six intents mapped to `Update` or
  `ClaimStore::force_release`, serialized by `tasks/.tatrs.lock` and saved atomically.
- Each applied intent writes `UPDATED: <ISO-8601>` (the reserved property), so the change is visible
  in `git diff`; repeat calls return `changed: false`.
- Task view renders only matching buttons (`release-lease`, `resume`, `unblock`, `reopen`,
  `mark-notified`, `clear-needs-human`) and asks `confirm()` before posting.
- `allow_actions` is part of the snapshot so the UI can stay read-only when the flag is absent.
- D-006 records the tokenless release authority and blast radius; JSON/DASHBOARD/CLAIMS updated.
- Tests: unknown intent 400, no-route 404, release idempotence, resume writes `UPDATED`, buttons and
  confirm appear in the page.
