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

# Reference data: F-682 the interaction integral on curved crack fronts (ADR 0610)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-682 the interaction integral on curved crack fronts (ADR 0610)" (lines 2315-2327) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-682 interaction integral on curved crack fronts (ADR 0610).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-682. The papers below are cited from memory. The correction terms are derived in ADR 0610 D1 and verified against central differences (arithmetic 1, `f682-scratch/py/curv.py`, numpy, no rcfd code); the closed forms of the penny under shear are derived from the 2-D forms of ADR 0605 (D2) and verified numerically (arithmetic 2, `f682-scratch/py/shear_penny.py`).

| Item | Status | Source |
|---|---|---|
| Correction terms of the incompatible auxiliary field $H = R G_{\mathrm{loc}} R^T$ on a curved plane front: $\operatorname{div} \sigma^{\mathrm{aux}} = (\kappa/h) (sa_{11} - sa_{33}, sa_{21}, 2 sa_{13})$ and $\sigma_{ij} (H_{ik,j} - \varepsilon^{\mathrm{aux}}_{ij,k}) q_k$ in the local frame ($h = 1 + \kappa x_{1}$) | derived, verified | ADR 0610 D1 and arithmetic 1 (agreement with central differences `1.2e-10` of the scale); M. Gosz and B. Moran, Eng. Fract. Mech. (2002); M. C. Walters, G. H. Paulino and R. H. Dodds, Eng. Fract. Mech. (2005): NOT read, from memory (journal and year as recalled, no volume or page) |
| Penny crack, remote shear $\tau$ along $y$: $K_{\mathrm{II}} = 4 \tau \sqrt{a/\pi} \sin \theta/(2 - \nu)$, $K_{\mathrm{III}} = -4 (1 - \nu) \tau \sqrt{a/\pi} \cos \theta/(2 - \nu)$ (frame `(e, n, e x n)`, $\theta$ from $x$) | derived, verified | ADR 0610 D2: the Fourier reduction to the 2-D forms of ADR 0605 D6, the uniform slip traction of the ellipsoidal slip ($\phi_{xx} = \phi_{yy} = -\pi^2/2$, $\phi_{xy} = 0$ inside, verified at five points), $\nu \to 0$, the energy average; M. K. Kassir and G. C. Sih, J. Appl. Mech. (1966); H. Tada, P. C. Paris and G. R. Irwin, The Stress Analysis of Cracks Handbook: NOT read, from memory |
| Measured: penny under inclined tension ($\beta = 45^\circ$, half $x \ge 0$ of the whole cylinder, 122 145 unknowns) ring 3: $K_{\mathrm{I}}$ within `5.8e-4`, $K_{\mathrm{II}}$ within $1.6 \times 10^{-4} A_{\mathrm{II}}$, $K_{\mathrm{III}}$ within $1.5 \times 10^{-3} A_{\mathrm{III}}$ of the hat-averaged closed forms; J of the domain integral within `1.8e-2` of $G$ of the closed forms; the domain term alone off by `3.1e-2`, `0.135`, `7.0e-2`; elliptical crack $b/a = 0.5$ mode I within `2.5e-3` of Irwin | computed | ADR 0610 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
