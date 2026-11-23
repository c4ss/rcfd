---
type: concept
tags: [process, roadmap]
sources: [doc-glossary, doc-roadmap-overview, doc-process-agent-rules, doc-roadmap-json]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# F-item (roadmap item)

A roadmap item id such as F-028. Each item has dependencies, acceptance criteria and a status ([[doc-glossary]]).

## How rcfd uses it

- Stored in `docs/roadmap.json` (fields include id, title, phase, deps, status, acceptance, gap, note); `docs/ROADMAP.md` is generated ([[doc-roadmap-overview]]).
- A partial item carries a `gap` of at most 500 characters saying what is missing now ([[doc-process-agent-rules]], rule 9).
- 786 items as of the 2026-11-18 overview ([[doc-roadmap-overview]]); 791 at origin/main 5b96a1f8 (2026-11-22), after F-881 to F-885 were entered ([[doc-roadmap-json]]).

## Related

- [[roadmap-status]]
- [[roadmap]]
- [[adr]]
