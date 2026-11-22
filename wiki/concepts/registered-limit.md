---
type: concept
tags: [process, evidence]
sources: [doc-glossary, doc-process-agent-rules]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Registered limit

A pass/fail limit written down before the run it judges. A miss is recorded, not tuned away ([[doc-glossary]]).

## How rcfd uses it

- Every limit cites a source: a row of `docs/process/tolerances.md`, a published reference with its place, or the parent item's acceptance; a limit backed only by the agent's own estimate is listed by the process report and blocks issuing the code ([[doc-process-agent-rules]], rule 15).
- Each row of a checks table states limit, source, estimate, margin and kind ([[doc-process-agent-rules]], rule 14).

## Related

- [[registration]]
- [[known-miss]]
