# ADR 0203: F-290 registration: inverse-volume mesh smoothing in the case tree

Status: **REGISTERED 2026-11-06**, written before any code of F-290 (the one test run made before it is disclosed below). Not
yet accepted by the maintainer. Results go in an addendum at the end.

Parents: [ADR 0090](0090-wave38c-dynamic-mesh-layering.md) D4 and its checks S1 to S3 (the library stiffness
`SmoothingMethod::InverseVolume(k)` of F-230), [ADR 0033](0033-wave20b-dynamic-mesh.md) D5 and D8 (displacement-diffusion
smoothing and the `dynamic_mesh` section), [ADR 0191](0191-f259-registration-case-tree-and-cli-wiring-of-layering-and-compressible-ale.md)
Q2 and its decision record of 2026-11-06 (answer (b): a third value `inverse_volume` of `dynamic_mesh.smoothing.method`, exponent
from `smoothing.exponent`, as a separate small item). Roadmap item F-290 (depends on F-230 and F-063, both done).

Read at main 9edfbda, on branch `f289-f290` after F-289 (ADR 0202), which does not touch the dynamic mesh. Number: assigned by
the caller.

House rules: limits are fixed here before the code and the runs; a miss becomes an ignored `*_known_miss` test with the limit
intact and the measured number recorded; a defective set-up is corrected by an amendment written before the corrected run;
defaults stay bit-identical; a new enumeration value changes no settings hash of an existing case;
`crates/rcfd-api/tests/baseline_hashes.rs` and `case_hashes_w48u.rs` are not edited; no new third-party crate; no commit
trailers. Clean-room: first principles and our own ADRs only.

## What F-290 asks

The roadmap entry: a third value `inverse_volume` of `dynamic_mesh.smoothing.method`, with its exponent from
`dynamic_mesh.smoothing.exponent` (an enumeration value, no new key; ADR 0191 Q2 (b)), runs the `InverseVolume(k)`
displacement-diffusion stiffness of ADR 0090 D4, today in the library only: invalid settings give located errors, a tree run
equals the library run to a limit fixed by its registration ADR, and a case without the value is bit-identical (baseline hashes
unchanged). The caller adds: the smoothing improves or at least preserves the minimum cell quality and volume ratio on a
registered moving-wall case against the default method, with a limit registered before the run, and a mutation that must fail.

## What the code read shows (read only)

1. **The library.** `rcfd_fv::smoothing::smooth_displacement(geom, classes, prescribed, method, control)` solves
   `div(gamma grad d) = 0` per active component. `InverseVolume(k)`: `gamma_f = V_f^-k` with `V_f` the mean of the owner and
   neighbour cell volumes of `geom` (the owner's on boundary faces). ADR 0090 measured it (S1 `6.8e-17`, S2 smallest-cell strain
   ratio `0.179` against the Laplacian on a graded 1-D compression).
2. **The tree.** `rcfd-api/src/dynmesh.rs`: the schema field `smoothing.method` is
   `Schema::enumeration("laplacian", &["laplacian", "inverse_distance"])`; `smoothing.exponent` is a float, default 1, range 0 to
   10, documented as the exponent of the inverse-distance diffusivity; `check` refuses a negative exponent at its key and a
   `substeps` other than 1. `DynMesh::from_case` maps `inverse_distance` to `InverseDistance(exponent)` and anything else to
   `Laplacian` (an `if`), and `new_points` calls `smooth_displacement` on `geom0`, the geometry of the initial mesh, every
   step (the displacement from the initial mesh, ADR 0033 D5). So the inverse-volume stiffness uses the initial cell volumes.
   The same `new_points` serves the incompressible step (`DynMesh::step`) and the compressible one (`compressible_geometry`,
   ADR 0191 W4). The control of the solve is `tol_abs = 1e-16`, `tol_rel = max(tolerance, 1e-14)`, `max_iter = 5000`.
3. **The settings hash** (`SettingsTree::content_hash`) hashes the data, not the schema, so a new enumeration value changes the
   hash of no case that does not use it; `Case::to_toml` writes the value a case holds. ADR 0191 added two values the same way
   with no edit of H1 or `baseline_hashes.rs`.
4. **No other list.** No other source file, Studio file or document matches on the smoothing method names
   (`grep inverse_distance`: only `dynmesh.rs` and the documents for this key).
5. **The smoothing keys are inert** unless `deformation = "smoothing"` (schema text "Interior mesh smoothing (deformation =
   smoothing)"); with `rigid` or `layering` no smoothing is solved and no key of `smoothing` is refused except by the checks of
   item 2.
6. **Graded generated meshes.** `box_2d` with `mesh.grading = g` is graded symmetrically in x and y (small cells at both ends,
   `Grading::Symmetric(g)`); with `ny = 2` the y spacing is uniform.

## Run before this registration (disclosed)

The test D0 below was written as the new file `crates/rcfd-api/tests/inverse_volume_f290.rs`, with no source change of F-290,
and run once on the branch at `ca0fb7b` (F-289 only; Windows, release). It printed the settings content hashes:

| Case text | Content hash |
|---|---|
| ADR 0033's piston, `piston("", "")` of `dynamic_mesh.rs` | `536d14dc8993d75b` |
| P-I without a smoothing table (method laplacian by default) | `bd8183148b7a2d8c` |
| P-I with `method = "inverse_distance"`, `exponent = 2` | `2812b71792835d54` |
| P-V without a smoothing table | `836d0833059d0d2b` |
| ADR 0191's P-B | `27a988665a8f9dae` |

No other run was made.

## Decisions

**W1. The value.** `dynamic_mesh.smoothing.method` gains `inverse_volume`; the schema becomes
`Schema::enumeration("laplacian", &["laplacian", "inverse_distance", "inverse_volume"])` with its text naming `1/V^exponent`.
The text of `smoothing.exponent` names both inverse methods. No key, default, range or check changes.

**W2. The mapping.** `DynMesh::from_case` maps the method by a `match`: `inverse_distance` to `InverseDistance(exponent)`,
`inverse_volume` to `InverseVolume(exponent)`, anything else to `Laplacian` (the existing arms unchanged). The stiffness is that of
the initial mesh, as the displacement is (item 2); no other code changes. Both the incompressible and the compressible moving
paths use it through `new_points`.

**W3. Refusals.** None new (Q1). The value goes through the existing located checks: an unknown name, an exponent outside 0 to
10 (schema), a negative exponent and `substeps` other than 1 (`check`).

**W4. Separable commits.** (1) this text (with D0's test file); (2) the code (W1, W2) with the tests; (3) documents and roadmap;
(4) the addendum.

## Checks and limits (fixed here, before any code)

Set-ups (case text inline in `crates/rcfd-api/tests/inverse_volume_f290.rs`; no case file, fixture or docs TOML block is added):

* **P-I**, the moving-wall case: ADR 0033's piston (`dynamic_mesh.rs`: `box_2d`, `nx = 20`, `ny = 2`, `lx = 1`, `ly = 0.2`,
  `nu = 0.01`, `rho = 1`, `xmin` the piston, `xmax` a zero-pressure outlet, `piso`, `central`, `dt = 0.005`, `end_time = 0.5`,
  `deformation = "smoothing"`, `moving_patches = ["xmin"]`, `fixed_patches = ["xmax"]`,
  `displacement_expression_x = "1 [m/s^2] * t^2"`: 100 steps, the piston advances 0.25 m into the mesh) on a graded mesh,
  `mesh.grading = 8` (cells 8 times smaller at both x ends than in the middle). The method lines go in
  `[dynamic_mesh.smoothing]`.
* **P-V**, the compressible moving wall: ADR 0191's P-B (`box_2d`, `nx = 20`, `ny = 2`, `lx = 1`, `ly = 0.1`, HLLC, van Leer,
  SSP-RK3, CFL 0.4, `end_time = 1`, `xmin` a static pressure outlet at 1, `xmax` the piston, `fixed_patches = ["xmin"]`,
  `displacement_expression_x = "-0.2 [m] * (1 - cos(pi * t / 2 [s]))"`) with `mesh.grading = 8`.

**The library points** of a step of P-I at time `t` (the time of the step's summary): `p0 + smooth_displacement(geom0, classes,
presc(t), InverseVolume(k), control).displacement`, with `geom0 = FvGeometry::new` of the tree's initial mesh, `classes`
`Prescribed` for `xmin`, `Fixed` for `xmax`, `Slip` for the others, `presc` the pose of ADR 0191's library run (the case's
expression compiled with `rcfd_core::expr` in `rcfd_api::exprs::spatial_context`, the tree's operations) on the points of
`xmin`, and the tree's control (`1e-16`, `1e-12`, `5000`). **The library run** of P-V is ADR 0191's loop for P-B
(`CompressibleSolver`, `step_ale` onto `moved_mesh` of the smoothed points, the tree's schedule) with `InverseVolume(1)`.

Quality measures of a mesh (`rcfd_mesh::quality::check_mesh`): `min_volume`, `max_volume_ratio` (largest ratio of the two cell
volumes of an internal face), `max_aspect_ratio`.

| Id | Check | Limit |
|---|---|---|
| V1 | P-I with `method = "inverse_volume"` (exponent 1 by default): after each of the 100 steps, every point of `Simulation::geom` against the library points | equal bits |
| V1b | P-I with `inverse_volume`, `exponent = 2`: the same against `InverseVolume(2)`; and its final points against V1's | equal bits; max distance `> 1e-9` m |
| V2 | P-V with `inverse_volume`: tree run against its library run | the same steps, cell counts and end time; differences of `rho`, `p`, `u` all 0 (equal bits) |
| V3 | P-I with `inverse_volume` and `exponent = 0` against P-I with `laplacian` (`V^-0 = 1` exactly) | equal bits of every point after every step |
| Q1 | P-I, `inverse_volume` against the default method (`laplacian`), after every step: `min_volume` | ratio `>= 1` |
| Q2 | the same: `max_volume_ratio` | `inverse_volume <= laplacian` |
| Q3 | the same: `max_aspect_ratio` | `inverse_volume <= laplacian` |
| T | P-I at the end: the largest distance between a point of the `inverse_volume` mesh and the same point of the `laplacian` mesh | `> 1e-6` m (Q1 to Q3 are not met by two equal meshes) |
| D0 | the five hashes of the table above | equal |
| D1 | P-I with `method = "laplacian"` written against P-I without the line: settings content hash; the bits of every point and of `ux` after the run | equal |
| D2 | `rcfd-api`: `baseline_hashes.rs`, `case_hashes_w48u.rs`, `dynamic_mesh.rs`, `layering_ale_f259.rs`, `exp_rot_pab.rs`, `exp_rot_pc.rs`; `rcfd-cli`: `layering_ale_f259.rs`; `rcfd-fv`: `layering_w38c.rs`, `mesh_smoothing.rs` | pass unedited |
| L | located errors, each one change of P-I with `inverse_volume`: `method = "inverse_volumes"` at `dynamic_mesh.smoothing.method`; `exponent = -1` and `exponent = 11` at `dynamic_mesh.smoothing.exponent`; `substeps = 2` at `dynamic_mesh.smoothing.substeps`. Controls with no diagnostic: P-I with `inverse_volume` and exponents 0, 1 and 10; P-V with `inverse_volume` | as stated |

Q1 to Q3 are the "improves or at least preserves" of the caller, on the moving-wall case P-I; their limits are directions, not
amounts. Reported without a limit: the three measures of the initial mesh and of the last step of both runs, the same measures
for P-V at its end time (against P-V with `laplacian`), and V2's step count.

**Can fail.** Two mutations, each made, its test run, the file restored and compared byte for byte with the kept copy: M1, the
`inverse_volume` arm of W2 maps to `Laplacian`: V1 and T must fail. M2, the arm passes `1.0` in place of the exponent: V1b must
fail.

`scripts/limit_audit.py` is run on the new test file.

**Gate.** The one gate of F-289 and F-290 together (ADR 0202): fmt per package, clippy `--workspace --all-targets -D warnings`,
every test in release on a `git archive` copy of the gated commit (`scripts/gate_test.py`, one process per test, three jobs).

**Decision rule.** Every check met: F-290 is done. A miss: an ignored `*_known_miss` test with the limit intact and the number
recorded, and F-290 stays open with the miss named.

## Open questions

**Q1. Refuse an inverse method outside smoothing?** With `deformation = "rigid"` or `"layering"` the method is inert (item 5).
(a) No new refusal, as for `inverse_distance` and the other smoothing keys today. (b) Refuse `inverse_volume` there at
`dynamic_mesh.smoothing.method` (new value only, no existing case changes). (c) Refuse both inverse methods there (an existing
valid case with `inverse_distance` and `rigid` would be refused). Recommendation (a): the section is documented as inert outside
smoothing. Proceeding on (a).

## Not in F-290

A stiffness from the current (moved) volumes or an incremental smoothing (the displacement stays solved from the initial mesh),
other stiffness laws, substeps, layering or smoothing on new paths, a Studio list (none exists for this key).

## Where to go next

* [ADR 0090](0090-wave38c-dynamic-mesh-layering.md) D4 and S1 to S3 for the stiffness and its library checks.
* [Dynamic mesh](../dynamic-mesh.md) and [case schema](../case-schema.md) for the keys.

## Addendum 1: what was built and the results (2026-11-06)

Branch `f289-f290`, after this text and D0's test file (`d7154bd`): the code with the tests (`ed1e512`), the documents (`36b8e7a`).
Not merged.

**Built as registered.** W1 to W4 as written. `crates/rcfd-api/src/dynmesh.rs`: the value `inverse_volume` and the texts of
`method` and `exponent` (W1), the `match` of W2. No key, default, range or check changed; no Studio file changed. The test file
`crates/rcfd-api/tests/inverse_volume_f290.rs` gained the checks (6 tests: D0; V1 with V1b, Q1 to Q3 and T; V3; V2; D1; L); D0
now asserts the recorded hashes. No existing test was edited. Documents: the two `smoothing` rows of `docs/case-schema.md`, the
method and stiffness paragraphs of `docs/dynamic-mesh.md` (no TOML block added), the F-290 entries of `docs/ROADMAP.md` and
`docs/roadmap.json` (done, by the decision rule).

**Results.** Release build, Windows. P-I takes 100 steps with each method.

| Check | Measured | Limit | Verdict |
|---|---|---|---|
| V1 | 0 of 12600 step points (100 steps of 126 points) differ in a bit | equal bits | met |
| V1b | 0 differ; exponent 2 against 1 at the end 1.67e-2 m | equal bits; `> 1e-9` m | met |
| V2 | 314 tree steps and 314 library steps, the same cell counts and end time (1); 0 of 200 values differ in a bit | equal bits | met |
| V3 | 0 step points differ in a bit over 100 steps | equal bits | met |
| Q1 | the smallest ratio over the steps 1.000020 (the first step); at the end `6.764e-5 / 5.368e-5 = 1.260` | `>= 1` | met |
| Q2 | the largest excess over the steps -1.4e-6; at the end 1.2454 against 1.2599 (initial 1.2599) | `<= 0` | met |
| Q3 | the largest excess over the steps -1.4e-4; at the end 7.392 against 9.315 (initial 6.986) | `<= 0` | met |
| T | 2.45e-2 m | `> 1e-6` m | met |
| D0 | the five hashes as recorded | equal | met |
| D1 | settings hash `bd8183148b7a2d8c` both; 0 points and 0 `ux` values differ in a bit | equal | met |
| D2 | `rcfd-api`: `baseline_hashes.rs`, `case_hashes_w48u.rs`, `dynamic_mesh.rs`, `layering_ale_f259.rs`, `exp_rot_pab.rs`, `exp_rot_pc.rs`; `rcfd-cli`: `layering_ale_f259.rs`; `rcfd-fv`: `layering_w38c.rs`, `mesh_smoothing.rs`: all pass unedited (`baseline_hashes.rs` 3 passed, 1 ignored) | pass | met |
| L | `inverse_volumes` at `dynamic_mesh.smoothing.method`; exponents -1 and 11 at `dynamic_mesh.smoothing.exponent`; `substeps = 2` at `dynamic_mesh.smoothing.substeps`; the four controls give none | as stated | met |

Reported without a limit: the initial P-I mesh has `min_volume` 7.157e-5, `max_volume_ratio` 1.2599 and `max_aspect_ratio`
6.986; the smallest cell ends at 0.945 of its initial volume with `inverse_volume` and at 0.750 with `laplacian` (a uniform
strain: the piston advances a quarter of the length). P-V at its end: `inverse_volume` `min_volume` 3.421e-5, volume ratio
1.2484, aspect ratio 3.654, in 314 steps; `laplacian` 2.863e-5, 1.2599, 4.366, in 332 steps (the explicit step follows the
smallest cell).

**Can fail.** Each file was restored afterwards and compared byte for byte with the kept copy (equal each time).

* M1 (`inverse_volume` mapped to `Laplacian`): V1 failed (11120 of 12600 step points differ) and T failed (0 m); Q1 to Q3 were
  met with ratio 1 and excess 0, as the registration expects, so T is the check that catches it.
* M2 (`InverseVolume(1.0)` in place of the exponent): V1b failed (10938 step points differ; exponent 2 against 1, 0 m); V1
  passed.

`scripts/limit_audit.py` reports no hit in the new test file.

**Gate.** `387236a` (the code, the documents and addendum 1 of ADRs 0202 and 0203, on main 9edfbda), `git archive` copy, three jobs (`scripts/gate_test.py`, one process per test), one gate for F-289 and F-290 under the machine lock: fmt 0, clippy `--workspace --all-targets -D warnings` 0, tests 2938 passed, 2 failed, 822 ignored, 2940 test processes in 569 binaries, 85 min of tests, peak summed working set 3.6 GB. The run waited for memory from 15:59 to 16:54 under the 30 GB free threshold of the gate script; on the coordinator's instruction the wait was restarted with 20 GB (the lock kept) and the gate ran at once. Every test of F-289 and F-290, `baseline_hashes.rs`, `case_hashes_w48u.rs` and `rules_w19c.rs` passed. The 2 failures are the known flaky tests of `rcfd-runner` `runner_w47b.rs`, a crate F-289 and F-290 do not change:

* `s1_concurrency_never_exceeds_max_runs` ("the sampler does not see an over-subscription of 2"): rerun alone on the gate's binary it failed 3 of 3; control: the same test from the binary of the ADR 0196 gate (`1c86f24`, without this work) failed 2 of 2 right after, with the same message;
* `s9_memory_and_thread_budget_refusals_are_located` ("two 60 MB runs ran together under a 100 MB budget"): rerun alone on the gate's binary it failed 1 of 3 and passed 2 of 3.

This paragraph is documents only and was not gated again.

**Questions for the maintainer.** Q1 stands as registered (proceeding on (a)).
