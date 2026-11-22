---
type: source
tags: [interface, case, numerics]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: limited linear-upwind extrapolation

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Limited linear-upwind extrapolation (`solver.linear_upwind_limiter`)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`solver.linear_upwind_limiter` (`none` default, `barth_jespersen`) for `linear_upwind` with `simple`/`simplec` (ADR 0081 part L) and `piso`/`pimple` including the moving-mesh step (ADR 0183 P-A). Opt-in "because it moves every `linear_upwind` result".

## Key points

- Barth-Jespersen factor per velocity component before the face extrapolation; turbulence, scalar and energy equations not limited.
- Measured (ADR 0081): stable on the FAN-01 sector mesh where the unlimited scheme diverges at iteration 7; forced on in all 542 steady `linear_upwind` runs of the test suite, no new divergence-guard trip; Kovasznay L2 order 2.17.
- Not checked: smooth flows with interior extrema of a velocity component.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[fan-01]], [[divergence-guard]]
- [[doc-investigation-job49h-te-divergence]] (`barth_jespersen` removes the NACA 0012 trailing-edge divergence at y+ 2)
