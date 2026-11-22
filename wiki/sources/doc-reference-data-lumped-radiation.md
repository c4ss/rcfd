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

# Reference data: Lumped zones and radiation (F-113, F-130, ADR 0012, `crates/rcfd-api/tests/lumped.rs`, `radiation.rs`)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Lumped zones and radiation (F-113, F-130, ADR 0012, `crates/rcfd-api/tests/lumped.rs`, `radiation.rs`)" (lines 561-576) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Lumped zones and radiation (F-113, F-130, ADR 0012): Milne value, known misses, WSGG not implemented.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | Reference | Source status |
| --- | --- | --- |
| Porous-jump law $\Delta p = (\mu/\alpha)\, t v + C_2 t \rho v^2/2$ | analytic Darcy-Forchheimer sheet | definition used by the code itself; not checked against vendor documentation (clean-room) |
| Fan operating point | intersection of fan curve and system curve by bisection | standard fan-engineering definition; no published benchmark case compared |
| Effectiveness $Q = \varepsilon\, C_{min} (T_{sec} - T_{in})$ | Kays and London, Compact Heat Exchangers | formula from memory of the standard definition; the book was not accessed |
| P1 / Marshak slab $q = \sigma (T_1^4 - T_2^4) / (1/e_1 + 1/e_2 - 1 + 3\tau/4)$ | Modest, Radiative Heat Transfer (P1 approximation, Marshak condition $\mathbf{q}\cdot\mathbf{n} = -\left(e/(2(2-e))\right)(E_b - G)$) | derived from the equations here; the book was not accessed |
| Milne extrapolation length 0.7104 and P1 value 2/3 | secondary source excerpt (ANS letter on extrapolation distances, found through a web search) | NOT verified against a primary source; that radiative equilibrium between plates reduces to the Milne problem is my own derivation. R2 (0.41 % P1 vs Milne asymptote) is therefore only as good as these two statements. Wave 13: the value 0.7104 is confirmed (Hopf constant 0.7104460895, arXiv:1704.05940 eq. 62; ANS letter wx1.ans.org/pubs/journals/download/a_26506 gives 0.7104 Milne vs 0.6667 Marshak/P1); the slab mapping Psi = 4/(3 tau + 6 x 0.7104) was re-derived by the verifier, not found in a source |
| Rosseland diffusion $k_r = 16 \sigma n^2 T^3 / (3 \beta)$ | Modest (diffusion approximation) | standard formula; book not accessed. Wave 13: form confirmed (secondary) against the COMSOL Rosseland model description |
| WSGG coefficients | none | not implemented: no coefficient table could be verified from an accessible published source |

ADR 0012 (addendum) records the measurements against these references. Registered misses: Z3 network pressure 8.5e-4 vs 1e-4 (`z3_network_pressure_known_miss`), Z4 effectiveness duty vs nominal formula 1.45e-6 vs 1e-6 (`z4_effectiveness_nominal_formula_known_miss`).


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
