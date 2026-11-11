# Rail: derived label, honest connection text, no overlap with the legend

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui

Three small board defects:

1. The rail note reads "derived · not stored"; the requested wording is "derived states" (the tooltip already explains that nothing is written).
2. The status text "updated 17m ago" is read as connection age, but it is the snapshot's `generated_at`, i.e. the last time the task tree changed. With a healthy connection and no work happening it grows old and looks broken. Separate the two: connection state belongs to the dot and the word "connected"; the timestamp should say what it measures.
3. With the "what do these mean?" legend expanded, the bottom `board`/`files` links overlap the text. The rail must scroll without letting the nav collide.

## Acceptance

- [x] the rail note reads "derived states"
- [x] the status line names the connection and, separately, when the board last changed
- [x] a reconnecting state never shows a stale "changed N ago"
- [x] with the legend expanded the nav stays in flow and nothing overlaps at 900px height
