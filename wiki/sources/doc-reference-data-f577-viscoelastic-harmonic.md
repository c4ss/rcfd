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

# Reference data: F-577 the viscoelastic material in the direct harmonic solve (ADR 0491)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-577 the viscoelastic material in the direct harmonic solve (ADR 0491)" (lines 1993-2004) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-577 viscoelastic material in the direct harmonic solve (ADR 0491).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-577. Every formula used is derived in ADR 0491 (arithmetic 1 to 9) or re-used from ADR 0485 and ADR 0448; the references of the checks are closed forms and the 1-D chain of linear elements solved in the test. The material constants are inputs (the values of ADR 0485 V1), not data.

| Item | Status | Source |
|---|---|---|
| Steady harmonic stress of the isotropic viscoelastic solid $\sigma^{*} = 2 G^{*} \operatorname{dev} \varepsilon^{*} + K^{*} \operatorname{tr} \varepsilon^{*} 1$, so the element stiffness is $(G^{*}/G_0) K^G + (K^{*}/K_0) K^K$ with $K^G$, $K^K$ the stiffness of $2 G_0 I_{\mathrm{dev}}$ and $K_0 1 \otimes 1$ (exact, the stiffness being linear in the elasticity matrix); for equal weights `E*/E_0` times the base stiffness | derived | the correspondence principle of linear viscoelasticity, from the steady state of the hereditary law (ADR 0485 arithmetic 2, ADR 0491 arithmetic 1); textbook statement (Tschoegl 1989, Ferry 1980): NOT read, from memory |
| Clamped rod with an end force $F$: $u(L) = F \tan(kL)/(A\, \mathrm{Mod}^{*} k)$, $k = \omega \sqrt{\rho/\mathrm{Mod}^{*}}$; the rod with the lateral motion held is in uniaxial strain, modulus `K* + 4 G*/3` ($= E^{*}$ for $\nu = 0$ and equal weights), the shear rod `G*` | derived | ADR 0491 arithmetic 2 |
| Discretisation error of the linear element with consistent mass near the first resonance, $2 (kh)^2/(24 \eta)$ at the peak ($kh = (\pi/2)/n$); `2.47e-3` (rod A), `2.65e-3` (B), `1.60e-3` (C) at 40 elements, second order | derived, computed | ADR 0491 arithmetic 4; probe `f577-scratch/probe.py` (numpy, no rcfd code), reproduced by the test |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
