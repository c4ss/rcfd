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

# Reference data: F-355 second-order immersed forcing in transient PISO: bounded weights (ADR 0254)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-355 second-order immersed forcing in transient PISO: bounded weights (ADR 0254)" (lines 1480-1488) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-355 second-order immersed forcing in transient PISO (ADR 0254).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Opt-in (`ImmersedForcing::build_second_order_bounded`); `build` and `build_second_order` are unchanged.

| Item | Status | What was read (source) |
|---|---|---|
| Ghost-cell extrapolation near a boundary point close to a grid node has large weights of alternating sign and can destabilise the update; remedy: a stencil with bounded weights through an image point in the fluid (Tseng and Ferziger, J. Comput. Phys. 192 (2003) 593-623; Mittal et al., J. Comput. Phys. 227 (2008) 4825-4852) | cited from memory, not read for this work | the formula used (the wall value reflected through the value at the image point of the face, `2 u_G - ((1 - a) HbyA_c + a HbyA_d)`, error `xi_f^2 u''` plus the linear interpolation) is derived in ADR 0254 amendment 1; measured on circular Couette: order 1.947, face-flux reconstruction error 7.99e-4 at N 96 with ratio 3.86 |
| DFG 2D-2 level-6 values | read | as the F-219 row above (`St` 0.30188, max `C_D` 3.2200, max `C_L` 0.9859); measured on M2 with the bounded form in ADR 0254 addendum 1: 0.30112, 3.20930, 0.97593 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[dfg-cylinder-benchmark]]
