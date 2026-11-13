# ADR 0383: F-484 registration: Hex20 face loads through the model path

Status: **REGISTERED 2026-11-13, written before any code of F-484 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-484 was written, compiled or run: the code listed under "What was read" was read and
the comparisons under "Arithmetic before this registration" were made by hand. Results go in the addendum at the end; nothing
above the addendum is changed after a run.

Parent: [ADR 0292](0292-quadratic-hexahedron-hex20.md) (F-389, the Hex20 element; its finding 1, its open question 1 and its
remaining question 1, option (a), from which this item comes). Roadmap item F-484 (depends on F-389). Size S. Branch
`f484-hex20-face-loads` from main e05871f6, worktree `w49i-work/f484`, scratch `w49i-work/f484-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited (the one permitted edit,
the maintainer's standing decision of 2026-11-13 for `si_scaffold.rs`, does not arise here: see "Existing tests"); no new
dependency, no unsafe code; clean-room.

## What F-484 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Load::Traction and Load::Pressure on surface sets of
> Hex20 faces give through nodal_load_vector the per-node forces of hex20::pressure_loads and hex20::traction_loads bit for
> bit; SolidMesh::face_area and face_area_vector of an 8-node face equal hex20::face_area and the 3 x 3 Gauss area vector
> within 1e-15 relative (no panic); every load vector and area of Tri3, Quad4 and Tri6 faces is bit-identical to before; the
> baseline hashes are unchanged.

## What was read (no run)

- `crates/rcfd-fem/src/loads.rs` (whole): `nodal_load_vector`, `add_face_load` (the generic face loop over `face_rule(n)` and
  `face_shape_functions(n, uv)`), `face_shape_functions` and `face_rule` with arms 3, 4, 6 and the refusal
  `face_count_error(n)` = "a face of {n} nodes (faces of 3, 4 or 6 nodes are supported)", `add_force`.
- `crates/rcfd-fem/src/hex20.rs` (whole): `face_shape_functions(uv) -> ([f64; 8], [[f64; 2]; 8])`, `face_rule() -> [([f64;
  2], f64); 9]`, `face_load`, `face_area`, `set_loads`, `pressure_loads`, `traction_loads`, `faces()`.
- `crates/rcfd-fem/src/mesh.rs` lines 772 to 861: `face_nodes`, `face_tangents` (arms 3, 4, 6, then `unreachable!`),
  `face_area`, `face_area_vector`; the constant `G` (line 451).
- `crates/rcfd-fem/src/model.rs` `add_load` (lines 683 to 700: a Traction or Pressure needs only an existing surface set and
  finite values; no element-kind check), `dof_map`, `DofMap::equation`.
- `crates/rcfd-fem/src/builders.rs` (`hex_box`, `tet_box`, `upgrade_to_tet10`, the surface sets of the box faces).
- `crates/rcfd-fem/tests/si_hex20.rs` (L1 to L3, the `Checks` helper), `crates/rcfd-fem/tests/si_scaffold.rs` (the refusal rows
  and the `LANDED` list; no row there is a Hex20 face load), ADR 0292 (D5, finding 1, open and remaining questions).
- `grep` over `crates/`: the strings "a face of 8 nodes" and "faces of 3, 4 or 6" are asserted only in `si_hex20.rs` line 807
  (L3); `SolidMesh::face_area` / `face_area_vector` are called in rcfd-fem by no library code other than `mesh.rs` itself.

## Design

### D1. The model path (loads.rs)

Add an arm `8 =>` to `loads::face_shape_functions` returning `hex20::face_shape_functions(uv)` as vectors, and an arm `8 =>`
to `loads::face_rule` returning `hex20::face_rule()` as a vector. The refusal of another count names the four supported counts
("faces of 3, 4, 6 or 8 nodes are supported"; the prefix "a face of {n} nodes" is kept). `add_face_load` is not changed: its
generic loop then runs on 8-node faces.

Why bit for bit (operation by operation, read from the two sources): with the rule and the functions of `hex20`, the loop of
`add_face_load` and the loop of `hex20::face_load` do the same floating-point operations in the same order: points and weights
in the same order (`j` outer, `i` inner, `w = W_i W_j`); the tangents `xu[i] += xa[i] * da[0]`, `xv[i] += xa[i] * da[1]` in
node order; `c = cross(xu, xv)`; pressure `[-p * c[0], -p * c[1], -p * c[2]]` (the closure of `pressure_loads`); traction `s =
norm(c)`, `[t[0] * s, t[1] * s, t[2] * s]` (the closure of `traction_loads`); `fa[i] += w * na * t[i]` evaluated as `(w *
na) * t[i]` in both. `hex20::face_load` also computes the point `xq`, which the two closures do not use. Assembly: `set_loads`
sums per node in face order into an accumulator starting at `+0.0` and the sums are then added by `add_force` to a vector of
`+0.0`; `nodal_load_vector` adds each face's forces in face order to the vector of `+0.0`. For a node of faces `f1, f2, ...`
both give `((0 + f1) + f2) + ...` and then, on the hex20 side, `0 + s`. In round-to-nearest `+0 + x = x` for every `x`
except `x = -0`, and `s` is never `-0`: it begins at `+0`, and a sum is `-0` only when both terms are `-0`. So `0 + s` equals `s`
in every bit. So the comparison is made through `nodal_load_vector` on both sides: the model with the load and no extra
forces, against the model without the load given `hex20::pressure_loads` (or `traction_loads`) as extra forces.

### D2. Face area and area vector (mesh.rs)

Add an arm `8 =>` to `SolidMesh::face_tangents`: for each `(uv, w)` of `hex20::face_rule()`, the derivatives of
`hex20::face_shape_functions(uv)`, the tangents summed in node order as in `hex20::face_area`, pushed as `(w, xu, xv)`. Then
`face_area` = `sum w |xu x xv|` performs the operations of `hex20::face_area` in the same order (the iterator sum starts at
`-0.0` or `+0.0`; both give the same bits when added to a positive term), and `face_area_vector` = `sum w (xu x xv)`. The
`unreachable!` stays for counts other than 3, 4, 6, 8 (no element kind has such a face). The 3, 4 and 6 arms are not touched.

Accuracy (first principles, the serendipity face of ADR 0292 D5, functions after Zienkiewicz and Taylor, The Finite Element
Method, vol. 1, cited from memory as in ADR 0292 and derived there): the face map has degree (2, 1) and (1, 2) terms, so
`x_u` has degree <= 1 in `u` and <= 2 in `v`, `x_v` the converse, and `x_u x x_v` degree <= 3 per direction: the 3 x 3 Gauss
rule (exact to degree 5 per direction) gives the exact area vector of any 8-node face. The area `int |x_u x x_v|` is exact
where `|x_u x x_v|` is constant (a parallelogram face with straight edges and mid-edge nodes at the midpoints) and is the 3 x 3
Gauss approximation otherwise, as `hex20::face_area`.

### Arithmetic before this registration (disclosed)

1. The operation comparison of D1 and D2, made by reading the two sources side by side (no run).
2. Closed form of the area vector of a planar quadrilateral with straight edges: `A = (x2 - x0) x (x3 - x1) / 2` (the
   cross product of the diagonals; for a parallelogram `|A|` is its area). Used by K1 on an affine mesh, where every face is a
   parallelogram with mid-edge nodes at the midpoints.
3. Divergence theorem: the area vectors of the faces of a closed surface sum to zero, so a uniform pressure on the whole
   boundary of a box gives zero total force (K2b); and with `sum_a N_a = 1` the total force of one set is `-p sum A_f` (K2a).

## Set-ups

- B: `hex_box([0, 0, 0], [0.3, 0.5, 0.7], [2, 2, 2])` upgraded with `upgrade_to_hex20` (flat faces; nodes shared between the
  faces of a set).
- C: `hex_box([1, 0, 0], [1, pi/2, 0.2], [2, 3, 1])` upgraded with `upgrade_to_hex20_mapped` and the map `(r, t, z) -> (r cos
  t, r sin t, z + 0.1 r sin t)` (curved faces; `dz'/dz = 1` and the in-plane map is the polar map with `r >= 1`, so no element is
  inverted).
- F: `hex_box([0, 0, 0], [1, 1, 1], [2, 1, 1])` upgraded with the affine map `x -> (x + 0.3 y + 0.1 z, 0.9 y + 0.2 z, 1.1 z +
  0.05 x)` (every face a parallelogram).
- Q (Quad4, Tri3, Tri6 controls): `hex_box([0, 0, 0], [1, 0.6, 0.4], [2, 2, 1])`, `tet_box([0, 0, 0], [1, 0.6, 0.4], [2, 2,
  1])` and `upgrade_to_tet10` of that `tet_box`, each with every node moved by the map of C applied to `(1 + x, y, z)` (a
  rebuild with the same blocks and sets), so that the arithmetic is not trivial.
- Sets: the six box-face surface sets `xmin` to `zmax` of each mesh. Loads: pressure `p = 1.3e5`, traction `t = (2.0e4,
  -3.5e4, 1.1e4)`. Material: steel `E = 2e11`, `nu = 0.3`, `rho = 7850` (needed only to build the model).

## Registered checks

Tests in the new file `crates/rcfd-fem/tests/si_hex20_model_loads.rs`, release profile.

### A. The acceptance

| Check | Clause | What | Limit |
|---|---|---|---|
| A1 | 1 | B and C, each of the six sets: `nodal_load_vector` of the model with `Load::pressure(set, p)` against `nodal_load_vector` of the model without loads with the extra forces `hex20::pressure_loads(mesh, set, p)`: entries whose bits differ | 0 of every entry (12 comparisons) |
| A2 | 1 | as A1 with `Load::traction(set, t)` and `hex20::traction_loads` | 0 (12 comparisons) |
| A3 | 2 | B and C, every local face of every element: `|SolidMesh::face_area - hex20::face_area| / hex20::face_area` | <= 1e-15 (max) |
| A4 | 2 | the same faces: `|face_area_vector - a_ref| / |a_ref|`, `a_ref = sum_q w_q x_u x x_v` computed in the test from `hex20::face_rule` and `hex20::face_shape_functions` | <= 1e-15 (max) |
| A5 | 2 | no panic: A3 and A4 complete on every 8-node face | as stated |
| A6 | 3 | Q (Quad4, Tri3, Tri6), each of the six sets, pressure and traction: `nodal_load_vector` against a transcription in the test of the code before this item (`add_face_load`, `face_shape_functions`, `face_rule` of loads.rs at e05871f6, arms 3, 4, 6), assembled into the vector by `DofMap::equation`: entries whose bits differ | 0 (36 comparisons) |
| A6c | 3 | control: A6 and A7 also run once on an archive copy of e05871f6 (the code before this item) with the same test file | 0 (both) |
| A7 | 3 | Q, every local face of every element: `face_area` and `face_area_vector` against a transcription of `face_tangents` at e05871f6: values whose bits differ | 0 |
| A8 | 4 | `cargo test -p rcfd-api --release --test baseline_hashes` (unedited) | passes |

### K. Closed forms and conservation

| Check | What | Limit | Estimate |
|---|---|---|---|
| K1 | F, every face: `|face_area_vector - (x2 - x0) x (x3 - x1) / 2| / |.|` and `|face_area - |(x2 - x0) x (x3 - x1)| / 2| / .` | <= 1e-14 | a few roundings of O(1) terms, 1e-16 each |
| K2a | B and C, each set, pressure through the model: `|sum_a f_a + p sum_f A_f| / (p sum_f |A_f|)` (`A_f` from `face_area_vector`) | <= 1e-13 | sums of up to ~60 terms; 1e-14 expected |
| K2b | B, `Load::pressure` on all six sets in one model: `|sum_a f_a| / (p sum_f |A_f|)` | <= 1e-13 | as K2a |

### N. Negative controls

| Check | What | Limit |
|---|---|---|
| N1 | the bit comparator of A1 sees one ulp: one entry of the reference moved by one ulp gives exactly 1 differing entry | 1 |
| N2 | `loads::face_shape_functions(5, ..)` and `loads::face_rule(5)` refused, the message contains "a face of 5 nodes" | as stated |
| N3 | A1 is not vacuous: in each pressure vector of B and C every node of the set has a non-zero force component | as stated |

### S. Existing tests

| Check | What | Limit |
|---|---|---|
| S1 | every other test of rcfd-fem, release, unedited | 0 failed, except `si_hex20::l1_to_l3_face_loads_and_uniaxial_tension`, whose L3 asserts the refusal this item retires (predicted; reported, not edited) |

### Mutations (scratch copy, not committed)

- M1 `loads::face_shape_functions` arm 8: the derivatives of nodes 4 and 5 swapped. Expected caught by A1, A2.
- M2 `loads::face_rule` arm 8: the points in the order `i` outer, `j` inner (same rule, other summation order). Expected
  caught by A1 or A2 (bit sensitivity), possibly not on every set.
- M3 `mesh::face_tangents` arm 8: the 2 x 2 rule of Quad4 instead of 3 x 3. Expected caught by A3, A4 (curved C).
- M4 `mesh::face_tangents` arm 8: `xu` and `xv` exchanged. Expected caught by A4, K1.
- M5 `mesh::face_tangents` arm 4: `G` multiplied by `1 + 1e-15`. Expected caught by A7 (A6 does not use `mesh.rs`).

## Existing tests

`si_hex20::l1_to_l3_face_loads_and_uniaxial_tension` (ADR 0292) asserts as L3 that `Load::pressure` on a Hex20 set is refused
with "a face of 8 nodes". This item's acceptance retires that refusal, so the test fails on L3 (L1 and L2 still pass). It is
not in `si_scaffold.rs`, so the maintainer's standing permission does not cover it, and it is not edited. The step that remains
for the maintainer: mark it `#[ignore = "record of ADR 0292: model-path pressure on Hex20 refused until ADR 0383"]` and add a
copy without L3 (the pattern of `si_scaffold.rs`). The `si_scaffold.rs` rows of ADR 0291 contain no Hex20 face load: none is
retired by this item.

## Decision rule

- done: A1 to A8, K1, K2a, K2b, N1 to N3 met; S1 as predicted (the one L3 failure, its record step left to the maintainer).
- partial: any clause of A missed or not run (A8 included), or another existing test fails.
- todo: the model path still refuses or panics on an 8-node face.

## Limits of this registration

- Only uniform pressure and uniform traction (the two `Load` variants); a traction field stays a per-node force from
  `hex20::traction_field_loads`.
- Mixed surface sets (Hex20 faces with faces of other kinds) are not checked separately; each face takes its own arm.
- The area of a curved 8-node face is the 3 x 3 Gauss approximation (as `hex20::face_area`), not an exact area.
- No solve is run: the acceptance is about the load vector and the areas.

## Addendum 1: results (2026-11-13)

Code: `crates/rcfd-fem/src/loads.rs` (the `8 =>` arms of `face_shape_functions` and `face_rule`, the refusal text "faces of 3,
4, 6 or 8 nodes are supported", doc comments), `crates/rcfd-fem/src/mesh.rs` (the `8 =>` arm of `face_tangents`, lines 837 to
853; the doc comment of `face_area`); tests `crates/rcfd-fem/tests/si_hex20_model_loads.rs`. `add_face_load`, the 3, 4 and 6
arms and every other line of both files are unchanged. All runs in the release profile.

### Checks

| Check | Limit | Measured | Met |
|---|---|---|---|
| A1 pressure, B and C, six sets each: differing entries | 0 | 0 in all 12 | yes |
| A2 traction, B and C, six sets each: differing entries | 0 | 0 in all 12 | yes |
| A3 `face_area` vs `hex20::face_area`, B (48 faces), C (36 faces) | <= 1e-15 | 0 and 0 | yes |
| A4 `face_area_vector` vs the 3 x 3 Gauss vector, B, C | <= 1e-15 | 0 and 0 | yes |
| A5 no panic on 84 faces of 8 nodes | as stated | completed | yes |
| A6 Quad4, Tri3, Tri6 loads vs the transcription of e05871f6 | 0 | 0 in all 36 | yes |
| A6c control: A6 and A7 on an archive copy of e05871f6 | 0 | 0 in all 39 (A1 to A5 there refuse or panic, as before) | yes |
| A7 Quad4 (24), Tri3 (96), Tri6 (96 faces): areas and area vectors | 0 | 0, 0, 0 | yes |
| A8 `cargo test -p rcfd-api --release --test baseline_hashes` | passes | 3 passed, 1 ignored (pre-existing), 0 failed | yes |
| K1 F, area vector vs diagonals / area | <= 1e-14 | 3.0e-16 / 4.0e-16 | yes |
| K2a total force of a set, B / C (max over six sets) | <= 1e-13 | 1.9e-16 / 1.9e-16 | yes |
| K2b closed boundary of B | <= 1e-13 | 2.0e-17 | yes |
| N1 one ulp seen | 1 | 1 | yes |
| N2 count 5 refused, "a face of 5 nodes" | as stated | refused by both, message as stated | yes |
| N3 nodes of a set without force, B and C | 0 | 0 in all 12 | yes |
| S1 other tests of rcfd-fem | 0 failed except the predicted L3 | 29 binaries, 163 passed, 33 ignored, 1 failed: `si_hex20::l1_to_l3_face_loads_and_uniaxial_tension`, only its L3 row ("L3 model pressure on a Hex20 face refused: accepted") | yes (as registered) |

### Mutations (scratch copies, `w49i-work/f484-scratch`)

| Mutation | Caught by |
|---|---|
| M1 face derivatives of nodes 4 and 5 swapped (loads.rs) | A1, A2 (all 24), K2a (all 12) |
| M2 face rule order `i` outer (loads.rs) | A1, A2 (all 24): the bit comparison sees a change of summation order |
| M3 2 x 2 rule in the 8-node arm (mesh.rs) | A3 C (5.1e-5), A4 C (2.1e-15); not on B (flat faces) |
| M4 `xu`, `xv` exchanged (mesh.rs) | A4 B and C, K1, K2a (all 12) |
| M5 `G` of the Quad4 arm times `1 + 1e-15` (mesh.rs) | A7 Quad4 (82 values) |

M3 is caught by A4 only by roundoff (2.1e-15): the 2 x 2 rule is exact to degree 3 per direction, the degree of `x_u x x_v`
(D2), so the area vector itself is still exact; A3 (the area of a curved face) carries that catch.

### After the tests

`cargo fmt -p rcfd-fem --check`: clean (the new test file was formatted with rustfmt before the commit). `cargo clippy -p
rcfd-fem --all-targets -- -D warnings`: clean.

### Deviations and disclosures

- The new test file was run once without `--nocapture` (all four tests passed) and then once with `--nocapture` to read the
  printed numbers; the tests are deterministic and nothing was changed between the two runs.
- The mutation script failed twice on a Python version issue before any mutated build; no mutated run was made before the
  script worked.

### Not run

The workspace gate (the lead's). rcfd-post and rcfd-io tests (not touched; the only change is in rcfd-fem and no default path
reaches the new arms: an 8-node face exists only on Hex20 meshes, where the old code refused or panicked).

### Decision by the registered rule

Every check of A, K and N met and S1 as predicted: **done**, with one step left to the maintainer (below).

### Remaining questions

1. `si_hex20::l1_to_l3_face_loads_and_uniaxial_tension` (ADR 0292) now fails on L3, the refusal this item retires; it is not in
   `si_scaffold.rs`, so the standing permission does not cover it and it was not edited. Options: (a) mark it `#[ignore =
   "record of ADR 0292: model-path pressure on Hex20 refused until ADR 0383"]` and add a copy `..._after_landed` without L3
   (L1 and L2 still checked; the pattern of `si_scaffold.rs`); (b) leave it failing. Recommendation (a): needs the maintainer.
2. `Load` has no traction-field variant; the model path covers uniform pressure and traction only. Options: (a) leave
   `hex20::traction_field_loads` as per-node forces; (b) a follow-up adds a field load to `Load` for every face kind.
   Recommendation (a) until a case needs it.

## For the lead: roadmap entries

Proposed status of F-484: done (pending the maintainer's step for the ADR 0292 L3 record).

> ADR 0383, 2026-11-13: Load::Pressure and Load::Traction on Hex20 faces go through nodal_load_vector, and SolidMesh::face_area and face_area_vector take 8-node faces (8-node arms in loads::face_shape_functions, loads::face_rule and SolidMesh::face_tangents delegating to hex20). Met: per-node forces equal hex20::pressure_loads and traction_loads bit for bit on flat and curved meshes (24 of 24 sets), face_area and face_area_vector equal hex20::face_area and the 3 x 3 Gauss vector exactly (84 faces), Tri3, Quad4 and Tri6 loads and areas bit-identical to a transcription of the previous code (checked also on the previous code), total force 2e-16, baseline hashes unchanged; five mutations caught. si_hex20 l1_to_l3 fails on its L3 row (the retired refusal): recorded as needing the maintainer (ignore as record of ADR 0292 and add a copy without L3).

Note for F-389:

> ADR 0383, 2026-11-13: the model-path refusal of Hex20 face loads and the panic of SolidMesh::face_area on an 8-node face are retired (F-484).
