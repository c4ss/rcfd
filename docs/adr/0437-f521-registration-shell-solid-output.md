# ADR 0437: F-521 registration: shell results in the solid output (Shell4 as VTK quad cells, rotations and reaction moments as point data, the resultants and surface stresses as cell data)

Status: **REGISTERED 2026-11-14, written before any code of F-521 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-521 was written, compiled or run: the files listed under "What was read" were read; no
arithmetic beyond counting array lengths and the cell offsets of the fixtures was done. Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Parents: [ADR 0432](0432-f392-registration-flat-facet-shell.md) (F-392: Shell4, `rcfd_fem::shell::stresses`; its limit L1 and
remaining question 1 are this item), [ADR 0423](0423-f512-registration-beam-results-in-solid-output.md) (F-512: `beam_fields`,
the layout of the beam output, the byte test B0/B1), [ADR 0430](0430-f516-registration-beam-output-layout-refusal.md) (F-516:
the layout refusal naming the node and both counts). Roadmap item F-521 (deps F-392, F-512). Size S. Number 0437 given by the
lead. Branch `f521-shell-solid-output`, worktree `w49i-work/f521`, scratch `w49i-work/f521-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; each registered run made once;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the hash tables or of a golden file; no new dependency, no unsafe
code; clean-room. Another item (F-384) edits `crates/rcfd-post/src/solid_loads.rs`; this item does not touch that file.

## What F-521 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. rcfd_post::solid_output writes a model with Shell4 elements: displacement and rotation point arrays, reaction forces and moments, Shell4 as VTK quad cells and the membrane forces, moments, shear forces and top and bottom surface stresses of rcfd_fem::shell::stresses as cell data; a model without shells writes the bytes it wrote before; the arrays read back equal the result within 1e-15.

Clauses: (a1) displacement and rotation point arrays; (a2) reaction forces and moments; (a3) Shell4 as VTK quad cells; (a4) the
membrane forces, moments, shear forces and top and bottom surface stresses as cell data; (a5) a model without shells writes the
bytes it wrote before; (a6) the arrays read back equal the result within 1e-15; (a7) every limit not stated is registered before
the run (this document).

## What was read

- `crates/rcfd-post/src/solid_output.rs` (all): `vtk_from_fem(Shell4) = (VTK_QUAD, [0, 1, 2, 3])` exists already (ADR 0432);
  `beam_fields`, `BeamFields`, `beam_vtu_string`, `solid_vtu_string` (the beam test `has_beam` first, otherwise the former body),
  the layout refusals of ADR 0430 inside `beam_fields` (a node-count mismatch, then the first node whose count differs).
- `crates/rcfd-fem/src/shell.rs`: `ShellStresses { axes, membrane, bending, shear, top, bottom }` and
  `stresses(model, displacement) -> Vec<(element, ShellStresses)>` in element order for the Shell4 elements only, at the element
  centre, in the element's local axes; `check_shell_model` (a shell node shared with a solid element is refused; a shell node
  shared with a beam element is not). `crates/rcfd-fem/src/beam.rs`: `section_forces` returns one row per Beam2 element.
- ADRs 0423, 0430, 0432 (above); `crates/rcfd-post/tests/si_beam_output.rs` (the parser, the fixture, the checks A1 to A6, N1 to
  N3), `si_beam_output_bytes.rs` (B0/B1), `si_beam_layout.rs` (C1 to C5), `si_shell_output.rs` (check N2 of ADR 0432, which
  asserts that a shell result is refused: `node 0 has 6 components`), `si_scaffold_output.rs` (asserts only
  `vtk_from_fem(Shell4) == (9, [0, 1, 2, 3])`).
- `grep` over `crates/*/tests` and `crates/*/src` for `solid_vtu_string`, `write_solid_vtu`, `solid_fields`, `beam_fields`: the
  callers are the files above and `sb_solid_output.rs`; the only test that asserts the refusal of a shell result is
  `si_shell_output.rs`.

## Design

### D1. Which models take the shell path

`solid_vtu_string(model, result)` tests, in this order: any block Shell4 (new, `has_shell`): the shell path
`shell_vtu_string`; otherwise any block Beam2: the beam path, unchanged; otherwise the former body, unchanged. A model without a
Shell4 block therefore runs the code it ran before (checks B).

The shell path takes a model whose blocks are all Shell4 or Beam2 (both have six unknowns at their nodes, and a node may be
shared by a shell and a beam element: `check_shell_model` allows it). A block of another kind (a solid kind) is refused:
`InvalidValue { path: "element kind" }` naming the first such block, its kind, `Shell4` and `ADR 0437`. Reason: a node shared by
a shell and a solid is refused by the model (ADR 0432 D4), and the stress recovery of the solid blocks of a mixed model is a
follow-up as in ADR 0423 L1. Mixed Beam2 + Shell4 models are written (decision taken: it costs one NaN fill, below).

### D2. The arrays (`shell_fields`, `ShellFields`)

New public `shell_fields(model, result) -> Result<ShellFields>`. It refuses, in order: a model with no Shell4 block (`the model has
no Shell4 block; use solid_fields or beam_fields`); a block of a kind other than Shell4 and Beam2 (D1); a result whose
`DofMap` is not the model's, by exactly the refusals of ADR 0430 (the length of `displacement_values`, then the node count, then
the first node whose count differs, with the same texts), implemented as one private function used by `beam_fields` and
`shell_fields` so the two cannot differ; a Shell4 block without its thickness (the error of `shell::stresses`, naming the
block). Arrays, flat and row-major, `n` nodes, `m` elements (all elements of the mesh, in mesh order, so cell `k` is element `k`):

| field | per | content |
| --- | --- | --- |
| `displacement` | node, 3 | `result.displacement(node)[0..3]` |
| `rotation` | node, 3 | `[3..6]`, the small rotations about x, y, z; NaN at a node with three unknowns |
| `reaction` | node, 3 | when the result carries reactions: components 0 to 2 |
| `reaction_moment` | node, 3 | when the result carries reactions: components 3 to 5; NaN at a node with three unknowns |
| `membrane_force` | element, 3 | `ShellStresses::membrane` `(N_xx, N_yy, N_xy)`, N/m, local axes; NaN for a non-shell element |
| `moment` | element, 3 | `bending` `(M_xx, M_yy, M_xy)`, N m/m; NaN for a non-shell element |
| `shear_force` | element, 2 | `shear` `(Q_x, Q_y)`, N/m; NaN for a non-shell element |
| `stress_top` | element, 3 | `top` `(sigma_xx, sigma_yy, sigma_xy)` at `z = +t/2`, Pa; NaN for a non-shell element |
| `stress_bottom` | element, 3 | `bottom`, at `z = -t/2`; NaN for a non-shell element |
| `shell_axes` | element, 9 | `axes` `[e_1, e_2, e_3]` row by row, so the local components can be rotated to global; NaN for a non-shell element |
| `section_force_start`, `section_force_end` | element, 6 | only when the model has a Beam2 block: `beam::section_forces` as in ADR 0423; NaN for a non-beam element |

Only values are copied: nothing is computed in the output that `shell::stresses`, `beam::section_forces` or the result do not
give. NaN marks a value that does not exist (a rotation at a node of no shell or beam, a shell quantity on a beam cell, a section
force on a shell cell), as in ADRs 0278 and 0423. The values are those at the element centre in the element's local axes
(ADR 0432 arithmetic 9); no interpolation, no transform to global axes (L2).

### D3. The `.vtu` of a shell result

- Cells: every element through `vtk_from_fem`: Shell4 as `VTK_QUAD` (type 9, four points, the element's node order, which is
  VTK's quad order; identity map), Beam2 as `VTK_LINE` (type 3); blocks in mesh order.
- Point data in this order: `displacement` (3), `rotation` (3), then, when the result carries reactions, `reaction` (3),
  `reaction_moment` (3).
- Cell data in this order: `membrane_force` (3), `moment` (3), `shear_force` (2), `stress_top` (3), `stress_bottom` (3),
  `shell_axes` (9), then, only when the model has a Beam2 block, `section_force_start` (6), `section_force_end` (6).
- A shell-only model has no `section_force_*` arrays; a beam-only model is written by the beam path exactly as before.

`write_solid_vtu` writes `solid_vtu_string`, so it takes shell models without change. The module documentation lists the shell
arrays.

### D4. The retired row

`si_shell_output.rs` asserts that a shell result is refused (`node 0 has 6 components`), which is no longer true. By the standing
decision the test gets one `#[ignore = "record of ... before ADR 0437 (F-521) ..."]` line (nothing else of it is edited) and a
running copy `n2_shell_result_refused_by_the_solid_output_after_landed` is added in the new test file that asserts the new
behaviour on the same fixture (the output is written, and the displacement, rotation and the element 0 resultants read back).

## Checks (limits fixed now)

Shell fixture S (fixed now): a plane strip of 4 Shell4 elements in a tilted plane. With `e_a = (2, 1, 2)/3` and
`e_b = (-2, 2, 1)/3` (orthonormal), node `5 j + i` (i in 0..=4, j in 0..=1) is `0.25 i e_a + 0.1 j e_b` (m), plus a node 10 of no
element at `(1, 1, 1)`; elements `[i, i+1, i+6, i+5]` for `i = 0..3` in one Shell4 block with `BlockProperty::shell(0.01)`;
material `isotropic_elastic(2.1e11, 0.3, 7850, 0)`; nodes 0 and 5 clamped (components 0 to 5), node 10 with components 0 to 2
prescribed zero; at node 4 the force `(30, -40, 50)` N, at node 9 the moment `(3, -2, 1)` N m. Solved by `solve_static` (default
options). S' is the same displacement without reactions (`SolidResult::new(dofs, displacement)`). Unknowns: 10 nodes x 6 + 3 = 63.

Mixed fixture X (fixed now): the strip of S with a Beam2 block placed **first** (block 0: one element `[4, 11]`, node 11 at node
4 plus `(0.3, 0.2, -0.1)`, section `BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75)`, orientation `(1, 2, 3)`), block 1
the four shells, so element 0 is the beam and elements 1 to 4 are shells (shell element `i` of S is element `i + 1`). Node 12 of no
element replaces node 10 (at `(1, 1, 1)`, components 0 to 2 prescribed zero). Same material, clamps and loads.

"Read back" is the test's own parser (the one of `si_beam_output.rs`, copied) applied to `solid_vtu_string`, tokens parsed with
`str::parse::<f64>`. A read-back error is `max |read - expected| / max |expected|` over the finite entries, the NaN entries
required to be NaN at the same positions (exact).

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| S1 | a1, a6 | S: `displacement` against `result.displacement(node)[0..3]`, `rotation` against `[3..6]` at nodes 0 to 9, NaN at node 10; S' too | `1e-15` relative; NaN exact | `{:?}` round-trips: 0 |
| S2 | a2, a6 | S: `reaction`, `reaction_moment` against the result (components 0 to 2 and 3 to 5; NaN moment at node 10); S': neither array present | `1e-15`; NaN and absence exact | 0 |
| S3 | a3 | S: 11 points equal the mesh nodes bit for bit; `NumberOfCells = 4`, every type 9, offsets `4, 8, 12, 16`, the points of cell `k` equal `mesh.element_nodes(k)` in order | exact | D3 |
| S4 | a4, a6 | S and S': `membrane_force` (3), `moment` (3), `shear_force` (2), `stress_top` (3), `stress_bottom` (3), `shell_axes` (9), per cell, against `shell::stresses` | `1e-15` relative each | 0 |
| S4b | a4 | S: from the file's own arrays, `stress_top - stress_bottom` against `12 moment / t^2` (t = 0.01) per cell | `1e-12` relative to `max stress_top` | cancellation of terms of size N/t: a few ulps times the ratio of `N/t` to `6M/t^2`, below 1e-13 |
| S5 | D3 | S: point names and components exactly `displacement 3, rotation 3, reaction 3, reaction_moment 3`; S': the first two; cell names exactly `membrane_force 3, moment 3, shear_force 2, stress_top 3, stress_bottom 3, shell_axes 9` (no `section_force_*`) | exact | D3 |
| S6 | a6 | S and S': `shell_fields` against the result and `shell::stresses` bit for bit; the file of `write_solid_vtu` equals `solid_vtu_string` | exact | D2 |
| X1 | D1, D2, a3 | X: 5 cells, types `3, 9, 9, 9, 9`, offsets `2, 6, 10, 14, 18`, connectivity = element nodes; the six shell arrays all NaN on cell 0, equal to `shell::stresses` of elements 1 to 4 on cells 1 to 4 (`1e-15`); `section_force_start`, `section_force_end` equal to `beam::section_forces` on cell 0 (`1e-15`) and NaN on cells 1 to 4; cell names exactly the six shell arrays then `section_force_start 6, section_force_end 6`; point arrays as S with node 12 NaN in the rotation and in the reaction moment | exact / `1e-15` | D2 |
| B1 | a5 | the six no-beam, no-shell fixtures of ADR 0278: `si_beam_output_bytes.rs` run **unedited** | pass | D1: the former body |
| B1b | a5 | the beam frame fixture F of ADR 0423 (its model, solved with reactions, and without): FNV-1a 64 hash and byte length of `solid_vtu_string` equal the values recorded at the base commit `af0fd223` (procedure B0 below) | exact | D1: the beam path unchanged |
| B2 | a5 | `si_beam_output.rs`, `si_beam_layout.rs`, `sb_solid_output.rs`, `si_scaffold_output.rs` and the whole `rcfd-post` suite pass unedited, except `si_shell_output.rs` (D4: one ignore line) | pass | D1 |
| R1 | D4 | `n2_shell_result_refused_by_the_solid_output_after_landed` passes (the old N2 fixture is written and read back); the retired `n2_shell_result_refused_by_the_solid_output` is ignored with its text intact | pass | D4 |

Procedure B0 (the bytes before): a test `b0_no_shell_hashes_at_the_base` in the new file
`crates/rcfd-post/tests/si_shell_output_bytes.rs` builds the beam frame fixture F (copied from `si_beam_output.rs`) and prints
`(fixture, FNV-1a 64 hash, length)` for F with and without reactions, using only the API of the base commit. It is committed and
run once on the library of `af0fd223`, before any library change; its values are recorded in Addendum 0 (committed before the
library change) and copied into `b1b_no_shell_bytes_unchanged`, run after. FNV-1a 64: offset basis `0xcbf2_9ce4_8422_2325`, prime
`0x100_0000_01b3`.

Negative controls, including the layout refusals of ADR 0430 registered **now** (the lesson of ADR 0423, whose mutation MU7 was
caught only by a check added after its run):

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | a mesh with a Tet4 block (nodes 0 to 3) and a separate Shell4 block (nodes 4 to 7), result `SolidResult::new(model.dof_map(), zeros)` | `solid_vtu_string` and `shell_fields` refused, the message naming `Tet4`, `Shell4` and `ADR 0437` | exact |
| N2 | `shell_fields` on the Hex8 box of ADR 0278 and on the beam frame F | refused with `no Shell4 block` | exact |
| N3 | S with the result `SolidResult::new(DofMap::uniform(11, 3), zeros)` (33 unknowns against 63) | both entry points refused, equal text, containing `length 33` and `expected 63`, not `unknowns in the result and` | exact |
| N4 | S with `DofMap::from_counts([6,6,6,6,3,6,6,6,6,6,6])` (63 unknowns, node 4 with three, node 10 with six) | both entry points refused, equal text, containing `node 4 has 3 unknowns in the result and 6 in the model` and `displacement` | exact |
| N5 | S with `DofMap::from_counts([3,6,6,6,6,6,6,6,6,6,6])` (63 unknowns; first difference at node 0) | both refused, text containing `node 0 has 3 unknowns in the result and 6 in the model` | exact |
| N6 | S with `DofMap::uniform(9, 3)` plus zeros (27 unknowns, 9 nodes) | both refused, text containing `the result numbers 9 nodes and the model 11` | exact |
| N7 | S with its own layout (`from_counts` equal to the model's) and zeros | both entry points accept | exact |
| N8 | a Shell4 block without `BlockProperty::shell` (model of S without `set_block_property`), result `SolidResult::new(model.dof_map(), zeros)` | `solid_vtu_string` and `shell_fields` refused, the message naming `Shell property` | exact |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | `rotation` copied from components 0 to 2 | S1 |
| MU2 | `reaction_moment` copied from components 0 to 2 | S2 |
| MU3 | `stress_bottom` written from `top` | S4, S4b |
| MU4 | `shear_force` written from `bending` (the first two values) | S4 |
| MU5 | the shell path taken for every model (the `has_shell` test made always true) | B1 (and B1b, B2) |
| MU6 | the cell connectivity of Shell4 written reversed | S3 |
| MU7 | the layout check removed from `shell_fields` | N4, N5, N6 |
| MU8 | the NaN fill of a shell quantity on a beam cell written as 0 | X1 |
| MU9 | the shell rows placed by a running shell count instead of the element index (beam first) | X1 |
| MU10 | `has_beam` tested before `has_shell` in `solid_vtu_string` | X1 (refused), S unaffected |
| MU11 | the refusal of a solid block in the shell path removed | N1 |
| MU12 | `shell_axes` written as `e_1, e_1, e_3` | S4 |

## Decision rule

- **done:** every check S1 to S6, X1, B1, B1b, B2, R1 and N1 to N8 met as registered, every mutation caught by a check registered
  here, no amendment after a run.
- **partial:** a registered check missed, a mutation not caught, or a check met only after an amendment made after its run.
- **todo:** the shell path does not build.

## Limits (not covered)

- L1. A model mixing Shell4 or Beam2 with solid blocks is refused (D1); a shell-solid joint does not exist yet (ADR 0432).
- L2. Resultants and stresses at the element centre only, in local axes (written as `shell_axes` so that a reader can rotate
  them); no value at the Gauss points, no nodal averaging, no global-axis transform, no von Mises of the surface stress.
- L3. Results of the other paths (modal shapes) of shell models are not exercised; they carry the same `DofMap`.
- L4. Mid-surface only: no extrusion of the shell to its thickness for display.

## Open questions taken

1. Cell array names `membrane_force`, `moment`, `shear_force`, `stress_top`, `stress_bottom`, `shell_axes`, and `shell_axes` as an
   addition to the acceptance's list: taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. NaN (not 0) for a quantity that does not exist, as in ADRs 0278 and 0423: taken on the same instruction.
3. Mixed Beam2 + Shell4 written (both have six unknowns), solid blocks refused: taken on the same instruction.

## Addendum 0: procedure B0 (the bytes before)

Run once on the library of `af0fd223` (`cargo test -p rcfd-post --release --test si_shell_output_bytes`, before any library
change of F-521). Recorded values, copied into `b1b_no_shell_bytes_unchanged`:

| fixture | FNV-1a 64 | length (bytes) |
| --- | --- | --- |
| frame reactions | `0x34ff07d075a2933a` | 2637 |
| frame no reactions | `0xde67d0587843b447` | 2231 |

No limit, check or design text above is changed.

## Amendment 1: the set-up of N6, 2026-11-14 (written after the first run of `si_shell_output_vtk.rs`, before the corrected run of N6)

The first run of the layout test met N3, N4 and N5 and then failed at N6: a result with `DofMap::uniform(9, 3)` has 27
unknowns, the model 63, so the length test refuses it with the length text and never reaches the node-count test that N6 is
meant to cover (the same mistake as ADR 0423 Amendment 1, made here in the registration by not counting the unknowns of the
second fixture). Corrected set-up N6b: the result `DofMap::from_counts([6, 6, 6, 6, 6, 6, 6, 6, 6, 3, 3, 3])` (12 nodes, 63
unknowns), expected: both entry points refused with the same text containing `the result numbers 12 nodes and the model 11`.
The limit (exact) is unchanged. The registered N6 stays in the file as an ignored record (`n3_to_n7_layout_refusals`, one ignore
line); the corrected test `n3_to_n7_layout_refusals_amended` re-runs N3, N4, N5, N7 (met in the first run, repeated unchanged
because they sit in the same test) and N6b. By the decision rule the item is **partial** whatever N6b gives: a check was met only
after an amendment made after its run.
