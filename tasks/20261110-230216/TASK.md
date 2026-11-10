# Publish agent adoption artifacts: AGENTS snippet, tatrs skill, MCP quickstart

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,docs

Adoption needs two layers that fail differently: rules an agent cannot miss (always-on instructions) and a workflow it loads on demand (a skill). The portable integration is MCP.

Deliverables:

- `templates/AGENTS.snippet.md` - paste-in block for a consumer repo's `AGENTS.md`: use tatrs, never hand-edit STATUS/TAGS, claim before starting, close only with acceptance ticked, prefer `--json`.
- `templates/skills/tatrs/SKILL.md` - canonical skill (folder name must equal `name: tatrs`), trigger-rich description, the ready -> claim -> update -> release loop, exit codes, pointers to `docs/`.
- `templates/README.md` - where each harness discovers a skill (`.agents/skills/`, `.claude/skills/`, `.github/skills/`), copy/symlink install, and MCP configs.
- `docs/MCP.md` - quickstart with read-only and write configs plus the VS Code `servers` shape.

Verified on this machine: `~/.agents/skills` and `~/.claude/skills` both exist and are read by the harnesses in use. `~/.codex` exists but is empty, so Codex skill discovery is unverified and must be labelled as such. Cursor and opencode are not installed.

## Acceptance

- [x] the snippet states only always-on rules and does not duplicate `docs/`
- [x] `SKILL.md` frontmatter is valid YAML, `name` matches the folder, and the description contains trigger phrases
- [x] `templates/README.md` separates verified discovery paths from unverified ones
- [x] `docs/MCP.md` has copy-paste read-only and write configs
- [x] the main README links the templates
