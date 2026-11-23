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
| Hartmann flow, short-circuited walls $u = \dfrac{G}{\sigma B_0^2} \left(1 - \dfrac{\cosh(Ha\, y/a)}{\cosh Ha}\right)$, mean $\dfrac{G}{\sigma B_0^2}(1 - \tanh Ha/Ha)$, ratio to Poiseuille $3(Ha - \tanh Ha)/Ha^3$ | J. Hartmann (1937); textbook MHD (Davidson) | citation UNVERIFIED (memory); the profile is DERIVED in ADR 0039 from $0 = G + \mu u'' - \sigma B_0^2 u$ and substituted into the momentum equation numerically (consistency test); the solver is compared with it (A1) |
| Hartmann flow, insulating walls, $u/U = \dfrac{1 - \cosh(Ha\, y/a)/\cosh Ha}{1 - \tanh Ha/Ha}$ and $G = \sigma B_0^2 U \dfrac{\tanh Ha/Ha}{1 - \tanh Ha/Ha}$ | same | citation UNVERIFIED; DERIVED in ADR 0039 (zero net current); compared in A2 |
| Lorentz force identities (work $\mathbf{J}\cdot\mathbf{E}$ split, global $\int \mathbf{J}\times\mathbf{B}\, dV$ balance, electrode current $\sigma B \bar U$) | electromagnetism | derived/standard; checked at round-off (M1, M2, A3) |
| Effectiveness-NTU, counterflow $(1 - e^{-N(1-C_r)})/(1 - C_r e^{-N(1-C_r)})$ ($N/(1+N)$ at $C_r = 1$), parallel flow $(1 - e^{-N(1+C_r)})/(1+C_r)$, $C_r = 0$: $1 - e^{-N}$ | textbook heat-exchanger theory (Kays and London type) | citation UNVERIFIED (memory); DERIVED in ADR 0039 and compared with an independent marched two-stream integration, worst relative error 4.99e-13 (H1) |
| Crossflow, one stream mixed ($C_{max}$ mixed: $\left(1 - \exp(-C_r(1 - e^{-N}))\right)/C_r$; $C_{min}$ mixed: $1 - \exp\!\left(-(1 - e^{-N C_r})/C_r\right)$) | same | DERIVED in ADR 0039; compared with cell-exact row recursions, relative error at most 4.4e-9 (H3) |
| Crossflow, both streams unmixed, curve fit $1 - \exp\!\left[\dfrac{N^{0.22}}{C_r} \left(e^{-C_r N^{0.78}} - 1\right)\right]$ | Kays and London type correlation | RETIRED in wave 25B (ADR 0044): replaced by the exact series; the fit itself was never located in a source. Historical note: it is a fit: measured deviation from the exact (cell recursion) solution up to 3.3 percent (H3 known miss) and a wrong small-N slope (H2 known miss). The series solution IS implemented since wave 25B |
| Shell and tube, one shell pass, 2n tube passes closed form | textbook | recalled from memory, UNVERIFIED; checked only for $C_r = 0$, limits, ordering and monotonicity (H2); NO numeric agreement with a published table is claimed |
| Pressure-loss closure $\Delta p = K \rho v^2/2 + r_1 v$, vent $\Delta p = K \rho v^2/2$ | standard hydraulics | derived/standard; hand values (H4) and exact imposed drop in the CFD (X2) |
| Porous jump $\Delta p = (\mu/\alpha)\, v t + C_2 t \rho v^2/2$, fan curve (piecewise linear) | ADR 0012 | prior evidence Z1, Z2 (not re-registered); used as analytic operating point in X4 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
