# Add a stdio MCP server

- STATUS: OPEN
- PRIORITY: 100
- TAGS: mcp,m2,agent

## Context

`tatrs` is CLI-first today. An MCP adapter lets agent harnesses call the same core without spawning a process per call. Read-only tools are safe by default; writes are opt-in.

## Acceptance

- [ ] `tatrs mcp` speaks JSON-RPC over stdio and exposes `list_tasks`, `get_task`, `create_task`, `claim_task`, and `release_task`
- [ ] read-only tools are enabled by default; write tools require `--allow-write`
- [ ] tool results reuse the schema-1 task objects
- [ ] `--read-only` hides write tools entirely
- [ ] tests drive the server over stdin/stdout, with no prompts
