# ADR 0107: Wave 42C, the tetrahedral quality pass as a default? (F-131, F-132)

Status: registered before the experiment, the timing run and the registered tests were run. Limits and the decision rule are not
changed after a run; a missed limit becomes an ignored `*_known_miss` test with the limit intact. The main tree keeps
`TetOptions::quality_sweeps = 0` unless the decision rule below is met. Written at HEAD 579be92, target `<target dir>`.
Clean-room: first principles only; no literature value enters a limit.

## Context

ADR 0104 (wave 41D) added an opt-in quality pass to `rcfd_mesh::tetmesh::tetrahedralize` (`quality_sweeps`, default 0): per sweep,
constrained circumcentre insertion for tetrahedra over the radius-edge bound, interior-vertex smoothing checked with exact
`orient3d`, and flips of inside cells below volume quality 0.06; at most `quality_sweeps` sweeps, stops early when a sweep changes
nothing. With 10 sweeps the cylinder-in-box STL mesh goes from minimum dihedral 1.84 deg to 17.66 deg and radius-edge fraction
0.936 to 1.000. The user decision of 2026-10-22 (F-131 note): evaluate making the pass the default, only if every existing
baseline hash stays identical.

## Who produces a tetrahedral mesh by default (enumerated at 579be92)

`tetrahedralize` has no caller in any library or binary source: grep over `crates/*/src` finds only its definition, and
`rcfd_mesh::polydual::tet_to_polydual` only consumes a `TetMesh`. `rcfd-api` (including `tests/baseline_hashes.rs`), `rcfd-cli`,
the three `examples/` programs, and all doctests (the only code blocks in `rcfd-mesh/src` are in `motion.rs`) do not reach
`tetmesh`. So `baseline_hashes.rs` cannot change whatever the default; it is still run in the experiment. No `docs/*.md` is pulled
into code by `include_str!`. `docs/meshing.md` states the default (0) in prose.

Tests that call `tetrahedralize` with options taken from `TetOptions::default()` (all fields, or `size` only):

| file | tests |
| --- | --- |
| `crates/rcfd-mesh/tests/tetmesh.rs` | t1_cube, t2_sphere, t3_torus, t4_cylinder_in_box, t5_quality_cube_sphere, t5_torus_known_miss (ignored), t6_size, t7_determinism, t8_located_errors (errors before the pass), p1_pipeline_cylinder_in_box, t5b_torus_after_remesh |
| `crates/rcfd-mesh/tests/polydual.rs` | a5_dual_of_tetrahedralised_sphere |
| `crates/rcfd-fv/tests/w19b_meshing_solve.rs` | t10_sphere_harmonic, t10_cube_linear |
| `crates/rcfd-io/tests/stl_cylinder_in_box.rs` | s2b_pipeline_watertight_and_check_mesh, s2_known_miss_pipeline_watertight_and_quality (ignored), s3_known_miss_radius_edge_fraction (ignored) |
| `crates/rcfd-io/tests/w41d_tet_quality.rs` | w0_default_output_bit_identical; d012_numbering_dependence and d3_bad_cells (ignored diagnostics) |

The w41d Q1 to Q7 tests set `quality_sweeps: 10` explicitly and do not depend on the default.

Equality-asserted literals among them: the nine W0 fingerprints (cell count and `Mesh::content_hash`); the t8 error substrings; the
invariants t1 to t4 and p1 assert (4 faces per cell, Euler numbers, 2 patches). No other test asserts a recorded count or hash of a
tetrahedral mesh.

## Smoke (run before this ADR was written; default options only, the pass was not run)

Finer candidate set-ups for the "about 10 times more cells" mesh: the ADR 0093 STL path with a finer curvature remesh, default
`tetrahedralize`, cells:

| theta, h_min, h_max | surface triangles | cells |
| --- | --- | --- |
| 0.5, 0.1, 1.0 (the F-131 case) | 372 | 1115 |
| 0.25, 0.05, 0.5 | 1378 | 3499 |
| 0.2, 0.04, 0.4 | 2010 | 4439 |
| 0.15, 0.03, 0.3 | 3364 | 6866 |
| 0.12, 0.025, 0.25 | 5496 | error: the remeshed surface has 2 degenerate triangles |
| 0.1, 0.02, 0.2 | 8164 | 16868 |
| 0.15, 0.03, 0.2 | 5564 | 10710 |
| 0.15, 0.03, 0.15 | 9058 | 16855 |

Selection rule (fixed before the table was read): the candidate whose default cell count is closest to 10 x 1115 = 11150 by
ratio. Chosen: theta 0.15, h_min 0.03, h_max 0.2 (10710 cells, 9.6x). Side finding, not pursued here: the remesher produced
degenerate triangles at theta 0.12, h_min 0.025, h_max 0.25 (a remesher defect, not a tetrahedral-mesher one).

## Experiment (registered)

Two scratch trees from `git archive` of the HEAD that carries this ADR and the registered tests: BASE unchanged, EXP with the one
line `quality_sweeps: 0` -> `quality_sweeps: 10` in `impl Default for TetOptions`, plus (scratch only) W0 changed to print all nine
fingerprints instead of stopping at the first mismatch.

* E1: in both trees, the five test binaries above (`rcfd-mesh` tetmesh and polydual, `rcfd-fv` w19b_meshing_solve, `rcfd-io`
  stl_cylinder_in_box and w41d_tet_quality) with `--include-ignored --nocapture --test-threads=1`, release; diff the outputs. Every
  test whose result or printed numbers differ is listed.
* E2: in EXP only, `cargo test --workspace --release --no-fail-fast` (non-ignored), including `rcfd-api` baseline_hashes.

## Decision rule (registered before E1 and E2)

The default switches to 10 only if none of these blocks:

* B1: a non-ignored test fails in EXP (W0 excepted, see below).
* B2: an equality-asserted literal changes (W0 excepted); `baseline_hashes.rs` fails.
* B3: a `tetrahedralize` call that succeeds with the old default returns `Err` with the pass (budget exhaustion in
  `refine_constrained` is the pass's only new error path).
* B4: a recorded measured value of a test that is not a default-vs-pass record changes. Classified now, by name:
  `t5_torus_known_miss` (ADR 0030, written before the pass existed; its ignore reason records 0.634) blocks if the fraction it
  prints changes at 3 decimals. Its text gives no ground to treat it as a record of "the default mesher without the pass".
* B5: a cost or safety limit L1 to L3 below is missed.

Exempt, as default-vs-pass records:

* W0 (`w0_default_output_bit_identical`): its registered purpose (ADR 0104) is that the mesher without the pass stays
  bit-identical. Decision: its nine fingerprints are NOT re-recorded. If the default switches, W0 is pinned to
  `quality_sweeps: 0`, so the same nine fingerprints keep guarding the mesher without the pass. The number of fingerprints that
  change under the new default is reported.
* `s2_known_miss_*`, `s3_known_miss_*` (ADR 0093): their doc text since ADR 0104 says "this record keeps the default mesher; the
  same limits with the opt-in quality pass pass in w41d". If the default switches they are pinned to `quality_sweeps: 0`, so the
  recorded misses (1.84 deg, 0.936) stay true; nothing is re-recorded.
* `d012_numbering_dependence`, `d3_bad_cells` (ADR 0104 diagnostics of the mesher without the pass): pinned to 0 likewise.

Listed, not blocking: printed numbers of limit tests that still pass (t1 to t7, p1, t5b, polydual sphere, T10, s2b), and
measurements recorded in ADR text that would no longer reproduce with default options (ADR 0030 T5/T6/T10/P1 tables, ADR 0093
S2b, ADR 0104 smoke and D tables); these would need an addendum in their ADR. `baseline_hashes.rs` is never re-recorded.

If the rule is met, the main tree gets: the `Default` change, the pins above, the `TetOptions` field doc, the module doc and the
`docs/meshing.md` default line. If not, the default stays 0 and this ADR lists the blocking tests and what would be needed.

## Cost and safety limits (registered tests in `crates/rcfd-io/tests/w42c_tet_default_eval.rs`)

Meshes: M1 = the F-131 STL path (curvature remesh theta 0.5, h_min 0.1, h_max 1.0); M10 = the same STL file remeshed with theta
0.15, h_min 0.03, h_max 0.2 (smoke above). "Pass" = `quality_sweeps: 10`, other options default.

* L1 cost factor (policy bound, not an accuracy limit): median wall time of `tetrahedralize` with the pass over median without,
  5 alternating repeats each in one process, release, single test thread: at most 3.0 on M1 and on M10. Reason for 3.0: a default
  that triples the mesher's time is the most I accept without a user decision; it is a policy number.
* L2 scaling: added time per output cell, (median pass time - median default time) / pass cell count, on M10 at most 2.0 times
  that on M1 (the pass is meant to be linear in the cell count per sweep; a factor above 2 at 9.6x cells would show superlinear
  growth). Policy bound.
* L1 and L2 run in an ignored test (`l12_cost_factor_and_scaling`, wall-clock time is load-sensitive and other agents share the
  machine); run once, reported with the machine load caveat.
* L3 safety, non-ignored (`l3_pass_safety_m1_m10`): on M1 and M10 with the pass, `tetrahedralize` returns `Ok`; every output cell
  has exact `orient3d > 0` (no volume sign flip); every `to_mesh` cell volume is positive; `check_mesh` ok; total volume equal to
  the remeshed surface volume within 1e-10 relative; 2 patches; surface triangle count of the output unchanged against the run
  without the pass (the pass never splits the surface).
* Q8 held-out quality (non-ignored, `q8_m10_t5_limits_with_pass`): M10 with the pass meets the ADR 0030 T5 limits (minimum
  dihedral at least 3.0 deg, radius-edge fraction at least 0.99). Informative for the default question; a miss does not by itself
  block the rule (it is not an existing test), it is reported.
* Reported, no limit: Steiner points used over the Steiner budget (`max_steiner_factor` x input vertices + 10000) for M1, M10 and
  every E1 case where the report is printed; pass counters (insertions, moves, flips).

Worst-case behaviour, from the code (read before the runs): the outer loop is capped at `quality_sweeps`; each sweep inserts at most
one point per bad inside cell found at the sweep start, so a sweep is bounded; `improve` is capped at 30 passes; smoothing moves only
interior vertices and only if exact `orient3d` keeps every incident cell positive, so it cannot invert a cell; the one error path is
the Steiner budget (`Err`, the whole call fails, not a partial mesh). There is no quality guarantee (no sliver exudation): a sweep
may leave bad cells.

## Amendment 1 (written after the first L1/L2 run, before the second)

The registered run of `l12_cost_factor_and_scaling` (main tree, default 0, release, one test thread, other agents' builds running
on the machine) measured on M1: default median 0.0909 s, pass median 0.2772 s, factor 3.051 > 3.0: L1 MISSED on M1. The test
asserted per mesh and stopped there, so M10 and L2 were not measured: a defect of the harness, not of the set-up. Correction:
the test is renamed `l12_known_miss_cost_factor_and_scaling` (ignored, limits 3.0 and 2.0 unchanged) and now measures M1 and M10
before asserting. It is run once more to get M10 and L2. The L1 verdict on M1 stays the first run's 3.051 (miss) whatever the
second run gives on M1; the second run's M1 value is reported but not used. What I knew when writing this: only the M1 numbers
above.

## Separate fix (done before this ADR, commit 579be92)

`crates/rcfd-io/tests/stl_cylinder_in_box.rs` wrote one fixed temp STL name from parallel tests (the same race that commit 9f05d6b
fixed in w41d after it failed a wave-41 gate run); it now writes one file per call, as w41d does.

## Results

### E1: every test whose output changes with the default at 10 (BASE vs EXP, five binaries, ignored tests included)

| test | BASE (default 0) | EXP (default 10) | class |
| --- | --- | --- | --- |
| w0_default_output_bit_identical | 9 fingerprints | 7 of 9 change: torus 899 -> 945 cells, cyl_in_box 1137 -> 1139, p1 1061 -> 1142, s2 1115 -> 1223, sphere h 0.3 823 -> 810, sphere h 0.25 1277 -> 1292, cube4 h 0.5 597 -> 711, each with a new hash; cube (108) and sphere (320) unchanged | exempt |
| t5_torus_known_miss (ignored) | fraction 0.634, 6.81 deg, fails | fraction 1.000, 25.80 deg, passes | **B4: blocks** |
| s2_known_miss_* (ignored) | 1115 cells, 1.84 deg, fails | 1223 cells, 17.66 deg, passes | exempt |
| s3_known_miss_* (ignored) | fraction 0.936, fails | 1.000, passes | exempt |
| d012_numbering_dependence, d3_bad_cells (ignored) | D0 1137 cells; D2 1061, 3.34 deg, 0.927; D3 77 / 71 failing cells | D0 1139; D2 1142, 16.33 deg, 0.997; D3 3 / 0 | exempt |
| t3_torus | 899 cells | 945 cells (52 moves) | listed |
| t4_cylinder_in_box | 1137 cells | 1139 cells (363 moves) | listed |
| p1_pipeline_cylinder_in_box | 1061 cells, 3.34 deg, 0.927 | 1142 cells, 16.33 deg, 0.997 | listed |
| t5b_torus_after_remesh | 1297 cells, 5.26 deg | 1265 cells, 16.73 deg | listed |
| t6_size | 823 cells | 810 cells (still within 0.3 to 3 of nominal 1272) | listed |
| t10_sphere_harmonic | 1277 cells, L2 1.098e-2 | 1292 cells, 9.70e-3 | listed |
| t10_cube_linear | 597 cells, L2 4.9e-14 | 711 cells, 4.4e-14 | listed |
| s2b_pipeline_watertight_and_check_mesh | 1115 cells | 1223 cells | listed |

Unchanged output: t1_cube, t2_sphere, t5_quality_cube_sphere, t7_determinism, t8_located_errors, a5 polydual sphere (the pass
inserts and moves nothing on the cube and the sphere). All non-ignored tests pass in both trees.

### E2: whole workspace in EXP

`cargo test --workspace --release --no-fail-fast`: 1961 passed / 0 failed / 578 ignored (the wave-41 gate was 1959 / 0 / 576; the
difference is this ADR's file: 2 passed, 2 ignored). `rcfd-api` baseline_hashes: 3 passed, 1 ignored (the pre-existing
rotating-channel record), unchanged. No test outside the five binaries and this ADR's file reaches `tetrahedralize`.

### Cost and safety (main tree, default 0)

| check | limit | measured | verdict |
| --- | --- | --- | --- |
| L1 M1 cost factor (registered run) | <= 3.0 | 3.051 (0.0909 s -> 0.2772 s) | MISS |
| L1 M10 cost factor (second run, amendment 1) | <= 3.0 | 4.810 (1.156 s -> 5.560 s) | MISS |
| L2 added time per cell, M10 / M1 (second run) | <= 2.0 | 2.787 (1.49e-4 s -> 4.15e-4 s per cell) | MISS |
| L3 M1 and M10 safety | Ok, no orient3d <= 0, volumes > 0, check_mesh, volume 1e-10, 2 patches, surface unchanged | all hold; volume error 8.0e-16 both | pass |
| Q8 M10 T5 limits with the pass | >= 3.0 deg, >= 0.99 | 14.19 deg, 1.000 (10614 cells) | pass |

The second run gave M1 3.080; the registered L1 verdict on M1 is the first run's 3.051 (amendment 1). Both runs had other agents'
builds on the machine; wall-clock numbers carry that caveat. L1 on M1 is within the repeat spread (per-repeat factors about 2.8
to 3.2 in the first run): a marginal miss. M10 (4.81) and L2 (2.79) exceed their limits by far more than the spread; the decision
does not depend on M1. Steiner budget used: M1 0.0048 with the pass (0.0039 without), M10 0.0018 (0.0017); no case came near the budget
error. Pass counters: M1 25 extra interior points, 530 moves, 277 extra flips; M10 28 extra points, 2592 moves, 1630 extra flips;
M10 ends with fewer cells (10614 vs 10710) because flips remove cells. The added cost per cell grows with the mesh (2.79x per cell
at 8.7x the output cells), so the pass is superlinear on this family; the cause was not isolated (candidates read from the code: every sweep
rescans all tetrahedra in `refine_constrained` and `improve`, whose 30-pass loop also rescans; not measured).

### Decision

The rule is NOT met. Blocking: B4 (`t5_torus_known_miss`: its recorded 0.634 becomes 1.000) and B5 (L1 on M1 and M10, L2).
Not blocking: B1, B2, B3 hold (no non-ignored failure, no changed literal outside W0, baseline hashes unchanged, no new `Err`). The
default stays `quality_sweeps = 0`; W0, S2/S3 and the diagnostics are not pinned (nothing to pin while the default is 0); no
fingerprint and no baseline hash was re-recorded.

What switching would need: (1) a user decision on `t5_torus_known_miss`: either it is declared a record of the mesher without the
pass and pinned to `quality_sweeps: 0`, or its record is retired as met with the pass on (and ADR 0030 gets an addendum); (2) a
cheaper pass or a user decision accepting about 3x on 1k cells and about 5x on 10k cells: the superlinear part would have to be
found and removed first (for example by working on a list of cells changed in the previous sweep instead of rescanning); (3) then
the pins, docs and the listed ADR addenda (ADR 0030 T5/T6/T10/P1/T5b, ADR 0093 S2b, ADR 0104 smoke and D tables).

Deviations: the L1/L2 harness stopped at the first miss and was corrected and run a second time (amendment 1, written before the
second run). The first registered run of `l3`, `q8` and `l12` used a version of the file not yet committed: the "without the pass"
side was changed from `TetOptions::default()` to an explicit `no_pass()` (`quality_sweeps: 0`) after the registration commit
63afe0b and committed with amendment 1 (293ab05); with the default at 0 the two are the same options, so no result changes. Side finding: the curvature remesher produced 2 degenerate triangles at theta 0.12, h_min 0.025, h_max 0.25 (smoke),
so `tetrahedralize` refused that surface; not pursued.

Gate on a fresh archive of 0f73dd5: `cargo fmt --check` and `cargo clippy --workspace --all-targets -D warnings` clean,
`cargo test --workspace --release --no-fail-fast` 1967 passed / 0 failed / 585 ignored (this ADR's file: 2 passed, 2 ignored; the
rest of the change since wave 41's 1959 / 576 is waves 42A and 42B). The commit after the gate changes this ADR's text only.
