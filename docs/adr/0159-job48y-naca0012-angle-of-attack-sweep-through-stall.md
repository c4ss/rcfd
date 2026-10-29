# ADR 0159: Job 48Y, NACA 0012 angle-of-attack sweep up to and through stall (rotated mesh, a finer O-grid, steady SA and SST, SA-DDES where steady fails)

Status: **registered 2026-10-29**, part 1 (design, set-up, every acceptance limit and every reading rule), before any
solver run of this job; the iteration cap of the steady arms and the time step and end time of the transient arm may be
changed by amendment 1 after the disclosed smoke runs (S0 below) and before the counted runs; nothing else. Job 48Y,
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

## Questions for the maintainer (open at registration; the addendum may add to them)

1. Storage (D6): accept "driver plus results, runs on demand" for sweeps of this size, rather than a new example project
   with a cap of its own?
2. A 2D aerofoil C-grid generator (wake cut, blunt or sharp trailing edge) as a mesh job of its own, for this sweep's
   follow-up?
3. URANS (transient SST/SA without DES) is missing (ADR 0136); is it wanted for stall work?
