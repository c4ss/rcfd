# ADR 0275: Assembly, displacement constraints, nodal forces and the linear static solve; patch test

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-5 of the structural solver, basic tier. Size M.

## Goal

Assemble the global stiffness of a `SolidModel` into a `CsrMatrix` in a deterministic order, impose displacement constraints per
node and component (zero or not, so rollers and symmetry planes work) by symmetric elimination, apply nodal forces, solve with
the direct solver of ADR 0274 (CG with Jacobi as the alternative), and recover reactions. This is the first ADR that computes a
displacement field.

## Dependencies

ADR 0271, ADR 0273, ADR 0274; existing roadmap items: F-025.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. The stiffness of a SolidModel is assembled into a CsrMatrix
deterministically (bit-identical for 1 and 8 threads) and is symmetric within 1e-13 of its norm; displacement constraints per
node and component, zero or not, are imposed by symmetric elimination and the reactions recovered. Patch test (Tet4, Hex8,
Tet10; a box whose interior nodes are displaced by a registered seed; boundary displacements of a linear field with all six
strains non-zero): interior displacements within 1e-10 of the field, relative to its maximum. Uniaxial tension by nodal forces
with statically determinate constraints: elongation F L/(E A) and lateral contraction nu times the axial strain within 1e-10.
The reactions balance the applied forces and their moment within 1e-10; a model that leaves a rigid-body motion free is refused
with a message saying so, a constraint on a missing node with its id.

## Reference to check against

Closed forms: a linear displacement field is an exact solution for any conforming element (the patch test, Irons; Taylor, Simo,
Zienkiewicz and Chan, Int. J. Numer. Meth. Engng 22 (1986) 39, from memory); uniaxial Hooke's law.

## Files

`crates/rcfd-fem/src/{assemble.rs, constraint.rs, solve.rs, loads.rs}` (in `loads.rs` the nodal force only),
`crates/rcfd-fem/tests/sb_static.rs`.

## What could make it miss

Detecting an under-constrained model from a small pivot alone is fragile (a threshold registered from the stiffness scale is
needed); the patch-test limit of 1e-10 on Tet10 with a direct solve of a moderately conditioned matrix.
