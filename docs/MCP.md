# MCP server

`tatrs mcp` serves the [Model Context Protocol](https://modelcontextprotocol.io) over stdio, so an
agent harness can call the same task database the CLI uses — no subprocess per call, no parsing
compiler-style output.

## Configure

Claude Code / any MCP client that launches a stdio server:

```json
{
  "mcpServers": {
    "tatrs": {
      "command": "tatrs",
      "args": ["mcp", "--allow-write"]
    }
  }
}
```

Drop `--allow-write` for a read-only server. Use `--cwd /path/to/repo` (before or after `mcp`) if the
client does not start the server in the project directory. `--read-only` also forces a read-only
server, even if `--allow-write` is present.

## Tools

| Tool | Access | Arguments |
|-|-|-|
| `list_tasks` | read | `status` (`open`/`closed`/`all`), `tag`, `query` (TQL), `limit` |
| `get_task` | read | `id` |
| `create_task` | write | `title`, `tags`, `priority`, `body`, `suffix`, `id` |
| `claim_task` | write | `id`, `owner`, `ttl`, `run` |
| `release_task` | write | `id`, `token` |

Tool results are MCP `content` blocks whose text is the same schema-1 JSON the CLI prints
(`docs/JSON.md`). Failures are returned as `isError: true` with a human-readable message, so the
model can recover without a protocol error.

`create_task` with an `id` is idempotent: repeating it returns the existing task, which makes retries
safe. `claim_task` enforces the same readiness gate as the CLI (`docs/CLAIMS.md`); it fails for
non-agent tasks, missing acceptance criteria, open dependencies, and live leases.

## Protocol

- Newline-delimited JSON-RPC 2.0 on stdin/stdout.
- Handshake: `initialize` (echoes the client's `protocolVersion`, advertises `tools`), then
  `notifications/initialized`.
- `tools/list`, `tools/call`, `ping` are supported. Unknown methods return `-32601`; malformed JSON
  returns `-32700`.
- stdout carries protocol messages only; diagnostics go to stderr.
- The server exits when stdin closes.

## Example session

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"demo"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_tasks","arguments":{"query":":bug and priority lt 50"}}}
```

```json
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\n  \"schema\": 1,\n  \"tasks\": []\n}"}]}}
```

## Limits

- Leases are local to the clone (`docs/DECISIONS.md` D-002); `claim_task` on another machine will
  happily hand out the same task.
- The server does not expose task updates or closing yet; use the CLI (`tatrs ls -u close <HUID>`)
  or extend the tool set.
