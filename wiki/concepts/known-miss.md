---
type: concept
tags: [process, evidence]
sources: [doc-glossary, doc-process-agent-rules]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Known miss (and MISSED)

A registered limit the current code does not meet; its test is kept, `#[ignore]`d, with the limit unchanged. 'MISSED' is the status-note word for a registered target a result did not reach ([[doc-glossary]]).

## How rcfd uses it

- Named `*_known_miss`, with the measured number recorded; diagnosed, not tuned ([[doc-process-agent-rules]], rule 2).
- Examples in the milestone gaps: W3b of ADR 0763 (cylinder observed order 1.387 outside [1.5, 2.5]); NACA 0012 at 17 degrees ([[doc-roadmap-overview]]).

## Related

- [[registered-limit]]
- [[roadmap-status]]
- [[simulation-milestones]]
