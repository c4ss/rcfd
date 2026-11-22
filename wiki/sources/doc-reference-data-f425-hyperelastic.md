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

# Reference data: F-425 hyperelastic materials with the mean-dilatation Hex8 (ADR 0541)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-425 hyperelastic materials with the mean-dilatation Hex8 (ADR 0541)" (lines 2169-2181) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-425 hyperelastic materials with mean-dilatation Hex8 (ADR 0541).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-425. The books and papers below are cited from memory for the energies, the element and the closed forms (volumes, pages and years as recalled, not checked; ADR 0541 open question 4). Every formula used is derived in ADR 0541 (arithmetic 1 to 5) and checked by plain numpy probes (`f425-scratch/probe_*.py` and their logs), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Compressible neo-Hookean and Mooney-Rivlin, isochoric-volumetric split: `W = c10 (Ibar1 - 3) + c01 (Ibar2 - 3) + (kappa/2)(J - 1)^2`, `Ibar1 = J^{-2/3} I1`, `Ibar2 = J^{-4/3} I2`, neo-Hookean `c10 = mu/2`, `c01 = 0`; `S = 2 dW/dC`, the tangent `4 d2W/dC2`; small-strain moduli `G = 2 (c10 + c01)`, `K = kappa`; `det C <= 0` refused | derived, cited | ADR 0541 arithmetic 1, D1; P. J. Flory, Trans. Faraday Soc. 57 (1961) 829-838; J. C. Simo, R. L. Taylor and K. S. Pister, Comput. Methods Appl. Mech. Engrg. 51 (1985) 177-208; G. A. Holzapfel, Nonlinear Solid Mechanics, Wiley, 2000, chapter 6; J. Bonet and R. D. Wood, Nonlinear Continuum Mechanics for Finite Element Analysis, Cambridge, 1997: NOT read, from memory |
| Mean dilatation: one volume ratio per element `J_bar = v / V`, the F-bar Hex8 of ADR 0506 (`F_bar = (J_bar / J)^{1/3} F`) as the element | derived, cited | ADR 0541 arithmetic 2, D5; J. C. Nagtegaal, D. M. Parks and J. R. Rice, Comput. Methods Appl. Mech. Engrg. 4 (1974) 153-177; Simo, Taylor and Pister (1985) above: NOT read, from memory |
| Incompressible closed forms: uniaxial nominal stress `mu (l - l^-2)` (neo-Hookean), `2 (c10 + c01 / l)(l - l^-2)` (Mooney-Rivlin), equibiaxial `mu (l - l^-5)`, simple shear `tau = mu gamma`, `N1 = mu gamma^2`; the compressibility correction solved from zero lateral nominal stress | derived, cited | ADR 0541 arithmetic 4, (a4); R. S. Rivlin, Phil. Trans. R. Soc. Lond. A 241 (1948) 379-397; L. R. G. Treloar, The Physics of Rubber Elasticity, 3rd ed., Oxford, 1975; R. W. Ogden, Non-linear Elastic Deformations, Ellis Horwood, 1984: NOT read, from memory |
| Thick sphere `B/A = 2` inflated to the inner stretch `l_a`: `p = mu int_{l_b}^{l_a} (l^-2 + l^-5) dl`, `l_b^3 = 1 + (A/B)^3 (l_a^3 - 1)`, closed form `(mu/2)((1/l_b^4 + 4/l_b) - (1/l_a^4 + 4/l_a))` | derived, cited | ADR 0541 D6, SP0; A. E. Green and W. Zerna, Theoretical Elasticity, Oxford, 1954; Ogden (1984) above: NOT read, from memory |
| Measured: tangent against central differences `<= 4.0e-9`; rotation `<= 3.2e-14`; uniaxial, equibiaxial `<= 1.7e-11` relative after the correction; simple shear `<= 7.2e-15`; sphere B-bar `n 6, m 8` at `l_a = 1.5` `2.61e-3`; small strain `4.7e-6` (neo-Hookean), `4.9e-6` (Mooney-Rivlin) against `1e-6` (known miss) | computed | ADR 0541 addendum 1 |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
