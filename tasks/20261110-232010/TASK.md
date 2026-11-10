# Board polish: back navigation and project name in the brand

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui

Two gaps on the board:

1. Drill-down views have no way back in-app. Open a file from the tree and the only route back to `#/files` is the rail link or browser history. The file view needs a back control to the tree, and the task view one to the board; returning to the tree must keep the expanded folders.
2. The top-left brand says only `tatrs`. With several clones open it is ambiguous. Show the project: `tatrs@` in small muted text followed by the repository root folder name in normal weight, supplied by the server rather than hardcoded.

## Acceptance

- [x] the file view has a back control that returns to `#/files` with expansion preserved
- [x] the task view has a back control that returns to the board
- [x] the brand renders `tatrs@` small and the project folder name at normal weight
- [x] the project name comes from the served repository root, not a constant
