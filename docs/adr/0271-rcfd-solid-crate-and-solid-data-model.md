# ADR 0271: Crate rcfd-solid: the data model of a solid model and the structured builders

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-1 of the structural solver, basic tier. Size M.

## Goal

Create the crate `rcfd-solid` (depends on `rcfd-core`, `rcfd-mesh`, `rcfd-linalg` only) with the data model that the whole tier
shares, as validated data without any solver: `SolidMesh` (nodes, element blocks of kind Tet4, Hex8, Tet10, node, element and
surface sets), `DofMap`, `Material` (isotropic linear elastic), `Constraint`, `Load`, `SolidModel`, `SolidResult`. Add the
builders that need nothing else: a box of hexahedra, the same box in tetrahedra (six per hexahedron, conforming), and the
upgrade of linear to quadratic tetrahedra by mid-edge nodes. `lib.rs` declares every module of the tier (section 2.3 of ADR
0270) so that each later ADR fills its own file and none edits `lib.rs`.

## Dependencies

none in this list; existing roadmap items: F-001, F-010, F-012.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A new crate rcfd-solid (dependencies rcfd-core, rcfd-mesh,
rcfd-linalg only) holds the solid data model; the workspace graph test passes with the one registered line for the new crate,
the architecture page lists it, and nothing existing changes (baseline hashes unchanged). On a box of n^3 hexahedra and on its
six-tetrahedra split, for n = 1, 2, 5 with unequal edge lengths: every element has a positive Jacobian at every node, the
element volumes sum to the box volume within 1e-12 relative, the area of each of the six face sets equals the box face within
1e-12, and every interior face is shared by exactly two elements; the quadratic upgrade has (nodes + edges) nodes and each
mid-edge node is the mean of its two ends within 1e-15; an inverted element, a node repeated in an element, an empty set, E <= 0
and a Poisson ratio outside (-1, 0.5) are each refused with an error naming the element or the key.

## Reference to check against

Geometry only: volume and face areas of a box, the edge count of a structured grid (derivable). No published benchmark.

## Files

`Cargo.toml` (members, workspace.dependencies), `Cargo.lock` (the new workspace member only), `crates/rcfd-solid/Cargo.toml`,
`crates/rcfd-solid/src/{lib.rs, mesh.rs, model.rs, builders.rs}` and one-line stubs of the other modules,
`crates/rcfd-solid/tests/sb_model.rs`, `docs/architecture.md`, and ONE line of the existing test
`crates/rcfd-cli/tests/workspace_graph.rs` (the layer of the new crate; needs the maintainer, decision M1).

## What could make it miss

The edit of the existing graph test is not approved (the crate then panics that test). A data model too narrow for the next
tier: three unknowns per node hard-coded instead of going through `DofMap`, or loads as closures that cannot be read from a
file.
