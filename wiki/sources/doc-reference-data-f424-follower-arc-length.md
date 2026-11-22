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

# Reference data: F-424 follower pressure, arc-length continuation and modes about a nonlinear state (ADR 0480)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-424 follower pressure, arc-length continuation and modes about a nonlinear state (ADR 0480)" (lines 1946-1962) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-424 follower pressure, arc-length continuation (ADR 0480).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-424; the books and papers below are cited from memory for the names and the published forms. Every formula used is derived in ADR 0480 (arithmetic 1 to 7); the references of the checks are closed forms, a thick-ring ODE integrated in plain Python (`f424-scratch/ring_ode.py`) and a probe of the solid two-bar truss (`f424-scratch/zz_probe_f424.rs`, `probe.log`).

| Item | Status | Source |
|---|---|---|
| Follower pressure on the current face `f_a = -p int N_a (x_u x x_v) du dv` and its load stiffness `K_p = -p int N_a [N_b,u (e_j x x_v) + N_b,v (x_u x e_j)]`; the antisymmetric part an edge term of the loaded surface | derived | linearisation of the pressure's virtual work, ADR 0480 arithmetic 1 and 3; H. D. Hibbitt, "Some follower forces and load stiffness", Int. J. Numer. Meth. Engng 14 (1979) 937 (the reference itself uncertain); J. Bonet and R. D. Wood, Nonlinear Continuum Mechanics for Finite Element Analysis, Cambridge 1997, chapter 6: NOT read, from memory |
| Thin ring under follower pressure, SVK, `nu = 0`: `p = E t (lambda^2 - 1)/(2 R)`; dead pressure `p = lambda E t (lambda^2 - 1)/(2 R)` | derived | hoop equilibrium on the deformed radius, ADR 0480 arithmetic 2 |
| Thick ring, plane, SVK, `nu = 0`: `d(R P_rR)/dR = P_tt`, bore traction `-p r_i/R_i` (follower) or `-p` (dead); thin-law departure `+1.01e-2` at `lambda -> 1`, `+0.67e-2` at `lambda = 1.5`, `t/R = 0.02` | computed | RK4, 2000 steps, `f424-scratch/ring_ode.py` (ADR 0480 arithmetic 2) |
| Ring buckling under external pressure: `p_cr = (n^2 - 1) E I/R^3` (hydrostatic, follower), `n^2 E I/R^3` (fixed direction); `3` and `4 E I/R^3` at `n = 2` | derived | inextensional ring energy to second order, ADR 0480 arithmetic 3; S. P. Timoshenko and J. M. Gere, Theory of Elastic Stability, McGraw-Hill 1961, chapter 7: NOT read, from memory |
| Two-bar truss of Green-Lagrange bars with SVK stress, exact at any depth: `P = E A (h - w)(2 h w - w^2)/L0^3`, limit points `w = h (1 -+ 1/sqrt 3)`, `P_max = 2 E A h^3/(3 sqrt 3 L0^3)` | derived | ADR 0480 arithmetic 4; M. A. Crisfield, Non-linear Finite Element Analysis of Solids and Structures, vol. 1, Wiley 1991, chapter 1: NOT read, from memory |
| Bipyramidal Tet4 bar: the homogeneous state is an exact FE equilibrium with effective area `A = s^2/3`; probe residuals `2.1e-12 .. 7.2e-12 P_max` at `w/h = 0.05, 0.4226, 1, 1.5, 2.2` | derived, computed | ADR 0480 arithmetic 5; `f424-scratch/zz_probe_f424.rs`, `probe.log` |
| Spherical arc-length constraint `|Du|^2 + c Dlambda^2 = Ds^2`, the quadratic for the load-factor correction, the root closer in angle to the previous increment | cited | M. A. Crisfield, "A fast incremental/iterative solution procedure that handles snap-through", Comput. Struct. 13 (1981) 55; E. Riks, "An incremental approach to the solution of snapping and buckling problems", Int. J. Solids Struct. 15 (1979) 529: NOT read, from memory (ADR 0480 D4) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
