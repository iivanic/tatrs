# tatrs

[![CI](https://github.com/iivanic/tatrs/actions/workflows/ci.yml/badge.svg)](https://github.com/iivanic/tatrs/actions/workflows/ci.yml)

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
`ref`, `summary`, `todo`, `ready`, `claim`/`release`, `check`, `mcp`, `serve`, `completions`,
`man`, `version`. Distribution is live: `v0.1.0` binaries and a container image. Next: M4, upstream
patches and a standalone format spec. See `docs/PLAN.md`, `docs/QUERY.md`, `docs/CLAIMS.md`,
`docs/DASHBOARD.md`, and `docs/MCP.md`.

## Build

```console
cargo build --release
./target/release/tatrs --help
```

## Install

```console
cargo install --path crates/tatrs-cli

# optional: shell completion and man page
tatrs completions bash > ~/.local/share/bash-completion/completions/tatrs
tatrs man > tatrs.1
```

Prebuilt binaries are published with each `v*` tag as
`tatrs-<version>-<target>.zip` plus a matching `.sha256`
(`x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`):

```console
curl -LO https://github.com/iivanic/tatrs/releases/download/v0.1.0/tatrs-0.1.0-x86_64-unknown-linux-musl.zip
sha256sum -c tatrs-0.1.0-x86_64-unknown-linux-musl.zip.sha256
unzip tatrs-0.1.0-x86_64-unknown-linux-musl.zip -d ~/.local/bin
```

A container image for agent sandboxes can be built from the repo:

```console
docker build -t tatrs:dev .
docker run --rm -v "$PWD:/work" tatrs:dev ls
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
LIFECYCLE:
    received      1
    in progress   1
    paused        0
    waiting       1
    completed     0
    refused       0
    ready         1
    attention     0
TAGGED:
       agent =>   2

$ tatrs todo src/parser.c -y -t cleanup

$ tatrs check --strict

$ tatrs serve                     # live board at http://127.0.0.1:7787/?t=…
```

`tatrs serve` derives the six lifecycle states from tags, leases, and `DEPENDS`, pushes changes over
server-sent events, and lets you read repository Markdown and code in the same page without losing
the status rail (`docs/DASHBOARD.md`).

## Docs

- `docs/CONCLUSION.md` — why this exists, what is preserved, what changes.
- `docs/PLAN.md` — architecture, milestones, CLI and exit-code contracts.
- `docs/QUERY.md` — TQL and the update language, with precedence and type rules.
- `docs/CLAIMS.md` — leases, readiness, and the agent workflow.
- `docs/DASHBOARD.md` — derived lifecycle states and the live board.
- `docs/MCP.md` — the stdio MCP server and its tools.
- `docs/FORMAT.md` — the normative `TASK.md` grammar, including upstream quirks.
- `docs/JSON.md` — versioned output schema.
- `AGENTS.md` — how to work on this repo (human or agent).

## License

MIT. Upstream `tatr` is GPL-2.0; no upstream code is used here. See `docs/CONCLUSION.md`.
