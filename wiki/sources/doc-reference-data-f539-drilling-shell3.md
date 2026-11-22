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

# Reference data: F-539 drilling-unknown membranes for Shell3: ANDES OPT and Allman's triangle (ADR 0466)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-539 drilling-unknown membranes for Shell3: ANDES OPT and Allman's triangle (ADR 0466)" (lines 1701-1715) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-539 drilling-unknown membranes for Shell3 (ADR 0466).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-539; every item is cited from memory. Allman's edge field and the basic stiffness are derived in ADR 0466 (arithmetic 1, 4); the higher-order stiffness of the ANDES element and its parameters `alpha_b = 3/2`, `beta_0 = max((1 - 4 nu^2)/2, 0.01)`, `beta_1 .. beta_9 = 1, 2, 1, 0, 1, -1, -1, -1, -2` are taken as recalled, once, and verified by the property they were designed for: exact in-plane pure bending of a rectangle cut into two triangles for any aspect ratio (check BE1, measured `3.7e-14`).

| Item | Status | Source |
|---|---|---|
| Allman's triangle: the edge-normal displacement quadratic with the mid-edge excess `l (theta_j - theta_i)/8`; the spurious mode of equal vertex rotations | derived | D. J. Allman, Comput. Struct. 19 (1984) 1-8: NOT read; ADR 0466 arithmetic 1, 2 |
| Basic stiffness of the free formulation: a constant stress lumped on the edge field with the excess times `alpha_b`, `K_b = A Bbar(alpha_b)^T A_m Bbar(alpha_b)` | derived | P. G. Bergan and C. A. Felippa, Comput. Methods Appl. Mech. Engrg. 50 (1985) 25-69: NOT read; ADR 0466 arithmetic 4 |
| ANDES higher-order stiffness `(3/4) beta_0 A sum_mid Q^T T_e^T A_m T_e Q` on the deviatoric corner rotations, the natural-strain matrices `Q_1, Q_2, Q_3` of `beta_1 .. beta_9`; the OPT parameters | from memory | C. A. Felippa and C. Militello, Finite Elem. Anal. Des. 12 (1992) 189-201; C. A. Felippa, Comput. Methods Appl. Mech. Engrg. 192 (2003) 2125-2168: NOT read; verified by BE1 (the factor `A/3` in place of `3A/4` gives 0.861) |
| Timoshenko cantilever tip `P L^3/(3 E I) + P L/(kappa G A)`, `kappa = 5/6`; end couple `M L^2/(2 E I)` | derived | beam theory; the clamped two-dimensional strip within `1.5e-4` of it by refinement (ADR 0466 arithmetic 6) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
