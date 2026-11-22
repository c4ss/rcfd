---
type: source
tags: [reference-data, structural]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-432 stress recovery by element patches and the energy-norm error estimate (ADR 0478)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-432 stress recovery by element patches and the energy-norm error estimate (ADR 0478)" (lines 1919-1931) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-432 stress recovery and energy-norm error estimate (ADR 0478).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-432; every method is cited from memory and derived in ADR 0478 as a discrete least-squares fit and an energy integral. The checks use closed forms (polynomial fields), the manufactured solution of F-378 (true error known) and Kirsch's hole as recorded above (F-413).

| Item | Status | Source |
|---|---|---|
| Superconvergent patch recovery: least-squares fit of a complete polynomial to the stresses at the sampling points of the elements around a vertex node, nodal values from the fit; boundary nodes from an interior patch | from memory | Zienkiewicz and Zhu, "The superconvergent patch recovery and a posteriori error estimates. Part 1: The recovery technique", Int. J. Numer. Meth. Engng 33 (1992) 1331-1364: NOT read; ADR 0478 D1 to D3 (the nearest-centre rule of D3 is this repository's) |
| Error estimate $\eta_e^2 = \int (\sigma^{*} - \sigma_h) \cdot D^{-1} (\sigma^{*} - \sigma_h) dV$, relative estimate $\eta/(\lVert u_h \rVert^2 + \eta^2)^{1/2}$ | from memory | Zienkiewicz and Zhu, "A simple error estimator and adaptive procedure for practical engineering analysis", Int. J. Numer. Meth. Engng 24 (1987) 337-357; Part 2 of the 1992 paper, ibid. 1365-1382: NOT read; ADR 0478 D4 |
| Effectivity index $\theta = \eta/\lVert u - u_h \rVert_E$, asymptotic exactness when the recovered stress converges faster than the raw one | from memory | Ainsworth and Oden, A Posteriori Error Estimation in Finite Element Analysis, Wiley (2000), the chapter on recovery-based estimators: NOT read; measured in ADR 0478 (M1, M2) |
| Optimal stress points: the centroid of the trilinear hexahedron, the 2 x 2 x 2 Gauss points of the quadratic hexahedron | from memory | Barlow, "Optimal stress locations in finite element models", Int. J. Numer. Meth. Engng 10 (1976) 243-251: NOT read; the 1-D argument of ADR 0478 arithmetic 3 |
| One-dimensional effectivities of the rule (linear `1.080, 1.025, 1.007` and quadratic `0.854, 0.971, 0.995` at $n = 4, 8, 16$) | computed | numpy, `f432-scratch/probe_1d.py`, `probe_1d_quad.py` (ADR 0478 arithmetic 1, 2) |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
