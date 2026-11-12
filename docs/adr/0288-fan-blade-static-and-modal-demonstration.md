# ADR 0288: Demonstration on a fan blade: static response to centrifugal and pressure load, and the modes at rest; user page

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-18 of the structural solver, basic tier. Size M.

## Goal

Bring the tier to the target of fan breakdown item E5: one blade of the parametric fan geometry (`rcfd_mesh::fan`) as a solid
clamped at the hub, under the centrifugal load at a stated speed and a given pressure distribution, with its lowest modes at
rest. There is no measured or published reference for this blade, so the checks are consistency and mesh convergence, plus the
flat-plate limit against closed forms; the result is a demonstration, not a validation. Also the user page
`docs/structural-solver.md`.

## Dependencies

ADR 0272, ADR 0276, ADR 0277, ADR 0284, ADR 0287; existing roadmap items: F-132.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. One blade of the parametric fan geometry, meshed in
quadratic tetrahedra and clamped at the hub: under the centrifugal load at a registered speed the reactions equal rho omega^2 V
r_c within 1e-8 and under a uniform pressure the integrated pressure force within 1e-8; the strain energy equals half the work
of the loads within 1e-8; between two meshes (the second with at least twice the elements) the tip displacement and the first
natural frequency change by less than 5 percent, both numbers recorded; as the flat-plate limit, an untwisted rectangular
cantilever plate of the blade's span, mean chord and thickness has its first bending frequency between the Euler-Bernoulli beam
value and that value divided by sqrt(1 - nu^2), with a 1 percent margin on each side. The page docs/structural-solver.md states
the scope, the limits and that nothing is validated against a measured blade.

## Reference to check against

Closed forms only for the limits: resultant of the centrifugal body force, Clapeyron's theorem, and the beam and wide-plate
(cylindrical bending, `E/(1 - nu^2)`) bounds of the first bending frequency. No reference value exists for the blade itself
(docs/fan-project-breakdown.md, part G).

## Files

`crates/rcfd-fem/tests/sb_fan_blade.rs` (or `crates/rcfd-api/tests/` if the Gmsh or tetrahedral-mesher route needs `rcfd-io`),
`docs/structural-solver.md`, a short section in `docs/fan-project-breakdown.md` (E5 status text).

## What could make it miss

The tetrahedral mesher on a thin twisted blade (quality known misses of F-132; slivers are tolerated less by a structural
element than by a finite-volume cell); a thin blade in solid elements needs several elements through the thickness or quadratic
ones, which may exceed the direct solver. If the blade cannot be meshed, the flat-plate limit alone is delivered and the item is
partial.
