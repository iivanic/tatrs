# AGENTS.md — working on `tatrs`

`tatrs` is a clean-room Rust reimplementation of [tsoding/tatr](https://github.com/tsoding/tatr):
a git-native task tracker where each task is `tasks/<HUID>/TASK.md`. The goal is a drop-in-compatible
format with an automation-first CLI: stable JSON, no interactive prompts, atomic writes, and
first-class agent workflow (claims, dependencies, validation).

Read before changing anything: `docs/CONCLUSION.md`, `docs/PLAN.md`, `docs/FORMAT.md`.

## Ground rules

1. **Clean-room.** Never copy, port, or transliterate code from `reference/tatr` (GPL-2.0). That
   checkout is behavioral reference only: run it, read its output, do not paste it. If upstream code
   ever enters this repo, the project becomes GPL-2.0 and that decision must be recorded in
   `docs/CONCLUSION.md` first.
2. **Format compatibility is the contract.** A tree written by `tatr` must load in `tatrs` and
   round-trip. Unknown properties and the task body are preserved byte-for-byte. When parity and
   cleanliness conflict, parity wins unless `docs/FORMAT.md` says otherwise.
3. **JSON is an API.** `"schema": 1` is versioned; changes are additive only. Renaming or removing a
   field is a breaking change and needs a new schema version plus a migration note.
4. **Never block on a human.** No command prompts unless stdin is a TTY *and* `--interactive` was
   passed. Non-TTY runs either do the unambiguous thing or fail with exit `2`.
5. **Writes are atomic.** Write to a temp file in the same directory, then `rename`. Nothing may
   truncate `TASK.md` in place. Batch mutations take the advisory lock (`tasks/.tatrs.lock`).
6. **No shell-outs.** Paths, `ref`, and `todo` are implemented in-process. No `grep`, no `git` calls
   from library code. Git is the user's tool, not ours.
7. **Cross-platform from day one.** No `#[cfg(unix)]` shortcut that leaves Windows untested. Use
   `PathBuf`, handle CRLF input, write LF.
8. **Errors are data.** Every failure carries a path, an optional line, and a stable machine-readable
   code so agents can branch on it. No `unwrap`/`expect` on user input in `tatrs-core`.
9. **Ephemeral state stays out of git.** Claims, locks, and run state live under the git common dir,
   never in `TASK.md`. Decisions D-001 and D-002 in `docs/DECISIONS.md` are binding: acceptance is
   enforced at dispatch, and leases are local to a clone.

## Layout

- `crates/tatrs-core` — pure library: HUID, properties, TASK.md parse/render, TQL, updates, store,
  diagnostics, JSON. No printing, no stdin, no exit codes.
- `crates/tatrs-cli` — binary `tatrs`: flag parsing, output formatting, exit codes. Thin.
- `crates/tatrs-mcp` — stdio MCP adapter over core (read-only by default).
- `reference/tatr/` — untracked upstream checkout. Read-only.
- `tests/fixtures/` — hand-written fixtures. Do not copy upstream task files into the repo.

## Commands

```console
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
pwsh -File scripts/smoke.ps1          # end-to-end on Windows
pwsh -File scripts/compat.ps1         # read reference/tatr/tasks if present
pwsh -File scripts/agent-flow.ps1     # ready -> claim -> close -> release, JSON only
```

Run all of them before declaring work done. `clippy -D warnings` is not advisory.

## Changing the format or a command

Checklist for a format change:

1. Write the rule in `docs/FORMAT.md` (grammar + one example + one counter-example).
2. Add a fixture under `tests/fixtures/valid|invalid/` and a diagnostics snapshot.
3. Implement in `tatrs-core`; keep `render(parse(x)) == x` for untouched input.
4. Add the JSON field (additive only) and document it in `docs/JSON.md`.
5. Update `docs/PLAN.md` if a milestone or decision changed.

Checklist for a command:

1. `cmd/<name>.rs` calls core; no business logic in the CLI layer.
2. Support `--json`, `--dry-run` if it mutates, and `--quiet`.
3. Define the exit code (`docs/PLAN.md` §4) and test it.
4. Add a row to the command table in `docs/PLAN.md` and a help snapshot test.

## Testing

- Unit tests live next to the module; fixtures are files, not string literals, when they exceed ~10 lines.
- Property tests: random task → render → parse → same model; arbitrary bytes → parser never panics.
- The upstream corpus test is read-only and skipped when `reference/tatr` is absent.
- Concurrency tests spawn processes, not just threads, and run in `tempfile`-style scratch trees.

## Style

- `rustfmt` defaults; no manual formatting debates.
- Errors: one `Error` enum per crate with `#[derive(thiserror::Error)]`; diagnostics carry
  `path`, `line`, `code`, `severity`.
- Public items get doc comments that state the invariant, not the mechanics.
- Prefer small, total functions over flag-threading; `tatrs-core` must be usable without a CLI.

## Dogfooding

`tatrs` tracks itself: `tasks/` in this repo is the real backlog. Create work with
`tatrs new`, reference the HUID in the commit subject (`20261109-133012: add atomic writes`), and
close it when the acceptance section passes. This doubles as an integration test.

## Definition of done

- Behaviour is covered by a test that fails before the change.
- `cargo test`, `clippy -D warnings`, `fmt --check`, `scripts/smoke.ps1` pass.
- Docs updated (`FORMAT.md` / `JSON.md` / `PLAN.md`) when behaviour is user-visible.
- No GPL code copied; no new dependency without a note in the commit message explaining why std
  cannot do it.
- If you touched the format, the upstream corpus still round-trips.

## Tatrs usage

- ALWAYS use tatrs for work in this repository
