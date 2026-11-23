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

# Reference data: Wave 28C adaptation and turbomachinery post-processing (F-102, F-123, ADR 0054)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 28C adaptation and turbomachinery post-processing (F-102, F-123, ADR 0054)" (lines 1114-1132) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 28C adaptation and turbomachinery post-processing references (ADR 0054).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read in this wave. Everything is first-principles or recalled.

| Item | Status | Note |
|------|--------|------|
| Linear reconstruction exact for linear fields; volume-weighted restriction conserves the integral | derived | tested to 6.7e-16 |
| Hanging-node face loops with all lattice vertices on the boundary edges (closed, two-cell faces) | derived | checked by closedness and face-count tests |
| Zienkiewicz-Zhu error indicator (patch-recovered gradient minus cell gradient) | unverified | the recovery is the volume-weighted patch mean, a simplified form; the original SPR/ZZ papers were not read |
| Gradient and Hessian indicators (least squares) | derived | |
| Polyhedral dual of a tet mesh (centroid rings, edge midpoints) | derived | volume and patch areas exact by construction, tested to 1e-15 |
| Kuhn six-tet split of the cube used for the dual test | derived | |
| Manufactured tanh-front diffusion solution | derived | |
| Pump head `dp0/(rho g)`, hydraulic efficiency `rho g H Q/(T omega)` | derived | definitions as used here; conventions of a specific standard not checked |
| Compressor `eta_is = (PR^k-1)/(tau-1)`, `eta_p = k ln PR/ln tau`, `k = (gamma-1)/gamma` | derived | perfect gas, adiabatic; recalled to agree with textbook forms, no book consulted |
| Head coefficient `gH/u_t^2`, flow coefficient `Q/(u_t A)` | unverified | normalisations vary between sources; these are the ones coded |
| Mass-flow and area-weighted averaging formulas | derived | |
| Published pump or compressor performance data (rotor 37, NASA, etc.) | not used | no comparison made; roadmap literature acceptance is not met |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
