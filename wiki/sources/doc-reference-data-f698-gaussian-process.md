---
type: source
tags: [reference-data, fan-and-validation]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-698 Gaussian-process surrogate and expected improvement: sources (ADR 0621, docs/design-loop.md)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-698 Gaussian-process surrogate and expected improvement: sources (ADR 0621, docs/design-loop.md)" (lines 2390-2409) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-698 Gaussian-process surrogate and expected improvement sources and test functions (ADR 0621).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Recorded 2026-11-16 for ADR 0621. No paper was opened (none is in the repository); no source listing of any Gaussian-process, kriging or Bayesian-optimisation code was opened. Every item is from memory and guarded by a check of its own.

| What is used | Status | Source and guard |
|---|---|---|
| Kriging with a constant mean (GLS), profiled process variance, concentrated likelihood, predictive variance with the mean-uncertainty term | from memory, derived | Jones, Schonlau and Welch (1998), J. Global Optim. 13; Rasmussen and Williams (2006), ch. 2; NOT read; guarded by ADR 0621 G2 (two-point closed form derived in the ADR) |
| Matern 5/2 and squared-exponential correlations | from memory | Rasmussen and Williams (2006), eq. 4.17; Stein (1999); NOT read; G2 writes the kernels out |
| Gradient of the log likelihood `(a^T dK a / s2 - tr(K^-1 dK)) / 2` | derived | envelope theorem on the profiled likelihood (ADR 0621 amendment 1); G4 against central differences |
| Leave-one-out from one factorisation with an estimated mean | from memory, derived | Dubrule (1983), Math. Geol. 15; NOT read; G7 against explicit refits |
| Expected improvement `(f_min - m) Phi(z) + s phi(z)` | from memory, derived | Mockus, Tiesis and Zilinskas (1978); Jones et al. (1998) eq. 15; NOT read; E1 against Simpson's rule |
| Imputation of failed points at `m + s` | from memory | Forrester, Sobester and Keane (2006), Proc. R. Soc. A 462; NOT read; F1, F2 |
| Probability of feasibility; least-squares classification on +-1 labels | from memory | Schonlau, Welch and Jones (1998); Gardner et al. (2014); Rasmussen and Williams (2006) section 6.5; NOT read; K1, R1 |
| Kriging believer | from memory | Ginsbourger, Le Riche and Carraro (2010); NOT read; P1b |
| Branin, Hartmann 3 and 6 (constants, minima 0.397887, -3.86278, -3.32237) | from memory | Dixon and Szego (1978); NOT read; ADR 0621 G0 evaluates the functions at the stated minimisers (within 4e-7 and 2e-6) |
| Basin restart of expected improvement (ADR 0651): trigger, basin ellipsoid in the length scales, a fresh process outside it | derived, parameters from smoke runs | the idea of excluding an exhausted region is in tabu search (Glover 1989) and the restarts of trust-region Bayesian optimisation (Eriksson et al. 2019); both from memory, NOT read; ADR 0651 A1 (Hartmann 6, minimum still from memory as above), A6 |
| Shekel 10 on `[0, 10]^4` (ten centres, widths `beta = (1, 2, 2, 4, 4, 6, 3, 7, 5, 5)/10`, minimum -10.5364 near (4, 4, 4, 4)) | from memory | Dixon and Szego (1978); NOT read; ADR 0673 S0 finds the minimum numerically by Nelder-Mead from (4, 4, 4, 4) and 40 seeded starts |



## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[design-loop]]
