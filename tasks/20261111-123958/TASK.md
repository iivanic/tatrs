# Board: last-activity column and default sort

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui,dashboard
- OWNER: dsh

## Context

The board's default sort is the HUID (creation time), so an old ticket that was edited or claimed today stays buried. The snapshot already carries `since`, derived from TASK.md's filesystem mtime, but the table neither shows nor sorts by it.

## Acceptance

- [x] the ticket table has a last-activity column showing relative time, with the ISO timestamp in the tooltip
- [x] that column is sortable and is the default sort (most recent activity first)
- [x] the task detail shows relative activity too
- [x] the serve test asserts the new column and default

## Progress

2026-11-11:

- Board table gained an `activity` column (`2h ago`, ISO in the tooltip) sourced from the existing
  `since` field (`TASK.md` mtime).
- Default sort is activity descending; clicking the header still toggles, and the other columns work
  as before.
- Task detail's `since` row is relative with the ISO timestamp in the title.
- Docs updated; serve test asserts the default hint and the activity column.
- Verified against the rebuilt server (port 7788): entries carry `since`, page exposes the column.
