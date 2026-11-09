# The `tatrs` format

Normative description of the `tasks/` database and `TASK.md`. `tatrs` is a clean-room
implementation of the format documented for [`tsoding/tatr`](https://github.com/tsoding/tatr);
compatibility with the upstream tool is a hard requirement (`AGENTS.md` rule 2).

Keywords: MUST, MUST NOT, SHOULD are used in the RFC 2119 sense.

## 1. Layout

```
project/
+- .git/
+- tasks/
|  +- tags                       # optional tag descriptions
|  +- README.md                  # optional
|  +- 20261109-133012/
|  |  +- TASK.md                 # mandatory
|  |  +- screenshot.png          # optional attachments
|  +- 20261109-141500-fix-parser/
|     +- TASK.md
+- src/
```

One repository contains at most one `tasks/` database. Tools discover it by walking up from the
current directory; the nearest `tasks/` directory wins, which means a checkout inside another
checkout behaves like the inner project.

## 2. Task identifiers (HUID)

```
huid    = date "-" time [ "-" suffix ]
date    = 4DIGIT 2DIGIT 2DIGIT        ; YYYYMMDD, UTC
time    = 2DIGIT 2DIGIT 2DIGIT        ; HHMMSS, UTC
suffix  = 1*( ALPHA / DIGIT / "-" )
```

- Timestamps MUST be UTC and MUST be a real calendar instant (`20260230-000000` is invalid).
- The suffix is optional, non-empty, and restricted to ASCII letters, digits, and `-`. It exists so
  parallel workers can guarantee uniqueness: `20260830-000838-rexim`, `20260829-235902-01`.
- IDs are generated at human speed. Two tasks created in the same second by the same worker collide;
  `tatrs` resolves this by appending `-01`, `-02`, … to the *base* timestamp, never by sleeping.
- IDs are immutable. Renaming a task directory breaks every `TASK(<id>)` reference in the codebase.

## 3. `TASK.md`

```
TASK.md = title, blank-line, properties, body
title   = "#" *space title-text newline
property= "-" *space key ":" *space value newline
key     = 1*( ALPHA / DIGIT )
```

### 3.1 Title

The first non-whitespace character MUST be `#`. The title is the rest of that line, trimmed. When
the title is missing, `tatrs` keeps the upstream sentinel
`!!! INVALID: TASK TITLE MUST START WITH # !!!`, emits `E0101`, and (like upstream) does not parse
properties or body for that file.

### 3.2 Property block

- Starts on the first line after the title, skipping blank lines before the first property.
- Each property is `- KEY: value`. `KEY` is ASCII alphanumeric only — **no hyphens, underscores, or
  dots**. `- DEPENDS-ON: x` is not a property; it starts the body.
- `value` is single-line. Leading and trailing whitespace is trimmed; interior whitespace is
  preserved.
- The block ends at the first line that is not `- KEY: value`. That line starts the body.
- A blank line between properties also ends the block: everything after it is body.
- Duplicate keys are allowed and the **last** value wins. `W0101` is reported; saving collapses the
  duplicates to the last value.
- Missing `STATUS`, `PRIORITY`, and `TAGS` are defaulted in memory (`OPEN`, `999999`, empty) but are
  **not** inserted by parsing, so an untouched file keeps its original shape. A mutation that
  rewrites the file materializes the keys it changes. `new` writes all three, using priority `100`
  (upstream's creation default).
- A rewritten file is byte-identical to the original when nothing semantic changed: all 94 tasks in
  the upstream corpus round-trip exactly (`crates/tatrs-core/tests/corpus_roundtrip.rs`).

Because keys cannot contain separators, the agent-workflow properties are single tokens:

| Key | Meaning |
|-|-|
| `OWNER` | durable assignment: who is accountable for the task |
| `AGENT` | agent/runtime name, e.g. `claude-code`, `dsh` |
| `BRANCH` | branch or worktree the work is happening in |
| `WORKTREE` | absolute or repo-relative worktree path |
| `DEPENDS` | comma-separated HUIDs that must close first |
| `RUN` | orchestrator run id |
| `PR` | pull request number or URL |
| `COMMIT` | commit that closed the task |
| `UPDATED` | last meaningful touch, ISO-8601 UTC |

Unknown properties MUST be preserved across every `tatrs` operation.

### 3.3 Body

Everything after the property block, byte-for-byte, including the blank line that normally precedes
the description. Consequences:

- Rewriting a task never reflows prose, lists, code fences, or `NOTE(...)` sections.
- The body is where logs, discovered details, and links belong; it is free-form Markdown.
- `tatrs` never writes into the body except when the caller supplies a body (`new --acceptance`).

### 3.4 Acceptance section

`tatrs check` requires open tasks to contain a level-2 heading whose text starts with `Acceptance`
(case-insensitive), e.g.:

```markdown
## Acceptance

- [ ] `tatrs ls :bug and priority lt 50` prints only bugs with priority below 50
- [ ] no existing fixture changes output
```

This is the only contract an automated worker has for "done". `W0301` is a warning by default and a
failure under `--strict`.

## 4. Rendering

`render(parse(x)) == x` for any file written by `tatrs`, and for upstream files whose header is
already canonical.

- Title, then exactly one blank line.
- Properties in their original order; `STATUS`, `PRIORITY`, and `TAGS` are rewritten in place from
  the parsed fields. Missing reserved keys are appended.
- `- TAGS: a,b` (comma-separated, no spaces). Empty values render as `- KEY:` with no trailing space.
- Body appended verbatim.
- LF line endings, UTF-8, no BOM, no trailing blank line unless the body has one.

CRLF input is accepted and normalized to LF on the next write.

## 5. `tasks/tags`

Optional documentation file, one tag per line, `name [ , ] description`. Parsers MUST tolerate
missing and malformed lines; `tatrs` does not rewrite this file.

## 6. Reserved tags

Tags are free-form, but these are reserved by convention so tools can rely on them:

| Tag | Meaning |
|-|-|
| `agent` | task is intended to be executed by an agent |
| `wip` | claimed, do not start redundantly |
| `blocked` | waiting on a human or an external event |
| `needs-human` | requires a decision, not just time |

`STATUS` stays binary (`OPEN`/`CLOSED`); lifecycle belongs in tags. `OWNER` records durable
accountability, not a lease: active claims are ephemeral runtime state and never enter `TASK.md`
(see `docs/DECISIONS.md` D-002). `ACCEPTANCE: waived` opts a task out of the acceptance rule
(D-001).

## 7. Diagnostics

Codes are stable and part of the JSON API.

| Code | Severity | Meaning |
|-|-|-|
| `E0101` | error | title is missing `#` |
| `E0102` | error | `STATUS` is not `OPEN` or `CLOSED` |
| `W0101` | warning | duplicate property |
| `W0102` | warning | property-like line with a non-alphanumeric key; starts the body |
| `W0103` | warning | `PRIORITY` is not an integer (treated as 0) |
| `E0104` | error | `TASK.md` is not valid UTF-8 |
| `E0106` | error | `TASK.md` cannot be read |
| `E0201` | error | task directory has no `TASK.md` |
| `E0202` | error | directory name is not a valid HUID |
| `W0203` | warning | stray file in `tasks/` |
| `W0204` | warning | leftover temporary file (`.tmp-*`) |
| `E0205` | error | `tasks/` cannot be read |
| `W0301` | warning | open task has no `## Acceptance` section |
| `W0302` | warning | tag contains `[` or `]`, which TQL cannot express |
| `W0303` | warning | duplicate tag |

## 8. Compatibility

- Files written by `tatrs` are readable by upstream `tatr`: reserved agent properties are just
  unknown properties to it, and it preserves them during mass updates.
- Files written by upstream `tatr` are readable by `tatrs`; the 94-task upstream corpus parses with
  zero errors (27 `W0301` warnings at the time of writing).
- Intentional differences: `tatrs` rejects unknown `STATUS` values in `check` (upstream ignores
  them), reports malformed property keys instead of silently treating them as body, and normalizes
  CRLF to LF when it rewrites a file.
