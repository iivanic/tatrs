# Board polish: brand link, ready order, searchable ticket list

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui,dashboard
- OWNER: dsh

## Context

Board polish from user feedback: the brand should return to the board, `ready` belongs next to `received`, and the ticket table needs search plus sorting so it does not require scrolling.

## Acceptance

- [x] the brand (`tatrs@project`) is a link to the board
- [x] `ready` is the second state in the counts and in the "what do these mean?" legend
- [x] the board has one search input filtering id, title, owner, tags, and flags
- [x] every column is sortable; default order is newest first (HUID descending)
- [x] search text and sort survive SSE refreshes without stealing focus
- [x] dashboard script passes a syntax check and `tatrs serve` still serves it

## Progress

2026-11-11:

- Brand is an anchor to `#/` with a hover cue.
- `KEYS` and `LEGEND` put `ready` second, so counts and the legend agree.
- Board shell now has a search box and sortable headers; sort state lives in `state` and rows
  repaint in place on SSE events, preserving focus and query.
- Default sort is `id` descending (newest ticket first); priority is descending, text columns
  ascending when first clicked.
- `node --check` on the inline script, the serve test asserts the new markers, and markdownlint is
  clean.
