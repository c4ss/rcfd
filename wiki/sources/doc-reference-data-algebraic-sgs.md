---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Algebraic subgrid and mixing-length models (F-045, F-206, ADR 0026)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Algebraic subgrid and mixing-length models (F-045, F-206, ADR 0026)" (lines 776-792) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Algebraic subgrid and mixing-length model constants (ADR 0026), unverified.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


All entries below are written from memory of the published literature; none could be checked against the source text in this environment (marked UNVERIFIED). The closed-form checks of ADR 0026 test the implementation against the formulas as written here, not against the papers.

| Quantity | Source | Status |
| --- | --- | --- |
| Smagorinsky model `nu_t = (Cs Delta)^2 sqrt(2 S_ij S_ij)`, Cs = 0.17 (range 0 to 0.5), `Delta = V^(1/3)` | Smagorinsky (1963); Lilly (1967) for the constant | UNVERIFIED, from memory; Cs is flow dependent (0.1 to 0.2 in the literature) |
| WALE operator, Cw = 0.325, exponents 3/2, 5/2, 5/4 | Nicoud and Ducros (1999) | UNVERIFIED, from memory; the zero for pure shear and the `y^3` wall scaling are derived in the tests, but the nonzero value for rigid rotation (2.86e-3 at w = 3) follows from the formula as written |
| van Driest damping `1 - exp(-y+/A+)`, A+ = 25 (Smagorinsky), 26 (mixing length) | van Driest (1956) | UNVERIFIED, from memory |
| Mixing length `l = kappa d` capped at 0.09 delta | Prandtl; common textbook form | UNVERIFIED, from memory |
| Nikuradse pipe profile `l/R = 0.14 - 0.08 (1 - d/R)^2 - 0.06 (1 - d/R)^4` | Nikuradse (1932) as quoted in textbooks | UNVERIFIED, from memory; no Nikuradse data are in the tree and none was compared |
| Channel Re_tau = 395 mean velocity (U+ at y+ = 30, 50, 100, 150, 300 = 13.493, 14.856, 16.534, 17.646, 19.745; centreline 20.133; bulk 17.545) | Moser, Kim and Mansour (1999) DNS | UNVERIFIED, from memory; used as the ML4/ML5 targets with a 10 percent limit |
| Taylor-Green vortex decay `exp(-2 nu t)` and the velocity field `(sin x cos y, -cos x sin y)` | analytic solution of the Navier-Stokes equations | derived |
| Decaying isotropic turbulence slope | none | not verified, not run, no claim made |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
