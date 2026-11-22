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

# Reference data: Wave 23A MHD Lorentz force and heat-exchanger references (F-214, F-217, ADR 0039)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 23A MHD Lorentz force and heat-exchanger references (F-214, F-217, ADR 0039)" (lines 950-967) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 23A MHD Lorentz force and heat-exchanger references (ADR 0039).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No web page and no paper was consulted in this wave. Every formula is derived from first principles in ADR 0039 or reproduced from knowledge, and is checked only against closed forms, independent numerical solutions and invariants, NOT against a published table or experiment. Entries are UNVERIFIED as citations unless marked "derived".

| Reference value | Source | Status |
| --- | --- | --- |
| Hartmann flow, short-circuited walls `u = G/(sigma B0^2) (1 - cosh(Ha y/a)/cosh Ha)`, mean `G/(sigma B0^2)(1 - tanh Ha/Ha)`, ratio to Poiseuille `3(Ha - tanh Ha)/Ha^3` | J. Hartmann (1937); textbook MHD (Davidson) | citation UNVERIFIED (memory); the profile is DERIVED in ADR 0039 from `0 = G + mu u'' - sigma B0^2 u` and substituted into the momentum equation numerically (consistency test); the solver is compared with it (A1) |
| Hartmann flow, insulating walls, `u/U = (1 - cosh(Ha y/a)/cosh Ha)/(1 - tanh Ha/Ha)` and `G = sigma B0^2 U (tanh Ha/Ha)/(1 - tanh Ha/Ha)` | same | citation UNVERIFIED; DERIVED in ADR 0039 (zero net current); compared in A2 |
| Lorentz force identities (work `J.E` split, global `integral J x B dV` balance, electrode current `sigma B Ubar`) | electromagnetism | derived/standard; checked at round-off (M1, M2, A3) |
| Effectiveness-NTU, counterflow `(1 - e^{-N(1-Cr)})/(1 - Cr e^{-N(1-Cr)})` (`N/(1+N)` at `Cr = 1`), parallel flow `(1 - e^{-N(1+Cr)})/(1+Cr)`, `Cr = 0`: `1 - e^{-N}` | textbook heat-exchanger theory (Kays and London type) | citation UNVERIFIED (memory); DERIVED in ADR 0039 and compared with an independent marched two-stream integration, worst relative error 4.99e-13 (H1) |
| Crossflow, one stream mixed (`Cmax` mixed: `(1 - exp(-Cr(1 - e^{-N})))/Cr`; `Cmin` mixed: `1 - exp(-(1 - e^{-N Cr})/Cr)`) | same | DERIVED in ADR 0039; compared with cell-exact row recursions, relative error at most 4.4e-9 (H3) |
| Crossflow, both streams unmixed, curve fit `1 - exp[N^0.22/Cr (exp(-Cr N^0.78) - 1)]` | Kays and London type correlation | RETIRED in wave 25B (ADR 0044): replaced by the exact series; the fit itself was never located in a source. Historical note: it is a fit: measured deviation from the exact (cell recursion) solution up to 3.3 percent (H3 known miss) and a wrong small-N slope (H2 known miss). The series solution IS implemented since wave 25B |
| Shell and tube, one shell pass, 2n tube passes closed form | textbook | recalled from memory, UNVERIFIED; checked only for `Cr = 0`, limits, ordering and monotonicity (H2); NO numeric agreement with a published table is claimed |
| Pressure-loss closure `dp = K rho v^2/2 + r1 v`, vent `dp = K rho v^2/2` | standard hydraulics | derived/standard; hand values (H4) and exact imposed drop in the CFD (X2) |
| Porous jump `dp = (mu/alpha) v t + C2 t rho v^2/2`, fan curve (piecewise linear) | ADR 0012 | prior evidence Z1, Z2 (not re-registered); used as analytic operating point in X4 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
