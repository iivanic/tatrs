# Implement TQL and the update language

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: tql,m1,agent
- OWNER: dsh

## Context

`tatrs ls` currently filters with flags only. The format defines TQL (`:tag`, `and`/`or`/`not`,
`lt/le/gt/ge/eq/ne`, `tagged`, `any`, `[expr]`) and the update language (`+tag`, `-tag`, `close`,
`open`). Upstream semantics are the reference; `reference/tatr/src/query.c` is behavioural material,
never a source of code.

## Acceptance

- [x] `tatrs ls :bug and priority lt 50` returns the expected subset of a fixture tree
- [x] precedence is `or` < `and` < comparison, and `not` binds tighter than `and`
- [x] `tatrs ls -u close,-wip :bug` rewrites only matched tasks, atomically, preserving unknown properties
- [x] malformed queries exit 2 with a caret diagnostic pointing at the offending token
- [x] recorded expectations against the upstream corpus still match

## Progress

Done 2026-11-09:

- `tatrs-core::query` compiles TQL to a postfix program with compile-time type checking
  (`not` binds to a primary; comparisons are integer-only, as upstream).
- `tatrs-core::update` implements `open`, `close`, `+tag`, `-tag`, comma- or whitespace-separated.
- `ls` takes a query, holds `tasks/.tatrs.lock` for batch updates, writes only changed tasks, and
  supports `--dry-run`; JSON gains `updated`, `dry_run`, and `update`.
- `docs/QUERY.md` documents grammar, precedence, types, and the status-filter interaction.
- Corpus baseline in `tests/fixtures/corpus-queries.json`, enforced by `scripts/compat.ps1`.
- 58 tests (40 core, 18 CLI), clippy clean.

Remaining from the original M1 scope: `find`, `id`, `ref`, `summary`, `todo`, and byte-identical
render comparison with upstream.
