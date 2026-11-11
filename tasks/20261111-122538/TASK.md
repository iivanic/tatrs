# Flag placeholder tasks in attention

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui,dashboard
- OWNER: dsh

## Context

A task created by `new` with no title, tags, or body is only a placeholder. It shows as `received` with no flags, so the board's attention rail stays silent even though nobody can act on it. `check` only warns about acceptance.

## Acceptance

- [x] an open task still called `New Task` with no tags and an empty body gets a `placeholder` flag
- [x] the board shows it in attention and the legend explains it
- [x] renaming it, adding a tag, or writing a body clears the flag
- [x] lifecycle tests cover the flag and its clearing conditions

## Progress

2026-11-11:

- `Flag::Placeholder` fires for an open task whose title is still `New Task`, with no tags and an
  empty body. It is computed, never stored, and clears as soon as the task is renamed, tagged, or
  given a body.
- The legend now lists flags under a separate heading and explains `placeholder`; the attention rail
  picks it up through the existing flag path.
- Verified on `20261111-120850`: state `received`, flags `placeholder`, `attention` count 1.
- `check` is unchanged (D-004: flags are data, not diagnostics).
