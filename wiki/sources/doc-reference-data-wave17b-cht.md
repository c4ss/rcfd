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

# Reference data: Wave 17B conjugate heat transfer (F-051, ADR 0024)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 17B conjugate heat transfer (F-051, ADR 0024)" (lines 742-762) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 17B conjugate heat transfer references (ADR 0024): Stefan-Boltzmann constant and Nusselt values.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Every reference value used by the registered checks is derived in the ADR or is closed-form arithmetic. Every source attribution below is from memory and unverified as a citation:

| Quantity | Source | Status |
| --- | --- | --- |
| 1-D steady conduction through layers in series with a convective end, `q = dT / (sum L_i/k_i + 1/h)` | standard heat transfer | derived (series resistances); unverified (memory) as a citation |
| Fin of uniform section, tip convection: `Q = sqrt(h P k A) theta_b (sinh mL + (h/mk) cosh mL) / (cosh mL + (h/mk) sinh mL)` | standard heat transfer | recalled from memory; wave 31C: the formula (convective tip, `m^2 = h P / (k A_c)`) is printed in the Wikipedia article "Fin (extended surface)" (secondary, formulas from the page's LaTeX alt text): verified (secondary); the 1% bound is an engineering limit and the measured 3.3e-4 compares the 2-D solve with this 1-D formula |
| Lumped slab `T = T_inf + (T0 - T_inf) exp(-t/tau)` (small Biot number) | standard | derived; unverified (memory); the 1% bound is an engineering limit (Biot number of the set-up is the ADR's) |
| Semi-infinite contact of two solids, `T_i = (e1 T1 + e2 T2)/(e1 + e2)`, `e = sqrt(k rho c)`, `T = T_i -/+ (..) erfc(x / (2 sqrt(alpha t)))` | Carslaw and Jaeger; Incropera | unverified (memory); the `erfc` series and continued fraction are implemented in the test file and were not compared with a table |
| Stefan-Boltzmann constant | CODATA exact SI values `sigma = 2 pi^5 k_B^4 / (15 h^3 c^2)` | computed in code from exact SI constants and unit-tested against 5.670374419e-8; the value 5.670374419e-8 is verified in wave 31C: the NIST CODATA 2022 page physics.nist.gov/cgi-bin/cuu/Value?sigma prints "5.670 374 419... x 10^-8 W m^-2 K^-4", standard uncertainty (exact) |
| Fully developed laminar flow between parallel plates with uniform wall heat flux, `Nu = 140/17 = 8.2353` (`D_h = 4H`, half channel `H`) | derived in ADR 0024 section D from the energy equation with Poiseuille velocity | derived; the textbook value 8.235 (uniform flux, plates) was recalled from memory and is verified since wave 30C (NPTEL ME-662 lecture 19, limit 8.2353); the derivation, not the recollection, is the reference |
| Contact resistance `R''` in series, `1/k_eff = 1/k + R''/Delta` | standard | derived; unverified (memory) |

Known deviations of the checks from the ADR text (details in the ADR addendum):

- The duct tests use central convection, since upwind is not exact for the linear axial profile at the Dirichlet inlet face.
- The steady stop criterion of the library tests is 1e-11, not 1e-13.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
