# ADR 0278: VTK output of a solid result: finite-element cells with point and cell data

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-8 of the structural solver, basic tier. Size S.

## Goal

The existing `.vtu` writer writes the polyhedral finite-volume mesh with cell data only. Add, beside it and without changing it,
a writer of finite-element cells from plain arrays (so `rcfd-io` needs no new dependency), and in `rcfd-post` the function that
writes a `SolidResult`: displacement, nodal von Mises and principal stresses, element stress components, reactions. This ADR
also adds the `rcfd-solid` dependency of `rcfd-post` and declares its three new modules, so that ADRs 0285 and 0286 touch only
their own file.

## Dependencies

ADR 0277; existing roadmap items: F-070.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. rcfd-io gains a writer of an unstructured grid of finite
elements from plain arrays (VTK cell types 10, 12 and 24, point data and cell data); the existing writers are unchanged and
their tests pass unedited. rcfd-post writes a SolidResult through it: displacement at the nodes, nodal von Mises and principal
stresses, the six stress components per element, reactions. A test parses the file back: counts, offsets and cell types are
consistent, every value has the bits that were written, and a box under uniform strain read back has displacement eps x at every
node within 1e-15; a field of the wrong length is refused by name.

## Reference to check against

The public VTK file-format description (Kitware) for the cell type numbers and the node order of each type; the repository's
existing writer is the model for the XML layout. The node order of the quadratic tetrahedron is to be confirmed from that public
document by the implementer (recalled here, UNVERIFIED).

## Files

`crates/rcfd-io/src/vtk.rs` (appended functions only), `crates/rcfd-io/tests/sb_vtk_elements.rs`, `crates/rcfd-post/Cargo.toml`,
`crates/rcfd-post/src/lib.rs` (three `pub mod` lines), `crates/rcfd-post/src/ solid_output.rs`, stubs `solid_loads.rs` and
`solid_coupling.rs`, `crates/rcfd-post/tests/sb_solid_output.rs`.

## What could make it miss

No reader other than the test's own parser checks the file: that a viewer shows it correctly stays the maintainer's visual
check. Mid-edge node order of Tet10 differing between the element library and VTK.
