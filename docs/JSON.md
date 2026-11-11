# JSON output

`--json` is a public API for agents, scripts, and CI. Rules:

1. stdout contains exactly one JSON document and nothing else. Diagnostics go to stderr.
2. `"schema": 1` is present in every payload. Changes are additive; removing or renaming a field
   requires a new schema version and a migration note.
3. Paths are repository-relative and use `/` on every platform.
4. Field order follows the examples below but MUST NOT be relied on; parse by key.
5. Numbers are integers unless stated otherwise. Ties are never represented as strings.

## Task object

```json
{
  "schema": 1,
  "id": "20261109-133012",
  "path": "tasks/20261109-133012/TASK.md",
  "title": "Fix flaky parser test",
  "status": "OPEN",
  "priority": 10,
  "tags": ["bug", "agent"],
  "closed": false,
  "acceptance": {"present": true, "criteria": 2, "waived": false, "ready": true},
  "props": {
    "STATUS": "OPEN",
    "PRIORITY": "10",
    "TAGS": "bug,agent",
    "OWNER": "agent-3"
  },
  "body": "\nBody line one.\n\n## Acceptance\n\n- [ ] test passes\n"
}
```

| Field | Type | Notes |
| - | - | - |
| `schema` | integer | always `1` today |
| `id` | string | HUID |
| `path` | string | `tasks/<id>/TASK.md`, forward slashes |
| `title` | string | empty string if absent; sentinel if invalid |
| `status` | string | raw value; `OPEN` or `CLOSED` when valid |
| `priority` | integer | `999999` when undeclared |
| `tags` | array of strings | order preserved, duplicates preserved |
| `closed` | boolean | `status == "CLOSED"` |
| `acceptance` | object | `present`, `criteria` (non-empty checkboxes), `checked` (of those, `- [x]`), `waived`, `ready` (open + `agent` tag + criteria) |
| `props` | object | raw property map; duplicate keys collapse to the last value |
| `body` | string | verbatim remainder, usually starts with `\n` |

## `tatrs ls --json`

```json
{"schema": 1, "tasks": [ <task object>, ... ]}
```

Filters and sorting are applied server-side. `[]` is a successful empty result; `--fail-if-empty`
turns it into exit code 5 with no JSON on stdout.

With `--update`, the payload gains three fields; `tasks` reflects the post-update state:

```json
{"schema": 1, "updated": 2, "dry_run": false, "update": "close,-wip", "tasks": [...]}
```

## `tatrs new --json`

A single task object. When `--id` refers to an existing task, the command prints that task and exits
`0` (idempotency key semantics); a note goes to stderr unless `--quiet`.

## `tatrs new --dry-run --json`

```json
{"schema": 1, "dry_run": true, "title": "Fix flaky parser test", "priority": 10, "tags": ["bug"], "id": null}
```

`id` is `null` unless `--id` was supplied.

## `tatrs show --json`

A single task object, including `body`. `--path-only` prints the relative path as plain text; `--raw`
prints the file bytes and is not valid JSON.

## `tatrs find --json`

A single task object, like `show`. `--path-only` prints the relative path as plain text.

## `tatrs id --json`

```json
{"schema": 1, "id": "20261109-133012"}
```

## `tatrs ref --json`

```json
{
  "schema": 1,
  "huid": "20261109-133012",
  "references": [
    {"path": "src/main.c", "line": 12, "text": "// TASK(20261109-133012): later"}
  ]
}
```

References are repository-relative, forward-slash separated, and sorted by path then line. `.git`
and binary files are skipped. An empty array is a success; `--fail-if-empty` makes it exit `5`.

Scanning honours the repository's `.gitignore` files (nested, with negation and `**`) and
`.git/info/exclude`; a user's global git excludes are **not** applied, so results are reproducible
across machines. `--no-ignore` scans ignored files too.

## `tatrs summary --json`

```json
{
  "schema": 1,
  "status": "OPEN",
  "total": 3,
  "untagged": 0,
  "tags": [
    {"tag": "agents", "count": 2, "description": null},
    {"tag": "cli", "count": 1, "description": null}
  ],
  "priorities": [
    {"priority": 100, "count": 3}
  ],
  "lifecycle": {
    "total": 9,
    "received": 3,
    "in_progress": 1,
    "paused": 1,
    "waiting": 2,
    "completed": 1,
    "refused": 1,
    "ready": 2,
    "attention": 1
  }
}
```

`tags` is sorted by count descending, then name. `priorities` is sorted by priority descending.
`description` comes from the optional `tasks/tags` file and is `null` when absent.

`lifecycle` is the derived supervisor view (`docs/DASHBOARD.md`). Unlike the other fields it spans
open **and** closed tasks and ignores `--closed`; `ready` counts open tasks that pass every dispatch
rule, and `attention` counts tasks carrying at least one flag. The derivation is computed from the
claim store and file activity at call time, so the same tree can produce different counts a minute
later.

## `tatrs todo --json`

Preview (no `--yes`):

```json
{"schema": 1, "dry_run": true, "todos": [
  {"file": "src/main.c", "line": 12, "title": "handle argc"}
]}
```

Write (`--yes`):

```json
{"schema": 1, "created": [ <task object>, ... ], "files": ["src/main.c"]}
```

`todo` never prompts: without `--yes` it previews and exits `0`; `--yes` rewrites the files. The
marker is replaced in place, so prefixes, suffixes, and line endings are preserved.

## `tatrs ready --json`

```json
{"schema": 1, "ready": [ <task object>, ... ]}
```

Only tasks that pass every dispatch rule appear: open, `agent` tag, satisfied acceptance, not
`wip`/`blocked`, dependencies closed, no live claim. Sorted by priority descending, then id.
`--explain` adds the excluded tasks:

```json
{"schema": 1, "ready": [], "excluded": [
  {"id": "20261109-133012", "title": "Fix parser", "reasons": ["no `## Acceptance` section"]}
]}
```

## `tatrs claim --json`

Acquire or refresh:

```json
{"schema": 1, "claimed": true, "task": "20261109-133012", "owner": "agent-3",
 "token": "000000006af1d5910000fb7c", "acquired_at": "2026-11-09T14:15:45Z",
 "expires_at": "2026-11-09T16:15:45Z", "run": "run-1"}
```

`token` is the proof of ownership for `refresh` and `release`. A live claim held by someone else
exits `4`; a task that is not dispatchable exits `3`.

## `tatrs claim --status --json`

```json
{"schema": 1, "claims": [
  {"task": "20261109-133012", "owner": "agent-3", "state": "live",
   "acquired_at": "2026-11-09T14:15:45Z", "expires_at": "2026-11-09T16:15:45Z",
   "remaining_seconds": 7200, "host": "desk", "pid": 41232, "run": "run-1"}
]}
```

Live claims come first, soonest expiry first, then expired ones, most recently expired first.
`remaining_seconds` is negative once expired. `--task` filters, `--fail-if-empty` exits `5`.

## `tatrs release --json`

```json
{"schema": 1, "released": true, "task": "20261109-133012"}
```

Compare-and-delete: a wrong token exits `4` and leaves the claim in place; no claim at all exits `5`.

## `tatrs serve`

Long-running and read-only. With `--json` it prints one object and keeps serving:

```json
{"schema": 1, "url": "http://127.0.0.1:7787/?t=8f2c1d0a59b7e314", "root": "/repo",
 "browse": true, "poll_ms": 1000}
```

The URL carries a per-session capability token; opening it sets an `HttpOnly` cookie so the page's
fetches and the event stream authenticate. All routes are `GET` and require the token:

| Route | Body |
| - | - |
| `/` | the dashboard: one self-contained HTML file |
| `/events` | `text/event-stream`; one `board` event per change, plus a comment keepalive |
| `/api/status` | the latest snapshot (below) |
| `/api/task/<HUID>` | a task object plus `lifecycle` and `files` |
| `/api/tree` | `{"schema":1,"truncated":false,"entries":[{"path":"src/main.rs","dir":false}]}` |
| `/api/file?path=REL` | UTF-8 file contents as `text/plain` |

Snapshot:

```json
{
  "schema": 1,
  "version": 7,
  "generated_at": "2026-11-10T22:06:27Z",
  "browse": true,
  "allow_actions": false,
  "counts": {"total": 9, "received": 3, "in_progress": 1, "paused": 1, "waiting": 2,
             "completed": 1, "refused": 1, "ready": 2, "attention": 1},
  "tasks": [
    {"id": "20261109-133012", "title": "Fix parser", "state": "in_progress",
     "also": [], "declared": false, "owner": "agent-3", "run": "run-1",
     "since": "2026-11-10T20:00:00Z", "rev": 1794350748551955000,
     "flags": [], "ready": false,
     "priority": 10, "tags": ["bug", "agent"], "path": "tasks/20261109-133012/TASK.md"}
  ]
}
```

`version` increases on every recomputation, so a reconnecting client can skip a redundant render.
`browse` mirrors `--no-browse`: when false, `/api/tree` and `/api/file` return `404` and the board
hides its `files` link. `allow_actions` mirrors `--allow-actions`: when false, `POST /api/action`
returns `404` and the task view renders no action buttons. `state` is the derived lifecycle
(`docs/DASHBOARD.md`), `declared` distinguishes a tag-driven state from an observed one, and
`since` is ISO-8601 UTC, best effort.
`also` lists the other lifecycle states that also hold (for example a paused task whose dependency
is still open), ordered by board rank; counting always uses `state`, so the six state counts
partition the tree. `rev` is
`TASK.md`'s modification time in nanoseconds: it changes on every edit, including body-only edits,
so a client can tell whether its cached view of a task is stale. Bodies are excluded from snapshots
(they are fetched per task) to keep the broadcast small.

`GET /api/task/<HUID>` adds the task folder next to `TASK.md`:

```json
{"files": [
  {"name": "notes.md", "path": "tasks/20261009-120000/notes.md", "size": 412, "kind": "text"},
  {"name": "screenshot.png", "path": "tasks/20261009-120000/screenshot.png", "size": 48120, "kind": "image"}
]}
```

`kind` is `image` (png/jpeg/gif/webp), `text` (markdown, logs, JSON, CSV, ...), or `file` for
anything else. Entries are sorted by name and exclude `TASK.md` and hidden files.
`/api/file` serves the image formats inline with their real content type; other binaries return
`415`, and SVG is treated as text so an attachment cannot script the board's origin.

### Actions (`--allow-actions`)

`POST /api/action` accepts one task and one named intent:

```json
{"task": "20261109-133012", "intent": "resume"}
```

```json
{"schema": 1, "intent": "resume", "changed": true, "update": "-paused",
 "task": { ... task object ... }}
```

`release-lease` omits `update` and reports `changed: true` only when a lease file was removed.
Unknown intents, malformed JSON, or a missing field return `400` with `{"error": "..."}`; an unknown
task returns `404`; a repository lock held for more than 5 s returns `409`. Repeating an intent that
no longer applies returns `200` with `changed: false`. The full contract is in `docs/DECISIONS.md`
D-006.

## `tatrs check --json`

```json
{
  "schema": 1,
  "ok": false,
  "tasks": 94,
  "errors": 1,
  "warnings": 27,
  "diagnostics": [
    {
      "severity": "error",
      "code": "E0101",
      "path": "tasks/20260101-000000/TASK.md",
      "line": 1,
      "message": "task title must start with `#`"
    }
  ]
}
```

`ok` is `errors == 0 && (!strict || warnings == 0)`, or always `true` under `--lenient`.
`line` is `null` when the diagnostic is not tied to one line.

## `tatrs init --json`

```json
{"schema": 1, "tasks_dir": "tasks", "created": true}
```

## `tatrs version --json`

```json
{"schema": 1, "name": "tatrs", "version": "0.2.0", "commit": null}
```

`commit` is `null` for local builds; release builds set `TATRS_GIT_COMMIT` at compile time.

## `--porcelain`

One line per task, tab-separated, no header, no quoting:

```text
<id>	<status>	<priority>	<tags>	<title>	<path>
```

Tabs, CR, and LF inside the title are replaced with spaces. Consumers that need arbitrary titles
should use `--json`.

## Exit codes

| Code | Meaning |
| - | - |
| 0 | success, including "nothing matched" without `--fail-if-empty` |
| 1 | runtime or I/O error |
| 2 | usage error (bad flags, invalid HUID argument, read-only violation) |
| 3 | validation failed (`check`, or malformed task data the command requires) |
| 5 | not found: missing task HUID, or empty result with `--fail-if-empty` |

Exit code 4 is reserved for lock contention once batch updates land.
