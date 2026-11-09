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
|-|-|-|
| `schema` | integer | always `1` today |
| `id` | string | HUID |
| `path` | string | `tasks/<id>/TASK.md`, forward slashes |
| `title` | string | empty string if absent; sentinel if invalid |
| `status` | string | raw value; `OPEN` or `CLOSED` when valid |
| `priority` | integer | `999999` when undeclared |
| `tags` | array of strings | order preserved, duplicates preserved |
| `closed` | boolean | `status == "CLOSED"` |
| `props` | object | raw property map; duplicate keys collapse to the last value |
| `body` | string | verbatim remainder, usually starts with `\n` |

## `tatrs ls --json`

```json
{"schema": 1, "tasks": [ <task object>, ... ]}
```

Filters and sorting are applied server-side. `[]` is a successful empty result; `--fail-if-empty`
turns it into exit code 5 with no JSON on stdout.

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
{"schema": 1, "name": "tatrs", "version": "0.1.0", "commit": null}
```

`commit` is `null` for local builds; release builds set `TATRS_GIT_COMMIT` at compile time.

## `--porcelain`

One line per task, tab-separated, no header, no quoting:

```
<id>	<status>	<priority>	<tags>	<title>	<path>
```

Tabs, CR, and LF inside the title are replaced with spaces. Consumers that need arbitrary titles
should use `--json`.

## Exit codes

| Code | Meaning |
|-|-|
| 0 | success, including "nothing matched" without `--fail-if-empty` |
| 1 | runtime or I/O error |
| 2 | usage error (bad flags, invalid HUID argument, read-only violation) |
| 3 | validation failed (`check`, or malformed task data the command requires) |
| 5 | not found: missing task HUID, or empty result with `--fail-if-empty` |

Exit code 4 is reserved for lock contention once batch updates land.
