# ADR 0760: F-868 registration: the NACA 0012 lift curve against Ladson's measured lift at Re 6e6

Status: **REGISTERED 2026-11-22, written before any code of F-868 and before any run of its checks.**
Before this text no solver iteration of this work ran, no test was run and no file of the repository was changed. What
was done: reading, one fetch of a published data file, the arithmetic below, and a release build of `rcfd-cli` from the
unchanged tree (made while this text was written; a build is not a run of a check). Results go in addenda at the end;
nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f868-naca0012-lift-curve.md` and follows `docs/process/agent-rules.md`, version 5.
Branch `f868-naca0012-lift-curve` from main 13b0937d, scratch `E:/projects/cfd/w49i-work/f868-naca0012-lift-curve-scratch`.
Parents: [ADR 0167](0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md) (the sweep whose set-up and
limits this work takes over), [ADR 0159](0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md) (the first sweep, the
mesh M1w and the value rules) and [ADR 0164](0164-job49a-why-the-sa-drag-of-the-naca0012-is-wrong.md) (the two fixes).

## What is asked

The acceptance of F-868 (`docs/roadmap.json`): "Every limit not stated here is registered in the ADR before the run. The
lift coefficient of the NACA 0012 at Re 6e6 in incompressible steady RANS, over the angles of attack and against the
measured data that ADR 0167 registered (Ladson, as recorded in docs/reference-data.md), is within a band registered from
that ADR's limits, on current main, by tests that the full gate runs or by a registered heavy record with a fast test on
the same path; the turbulence model, the mesh and the angles at which the comparison is not made or misses are stated."

The brief adds five tasks: read what exists against the acceptance; register the band before any run; run it on current
main and explain any difference from ADR 0167's record; make the comparison a test the gate holds, with a negative
control; state the limits of the claim.

## What was read

- `AGENTS.md`; `docs/process/agent-rules.md` (version 5) whole; the brief whole; the entries F-868 and F-043 of
  `docs/roadmap.json` and the milestone `aerofoil-lift` of `docs/milestones.json`, by a script.
- ADR 0167 whole. ADR 0159: its head, its table of limits (L1 to L4), the headings of its amendments and the table of
  results; **not read whole**. ADR 0164 was not read; what is used of it is what ADR 0167 states.
- `crates/rcfd-api/tests/aoa_sweep_w49d.rs` whole; the test names of `crates/rcfd-api/tests/aoa_sweep_w48y.rs`;
  `scripts/w49d_aoa_sweep_fixed.py` whole; `scripts/w48y_aoa_sweep.py` lines 1 to 470 and 550 to 632 (mesh builder, case
  text, `slope_fit`, `stall`, `ladson`, `interp`); `docs/investigations/job49d-aoa-sweep-fixed/README.md` and the SA runs
  of its `results.json`.
- [validation-data.md](../validation-data.md), section `naca0012-tmr` (lines 363 to 378); `docs/reference-data.md`, the
  row "NACA 0012 force and Cp data" (line 1292); `validation-data/reference/w31c_naca0012_ladson1988_80grit_cl_cd.csv`
  and `w49a_tmr_naca0012_sa_cfl3d_clcd.csv`; [tolerances.md](../process/tolerances.md);
  [test-process.md](../test-process.md), sections 7.4 and 7.5.
- `example-projects/wind-tunnel/notes/naca0012.md` (lines 1 to 80), the case `naca0012-aoa4-rotated.toml` and the recipe
  `naca0012-ogrid.recipe.toml`; in `crates/rcfd-api/src/sim.rs` the functions `run`, `is_converged`,
  `step_report_values` and the stop logic about line 4558; the schema's text of `component`.
- ADR 0759 as the newest registration, for the form; `scripts/process_report.py` lines 340 to 372 and 476 to 522.

**The measured file, fetched and read.** `CLCD_Ladson_expdata.dat` was fetched on 2026-11-22 from the pinned address
that `docs/validation-data.md` records (commit 20a39f54 of TMBWG/turbmodels, folder `NACA0012_validation`), 1343 bytes,
sha256 `78cd2f6aa4968e80f44cbf6c96f699bd9c6e45681d958ec528a10cf72ed23357`, equal to the page's record. Its header reads
"Data from Ladson, NASA TM 4074, 1988", "Re=6 million, with transition tripped", "M=0.15"; it has three zones, "80 grit"
(17 rows), "120 grit" (18 rows) and "180 grit" (18 rows), each alpha, cl, cd. The 80-grit zone equals the committed CSV
row for row. The report NASA TM 4074 itself was not read (the milestone's reference line says the same); the file states
no uncertainty, and neither page of the repository records one.

## What exists against the acceptance (task 1)

- **Angles, data, limits.** ADR 0167 ran steady Spalart-Allmaras at 16 angles (0 to 10 in steps of 2, 12 to 18 in steps
  of 1, 20, 22, 24). Its comparison with Ladson (80 grit, interpolated linearly to each angle) was **reported without a
  limit**. Its limits on lift were: L2, the slope over 0 to 8 degrees within 0.90 to 1.05 of 2 pi; L3, a drop of 10 % after
  the peak; L4, abs(cl) <= 0.005 at 0 degrees; LT10 and LT15, cl within 5 % of the TMR CFL3D SA values at 10 and 15
  degrees, which are computed values and not measured ones.
- **Mesh and model.** M1w-far500: a 256 x 128 O-grid, wall cells at y+ about 40, far field at 500 chords without a
  point-vortex correction; SA (noft2, negative form) with the Spalding wall law, SIMPLEC, linear upwind, first-order
  nuTilda convection, incompressible, Re 6e6, one build (f9a520a, 4038 commits before 13b0937d).
- **Tests.** `aoa_sweep_w49d.rs` has eight tests that the gate runs (L1, L2, L3, L4, LT0, LT10, P2, B1) and two ignored
  known misses (LT15, the drag at 15 degrees; LD, the drift metric). `aoa_sweep_w48y.rs` has five tests of ADR 0159's
  curve (no fixes, 50 chords). Every one of them reads committed numbers (`results.json`) and runs no solver.
- **At and beyond stall.** Steady SA peaks at 1.542 at 16 degrees; Ladson's 80-grit maximum is 1.6116 at 17.13. From
  17 degrees up the steady iteration does not converge and the recorded values are iteration means.
- **Shown by a test on main:** the slope band, the symmetry at 0 degrees, a stall, and cl within 5 % of a computed
  reference at 10 degrees, all on the numbers of the build of 2026-10-29. **Not shown:** any comparison with the measured
  lift by a test (ADR 0167's head states "within 0.013 of Ladson's from 2 to 15 degrees" from a report table); anything
  computed on current main; any solver run of this set-up in the gate. So no clause of the acceptance of F-868 is met by
  a test today.

## The reference and the arithmetic done beforehand

Reference: Ladson's 80-grit zone, as ADR 0167 used it, read from the committed CSV and interpolated linearly in alpha
between its rows (`cl_L(alpha)`). The 120-grit and 180-grit zones enter only the scatter. They are new reference values of
this work and go, with the scatter below, in `docs/reference-data.d/0760-F-868.part` in the commit after this one.

**Scatter of the measurement (computed from the fetched file, script `scatter.py` in the scratch folder).** Each zone
interpolated linearly to the registered angles; the spread is the largest less the smallest of the three:

| alpha | 80 grit | 120 grit | 180 grit | spread |
| --- | --- | --- | --- | --- |
| 0 | -0.0072 | -0.0121 | -0.0071 | 0.0050 |
| 2 | 0.2071 | 0.2071 | 0.2213 | 0.0142 |
| 4 | 0.4272 | 0.4276 | 0.4302 | 0.0030 |
| 6 | 0.6448 | 0.6476 | 0.6461 | 0.0028 |
| 8 | 0.8557 | 0.8615 | 0.8593 | 0.0058 |
| 10 | 1.0586 | 1.0672 | 1.0626 | 0.0086 |
| 12 | 1.2493 | 1.2594 | 1.2550 | 0.0101 |
| 13 | 1.3384 | 1.3474 | 1.3412 | 0.0090 |
| 14 | 1.4189 | 1.4319 | 1.4250 | 0.0130 |
| 15 | 1.4938 | 1.5083 | 1.4993 | 0.0145 |
| 16 | 1.5563 | 1.5738 | 1.5647 | 0.0175 |
| 17 | 1.6057 | 1.6234 | 1.6160 | 0.0177 |
| 18 | 1.0105 | 1.2740 | 1.1293 | 0.2635 |

The largest spread from 0 to 17 degrees is 0.01769 (at 17). A second sign of the measurement's own error: the aerofoil is
symmetric, and four of the five rows within 0.05 degrees of zero incidence give cl between -0.0115 and -0.0126. **The
scatter taken is s = 0.018**, the largest spread of the three zones below the stall, rounded up; it also covers the offset
at zero incidence.

**What I knew of the result (disclosed).** ADR 0167's SA curve, against the 80-grit values: +0.0072 at 0, +0.0130 at 2,
+0.0116 at 4, +0.0092 at 6, +0.0081 at 8, +0.0070 at 10, +0.0058 at 12, +0.0039 at 13, +0.0045 at 14, -0.0006 at 15,
-0.0143 at 16 and -0.558 at 17 degrees (iteration mean 1.0476, not steady). The band below was written knowing these
numbers. It is not fitted to them: its two terms are ADR 0167's 5 % and the scatter of the file, each taken whole.

## The band (task 2)

For a registered angle alpha, with `cl` the value of the curve (below):

    abs(cl - cl_L(alpha)) <= B(alpha) = max(0.05 abs(cl_L(alpha)), s),    s = 0.018

- **0.05** is the cl band of ADR 0167's LT10 and LT15, the only limits on a lift value that ADR registered; the acceptance
  asks for "a band registered from that ADR's limits".
- **s** is the scatter of the measured data as computed above; a computed value cannot be asked to agree with the
  measurement more closely than the measurement's three sets agree with each other. It decides the band at 0 and 2
  degrees; from 4 degrees up the 5 % term is the larger.

**Angles compared (12):** 0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16 and 17 degrees: ADR 0167's angles up to the last one below
Ladson's maximum lift (17.13 degrees), which is the milestone's "up to maximum lift". **17 degrees is compared although a
miss is expected there** (ADR 0167: steady SA has stalled at 17, the measured aerofoil has not). Leaving it out because the
computation fails there would turn a known miss into no comparison.

**Angles at which no comparison is made, named now:** 18, 20, 22 and 24 degrees. Reasons: they are beyond the measured
maximum lift; ADR 0167 found that steady SIMPLEC does not converge there, so there is no steady value to compare; the
three measured sets differ by 0.26 in cl at 18 degrees, and the 80-grit zone ends at 19.08, so 20 to 24 have no measured
value at all. These four angles are not run by this work. Negative angles are not run either (the aerofoil and the mesh
are symmetric; L4 covers the symmetry).

**A value that is not a flow value is a miss.** If the run of a compared angle neither met the residual targets (exit
code 0) nor is steady by ADR 0167's D5 (window range of `cl_eff` <= 0.005 and of `cd_eff` <= 0.0005), its number is an
iteration mean; it is reported, and the angle counts as missed whatever the mean is.

## Design

### The heavy record: the sweep on current main (task 3)

- **Build.** `cargo build --release --locked -p rcfd-cli` of the worktree's HEAD at the time of the build, with
  `CARGO_TARGET_DIR` under the scratch folder; the commit and the sha256 of the binary are recorded. No source of any
  crate is changed by this work, so the binary is that of main 13b0937d.
- **Mesh.** M1w-far500 built by `python scripts/w49d_aoa_sweep_fixed.py mesh`; its sha256 must be ADR 0167's
  `59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889` (check P2).
- **Cases.** The SA arm of ADR 0167's driver, byte for byte as `scripts/w49d_aoa_sweep_fixed.py cases` writes it today
  (the title still names ADR 0167): `turbulence = "freestream"` on the far field and the reports `cl_eff`, `cd_eff` with
  `component = "total_effective"`, both written out, so the cases mean what they meant on 2026-10-29 whatever the defaults
  are now ([ADR 0166](0166-job49c-effective-wall-shear-and-sa-far-field-defaults.md) changed them since). The twelve case
  texts are committed under `docs/investigations/f868-naca0012-lift-curve/cases/`.
- **Runs.** `python scripts/w49d_aoa_sweep_fixed.py run`, the twelve ids `SA-a00` to `SA-a17`, one thread each, at most
  three solver processes at a time (two other agents run heavy solves on this machine). Each run is made once. If the
  machine restarts, a run cut short is made again from the free stream with the same binary and case, and that is
  reported. No limit on time is registered; ADR 0167 took 56 s to 37 minutes a run.
- **The curve.** ADR 0167's D5 and D7, by its driver's `read_runs` and `analyse`, unchanged: `cl_eff`; the final iterate
  for a run that met the residual targets, the mean of the last min(1000, n/2) iterations for a run at its cap of 8000;
  an angle not steady by D5 enters with its window mean, flagged.
- **Evaluation.** A new driver `scripts/f868_naca0012_lift_curve.py` (`collect`, `selftest`) takes that curve, adds
  `cl_L`, `B` and the outcome per angle, and writes `docs/investigations/f868-naca0012-lift-curve/results.json` with the
  case text of every run in it. It is committed before the runs.
- **Against ADR 0167's record (report, no limit).** The difference of `cl` to the committed curve of
  `docs/investigations/job49d-aoa-sweep-fixed/results.json` at each of the twelve angles, and the iteration counts. If
  any differs, the reason is looked for: first in the history of the solver crates between f9a520a and 13b0937d, then, if
  the reading does not settle it, by building older commits and repeating the run at 0 degrees (the shortest, 254
  iterations). How far that got is reported; it has no limit.

This heavy record is **not an ignored cargo test**: the mesh is built by the Python driver and the twelve runs take up to
8000 iterations on 32 768 cells each. It is a record in the sense of ADR 0167's D9, re-made on demand by the two drivers.

### What the gate holds (task 4)

A new test file `crates/rcfd-api/tests/naca0012_lift_curve_f868.rs`, header "ADR 0760 (F-868)". None of its tests is
ignored at the start; a registered limit that misses becomes an ignored `*_known_miss` test with the limit intact (rule 2).

1. **The band on the record** (tests V00 to V17, one per angle, and the others of the table below): read the committed
   `results.json` and the committed 80-grit CSV, recompute `cl_L` and `B` independently of the driver, and assert the
   band. They also assert that the three zones of the measured file, held in the test as constants, give a largest spread
   of at most s (so s stays traceable), and that each committed case file equals the case text recorded in
   `results.json` and names its angle (`rotate_z_deg` = -alpha).
2. **The fast test on the same path** (FT0, FT4), which runs the solver in the gate: the committed case files `SA-a00.toml`
   and `SA-a04.toml`, with exactly three lines replaced (the mesh file, by the committed coarse O-grid
   `example-projects/wind-tunnel/meshes/naca0012-ogrid`, 6144 cells, far field at 20 chords; `max_iterations` and
   `min_iterations`, by 600), run in process by `Simulation::run(600)`; the value is `cl_eff` of the last step. Same case
   keys, same model, same far-field role, same mesh rotation, same force report as the heavy record; another mesh. This
   mesh is too coarse for a comparison with the measurement (the example's note records cl 0.347 at 4 degrees with SST,
   19 % under Ladson), so FT4 is a check against the code's own earlier output: the reference `R4` is the value of the
   first run of this test on the build of this work, written into the test as a constant in the commit that adds the
   test, and stated in the addendum. The tolerance around it is fixed now: s = 0.018. A change of the solver that moves
   the lift of this path by more than the scatter of the measurement fails the gate, and the heavy record is then due
   again. A smaller change is not seen by the gate; that is a limit of this design, stated below.
3. **Negative controls in the gate** (NC1, NC2): the band check applied to the committed curve with the angle of attack
   offset by one degree (the value computed at alpha judged at alpha - 1), and with the reference area wrong by 10 % (cl
   multiplied by 1.1), must reject the curve.

### Value rules fixed now

- Linear interpolation of the 80-grit rows; alpha - 1 = -1 lies inside the table (rows from -4.04).
- The peak `cl_max` is the largest `cl` among the compared angles whose value is a flow value (rule above).
- The slope is ADR 0167's L2 fit (least squares with intercept through 0, 2, 4, 6, 8 degrees).
- FT: 600 steps or fewer if the run ends earlier by its own stop logic; the count is recorded with `R4`.

## Table of checks

Estimates are ADR 0167's recorded values. Margin is the limit divided by the estimate.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| V00 | acceptance, lift within the band at 0 degrees: abs(cl - (-0.0072)) | <= 0.0180 | the scatter s of `CLCD_Ladson_expdata.dat` (three zones, this ADR's table) | 0.0072 | 2.5 | validation |
| V02 | the same at 2 degrees: abs(cl - 0.2071) | <= 0.0180 | the scatter s, as V00 | 0.0130 | 1.4 | validation |
| V04 | the same at 4 degrees: abs(cl - 0.4272) | <= 0.0214 | ADR 0167, LT10 and LT15 (cl within 5 %); acceptance of F-868 | 0.0116 | 1.8 | validation |
| V06 | the same at 6 degrees: abs(cl - 0.6448) | <= 0.0322 | as V04 | 0.0092 | 3.5 | validation |
| V08 | the same at 8 degrees: abs(cl - 0.8557) | <= 0.0428 | as V04 | 0.0081 | 5.3 | validation |
| V10 | the same at 10 degrees: abs(cl - 1.0586) | <= 0.0529 | as V04 | 0.0070 | 7.6 | validation |
| V12 | the same at 12 degrees: abs(cl - 1.2493) | <= 0.0625 | as V04 | 0.0058 | 10.8 | validation |
| V13 | the same at 13 degrees: abs(cl - 1.3384) | <= 0.0669 | as V04 | 0.0039 | 17 | validation |
| V14 | the same at 14 degrees: abs(cl - 1.4189) | <= 0.0709 | as V04 | 0.0045 | 16 | validation |
| V15 | the same at 15 degrees: abs(cl - 1.4938) | <= 0.0747 | as V04 | 0.0006 | 125 | validation |
| V16 | the same at 16 degrees: abs(cl - 1.5563) | <= 0.0778 | as V04 | 0.0143 | 5.4 | validation |
| V17 | the same at 17 degrees: abs(cl - 1.6057), and the value a flow value | <= 0.0803 | as V04 | 0.558, not steady: **a miss is expected** | 0.14 | validation |
| VM | maximum lift: abs(cl_max / 1.6116 - 1) | <= 0.05 | ADR 0167, LT10 and LT15 (cl within 5 %); Ladson's 80-grit row at 17.13 degrees | 0.043 (1.542 at 16) | 1.16 | validation |
| L2 | lift slope over 0 to 8 degrees, as a factor of 2 pi | 0.90 to 1.05 | ADR 0167, L2 (ADR 0159's band) | 0.986 | n/a | solution verification |
| L4 | symmetry: abs(cl) at 0 degrees | <= 0.005 | ADR 0167, L4 | 1.4e-6 | 3571 | code verification |
| R1 | acceptance, "on current main": every one of the twelve runs ends with exit code 0 or 5, on a binary built from this work's HEAD with no crate source changed | 12 of 12 | ADR 0167, L1; acceptance of F-868 | 12 | n/a | code verification |
| P2 | the mesh is ADR 0167's: sha256 of M1w-far500 | equal | ADR 0167, P2 | equal | n/a | unchanged default output |
| T1 | acceptance, "the turbulence model, the mesh and the angles ... stated": the committed case files equal the texts recorded in `results.json`, each names its angle, the model is `spalart_allmaras` and the mesh `M1w-far500` | all twelve | acceptance of F-868 | holds | n/a | code verification |
| S1 | the scatter is traceable: largest spread of the three zones at the twelve angles | <= 0.018 | `CLCD_Ladson_expdata.dat`, zones "80 grit", "120 grit", "180 grit" | 0.01769 | 1.02 | code verification |
| FT0 | fast test, coarse mesh, 0 degrees: abs(cl_eff) after the run | <= 0.005 | ADR 0167, L4 | below 1e-4 | above 50 | code verification |
| FT4 | fast test, coarse mesh, 4 degrees: abs(cl_eff - R4), R4 recorded once as said above | <= 0.018 | the scatter s, as V00 | 0 on the same build; platform differences far below 1e-3 | above 18 | code verification |
| FT4u | fast test, 4 degrees: cl_eff positive and not above 1.05 x 2 pi x 4 degrees = 0.4606 | 0 < cl_eff <= 0.4606 | ADR 0167, L2 (upper edge of the slope band) | 0.35 to 0.44 | n/a | solution verification |
| NC1 | negative control: angle of attack offset by one degree; angles of 0 to 16 outside the band | >= 8 of 11 | the band of V00 to V16, the other way round ([tolerances.md](../process/tolerances.md), its paragraph on negative controls) | 10 or 11 | n/a | validation (negative control) |
| NC2 | negative control: reference area wrong by 10 % (cl x 1.1); angles of 0 to 16 outside the band | >= 8 of 11 | as NC1 | 10 | n/a | validation (negative control) |
| D1 | report: cl of each angle against ADR 0167's committed curve, and the reason of any difference | none (reported) | the brief, task 3 | equal or within 1e-3 | n/a | code verification (reported without a limit) |
| B1 | storage: `docs/investigations/f868-naca0012-lift-curve/` | <= 200 000 bytes | ADR 0167, B1 (400 000 bytes for 28 runs with plots) | 80 000 | 2.5 | unchanged default output |
| E | `git diff --stat 13b0937d` names no file under `crates/*/src/`, no existing test, no golden and no hash table | none | rule 3 of `docs/process/agent-rules.md`; the brief ("change no solver source") | holds | n/a | unchanged default output |
| F | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; the selftests of the three drivers | clean; clean; passes; pass | rule 7 of `docs/process/agent-rules.md` | clean | n/a | code verification |
| P | `python scripts/process_report.py --check` and `python scripts/roadmap_proposals.py check` on the last commit | no failure of this work | rules 7 and 9 of `docs/process/agent-rules.md` | none | n/a | code verification |

**On the margins.** From 6 degrees up the 5 % term gives a band 3.5 to 125 times the difference ADR 0167 recorded; the
process report will list these as wide. The band is what the acceptance names (ADR 0167's limit) and is not narrowed here
to fit the known curve. Beside it the addendum reports, without a limit, at how many angles the difference is within the
scatter s alone.

**On the tolerance table.** Its row "Agreement with a measured case" has no value: "to be set by the maintainer per
measured case". The band above is this registration's proposal for this case; see the questions.

## Negative controls and mutations

NC1 and NC2 run in the gate. Mutations, each on a scratch copy of the tree at the last code commit, release, run once;
what failed is recorded:

- **M1** (committed, `crates/rcfd-api/tests/naca0012_lift_curve_f868.mutation.patch`, first line
  `# expect-fail: cargo test -p rcfd-api --release --test naca0012_lift_curve_f868`): in the committed case file
  `SA-a04.toml`, `rotate_z_deg = -4` becomes `-5` (the angle of attack offset by one degree). FT4 must fail (the lift of
  the coarse run moves by about 0.09), and T1.
- **M2**: in `SA-a04.toml`, `a_ref = 0.1` of the report `cl_eff` becomes `0.11` (the reference area wrong by 10 %). FT4
  must fail if R4 is above 0.198 (then a tenth less is more than 0.018 away); T1 fails too.
- **M3**: in `results.json`, `cl` at 10 degrees raised by 0.06. V10 must fail.
- **M4**: in the test file, s and the factor 0.05 both multiplied by ten. NC1 or NC2 must fail.

## Size

Size (agent): partial; 0; 1; 2; 260000

The one heavy run is the sweep (twelve solver runs, three at a time, about an hour on the shared machine) with one
release build of `rcfd-cli` and the release build of the test file. Partial is expected because of V17.

## Decision rule

- **done** only if V00 to V17 and VM are all met on the runs of this work, R1, P2, T1, S1, L2 and L4 hold, FT0, FT4 and
  FT4u pass in the gate-side test file, NC1 and NC2 reject, M1 to M4 are each caught, and E, F and P hold.
- **partial** if some of V00 to V17 or VM miss, or a gate-side check or a mutation is not in place; the addendum lists
  each miss with its number, and a missed limit stays as an ignored `*_known_miss` test with the limit intact.
- **stopped and reported** if the measured data cannot be traced to a source that was read (it can: the file above), if
  P2 fails, or if a defect of the solver appears; no solver source is changed by this work.
- **todo** if the sweep cannot be run on current main.

No limit is changed after a run, no run is repeated to get another number, and no test is ignored, skipped or retried to
make a run pass.

## Limits of the claim (task 5), stated before any result

- **Model:** steady RANS, Spalart-Allmaras (noft2, negative form) with the Spalding wall law, fully turbulent, against a
  tripped boundary layer. No SST, no DDES, no transition.
- **Mesh:** one mesh, M1w-far500, 256 x 128, wall cells at y+ about 40. No grid-convergence check (ADR 0159's M2w was not
  run with the fixes), no wall-resolved mesh.
- **Far field:** a circle of 500 chords with a fixed free stream, no point-vortex correction.
- **Flow:** incompressible against Mach 0.15; Re 6e6; ADR 0092's closed-trailing-edge thickness form.
- **Numerics:** SIMPLEC, linear upwind for momentum, first-order convection of nuTilda; every angle from the free stream,
  so no hysteresis.
- **Angles:** 0 to 17 degrees compared; 18 to 24 not compared and not run; the stall itself (its angle, the lift after
  it) is not claimed.
- **Not claimed:** drag (ADR 0167 records +26 % against TMR at 15 degrees, a known miss), moment, pressure distribution,
  skin friction.
- **The gate** sees a change of the lift on the coarse mesh above 0.018 at 4 degrees and any change of the committed
  record; it does not re-run the 32 768-cell sweep, and it does not run on Linux anything but the same two fast runs.
- **Platform:** the heavy record is made on Windows only.

## Questions foreseen (rule 6: not taken, they change the scope of the item or need the maintainer's value)

1. Whether the headline needs 17 degrees. The measured aerofoil is attached there and steady SA on this mesh is not.
2. Whether the band max(5 %, 0.018) is the maintainer's value for this measured case in the tolerance table.

Both are written out with options and a recommendation in the addendum, under the fixed heading.
