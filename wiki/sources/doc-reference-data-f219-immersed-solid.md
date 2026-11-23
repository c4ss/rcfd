---
type: source
tags: [reference-data, f-item-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-219 immersed solid with modified forcing (ADR 0218)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-219 immersed solid with modified forcing (ADR 0218)" (lines 1437-1446) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-219 immersed solid with modified forcing (ADR 0218): DFG 2D-1 and 2D-2 reference values.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | Status | What was read (source) |
|---|---|---|
| DFG 2D-1 set-up (channel `[0, 2.2] x [0, 0.41]`, cylinder centre `(0.2, 0.2)`, radius 0.05, $\rho = 1$, $\nu = 10^{-3}$, inflow $4 U y (0.41 - y)/0.41^2$, $U = 0.3$, $U_{\mathrm{mean}} = 0.2$, Re 20, $C_{D,L} = 2 F_{D,L}/(U_{\mathrm{mean}}^2 L)$, $L = 0.1$, pressure difference between `(0.15, 0.2)` and `(0.25, 0.2)`, do-nothing outflow) and reference values $C_D = 5.57953523384$, $C_L = 0.010618948146$, $p_{\mathrm{diff}} = 0.11752016697$ | read | FeatFlow benchmark page "DFG flow around cylinder benchmark 2D-1, laminar case (Re=20)", `wwwold.mathematik.tu-dortmund.de/~featflow/en/benchmarks/cfdbenchmarking/flow/dfg_benchmark1_re20.html`, read 2026-11-07 ("computed with high order spectral methods") |
| DFG 2D-2 set-up ($U = 1.5$, $U_{\mathrm{mean}} = 1$, Re 100, $\mathrm{St} = L f/U_{\mathrm{mean}}$) and the level-6 values at $k = 1/200$: $\mathrm{St} = 0.30188$, $1/f = 0.33125$, max/min/mean $C_D$ 3.2200/3.1569/3.1884, max/min $C_L$ 0.9859/-1.0206 | read | FeatFlow benchmark page "DFG flow around cylinder benchmark 2D-2, time-periodic case (Re=100)", `.../dfg_benchmark2_re100.html`, read 2026-11-07 |
| The lower and upper bounds of Schaefer and Turek (1996), Notes on Numerical Fluid Mechanics 52, 547-566 | not obtained | the paper was not read; the bounds are not used |
| Volume penalisation (Angot, Bruneau, Fabrie 1999), direct forcing (Mohd-Yusof 1997; Fadlun et al. 2000), the linear Dirichlet extrapolation to the interface crossing (Gibou, Fedkiw, Cheng, Kang 2002) | from memory | NOT read; the algebra of ADR 0218 D2 is derived on rcfd's discrete equations |
| Circular Couette flow $u_{\theta} = A r + B/r$, $A = -\omega R_{1}^2/(R_{2}^2 - R_{1}^2)$, $B = \omega R_{1}^2 R_{2}^2/(R_{2}^2 - R_{1}^2)$, torque $4 \pi \mu \omega R_{1}^2 R_{2}^2/(R_{2}^2 - R_{1}^2)$ | derived | textbook solution of the axisymmetric Navier-Stokes equations, derived here |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[dfg-cylinder-benchmark]]
