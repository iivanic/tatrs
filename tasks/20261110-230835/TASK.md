# Make Markdown docs lint-clean under markdownlint defaults

- STATUS: CLOSED
- PRIORITY: 100
- TAGS: agent,docs

VS Code's markdownlint reports ~200 issues. The dominant one is MD060: tables mix styles - header and body rows are `compact` (`| a | b |`) while delimiter rows are `tight` (`|-|-|`). Also present: MD040 (bare fenced blocks), MD012 (double blank line), MD010 (tabs inside an intentional tab-separated output sample), plus MD013 if a contributor uses stock settings.

Plan: ship one `.markdownlint.json` that records the deliberate exceptions (no line-length limit, tabs allowed in code blocks) and `.markdownlintignore` for `reference/` and `target/`; normalize every table delimiter row to one style; give every fence a language. Prefer fixing the files over disabling rules, so default settings are clean too.

## Acceptance

- [x] `.markdownlint.json` declares only deliberate exceptions, with a comment in `docs/` explaining why
- [x] every table delimiter row uses the same style
- [x] every fenced block declares a language
- [x] `npx markdownlint-cli2 "**/*.md"` reports zero issues
- [x] VS Code diagnostics for Markdown are clear
