---
type: concept
tags: [case]
sources: [doc-glossary, doc-mcp-tools, doc-run-control, doc-tutorial-gallery]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Template

A ready-made case the engine can write into a project (`new_case {name, template}`); the engine checks the case first, never replaces a file, and logs a line in the project journal ([[doc-glossary]]).

## How rcfd uses it

- Example: `lid_driven_cavity`, also the quick-start case ([[doc-glossary]]).
- `new_case` takes exactly one of `example` (a shipped case of `cases/`, written byte for byte with its data files), `template`, `toml`, `settings` or `copy`; it validates first (1003 with located diagnostics) and never replaces a file (ADR 0186 G3) ([[doc-mcp-tools]]).
- The session tool `list_cases` lists benchmark cases and built-in templates; `create_case` makes a session from a template ([[doc-mcp-tools]], [[doc-tutorial-gallery]]).
- Workflow templates are a different thing: `mesh_to_results`, `channel_flow`, `empty` ([[doc-run-control]]).

## Related

- [[case-file]]
- [[mcp]]
