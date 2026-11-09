# Add a stdio MCP server

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: mcp,m2,agent
- OWNER: dsh

## Context

`tatrs` is CLI-first today. An MCP adapter lets agent harnesses call the same core without spawning a process per call. Read-only tools are safe by default; writes are opt-in.

## Acceptance

- [x] `tatrs mcp` speaks JSON-RPC over stdio and exposes `list_tasks`, `get_task`, `create_task`, `claim_task`, and `release_task`
- [x] read-only tools are enabled by default; write tools require `--allow-write`
- [x] tool results reuse the schema-1 task objects
- [x] `--read-only` hides write tools entirely
- [x] tests drive the server over stdin/stdout, with no prompts

## Progress

Done 2026-11-09:

- New `tatrs-mcp` crate: newline-delimited JSON-RPC, `initialize`, `ping`, `tools/list`,
  `tools/call`, notifications ignored, parse errors as `-32700`, unknown methods as `-32601`.
- Tools reuse `tatrs-core` (tasks JSON, readiness, claims); tool failures are `isError` results so
  the model can recover.
- `tatrs mcp [--allow-write]` is a thin CLI wrapper; `--read-only` forces the read-only tool set.
- 7 crate tests plus a CLI test that spawns the binary and drives it over real stdin/stdout.
- `docs/MCP.md` documents configuration, tool schemas, and the local-lease limitation.
