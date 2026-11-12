# ADR 0296: Linear multi-point constraints by a transformation, rigid sets with a reference point, elastic supports

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-6 of the structural solver, intermediate tier. Size M.

## Goal

Replace the reduction of the system by a transformation `u = T q + g` whenever a constraint other than a prescribed displacement
is present (the elimination of the basic tier stays for models that have none, bit for bit). On it: the general linear constraint
`sum a_i u_i = b`, a rigid set of nodes that follows a reference point with six unknowns (small rotation) and takes a force and a
moment there, and elastic supports (a spring to ground per node and direction, and a foundation stiffness per area on a surface
set). This is what cyclic symmetry (ADR 0297) is built on.

## Dependencies

ADR 0291; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-1, F-374, F-375.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A model with prescribed displacements only gives the bits of
the basic tier; the same constraints written as linear constraints give the same displacements within 1e-12. The reduced matrix is
symmetric within 1e-13 and the constraint forces do no work on any admissible motion within 1e-12. Skew roller: a block in
uniaxial stress along a direction 30 degrees off the axes, held by the constraint u.n = 0 on its inclined faces, has the rotated
uniaxial solution within 1e-10. Periodic pair u(high) - u(low) = eps L on a box with displaced interior nodes: uniform strain
within 1e-10. Rigid set on the end face of a bar: an axial force at the reference point gives F L/(E A) within 1e-10 (nu = 0), a
moment the tip rotation M L/(E I) within 1 percent (Tet10, slenderness 20), and the constraint forces have the resultant and
moment applied within 1e-10. Supports: a bar on an end spring k has the tip displacement F (1/k + L/(E A)) within 1e-10; a block
on a foundation k_f under pressure p settles by p/k_f within 1e-10. A redundant constraint is dropped with a report, a
contradictory one and a chain that closes on itself are refused naming the constraints.

## Reference to check against

Closed forms: uniaxial Hooke's law in rotated axes, springs in series, beam bending by an end moment. Method: master-slave
elimination written as a congruence transformation (Cook, Malkus, Plesha and Witt, Concepts and Applications of Finite Element
Analysis, the chapter on constraints; from memory); derivable from the principle of virtual work.

## Files

`crates/rcfd-fem/src/mpc.rs`, `crates/rcfd-fem/src/constraint.rs` (the dispatch to the transformation),
`crates/rcfd-fem/tests/si_mpc.rs`.

## What could make it miss

`T^T K T` fills the matrix where a rigid set couples many nodes (the direct solver's envelope grows; reported, not hidden).
Choosing the eliminated unknown of each constraint so that chains resolve; detecting redundancy needs a rank decision with a
registered threshold. Tied non-matching meshes are NOT included (advanced, with bonded contact).
