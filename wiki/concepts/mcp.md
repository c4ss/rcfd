---
type: concept
tags: [interface, assistant]
sources: [doc-glossary, doc-readme, doc-roadmap-overview]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# MCP (Model Context Protocol)

The interface through which an AI assistant calls rcfd's tools ([[doc-glossary]]).

## How rcfd uses it

- Served by [[rcfd-mcp]]; project tools sit over the workspace layer [[rcfd-workspace]] ([[doc-architecture]]).
- Claude Code, in its own terminal with the project server registered, is the only assistant ([[doc-readme]]).
- Tool list in `docs/mcp-tools.md` (P1).

## Related

- [[studio]]
- [[template]]
