---
type: source
tags: [interface, case, time]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: landing on solver.end_time

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Landing on `solver.end_time` (ADR 0145)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

How a fixed-step transient run ends on `solver.end_time` despite summed rounding errors.

## Key points

- "2000 steps of 0.05 reach 99.99999999999646, not 100."
- A step short of the end time by less than 1e-6 of itself is lengthened; a larger remainder is a separate last step, with the warning `short_last_step` when it is below 5 % of the step.
- Within the end tolerance (1e-12) the run reports `solver.end_time` exactly as its final time in the last `iteration` event, the `finish` event, the last snapshot and the monitors (ADR 0160); only the time stamp changes.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[status-stream]]
