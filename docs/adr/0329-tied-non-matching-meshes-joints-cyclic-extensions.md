# ADR 0329: Tied non-matching meshes, shell and beam joints to solids, non-matching cyclic boundaries and nodes on the axis

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-19 of the structural solver, advanced tier. Size M.

## Goal

Parts meshed apart must be joined. A node-to-surface search (bucket grid, projection onto the face) that turns each node of the
fine side into a linear constraint on the face it lies on (ADR 0296); the joints of a shell edge and of a beam end to a solid,
where the rotation follows the solid's surface nodes; and the same interpolation for cyclic boundaries that do not match, with the
rule for nodes on the axis by harmonic index. This is bonded contact; the search is reused by ADR 0330.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-391, F-392, F-393, F-394.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. With matching meshes the tied model equals the merged one
within 1e-10. Flat non-matching interfaces 2:3 and 3:5 under uniaxial stress: far-end displacement within 0.1 percent and
interface stress within the registered 3 percent (a one-sided node-to-surface tie does not pass the patch test exactly), the error
falling with an observed order >= 1 under refinement. A cantilever cut in two and tied 2:3 (Tet10 to Hex20): tip deflection within
1 percent of the closed form. The constraint forces of a tie have zero resultant and moment within 1e-10. A node farther from
every face than the registered tolerance is refused with its id. Joints: a beam continuing a solid bar and a shell continuing a
solid plate give the tip deflection of the one-piece cantilever within 1 and 2 percent. Cyclic: the ring sector of F-394 with a
2:3 boundary has its frequencies within 0.5 percent of the matching sector; a solid disc with nodes on the axis, spinning (index
0): centre stress (3 + nu) rho Omega^2 b^2/8 within 1 percent; a rod on the axis at index 1 has the bending frequency of the beam
within 1 percent, and at index 2 its axis nodes are held.

## Reference to check against

The merged or matching model is the reference of the equivalences; closed forms: the cantilever, the rotating solid disc in plane
stress (Timoshenko and Goodier), the beam frequency. That a one-pass tie fails the patch test is known (Taylor and Papadopoulos,
1991, from memory); the limit is registered from an estimate of the gap error.

## Files

`crates/rcfd-solid/src/tie.rs`, `crates/rcfd-solid/src/cyclic.rs` (additions), `crates/rcfd-solid/tests/sa_tie.rs`.

## What could make it miss

At the upper end of M; split: ties first, joints and cyclic extensions second. Curved interfaces leave gaps and overlaps that a
mortar method would treat properly; mortar ties are not planned. Many constraints fill the reduced matrix (reported).
