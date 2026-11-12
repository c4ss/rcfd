# ADR 0327: Follower pressure, arc-length continuation, and modes about a nonlinear equilibrium

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-17 of the structural solver, advanced tier. Size M.

## Goal

Three additions to nonlinear statics, each through the scaffold's driver extension points: pressure that follows the deformed
surface, with its load stiffness; continuation along the load-displacement path with a spherical arc-length constraint, through
limit points; and the modal analysis with the tangent stiffness of a converged nonlinear state (for a rotating part: the
pre-stress including untwist) in place of the linear pre-stress of ADR 0298.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-395, F-396, F-397.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Follower pressure: the load stiffness agrees with central
differences of the load within 1e-6. Thin ring (thickness/radius 0.02, nu = 0) under internal pressure up to the stretch 1.5: p =
E t (lambda^2 - 1)/(2 R) within 1 percent, the dead pressure on the same mesh being the registered control (off by the factor
lambda). Linear buckling of the ring under external pressure with the load stiffness: 3 E I/R^3 within 2 percent; without it the
registered control 4 E I/R^3 within 3 percent. Arc-length: a two-bar truss of solid bars (nu = 0, height h, bar length L0) is
followed to the apex displacement 2.2 h with P = E A (h - w)(2 h w - w^2)/L0^3 within 1e-6 at the registered points and the limit
load 2 E A h^3/(3 sqrt(3) L0^3) within 0.5 percent, load control failing at the first limit point (control). Modes about a state:
a clamped-clamped beam stretched by an end displacement has the tension frequencies of F-395 within 1 percent, and at a small
stretch the tangent modes equal the linear pre-stressed modes within 1e-4; for the rotating radial beam at 0.2 w_0 they equal
F-396 within 1e-4, and the difference at higher speed is recorded.

## Reference to check against

Closed forms: equilibrium of a thin ring in the deformed state; buckling of a ring under hydrostatic and under constant-direction
pressure (Timoshenko and Gere; the two factors 3 and 4 are recalled and re-derived); the two-bar truss with Green-Lagrange strain
(one line). Methods: Riks, Int. J. Solids Struct. 15 (1979) 529; Crisfield, Comput. Struct. 13 (1981) 55; the load stiffness of a
pressure (Hibbitt, 1979); all from memory, derived in the registration.

## Files

`crates/rcfd-fem/src/follower.rs`, `crates/rcfd-fem/src/arclength.rs`, `crates/rcfd-fem/tests/sa_follower.rs`,
`crates/rcfd-fem/tests/sa_arclength.rs`.

## What could make it miss

At the upper end of M; split: arc-length as its own S package. The load stiffness is unsymmetric on a free edge; the symmetric
part is used and the registration must say when that is exact. The two buckling factors of the ring are recalled.
