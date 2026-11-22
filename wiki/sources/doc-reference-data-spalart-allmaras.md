---
type: source
tags: [reference-data, model-constants]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Spalart-Allmaras (F-041, ADR 0014)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Spalart-Allmaras (F-041, ADR 0014)" (lines 593-603) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Spalart-Allmaras constants and the TMR flat-plate skin friction (F-041, ADR 0014).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Quantity | Source | Status |
| --- | --- | --- |
| SA-noft2 equations, sigma 2/3, cb1 0.1355, cb2 0.622, kappa 0.41, cw2 0.3, cw3 2, cv1 7.1, S~ limiter (c2 0.7, c3 0.9), SA-neg (cn1 16, fn, far-field 3-5 nu) | NASA TMR https://tmbwg.github.io/turbmodels/spalart.html | verified by web fetch 2026-10-15; the original Spalart and Allmaras 1994 paper and Allmaras et al. 2012 were not re-fetched |
| Cf(x = 0.97) = 2.70565e-3 (CFL3D SA) | TMR `FlatPlate/SA/cf_cfl3d.dat` (pinned commit 20a39f54..., SHA-256 2298419a...5c6c), interpolated between rows 0.96005368 (2.70961295e-3) and 0.97008407 (2.70562153e-3) | value verified; the grid is not stated in the file ("finest 545x385" is an inference from the page, UNVERIFIED) |
| Theory curves at x = 0.97: 2.72379e-3, 2.88302e-3, 2.87459e-3 | TMR `cf_as_function_of_x.dat` read off the pinned file | read from file; their exact definition is not documented there |
| White 0.455/ln^2(0.06 Re_x), power law 0.0576 Re_x^-0.2 | White, Viscous Fluid Flow, flat-plate correlations | from memory, as for the SST entry; wave 13: White's form confirmed (Jaffer arXiv:1810.05743v12 eq. 8), the 0.0576 power law by CFD-Online |
| S5 tolerance 10 % and the y+ band [20, 60] | rcfd choice (ADR 0014) | not a literature value |



## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[flat-plate]]
