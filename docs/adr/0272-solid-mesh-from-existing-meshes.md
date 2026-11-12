# ADR 0272: Solid mesh from the existing meshes: polyhedral mesh, tetrahedral mesher output, patches as surface sets, curved edges

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-2 of the structural solver, basic tier. Size M.

## Goal

Build a `SolidMesh` from what the repository already produces: an `rcfd_mesh::Mesh` whose cells are all tetrahedra or all
hexahedra (the node order of each element is recovered from the face lists), and a `rcfd_mesh::tetmesh::TetMesh`. Boundary
patches become surface sets with outward normals; cell zones become element sets. A caller-supplied projection moves the
mid-edge nodes of boundary edges onto a curved surface.

## Dependencies

ADR 0271; existing roadmap items: F-010, F-012, F-132.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A SolidMesh built from box_mesh (hexahedra), from the
annulus generator (hexahedra with curved faces) and from the tetrahedral mesher's output of a box surface has the node and
element counts of its source, element volumes summing to the source volume within 1e-12 relative (tetrahedra, planar hexahedra)
or 1e-3 (annulus, registered from the chord error), one surface set per patch whose area equals the patch area within 1e-12 and
whose face normals point along the source's outward area vectors; a prism, pyramid or general polyhedral cell is refused naming
the cell. With mid-edge nodes projected onto the cylinder walls of a quarter annulus the projected nodes lie on their radius
within 1e-14 and the volume error of the quadratic tetrahedra falls at an observed order of at least 3 under refinement.

## Reference to check against

Closed forms: volume of a box and of an annulus sector, `pi (b^2 - a^2) h / 4`. No published benchmark.

## Files

`crates/rcfd-fem/src/convert.rs`, `crates/rcfd-fem/tests/sb_convert.rs`.

## What could make it miss

Recovering the hexahedron node order from face lists on warped cells; the orientation convention of `TetMesh`; quadratic
elements inverted by a projection on a coarse curved mesh (must be refused, not repaired). The Gmsh route is exercised only in
ADR 0287, because `rcfd-fem` does not depend on `rcfd-io`.
