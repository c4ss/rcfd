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

# Reference data: F-578 geometrically nonlinear shells in the Newton driver (ADR 0490)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-578 geometrically nonlinear shells in the Newton driver (ADR 0490)" (lines 2055-2071) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-578 geometrically nonlinear shells (ADR 0490).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-578; the papers below are cited from memory for the names and the published forms, NOT read and not checked. Every formula used is derived in ADR 0490 (D1, D2, arithmetic 1 to 6) from first principles; the references of the checks are closed forms (the Euler-Bernoulli arc under a pure end moment, the St. Venant-Kirchhoff thin ring) and the linear Shell4 and Shell3 of the repository.

| Item | Status | Source |
|---|---|---|
| Corotational splitting: element frame from the current nodes, deformational translations and rotations, the linear element as the local response, the projector of the rigid motions | cited | C. C. Rankin and B. Nour-Omid, Comput. Struct. 30 (1988) 257; C. A. Felippa and B. Haugen, Comput. Methods Appl. Mech. Engrg. 194 (2005) 2285: NOT read, from memory |
| Energy form of the corotated element $\Pi = (1/2) d^T K_r d$, force and tangent as its exact derivatives (symmetric tangent with additive rotation parameters); corotated flat facet shells | cited | J.-M. Battini and C. Pacoste, Comput. Methods Appl. Mech. Engrg. 191 (2002) 1755; C. Pacoste, Comput. Methods Appl. Mech. Engrg. 156 (1998) 75: NOT read, from memory |
| Additive total rotation vector, Rodrigues formula $R = I + (\sin t/t)[\theta]_x + ((1 - \cos t)/t^2)[\theta]_x^2$, its differential singular at $t = 2 \pi k$ (determinant $2 (1 - \cos t)/t^2$) | derived | ADR 0490 D1; vector-like parametrisation after A. Ibrahimbegovic, F. Frey and I. Kozar, Int. J. Numer. Methods Engrg. 38 (1995) 3653: NOT read, from memory |
| Logarithm $\phi = (t/\sin t) v$ with $v$ the axial vector of the skew part, $\cos t = (\operatorname{tr} R - 1)/2$; Taylor series of `sin t/t`, $(1 - \cos t)/t^2$, $\arcsin(x)/x$ | derived | ADR 0490 D2, arithmetic 1 |
| Second-order forward differentiation (value, gradient, Hessian) | derived | the chain rule; hyper-dual numbers after J. A. Fike and J. J. Alonso, AIAA paper 2011-886: NOT read, from memory |
| Total Lagrangian Green-strain membrane of the bilinear / linear interpolation; with $\nu = 0$ the thin ring $p = E t (\lambda^2 - 1)/(2 R)$, with the facet polygon $1/\cos(\pi/N)$ | derived | ADR 0490 D0, arithmetic 6; ADR 0487 arithmetic 6 |
| End-moment cantilever: circular arc of radius `E I/M`, tip $(\rho \sin \phi, \rho (1 - \cos \phi))$, $\phi = M L/(E I)$, full circle at $M = 2 \pi E I/L$; the facet chain $e = \phi \sin(\phi/2)/3072$ (16 elements), largest `1.18e-3` | derived | ADR 0490 D5, arithmetic 5; parameters $L = 12, b = 1, t = 0.1, E = 1.2 \times 10^{6}$ after K. Y. Sze, X. H. Liu and S. H. Lo, Finite Elem. Anal. Des. 40 (2004) 1551: NOT read, from memory (only the parameters) |
| Measured: roll-up tip error `1.1868e-3` (Shell4, Shell3, Shell4 ANDES), the tip back at the root within `5e-15 L`, tip rotation within `7e-15`; cylinder `+5.357e-4` (Shell4), `+5.395e-4` (Shell3) to the stretch 1.51 | computed | ADR 0490 addendum 1 |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
