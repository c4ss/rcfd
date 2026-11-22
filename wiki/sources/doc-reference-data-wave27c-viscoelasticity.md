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

# Reference data: Wave 27C viscoelasticity and structural export (F-135, F-237, ADR 0051)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 27C viscoelasticity and structural export (F-135, F-237, ADR 0051)" (lines 1084-1098) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 27C viscoelasticity and structural export references (ADR 0051).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | Status | Note |
|------|--------|------|
| Oldroyd-B start-up `tau_xy = eta_p gdot (1 - e^{-t/lambda})`, N1 start-up | derived | from the constitutive ODE |
| Uniaxial extension `C_zz = 1/(1 - 2 Wi_e)`, linear growth at `Wi_e = 0.5` | derived | |
| Steady shear: PTT `Y^2 (Y-1) = 2 eps Wi^2`, FENE-P `L2 f^2 (f-1) = 2 Wi^2`, Giesekus implicit form in `f = -A_yy` | derived | checked against the ODE to 1e-12 |
| Giesekus closed form as recalled from the literature (chi form) | unverified, found wrong | 5.1e-3 relative error at Wi = 0.1; kept only as an ignored `known_miss` test |
| FENE-P normalisation `f = (L2-3)/(L2-tr C)`, tau = G(fC - I) | unverified | recalled |
| Log-conformation (matrix logarithm) transport, divided-difference form | derived | chain rule of the matrix logarithm |
| Poiseuille start-up modal series (Oldroyd-B) | derived | cross-checked against an independent 1-D PDE solve in the test |
| PLOAD4, TEMP field layouts | verified | pyNastran documentation pages |
| GRID, element node orders, CHEXA continuation, free-field syntax, PLOAD4 sign | unverified | recalled |
| Third-party solver acceptance of the deck | not tested | no solver available |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
