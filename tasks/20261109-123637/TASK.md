# Implement TQL and the update language

- STATUS: OPEN
- PRIORITY: 100
- TAGS: tql,m1,agent

## Context

`tatrs ls` currently filters with flags only. The format defines TQL (`:tag`, `and`/`or`/`not`,
`lt/le/gt/ge/eq/ne`, `tagged`, `any`, `[expr]`) and the update language (`+tag`, `-tag`, `close`,
`open`). Upstream semantics are the reference; `reference/tatr/src/query.c` is behavioural material,
never a source of code.

## Acceptance

- [ ] `tatrs ls :bug and priority lt 50` returns the expected subset of a fixture tree
- [ ] precedence is `or` < `and` < comparison, and `not` binds tighter than `and`
- [ ] `tatrs ls -u close,-wip :bug` rewrites only matched tasks, atomically, preserving unknown properties
- [ ] malformed queries exit 2 with a caret diagnostic pointing at the offending token
- [ ] recorded expectations against the upstream corpus still match
