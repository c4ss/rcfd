# ADR 0236: F-327 registration: the tet output check on the outputs of the F-247 remesher

Status: **REGISTERED 2026-11-12**, written before any code of F-327 and before any run of its checks. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0231](0231-f324-registration-tet-output-check.md) (F-324: `TetMesh::check_output`, its invariants (a) and (b),
its rounding bound B, question Q2 whose option (c) "call `check_output` on the F-247 remesher's outputs" became this item),
[ADR 0215](0215-f247-registration-local-remeshing-on-unstructured-tetrahedral-meshes.md) (F-247: `rcfd_mesh::remesh`, the
operators, the validity M4, the event M7, P-T and its records). Roadmap item F-327 (depends on F-247 and F-324).

Number: the lead gave 0236 to F-327. Branch `f327-remesher-output-check` from main d1591e54, worktree `w49i-work/f327`.

House rules (as ADR 0231):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written (and committed) before the corrected run, and the old one
  stays as an ignored record;
- no case-schema key, no option, no field of `RemeshSettings`; neither `crates/rcfd-api/tests/baseline_hashes.rs` nor any case
  or mesh hash table nor any golden is edited or re-recorded, and no existing test is edited (`remesh_f247.rs`, the unit tests
  U1 to U3 of `remesh.rs` and its test helper `apply_one` stay as they are);
- if the new check rejects an output that an existing test accepts, the test is not weakened and the check is not loosened: it
  is recorded with its numbers and reported, and the item is then partial;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no `unsafe`, unsigned commits, no trailers, no push.

## What F-327 asks

The roadmap entry (acceptance, quoted, binding): "TetRemesher's output meshes pass TetMesh::check_output after every remeshing
step, or the step returns its located error; the remesh_f247 hashes stay unchanged."

## What was read and computed before this text (disclosed)

- **Read:** ADR 0231 in full (design, bound B, checks, amendments, addendum, Q2); ADR 0215: the model M1 to M7, settings S1,
  the registered checks P, N, U, D, the mutations, addendum 1 (results, printed numbers, gate); the roadmap entries F-247,
  F-324, F-327; `crates/rcfd-mesh/src/remesh.rs` at d1591e54 in full; in `crates/rcfd-mesh/src/tetmesh.rs` the `TetMesh` and
  `TetReport` structs, `check_output`, `OutputCheck`, `det_perm`, `neumaier`, `locate_cover_defect`; `TriSurface`'s fields
  (`surface.rs`); `crates/rcfd-fv/tests/remesh_f247.rs` in full (P-T, N2).
- **Found:** the only recorded hash of `remesh_f247` is M0's (`aaf8f4c36f6446e4`, asserted in P-T's P0); M0 is made by
  `tetrahedralize`, which F-324 already checks, so the hash depends on no remesher code. The remesher's own outputs are recorded
  in ADR 0215's addendum as printed numbers (A1 `3.69e-6`, A2 `9.45e-6`, 1806 events, the operator counts, the cells at `T` and
  `2T`, 55497 steps, the smallest mean ratio `0.2407`), not as hashes. P-T asserts `event().expect("R1: no event returns an
  error")`, so a check inside `event()` that rejected a P-T output would fail P-T.
- **Searched:** the callers of `TetRemesher::event`, `remesh` and `RemeshChange`: `remesh.rs` (its unit tests) and
  `crates/rcfd-fv/tests/remesh_f247.rs` only; `RemeshChange` is built only inside `remesh.rs`. No case, CLI or API path uses
  the remesher (ADR 0215 S3), so no case hash can depend on it.
- **Arithmetic on paper (no run):** the bound of the volume comparison (D) below is of the size of ADR 0231's B, about `1e-14`
  of the volume (the same permanents, summed over the cells before and after). The negative control NC4 lengthens M_s's box by
  `delta = 1e-3` in x with a cross-section `0.1 x 0.1`, so the volume grows by `1e-5`, about `3.4e-4` of M_s's volume (0.03
  less the piston's move), ten orders above the bound.
- A release build of the existing `rcfd-mesh` test targets and of `rcfd-fv`'s `remesh_f247` was started (compilation only,
  nothing run) while this text was written.

No code of F-327 existed when this text was written, and no check below was run.

## Design (registered)

### What "the output mesh" is, and why the check needs a variant

`TetMesh::check_output` compares the cell volume with the volume enclosed by `TetMesh::surface`. For `tetrahedralize` that
surface is independent of the cells: it is the conforming subdivision of the input surface. The remesher has no such surface.
Its boundary is re-triangulated by the events (a split of a boundary edge adds a point on a plane, the 2-2 flip of a wall edge
swaps two wall triangles, a collapse along a plane removes a boundary point, a relocation moves a boundary point inside its
plane), and its planes move between events. So the remesher's output does not satisfy (b) "by construction" against any
stored surface; the only surface it has is the set of its free faces (faces of exactly one cell), which `mesh()` already
builds.

With the free faces as the surface, `check_output` is not vacuous but it is weaker than in `tetrahedralize`:

- it still catches a cell that is not positively oriented ((a)), a face of three or more cells and two cells on the same side
  of a face (such faces are not free, so the cones over them are missing from the surface sum and (b) fails, unless the cone
  of every such face about the centre happens to be zero);
- it cannot see a gap: when cells are missing, the faces around the hole become free, and in exact arithmetic the cell volume
  equals the volume enclosed by the free faces whatever is missing (divergence theorem); nor can it see a valid tiling of a
  different region.

The variant adds two checks that use what the remesher knows and `tetrahedralize`'s surface stands for: the region is the
polyhedron bounded by the planes, and an event does not change it.

### The step check (registered)

After an event that applied at least one operator, and before the remesher's state is replaced, the step's output (the
renumbered points, plane masks and cells that `finish` makes today) is checked:

- **(A) and (B): `TetMesh::check_output`** on the `TetMesh` with the output's points and cells and the surface of its free
  faces (each in the outward orientation of its cell, in the order: cells in index order, then the four faces in the order of
  `FACE`; `TetReport::default()`). Unchanged code of F-324: exact `orient3d` per cell, and the cell volume against the volume
  enclosed by the free faces within ADR 0231's rigorous B.
- **(C) Free faces on the planes:** every free face lies on a plane: the plane given by the common mask of its three points
  (the lowest set bit, as `mesh()` chooses the patch) must exist, and each of the three points must have the plane's
  coordinate exactly (`p[axis] == value`, bit for bit; the remesher keeps boundary points exactly on their planes: M1, the
  split's snap, the relocation's fixed coordinates, `move_plane_with_band`). This is the condition `mesh()` applies by mask,
  plus the coordinates.
- **(D) The region is unchanged:** `S_after`, the cell-volume sum of the output, against `S_before`, the cell-volume sum of the
  input of the step, both computed as in `check_output` (`det_perm` per cell divided by 6, Neumaier summation):
  `|S_after - S_before| <= 8 eps (sum P_before + sum P_after) / 6`, P the permanents of the cells. *Why this is rigorous.* An
  event moves no plane, and every operator keeps the boundary points exactly on their planes (C), so in exact arithmetic on
  the floating-point points both meshes tile the same polyhedron when they are valid and `S_before = S_after` exactly; each
  computed sum is within `(4 eps + 2 eps) sum P / 6` of its exact value (ADR 0231's per-simplex and summation terms), which
  leaves `2 eps` for the subtraction and the rounding of the bound. A valid step can never fail (D); a gap, an overlap or a
  changed region of volume above the bound fails it. The input's volume is computed at the start of `event()` (one pass over
  the cells; the input itself is not checked here: its validity is the previous step's check, or the motion's, Q2).

(A) to (D) run in this order; the first failure is the step's error.

### Where, and what a failed step does

- `TetRemesher::event()` (and so `remesh()`, which calls it) runs the step check on every event that applied an operator; an
  event that applied none returns `Ok(None)` as today, its mesh being the input unchanged.
- On a failure `event()` returns `Err(Error::Mesh(..))` and **leaves the remesher's state as it was before the event** (the
  output is built, checked, and only then committed), so the caller still holds the last checked mesh and can stop or retry.
- On success the state and the returned `RemeshChange` are those of main bit for bit (the check only reads). `RemeshChange`
  gets one new public field, `check: Option<StepCheck>`, with the measured numbers (`Some` from `event()`; `None` from the
  test-only one-operator helper `apply_one`, which is not a remeshing step of the public interface and is not changed).
- New public items: `StepCheck { output: OutputCheck, volume_before, volume_after, volume_difference, volume_bound }`;
  `TetRemesher::to_tet_mesh()` (the current mesh with its free-face surface) and `TetRemesher::check_output()` ((A) to (C) on
  the current mesh, for callers that change the mesh by motion). In `tetmesh.rs` one additive `pub(crate)` helper giving the
  Neumaier cell-volume sum and the permanent sum of a cell list (the code of `check_output`'s first loop, which itself is not
  changed).

### The located error

Every message starts with `remesh output check:`, then names the event (its operator counts and the box of the touched points,
`RemeshChange::lo` and `hi`), then the defect:

- (A) and (B): `check_output`'s own text unchanged (it starts `tetrahedralize output check:` and names the cell with its
  vertices and centroid, or the first face of more than two cells, or of two cells on one side, or free but not a surface
  triangle); cell and vertex indices are those of the output the step would have made;
- (C): the free face (its vertices, the cell holding it, its centroid) and the plane it should lie on, or "lies on no plane";
- (D): the two volumes printed with sixteen significant digits (`{:.15e}`, as amended in ADR 0231), the difference and the
  bound, and "the event changed the region".

### What is not changed

The operators, the validity M4, the event procedure M7, the trigger, `mesh()`, `RemeshSettings`, `remap`, the motions; the
existing tests (U1 to U3, `apply_one`, `remesh_f247.rs`); `TetMesh::check_output` and every other line of `tetmesh.rs` but
the one new helper.

## Registered checks

New tests in a new test module `tests_f327` inside `crates/rcfd-mesh/src/remesh.rs` (they need the private `Work` to break a
step on purpose). A test-only hook is the only way in: `event()` becomes `self.run_event(|_| ())` with
`fn run_event(&mut self, hook: impl FnOnce(&mut Work))` called between `Work::run` and the output; the tests call `run_event`
with a hook that breaks the step. All tests run in release.

Set-ups: **M_s** is U2's mesh (`tetrahedralize(box_surface([0, 0, 0], [0.3, 0.1, 0.1], [6, 2, 2]))`, size 0.035,
`quality_sweeps` 3, the six planes, `RemeshSettings::for_size(0.035)`). **Hook R** applies the first valid relocation in index
order (U2's `first_valid(w, Kind::Relocation)`, with λ = 0.5) after `Work::run`, so that every control's step applied at least
one operator. **M0** is ADR 0215's M0 (1711 cells). **Piston G**: M0, `xmax` moved with `move_plane_with_band(xmax, x, 0.07)`
from 1.0 to 0.4 in steps of `1e-3` and back to 1.0 in steps of `1e-3` (x rounded to the step as in U2), `remesh()` after
every move; no solver.

| Id | Check | Limit |
| --- | --- | --- |
| C1 P-T | `remesh_f247` unedited at the final commit: P-T and N2 | both pass; P0's M0 hash `aaf8f4c36f6446e4`, 1711 cells; the printed A1, A2, events (1806), operator counts (435, 454, 337, 5, 2086), cells at `T` and `2T` (729, 1860), steps (55497) and the smallest mean ratio (0.2407) equal to ADR 0215's addendum at the digits it records; no event returns an error (P-T's R1, now including the step check) |
| C2 suites | `cargo test --release -p rcfd-mesh` at the final commit: the unit tests U1 to U3 and every other lib test, `tetmesh`, `polydual`, `tetmesh_fold_f306`, `tetmesh_check_f324` and every other test binary of the crate | pass; known misses stay ignored; no output that a test accepts is rejected |
| PC1 | M_s, `run_event` with hook R only | `Ok(Some(change))` with `change.check` `Some`; `|difference| <= bound` in (B) and (D) (recorded) |
| PC2 | Piston G: every `remesh()` | `Ok` for every step (no step check error); at least 5 events that changed the mesh (set-up: otherwise the control is defective); the largest `|difference| / bound` of (B) and of (D) over the events recorded |
| NC1 dup | M_s, hook R then the last alive cell appended a second time (with its sources) | `Err`; the text starts `remesh output check:` and names a face "shared by the cells" or with two cells "on the same side"; the remesher's points and cells equal to those before the call |
| NC2 flip | M_s, hook R then vertices 2 and 3 of the last alive cell swapped | `Err` naming that cell, by its index in the output numbering (the number of alive cells before it), as "not positively oriented"; state unchanged |
| NC3 gap | M_s, hook R then the first alive cell in index order none of whose four faces has its three points on a common plane killed | `Err` whose text names a free face that "lies on no plane" with its cell; state unchanged |
| NC4 region | M_s, hook R then every point on `xmax` (by mask) moved by `+1e-3` in x and the `xmax` plane of the event's copy of the planes set to the same value (a valid tiling of a longer box) | `Err` naming (D); the difference, the printed after-volume minus the printed before-volume, equal to `1e-5` within `1e-9` relative; state unchanged |
| C4 cost | Piston G, an ignored test run once by this ADR (timing does not belong in the gate): for every event that changed the mesh, the time of the step check (the input's volume sum, timed on a clone of the input, plus (A) to (D) on the output, timed after the event) against the time of the `remesh()` call (which includes its own check) | sum of the check times at most 25 percent of the sum of the event times |

For NC4 the step check reads the planes of the event's copy (`Work::planes`, equal to the remesher's planes on every path but
this hook); this is how a step that changes the region while keeping its faces on planes is made, and it is disclosed as a
property of the set-up.

C1 is the "remesh_f247 hashes stay unchanged" part of the acceptance, and with PC2 the "pass after every remeshing step" part;
NC1 to NC4 are the "or the step returns its located error" part.

### Negative controls

NC1 to NC4: a duplicated cell, a negative cell, a missing cell and a changed region must each be rejected, located, with the
state left as it was. If any of them is accepted, the check is defective. PC1, PC2 and C1: valid steps must pass; if any is
rejected, either the bound is defective or the remesher made an invalid output that its existing checks accepted, which is a
finding, recorded with its numbers.

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final commit by an exact text replacement (each pattern found exactly
once), the `rcfd-mesh` lib tests are run once in release in their own target dir, and the file is checked byte for byte
against a kept copy afterwards.

| Id | Mutation | Must |
| --- | --- | --- |
| MU1 | the step check not called in `event()` | NC1, NC2, NC3, NC4 fail |
| MU2 | (C) removed | NC3 fails (the gap is then found by (D), whose text does not name a face on no plane) |
| MU3 | (D) removed | NC4 fails |
| MU4 | the output committed before the check | the state-unchanged part of NC1 to NC4 fails |
| MU5 | the bound of (D) set to 0 | reported, not counted: PC1 or PC2 fails if the computed sums of a valid step differ |
| MU6 | V2 and V3 of `evaluate` removed (only V1 and V4 kept) | reported, not counted: whether Piston G (PC2) then meets a step check error |

### Predictions (mine)

| Id | Prediction |
| --- | --- |
| PR1 | C1, C2, PC1, PC2 pass: the remesher's outputs are valid, so the step check rejects none |
| PR2 | C4: the check costs 5 to 20 percent of an event (a face map over the cells against the event's passes over all edges) |
| PR3 | NC1 is caught by (B) with a face of more than two cells; NC3 by (C) |
| PR4 | MU6 makes no step check fail in Piston G (V1 and V4 alone already rule out most invalid candidates) |

## Decision rule (registered)

- **done**: C1, C2, PC1, PC2, NC1 to NC4 met, C4 met, MU1 to MU4 fail their checks as registered.
- **partial**: C1's outputs unchanged and NC1 to NC4 met, but C4 missed, or a mutation of MU1 to MU4 does not make its check
  fail, or the step check rejects an output that an existing test (C1, C2) or PC2 accepts (a finding: recorded with its
  numbers, the test not weakened, the check not loosened).
- **todo**: an output changed (a C1 number or the M0 hash differs, or a C2 record fails for another reason than a rejection),
  or a negative control accepted (NC1 to NC4); such a change is not merged.

MU5 and MU6 are reported and do not count.

## Limits of this item

- The check runs after events only. A plane motion (`set_plane`, `move_plane_with_band`) is not a remeshing step and is not
  checked; a motion that inverts a cell is seen by the next event's (A) if the event does not remove the cell, and by the
  caller's quality check (P-T's Q) otherwise. `TetRemesher::new` is not checked either (Q2).
- (B) against the free faces is weaker than in `tetrahedralize`; (C) and (D) stand in for the independent surface. A defect of
  volume below the bound that keeps every free face on a plane and every face of at most two cells on opposite sides is not
  caught (ADR 0231's limit, Q2 there).
- (D) assumes the event moves no plane; an event that did would be rejected as "changed the region", which is the intent.
- The check reports the first defect only.

## Open questions and choices for the maintainer

- **Q1. Status of F-327.** Recommendation: by the rule, with the maintainer's acceptance.
- **Q2. Where to check.** Options: (a) events only (the acceptance's "remeshing step"); (b) also `TetRemesher::new`; (c) also
  after every motion. Taken for this branch on the maintainer's standing instruction of 2026-11-11 (take the recommendation):
  (a). (b) and (c) would add error paths to calls that the acceptance does not name; (c) costs a check per time step.
- **Q3. The reference of (B).** Options: (a) the free faces plus (C) and (D) (this design); (b) a stored boundary surface
  updated by every operator (a larger change of the remesher, and it would itself be derived from the cells). Taken on the
  maintainer's standing instruction of 2026-11-11 (take the recommendation): (a). It changes no default result.
- **Q4. The state after a failed step.** Options: (a) unchanged (the last checked mesh); (b) the rejected output committed.
  Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (a). It changes only the error path,
  which no existing test reaches.
- **Q5. The inner message prefix.** `check_output`'s text starts `tetrahedralize output check:` also when it checks a remesher
  output (wrapped after `remesh output check:`). Options: (a) leave it (F-324's tests match its text); (b) a prefix parameter.
  Taken (a) on the same standing instruction; no result depends on it.

## Addendum 1: results (2026-11-12)

Commits on `f327-remesher-output-check` (from main d1591e54): the registration `3edc6ed9`; the check and its tests `351f6fd9`
(`crates/rcfd-mesh/src/remesh.rs`: `StepCheck`, `RemeshChange::check`, `TetRemesher::to_tet_mesh`, `TetRemesher::check_output`,
the step check at the end of `event()` through `run_event`, the test module `tests_f327`; `crates/rcfd-mesh/src/tetmesh.rs`: the
`pub(crate)` helper `cell_volume_sum`). No other file changed; `Cargo.lock` unchanged; no `unsafe`; no existing test edited;
`baseline_hashes.rs` and every hash table and golden untouched. No amendment was needed.

All runs in release (`cargo test --release`), Windows, the worktree's own target dir, each once: `cargo test --release -p
rcfd-mesh --no-fail-fast` (C2, PC1, PC2, NC1 to NC4); `cargo test --release -p rcfd-fv --test remesh_f247` (C1), at the same
time as the first; then C4 alone (`--ignored --exact remesh::tests_f327::c4_cost_of_the_step_check`).

### Results

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| C1 P-T | pass; M0 hash, 1711 cells; the ADR 0215 numbers; no event error | P-T and N2 passed; `aaf8f4c36f6446e4`, 1711 cells, 522 points; A1 `3.692e-6`, A2 `9.447e-6`, 1806 events, operators 435 / 454 / 337 / 5 / 2086, cells 729 and 1860, 55497 steps, smallest mean ratio 0.2407; also B1v to B1e, B2s, B3, L1, R2, N1, N2's `t = 5.199` and the mass drift `3.228e-12` as recorded; all 1806 events passed the step check (P-T's R1). Wall 217.8 s against 193 s recorded (another test binary ran at the same time; not a check) | yes |
| C2 suites | pass | 22 test binaries and the doctests: 243 passed, 0 failed, 49 ignored (the lib: 70 passed, 2 ignored, U1 to U3 among them; `tetmesh` 10, `polydual` 3, `tetmesh_fold_f306` 19, `tetmesh_check_f324` 9 and every other binary passed) | yes |
| PC1 | `Ok`, check `Some`, within the bounds | `Ok`, one relocation; (B) difference 0 (bound `5.05e-17`), (D) difference 0 (bound `1.25e-17`) | yes |
| PC2 | every step `Ok`; at least 5 events | 1200 steps, 572 events, no error; largest `abs(difference) / bound` (B) `9.05e-3`, (D) `4.35e-2` | yes |
| NC1 dup | `Err`, located, state unchanged | "face [61, 62, 116] at (8.29e-2, 1.72e-2, 8.33e-3) is shared by the cells [530, 541, 542]" (by (B): difference `4.679e-6` against `5.05e-17`); state unchanged | yes |
| NC2 flip | `Err` naming the cell by its output index | "cell 541 [62, 116, 146, 61] at (8.53e-2, 2.03e-2, 1.54e-2) is not positively oriented" (`k = 541`); state unchanged | yes |
| NC3 gap | `Err` naming a free face on no plane | work cell 1 removed: "the free face [146, 160, 152] of cell 26 at (6.77e-2, 3.90e-2, 4.90e-2) lies on no plane"; state unchanged | yes |
| NC4 region | `Err` by (D); difference `1e-5` to `1e-9` | "the cell volumes sum to 3.010000000000000e-3 after the event but to 3.000000000000000e-3 before it ... the event changed the region"; difference `1.000000000000003e-5`, relative `2.4e-15`; state unchanged | yes |
| C4 cost | at most 25 percent | 572 events: events 1.776 s, check 0.184 s, fraction `0.103` | yes |

Every message starts `remesh output check: an event of OpCounts { .. } touching (lo) to (hi):`, as registered.

Predictions: PR1 held. PR2 (5 to 20 percent) held: 10.3 percent. PR3 held (NC1 by (B) with a face of three cells, NC3 by (C)).
PR4 held: with V2 and V3 removed (MU6) piston G met no step check error and gave the same numbers (572 events, the same
ratios), so V1 and V4 alone decided every candidate of that run.

### Mutations (scratch copy `w49i-work/f327-mut`, a `git archive` of `351f6fd9`, target dir `target-f327-mut`, release)

Each applied alone by `w49i-work/f327-mut-tools/mutate.py` (exact text replacement, each pattern found once), the
`tests_f327` lib tests run once with `--no-fail-fast`, the file restored and its SHA-256 checked against the kept copy
(`c52dc129...65e1`, equal after every mutation). Two first starts of the script stopped on a Python error before writing the
file (the checksum was equal afterwards, no test ran) and it was started again from MU1.

| Id | Must | Result | Caught by |
| --- | --- | --- | --- |
| MU1 check not called | NC1 to NC4 fail | all four failed ("accepted"), and PC1, PC2 failed (no check carried) | NC1 to NC4 |
| MU2 (C) removed | NC3 fails | NC3 failed: the gap was then found by (D) ("difference -1.527e-5 ... the event changed the region"), not as a face on no plane; the others passed | NC3 |
| MU3 (D) removed | NC4 fails | NC4 failed: accepted, (B) difference `-4.3e-19` (the longer box passes (A) to (C)) | NC4 |
| MU4 commit before the check | state part of NC1 to NC4 fails | all four failed on "state unchanged" (each still returned its located error) | NC1 to NC4 |
| MU5 bound of (D) 0 (reported) | PC1 or PC2 fails if the sums differ | PC2 failed at `x = 0.567`: an event of one split and one removal whose sums differ by `-8.7e-19`; PC1 passed (its sums equal) | PC2 |
| MU6 V2, V3 removed (reported) | whether PC2 meets an error | none (see PR4) | not caught |

### Format and lint

`cargo fmt -p rcfd-mesh -- --check`: clean. `cargo clippy -p rcfd-mesh --all-targets -- -D warnings`: clean. `rcfd-fv` was not
edited (its test ran unchanged in C1).

### Deviations and what was not run

- An error of the registration's paper arithmetic, not of a limit: M_s's volume is `0.3 x 0.1 x 0.1 = 3e-3` (the text said
  0.03), so NC4's `1e-5` is `3.3e-3` of the volume, not `3.4e-4`; still twelve orders above the bound.
- C1 ran at the same time as C2 (its wall time is not a check). C4 ran alone.
- Not run: the whole workspace suite and the gate (the lead runs it); `rcfd-api` and `rcfd-cli` (no path from a case to the
  remesher, ADR 0215 S3); Linux.

### Decision by the registered rule

C1, C2, PC1, PC2, NC1 to NC4 and C4 met; MU1 to MU4 failed their checks as registered: **done**, with the maintainer's
acceptance (Q1). The acceptance's parts: every output of a remeshing step passes `TetMesh::check_output` (on the free-face
surface, with (C) and (D)) or the step returns its located error (NC1 to NC4, the state left as it was); the remesh_f247 record
(M0's hash) and every P-T number are unchanged (C1).

### Remaining questions

- **Q1. Status.** Options: (a) done; (b) partial until the gate. Recommendation: (a), with the gate line added at merge.
- **Q2. Where to check.** Taken (a) events only. Options for later: (b) `new()`, (c) after motions. Recommendation: keep (a);
  callers that move planes can call the public `TetRemesher::check_output()`; (b) only if a caller builds remeshers from meshes
  not made by `tetrahedralize`.
- **Q3 to Q5** as registered, taken on the standing instruction; nothing measured argues against them.
- **Q6. MU6.** V2 and V3 of the operators' validity were not decisive in piston G. Options: (a) leave; (b) a unit test that
  makes V2 or V3 decisive. Recommendation: (a); the step check now catches what they would let through.

### For the lead: roadmap entries

- **F-327**: status `done`. New note:

  > From ADR 0231 question 2, recommendation taken on the maintainer's standing instruction of 2026-11-11. DONE (ADR 0236, the registration of F-327, 2026-11-12, branch f327-remesher-output-check): every registered check is met at the first run; the acceptance is still the maintainer's (ADR 0236 Q1). TetRemesher::event (and remesh) ends every event that applied an operator with a step check before committing: TetMesh::check_output on the output with the surface of its free faces (the remesher has no stored surface; its boundary is re-triangulated), every free face exactly on its plane, and the cell volume equal to that of the step's input within a rigorous bound (an event does not change the region); a failure returns a located Error::Mesh ("remesh output check: ...", naming the cell, the face or the volumes, and the event's operators and box) and leaves the mesh as it was. The check only reads: M0's hash aaf8f4c36f6446e4 and every P-T number of ADR 0215 are unchanged, and all 1806 P-T events and the 572 events of a geometric piston pass. A duplicated cell, a negative cell, a missing cell and a valid tiling of a changed region are each rejected and located. Cost: 10.3 percent of an event. Four registered mutations failed their checks. Library only; RemeshChange gains the field check. Open: ADR 0236 Q2 (checking new() and motions), Q6 (V2 and V3 not decisive in the piston).

- No follow-up item proposed.
