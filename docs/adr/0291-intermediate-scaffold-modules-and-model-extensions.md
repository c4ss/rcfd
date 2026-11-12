# ADR 0291: Scaffold of the intermediate tier: module layout, model extension data, element-trait extension points

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-1 of the structural solver, intermediate tier. Size S.

## Goal

Prepare the shared files once, so that every later package of the tier fills a file of its own and no two packages of a wave edit
the same file. Declare the new modules of `rcfd-solid` (section 2.2 of ADR 0290) and of `rcfd-post` as stubs; add the element
kinds `Hex20`, `Beam2` and `Shell4` and the per-block property data (Hex8 formulation, beam section, shell thickness); add the
constraint and support data (linear constraint, rigid set, cyclic pair, elastic support); give the `Element` trait the defaulted
entry points `geometric_stiffness`, `internal_force` and `tangent`; add one generic assembly hook that turns a per-element
symmetric matrix into a `CsrMatrix`; admit the VTK cell types 3, 9 and 25 in the output mapping. Everything new is refused with a
message naming it until its package lands. No numerics.

## Dependencies

none in this list; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): F-370, F-372, F-374, F-377.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. The modules, element kinds, property data, constraint and
support data and the three defaulted element entry points of ADR 0290 section 2 exist in rcfd-solid and rcfd-post; every test of
the basic tier (the sb_ files of rcfd-solid, rcfd-linalg, rcfd-io, rcfd-post and rcfd-api) compiles and passes unedited, and a
registered set of basic models (patch test, cantilever, bar modes) gives displacements, stresses and frequencies with the same
bits as before the change; the generic assembly hook called with the element stiffness reproduces assemble::stiffness bit for bit
for 1 and 8 threads; each new element kind, formulation, constraint and support variant is refused by the static solve with an
error naming the variant and the set, never ignored; the baseline hashes are unchanged.

## Reference to check against

No physics: the reference is the basic tier itself (bit identity) and the refusals. The model for a declare-once scaffold is ADR
0271.

## Files

`crates/rcfd-solid/src/{lib.rs, mesh.rs, model.rs, element.rs, assemble.rs}` (additions only), one-line stubs of `hex20.rs`,
`hex8x.rs`, `beam.rs`, `shell.rs`, `mpc.rs`, `cyclic.rs`, `geometric.rs`, `buckling.rs`, `rotating.rs`, `nonlinear.rs`,
`transient.rs`, `harmonic.rs`; `crates/rcfd-post/src/lib.rs` (two `pub mod` lines), stubs `solid_dynamic.rs`, `solid_fsi.rs`, the
kind-to-cell-type mapping in `solid_output.rs`; `crates/rcfd-io/src/vtk.rs` only if the finite-element writer of ADR 0278
restricts the cell types; `crates/rcfd-solid/tests/si_scaffold.rs`.

## What could make it miss

The basic tier's types are not written yet. If its tests build `SolidModel` or the element blocks by struct literals, a new field
breaks them and house rule 2 forbids editing them: the new data then go into a side structure passed to the new entry points,
which is uglier. The lead should ask the implementer of ADR 0271 to build the model through constructors (section 2.4 of ADR
0290).
