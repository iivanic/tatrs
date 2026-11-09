# tatrs — implementation plan

Companion to `docs/CONCLUSION.md`. This is the working plan; update it when a decision changes.

## 1. Repository layout

```
tatrs/
+- Cargo.toml              # workspace
+- rust-toolchain.toml     # pinned stable
+- AGENTS.md               # agent operating manual
+- README.md
+- LICENSE                 # MIT (clean-room; see CONCLUSION.md)
+- docs/
|  +- CONCLUSION.md
|  +- PLAN.md
|  +- FORMAT.md            # normative grammar, incl. upstream quirks
|  +- JSON.md              # versioned output schema
+- crates/
|  +- tatrs-core/          # library: no I/O policy, no CLI
|  |  +- src/huid.rs
|  |  +- src/props.rs
|  |  +- src/task.rs
|  |  +- src/query.rs
|  |  +- src/update.rs
|  |  +- src/store.rs
|  |  +- src/diagnostics.rs
|  |  +- src/json.rs
|  |  +- tests/
|  +- tatrs-cli/           # binary `tatrs`
|  |  +- src/main.rs
|  |  +- src/cli.rs
|  |  +- src/cmd/*.rs
|  +- tatrs-mcp/           # M4: stdio MCP server (optional at first)
+- reference/tatr/         # untracked upstream checkout (read-only)
+- tests/fixtures/         # synthesized, not copied from upstream
```

`reference/` is git-ignored. Fixtures are written by hand from the documented grammar so the repo
stays clean-room; the upstream corpus is used only as an external compatibility test.

## 2. Crate boundaries

`tatrs-core` is pure: it never prints, never reads stdin, never decides exit codes.

| Module | Responsibility |
|-|-|
| `huid` | parse/validate/format `YYYYMMDD-HHMMSS[-suffix]`, UTC from `SystemTime` (civil-date math, no time crate) |
| `props` | ordered property list, duplicate handling, alnum-key rule, unknown-value preservation |
| `task` | `TASK.md` parse + canonical render; title, status, priority, tags, body |
| `query` | TQL lexer/parser/evaluator (precedence: `or` < `and` < comparison) |
| `update` | update-language parse and application to a `Task` |
| `store` | discovery of `tasks/`, load-all, atomic save, ID allocation, advisory lock |
| `diagnostics` | structured warnings/errors with path + line, severity |
| `json` | versioned serialization of tasks, diagnostics, results |

`tatrs-cli` maps flags → core calls, formats output, owns exit codes. `tatrs-mcp` is a thin adapter
over the same core; it must not grow logic of its own.

## 3. Format decisions (compatibility contract)

Normative grammar lives in `docs/FORMAT.md`. Decisions that differ from, or clarify, upstream:

1. **Property keys are `[A-Za-z0-9]+`.** Upstream parses `isalnum` and silently stops at `-`/`_`;
   we reject with a diagnostic. Reserved keys use no separators: `DEPENDS`, `OWNER`, `AGENT`,
   `BRANCH`, `WORKTREE`, `RUN`, `PR`, `COMMIT`, `UPDATED`.
2. **Values are single-line.** Trailing/leading whitespace is trimmed; everything after `:` is
   preserved verbatim otherwise.
3. **Duplicate keys: last wins** (upstream behavior), but `check` reports them and `--fix` can drop
   earlier duplicates.
4. **Property block ends at the first line that is not `- KEY: value`.** Body starts there and is
   preserved byte-for-byte on rewrite; `render` never reflows it.
5. **Canonical render:** title, one blank line, properties in their original order, body verbatim.
   `STATUS`, `PRIORITY`, and `TAGS` are rewritten in place from the parsed fields; missing reserved
   keys are appended with `OPEN`, `999999`, and empty. Order is preserved because reordering creates
   review noise in every task's history.
6. **STATUS is validated** against `OPEN|CLOSED`; other values are a `check` error and `ls` still
   shows them (no silent coercion).
7. **Tags** split on commas and whitespace; empty segments dropped; square brackets allowed but a
   `check` warning (they break TQL).
8. **HUIDs** must be UTC; a task directory name that is not a valid HUID is an error in `check` and a
   warning+skip in `--lenient` mode.
9. **Line endings:** CRLF tolerated on read, always written as LF.
10. **Encoding:** UTF-8; invalid UTF-8 in `TASK.md` is an error (upstream would pass bytes through).

## 4. CLI surface

Global: `--json`, `--porcelain`, `--quiet`, `--no-color`, `--cwd <path>`, `--lenient`,
`--read-only`, `-v/--verbose`.

| Command | Purpose | Key flags |
|-|-|-|
| `init` | create `tasks/`, `tasks/tags`, `.gitignore` hint | `--no-readme` |
| `new` | create task | `-t/--tag`, `-p/--priority`, `-s/--suffix`, `--id <HUID>`, `--body`, `--body-file`, `--acceptance`, `--print-id`, `--json`, `-y`, `--dry-run` |
| `ls` | list/update tasks | `-a`, `--closed`, `--all`, `--sort id\|priority\|updated`, `-u/--update <ops>`, `--fail-if-empty` |
| `show` | full task incl. body | `<HUID>`, `--json`, `--path-only` |
| `find` | locate task regardless of status | `<HUID>`, `--path-only` |
| `id` | current task HUID (inside a task dir) | |
| `ref` | find references to a HUID | `[HUID]`, `--fail-if-empty`, `--json` |
| `summary` | counts by status, tag, and priority | `-c/--closed`, `--json` |
| `check` | validate tree | `--strict`, `--json` (M2: `--fix`) |
| `ready` | runnable tasks (M2) | `--json`, `--limit`, `--explain` |
| `claim` | lease a task, or inspect leases (M2) | `<HUID>`, `--owner`, `--ttl`, `--run`, `--refresh`, `--status`, `--task`, `--fail-if-empty`, `--json` |
| `release` | drop a lease (M2) | `<HUID>`, `--token`, `--json` |
| `todo` | convert `TODO:` comments | `FILES...`, `-t`, `-s`, `-y`, `--dry-run`, `--json` |
| `mcp` | stdio MCP server (M4) | `--allow-write` |

Examples:

```console
$ tatrs new "Fix flaky parser test" -t bug,agent -p 10 --json
{"schema":1,"id":"20261109-133012","path":"tasks/20261109-133012/TASK.md","title":"Fix flaky parser test","status":"OPEN","priority":10,"tags":["bug","agent"]}

$ tatrs ls --json :bug and status eq OPEN
{"schema":1,"tasks":[...]}

$ tatrs ready --json --limit 5
{"schema":1,"ready":[{"id":"...","depends":["..."],"priority":10,"title":"..."}]}
```

Implemented: `init`, `new`, `ls` (including TQL and `--update`), `show`, `find`, `id`, `ref`,
`summary`, `todo`, `check`, `version`. Everything else in the table is specified for M2–M4.

Exit codes: `0` success, `1` runtime/IO error, `2` usage error, `3` invalid data (`check` failures,
malformed `TASK.md`), `4` lock contention/timeout, `5` not found. `ls` with no matches exits `0`
unless `--fail-if-empty`, which returns `5`.

## 5. Concurrency and durability

- **Create:** allocate HUID, `create_dir` (atomic), retry on `AlreadyExists` by appending `-01`,
  `-02`, …; then write `TASK.md` via temp+rename. Never sleep.
- **Update:** read `TASK.md`, apply ops, write temp file in the same directory, `rename` over the
  original. Batch operations (`ls -u`, `claim`) hold an advisory lock on `tasks/.tatrs.lock`
  (`try_lock` with `--timeout`, default 5s; exit `4` on failure).
- **Crashes:** a rename is atomic on the same volume, so a task file is either old or new, never
  truncated. Temp files are named `.tmp-<pid>-<rand>` and cleaned by `check --fix`.
- **Claims:** leases live outside the working tree, under `<git-common-dir>/tatrs/claims/`, so all
  worktrees of one clone see them and nothing is ever committed. `ready` excludes unexpired claims
  and `wip`; expiry is evaluated on read. Humans inspect leases with `tatrs claim --status`
  (`docs/DECISIONS.md` D-002 and D-003), which shows live claims first and expired ones for cleanup.

## 6. Milestones

### M0 — bootstrap (done 2026-11-09)
Workspace, toolchain pin, docs (`CONCLUSION`, `PLAN`, `FORMAT`, `JSON`), `AGENTS.md`, `tatrs-core`
with HUID + task parse/render/validation + store, `tatrs-cli` with `init`, `new`, `ls`, `show`,
`check`, `version`, JSON/porcelain output, exit-code contract.
**Verified:** 34 tests, `clippy -D warnings`, `scripts/smoke.ps1`, and `scripts/compat.ps1`
(94/94 upstream tasks, 0 errors).

### M1 — format parity
Full TQL, update language, `summary`, `find`, `id`, `ref`, `todo` (non-interactive), `--porcelain`,
compat corpus test against `reference/tatr/tasks` (read-only, `.git` excluded), `docs/FORMAT.md`
frozen with tests for every quirk.
**Done when:** 94/94 upstream tasks load with identical status/priority/tags/title, and rewriting
untouched tasks is byte-identical to upstream's renderer (or the diff is documented and intentional).

**Done 2026-11-09.** TQL and the update language with compile-time type checking (`docs/QUERY.md`),
batch lock, dry-run, corpus query baselines (`tests/fixtures/corpus-queries.json`), the remaining
read commands (`find`, `id`, `ref`, `summary`, `todo`), and a corpus round-trip test proving that
all 94 upstream tasks parse and render back byte-identically (LF-normalized).

### M2 — agent surface
`check --fix`, `ready`, `claim`/`release`, `--read-only`, non-TTY safety, structured diagnostics,
shell completions, man page, `tatrs-mcp` read-only tools.
**Ahead of the rest (done 2026-11-09):** `ref` honours `.gitignore` and `.git/info/exclude` via the
`ignore` crate, with `--no-ignore` as an escape hatch.
**Done when:** an end-to-end script lists ready work, claims, edits, closes, and unclaims using only
JSON, with no prompts.

### M3 — distribution
Static builds for `x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`;
GitHub Actions matrix; `cargo install` instructions; `--version` includes commit; Docker image for
agent sandboxes.

### M4 — upstream and ecosystem
PR the low-risk fixes to `tsoding/tatr` (`-y` on `todo`, `--json`, atomic writes), publish
`docs/FORMAT.md` as a standalone spec, provide a `tatr` → `tatrs` migration note, and an
`emacs`/`vim` quickstart snippet.

## 7. Testing strategy

1. **Unit:** HUID round-trip and civil-date math against known timestamps; property parsing edge
   cases (duplicates, `- KEY:value`, non-alnum keys, empty values); title/body boundaries.
2. **Golden fixtures:** `tests/fixtures/{valid,invalid}/` with one case per documented quirk, plus a
   `diagnostics.txt` snapshot.
3. **Property tests:** random valid task → render → parse → identical model; random bytes → parser
   never panics.
4. **Compatibility:** parse the upstream corpus read-only; assert count and field equality; skip if
   `reference/tatr` is absent so CI without the checkout still works.
5. **Concurrency:** N threads/processes calling `new` and `ls -u` in a temp tree; assert no task loss
   and no lost property updates.
6. **CLI contract:** `assert_cmd`-style tests on exit codes, `--json` schema version, and stderr
   diagnostics.

## 8. Risks and mitigations

| Risk | Mitigation |
|-|-|
| Upstream format drifts during rewrite | pinned commit, corpus test, `docs/FORMAT.md` as the contract |
| Clean-room claim challenged | no upstream code in tree, provenance note in `CONCLUSION.md`, review gate in `AGENTS.md` |
| JSON schema churn | `"schema": 1`, additive-only changes, documented in `docs/JSON.md` |
| Windows path/CRLF bugs | CI matrix from M0, no shell-outs, `PathBuf` everywhere, tests with `\r\n` fixtures |
| Overbuilding before parity | milestones gate features on M1 parity, not the reverse |
| Agent misuse of write commands | `--read-only`, `--dry-run`, claim/lease, MCP read-only default |

## 9. Open questions

- Binary name: `tatrs` now; add a `tatr` compatibility symlink at install time?
- Should `check` treat a missing `## Acceptance` as warning or error by default? (proposed: warning;
  error under `--strict`, which CI uses)
- Do we need a `TASKS.md` index artifact for PR review, or is `tatrs summary` enough?
- Cross-machine coordination: out of scope until someone needs it; the local lease is per clone
  (`docs/DECISIONS.md` D-002). If it becomes necessary, the cheapest opt-in is a queue integration —
  publishing claims into `TASK.md` is rejected by D-003.
