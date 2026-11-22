---
type: concept
tags: [interface, assistant]
sources: [doc-glossary, doc-readme, doc-roadmap-overview, doc-mcp-tools, doc-tutorial-mcp-design-loop, doc-tutorial-desktop-assistant]
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
- Ingested in P1: [[doc-mcp-tools]]. The server exposes the solver service ([[rcfd-service-protocol]]) as tools; every tool has an [[mcp-access-class]] and names its service method ([[doc-mcp-tools]]).
- Two modes: session tools (34) working on in-memory service sessions, and `--project <dir>` with 25 project tools over the workspace layer (42 in all) ([[doc-mcp-tools]]).
- Transport is stdio JSON-RPC 2.0; a tested walk-through of the exact messages is [[doc-tutorial-mcp-design-loop]].
- In the Studio flow, Use with Claude Code registers `rcfd-mcp --project <folder>` with Claude Code at local scope ([[doc-tutorial-desktop-assistant]]).

## Related

- [[studio]]
- [[template]]
- [[mcp-access-class]]
- [[rcfd-service-protocol]]
- [[project-journal]]
- [[claude-plugin]] (the Claude Code plugin with three skills over the project tools; [[doc-claude-plugin-skills]])
