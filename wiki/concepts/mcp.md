---
type: concept
tags: [interface, assistant]
sources: [doc-glossary, doc-readme, doc-roadmap-overview, doc-mcp-tools, doc-tutorial-mcp-design-loop, doc-tutorial-desktop-assistant, adr-thread-mcp-tools, adr-thread-workspace-layer]
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

### From the P3 ADR threads (2026-11-22)

- Project mode is built on ADR 0173's principle: "in project mode, a capability exists only as a workspace command that writes the project folder, and the MCP server and the Studio offer the same commands" ([[adr-thread-workspace-layer]]).
- The tool list grows only additively: each new tool is checked by restating the list tests, and without `--project` the list stays byte-identical to the ADR 0135 golden (`default_f1.rs`) ([[adr-thread-mcp-tools]]).
- With `--project` the 16 session-only tools are "neither listed nor callable" (ADR 0195) ([[adr-thread-workspace-layer]]).

> [!warning] Contradiction (within the repo docs)
> The count "25 project tools (42 in all)" above comes from [[doc-mcp-tools]], whose own project-tool table names 36 tools; ADR 0738 already stated "27 project tools, 44 with `--project`" before the trash tools and `list_sites` were added. See [[adr-thread-mcp-tools]].

## Related

- [[studio]]
- [[template]]
- [[mcp-access-class]]
- [[rcfd-service-protocol]]
- [[project-journal]]
- [[claude-plugin]] (the Claude Code plugin with three skills over the project tools; [[doc-claude-plugin-skills]])
