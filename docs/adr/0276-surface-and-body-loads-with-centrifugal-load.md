# ADR 0276: Surface and body loads: traction, pressure, gravity, centrifugal load of a rotating part, body force field

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-6 of the structural solver, basic tier. Size M.

## Goal

Consistent nodal loads `f = int N^T t dS` and `f = int N^T b dV` for the three elements: a traction vector and a pressure on a
surface set, gravity, the centrifugal body force `rho omega^2 r_perp` of a part rotating about a given axis at constant speed
(the dominant blade load of a fan), and a body force given at the nodes or by a function (used by the manufactured solution of
ADR 0279). Pressure is positive when it presses on the solid.

## Dependencies

ADR 0275; existing roadmap items: none.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Consistent nodal loads for surface traction, pressure
(positive pressing on the solid), gravity, the centrifugal load rho omega^2 r_perp about a given axis, and a body force field,
for Tet4, Hex8 and Tet10. Resultants on a box: a uniform pressure on the closed surface gives zero force and moment within 1e-12
p A L; the traction resultant is t A, gravity rho V g and the centrifugal resultant rho omega^2 V r_c (r_c the centroid's
distance from the axis), each within 1e-12. Closed forms with nu = 0, displacements only: hanging bar, u(x) = rho g (L x -
x^2/2)/E: Tet10 nodal error <= 1e-9 relative, Hex8 and Tet4 tip displacement within 1 percent at 8 elements along; rotating
radial bar clamped on the axis: tip displacement rho omega^2 L^3/(3 E) within 1 percent (Tet10 at 8, Hex8 at 16 elements along);
thin rotating ring (thickness/radius 0.02): radial displacement rho omega^2 r^3/E within 2 percent.

## Reference to check against

Closed forms, all derivable in a few lines: 1-D equilibrium `d sigma/dx + b = 0` with `b = rho g` and `b = rho omega^2 r`; the
thin-ring hoop stress `rho omega^2 r^2` (membrane equilibrium).

## Files

`crates/rcfd-fem/src/loads.rs`, `crates/rcfd-fem/tests/sb_loads.rs`.

## What could make it miss

The consistent load of a uniform pressure on a quadratic triangle is zero at the corners and one third at each mid-edge node:
correct, but a resultant-only check would not catch a wrong distribution, hence the displacement checks. The ring needs curved
geometry (ADR 0272 is not a dependency: the test builds its ring with the hexahedron builder and a coordinate map). Spin
softening and stress stiffening are NOT included (intermediate tier).
