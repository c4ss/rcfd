# ADR 0294: Six unknowns per node: rotations in the DofMap, nodal moments, and a two-node shear-flexible beam element

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-4 of the structural solver, intermediate tier. Size M.

## Goal

Make the unknown numbering carry three or six unknowns per node (six where a beam or shell is attached), with prescribed
rotations, nodal moments, moment reactions and rotations in the output, and exercise it with the simplest element that needs it: a
straight two-node beam in space with axial, torsional, bending and shear stiffness (the exact stiffness of a prismatic Timoshenko
beam), consistent mass, section data given as numbers (A, I_y, I_z, J, shear factors, orientation vector) with helpers for
rectangular and circular sections, and section forces as results.

## Dependencies

ADR 0291; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-1, F-374, F-375, F-376, F-377, F-383.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Nodes carry three or six unknowns; a model without structural
elements gives the bits it gave before. Beam, ONE element, any orientation in space (registered seed): tip deflection P L^3/(3 E
I) + P L/(kappa G A), tip rotation P L^2/(2 E I), twist T L/(G J) and elongation F L/(E A) within 1e-10, section forces at the
root equal to the applied loads within 1e-10, reactions (forces and moments) balancing the loads within 1e-10; a rotation of the
whole model leaves the results unchanged within 1e-10. Quarter-circle arch of radius R clamped at one end, 32 elements, slender:
deflection under the tip load pi P R^3/(4 E I) within 0.5 percent. Free beam: six zero frequencies below 1e-6 of the seventh.
Pinned-pinned beam, 20 elements, slenderness 50: f_n = (n pi/L)^2 sqrt(E I/(rho A))/(2 pi), n = 1, 2, 3, within 0.5, 1 and 2
percent after the registered shear and rotary-inertia correction. A beam node shared with a solid element is refused (no
rotational stiffness from the solid) with the node id.

## Reference to check against

Closed forms: Euler-Bernoulli and Timoshenko beam theory; Castigliano's theorem for the arch (bending energy only, derivable in a
few lines); the pinned-pinned frequencies. Method: the exact stiffness matrix of a prismatic shear-flexible beam (Przemieniecki,
Theory of Matrix Structural Analysis, 1968; from memory, re-derived from the beam equations in the registration).

## Files

`crates/rcfd-solid/src/beam.rs`, and the generalisation in `model.rs` (`DofMap`), `assemble.rs`, `constraint.rs`, `loads.rs`,
`mass.rs`, `recover.rs`; `crates/rcfd-post/src/solid_output.rs` (rotations as point data, line cells);
`crates/rcfd-solid/tests/si_beam.rs`.

## What could make it miss

It edits many basic files, so it runs alone among packages that touch them (wave 3). ADR 0270 promises that no code outside
`DofMap` assumes three unknowns per node; if that did not hold, this package finds out and grows. Joining a beam to a solid needs
the rigid set of ADR 0296 (a follow-up check, not in this package). The torsion constant of a rectangle is a series: it is an
input.
