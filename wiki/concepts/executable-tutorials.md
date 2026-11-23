---
type: concept
tags: [docs, testing, tutorial]
sources: [doc-tutorial-quick-start-cli, doc-tutorial-cavity, doc-tutorial-gallery, doc-tutorial-mcp-design-loop, doc-tutorial-desktop-assistant]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Executable tutorials

The tutorials under `docs/tutorials/` are run by `cargo test -p rcfd-cli --test tutorials`, so their commands and quoted output lines are what the current code does (ADR 0089) ([[doc-tutorial-quick-start-cli]]).

## How rcfd uses it

- Fence tags: `file=<name>` (written to a fresh working directory holding a copy of `cases/`), `run` (executed with the real binary, must reach `exit=<n>`, default 0), `output` (lines that must appear), `run-slow` (only in the ignored slow test), `not-executed`. "An untagged block fails the test" ([[doc-tutorial-quick-start-cli]]).
- MCP tutorial: `jsonl run` blocks are sent to `rcfd-mcp --root .`; every response must be free of errors ([[doc-tutorial-mcp-design-loop]]).
- What the test does not check: the cavity tutorial's comparison with Ghia et al. is by eye ([[doc-tutorial-cavity]]); the gallery runs `rcfd check` on every case, full runs only in the slow test ([[doc-tutorial-gallery]]); the desktop tutorial executes only its CLI steps and "Nobody has walked through this page step by step against the running app" ([[doc-tutorial-desktop-assistant]]).

## Related

- [[rcfd-cli]], [[benchmark-descriptor]], [[gate]]
