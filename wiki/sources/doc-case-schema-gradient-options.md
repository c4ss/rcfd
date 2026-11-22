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

# Case schema: gradient options of the diffusion term

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Gradient options of the energy and scalar diffusion term (`solver.gradient_*`)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Four keys that change the explicit non-orthogonal part of `-div(gamma grad phi)` (ADR 0016 D2, wired by ADR 0190), and the table `[solver.gradient_equations]` that extends them to momentum, pressure, turbulence and the electric potential (ADR 0204, ADR 0221).

## Key points

- `gradient_face_correction` (`false`), `gradient_secondary_limiter` (`none`/`barth_jespersen`/`venkatakrishnan`), `gradient_limiter_k` (5.0), `gradient_boundary` (`owner`/`corrected`/`dropped`).
- "No benefit is claimed": on the manufactured problem the options keep second order but "do not reduce the error"; the face correction raised it 2.1 times on an affine skewed mesh.
- With `pressure`, the fixed-pressure face flux correction is consistent (without it, 2.0e-3 of the inflow mass-balance error on a 30-degree wedge, ADR 0221 M).
- At defaults: bit for bit, left out of the settings hash; written case files list the keys (the table is left out).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[bit-identical]]
