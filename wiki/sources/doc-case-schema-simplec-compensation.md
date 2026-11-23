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

# Case schema: SIMPLEC non-orthogonal compensation

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "SIMPLEC non-orthogonal compensation (ADR 0050 Part 1)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Three opt-in SIMPLEC options and the measured stability thresholds on uniformly sheared meshes.

## Key points

- `solver.simplec_non_orthogonal_compensation` (default `false`, ADR 0050): carries the deferred non-orthogonal part in the SIMPLEC flux compensation; without it an axial mode of a swept mesh grows (diverged at `alpha_u = 0.9`, converged at 0.8 and below).
- Sheared channel (ADR 0081 part X, ADR 0084): SIMPLEC with `alpha_u = 0.9`, `alpha_p = 1.0` overshoots from 50 degrees and blows up from 55; e.g. `alpha_p` 0.3 is stable up to 79 degrees. Mechanism: a period-two pressure mode whose eigenvalue passes -1 between 57.5 and 60 degrees.
- `solver.velocity_correction = face_flux` (steady only): stable up to 79 degrees; converged solution the same to 2e-9.
- `solver.simplec_boundary_compensation` (ADR 0102, default `false`): converged SIMPLEC equals SIMPLE (3.5e-12 / 4.7e-13); remaining limit with a strong vent (K = 10). `false` keeps every earlier result bit for bit.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[bit-identical]]
