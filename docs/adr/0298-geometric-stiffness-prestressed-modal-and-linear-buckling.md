# ADR 0298: Geometric (initial-stress) stiffness of solid elements: pre-stressed modal analysis and linear buckling

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-8 of the structural solver, intermediate tier. Size M.

## Goal

From the stress of a linear static solution, the geometric stiffness `K_sigma = int G^T S G dV` of Tet4, Hex8, Tet10 (and Hex20
when present). With it: the modal analysis of a pre-stressed part (`K + K_sigma`), and linear buckling, the lowest positive load
factors of `(K + lambda K_sigma) x = 0`, for which `rcfd-linalg::eigen` gains an iteration on the factorisation of K (the
mass-matrix form of ADR 0283 does not apply: `K_sigma` is indefinite).

## Dependencies

ADR 0291; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-1, F-374, F-376, F-382, F-383.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. K_sigma is symmetric within 1e-13 of its norm, gives no force
for a rigid translation within 1e-12, and for a uniform uniaxial stress s and a linear displacement field w gives x^T K_sigma x =
s V |grad w . e|^2 within 1e-12. rcfd-linalg: for K = tridiagonal (2, -1), n = 200, and K_sigma = -I the ten lowest buckling
factors equal 4 sin^2(j pi/(2 (n + 1))) within 1e-10; a K_sigma with no negative direction returns no factor and says so.
Pre-stressed modes: a beam on knife-edge supports at its neutral axis (slenderness 20, nu = 0) under an axial tension stress s has
(omega_n^2 - omega_n,0^2) = (n pi/L)^2 s/rho within 1 percent for n = 1 and 2, and under compression the frequency falls by the
same amount. Buckling: the same beam in compression has the critical load pi^2 E I/L^2 reduced by the shear term (0.6 percent at
this slenderness, registered) within 1 percent, the second factor four times the first within 2 percent; a clamped-free column
pi^2 E I/(4 L^2) within 1 percent (Tet10 or Hex20).

## Reference to check against

Closed forms: Euler's column with Engesser's shear correction P_E/(1 + P_E/(kappa G A)); the beam under axial force, omega_n^2 =
(n pi/L)^4 E I/(rho A) + (n pi/L)^2 T/(rho A) (derivable from the beam equation with the term T w''). The geometric stiffness is
the second variation of the Green-Lagrange strain energy at fixed stress (Bathe, Finite Element Procedures, the total Lagrangian
formulation; Zienkiewicz and Taylor, vol. 2; from memory, derived in the registration).

## Files

`crates/rcfd-solid/src/{geometric.rs, buckling.rs}`, `crates/rcfd-linalg/src/eigen.rs` (appended function),
`crates/rcfd-solid/tests/si_geometric.rs`, `crates/rcfd-linalg/tests/si_buckling_eigen.rs`.

## What could make it miss

Knife-edge supports on a solid mesh need nodes on the neutral axis and leave a local stress concentration. The
frequency-difference check is built to cancel the discretisation error of the unstressed beam; if the two runs do not share the
mesh it does not. Beams and shells get no geometric stiffness here. Buckling under a load that has a fixed and a scaled part
(pre-load) is not included.
