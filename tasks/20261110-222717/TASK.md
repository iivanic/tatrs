# Derive lifecycle states and serve a live board

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui,m5

Humans supervising parallel agent work need one screen: where everything is, and what needs them. `STATUS` stays binary for format compatibility, so the six supervisor states are derived from tags, leases and `DEPENDS`, and `tatrs serve` publishes them as a read-only board.

## Acceptance

- [x] `summary --json` reports lifecycle counts across open and closed tasks
- [x] `paused`/`refused`/`notify` documented as reserved tags in FORMAT §6
- [x] `tatrs serve` streams versioned snapshots over SSE and refuses path traversal
- [x] dashboard states are labelled as derived, with tooltips, a legend and secondary "also" conditions
- [x] `cargo test --workspace`, `clippy -D warnings` and `fmt --check` pass
