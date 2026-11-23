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

# Reference data: F-260 CMA-ES: the algorithm read in Hansen's tutorial and the restart paper (ADR 0222, docs/design-loop.md)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-260 CMA-ES: the algorithm read in Hansen's tutorial and the restart paper (ADR 0222, docs/design-loop.md)" (lines 1514-1534) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-260 CMA-ES read in Hansen's tutorial and the restart paper (ADR 0222).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Read 2026-11-08 for the registration of ADR 0222, before any code. No source listing of any CMA-ES or other optimisation code was opened: the tutorial's Appendix C (MATLAB source code, pages 36 and 37) was not opened or rendered, its link to a Python implementation of the weights (page 32) was not followed, and the MATLAB code version named in a footnote of Auger and Hansen (2005) was not looked at. Nothing in F-260 is compared with measured data.

| Item | Status | What was read (source) |
|---|---|---|
| The (mu/mu_W, lambda)-CMA-ES: sampling $z \sim \mathcal{N}(0, I)$, $y = B D z$, $x = m + \sigma y$; recombination $\langle y\rangle_w$; mean update with $c_m$; step-size path with $C^{-1/2} = B D^{-1} B^T$ and the step-size update with $d_\sigma$ and $\mathrm{E}\lVert\mathcal{N}(0, I)\rVert$; $h_\sigma$ with $(1.4 + 2/(n+1))\, \mathrm{E}\lVert\mathcal{N}(0, I)\rVert$; covariance path and update with $c_1$, $c_\mu$, $\delta(h_\sigma)$ and the negative weights $w_i n / \lVert C^{-1/2} y_i\rVert^2$; $\mathrm{E}\lVert\mathcal{N}(0, I)\rVert \approx \sqrt{n}\left(1 - \frac{1}{4n} + \frac{1}{21 n^2}\right)$ | **read** | N. Hansen, "The CMA Evolution Strategy: A Tutorial", arXiv:1604.00772v2 (2023), Appendix A, Fig. 6, eqs. (38) to (47), page 28 (the arXiv PDF, pages read as rendered images) |
| Default parameters: $\lambda = 4 + \lfloor 3 \ln n\rfloor$, $w'_i = \ln\frac{\lambda + 1}{2} - \ln i$, $\mu = \lfloor \lambda/2\rfloor$, $\mu_{eff}$, $\mu_{eff}^-$, $\alpha_\mu^- = 1 + c_1/c_\mu$, $\alpha_{\mu_{eff}}^- = 1 + 2\mu_{eff}^-/(\mu_{eff} + 2)$, $\alpha_{posdef}^- = (1 - c_1 - c_\mu)/(n c_\mu)$, the weights (53), $c_m = 1$, $c_\sigma = \frac{\mu_{eff} + 2}{n + \mu_{eff} + 5}$, $d_\sigma = 1 + 2\max\left(0, \sqrt{\frac{\mu_{eff} - 1}{n + 1}} - 1\right) + c_\sigma$, $c_c = \frac{4 + \mu_{eff}/n}{n + 4 + 2\mu_{eff}/n}$, $c_1 = \frac{2}{(n + 1.3)^2 + \mu_{eff}}$, $c_\mu = \min\left(1 - c_1,\ \frac{2\,(1/4 + \mu_{eff} + 1/\mu_{eff} - 2)}{(n + 2)^2 + \mu_{eff}}\right)$ | **read** | the same, Table 1 (eqs. 48 to 58), page 31; the generalised $c_c$ (61) on page 32 was read and not used |
| Initial values: $p_\sigma = p_c = 0$, $C = I$; "If the optimum is expected to be in the initial search interval [a, b]^n we may choose ... m uniformly randomly in [a, b]^n, and sigma = 0.3(b - a)"; different intervals by $c_{ii}$ or by a scaling of the variables | **read** | the same, Fig. 6 and its footnote, page 29 |
| Termination criteria NoEffectAxis (0.1 standard deviation along an axis, $i = (g \bmod n) + 1$), NoEffectCoord (0.2 standard deviations in a coordinate), ConditionCov ($10^{14}$), EqualFunValues and TolFun ($10 + \lceil 30 n/\lambda\rceil$ generations, $10^{-12}$), Stagnation (20 %, at least $120 + 30 n/\lambda$, at most 20 000 iterations, medians of the first and last 30 %), TolXUp ($10^4$), TolX ($10^{-12}$ times the initial $\sigma$) | **read** | the same, Appendix B.3, pages 33 and 34 |
| Box handling: $f_{max} + \lVert x - x_{feasible}\rVert$ (62) with $x_{feasible}$ "preferably in the middle of the feasible domain", or resampling, when the optimum is not too close to the boundary; repair with the penalty $\alpha \lVert x - x_{repaired}\rVert^2$ (63) | **read** | the same, Appendix B.5, pages 34 and 35 (rcfd uses (62) and does not evaluate the infeasible point) |
| Recomputing $B$ and $D$ only every $\max\left(1, \left\lfloor \frac{1}{10 n (c_1 + c_\mu)}\right\rfloor\right)$ generations | **read**, not used | the same, Appendix B.2 (rcfd decomposes every generation) |
| Restarts with the population doubled ("increased by a factor of 2"); stopping criteria Tolfun $10^{-12}$, TolX $10^{-12}\,\sigma_0$, noeffectaxis, noeffectcoord, conditioncov $10^{14}$; initial point uniform in $[A, B]^n$ and $\sigma_0 = (B - A)/2$ per restart; budget $n \times 10^4$, target error $10^{-8}$; Table 2 (n = 10): shifted Rastrigin success rate 0.76 (25 runs, error $10^{-2}$ within $n \times 10^4$), median 7.85e4 evaluations | **read** | A. Auger, N. Hansen, "A Restart CMA Evolution Strategy With Increasing Population Size", Proc. IEEE CEC 2005, 1769-1776, pages 1 to 4 (copy in the CEC 2005 contributions at sci2s.ugr.es). rcfd keeps the given start point for every restart (ADR 0222 question 5) and the tutorial's $0.3\,(b - a)$ |
| Box-Muller normal draws $\sqrt{-2 \ln u_1}\, \cos(2\pi u_2)$ | from memory | G. E. P. Box, M. E. Muller, Ann. Math. Statist. 29 (1958) 610-611, NOT read; checked by ADR 0222 K3 (moments and the distribution function against Simpson's rule) |
| Cyclic Jacobi eigenvalue method for symmetric matrices | from memory | the textbook method (for example Golub and Van Loan, Matrix Computations), NOT read for ADR 0222; checked by ADR 0222 K2 (reconstruction and orthogonality) |
| Rastrigin ($10 d + \sum (x_i^2 - 10 \cos 2\pi x_i)$), sphere, ellipsoid $\sum 10^{6 (i-1)/(d-1)} x_i^2$, Rosenbrock | from memory | standard test functions; checked by their closed forms (ADR 0222 K0) |
| Hansen and Kern (2004), "Evaluating the CMA Evolution Strategy on Multimodal Test Functions", PPSN VIII | not obtained | no open copy found |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[design-loop]]
