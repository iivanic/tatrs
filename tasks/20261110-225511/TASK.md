# Add allowlisted recovery actions to the live board

- STATUS: OPEN
- PRIORITY: 100
- TAGS: agent,ui

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

- [ ] `POST /api/action` accepts only the allowlisted intents and returns 400 with a JSON error for anything else
- [ ] without `--allow-actions` there is no mutating route
- [ ] every intent is idempotent and leaves a `<UPDATED: ...>` marker
- [ ] D-006 records tokenless lease release and its blast radius
- [ ] buttons render only for flags they can resolve, with a confirm step
