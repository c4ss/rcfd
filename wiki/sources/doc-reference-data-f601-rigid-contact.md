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

# Reference data: F-601 rigid sphere and cylinder targets, Hex20 and Tet faces in contact (ADR 0540)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-601 rigid sphere and cylinder targets, Hex20 and Tet faces in contact (ADR 0540)" (lines 2182-2196) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-601 rigid sphere and cylinder contact targets (ADR 0540).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-601. The closed forms below are cited from memory (K. L. Johnson, Contact Mechanics, Cambridge University Press, 1985, chapters 2 to 4: NOT read, chapter numbers as recalled); the checks of ADR 0540 use them in their half-space idealisation. Flamant's field is checked against itself (equilibrium, the free surface, the resultant: CL0) before it is used.

| Item | Status | Source |
|---|---|---|
| Flamant's line load `P` (per length) on a half-space: `sigma_ij = -2 P x_i x_j z / (pi r^4)` in the `x`-`z` plane (`r^2 = x^2 + z^2`, `z` into the body), radial; measured: equilibrium `1.9e-8` of `P/r^2` (central differences), surface traction `0`, resultant `4.2e-6` of `P` | derived, cited | ADR 0540 CL0; Johnson chapter 2: NOT read, from memory |
| Hertz line contact (a cylinder of radius `R` on a half-space, load `P` per length): half-width `b = sqrt(4 P R / (pi E*))`, peak `p_0 = 2 P / (pi b)`, `1/E* = (1 - nu^2)/E` (rigid cylinder); measured on the plane-strain slice: `b` within `1.06e-2`, `p_0` within `9.5e-3` | derived, cited | ADR 0540 CL1, CL2; Johnson chapter 4: NOT read, from memory |
| Hertz point contact (a rigid sphere of radius `R`, force `F`): radius `a = (3 F R / (4 E*))^(1/3)`, peak `p_0 = 3 F / (2 pi a^2)`, approach `delta = a^2 / R`; measured (H-fast mesh): radius `5.9e-3`, peak `3.9e-3`, approach `1.1e-2` | derived, cited | ADR 0540 HS; Johnson chapter 4: NOT read, from memory; the same forms as ADR 0507 a3 |
| The rigid sphere's gap `|x - c| - R`, normal `(x - c)/|x - c|`, second derivative `(I - n n^T)/|x - c|`; the cylinder's the same with the axis component removed (`(I - e e^T - n n^T)/d`) | derived | ADR 0540 D2; checked by T1s, T1c against central differences (`2.4e-10`, `1.7e-10`) |
| The share of a uniform pressure at a corner node of a flat Hex20 face (2 x 2 serendipity, 3 x 3 Gauss): `-1/12` of the face's total (negative: a node contactor cannot carry it, hence the Gauss-point contactor) | derived | ADR 0540 NG |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
