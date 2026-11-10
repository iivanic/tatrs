# Conclusion: rewrite `tatr` as `tatrs`

Status: accepted (2026-11-09). Reference pinned at `bd29290` (2026-11-06) under `reference/tatr/`.

## The verdict, restated

`tatr` is a strong **data model** wrapped in a tool that was optimized for one human in Emacs.
For agentic development the valuable parts are:

1. A task is a directory + `TASK.md`, so it is a ready-made prompt payload with attachments.
2. IDs are UTC timestamps (`YYYYMMDD-HHMMSS[-suffix]`), so parallel branches and worktrees do not
   collide and merges stay trivial.
3. Everything lives in git: diffs are reviewable, `git log`/`git blame` is the audit trail, and no
   service, token, database, or rate limit sits between an agent and its work queue.
4. `TASK(<HUID>):` markers in source give bidirectional task ↔ code traceability.

The parts that block automation are tool-level, not model-level:

| Gap | Consequence for agents |
| - | - |
| Human/compiler-format output only | parsing `TASK.md:1: OPEN [PRIORITY: 3] [bug] Title` is brittle |
| Interactive prompts (`todo`, duplicate-title check) | agent runs hang instead of failing fast |
| In-place `TASK.md` truncation, check-then-mkdir | concurrent agents can clobber tasks |
| Silent skips of invalid dirs/titles | broken tasks disappear without an error |
| Alnum-only property keys (undocumented) | `DEPENDS-ON`, `OWNER_1` silently become body text |
| No lifecycle, claims, or dependencies | no safe scheduling of parallel agents |
| Linux-only, no releases, issues disabled | not deployable as shared infrastructure |

## Why a rewrite instead of patching the C tool

Patching upstream was considered and rejected as the primary path:

- **Scope.** The changes are not additive flags; they touch the parser (strict diagnostics), the
  storage layer (atomic writes, locking), the ID generator (collision strategy), and the process
  model (non-interactive, JSON). That is most of `tatr.c`.
- **Platform.** Upstream is deliberately Linux-first; Windows is an open task. A Rust rewrite gets
  Windows, macOS, and Linux from one codebase with no POSIX shims, and no compiler requirement in
  agent images.
- **Distribution.** `cargo install`/single static binary beats `cc -o nob nob.c` for CI images.
- **Testability.** The format deserves unit and property tests (round-trip, malformed input,
  concurrency). The C tool's tests are `test_*` functions behind a `-t` flag.

A patch set is still worth sending upstream later for the parts that are pure wins
(`-y` on `todo`, `--json`, atomic writes). `tatrs` is not a fork; it is a compatible
reimplementation with a superset feature set.

## What we deliberately preserve

- **Directory layout** `tasks/<HUID>/TASK.md` plus attachments, and the optional `tasks/tags`.
- **HUID grammar** and UTC generation, including the optional suffix.
- **TASK.md grammar**: `# title`, `- KEY: value` block, free-form body. Unknown properties must
  survive every operation.
- **TQL** semantics (`:tag`, `and`/`or`/`not`, `lt/le/gt/ge/eq/ne`, `tagged`, `any`, `[expr]`) and
  the **update language** (`+tag`, `-tag`, `close`, `open`).
- **Exit status and output** for the commands we implement, so `tatrs` can stand in for `tatr` in
  existing scripts (the default human output stays compiler-style).

## What we change

1. **Machine-readable first.** `--json` (stable, versioned schema) and `--porcelain` (TSV/NUL) on
   every command; the human format remains the default.
2. **Never interactive by default.** Prompts only when stdin is a TTY and `--interactive` is passed;
   otherwise the command does the safe thing or fails with a clear error. `--yes`/`--dry-run` exist
   everywhere mutations happen.
3. **Strict by default, lenient on request.** Invalid trees produce diagnostics and a non-zero exit
   from `tatrs check`; `--lenient` reproduces upstream's skip-and-continue behavior for migration.
4. **Correct under concurrency.** `TASK.md` writes go to a temp file then `rename`; batch updates
   take an advisory lock; ID collisions resolve by numeric suffix, not `sleep(1)`.
5. **Agent-native schema, backwards compatible.** Reserved properties `OWNER`, `AGENT`, `BRANCH`,
   `WORKTREE`, `DEPENDS`, `RUN`, `PR`, `COMMIT`, plus reserved tags `wip`, `blocked`, `needs-human`,
   `agent`. Because upstream preserves unknown properties, these work with either tool.
6. **`## Acceptance` required by lint.** A task with no checkable done-condition cannot terminate an
   agent loop; `tatrs check` flags it (warning, promotable to error in CI).
7. **Scheduling primitives.** `tatrs ready` computes the runnable set from `DEPENDS` minus
   `wip`/`blocked`; `tatrs claim`/`release` give a lease with an owner and expiry.
8. **Native `ref`.** Reference search walks the worktree in-process instead of spawning `grep`, so it
   works on Windows and cannot be influenced by shell configuration.

## Licensing and provenance

Upstream is GPL-2.0. `tatrs` is a clean-room implementation of the *format and command behavior*:
the C sources are reference material for compatibility, never copied, ported, or linked. Our code is
MIT (see `LICENSE`). The reference checkout stays untracked and is excluded from release artifacts.
If any code is ever lifted from upstream, the project relicenses to GPL-2.0 — a decision that must be
recorded here first.

## Non-goals

- A web UI, dashboards, or a hosted service.
- Replacing GitHub Issues for external contributors.
- Multi-user permissions, notifications, or time tracking.
- Cross-repo aggregation (one repo = one task database, same as upstream).

## Success criteria

- Existing `tatr` trees load and round-trip with zero diffs for every task in a 94-task corpus
  (`reference/tatr/tasks`).
- `tatrs check` catches every malformed case in our fixture suite with a precise message.
- Two concurrent `tatrs new` runs in one tree never lose a task, and two concurrent `ls -update`
  runs never lose a property.
- An agent can go from "list ready work" to "claim, work, close" using only JSON output.
- Windows, macOS, and Linux pass the same test suite in CI.
