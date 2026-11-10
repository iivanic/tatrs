# Render the board file browser as a collapsible tree

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui

`#/files` is a flat list of full paths. A supervisor scanning a repository needs the shape of it: folders that open and close, files under them.

Build the tree client-side from `/api/tree` (no API change): `<details>`/`<summary>` for folders, sorted dirs-first, files as links into the content pane. Filtering should keep ancestor folders and open the matches; expansion state should survive a filter round-trip.

## Acceptance

- [x] folders are expandable and collapsed by default
- [x] a filter shows matching files with their ancestor folders expanded
- [x] expansion state survives clearing the filter
- [x] files still open in the content pane via `#/file/<path>`
