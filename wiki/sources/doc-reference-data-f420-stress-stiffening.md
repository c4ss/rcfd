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

# Reference data: F-420 stress stiffening of beams and shells (ADR 0473)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-420 stress stiffening of beams and shells (ADR 0473)" (lines 1859-1876) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-420 stress stiffening of beams and shells (ADR 0473).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-420; the books and the paper below are cited from memory for the names and the published form. Every matrix is derived in ADR 0473 (arithmetic 1 to 5, D1 to D5) from the second variation of the strain energy under a constant pre-stress and from the kinetic energy of the motion perpendicular to the axis; the references of the checks are closed forms or the solid model of the existing solver (Hex20 for E1 to E3).

| Item | Status | Source |
|---|---|---|
| Euler loads `pi^2 E I/L^2` (pinned) and `pi^2 E I/(4 L^2)` (clamped-free) with the shear correction `P = P_E/(1 + P_E/(kappa G A))` (Engesser) | cited | S. P. Timoshenko and J. M. Gere, Theory of Elastic Stability, 2nd ed., McGraw-Hill 1961, ch. 2: NOT read, from memory |
| Taut-beam frequencies `w_n^2 = ((n pi/L)^4 E I + (n pi/L)^2 T)/(rho A)` of a pinned beam under tension `T` | derived | separation of variables of `E I w'''' - T w'' + rho A w_tt = 0` (ADR 0473 arithmetic 3) |
| Beam geometric stiffness `N/(30 L)[36, 3L, -36, 3L; ...]` per bending plane, `N/L` axial and `N I_p/(A L)` torsion terms, with the shear-flexible (Timoshenko) interpolation | derived | second variation of the axial strain under `N` (ADR 0473 arithmetic 1, D1); W. McGuire, R. H. Gallagher and R. D. Ziemian, Matrix Structural Analysis, 2nd ed., Wiley 2000, ch. 9: NOT read, from memory |
| Plate buckling `N_cr = 4 pi^2 D/a^2` (uniaxial) and `2 pi^2 D/a^2` (equal biaxial) of a simply supported square plate, `D = E t^3/(12 (1 - nu^2))` | cited | Timoshenko and Gere, op. cit., ch. 9: NOT read, from memory |
| Shell geometric stiffness `int N_ab (dN_i/dx_a)(dN_j/dx_b) dA` on the three translations, `N = t Lambda S Lambda^T` | derived | ADR 0473 D2 (second variation of the membrane strain) |
| Spin-softening mass `int rho (P u) . (P u) dV`, `P = I - a a^T`, beam section and shell rotary terms; the flap-lag identity `w_flap^2 - w_lag^2 = Omega^2 (1 - f_rot)` | derived | ADR 0473 arithmetic 4, D3 |
| Centrifugal load of a beam: the consistent nodal load of `rho A Omega^2 P (x - p)` and the section moment `-rho Omega^2 (I_z a_2 e_2 x a + I_y a_3 e_3 x a)` | derived | ADR 0473 arithmetic 5, amendment 1 (U6) |
| Stiffening slope of a rotating radial cantilever (Southwell coefficient) `Q = 1.193336` of the first flap mode | computed | numpy Rayleigh quotient, `f420-scratch/probe_rot.py` (ADR 0473 arithmetic 4, after ADR 0403); measured 1.19395 (R3) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
