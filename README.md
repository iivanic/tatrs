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

Working today: `init`, `new`, `ls` (TQL queries, batch updates, `--dry-run`), `show`, `find`, `id`,
`ref`, `summary`, `todo`, `ready`, `claim`/`release`, `check`, `version`. Planned: `check --fix`,
completions, man page, and the MCP server. See `docs/PLAN.md`, `docs/QUERY.md`, and
`docs/CLAIMS.md`.

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

$ tatrs find 20261109-133012
tasks/20261109-133012/TASK.md:1: OPEN [PRIORITY:  10] [bug,agent] Fix flaky parser test

$ tatrs ref 20261109-133012
src/parser.c:88:// TASK(20261109-133012): remove the retry once the flake is fixed

$ tatrs summary
STATUS:   OPEN
TOTAL:    3
TAGGED:
       agent =>   2

$ tatrs todo src/parser.c -y -t cleanup

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
