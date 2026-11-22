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

# Reference data: F-411 power-law creep with time hardening (ADR 0489)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-411 power-law creep with time hardening (ADR 0489)" (lines 2039-2054) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-411 power-law creep with time hardening (ADR 0489).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-411; the books below are cited from memory for the names and the published forms (the chapters, editions and page numbers were not checked). Every formula used is derived in ADR 0489 (arithmetic 1 to 7) and checked by plain numpy prototypes (`f411-scratch/probe_point.py`, `probe_extra.py`, `probe_cylinder.py`, `probe_bar.py` and their logs), no rcfd code. No polymer data is used: every creep constant, modulus and load of the checks is an input (decision A1 of ADR 0310).

| Item | Status | Source |
|---|---|---|
| Von Mises form of the power law with time hardening, $d \varepsilon_c/dt = (3/2) A q^{n-1} t^m s$, uniaxial $A \sigma^n t^m$ (Norton for $m = 0$) | cited, derived | H. Kraus, Creep Analysis, Wiley, 1980; F. K. G. Odqvist, Mathematical Theory of Creep and Creep Rupture, Oxford, 1966; R. K. Penny and D. L. Marriott, Design for Creep, McGraw-Hill, 1971: NOT read, from memory |
| Backward Euler radial return, local equation $q_{\mathrm{tr}} - q - 3 G A \tau q^n = 0$, $\tau = \int t^m dt$ exact over the step; consistent tangent $K 1 \otimes 1 + 2 G \theta I_{\mathrm{dev}} + 2 G (\beta - \theta) N \otimes N$ | derived | ADR 0489 arithmetic 1; the radial return of ADR 0475; J. C. Simo and T. J. R. Hughes, Computational Inelasticity, Springer, 1998 (viscoplasticity chapter, number not checked): NOT read, from memory; probe: tangent against central differences $\le 1.3 \times 10^{-9}$ |
| Uniaxial constant stress $\varepsilon_c = A \sigma^n t^{m+1}/(m+1)$; relaxation at fixed strain $\sigma(t) = (\sigma_{0}^{1-n} + (n - 1) E A t)^{1/(1-n)}$ | derived | integrals of the uniaxial law, ADR 0489 arithmetic 2, 3; probe of the relaxation: `1.717e-3` ($n = 3$), `7.16e-4` ($n = 5$) at 800 steps to `10 t*`, order `0.994` |
| Stationary creep of a thick cylinder in plane strain, $\sigma_r = -p ((b/r)^{2/n} - 1)/((b/a)^{2/n} - 1)$, $\sigma_{\theta} = p ((2 - n)/n (b/r)^{2/n} + 1)/((b/a)^{2/n} - 1)$ (Lame at $n = 1$) | derived | incompressible stationary flow, compatibility $v = C/r$, equilibrium, ADR 0489 arithmetic 6; the classical result as in Kraus (1980), J. T. Boyle and J. Spence, Stress Analysis for Creep, Butterworths, 1983, Penny and Marriott (1971): NOT read, from memory |
| Redistribution time of the cylinder, estimate $t^{*} = 1/(E A p^{n-1})$; registered `5 t*` | estimate, computed | ADR 0314 (stub); probe on 12 x 8 serendipity quadrilaterals (element means `1.2e-4` of $p$ at `5 t*`; point values drift by volumetric locking, `3.0e-2`) |
| Rotating radial bar, $\sigma(r) = \rho \Omega^2 (L^2 - r^2)/2$, tip creep rate $A (\rho \Omega^2/2)^n \int_0^L (L^2 - r^2)^n dr$, $\int_0^L (L^2 - r^2)^3 dr = (16/35) L^7$ | derived | statics of the bar, ADR 0489 arithmetic 7 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
