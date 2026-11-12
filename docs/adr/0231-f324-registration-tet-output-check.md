# ADR 0231: F-324 registration: an output validity check inside `tetrahedralize` (positive cells, cell volume equal to the enclosed volume)

Status: **REGISTERED 2026-11-12**, written before any code of F-324 and before any run of its checks. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0228](0228-f306-registration-tetmesh-neighbour-link-panic-on-a-thin-box.md) (F-306: the folded edge removal, its
validity V, its sweep and fingerprints, question Q3, whose recommendation (a) "a small follow-up item" the lead took for F-324),
[ADR 0030](0030-wave19b-meshing.md) (F-132: `tetmesh::tetrahedralize`). Roadmap item F-324 (depends on F-306).

Number: the lead gave 0231 to F-324. Branch `f324-tet-output-check` from main 0d250c75, worktree `w49i-work/f324`.

House rules (as ADR 0228):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written (and committed) before the corrected run, and the old one
  stays as an ignored record;
- no case-schema key, no option, no field of `TetOptions` or `TetReport`; neither `crates/rcfd-api/tests/baseline_hashes.rs` nor
  any case or mesh hash table nor any golden is edited or re-recorded, and no existing test is edited;
- if the new check rejects a mesh that an existing test accepts, the test is not weakened and the check is not loosened: it is
  recorded and reported;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no `unsafe`, unsigned commits, no trailers, no push.

## What F-324 asks

The roadmap entry (acceptance, quoted, binding): "tetrahedralize checks its output (every cell positive, volume equal to the
enclosed volume) and returns a located error instead of an overlapping Ok; the F-306 sweep inputs still give identical meshes."

## What was read and computed before this text (disclosed)

- **Read:** ADR 0228 in full (registration, addendum, questions Q2 and Q3, gate section); `crates/rcfd-mesh/src/tetmesh.rs` at
  0d250c75; the F-306 tests `crates/rcfd-mesh/tests/tetmesh_fold_f306.rs` (R1, R2, N1 with the 111 recorded fingerprints) and
  `crates/rcfd-io/tests/tet_fold_f306.rs` (R3, N1s with 7 records); `crates/rcfd-mesh/tests/tetmesh.rs` (head);
  `crates/rcfd-mesh/src/predicates.rs` (`orient3d`: a floating-point filter `|det| > 1e-13 perm`, then an exact expansion);
  `TriSurface::enclosed_volume` (`surface.rs`: `sum p0 . (p1 x p2) / 6` about the origin, plain summation); the roadmap entries
  F-306, F-324, F-325. ADR 0226 for the style.
- **Searched:** every caller of `tetrahedralize` and `TetMesh` outside `tetmesh.rs`: tests only (`rcfd-mesh` `tetmesh`,
  `polydual`, `tetmesh_fold_f306`, the `remesh.rs` unit tests; `rcfd-io` `w41d_tet_quality`, `w42c_tet_default_eval`,
  `stl_cylinder_in_box`, `tet_fold_f306`; `rcfd-fv` `remesh_f247`, `w19b_meshing_solve`), plus `polydual.rs`, `remesh.rs` and
  `rcfd-io/src/common.rs`, which take a `TetMesh` but do not call the mesher. No case, CLI or API path calls it, so no case hash
  and no `baseline_hashes.rs` entry can depend on it.
- **Arithmetic on paper (no run):** the size of the rounding bound B of check (b) below. For a box of well-shaped cells meshed
  about its own centre, the permanents (below) of a cell are a small multiple of `6 |vol|`, and those of the surface triangles
  about the centre are about `6 |w_t|` with every `w_t > 0`; so `B` is of the order `8 eps (a few E) ~ 1e-14 E` (`eps` =
  `f64::EPSILON` = `2^-52`). The overlaps that F-306 measured on main were `+7.3e-5` to `+1.6e-2` relative (ADR 0228, N2), nine to
  twelve orders above it.
- A release build of the existing `rcfd-mesh` and `rcfd-io` test targets was started (compilation only, nothing run) while this
  text was written.

No code of F-324 existed when this text was written, and no check below was run.

## Design (registered)

### Where

At the end of `tetrahedralize`, after the compaction builds the `TetMesh`, a new public method `TetMesh::check_output(&self) ->
Result<OutputCheck>` is called; `Err` is returned as the result of `tetrahedralize`, `Ok` returns the mesh unchanged. The method
reads the mesh only: the returned `TetMesh` of an accepted run is the one main returns, bit for bit (same points, cells, surface,
report). `OutputCheck` is a small public struct of the measured numbers (the two volumes, their difference and the bound B), so
tests can record the margin. The method is public so that tests, and later callers holding a `TetMesh` built elsewhere, can run
the same check.

### The invariants (taken from the acceptance)

- **(a) Positive cells.** Every cell `[p0, p1, p2, p3]` has `orient3d(p0, p1, p2, p3) > 0` with the exact predicate of
  `predicates.rs` (no tolerance: the predicate is exact). The mesher builds every cell positive (cavity faces seen from the new
  point, flips with non-zero signs, ring triangles with `od == -s`, smoothing moves kept only if every star cell stays positive),
  so (a) is an invariant check.
- **(b) Volume equal to the enclosed volume.** With `c` the midpoint of the bounding box of the output points,
  `S_cell = sum over cells of det(p1 - p0, p2 - p0, p3 - p0) / 6` and `S_surf = sum over the output surface triangles
  (q0, q1, q2) of det(q0 - c, q1 - c, q2 - c) / 6`, it requires `|S_cell - S_surf| <= B`.

  *Why the output surface and `c`.* The output surface is the conforming subdivision of the input surface that the cells are
  meant to fill (same point vector, outward orientation kept by `SubSurf::split_edge`). In exact arithmetic, on the floating-point
  points as they are, `S_cell = S_surf` holds exactly for any `c` whenever the cells fill the region once: each cell's volume is
  the sum of the volumes of the cones from `c` over its four outward faces, the inner faces cancel in pairs, and the free faces are
  the surface triangles (divergence theorem, e.g. any calculus text; the cone decomposition is first principles). A fold adds
  `2 |V_f|` (the folded cells' volume, ADR 0228's ring arithmetic: the new cells exceed the ring by twice the folded part), a gap
  removes volume. Comparing with the input surface's `enclosed_volume()` instead would mix in the rounding of the Steiner midpoints
  and of a sum about the origin, which has no rigorous bound here. On every accepted output the two references agree to
  `1e-12` relative (that is V(b), which the F-306 tests keep checking against `diagnose(S).enclosed_volume`).

  *The bound B (Shewchuk 1997).* For a 3 by 3 determinant of rounded differences, evaluated as a cofactor expansion
  `bx (cy dz - cz dy) - by (cx dz - cz dx) + bz (cx dy - cy dx)` exactly as `predicates::orient3d` does, the absolute error against
  the exact determinant of the input points is at most `(7 + 56 u) u P` with `u = 2^-53` and `P` the permanent
  `|bx| (|cy dz| + |cz dy|) + |by| (|cx dz| + |cz dx|) + |bz| (|cx dy| + |cy dx|)` computed in floating point (J. R. Shewchuk,
  "Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates", Discrete Comput. Geom. 18 (1997) 305-363,
  the bound `o3derrboundA`). Dividing by 6 adds at most `u |det| / 6 <= u P / 6`. Both sums are formed by Neumaier's compensated
  summation (A. Neumaier, ZAMM 54 (1974) 39-51; N. J. Higham, *Accuracy and Stability of Numerical Algorithms*, 2nd ed., §4.3),
  whose error is at most `2u |S| + O(n u^2) sum |x_i|`, below `4u sum |x_i|` for any `n < 2^50`. With `eps = 2u`:

  `B = 8 eps (sum of P over the cells + sum of P over the surface triangles) / 6`

  which covers `(7 + 56u + 1) u = 4 eps` (per simplex) plus `2 eps` (the two sums, `sum |x_i| <= sum P / 6`) and leaves `2 eps`
  for the final subtraction and the rounding of B itself. B is a rigorous bound up to these second-order terms, so a mesh that
  tiles its surface exactly once can never fail (b): the check cannot reject a valid output, whatever its size or offset. It is
  not a tuned tolerance and it has no free constant to adjust.

### Location

On a failure the error names where (cost spent only on the failing path):

- (a): the first cell index that fails, its four vertex indices and its centroid;
- (b): the two volumes, the difference and B, then the first local defect found by one pass over the cell faces keyed by sorted
  vertex triple: a face of more than two cells, a face whose two cells lie on the same side (same orientation), a free face that
  is not a surface triangle, or a surface triangle that is not a free face; with the cell indices and the face centroid. If none
  is found the text says so ("the cells cover the region more than once").
- A surface triangle that refers to a vertex outside `points` (the compaction's `NONE` when a surface vertex is in no cell) is a
  located error too, instead of an index panic.

Every message starts with `tetrahedralize output check:` and is an `Error::Mesh`.

### Cost and how it is bounded

One pass over the cells (one `orient3d`, whose filter decides with the same floating-point determinant except for nearly flat
cells, then one floating-point determinant and permanent) and one pass over the surface triangles: `O(cells + triangles)`, no
allocation on the accepting path. The meshing itself does Delaunay insertion, recovery and flips over the same cells many times,
so the check is expected to be under one percent of the run (C5 below fixes a limit). The localisation pass (a hash map of the
faces) runs only when the check fails.

### What is not changed

The mesher's operations, `TetOptions`, `TetReport`, `to_mesh`, the two `expect("neighbour link")` invariants (ADR 0228 Q2: option
(a), kept; a panic stays a panic, the check runs on what comes out), the F-306 tests and every existing test.

## Registered checks

New tests in two new files, `crates/rcfd-mesh/tests/tetmesh_check_f324.rs` (C3, C4, C6, C7 for F1 to F13, C5 on F12) and
`crates/rcfd-io/tests/tet_check_f324.rs` (C4, C6 for F14, C5 on F14); the F-306 helpers (inputs, `run` under `catch_unwind`, V,
the fingerprint, the STL helpers) are copied, as F-306 copied the w42c helpers. All tests run in release.

| Id | Check | Limit |
| --- | --- | --- |
| C1 | The F-306 tests unedited at the final commit: R1, R2 (12 tests), R3, N1 (6 tests, 111 fingerprints), N1s (7 records) | all pass: every fingerprint and error text equal to the record (Windows), V on every output |
| C2 | The existing suites unedited at the final commit: `rcfd-mesh` lib (the `remesh` unit tests), `tetmesh`, `polydual`; `rcfd-io` `w41d_tet_quality`, `w42c_tet_default_eval`, `stl_cylinder_in_box`; `rcfd-fv` `remesh_f247` (M0 hash `aaf8f4c36f6446e4`), `w19b_meshing_solve` | pass, known misses stay ignored; no output that a test accepts is rejected by the check |
| C3a NC-dup | `tetrahedralize(cube(2, 2))` (default options), then cell 0 appended a second time, `check_output` | `Err`; the text names a face of more than two cells (or two cells on one side) with cell indices; the measured difference equals cell 0's volume to `1e-9` relative |
| C3b NC-flip | the same mesh with vertices 2 and 3 of cell `k = cells / 2` swapped | `Err` naming cell `k` as not positively oriented |
| C3c NC-gap | the same mesh with its last cell removed | `Err`; difference equal to minus that cell's volume to `1e-9` relative; the text names a free face that is not a surface triangle |
| C3d NC-surf | the same mesh with one surface triangle's vertex set to `points.len()` | `Err` naming that triangle, no panic |
| C3e PC-offset | the same mesh with every point translated by `(1e3, -2e3, 5e2)`, and separately scaled by `1e-6` and by `1e6` (cells whose exact orientation changes by the rounding, if any, are not expected for this well-shaped mesh) | `Ok` for all three (B must hold under offset and scale) |
| C4 K1 | the 13 folded inputs F1 to F13 of ADR 0228 (`tetmesh_check_f324`) and F14 (`tet_check_f324`), at the final commit: each outcome is classified | no input returns an `Ok` that fails V; every one returns `Ok` (as R1 to R3 at main) |
| C5 cost | F12 (`box [1, 1, 0.1] [10, 10, 1]` size 0.035, sweeps 0, 11292 cells at main) and F14 (17134 cells): the time of one `check_output` call against the time of the `tetrahedralize` call that produced the mesh; one run each, an ignored test run once by this ADR (timing does not belong in the gate) | check time at most 5 percent of the `tetrahedralize` time, for both |
| C6 R-fp | the R outputs at the final commit against ADR 0228's addendum: cells (and points where recorded) of F1 to F14; hashes of F6, F7, F8 (`2d3ca7189bac0af7`, `1fb9fcdc40f95c94`, `6e90359a20d929c4`) | equal (Windows); on other platforms printed only |
| C7 margin | `diff / B` on every accepted output of C4 and of a print of the N1 inputs' C-group (14 inputs) | recorded, not a limit (B is rigorous; a value above 1 cannot be accepted) |

C1 and C6 are the "identical meshes" part of the acceptance: the check only reads the mesh, so an accepted output is the output of
main; N1, N1s and C6 confirm it on all 132 sweep inputs.

### Negative controls

- NC1 (C3a to C3d): the check must reject a duplicated cell, a negative cell, a missing cell and a broken surface reference, each
  with a location. If any of them is accepted the check is defective.
- NC2 (mutation MU1 below): with the F-306 fold re-introduced, the overlapping `Ok` outputs of main must become located errors of
  the check, which is the acceptance's "instead of an overlapping Ok".
- PC (C3e and C1): valid meshes, also offset and scaled, must pass; if the check rejected any, the bound would be defective.

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final commit by an exact text replacement (a script in
`w49i-work/f324-scratch`), the named tests are run once in release, and the file is checked byte for byte against a kept copy
afterwards.

| Id | Mutation | Must |
| --- | --- | --- |
| MU1 | ADR 0228's MU1: the triangle test of `try_edge_removal` back to main 9f95fb12's (`od == 0 \|\| oe == 0 \|\| od == oe`), the ring-sign check removed | C4 K1 passes (no overlapping `Ok`); R2 F1, F5, F9, F10 and R3 F14 (the five overlapping `Ok` outputs of main) each fail with `located error instead of a mesh: ... tetrahedralize output check:` and a location in the text; R2 F2 to F4, F12, F13 and R1 panic as on main (out of scope, Q2); R2 F6 to F8 pass (valid on main) |
| MU2 | MU1 and the `check_output` call removed from `tetrahedralize` | C4 K1 fails (the overlapping `Ok` outputs return) |
| MU3 | the volume comparison (b) removed (always accept) | C3a and C3c fail |
| MU4 | the orientation test (a) removed | C3b fails (the volume check still rejects, but the text does not name the cell as not positively oriented) |
| MU5 | B set to 0 | at least one N1 test fails (the floating-point sums of valid meshes differ; B is needed) |

### Predictions (mine)

| Id | Prediction |
| --- | --- |
| PR1 | C1, C2, C4, C6 pass: no valid output fails a rigorous bound and the check changes nothing it accepts |
| PR2 | C5: under 1 percent on both inputs |
| PR3 | C7: `diff / B` below `1e-2` on every accepted output |
| PR4 | MU1: on the five overlapping outputs `diff / B` above `1e6`; the localisation finds a face of more than two cells on F1, F9, F10 (as `to_mesh` did on main) |

## Decision rule (registered)

- **done**: C1, C2, C3 (all five), C4 and C6 met, MU1, MU2, MU3 and MU4 fail their checks as registered, C5 met.
- **partial**: C1, C2, C3, C4 and C6 met, but C5 missed, or a mutation does not make its check fail (the check works on the
  sweep, its tests are not shown able to fail); the miss is recorded with its number.
- **todo**: C1 or C6 missed (an output changed), C4 missed (an overlapping `Ok` remains), C3 missed (a defect accepted or a valid
  mesh rejected), or C2 shows the check rejecting a mesh an existing test accepts; such a change is not merged, and the case is
  recorded and reported.

MU5 is reported; a survival of MU5 would mean the sums of valid meshes are exact in the sweep, which is a finding, not a defect of
the check, and does not count in the rule.

## Limits of this item

- The check finds overlaps and gaps whose volume exceeds B (about `1e-14` of the enclosed volume on the sweep, by the arithmetic
  above). A fold whose folded cells are slivers of total volume below B is not caught by (b); a face-pairing check (every inner face
  of exactly two cells on opposite sides, free faces equal to the surface triangles, which `to_mesh` does) would catch it at the
  cost of a hash map of the faces on every run. Not delivered here (Q2 below).
- A panic inside the mesher (the `expect("neighbour link")` of `commit` and `replace`) is not turned into an error (ADR 0228 Q2,
  option (a) kept). With the F-306 fix no known input reaches it.
- Fingerprints are asserted on Windows only, as in F-306 (F-325 is the Linux record).
- Meshes built by the F-247 remesher (`remesh.rs`) do not pass through `tetrahedralize`; `check_output` is available to them but not
  called.

## Open questions and choices for the maintainer

- **Q1. Status of F-324.** If the rule gives done, the acceptance is met. Recommendation: done, with the maintainer's acceptance.
- **Q2. Face pairing in the check.** Options: (a) (a) and (b) only, as the acceptance lists; (b) also the face-pairing test on every
  run (catches sub-B overlaps; costs a face map, about the cost of `to_mesh`'s); (c) a follow-up item. Taken for this branch on the
  maintainer's standing instruction of 2026-11-11 (take the recommendation): (a), with (c) proposed in the addendum if the measured
  cost of the localisation pass makes it cheap.
- **Q3. Reference volume.** The check compares with the output surface (rigorous), not with `diagnose(input).enclosed_volume`
  (as V does, with a fixed `1e-12`). Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the
  output surface. It changes no default result: both references agree on every accepted output, which C1 shows.

## Amendment 1 (2026-11-12, before any test code and before any run)

A defect of the set-up found while writing the tests: C4 asks both "no `Ok` that fails V" and "every input returns `Ok`", while
MU1 requires C4 to *pass* with the fold re-introduced, where five inputs panic and five return the check's error. The two halves
are split, limits unchanged:

| Id | Check | Limit |
| --- | --- | --- |
| C4a K1 | F1 to F13 (`tetmesh_check_f324`), F14 (`tet_check_f324`): no outcome is an `Ok` that fails V (a located error or a panic is not an overlapping `Ok`) | none |
| C4b | the same inputs | every one returns `Ok` |

MU1 must: C4a passes, C4b fails (and the R2 and R3 texts as registered). MU2 must: C4a fails. The decision rule reads C4 as C4a
and C4b. To keep the run time of one pass, C4a, C4b, C6 and the C7 print share one computation of the 13 (and 1) outcomes per test
process (`OnceLock`); the gate's one process per test computes it once per test.
