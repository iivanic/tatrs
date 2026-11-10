# Show task-folder attachments on the live board

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,ui

A task folder can hold screenshots and documents next to TASK.md, but the task view only renders TASK.md. The view should list the folder: images as thumbnails, text files as links that open in the content pane, anything else as a raw link.

## Acceptance

- [x] `GET /api/task/<HUID>` lists the folder's files by name, path, size and kind, excluding TASK.md
- [x] images are served with an image content type while other binaries still return 415
- [x] the task view shows thumbnails and links; clicking one opens it in the content pane
- [x] a task holding a real screenshot and a Markdown note renders both
