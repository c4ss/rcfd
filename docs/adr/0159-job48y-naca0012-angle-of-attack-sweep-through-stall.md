# ADR 0159: Job 48Y, NACA 0012 angle-of-attack sweep up to and through stall (rotated mesh, a finer O-grid, steady SA and SST, SA-DDES where steady fails)

Status: **registered 2026-10-29**, part 1 (design, set-up, every acceptance limit and every reading rule), before any
solver run of this job; the iteration cap of the steady arms and the time step and end time of the transient arm may be
changed by amendment 1 after the disclosed smoke runs (S0 below) and before the counted runs; nothing else.
**Amendment 1:** the registered mesh was defective (it diverged at the trailing edge); corrected before any counted run.
**Amendment 2:** the report-only SST arm was reduced for cost. **Results in the addendum:** every registered limit is
met (L3 only with the registered extension to 24 degrees); no known miss. Not yet accepted by the maintainer. Job 48Y,
on local main 4e795b1 (waves 48A-48W merged; ADR 0158 is job 48X on another branch). House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed here before the registered runs; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before
the corrected run; defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not
re-recorded; no new crate, no unsafe code; no commit trailers; nothing is pushed. Clean-room: our own code, public
experimental data only (the NASA TMR copy of Ladson 1988 that the repository already holds). Parents:
[ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md) (`[mesh.transform]`, alpha = -`rotate_z_deg`),
[ADR 0156](0156-job48v-adaptive-pressure-correctors.md) (`solver.corrector_control = "adaptive"`),
[ADR 0142](0142-wave48h-example-projects.md) / [ADR 0150](0150-job48p-linux-example-stores.md) /
[ADR 0157](0157-job48w-merge-48u-rotated-cases-with-the-linux-tree.md) (the example projects, their per-platform trees
and size caps), [ADR 0092](0092-wave39a-partials-closeout-rules-fluent-render.md) (the NACA 0012 O-grid route).
[ADR 0153](0153-job48s-time-step-set-when-a-run-starts.md) (the Studio's time step) is read for context only: these runs
go through `rcfd run`, not the Studio, and set `solver.dt` in the case.

## The request (the maintainer, 2026-10-29, relayed)

"Create angle of attack simulations up until full stall." The details were not fixed; the framing approved for this job:
alpha 0 to 20 degrees in 2-degree steps plus 1-degree steps from 12 to 18; the angle set by the mesh rotation key, not by
turning the flow; a mesh fine enough to be credible (a finer mesh or a C-grid, decided here from what the repository's
NACA mesh route supports); transient runs (PISO/PIMPLE, adaptive correctors) at the high angles where steady runs fail to
converge; cl(alpha), cd(alpha) and the stall angle compared with published NACA 0012 data, honestly and without tuning;
a storage route with the smaller footprint (the example trees have about 175 KB left under their caps); a driver
`scripts/w48y_aoa_sweep.py` that builds the cases, runs them through the `rcfd` command line, collects cl and cd
(time-averaged with min and max for unsteady runs) and writes a table and small plots.

## Context (read at 4e795b1, before this registration)

* **The example aerofoil** (`example-projects/wind-tunnel/cases/naca0012-aoa4-rotated.toml`, ADR 0142, 0155): Re 6e6
  (nu = 1/6e6, U = c = 1), k-omega SST, SIMPLEC, linear upwind, 2000 iterations; mesh `meshes/naca0012-ogrid`, the
  `cylinder_ogrid` generator (128 around, 48 radial, grading 200) written by `rcfd convert` and warped onto the aerofoil by
  `scripts/make_example_projects.py` `naca_warp` (a straight blend from the aerofoil point to a far-field circle of 20 chords
  about mid-chord). First cell about 0.01 chord (wall y+ about a thousand), maximum non-orthogonality 80 degrees.
  Results: cl 0.348 at 4 degrees, 0.199 at 2 (a slope well below 2 pi), not converged at the 2000-iteration cap; at 8 degrees
  the same set-up did not converge ("lift swung between -0.1 and 0.9"); its notes say "a finer O-grid or a C-grid is
  needed for higher angles".
* **No aerofoil C-grid exists.** `cases/naca0012/case.json` says "requires an airfoil C-grid generator (not yet
  implemented)"; `crates/rcfd-mesh/src/fan_cgrid.rs` is a 3D blade-passage builder of the fan (periodic seams, tip gap), not
  a 2D aerofoil grid. The supported route is the O-grid warp above.
* **Transient turbulence.** `crates/rcfd-api/src/turb.rs` (`check`): k-epsilon, k-omega SST and plain Spalart-Allmaras run
  with `simple`/`simplec` only; the transient closures are SA with `turbulence.sa.des = "des"` or `"ddes"`, the LES models
  (`smagorinsky`, `wale`), `mixing_length` and `rsm` (no wall treatment). ADR 0136 records "URANS missing". So a transient
  run of this aerofoil at Re 6e6 with a wall-bounded RANS closure means SA-DDES.
* **Adaptive correctors** (ADR 0156 D5): accepted with `piso`/`pimple`, `time_advance = "iterative"`, constant density, no
  dynamic mesh, no `error_tolerance`; turbulence is not excluded. ADR 0156's census found that the pre-run warning SV301
  can miss cases whose viscosity a model sets before each step (eddy viscosity); the adaptive control itself estimates
  inside the step and is not affected.
* **Case keys.** `solver.max_iterations` caps steady runs only; a transient run ends at `solver.end_time`. A steady case
  takes `output.store_interval`, a transient one `output.store_time_interval` (ADR 0156 amendment 2: the parser refuses the
  other one). The `y_plus` report is documented for k-epsilon runs only, so y+ is estimated here, not reported by the run.
* **Reference data in the repository**: `validation-data/reference/w31c_naca0012_ladson1988_80grit_cl_cd.csv` (Ladson,
  NASA TM 4074, 1988, via the NASA TMR, Re 6e6, M 0.15, transition tripped with 80 grit: cl_max 1.6116 at 17.13 degrees,
  0.9967 at 18.02; cd 0.00809 near 0 degrees; the linear-range slope about 0.108 per degree). The Abbott and von Doenhoff
  digitisation is listed in `docs/validation-data.md` (fetched on demand, not in git); it is not used here.
* **Storage.** ADR 0157 T5: the wind-tunnel trees are 173 707 (Windows) and 176 667 (Linux) bytes under their 35 000 000
  caps; an aerofoil run of the example is about 3.7 MB. A run on a 32 768-cell mesh with one snapshot is still of order a
  megabyte (mesh, monitors, residuals), so 14 or more angles do not fit, in either tree.

## Decisions (mine; none is a maintainer decision; the open points are questions at the end)

1. **D1 Angles.** Base sweep alpha = 0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16, 17, 18, 20 degrees (14 angles). Extension 22 and
   24 only by D8. Each angle by `[mesh.transform] rotate_z_deg = -alpha`, `rotate_origin = [0.25, 0, 0]` (ADR 0155); the
   flow is along +x, `reports.cl.direction = [0, 1, 0]`, `reports.cd.direction = [1, 0, 0]`, `a_ref = 0.1` (c x depth),
   `u_ref = 1`; every angle starts from the free stream (no continuation between angles: restart files exist only for
   steady runs without turbulence, `rcfd run --help`), so hysteresis is not explored.
2. **D2 Mesh: a finer O-grid through the repository's route, with wall-normal grid lines.** The `cylinder_ogrid` generator
   written by `rcfd convert` and every point moved onto the aerofoil by the driver (`scripts/w48y_aoa_sweep.py`, its own
   `warp`; `make_example_projects.py` is not changed, only its `naca_thickness` is imported). Two changes against the
   example's straight blend, both needed for a wall cell of about 1e-5 chord: the grid line leaves the wall along the
   surface normal and turns into the straight line to its far-field point over the first `blend` = 0.1 of the radial
   coordinate (smoothstep); near the sharp trailing edge the normal is turned towards +x with the weight
   exp(-(theta/0.05)^2) (`te_fan` = 0.05 rad, about two cells), so that the two lines next to the trailing edge do not
   jump by the trailing-edge angle. Far field: a circle of **50 chords** about mid-chord (the example: 20), to reduce the
   circulation error of a fixed free stream on the boundary (of order cl c / (4 pi R)). **M1** (the sweep mesh): 256 cells
   around, 128 radial, grading 3e5 (last/first), 32 768 cells; first cell at mid-chord 1.574e-5 chord, a flat-plate
   estimate (White's Cf at x = 0.5) of the wall-cell centre y+ of 1.9; `rcfd check-mesh`: maximum non-orthogonality 77.6
   degrees (the example mesh: 80.3), skewness 3.16, aspect ratio 6411, volume ratio 6.9, no error (warnings SV101 > 70
   degrees, SV103 aspect > 1000, as the example's). Points file and topology sha256
   `5d5c89177c7a18723333202023dd9c6dca90c09e59c3522fa3db949ab52523f4` (this host; the driver writes it with the mesh).
   **M2** (grid check only, D9): 384 x 192, grading 1.9616e5 (the same first radial step as M1), 73 728 cells, 77.2
   degrees, sha256 `8889822656d1da0e5d2c73b9aa91592b86c64aca9aec65c03a5c4cca5dcba5bd`. How these were chosen: by
   `rcfd check-mesh` of candidate meshes only, no flow solved (below, "What I knew"). Why not a C-grid: none exists for a
   2D aerofoil (Context); writing one is a mesh-generator job of its own (question 2).
3. **D3 Steady arms.** **SA** (primary): `turbulence.model = "spalart_allmaras"` with its defaults (SA-noft2, SA-neg,
   `wall_mode = "auto"`), SIMPLEC, `convection = "linear_upwind"`, every other key at the example's value or default;
   **SST** (report only): the same with `k_omega_sst` (the example projects' model). Why SA is primary: the only
   transient closure with a wall treatment is SA-DDES (Context), whose attached region is SA; with SA as the steady model
   the curve keeps one attached-flow model on both sides of the steady/transient join; SST is kept as a column so that the
   sweep ties to the example. Iteration cap **8000** (amendment 1 may change it after S0), store: the last iteration only
   (`store_interval` = the cap). One thread per run (results do not depend on it, `docs/run-control.md`).
4. **D4 Steady value and the "fails to converge" rule.** The residual targets (1e-6) are not used: the example's runs
   never reach them. Over the **last 1000 iterations** of a steady run: mean, min and max of cl and cd; the run is
   **steady** when cl's max - min <= **0.005** and cd's max - min <= **0.0005**; its value is the window mean. A run that
   is not steady is "unsteady (iterations)": its window mean, min and max are reported, never as a converged value.
5. **D5 Transient arm DD.** SA-DDES (`turbulence.sa.des = "ddes"`, the other SA keys as in D3), `algorithm = "piso"`,
   `corrector_control = "adaptive"` (ADR 0156), `n_correctors` 2 (default; the adaptive search starts there), linear upwind,
   Euler (the default time scheme), from the free stream; **dt 0.01** c/U, **end time 40** c/U (amendment 1 may change both
   after S0); store: the last time only. Statistics over the **second half** [end/2, end]: the time mean (trapezoid), min
   and max of cl and cd; the half-window drift (the mean over [3 end/4, end] minus the mean over [end/2, 3 end/4])
   reported; a chord Strouhal number
   1/(mean period of the upward crossings of cl - mean) when at least four crossings lie in the window. **Which angles:**
   every angle at which SA is not steady (D4), and the two highest SA-steady angles below the lowest unsteady one (overlap,
   to measure the model offset at the join); if SA is steady at every base angle, DD runs at 18 and 20 only (report only).
   **dt fallback ladder:** if a DD run ends with a `corrector_cap` warning (ADR 0156 D4), is stopped by the guard, or exits
   with a code other than 0, it is run again with dt halved, at most twice (0.005, 0.0025; ids `DD-aNN-dt1`, `-dt2`); the
   first run of an angle that completes is the counted one. Wall cap 12 h per DD run (`--max-elapsed`).
6. **D6 Storage: the driver, a results table and plots in `docs/investigations/job48y-aoa-sweep/`; runs on demand.** Of
   the two routes of the brief this is the smaller: no run, mesh or field store enters git; the committed files are
   `README.md`, `results.json`, `results.csv`, `histories.csv` (cl and cd of every counted run, at most 201 samples per
   run) and the PNG plots `cl_alpha.png`, `polar.png`, `histories.png`. **Budget (registered): the folder at most
   400 000 bytes, each PNG at most 120 000 bytes** (B1). `example-projects*/` and their caps are not touched. The runs are
   rebuilt by `python scripts/w48y_aoa_sweep.py mesh|cases|run|collect` in a work folder outside the repository (this job:
   `E:\projects\cfd\w48y-work\w`).
7. **D7 The sweep curve C(alpha).** At each angle: the SA window mean when SA is steady; otherwise the DD time mean when the
   counted DD run completed; otherwise the SA window mean, flagged. Min and max are those of the source. The SST arm and
   the DD overlap runs are reported next to it, not in it.
8. **D8 Extension.** If L3 finds no stall on the base angles, the SA (and, by D5's rule, DD) runs at 22 and 24 degrees are
   made and L3 is evaluated on the extended curve. No further extension.
9. **D9 Grid check G1 (report only).** SA on M2 at 6 and 14 degrees (the cap of D3); the differences of cl and cd against
   M1 are reported.
10. **D10 Limits as tests.** `crates/rcfd-api/tests/aoa_sweep_w48y.rs` reads the committed `results.json` and asserts each
    registered limit (no solver run); a met limit is a plain test, a missed one an ignored `*_known_miss` test with the
    limit unchanged. The driver's `selftest` checks its statistics, the D4/D5/L3 rules and the warp (P1).
11. **D11 Placement.** New: `scripts/w48y_aoa_sweep.py`, `docs/investigations/job48y-aoa-sweep/`, the test file of D10, this
    ADR. No crate source, no `Cargo.toml`/`Cargo.lock`, no example file, no existing test changes.

## Registered limits (fixed here, before any solver run of this job)

Set-up: release build of 4e795b1's code (`cargo build --release --locked -p rcfd-cli`, own `CARGO_TARGET_DIR`), the binary
copied out of the target folder before the counted runs; `rcfd run <case> --case-dir <work>/cases --status-every 1
--status-monitors --threads 1 --quiet --max-elapsed <cap>` (the driver's `run`), at most 4 runs at a time, free memory
checked before each batch (other agents share the 94 GB machine).

| Id | Check | Limit |
|---|---|---|
| L1 | Runs complete | every counted run: SA, SST and SA2 `finished` with exit code 0 (converged) or 5 (iteration cap); DD reaches its end time with exit code 0 (after D5's ladder). Any other end (divergence, guard stop, wall cap) is a miss of that run |
| L2 | Lift slope | least squares cl = a + b alpha (alpha in radians, intercept fitted) through C at 0, 2, 4, 6, 8 degrees: **0.90 <= b / (2 pi) <= 1.05** (Ladson's 80-grit points from -4 to 8.3 degrees give 0.98; thin-aerofoil theory 1) |
| L3 | Stall | on C (D7, D8): with cl_max = max C at alpha_peak, some alpha > alpha_peak of the sweep has **C(alpha) <= 0.90 cl_max** (a drop of at least 10 percent after the peak). Not found: "no stall", a known miss |
| L4 | Symmetry | SA at 0 degrees: **abs(cl) <= 0.005** |
| B1 | Storage | `docs/investigations/job48y-aoa-sweep/` <= 400 000 bytes; each PNG <= 120 000 bytes; nothing under `example-projects*/` changed |
| P1 | Driver | `python scripts/w48y_aoa_sweep.py selftest` passes |
| G0 | Hygiene | `cargo fmt -p rcfd-api -- --check` clean, `cargo clippy --release -p rcfd-api --tests -- -D warnings` clean (the new test file); `git diff 4e795b1 -- crates/rcfd-api/tests/baseline_hashes.rs` empty |

**Reported, no limit** (so that nothing is tuned to them): cl_max, alpha_peak and the first angle below 0.9 cl_max
against Ladson (1.6116 at 17.13, the drop between 17.13 and 18.02); cl and cd at every angle against Ladson interpolated
(cd near 0 degrees against 0.00809); the SST curve, its slope factor and its stall by L3's rule; the DD overlap offsets
(DD time mean minus SA at the overlap angles); the DD drift, Strouhal number, corrector counts (mean, max) and
cap-bound estimates; the grid check G1; the wall time of every run; the mesh hash.

**Reading rules.** A limit is evaluated once, on the counted runs, by `collect` as committed. A smoke run (S0) is never
counted. If an L1 miss leaves an angle without a value, C uses D7's fallback and the miss is reported with the angle.

## Smoke runs before the counted runs (S0, disclosed, not counted)

`smoke-SA-a06` and `smoke-SA-a16` (SA, 3000 iterations) and `smoke-DD-a16` (DD, dt 0.01 to t = 2): cost per iteration
and per step, the corrector count of the adaptive control at dt 0.01, and whether the steady arm settles within the cap.
Amendment 1, written after S0 and before any counted run, may change only the steady cap (D3), dt and the end time (D5),
each with its reason; the windows (last 1000 iterations; the second half of a DD run), the rules and the limits stay.

## What I knew when writing this (rule d)

* The files read above, and the mesh probes: candidate meshes built with `rcfd convert` and checked with
  `rcfd check-mesh` only (no flow solved). The straight blend of the example at 256 x 128 with a wall cell of 4e-6 chord:
  86.8 degrees, refused (> 85); the normal-blended lines without a trailing-edge fan: 85.5 degrees and a volume ratio of
  960 at the trailing edge; with `te_fan` 0.05 and wall cells of 4.3e-5, 1.6e-5 and 4.1e-6 chord: 70.6, 77.6 and 87.2
  degrees; 128 x 64 with M1's wall cell: 85.7 to 86.5 degrees for every `te_fan` tried (0 to 0.07), so a coarser grid
  check was replaced by the finer M2 (77.2 degrees). M1 is the finest wall spacing (y+ about 2) that stays well under
  the 85-degree refusal.
* `rcfd validate` of one case per arm (SA, SST, DD, SA2 at 16 and 14 degrees, SA at 0): valid, warnings SV101 and SV103
  only; `python scripts/w48y_aoa_sweep.py selftest` passed. No solver iteration of this job had run.

## Honest differences from the experiment (stated before any result)

Re 6e6 is Ladson's Reynolds number, so the comparison is at the measured condition, but: incompressible here against
M 0.15 in the tunnel; 2D, without the tunnel's walls and side-wall effects; a far field of 50 chords with a fixed free
stream (no point-vortex correction); fully turbulent RANS against a tripped boundary layer; SA and SST are not calibrated
for stall, and steady RANS of a separating aerofoil is known to depend on the model; SA-DDES in 2D gives coherent
two-dimensional shedding, not resolved turbulence (its "LES" region has no third direction); each angle starts from the
free stream, so a hysteresis loop of the experiment would not be seen.

## Out of scope

An aerofoil C-grid generator; transient SST or SA (URANS) without DES; compressibility; a run in the example projects;
Linux runs (the curve is computed on this host; the meshes' last bits depend on the platform's libm, ADR 0137); tuning
of any model coefficient or mesh parameter to the data.

## Amendment 1 (2026-10-29, after S0 and disclosed diagnostic runs, before any counted run): the sweep mesh M1 is defective; no stop before the cap

What had run when this was written (binary `rcfd-4e795b1.exe`, the registration's build; work folders
`E:\projects\cfd\w48y-work\w` and `...\diag`, outside the repository):

1. **S0 as registered, on M1: all three diverged at the trailing edge.** `smoke-SA-a06` stopped by the steady divergence
   guard at iteration 15 (velocity runaway, cell 0 at (0.9958, -0.0784), a trailing-edge wall cell of the rotated mesh),
   `smoke-SA-a16` at iteration 11 (cell 0), `smoke-DD-a16` at step 8 (the adaptive control took 5 correctors at step 1,
   rho 0.39; at step 6 the search ended at the cap of 10 with rho 1.049 at cell 255, the other trailing-edge wall cell,
   then the PIMPLE guard stopped the run on a Courant number of 4e6; cl swung from 0.64 to -2.8 to -38 in steps 4-6).
2. **Diagnostics (not registered, not counted; a scratch script `diag.py` that writes the driver's case text with another
   mesh path and runs it).** SA and SST at 6 degrees on the example's mesh: 300 iterations, no divergence. On M1: SST at 6
   degrees diverged at iteration 16 (cell 0); M1 without the trailing-edge fan, SA at 6: iteration 24 (cell 255); the M1
   grid with a wall cell of 4.3e-5 chord (grading 1e5): iteration 26 (cell 254). With wall cells of 1.3e-4 (grading 3e4)
   and **3.5e-4** chord (grading **1e4**): 300 iterations each, no divergence. On the grading-1e4 grid (bit-equal points to
   M1w below): SA-DDES at 16 degrees, dt 0.01, 100 steps (t = 1): no divergence, 2 correctors at every step (one estimate),
   0.3 s per step; SA at 6 and 14 degrees (stopped by me after 2751 and 2746 of 12000 iterations to free the CPU) and
   SST at 6 (2972): SA's cl falls and cd rises slowly (6 degrees: cl 0.6383 / 0.6248 / 0.6219 and cd 0.0101 / 0.0157 /
   0.0168 at 500 / 2000 / 2751; 14 degrees: cl 1.356 / 1.303 / 1.295, cd 0.042 / 0.065 / 0.069), the increments shrinking by
   about 0.7 per 500 iterations; SST at 6: cl 0.64405, cd 0.00611 from iteration 500 on. SA's momentum residual was
   1.1e-6 at 2751 iterations, next to the 1e-6 target, while cd still rose by about 3e-4 per 250 iterations. Cost on the
   loaded host (total CPU 70 to 100 % with other agents): 0.17 to 0.5 s per iteration on 32 768 cells.

**Defect 1 (the mesh, D2).** M1's wall cell of 1.6e-5 chord makes the two trailing-edge wall cells slivers (face angles
8.4 and 171.7 degrees, aspect ratio in the thousands), and both solvers diverge there within 30 iterations or steps; the
mesh is not usable with this solver. Wall-resolved spacing (y+ about 1-2) is therefore not reachable on this O-grid route;
the finest wall cell that ran is 1.3e-4 chord (centre y+ about 15, the buffer layer, where a wall law is least accurate).
**Corrected:** **M1w**, the M1 grid with grading **1e4**: first cell 3.494e-4 chord at mid-chord, flat-plate centre
y+ 41 (the log layer, SA's `wall_mode = "auto"` uses the Spalding law there), `rcfd check-mesh` maximum non-orthogonality
73.5 degrees, skewness 0.70, aspect ratio 657, volume ratio 4.9, no error; sha256
`bd8ef3c6da2dd3681d38c18914c8fbdcddab77bb1381c0688e1e2dce5f0c0299`. Grid check **M2w**: 384 x 192, grading 6412.8 (M1w's
first step), 73 728 cells, 78.4 degrees, sha256 `54eb17c6c3ef90f658d1947374684945b257e997779891904f5982a7a47d6a90`. Every
other mesh parameter (far field 50, `blend` 0.1, `te_fan` 0.05) as registered. M1 and M2 stay in the driver for the S0
record; the counted arms use M1w and M2w.

**Defect 2 (the stop, D3).** D3 meant every steady run to reach its cap so that the window is the last 1000 of 8000
iterations; with the default residual targets a run can stop much earlier (SA at 6 degrees was at 1.1e-6 by 2751) while its
forces still drift, and the window would then be read on a run that ended by chance. **Corrected:** steady cases set
`solver.min_iterations` = the cap (the targets are then only checked at the cap), so every counted steady run ends at
iteration 8000 with exit code 0 or 5.

**Unchanged** (with reasons): the cap 8000 (the diagnostic increments of SA's cd shrink by about 0.7 per 500 iterations, so
D4's cd range is met by about 4200 iterations at 6 degrees and 5400 at 14; 8000 leaves margin); DD's dt 0.01 and end time 40
(2 correctors at dt 0.01 on M1w, 0.3 s per step, about 20 min per run unloaded); the windows, D4-D8, every limit. No other
set-up value was changed after seeing these numbers; in particular nothing in the model, the far field or the schemes.

**Order of the counted runs (cost, not a rule):** the SA arm (14 angles) first, then the DD runs that D5's plan asks for,
then SA2, then SST; at most 4 at a time. A run of a later arm that cannot be made in this job is reported as not run.

## Amendment 2 (2026-10-29, before any SST run): the report-only SST arm on fewer angles and a shorter cap

What had run when this was written: the counted SA runs at 15, 16, 17, 18 and 20 degrees (finished, each steady by D4),
SA at 8 to 14 and the extension 22 and 24 (started before SA at 20 had finished, because with SA's cl rising through
1.4988 at 18 to 1.5277 at 20 no base angle could give D8's drop after a peak at 20; D8's rule is re-evaluated on the
finished runs in the addendum) and SA2 at 6 and 14 were running; no SST and no DD run. On the loaded host the steady runs
take 0.5 to 1.1 s per iteration (9 of my processes next to other agents' builds and containers, total CPU 100 %), so a
steady run of 8000 iterations takes 1.2 to 2.4 hours. The SST arm at all 14 (16) angles would take another 6 or more
hours. **Changed (report-only arm, no limit depends on it):** SST runs at **0, 4, 8, 12, 14, 16, 18, 20** degrees (and 22,
24 when its own curve shows no 10 % drop by 20), with a cap and `min_iterations` of **4000** (the diagnostic SST run at 6
degrees on M1w held cl 0.64405 and cd 0.00611 to every printed digit from iteration 500 to 2972); D4's window (the last
1000 iterations) and rule are unchanged. The SA, DD and SA2 arms are unchanged.

## Addendum: results (2026-10-29)

Commits: 17cfe6d (registration and the driver), d9ba558 (amendment 1), a79aa5c (amendment 2), then the results, the
limit tests and this addendum. All counted runs used `E:\projects\cfd\w48y-work\rcfd-4e795b1.exe`: the registration's
release build of 4e795b1's code, copied out of the target folder. Work folder `E:\projects\cfd\w48y-work\w` (meshes,
cases, runs); results in [`docs/investigations/job48y-aoa-sweep/`](../investigations/job48y-aoa-sweep/README.md). The host
was shared for the whole job: total CPU was at 100 %, free memory 5 to 34 GB, and up to 9 of my single-thread runs went at
a time. Wall times (results.json) are therefore rough.

### Limits

| Id | Limit | Measured | Outcome |
|---|---|---|---|
| L1 | every counted run complete | 28 of 28: 16 SA (0-24 degrees; exit 5 at the cap except 0 and 17, which met the targets at iteration 8000, exit 0), 8 SST (exit 5), 2 SA2 (exit 5), 2 DD (t = 40 reached, exit 0, no `corrector_cap` warning). No guard stop and no wall cap | met |
| L2 | 0.90 <= slope / 2 pi <= 1.05 | 0.1009 per degree = 5.779 per rad = **0.920** x 2 pi (intercept 0.0083) | met (0.02 above the lower edge) |
| L3 | some cl after the peak <= 0.90 cl_max | base sweep: peak at 20 degrees (1.5278), nothing after it, so no stall, and D8's extension ran. With 22 and 24 degrees: 1.3951 (0.913) and **1.2039 (0.788)**. Stall detected, first below 0.9 at 24 degrees | met (with the extension) |
| L4 | abs(cl(0)) <= 0.005 | -2.3e-11 | met |
| B1 | folder <= 400 000 bytes, each PNG <= 120 000 | 288 909 bytes (7 files); PNGs 37 520, 35 009, 80 749 | met |
| P1 | selftest | passed (registration, amendments, final) | met |
| G0 | fmt, clippy, baseline_hashes untouched | `cargo fmt -p rcfd-api -- --check` clean; `cargo clippy --release --locked -p rcfd-api --tests -- -D warnings` clean; `git diff 4e795b1 -- crates/rcfd-api/tests/baseline_hashes.rs example-projects example-projects-linux` empty | met |

**Correction (2026-10-29, ADR 0164 amendment 1, ADR 0166):** in L1, SA-a00 met the residual targets at iteration 5157 (exit 0, `monitors.csv` 5157 rows), not 8000; only SA-a17 met them at 8000.

D10's tests: `crates/rcfd-api/tests/aoa_sweep_w48y.rs`, 5 passed (L1, L2, L3, L4, B1; no known miss). They recompute the
slope and the stall rule from the committed curve and agree with the driver.

### The sweep curve and the report-only results

The table with Ladson is in the folder's README; the figures are `cl_alpha.png` and `polar.png` there.

* **D5's plan.** SA was steady by D4 at all 16 angles (the largest cl range over the last 1000 iterations was 8.3e-4, at
  22 degrees), so the curve is SA throughout. DD ran at 18 and 20 degrees only, as D5's no-unsteady-angle case says.
* **SA-DDES** (report only).
  * 18 degrees: cl 1.343 (1.333 to 1.380), cd 0.0666. That is 0.156 below steady SA (1.499), and nothing sheds.
  * 20 degrees: cl 0.864 (0.144 to 1.673), cd 0.390, chord Strouhal 0.239 over 4 periods; half-window drift -0.21 in cl.
    The flow separated where steady SA (1.528) stays attached, and the mean over [20, 40] is not settled.
  * Correctors: means 2.75 and 2.25, at most 3, never cap-bound; 2736 to 2883 s per run.
* **SST** (report only).
  * Slope 0.970 x 2 pi over 0, 4 and 8 degrees.
  * Steady to 14 degrees (cl 1.3973, the peak). Not steady by D4 at 16, 18 and 20 degrees: iteration means 0.886,
    0.968, 0.946, with the cl range over the window up to 2.3. Its own L3: stall detected, first below 0.9 at 16.
  * Drag 0.0036 / 0.0047 / 0.0083 / 0.0162 / 0.0232 at 0 / 4 / 8 / 12 / 14 degrees, against Ladson's 0.0081 / 0.0082 /
    0.0103 / 0.0132 / 0.0160.
* **Grid check G1** (SA, M2w against M1w).
  * 6 degrees: cl 0.6232 against 0.6185 (+0.8 %), cd 0.0176 against 0.0182 (-3.5 %).
  * 14 degrees: cl 1.2913 against 1.2872 (+0.3 %), cd 0.0707 against 0.0734 (-3.6 %). M2w at 14 is not steady by D4:
    the cd range is 8.8e-4, above 5e-4.
* **Against Ladson** (80 grit; cl_max 1.612 at 17.1 degrees, 1.00 at 18.0; slope 0.985 x 2 pi).

  | | cl_max | at | slope / 2 pi |
  |---|---|---|---|
  | SA | 1.528 (-5 %) | 20 degrees (+3) | 0.920 |
  | SST | 1.397 (-13 %) | 14 degrees (-3) | 0.970 |

  The two RANS models bracket the measured stall angle by about 3 degrees on either side. SA's lift falls gradually
  after its peak; the experiment's drops by 38 % within a degree. SA's drag is 0.0037 at 0 degrees, half the measured
  0.0081 (too low), and 1.4 to 4.6 times the measured value from 4 degrees up, growing with alpha. SST's drag is the
  closer of the two up to 14 degrees.

### Deviations

1. **Amendment 1.** The registered sweep mesh M1 diverged at its trailing-edge cells in every smoke run. The counted runs
   use M1w, with a wall y+ of about 40 instead of about 2. Steady runs got `min_iterations` = the cap. Both changes were
   made before any counted run.
2. **Amendment 2.** The report-only SST arm ran at 8 angles with 4000 iterations, not at every angle with 8000. This
   was done for cost, before any SST run.
3. **Runs started before the rule that asks for them had its last input.**
   * SA at 22 and 24 degrees started while SA at 20 was still running. As amendment 2 says, the base sweep could not
     show a stall with its peak at 20; the finished runs confirm it (L3 on the base: not detected).
   * DD at 18 and 20 started while SA at 0 to 6 was still running. The finished runs confirm D5's plan [18, 20].
4. **Diagnostics.** The diagnostic runs of amendment 1 came from a scratch script (`diag.py`, not committed). I stopped
   3 of them myself, after 2746 to 2972 of their 12000 iterations, to free the CPU.
5. **Sample count.** `histories.csv` keeps at most 101 samples per run (D6 allowed at most 201), to stay well inside B1.
6. **Case titles.** The `title` line was reworded after S0 (it names the mesh). The S0 snapshots keep the original text.
   No setting changed.
7. **Python packages.** The plots use matplotlib from the user's site packages (Python 3.9.13), without `python -I`. The
   driver reads only its own files and `validation-data/reference/`.
8. **Run status.** The registration's "finished" means the `rcfd run` process ended normally. The finish event's
   `state` is `converged` (exit 0) or `not_converged` (exit 5) for the steady runs and `finished` for the DD runs.

### Not run

* SST at 2, 6, 10, 13, 15, 17 degrees (amendment 2). SST at 22 and 24 degrees, which its rule did not ask for since its
  curve drops by 16 degrees.
* DD at every angle but 18 and 20 (D5); the dt ladder (never needed).
* Linux, where the meshes' last bits depend on libm.
* The workspace test suite and the gate: only the new test file, rcfd-api's clippy of its tests and fmt ran. No crate
  source changed, so `baseline_hashes` was not re-run.
* No ADR index lists ADR 0157 or earlier ones, and no test or script self-test reads `docs/adr/` or `docs/investigations/` (`git grep`), so none needed an update or a run.
* A field-level look at the solutions (separation point, nuTilda in the wake). The drag finding below is from the
  monitors only.

## Questions for the maintainer (1-3 at registration, 4-8 from the results; none is decided here)

1. **Storage (D6).** Options:
   - (a) driver plus results, runs on demand (this job, about 0.3 MB);
   - (b) a new example project with its own cap (about 1 MB per stored run, two trees).

   **Recommendation (a).** No existing cap moves, and the runs rebuild with one command.
2. **A 2D aerofoil C-grid generator** (wake cut, sharp or blunt trailing edge) as a mesh job of its own. Options:
   - (a) yes;
   - (b) keep the O-grid warp.

   **Recommendation (a).** The O-grid's trailing-edge slivers are what stopped wall-resolved spacing (amendment 1, and
   question 5).
3. **URANS (transient SST or SA without DES)** is missing (ADR 0136). Options:
   - (a) add transient SST for stall work;
   - (b) keep SA-DDES as the only transient closure.

   **Recommendation (a).** SST is the closer model here up to its stall, and it is the model that fails to be steady
   past 14 degrees (D4), which is where a URANS run is the usual next step.
4. **SA's drag.** The steady SA arm gives cd half the measured value at 0 degrees and 1.4 to 4.6 times it from 4
   degrees up. In the amendment-1 diagnostics, cd also kept rising for thousands of iterations; SST did not do either.
   This may be SA's wall law at y+ about 40, the nuTilda far-field or initial values, or the first-order scheme of the
   turbulence variable. Not investigated here. Options:
   - (a) a registered investigation against the NASA TMR NACA 0012 SA results (alpha 0 and 10, their grids), the
     wall-law path first;
   - (b) accept SA as is for aerofoils.

   **Recommendation (a).**
5. **Trailing-edge divergence at fine wall spacing.** Steady SIMPLEC and adaptive PISO both diverged within 30
   iterations at the two sliver cells of the sharp trailing edge (wall cell 1.6e-5 or 4.3e-5 chord; amendment 1). This
   is the "stable when the resolution changes" concern of ADR 0156's request, on another mesh family. Options:
   - (a) a solver-robustness job with M1 as its test case (the driver builds it: `mesh ... M1`);
   - (b) leave it to the C-grid of question 2.

   **Recommendation (a), then (b).**
6. **Which curve is "the" result.** By the registration it is steady SA, which stalls at about 20 to 24 degrees; SST
   stalls at 14 to 16. Options:
   - (a) present both, as here;
   - (b) designate SST after seeing the data.

   **Recommendation (a).** Re-designating after the comparison would be choosing the model that matches.
7. **SA-DDES at 20 degrees.** The run is not settled (half-window drift -0.21 in cl). Options:
   - (a) run it to t = 100 and add 19 and 21 degrees;
   - (b) leave it reported as is.

   **Recommendation (a)**, if the transient picture of stall matters before URANS exists.
8. **`CHANGELOG.md`.** Not edited, as in the other 48x jobs (ADR 0155 question 6).
