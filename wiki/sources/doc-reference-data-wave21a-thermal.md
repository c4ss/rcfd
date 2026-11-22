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

# Reference data: Wave 21A thermal extras references (F-215, F-213, F-212, ADR 0035)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 21A thermal extras references (F-215, F-213, F-212, ADR 0035)" (lines 896-910) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 21A thermal extras references (ADR 0035).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


ADR 0035 derives the closed forms below from conservation and Fourier/Ohm laws, and each is checked by a registered test. None is a data claim. Web search was attempted for the Stefan solution and the enthalpy-porosity paper; primary texts were not retrievable.

| Quantity | Source | Status |
| --- | --- | --- |
| Series thermal resistance, composite wall with contact resistance, `q = dT / (sum t_i/k_i + R_c)` | heat-conduction theory | derived here (ADR 0035 D1); no external source needed |
| Ohm bar, Joule parabola `T = T0 + (J^2 rho/ (2 k)) x (L - x)`, `sigma(T) = sigma_0/(1 + a (T - T_ref))` bar solution and lumped Joule ODE | conduction theory | derived here (ADR 0035 D2); checked by H1 to H6 |
| Two-phase Neumann/Stefan solution `St_l e^{-l^2}/(sqrt(pi) erf l) - St_s e^{-l^2}/(sqrt(pi) erfc l) = l`, front `s = 2 l sqrt(alpha t)` (one-phase limit `St_s = 0`) | textbook form (H. S. Carslaw and J. C. Jaeger 1959, Conduction of Heat in Solids; V. Alexiades and A. D. Solomon 1993; J. Crank 1984) | UNVERIFIED against a primary text: a search tool returned the same form but not from the textbook; the root is checked in the tests through the residual (P7) and the numerical front converges to it |
| Enthalpy-porosity model, Carman-Kozeny damping `A(f) = C (1 - f)^2 / (f^3 + b)` | V. R. Voller and C. Prakash (1987), Int. J. Heat Mass Transfer 30(8), 1709-1719 | verified citation (bibliographic record by search); paper not read. `C` (mushy constant) and `b` are case dependent: no universal value is claimed, the tests choose `C` from the target permeability `kappa` |
| Brinkman channel `u = (G/kappa)(1 - cosh(m(y - H))/cosh(mH))`, `m = sqrt(kappa/nu)` | momentum balance with a linear sink | derived here (ADR 0035, K2) |
| Gallium-melting cavity experiment (published front shape) | not obtained | NOT used: not delivered, named gap of F-212 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
