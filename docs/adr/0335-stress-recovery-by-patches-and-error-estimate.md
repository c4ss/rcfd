# ADR 0335: Stress recovery by element patches and an energy-norm error estimate

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-25 of the structural solver, advanced tier. Size S.

## Goal

How far is the stress from converged? Recover nodal stresses by a least-squares fit of the integration-point values over the patch
of elements around each node, and from the difference to the raw stress an element and a global error in the energy norm, a
relative error, and a refinement indicator per element as data. No mesh is changed.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-376, F-378.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A linear displacement field gives the exact stress and a zero
estimate within 1e-10. On the manufactured solution of F-378 for Hex8, Tet10 and Hex20: the effectivity index (estimate over true
error in the energy norm) lies in [0.8, 1.2] on the three finest meshes and its distance from 1 does not grow under refinement;
the recovered stress of Hex8 converges in L2 at an observed order >= 1.7 where the raw stress shows >= 0.9. Plate with a small
circular hole under tension: the recovered peak stress is within 2 percent of 3 sigma on the mesh on which the averaged nodal
stress of F-376 is recorded, and the tenth of the elements with the largest indicator contains the elements at the hole edge.
Boundary and corner nodes take the patch of the nearest interior node by the registered rule; the result is bit-identical for 1
and 8 threads; a patch with too few points is reported, not solved.

## Reference to check against

The manufactured solution of ADR 0279 (true error known) and Kirsch's hole. Method: Zienkiewicz and Zhu, Int. J. Numer. Meth.
Engng 33 (1992) 1331 and 1365 (from memory; the effectivity is measured, not assumed).

## Files

`crates/rcfd-fem/src/estimate.rs`, `crates/rcfd-fem/tests/sa_estimate.rs`.

## What could make it miss

Superconvergence of the patch recovery is not guaranteed on tetrahedra: the effectivity band is a draft. Shells and beams are not
covered. Adaptation is not planned (ADR 0310).
