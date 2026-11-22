---
type: concept
tags: [case]
sources: [doc-glossary]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case file

A TOML or JSON file that describes one simulation: mesh, physics, boundaries, solver and reports ([[doc-glossary]]).

## How rcfd uses it

- Every setting is documented in `docs/case-schema.md` (to ingest in P1).
- `rcfd check <case>` builds the mesh and validates the set-up without solving ([[doc-readme]]).

## Related

- [[benchmark-descriptor]]
- [[template]]
- [[report]]
- [[run]]
