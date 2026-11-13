# ADR 0423: F-512 registration: beam results in the solid output (rotations and moment reactions as point data, Beam2 as VTK line cells, section forces as cell data)

Status: **REGISTERED 2026-11-13, written before any code of F-512 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-512 was written, compiled or run: the files listed under "What was read" were read; no
arithmetic beyond counting array lengths was needed. Results go in the addenda at the end; nothing above the addenda is changed
after a run.

Parent: [ADR 0417](0417-f391-registration-six-unknowns-and-beam.md) (F-391: six unknowns per node, Beam2, `beam::section_forces`;
its limit L1 and remaining question 1 are this item), [ADR 0278](0278-vtk-output-of-solid-results.md) (F-377: the solid `.vtu`
writer). Roadmap item F-512 (deps F-391, F-377). Size S. Number 0423 given by the lead. Branch `f512-beam-solid-output`, worktree
`w49i-work/f512`, scratch `w49i-work/f512-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the hash tables or of a golden file; no new dependency, no unsafe code;
clean-room.

## What F-512 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. rcfd_post::solid_output writes a model with Beam2 elements: displacement and rotation point arrays, reaction forces and moments, Beam2 as VTK line cells and the section forces as cell data; a model without beams writes the bytes it wrote before; the arrays read back equal the result within 1e-15.

Clauses: (a1) displacement and rotation point arrays; (a2) reaction forces and moments; (a3) Beam2 as VTK line cells; (a4) the
section forces as cell data; (a5) a model without beams writes the bytes it wrote before; (a6) the arrays read back equal the
result within 1e-15.

## What was read

- `docs/adr/0417-f391-registration-six-unknowns-and-beam.md` (D1 the `DofMap`: 6 unknowns at the nodes of Beam2 elements, 3
  elsewhere, components 3 to 5 the small rotations about x, y, z; D2 the reactions `K u - f` per unknown, moment reactions at the
  rotation equations, `beam::section_forces` with `(N, V_y, V_z, T, M_y, M_z)` in local axes, `start = -f_0`, `end = f_1`; D5 a
  node shared by a beam and a solid element is refused; D7 `recover` reaches Beam2 through the solid-element routines and is
  refused; L1 the output refusal).
- `docs/adr/0278-vtk-output-of-solid-results.md`, `crates/rcfd-post/src/solid_output.rs` (`solid_fields` refuses a node with
  other than three components; `vtk_from_fem(Beam2) = (VTK_LINE, [0, 1])` exists since ADR 0291 O1; `solid_vtu_string` writes
  through `rcfd_io::vtk::fe_vtu_string`, which prints every `f64` with `{:?}`, the shortest text that parses back to the same
  bits).
- `crates/rcfd-post/tests/sb_solid_output.rs` (the F-377 fixtures: Hex8, Tet4, Tet10 boxes under a uniform strain, and the test's
  own parser), `crates/rcfd-post/tests/si_scaffold_output.rs` (O1), `crates/rcfd-fem/tests/si_beam.rs` (how a beam model is
  built and solved), `crates/rcfd-fem/src/model.rs` (`SolidResult`, `DofMap`), `crates/rcfd-fem/src/mesh.rs` (element order is
  block after block), `crates/rcfd-fem/src/beam.rs` (`section_forces`, read only).
- `grep` over `crates/*/tests` and `crates/*/src` for `solid_fields`, `solid_vtu_string`, `write_solid_vtu` and the refusal text
  `the solid output writes 3`: the only callers are `sb_solid_output.rs` and `si_scaffold_output.rs`, and **no existing test
  asserts the six-unknown refusal**. There is therefore no test to retire: the refusal stays in `solid_fields` (D1 below), and
  check N3 exercises it as a new negative control. Nothing is ignored.

## Design

### D1. Which models take the beam path

`solid_vtu_string(model, result)` first tests whether any block of the mesh is Beam2. If none is, it runs the former body
unchanged (the same calls in the same order, so the same bytes: check B1). If one is, it calls the new `beam_vtu_string`. A
model with a Beam2 block and any block of another kind is refused by the beam path (`InvalidValue { path: "element kind" }` naming
the block and "a follow-up of ADR 0423"): the stress recovery of the solid blocks goes through `IntegrationPointField`, which is
refused on a Beam2 element (ADR 0417 D7), and splitting the model is a follow-up (L1). `solid_fields` and `SolidFields` are not
changed: on a beam model `solid_fields` keeps refusing a node with six components (its stress arrays do not apply to a beam).

### D2. The arrays of a beam result (`beam_fields`, `BeamFields`)

New public `beam_fields(model, result) -> Result<BeamFields>`, refusing (in order): a model with no Beam2 block or with a block of
another kind (D1); a result whose `DofMap` is not `model.dof_map()` (`FemError::FieldLength { key: "displacement" }`, as the solid
path names it). Arrays, flat and row-major, `n` nodes, `m` elements:

| field | per | content |
| --- | --- | --- |
| `displacement` | node, 3 | `result.displacement(node)[0..3]` |
| `rotation` | node, 3 | `result.displacement(node)[3..6]` (rad, small rotations about x, y, z); NaN at a node with three unknowns (a node of no beam element) |
| `reaction` | node, 3 | when the result carries reactions: the reactions of components 0 to 2 |
| `reaction_moment` | node, 3 | when the result carries reactions: the reactions of components 3 to 5; NaN at a node with three unknowns |
| `section_force_start` | element, 6 | `beam::section_forces(model, result.displacement_values())[e].start`: `(N, V_y, V_z, T, M_y, M_z)` in the element's local axes, at the section just after node 0 |
| `section_force_end` | element, 6 | the same, `.end`, at the section just before node 1 |

NaN marks a value that does not exist, as the solid output does for the stress of a node of no element (ADR 0278). Only the
values are copied: nothing is computed in the output that the result or `beam::section_forces` does not give.

### D3. The `.vtu` of a beam result

Registered names and types (the file's only arrays):

- Cells: every element as `VTK_LINE` (type 3, two points), through `vtk_from_fem(Beam2)` (identity order: VTK point 0 is the
  element's node 0, the start of its local x axis), blocks in mesh order, so cell `k` is element `k`.
- Point data, in this order: `displacement` (3), `rotation` (3), then, when the result carries reactions, `reaction` (3) and
  `reaction_moment` (3).
- Cell data, in this order: `section_force_start` (6), `section_force_end` (6).
- No stress, von Mises or principal arrays (a beam has no recovered stress tensor in rcfd).

`write_solid_vtu` writes `solid_vtu_string`, so it takes beam models without change. The module documentation lists the beam
arrays.

## Checks (limits fixed now)

The beam fixture F (fixed now): nodes `x_0 = (0, 0, 0)`, `x_1 = (1, 0, 0)`, `x_2 = (1, 1, 0)`, `x_3 = (1, 1, 1)` (m) and a node
`x_4 = (2, 2, 2)` of no element; elements `[0, 1]`, `[1, 2]` in a first Beam2 block and `[2, 3]` in a second Beam2 block, both with
the section `BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75)` of ADR 0417 and the orientation `v = (1, 2, 3)` (not
parallel to any element axis); material `isotropic_elastic(2.1e11, 0.3, 7850, 0)`; node 0 clamped (components 0 to 5 zero); node 4
with components 0 to 2 prescribed zero (a node of no element must have every component prescribed); at node 3 the force
`(1200, -2500, 3700)` N and the moment `(450, -300, 200)` N m. Solved by `solve_static` (default options). F' is the same
displacement without reactions (`SolidResult::new(dofs, displacement)`).

"Read back" is the test's own parser (the one of `sb_solid_output.rs`, copied) applied to `solid_vtu_string`, the tokens parsed
with `str::parse::<f64>`. A read-back error is `max |read - expected| / max |expected|` over the finite entries, the NaN entries
required to be NaN at the same positions (exact).

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| A1 | a1, a6 | F: `displacement` read back against `result.displacement(node)[0..3]`; `rotation` against `[3..6]` at nodes 0 to 3, NaN at node 4 | `1e-15` relative; NaN exact | `{:?}` round-trips: 0 |
| A2 | a2, a6 | F: `reaction` against the reactions of components 0 to 2 (all nodes); `reaction_moment` against components 3 to 5 at nodes 0 to 3, NaN at node 4; F': neither array present | `1e-15`; NaN and absence exact | 0 |
| A3 | a3 | F: `NumberOfCells = 3`, every type 3, offsets `2, 4, 6`, the points of cell `k` equal `mesh.element_nodes(k)` in order; `NumberOfPoints = 5` and the points read back equal the mesh nodes bit for bit | exact | D3 |
| A4 | a4, a6 | F: `section_force_start`, `section_force_end` (6 per cell) against `beam::section_forces` | `1e-15` relative each | 0 |
| A5 | D3 | F: point array names and components exactly `displacement 3, rotation 3, reaction 3, reaction_moment 3`; F': `displacement 3, rotation 3`; cell arrays exactly `section_force_start 6, section_force_end 6` | exact | D3 |
| A6 | a6 | F and F': `beam_fields` against the result bit for bit (the same quantities as A1, A2, A4, from the library struct); the file written by `write_solid_vtu` equals `solid_vtu_string` | exact | D2 |
| B1 | a5 | the six no-beam fixtures of ADR 0278 (the Hex8, Tet4 and Tet10 boxes of `sb_solid_output.rs`, each solved with reactions, and each as `SolidResult::new(dofs, displacement)` without): the FNV-1a 64-bit hash and the byte length of `solid_vtu_string` equal the values recorded at the base commit `3cf27f63` (procedure B0) | exact | D1: the former body |
| B2 | a5 | `sb_solid_output.rs` and `si_scaffold_output.rs` pass unedited | pass | D1 |

Procedure B0 (recording the bytes before): a test file `crates/rcfd-post/tests/si_beam_output_bytes.rs` with one test
`b0_no_beam_hashes_at_the_base` that builds the six fixtures and prints `(fixture, FNV-1a 64 hash, length)`, using only the API of
the base commit. It is committed and run once, before any library change of F-512, on the library of `3cf27f63`. Its printed values
are recorded in Addendum 0 (written and committed before the library change) and copied into a new test
`b1_no_beam_bytes_unchanged` of the same file, which is run after the library change. FNV-1a 64: offset basis
`0xcbf2_9ce4_8422_2325`, prime `0x100_0000_01b3`, over the UTF-8 bytes (public domain; Fowler, Noll, Vo).

Negative controls:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | a mesh with a Tet4 block (nodes 0 to 3) and a separate Beam2 block (nodes 4, 5), result `SolidResult::new(model.dof_map(), zeros)` | `solid_vtu_string` and `beam_fields` refused, the message naming `Beam2` and `ADR 0423` | exact |
| N2 | F with the result `SolidResult::new(DofMap::uniform(5, 3), zeros)` | refused naming `displacement` | exact |
| N3 | the Hex8 box of B1 with a result `SolidResult::new(DofMap::uniform(n, 6), zeros)` | refused as before by the solid path: the message contains `has 6 components; the solid output writes 3` (the F-377 refusal, now covered by a test) | exact |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | `rotation` copied from components 0 to 2 | A1 |
| MU2 | `reaction_moment` copied from components 0 to 2 | A2 |
| MU3 | `section_force_end` written from `.start` | A4 |
| MU4 | the rotation at a three-unknown node written as 0 instead of NaN | A1 |
| MU5 | the beam path taken for every model (the Beam2 test made always true) | B1 (and N3, B2) |
| MU6 | the cell connectivity of the beam path written with the nodes reversed | A3 |
| MU7 | the `DofMap` equality check of D2 removed | N2 |

## Decision rule

- **done:** every check A1 to A6, B1, B2 and N1 to N3 met as registered, every mutation caught, no amendment after a run.
- **partial:** a registered check missed, a mutation not caught, or a check met only after an amendment made after its run.
- **todo:** the beam path does not build.

## Limits (not covered)

- L1. A model mixing Beam2 with solid (or shell) blocks is refused by the output (D1): writing its stress arrays needs the stress
  recovery on the solid blocks of a mixed model, a follow-up.
- L2. Section forces are written at the two end sections of each element only (the element's own output); no interpolation
  along the element, no global-axis transform of the section forces, no local frame vectors written.
- L3. The output of the other paths' results (modal shapes, transient frames) of beam models is not exercised; they produce a
  `SolidResult` with the same `DofMap`, which the writer takes the same way, but no check here.
- L4. Shell4 output is not changed (no shell element exists yet).

## Open questions taken

1. Rotation and moment at a node with three unknowns: NaN (as for the stress of a node of no element in ADR 0278), rather than
   0 or a refusal. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. Section forces as two six-component cell arrays (start, end) in local axes, rather than twelve scalars or global axes: taken on
   the maintainer's standing instruction of 2026-11-11 (take the recommendation).
3. Mixed beam-solid models refused (L1) rather than written without stress: taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation).

## Addendum 0: procedure B0 (the bytes before), 2026-11-13

Run once on the library of `3cf27f63` (`cargo test -p rcfd-post --release --test si_beam_output_bytes`, before any library change
of F-512). Recorded values, copied into `b1_no_beam_bytes_unchanged`:

| fixture | FNV-1a 64 | length (bytes) |
| --- | --- | --- |
| Hex8 reactions | `0x63fed7daa56f1a63` | 15571 |
| Hex8 no reactions | `0x989f73686c43913e` | 13987 |
| Tet4 reactions | `0xe741f54ce312ad4d` | 20588 |
| Tet4 no reactions | `0xf0a06ff6536f969c` | 19006 |
| Tet10 reactions | `0x65e38c3b684cf06c` | 68654 |
| Tet10 no reactions | `0x526bf4d74d0598c9` | 62488 |

No limit, check or design text above is changed.

## Amendment 1: the set-up of N3, 2026-11-13 (before any run of N3)

Found on reading `DofMap` while writing the test, before N3 was compiled or run: with `DofMap::uniform(n, 6)` the result has
`6 n` unknowns, so `solid_fields` refuses it at its first check (the length, naming `displacement`) and never reaches the
component check that N3 is meant to cover. Corrected set-up: the result's numbering is `DofMap::from_counts([6, 0, 3, ..., 3])`
(`3 n` unknowns, node 0 with six, node 1 with none), which passes the length check; expected: refused with the message containing
`node 0 has 6 components; the solid output writes 3`. The limit (exact) and every other check are unchanged.
