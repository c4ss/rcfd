# ADR 0228: F-306 registration: the tetrahedral mesher's neighbour-link panic on a thin box (a folded edge removal)

Status: **REGISTERED 2026-11-08**, written before any code of F-306 and before any run of its checks. Not yet accepted by the
maintainer. Before this text, existing code of main 9f95fb12 was run in scratch crates outside the repository to reproduce
the panic and to locate its cause; one of them ran a copy of `tetmesh.rs` with print-only instrumentation (listed under "Runs
before this registration"). No code of F-306 existed. Results go in the addendum at the end.

Parents: [ADR 0215](0215-f247-registration-local-remeshing-on-unstructured-tetrahedral-meshes.md) (F-247: the panic was seen
while choosing its initial mesh M0; question 7), [ADR 0030](0030-wave19b-meshing.md) (F-132: `tetmesh::tetrahedralize`, its
flips and edge removal), [ADR 0104](0104-wave41d-tet-sliver-removal.md) (the opt-in quality pass `quality_sweeps`),
[ADR 0107](0107-wave42c-tet-quality-pass-default-evaluation.md) (the STL candidate set-ups of `w42c_tet_default_eval`),
[ADR 0137](0137-linux-gate-platform-values.md) (platform values of mesh fingerprints). Roadmap item F-306 (depends on F-247).

Number: the lead gave 0228 to F-306. Branch `f306-tetmesh-thin-box` from main 9f95fb12, worktree `w49i-work/f306`. Main holds
0224 as its highest; 0225 is on `f300-transient-rans`, 0226 on `f320-wetsteam-checks`, 0227 on `f302-pair-diffusivity`. No
branch holds a 0228.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0207, ADR 0215):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- no case-schema key is added (the mesher has no case key); neither `crates/rcfd-api/tests/baseline_hashes.rs` nor
  `case_hashes_w48u.rs` is edited or re-recorded, no golden is edited or re-recorded, and no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no commit trailers.

Clean-room: the cause and the fix are derived here from the code of `tetmesh.rs` and from the geometry of an edge removal (a
ring of cells around an edge replaced by two cells per triangle of a triangulation of the ring polygon). No source outside the
repository was read for this work. The same validity rule (cells built in the turning order of the ring must be positively
oriented) is already used by F-247's remesher (`remesh.rs`, `cand_removal`, ADR 0215 M3 and M4).

## What F-306 asks

The roadmap entry (acceptance, quoted): "tetrahedralize of box_surface([0,0,0],[1,0.1,0.1],[10,1,1]) at size 0.035 and
quality_sweeps 3 returns a mesh or a located error instead of panicking at tetmesh.rs:386." The lead's brief adds: reproduce
the panic first and record the failing input here before any fix; the reproducer must fail on main and pass after the fix; every
existing tetmesh output for inputs that do not fail is bit-identical.

**Reading (registered here).** "Returns a mesh" is read as returning a *valid* mesh (V below), because main already returns
`Ok` with overlapping cells for neighbours of the failing input (the table below), which is the same defect without the panic.
"Inputs that do not fail" is read as the inputs whose run on main makes no folded edge removal (defined below): a run that
makes one has produced overlapping cells at that moment, whatever its end. Three such runs end with a valid mesh on main (F6 to
F8 below); their output changes with the fix, and this is disclosed, not hidden (Q4).

## Runs before this registration (disclosed)

All in `E:/projects/cfd/w49i-work/f306-scratch` (not committed), release, Windows, path dependencies on main 9f95fb12:

1. `repro`: `tetrahedralize(&box_surface([0, 0, 0], [1, 0.1, 0.1], [10, 1, 1]), &TetOptions { size: Some(0.035),
   quality_sweeps: 3, ..Default::default() })` under `catch_unwind`. **It panics at `crates/rcfd-mesh/src/tetmesh.rs:386:71`
   with `neighbour link`**, in release and in debug (the `debug_assert` of `commit` does not fire first). The backtrace:
   `Del::replace` (line 386) called from `Del::try_edge_removal` (line 555), from `Ctx::improve` (line 772), from
   `tetrahedralize` (line 1126: `ctx.improve(0.06)` of the quality pass). The same input with `quality_sweeps` 0, 1 and 2 returns
   `Ok` (1055, 1127 and 1339 cells).
2. A copy of `tetmesh.rs` in `repro`, then `main-instr` (a `git archive` of 9f95fb12 with print-only instrumentation
   applied by `f306-scratch/instrument.py`): after every `commit` and `replace` a full check of the neighbour
   links (each link points back exactly once, across the same face); in `try_edge_removal`, before `replace`, the turning signs
   of the ring and the orientation of every chosen triangle (below), with the summed volumes of the ring and of the new cells; at
   the end of `tetrahedralize`, the summed cell volume against the enclosed volume of the surface. The instrumentation changes
   no decision of the mesher.
3. `sweep`: the inputs N-A, N-B and N-C below on `main-instr`, recording for each the outcome, the cell and point counts, a
   fingerprint, `to_mesh` success, the folded removals and the volume. And the existing test suites that call the mesher, run
   on `main-instr` with `--include-ignored`: `rcfd-mesh` `tetmesh`, `polydual` and the `remesh` unit tests; `rcfd-io`
   `w41d_tet_quality`, `w42c_tet_default_eval` (whose print-only smoke holds the inputs N-S below), `stl_cylinder_in_box`. The
   `rcfd-fv` users (`remesh_f247`'s M0, `w19b`'s two inputs) are inputs of N-A and N-C and were run there.

## The cause (located by run 2)

`Del::try_edge_removal(d, e)` takes the `n` (3 to 7) inside cells around the edge `(d, e)`, builds the ring polygon `poly`
(the vertices opposite the edge, in ring order), triangulates it by dynamic programming and replaces the ring by the cells
`(a, b, c, d)` and `(a, b, c, e)` of each triangle. A triangle is accepted by `tri_tets` when `d` and `e` lie strictly on
opposite sides of its plane (`od != 0`, `oe != 0`, `od != oe`), and each new cell is then *made* positive by swapping two
vertices when needed.

That test is necessary but not sufficient. Let `s = orient3d(d, e, poly[i], poly[i+1])` be the turning sign of the ring: in a
valid mesh every cell around an interior edge is positive, so `s` is the same for every `i` (non-zero). Then a triangle
`(poly[i], poly[j], poly[k])`, `i < j < k`, of a valid triangulation has `orient3d(.., d) = -s` and `orient3d(.., e) = s` (a check
on coordinates: `d = (0, 0, 1)`, `e = (0, 0, -1)`, `poly` counter-clockwise on the unit circle of `z = 0` seen from `d` give
`s = -1` and `od = +1`). A triangle with `od = s` and `oe = -s` also passes the old test: it is a *folded* triangle, lying
across a reflex corner of the polygon, outside the ring. The vertex swap hides the fold: the new cells are positive but cover
space outside the ring, which the cells next to the ring already cover. The faces still match by their sorted vertex keys, so
nothing complains at first.

**Definition (registered).** A *folded edge removal* is a call of `replace` from `try_edge_removal` in which the ring's turning
signs are not all equal, or a chosen triangle has `od != -s` (with `s` the sign of the first ring edge).

What run 2 showed on the acceptance input (`quality_sweeps` 3): the first folded removal is operation 420 (the `commit` and
`replace` calls counted from 0), in the first sweep's `improve(0.06)`: `n = 4`, `poly = [147, 156, 11, 33]`, ring signs all
`-1`, triangle signs `[+1, -1]` (the second folded), ring volume `1.600e-4`, new cells `2.500e-4`. Two more folds follow (operations 425 and 431); after the
third, two cells hold the same four vertices (`[258, 16, 231, 51]` and `[16, 51, 231, 258]`, both positive) and a cell links to
the other across two faces: the first broken neighbour link. Later sweeps add folds (13 in all before the panic); the panic is
the first `replace` whose outer face points at a cell that no longer links back. With `quality_sweeps` 1 and 2 the first folds
happen (3 and 9) but no later `replace` reads a broken link, so `Ok` is returned with overlapping cells: total cell volume
`1.0048e-2` and `1.0158e-2` against the enclosed `1.0e-2`, and `to_mesh` fails with "face shared by more than two cells".

The fold is not specific to the quality pass: `improve(sliver_q)` of the default path calls the same edge removal. The plate F12
panics at the same line with default options, and an STL input of `w42c_tet_default_eval`'s print-only smoke (F14) returns `Ok`
with overlapping cells with default options.

No other mechanism was seen: in every run of N and of the existing suites, the neighbour links stayed consistent and the summed
cell volume matched the enclosed volume (largest relative difference `1.7e-14`) whenever no fold happened, and every broken link
came after a fold. The flips (`try_flip23`) are exact (all three `orient3d(d, e, ., .)` of one sign means the segment `de` pierces
the triangle) and the cavities of `commit` are star-shaped by construction; neither is changed.

## The fix (registered, not yet written)

In `Del::try_edge_removal`, after the ring polygon is built and before the triangulation:

1. compute `s = orient3d(d, e, poly[0], poly[1])`; if `s == 0` or any ring edge `(poly[i], poly[(i + 1) % n])` has a different
   sign, return `false` (no change);
2. in `tri_tets`, accept a triangle only if `od == -s` and `oe == s` (this implies the old `od != 0`, `oe != 0`, `od != oe`);
   the new cells are built as now.

Nothing else changes: no option, no field of `TetOptions` or `TetReport`, no other operator.

**Why fold-free runs stay bit-identical (derived).** The fix differs from main only when (i) the ring signs are mixed, or (ii)
some triangle with `od = s` is rejected. In case (ii) the triangulation values of the dynamic programme can only fall, and a
triangulation with no folded triangle keeps its value. If main's chosen triangulation has no folded triangle, induction along
its sub-polygons gives the same values and, because the programme keeps the first index of strict improvement and every earlier
index was strictly worse on main, the same choices; so the same cells, and the same accept or reject decision against the ring's
quality (a rejected removal stays rejected, its value can only fall). In case (i), on main either the removal was rejected later
(no change) or it was carried out, which is a folded removal by the definition. So a run changes only from its first folded
removal on, and every other step, including the random walk of `locate`, is the same. The fix is unconditional because the
acceptance input uses existing options only (Q4).

## Validity V (registered)

A `TetMesh m` from a surface `S` is valid when:

- (a) every cell has `orient3d(p0, p1, p2, p3) > 0` (exact predicate, `rcfd_mesh::predicates::orient3d`);
- (b) `abs(sum of cell volumes - E) <= 1e-12 E`, with `E = diagnose(S, &DiagOptions::default()).enclosed_volume`;
- (c) `m.to_mesh(&BTreeMap::new())` is `Ok` (`Mesh::from_face_lists` validates closure, positive volumes and owner-to-neighbour
  normals; `to_mesh` itself refuses a face of more than two cells and a boundary triangle that is not one free cell face).

## The inputs (registered)

All with `TetOptions::default()` except `size` and `quality_sweeps` as given (size "none" is `None`).

- **N-A** (the ADR 0215 family): `box_surface([0, 0, 0], [1, 0.1, 0.1], n)`, `n` in `[10, 1, 1]`, `[20, 2, 2]`, `[30, 3, 3]`,
  `[40, 4, 4]`; size none, 0.025, 0.0333, 0.035, 0.05; `quality_sweeps` 0, 1, 2, 3. 80 inputs.
- **N-B** (other boxes): `box_surface([0, 0, 0], hi, n)` for `(hi, n)` = `([0.5, 0.12, 0.08], [10, 3, 2])` (ADR 0215 D-a),
  `([0.8, 0.1, 0.12], [16, 2, 3])` (D-b), `([0.3, 0.1, 0.1], [6, 2, 2])` (U2's M_s), `([1, 0.05, 0.05], [20, 1, 1])` (thinner),
  `([1, 1, 0.1], [10, 10, 1])` (a plate); size none, 0.035, 0.05; `quality_sweeps` 0 and 3. 30 inputs.
- **N-C** (closed shapes of the existing tests): `cube(2, 2)`, `icosphere(1, 2)`, `torus(2, 0.5, 24, 12)`,
  `cylinder(0.5, 1, 16, 4)`, size none, `quality_sweeps` 0, 1, 3 (12 inputs); `icosphere(1, 2)` size 0.25 and `cube(2, 4)` size
  0.5 (`w19b_meshing_solve`), `quality_sweeps` 0 (2 inputs). 14 inputs.
- **N-S** (STL, the ADR 0107 candidates of `w42c_tet_default_eval::s0_smoke_finer_candidates`): `curvature_remesh(from_stl_file(),
  theta, h_min, h_max)` for `(0.5, 0.1, 1)`, `(0.25, 0.05, 0.5)`, `(0.2, 0.04, 0.4)`, `(0.15, 0.03, 0.3)`, `(0.12, 0.025, 0.25)`,
  `(0.1, 0.02, 0.2)`, `(0.15, 0.03, 0.2)`, `(0.15, 0.03, 0.15)`, `quality_sweeps` 0 (the helpers copied from that file). 8 inputs.

**What main does with them (runs 2 and 3; Windows).** Of the 132 inputs, 118 make no folded removal: each returns `Ok` with a
valid mesh (59325 edge removals in the 111 fold-free runs of N-A to N-C, so the sign rule is exercised), except N-S `(0.12,
0.025, 0.25)`, which returns the located error "surface has 2 degenerate and 0 duplicate triangle(s)" (the surface, not the
mesher). The other 14 make folded removals:

| Id | Input | Main | Folds | Main detail |
| --- | --- | --- | --- | --- |
| F1 | N-A `[10, 1, 1]` size 0.025 sweeps 0 | `Ok`, not valid | 10 | volume `+8.55e-3` relative; `to_mesh`: "face shared by more than two cells" |
| F2 | same, sweeps 1 | panic, line 386 | 32 before the panic | |
| F3 | same, sweeps 2 | panic, line 386 | 32 | |
| F4 | same, sweeps 3 | panic, line 386 | 32 | |
| F5 | N-A `[10, 1, 1]` size 0.0333 sweeps 0 | `Ok`, not valid | 1 | volume `+1.43e-3`; `to_mesh`: "cell 1051: a directed edge appears twice" |
| F6 | same, sweeps 1 | `Ok`, valid | 1 | the quality pass re-meshed the overlap; 1216 cells |
| F7 | same, sweeps 2 | `Ok`, valid | 1 | 1349 cells |
| F8 | same, sweeps 3 | `Ok`, valid | 1 | 1481 cells |
| F9 | N-A `[10, 1, 1]` size 0.035 sweeps 1 | `Ok`, not valid | 3 | volume `+4.83e-3`; "face shared by more than two cells" |
| F10 | same, sweeps 2 | `Ok`, not valid | 9 | volume `+1.58e-2`; same |
| **F11** | **same, sweeps 3 (the acceptance input)** | **panic, line 386** | 13 | |
| F12 | N-B plate size 0.035 sweeps 0 (default options otherwise) | panic, line 386 | 18 | |
| F13 | same, sweeps 3 | panic, line 386 | 18 | the panic is before the pass |
| F14 | N-S `(0.1, 0.02, 0.2)`, default options | `Ok`, not valid | 9 | volume `+7.3e-5` (the smoke prints "cells Ok(16868)") |

N-S depends on the platform's `sin` and `cos` (ADR 0137); the other inputs use no transcendental function. No existing test with
a limit or a fingerprint calls the mesher on a folded input: the only one, F14, is in an ignored print-only smoke.

## Registered checks

Tests in two new files, `crates/rcfd-mesh/tests/tetmesh_fold_f306.rs` (R1, R2, N1) and
`crates/rcfd-io/tests/tet_fold_f306.rs` (R3, N1s). Each input runs under `catch_unwind`, so a panic is reported as a failure of
the input, not of the whole binary.

| Id | Check | Limit |
| --- | --- | --- |
| R1 (acceptance) | F11 | `Ok`, and V |
| R2 | F1 to F10, F12, F13, each | `Ok`, and V |
| R3 | F14 | `Ok`, and V |
| N1 | the 111 fold-free inputs of N-A, N-B, N-C, each: the fingerprint, and V | the fingerprint equal to the one recorded at the record commit (below) on Windows; V on every platform |
| N1s | the 7 fold-free inputs of N-S, each | the fingerprint (or the error text) equal to the record on Windows; V for the six `Ok` ones |
| N2 | the record commit (the two test files and the recorded fingerprints, the library of main), each new test run once in release | R1 fails by the panic at `tetmesh.rs:386` with `neighbour link`; R2 fails on every input as in the table (F2 to F4, F12, F13 panic; F1, F5, F9, F10 fail V; F6 to F8 pass); R3 fails V; N1 and N1s pass |
| N3 | the existing suites, unedited, at the final commit: `rcfd-mesh` `tetmesh`, `polydual` and the `remesh` unit tests; `rcfd-io` `w41d_tet_quality` (W0's fingerprints), `w42c_tet_default_eval`, `stl_cylinder_in_box`; `rcfd-fv` `remesh_f247` (M0's hash `aaf8f4c36f6446e4`), `w19b_meshing_solve`; the gate | pass (known misses stay ignored) |

**The fingerprint** of a `TetMesh`: the cell and point counts and an FNV-1a 64 hash over the bits of every point coordinate,
every cell's vertex list, every surface triangle and tag, and the eight `TetReport` counters. **The record commit**: the test
files are written with N1 and N1s printing their fingerprints; they are run once on the library of main (no library change on
the branch yet); the printed values are pasted as the expected values (Windows), and that tree is committed before any library
change. On other platforms N1 and N1s print the fingerprint and assert only V (no Linux record is made here; Q6).

F6 to F8 are in R2: they are valid on main, their outputs are expected to change with the fix (they pass through a fold), and
their new fingerprints are recorded in the addendum.

### Negative controls

- NC1: V must reject the outputs of main that overlap: shown by N2 (F1, F5, F9, F10 and R3 fail V at the record commit, not by a
  panic). If V passes any of them, V is defective.
- NC2: the sign rule must change something on fold-free inputs when it is wrong: mutation MU2 below.

### Mutations (scratch, not committed)

Each is applied alone to the final tree by an exact text replacement, the named tests are run once in release, and they must
fail; the file is restored and checked byte for byte against a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the triangle test back to main's (`od == 0 \|\| oe == 0 \|\| od == oe`) and the ring-sign check removed | R1 |
| MU2 | the required signs swapped (`od == s`, `oe == -s`) | N1 (at least one fingerprint differs) |
| MU3 | the triangle test back to main's, the ring-sign check kept | R1 |

Probe P1 (no failure predicted): the ring-sign check removed, the triangle test kept, with `s` from the first ring edge. Run R1,
R2, N1; my prediction: all pass, because on a valid mesh the ring signs are never mixed (the check only guards a mesh that is
already broken). Its result is recorded, not counted.

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a `git archive`
copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), target dir
`E:/projects/cfd/w49i-work/target-f306-gate`, taken in turn through the machine's gate lock and started with at least 19 GB of
physical memory free. A failure of a known flaky test is run again alone on the same build and reported with its name.

### Classes

R1 and N1 are preconditions of landing: a miss of R1 means the acceptance is not met; a miss of N1 means the fix changed a
fold-free output, the change is not merged and the cause is recorded. A miss of R2, R3 or N1s is reported with its number and
kept as an ignored `*_known_miss` test with the limit intact. A negative control that does not hold, or a mutation that does
not make its check fail, means the check is not shown to be able to fail: a defective set-up, corrected by an amendment before
any re-run.

## Predictions (mine)

| Id | Prediction |
| --- | --- |
| PR1 | R1, R2, R3 pass: with the fold rejected, the ring is either removed by a valid triangulation or kept |
| PR2 | N1 and N1s pass (the derivation above) |
| PR3 | F6 to F8 change their fingerprints; F11's cell count is within 20 percent of F10's on main (1339), as a rough figure only |
| PR4 | the new tests take under two minutes in release in all |

## Not delivered

A validity check of the output inside `tetrahedralize` (Q3); turning the two `expect("neighbour link")` invariant checks into
located errors (Q2); Linux records of the new fingerprints (Q6); any change to the flips, the cavities, the smoothing or the
quality pass.

## Open questions and choices for the maintainer

- **Q1. Status of F-306.** If every registered check is met, the acceptance is met (a valid mesh instead of the panic).
  Recommendation: done, with the maintainer's acceptance.
- **Q2. The invariant checks.** `commit` (line 319) and `replace` (line 386) keep their `expect("neighbour link")`; with the fix no
  known input reaches them. Options: (a) keep them as invariant checks; (b) turn them into located errors (a `Result` through
  `replace`, the flips and `improve`). Recommendation: (a); (b) only with Q3, if an unknown mechanism shows up.
- **Q3. An output check.** V (a) and (b) cost one pass over the cells; a failure could return a located error ("the volume mesh
  overlaps itself") instead of an `Ok` that `to_mesh` later rejects. It changes no valid output. Options: (a) a small follow-up
  item; (b) not now. Recommendation: (a).
- **Q4. Unconditional fix.** The acceptance input uses existing options only, so an opt-in switch could not meet it; the fix is
  unconditional and changes exactly the runs that make a folded removal. Three of them (F6 to F8, opt-in quality pass) end with a
  valid mesh on main and will change. Options: (a) accept; (b) an opt-in switch, leaving the panic and the overlapping outputs in
  the default. Recommendation: (a).
- **Q5. The print-only smoke.** `w42c_tet_default_eval::s0_smoke_finer_candidates` (ignored, prints only) will print another
  cell count for `(0.1, 0.02, 0.2)`. Nothing to edit; noted.
- **Q6. Linux records.** N1 and N1s assert their fingerprints on Windows only. Recommendation: record the Linux values at the next
  Linux gate (ADR 0137's procedure); the box inputs probably agree, which is not proven.
