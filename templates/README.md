# Agent adoption kit

Two layers, because they fail differently:

| File | Layer | Why |
| - | - | - |
| `AGENTS.snippet.md` | always-on instructions | rules the agent must never miss |
| `skills/tatrs/SKILL.md` | on-demand skill | workflow loaded only when its description matches |

Instructions are read into every request, so keep the snippet short. A skill is model-invoked: it may
never load, which is why the hard rules live in the snippet too.

## Install the instructions

Append `AGENTS.snippet.md` to the consumer repository's `AGENTS.md`. Harnesses with their own file
name can point at it: `CLAUDE.md` (`@AGENTS.md` import), `.github/copilot-instructions.md`, or a
Cursor rules file.

## Install the skill

The folder name must equal the frontmatter `name` (`tatrs`). Copy or symlink; a symlink keeps the
repository and the installed skill in sync:

```console
# shared project location
mkdir -p .agents/skills
ln -s /path/to/tatrs/templates/skills/tatrs .agents/skills/tatrs

# Claude Code
mkdir -p .claude/skills
ln -s /path/to/tatrs/templates/skills/tatrs .claude/skills/tatrs
```

Discovery locations, checked on the machine this kit was written on:

| Harness | Project-level | User-level | Status |
| - | - | - | - |
| VS Code Copilot | `.agents/skills/<name>/`, `.github/skills/<name>/`, `.claude/skills/<name>/` | `~/.agents/skills/<name>/` | verified: both directories exist and are what this session loaded |
| Claude Code | `.claude/skills/<name>/` | `~/.claude/skills/<name>/` | verified: directory exists and carries the same skills |
| Codex CLI | `AGENTS.md` | `~/.codex/skills/` if the build supports skills | **unverified**: `~/.codex` exists but is empty, so treat the path as version-dependent |
| Cursor, opencode | `AGENTS.md` / rules file | — | not installed here; use the instructions snippet |

If a harness does not have a skills directory, the instructions snippet is still enough to keep an
agent on the CLI and away from hand-editing `TASK.md`.

## MCP

`tatrs mcp` is the harness-agnostic integration: any client that speaks MCP gets the same database
through tools, with no shelling out. Start read-only.

Read-only:

```json
{"mcpServers": {"tatrs": {"command": "tatrs", "args": ["mcp"]}}}
```

Write access (`create_task`, `claim_task`, `release_task`, `update_task`):

```json
{"mcpServers": {"tatrs": {"command": "tatrs", "args": ["mcp", "--allow-write"]}}}
```

VS Code (`.vscode/mcp.json`) names the map `servers` and the transport explicitly:

```json
{"servers": {"tatrs": {"type": "stdio", "command": "tatrs", "args": ["mcp"]}}}
```

`tatrs mcp --cwd /path/to/repo` pins the project when the client does not start the server in it.
Full tool list: `docs/MCP.md`.
