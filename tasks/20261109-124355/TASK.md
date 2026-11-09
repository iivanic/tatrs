# Enforce acceptance for agent-tagged tasks

- STATUS: OPEN
- PRIORITY: 100
- TAGS: agents,lint

## Context

`docs/DECISIONS.md` D-001 decides that acceptance is enforced at dispatch, not at authoring: an
`agent`-tagged open task without at least one non-empty `- [ ]` criterion is an error (`E0301` /
`E0304`), every other open task gets a warning, closed tasks are never linted, and
`ACCEPTANCE: waived` downgrades to info. `ready` and `claim` hard-gate on the same rule.

Today `Task::lint` warns `W0301` for open tasks without a `## Acceptance` heading and does not look
at the criteria themselves.

## Acceptance

- [ ] `Task::lint` implements the severity matrix in D-001: closed exempt, `agent` tag escalates to `E0301`/`E0304`, otherwise `W0301`/`W0304`
- [ ] a heading with no checkbox item, or with an empty `- [ ]`, does not count as acceptance
- [ ] `ACCEPTANCE: waived` emits `I0301` and passes
- [ ] `new -t agent` without criteria prints a stderr warning but still creates the task
- [ ] task JSON gains `acceptance: {present, criteria, waived, ready}` without changing `schema`
- [ ] `check --strict` fails on `E0301` and `W0301`; `--lenient` still exits 0
- [ ] the upstream corpus still reports 0 errors
