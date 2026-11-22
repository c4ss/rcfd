---
type: concept
tags: [process, evidence]
sources: [doc-glossary, doc-process-agent-rules, doc-process-tolerances, doc-process-documentation-review, doc-audit-report]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Registered limit

A pass/fail limit written down before the run it judges. A miss is recorded, not tuned away ([[doc-glossary]]).

## How rcfd uses it

- Every limit cites a source: a row of `docs/process/tolerances.md`, a published reference with its place, or the parent item's acceptance; a limit backed only by the agent's own estimate is listed by the process report and blocks issuing the code ([[doc-process-agent-rules]], rule 15).
- Each row of a checks table states limit, source, estimate, margin and kind ([[doc-process-agent-rules]], rule 14).
- The rows of `docs/process/tolerances.md` are the [[tolerance-class]] values (provisional version 1, approved 2026-11-18); a registration may always set a tighter limit than its row ([[doc-process-tolerances]]).
- Why: "Criteria fixed first stop a bar from being moved; they do not stop it from being set low" (premise P6); the "margin" column (limit divided by the agent's own estimate) flags rows above 3 and does not refuse ([[doc-process-documentation-review]]).
- What it guards against, from the 2026-10-14 audit: post-hoc widened tolerances (F-111 IAPWS 1e-8 to 1e-7), a criterion replaced by a weaker one (F-122), a residual assertion deleted (F-061) ([[doc-audit-report]]; unreviewed model output).

## Related

- [[registration]]
- [[known-miss]]
- [[tolerance-class]]
