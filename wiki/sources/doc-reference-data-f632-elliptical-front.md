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

# Reference data: F-632 the domain integral on an elliptical front, and the separation of K_I, K_II and K_III (ADR 0605)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-632 the domain integral on an elliptical front, and the separation of K_I, K_II and K_III (ADR 0605)" (lines 2300-2314) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-632 domain integral on an elliptical front (ADR 0605).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-632. The papers below are cited from memory. Every formula used is derived or verified numerically in ADR 0605 (D4 to D6, arithmetic 1 to 6) by plain Python scripts (`f632-scratch/py`, no rcfd code); the references of the checks are closed forms evaluated in the tests.

| Item | Status | Source |
|---|---|---|
| Embedded elliptical crack under remote tension, $K_{\mathrm{I}}(\phi) = \sigma \sqrt{\pi b}/E(k) (\sin^2 \phi + (b/a)^2 \cos^2 \phi)^{1/4}$, $k^2 = 1 - b^2/a^2$ | derived (shape), verified (factor) | ADR 0605 D4: the shape from the ellipsoidal opening and the local plane-strain opening; the factor `w0 ∝ b/E(k)` verified numerically ($-\operatorname{Lap} \phi\, b/E(k) = 2 \pi$ for $b/a = 1, 0.75, 0.5, 0.25$), its constant by Sneddon's penny and Westergaard's tunnel limits; G. R. Irwin, J. Appl. Mech. 29 (1962) 651-654; A. E. Green and I. N. Sneddon, Proc. Camb. Phil. Soc. 46 (1950) 159-163: NOT read, from memory |
| Near-tip displacement fields of modes I, II (plane strain, $\kappa = 3 - 4 \nu$) and III, unit K | verified | ADR 0605 D5, arithmetic 3 (equilibrium, traction-free faces, normalisation, Rice's J of each unit field `1/E'`, `1/E'`, $1/(2 \mu)$); M. L. Williams, J. Appl. Mech. 24 (1957) 109-114; T. L. Anderson, Fracture Mechanics, 3rd ed., 2005, chapter 2: NOT read, from memory |
| Interaction integral $M = \int (\sigma_{ij} u^{\mathrm{aux}}_{i,k} + \sigma^{\mathrm{aux}}_{ij} u_{i,k} - \sigma_{ij} \varepsilon^{\mathrm{aux}}_{ij} \delta_{jk}) q_{k,j} dV$, $K_{\mathrm{I}} = E' M_{\mathrm{I}}/2$, $K_{\mathrm{II}} = E' M_{\mathrm{II}}/2$, $K_{\mathrm{III}} = \mu M_{\mathrm{III}}$ | derived, cited | ADR 0605 D5; J. F. Yau, S. S. Wang and H. T. Corten, J. Appl. Mech. 47 (1980) 335-341; B. Moran and C. F. Shih, Eng. Fract. Mech. 27 (1987) 615-642: NOT read, from memory |
| Inclined centre crack, infinite plate: $K_{\mathrm{I}} = \sigma \sqrt{\pi a} \cos^2 \beta$, $K_{\mathrm{II}} = \sigma \sqrt{\pi a} \sin \beta \cos \beta$; antiplane $K_{\mathrm{III}} = \tau \sqrt{\pi a}$ | derived (I, III and the rotation), cited (II) | ADR 0605 D6; Westergaard (ADR 0547 reference) for I; the Westergaard function of mode II from memory; III derived from $w = (\tau/\mu) \operatorname{Im} \sqrt{z^2 - a^2}$ |
| Finite-body effects: the elliptical block (semi-axes `9 a`, $H = 10 a$) about `1.4e-3`; the plate $W = 10 a$ for II and III within `1 %` | estimate | ADR 0605 D4, D6: estimates, not references |
| Measured: elliptical crack $b/a = 0.5$ (87 663 unknowns) ring-3 K within `8.4e-3` of the hat-averaged Irwin form at all 13 stations, rings 2 to 4 within `2.2e-3`, $K(0)/K(\pi/2)$ within `5.7e-3`; coarse `3.3e-2`; uniform-expansion residual $\le 2.7 \times 10^{-14}$; inclined crack ($\beta = 30^\circ$) K_I `5.7e-3`, K_II `6.2e-3`, K_III `1.3e-16`; antiplane K_III `3.9e-3` | computed | ADR 0605 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
