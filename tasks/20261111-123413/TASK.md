# Clarify legend text: flags are not 'also'

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui,dashboard
- OWNER: dsh

## Context

The legend says "A task has exactly one state. Other conditions that also hold are shown as “also”." That is wrong: `also` carries secondary *states*, while flags such as `placeholder`, `stalled`, or `needs-human` are orthogonal and shown separately. `20261111-120850` has one state (`received`) and one flag (`placeholder`), which reads like two statuses.

## Acceptance

- [x] the legend text distinguishes secondary states (`also`) from flags
- [x] the serve test asserts the new wording

## Progress

2026-11-11:

- Legend now reads: "A task has exactly one state. Other states that also hold are listed under “also”;
  flags are independent and shown separately."
- The serve test asserts the new wording.
- Note: the HTML is embedded in the binary, so the running `serve` needs a restart to show it.
