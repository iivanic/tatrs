# tatrs

A clean-room Rust reimplementation of [`tsoding/tatr`](https://github.com/tsoding/tatr) —
a git-native task tracker where every task is a directory with a `TASK.md` file.

`tatrs` keeps the format and adds what agents and scripts need: stable JSON output,
no interactive prompts, atomic writes, validation, and scheduling primitives.

```
tasks/
+- tags
+- 20261109-133012/
|  +- TASK.md
|  +- attachment.png
+- 20261109-141500-fix-parser/
   +- TASK.md
```

## Status

Working today: `init`, `new`, `ls` (TQL queries, batch updates, `--dry-run`), `show`, `check`,
`version`. Planned: `find`, `id`, `ref`, `summary`, `todo`, claims and `ready`, and the MCP server.
See `docs/PLAN.md` and `docs/QUERY.md`.

## Build

```console
cargo build --release
./target/release/tatrs --help
```

## Use

```console
$ tatrs init
$ tatrs new "Fix flaky parser test" -t bug,agent -p 10
tasks/20261109-133012/TASK.md:1: OPEN [PRIORITY:  10] [bug,agent] Fix flaky parser test

$ tatrs ls --json
{"schema":1,"tasks":[...]}

$ tatrs check --strict
```

## Docs

- `docs/CONCLUSION.md` — why this exists, what is preserved, what changes.
- `docs/PLAN.md` — architecture, milestones, CLI and exit-code contracts.
- `docs/QUERY.md` — TQL and the update language, with precedence and type rules.
- `docs/FORMAT.md` — the normative `TASK.md` grammar, including upstream quirks.
- `docs/JSON.md` — versioned output schema.
- `AGENTS.md` — how to work on this repo (human or agent).

## License

MIT. Upstream `tatr` is GPL-2.0; no upstream code is used here. See `docs/CONCLUSION.md`.
