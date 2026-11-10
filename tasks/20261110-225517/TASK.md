# Board connection dot must differ by shape, not only colour

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui

WCAG 1.4.1: colour must not be the only way to carry meaning. The live indicator uses the same circle for connected and disconnected, so a daltonist user sees two identical shapes.

Use three shapes for three states: connecting = hollow ring, connected = filled disc, disconnected = square outline. Keep colour as reinforcement, and expose the state as text for assistive tech.

## Acceptance

- [x] connected, connecting and disconnected differ by shape, not only colour
- [x] the indicator has a title and an accessible label naming the state
- [x] the existing green/red colours are unchanged
