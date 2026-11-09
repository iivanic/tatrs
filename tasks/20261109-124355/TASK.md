# Enforce acceptance for agent-tagged tasks

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agents,lint
- OWNER: dsh

## Context

`docs/DECISIONS.md` D-001 decides that acceptance is enforced at dispatch, not at authoring: an
`agent`-tagged open task without at least one non-empty `- [ ]` criterion is an error (`E0301` /
`E0304`), every other open task gets a warning, closed tasks are never linted, and
`ACCEPTANCE: waived` downgrades to info. `ready` and `claim` hard-gate on the same rule.

## Acceptance

- [x] `Task::lint` implements the severity matrix in D-001: closed exempt, `agent` tag escalates to `E0301`/`E0304`, otherwise `W0301`/`W0304`
- [x] a heading with no checkbox item, or with an empty `- [ ]`, does not count as acceptance
- [x] `ACCEPTANCE: waived` emits `I0301` and passes
- [x] `new -t agent` without criteria prints a stderr warning but still creates the task
- [x] task JSON gains `acceptance: {present, criteria, waived, ready}` without changing `schema`
- [x] `check --strict` fails on `E0301` and `W0301`; `--lenient` still exits 0
- [x] the upstream corpus still reports 0 errors

## Progress

Done 2026-11-09: `Task::acceptance()` counts non-empty checkboxes inside the section, honours
`ACCEPTANCE: waived`, and stops at the next heading. Lint uses the D-001 matrix, `new` warns for
agent tasks, task JSON exposes the acceptance view, and `ready`/`claim` gate on it.
