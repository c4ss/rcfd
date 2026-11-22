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

# Reference data: Wave 27B robustness and adaptive time stepping (F-209, F-235, ADR 0050)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 27B robustness and adaptive time stepping (F-209, F-235, ADR 0050)" (lines 1073-1083) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 27B robustness and adaptive time stepping references (ADR 0050).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | Status | Note |
|------|--------|------|
| Poiseuille pipe `u_c = 2.5`, `U_b = 1.25` for `G = 1`, `nu = 0.1`, `R = 1` | derived | axisymmetric momentum balance, as wave 26B |
| Variable-step BDF2 coefficients `c0 = (1+2r)/(1+r)`, `c1 = 1+r`, `c2 = r^2/(1+r)` | derived | from exactness for `1, t, t^2`; tested to 1e-12; no source read |
| Step-doubling local error estimate, Courant RMS/MAX criteria, safety/growth/shrink factors | derived | first principles (implicit Euler local error `dt^2 y''/2`); no source read |
| Aitken delta-squared relaxation (omega_{k+1} = -omega_k (r_{k-1} . dr) / (dr . dr), dr = r_k - r_{k-1}) | unverified | recalled from the fixed-point acceleration literature; checked only on a linear test iteration |
| Taylor-Green decay `exp(-2 nu t)` | derived | analytic solution of the 2-D Navier-Stokes equations |
| SIMPLEC compensation `rAtU = V/(aP - sum a_nb)` | derived | first principles; the measured `alpha_u` threshold (diverged 0.9, converged 0.8) is a result of this work, not a reference value |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
