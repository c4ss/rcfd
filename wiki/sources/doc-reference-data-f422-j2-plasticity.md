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

# Reference data: F-422 J2 plasticity with isotropic and kinematic hardening, radial return and consistent tangent (ADR 0475)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-422 J2 plasticity with isotropic and kinematic hardening, radial return and consistent tangent (ADR 0475)" (lines 1890-1903) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-422 J2 plasticity with hardening (ADR 0475).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-422; the books are cited from memory for the names and the published form. Every formula used is derived in ADR 0475 (arithmetic 1 to 6) and checked by a plain numpy prototype (`f422-scratch/probe.py`): the tangent against central differences, the uniaxial bar, the closed forms of the sphere and the beam.

| Item | Status | Source |
|---|---|---|
| Radial return for von Mises plasticity with linear isotropic and kinematic hardening, $dg = f / (2 \mu + (2/3)(H_{\mathrm{iso}} + H_{\mathrm{kin}}))$ (exact for linear hardening) | derived | consistency of the yield condition after the step, ADR 0475 arithmetic 1; J. C. Simo and R. L. Taylor, Comput. Meth. Appl. Mech. Engng 48 (1985) 101; J. C. Simo and T. J. R. Hughes, Computational Inelasticity, Springer, 1998, chapter 3; E. A. de Souza Neto, D. Peric and D. R. J. Owen, Computational Methods for Plasticity, Wiley, 2008: NOT read, from memory |
| Consistent tangent $C = \kappa 1 \otimes 1 + 2 \mu \theta I_{\mathrm{dev}} - 2 \mu \bar{\theta} n \otimes n$, $\theta = 1 - 2 \mu dg/\lvert \xi_{\mathrm{tr}} \rvert$, $\bar{\theta} = 1/(1 + H/(3 \mu)) - (1 - \theta)$ | derived | linearisation of the return map, ADR 0475 arithmetic 2; prototype against central differences `1.2e-11 .. 6.9e-10`; same books: NOT read |
| Bilinear uniaxial response $\sigma_y + (E H/(E + H))(\varepsilon - \sigma_y/E)$, $H = H_{\mathrm{iso}} + H_{\mathrm{kin}}$; re-yield at $-\sigma_{\mathrm{max}}$ (isotropic) and $\sigma_{\mathrm{max}} - 2 \sigma_y$ (kinematic) | derived | uniaxial reduction of the yield condition, ADR 0475 arithmetic 2 |
| Partly plastic thick sphere: $p = 2 \sigma_y \ln(c/a) + (2 \sigma_y/3)(1 - c^3/b^3)$, limit $2 \sigma_y \ln(b/a)$, elastic shell $\sigma_{\theta} - \sigma_r = \sigma_y (c/r)^3$; $b/a = 2$: $p(1.25 a) = 0.950193 \sigma_y$, $p(1.5 a) = 1.196347 \sigma_y$ | derived | equilibrium, Tresca = von Mises under spherical symmetry, Lame shell, ADR 0475 arithmetic 3; R. Hill, The Mathematical Theory of Plasticity, Oxford, 1950, chapter 5: NOT read, from memory |
| Residual hoop stress at the bore after elastic unloading from `p(1.25 a)`, $b/a = 2$: $\sigma_y - (12/7) p = -0.628903 \sigma_y$ (elastic unloading for $p < (7/6) \sigma_y$) | derived | superposition of the Lame field of `-p`, ADR 0475 arithmetic 4 |
| Elastic-perfectly plastic rectangle in pure bending, $M/M_y = 1.5 - 0.5 (\kappa_y/\kappa)^2$ | derived | uniaxial fibres, elastic core $y_e = (h/2) \kappa_y/\kappa$, ADR 0475 arithmetic 5 |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
