# ADR 0764: F-868 registration: the NACA 0012 at 17 degrees by an unsteady arm

Status: **REGISTERED 2026-11-22, written before any code of F-868 and before any run of its checks.**
This is the second registration under F-868 (rule 13); [ADR 0760](0760-f868-registration-naca0012-lift-curve-against-ladson.md)
is the first and is not edited. Before this text no solver iteration or time step of this work ran, no test was run and
no file of the repository was changed. What was done: reading, the arithmetic below (a scratch script `arith.py`), a
release build of `rcfd-cli` from the unchanged tree, the mesh built by ADR 0167's driver (a conversion; its checksum is
the registered one), and `rcfd validate` and `rcfd run --check-only` of the two case texts given below, written by a
scratch script (the parser's check of the keys and the build of the simulation; neither takes a step). Results go in
addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f868-naca0012-17-degrees-unsteady.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f868-naca0012-17-degrees-unsteady` from main 2bab1863, scratch
`E:/projects/cfd/w49i-work/f868-naca0012-17-degrees-unsteady-scratch`. Parents: ADR 0760 (the band, the steady sweep, the
known miss at 17 degrees, the maintainer's answer),
[ADR 0167](0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md) (the set-up of the steady arm),
[ADR 0159](0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md) (the mesh M1w and the only unsteady runs of this
aerofoil) and [ADR 0210](0210-f294-registration-transient-k-epsilon-and-spalart-allmaras-with-pimple.md) (Spalart-Allmaras
without DES on `pimple`, which did not exist when ADR 0159 and ADR 0167 ran).

## What is asked

ADR 0760 found the lift within its band at the eleven angles from 0 to 16 degrees and no steady value at 17 degrees: the
SIMPLEC iteration ended at its cap with cl swinging between -0.066 and 1.955, iteration mean 1.0476, against the measured
1.6057 (band 0.0803). The maintainer took option (a) of its question 1 on 2026-11-22: the item stays partial and an
unsteady arm at 17 degrees is registered under it. The brief asks for one registration that fixes, before any run, the
band (unchanged), the unsteady set-up, a control at an angle where the steady value is in the band, and a decision rule;
then the run, and a statement of which it is: an unsteady flow whose mean is within the band, a mean outside the band (a
premature stall of this model and mesh, with its number), or a run that does not settle in the registered time.

## What was read

- `AGENTS.md`; `docs/process/agent-rules.md` (version 5) whole; the brief whole; ADR 0760 whole;
  `crates/rcfd-api/tests/naca0012_lift_curve_f868.rs` whole; `scripts/f868_naca0012_lift_curve.py` whole; the entry
  F-868 of `docs/roadmap.json`, by a script; the committed case `SA-a17.toml` and the points of 14 to 17 degrees of
  `docs/investigations/f868-naca0012-lift-curve/results.json`.
- ADR 0159 whole. ADR 0167: its lines 280 to 300 (the sweep's findings) and its question 2, and the lines a search for
  "unsteady", "transient", "URANS", "DDES" and "17 degrees" gave; **not read whole**.
- ADR 0210: its head, "What the code read shows" and the decisions D1 to D7 (lines 1 to 190); not its checks or addenda.
  ADR 0199: only the lines on warm starts and checkpoints (72, 262).
- `docs/case-schema.md`: the Spalart-Allmaras section (lines 165 to 180), the lines on `corrector_control`,
  `min_iterations`, `time_advance` (960 to 1000); `crates/rcfd-api/src/turb.rs`, the function `check` from line 440 to
  470; `crates/rcfd-api/src/schema.rs`, the fields `time_scheme`, `n_outer`, `corrector_control`, `ddt_correction`,
  `store_time_interval`; `crates/rcfd-api/tests/f294_transient_keps_sa.rs` lines 53 to 58 and 192 to 207; the help text
  of `rcfd run`.
- `scripts/w48y_aoa_sweep.py` lines 290 to 560 (case text, run command, the stream reader, `transient_summary`);
  `scripts/w49d_aoa_sweep_fixed.py` lines 150 to 245 and 280 to 332; `scripts/process_report.py`, the sections on
  registrations; [tolerances.md](../process/tolerances.md) whole; `docs/reference-data.d/0760-F-868.part`.

## What exists near and beyond stall (task 1), so that no run is repeated

- **ADR 0159** (2026-10-29; mesh M1w, far field at 50 chords, no far-field role for nuTilda, molecular wall shear in the
  force). Steady SA was steady at every angle to 24 degrees, because of an outer eddy viscosity of about 6e4 nu (ADR
  0167's finding), so its curve does not stall before 20 degrees. Its only unsteady runs were **SA-DDES** with `piso`,
  adaptive correctors, implicit Euler, dt 0.01 c/U to t = 40, statistics over [20, 40], at **18 and 20 degrees**: at 18
  degrees cl 1.343 (1.333 to 1.380), nothing sheds; at 20 degrees cl 0.864 (0.144 to 1.673), chord Strouhal number 0.239
  over four periods, half-window drift -0.21, "not settled". It ran two correctors (three at most) at dt 0.01 on M1w
  without a corrector-cap warning. It states "URANS missing" and lists transient SA without DES as out of scope.
- **ADR 0167** (2026-10-29; mesh M1w-far500, both fixes). Steady only: "there is no DDES fallback in this job (not
  run)". From 17 degrees up the steady iteration does not converge. Its question 2 recommends a DDES or URANS arm at 16
  to 20 degrees; none was run.
- **ADR 0760** (2026-11-22): ADR 0167's SA arm again on current main, bit for bit; "any unsteady arm" is under "Not
  run".

So no time-accurate run exists on M1w-far500 with the two fixes, and none with Spalart-Allmaras without DES on any
mesh of this aerofoil. The runs registered here repeat nothing.

## The band at 17 degrees (unchanged)

    abs(cl_mean - 1.6057) <= 0.0803

1.6057 is Ladson's 80-grit lift interpolated linearly to 17 degrees and 0.0803 is ADR 0760's band there
(max(0.05 x 1.6057, 0.018)); both are taken from ADR 0760's table and row V17 and are recomputed by the test from the
committed 80-grit table. No new reference value enters with this work, so no fragment is added under
`docs/reference-data.d/`.

## Arithmetic done beforehand

Script `arith.py` in the scratch folder; nothing here comes from a solver.

1. **The starting vortex.** A time-accurate run from the free stream is an impulsive start. The aerofoil's circulation
   Gamma = cl U c / 2 is matched by a starting vortex of the opposite sign that the stream carries away; at time t it is
   about d = U t behind the aerofoil. Taken as a point vortex it induces a downwash Gamma / (2 pi d) at the aerofoil,
   which lowers the angle of attack by Gamma / (2 pi d U) and, with a lift slope of 2 pi, the lift by
   `delta cl = cl c / (2 U t)`. This is a first-principles estimate, not a citation. Its mean over a window [a, b] is
   `cl ln(b/a) / (2 (b - a))`: 0.69 % of cl over [50, 100], **0.35 % over [100, 200]**, 0.41 % over [100, 150] and
   0.29 % over [150, 200]. At 15 degrees (cl 1.4932) the mean over [100, 200] is lower than the steady value by 0.0052;
   at 17 degrees with cl 1.6 the two half-window means differ by 0.0019 from this cause.
2. **The time scheme at the registered step.** For `y' = i w y` the second-order backward difference
   `(3 y1 - 4 y0 + y_1) / (2 dt) = i w y1` gives the amplification factor G as the larger root of
   `(3/2 - z) G^2 - 2 G + 1/2 = 0` with `z = i w dt`. Solved numerically: at dt 0.02 an oscillation of period 4.2 c/U
   (210 steps) keeps 0.99996 of its amplitude per period and 0.9997 of its frequency; of period 1.0 (50 steps) 0.9970
   and 0.9948; of period 0.8 (40 steps) 0.9943 and 0.9920.
3. **Whole periods.** The mean of `A sin(2 pi t / P)` over a window of length L that does not hold a whole number of
   periods is at most `A P / (pi L)` in magnitude (the integral over the cut-off part of a period, at most
   `A P / pi`, divided by L). For P = 4.2, L = 50 and A = 0.76 (ADR 0159's swing at 20 degrees) that is 0.020 per half
   window, more than the settled-mean limit below; hence the half-window means are taken over whole periods.

## The unsteady set-up (task 2)

**Model and mesh: those of ADR 0760, unchanged.** Spalart-Allmaras (noft2, negative form, `wall_mode = "auto"`, the
Spalding wall law at y+ about 40) with `sa.des` at its default `off`; mesh M1w-far500 (256 x 128 O-grid, far field at
500 chords, sha256 `59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889`); Re 6e6, incompressible; the
far-field role `turbulence = "freestream"`; the reports `cl_eff` and `cd_eff` with `component = "total_effective"`;
momentum convection `linear_upwind`, nuTilda convection first order. The case text of an angle is ADR 0760's committed
case text of that angle with the title and head comment reworded and exactly two tables replaced, `[solver]` and
`[output]`:

    [solver]
    algorithm = "pimple"
    time_scheme = "bdf2"
    n_outer = 3
    n_correctors = 2
    corrector_control = "adaptive"
    convection = "linear_upwind"
    dt = 0.02
    end_time = 200.0

    [output]
    store_time_interval = 200.0
    store_precision = "f32"
    store_fields = ["ux", "uy", "p", "nut"]

Every other line (mesh, rotation, viscosity, initial state, boundaries, turbulence, reports) is byte for byte that of the
steady case; a test asserts it (T1). `rcfd validate` and `rcfd run --check-only` accept both texts on the build of this
tree (warning SV101, the mesh's non-orthogonality above 70 degrees, as for the steady cases).

- **Algorithm.** `pimple`, the only transient algorithm that takes Spalart-Allmaras without DES (ADR 0210 D4; `piso`
  is a located error). With it nuTilda gets the time derivative of `solver.time_scheme` and one sweep at the start of
  every outer iteration (ADR 0210 D2), and `turbulence.relax` is not used (D5). This is the same model as the steady
  arm, time-accurate; ADR 0159's SA-DDES is another model in separated regions and is not used.
- **Time scheme.** `bdf2`, second order, for the momentum and for nuTilda. **`n_outer = 3`**: with one outer iteration
  the model sweep sees only the old time level (the schema's text; ADR 0210 D4 calls it the lagged update); three is
  the count of ADR 0210's own case-tree runs (`f294_transient_keps_sa.rs`, the rotating-channel case: `bdf2`,
  `n_outer = 3`, `n_correctors = 2`). **Correctors:** at least two, `corrector_control = "adaptive"` (ADR 0156), as ADR
  0159's transient arm on this mesh family: the wall cells of 3.5e-4 chord have a large diffusion number at any usable
  step, which is the case that key is for.
- **Time step: dt = 0.02 c/U.** The shedding expected: ADR 0159's SA-DDES at 20 degrees on M1w is the one unsteady
  result of this aerofoil in the repository, chord Strouhal number 0.239, a period of 4.2 c/U, which dt 0.02 resolves
  with 210 steps (arithmetic 2: the scheme loses 4e-5 of the amplitude per period). The scale argument: a separated
  aerofoil sheds at the scale of its projected height c sin 17 degrees = 0.292 c, so a period much below one chord
  flow time is not expected; a period of 1.0 still has 50 steps (0.3 % of the amplitude per period, 0.5 % of the
  frequency). A period found below 0.8 c/U (40 steps) is reported as under-resolved, and the mean then stands with that
  remark. The Courant number is far above one in the wall cells at any step of this size (ADR 0159 ran dt 0.01 there);
  the scheme is implicit, and the outer iterations and the adaptive correctors are there for that.
- **Start state.** The free stream (`initial.velocity = [1, 0, 0]`, nuTilda = 3 nu), as every steady run of ADR 0167
  and ADR 0760. A start from a steady solution is not used: at 17 degrees there is none, and the control must have the
  start of the run it controls. (ADR 0199, point 6 of its code read, records that the transient RANS path refuses warm
  starts and checkpoints; whether the later `rcfd restart` lifts that was not examined.)
- **Time run: to t = 200 c/U**, 10 000 steps. Why 200: by arithmetic 1 the lift of an attached flow still lacks 0.5 % at
  t = 100 and 0.25 % at t = 200 from the starting vortex; the window below then lies where the lack is 0.35 % in the
  mean (0.0052 at the control angle), under a third of the control's tolerance. ADR 0159's 40 was "not settled" at 20
  degrees.
- **Samples.** The values of `cl_eff` and `cd_eff` at every fifth step (t = 0.1 k), from the status stream of the run.
  At a period of 0.8 that is eight samples a period.
- **The window of the mean: [100, 200]**, the second half (ADR 0159's D5 rule): 1001 samples, at least 23 periods of
  4.2. **The time mean** `M` is the trapezoid mean of `cl_eff` over the window's samples; `Md` the same for `cd_eff`.
- **The rule that says the mean has settled.** Split the window into [100, 150] and [150, 200]. In each half take the
  upward crossings of `cl_eff - M` (linear interpolation between samples); if a half has at least two, its mean `M1`
  or `M2` is the trapezoid mean from its first to its last crossing (whole periods, arithmetic 3), otherwise over the
  whole half. **Settled** when the run reached t = 200 and `abs(M2 - M1) <= 0.018`. The limit is the scatter s of the
  measurement (ADR 0760, check S1): a mean that still moves by more than the three measured sets differ is not a value
  to compare with them. The starting vortex alone moves it by 0.0019 (arithmetic 1).
- **Oscillation.** The range of `cl_eff` over the window (largest less smallest sample) is reported. The flow is called
  **steady in time** when the range is at most 0.005 (ADR 0167's D5 limit on cl), else **oscillating**; then the
  amplitude is half the range, and when the window has at least four upward crossings the frequency is
  1 / (mean spacing of the crossings), reported as a Strouhal number on the chord and on c sin(alpha) (ADR 0159's D5
  rule for the frequency).
- **The fallback, fixed now.** If a counted run does not reach t = 200 with exit code 0, or its stream holds a
  `corrector_cap` warning (ADR 0156: ten correctors were not enough, "lower `solver.dt`"), **both** angles are run once
  more with dt = 0.01 (20 000 steps, samples every tenth step, ids `U-a15-dt1`, `U-a17-dt1`), and that pair is the
  counted one; the first pair is reported. There is no second fallback. Each run is otherwise made once.

**The control (15 degrees).** The same set-up at 15 degrees, the highest angle at which ADR 0760's steady run is steady
by ADR 0167's window rule without a flag (cl 1.4932034, range 7e-8 over the last 1000 iterations; 16 degrees met the
residual targets but is flagged by that rule, so its value is less sharp). The control holds when the run is settled and

    abs(M(15) - 1.4932034) <= 0.018

with 1.4932034 read from the committed `results.json` of ADR 0760. The tolerance is the scatter s again, which the
lead's reading in ADR 0760 fixes as the band of later registrations below stall. If the control does not hold, a
difference at 17 degrees cannot be told from the set-up, and the work stops there.

## Design

### The heavy record

- **Build.** `cargo build --release --locked -p rcfd-cli` of this tree with `CARGO_TARGET_DIR` under the scratch folder;
  the commit and the sha256 of the binary are recorded. No source of any crate is changed.
- **Mesh.** `python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>`; the sha256 must be the one above (P2).
- **Driver.** `scripts/f868_naca0012_lift_curve.py` gets new commands and no change to what its existing ones do:
  `ucases <dir>` (the two case texts, from ADR 0760's by the replacement above), `urun <rcfd> <work> [ids]` (the runs:
  `rcfd run` with one thread each, the status stream with monitors at every step, no limit on wall time), `ucollect
  <work> <outdir>` (the judgement below, `results.json`, the case files) and the extended `selftest`. It is committed
  with the two case texts before any run.
- **Runs.** `U-a15` and `U-a17`, started together, one thread each (three other agents run heavy solves). If the
  machine restarts, a run cut short is made again from the free stream with the same binary and case, and that is
  reported.
- **Record.** `docs/investigations/f868-naca0012-17-degrees-unsteady/`: `README.md`, `results.json` (per run: exit
  code, steps, the corrector counts, warnings, the samples of the window in full precision, samples at every whole time
  unit before it, `M`, `Md`, `M1`, `M2`, the range, the crossings and the frequency, the verdict; the case texts) and
  `cases/U-a15.toml`, `cases/U-a17.toml`.

This heavy record is **not a cargo test**: two runs of 10 000 time steps on 32 768 cells, hours on the shared machine.
It is re-made on demand by the driver.

### What the gate holds

A new test file `crates/rcfd-api/tests/naca0012_17_degrees_unsteady_f868.rs`, header "ADR 0764 (F-868)". None of its
tests is ignored at the start; a registered limit that misses becomes an ignored `*_known_miss` test with the limit
intact and the number recorded (rule 2). The file of ADR 0760 is not changed: its ignored known miss is the record of
the steady arm.

1. **The judgement on the record.** The test reads the committed `results.json`, computes `M`, `M1`, `M2`, the range and
   the verdicts again from the committed samples with its own code, and asserts the rows U17, S17, C15, S15, R1, P2, T1
   and T2 of the table.
2. **The fast test on the same path** (FT17), which runs the solver in the gate: the committed case file `U-a17.toml`
   with exactly one line replaced (the mesh file, by the committed coarse O-grid
   `example-projects/wind-tunnel/meshes/naca0012-ogrid`, 6144 cells, far field at 20 chords), run in process for 50 time
   steps (t = 1.0) by `Simulation::run(50)`. Same case keys, same algorithm, time scheme, outer iterations, corrector
   control, model, far-field role, rotation and force report as the heavy record; another mesh and a short time. It
   asserts that 50 steps were taken and the time is 1.0, that `cl_eff` is finite, and `abs(cl_eff - R17) <= 0.018`,
   where `R17` is the value of the first run of this test on the build of this work, written into the test as a
   constant in the commit that adds the test and stated in the addendum (ADR 0760's FT4 is made the same way). This
   mesh is too coarse to compare with a measurement and one time unit after an impulsive start is not a lift value;
   FT17 is a check against the code's own earlier output on the path the record uses.
3. **Negative controls in the gate.** NC1: the band at 17 degrees rejects ADR 0760's steady iteration mean (1.0476).
   NC2: the control's tolerance rejects `M(15)` judged against the steady values of 14 and of 16 degrees (1.4235 and
   1.5420, 0.070 and 0.049 from the value of 15), so the control tells one degree. NC3: the settled-mean rule calls a
   synthetic ramp `1 + 0.001 (t - 100)` not settled (its half means differ by 0.05) and a synthetic
   `1 + 0.76 sin(2 pi t / 4.2)` settled, while the plain half means of that sine without the whole-period rule are
   shown to differ by more than they do with it.

## Table of checks

Margin is the limit divided by the estimate.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| U17 | acceptance of F-868 at 17 degrees by the unsteady arm: abs(M(17) - 1.6057), the run settled (S17) and complete (R1) | <= 0.0803 | ADR 0760, row V17 (its band, unchanged); the brief | not known: 0.05 if the flow stays attached, 0.2 to 0.6 if it is stalled (ADR 0760's iteration mean is 0.56 low); **a miss is the likelier outcome** | 0.13 to 1.6 | validation |
| S17 | the mean at 17 degrees has settled: abs(M2 - M1), whole periods | <= 0.018 | ADR 0760, check S1 (the scatter s of the three measured sets) | 0.002 for an attached flow (arithmetic 1); not known for a shedding one | 9 | solution verification |
| C15 | the control: abs(M(15) - 1.4932034), the steady value of ADR 0760 at 15 degrees | <= 0.018 | ADR 0760, check S1 and the lead's reading there (the scatter is the band of later registrations below stall); the brief (a registered tolerance) | 0.0052 (arithmetic 1) | 3.5 | solution verification |
| S15 | the mean at 15 degrees has settled: abs(M2 - M1) | <= 0.018 | as S17 | 0.0018 | 10 | solution verification |
| R1 | both counted runs reach t = 200 with exit code 0 and no `corrector_cap` warning, on a binary built from this work's tree with no crate source changed (after the fallback, if it was needed) | 2 of 2 | ADR 0159, L1 (a transient run reaches its end time with exit code 0); ADR 0156 (the warning) | 2 | n/a | code verification |
| P2 | the mesh is ADR 0167's: sha256 of M1w-far500 | equal | ADR 0167, P2 | equal | n/a | unchanged default output |
| T1 | the set-up is the registered one: each committed case file equals the text recorded in `results.json`, names its angle (`rotate_z_deg` = -alpha), and differs from ADR 0760's committed case of that angle in the title, the head comment and the tables `[solver]` and `[output]` only, which are the lines given above | both files | this registration (the set-up above); the brief, task 2 (model and mesh unchanged) | holds | n/a | code verification |
| T2 | the record is traceable: `M`, `Md`, `M1`, `M2` and the range computed by the test from the committed samples against the driver's values in `results.json`, relative difference | <= 1e-8 | [tolerances.md](../process/tolerances.md), row "Same discrete problem by two routes" | 1e-15 | above 1e6 | code verification |
| FT17 | fast test, coarse mesh, 17 degrees, 50 steps: 50 steps taken, time 1.0, `cl_eff` finite, abs(cl_eff - R17), R17 recorded once as said above | <= 0.018 | ADR 0760, row FT4 (the same construction and tolerance) | 0 on the same build | above 18 | code verification |
| NC1 | negative control: the band of U17 applied to ADR 0760's steady iteration mean at 17 degrees, abs(1.0476 - 1.6057) | > 0.0803 | the band of U17, the other way round ([tolerances.md](../process/tolerances.md), its paragraph on negative controls) | 0.558 | 6.9 | validation (negative control) |
| NC2 | negative control: the tolerance of C15 applied to M(15) against the steady values of 14 and of 16 degrees | both > 0.018 | the tolerance of C15, the other way round | 0.070 and 0.049 | 2.7 | solution verification (negative control) |
| NC3 | negative control: the settled rule on a ramp of 0.001 per time unit (not settled) and on a sine of amplitude 0.76 and period 4.2 (settled, half means within 0.018; without whole periods they differ by more) | rejects the ramp, accepts the sine | the limit of S17, the other way round; arithmetic 3 | 0.05 for the ramp; below 1e-3 for the sine | 2.8 | code verification (negative control) |
| RP | report: at each angle the range, amplitude and frequency of `cl_eff` over the window, the Strouhal numbers, steps per period, `Md` with its range and half means for drag, the corrector counts | none (reported) | the brief, task 3 | n/a | n/a | solution verification (reported without a limit) |
| B1 | storage: `docs/investigations/f868-naca0012-17-degrees-unsteady/` | <= 300 000 bytes | ADR 0167, B1 (400 000 bytes for a record with plots) | 150 000 | 2 | unchanged default output |
| E | `git diff --stat 2bab1863` names no file under `crates/*/src/`, no existing test, no golden and no hash table; the test file of ADR 0760 is unchanged | none | rule 3 of `docs/process/agent-rules.md`; the brief ("change no solver source") | holds | n/a | unchanged default output |
| F | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; the selftest of the driver; `cargo test -p rcfd-api --release --test naca0012_lift_curve_f868` (the driver its record came from is edited) | clean; clean; passes; passes; as before (22 passed, 1 ignored) | rule 7 of `docs/process/agent-rules.md` | clean | n/a | code verification |
| P | `python scripts/process_report.py --check` and `python scripts/roadmap_proposals.py check` on the last commit | no failure of this work | rules 7 and 9 of `docs/process/agent-rules.md` | none | n/a | code verification |

## Negative controls and mutations

NC1, NC2 and NC3 run in the gate. Mutations, each on a scratch copy of the tree at the last code commit, release, run
once; what failed is recorded:

- **M1** (committed, `crates/rcfd-api/tests/naca0012_17_degrees_unsteady_f868.mutation.patch`, first line
  `# expect-fail: cargo test -p rcfd-api --release --test naca0012_17_degrees_unsteady_f868`): in the committed case
  file `U-a17.toml`, `rotate_z_deg = -17` becomes `-16`. T1 must fail; whether FT17 fails too (the lift one time unit
  after the start at another angle) is recorded.
- **M2**: in `U-a17.toml`, `time_scheme = "bdf2"` becomes `"euler"`. T1 must fail; FT17 is recorded.
- **M3**: in `results.json`, the recorded `M` of 15 degrees raised by 0.03. T2 must fail (the record no longer follows
  from its samples).
- **M4**: in the test file, the limit 0.018 of S17, S15 and C15 multiplied by ten. NC2 and NC3 must fail.

## Size

Size (agent): partial; 0; 1; 1; 200000

The one heavy run is the pair of time-accurate runs (two solver processes side by side, 10 000 steps each on 32 768
cells with three outer iterations a step; a steady iteration of this mesh took about 0.3 s under load in ADR 0760, so
of the order of three hours), with one release build of `rcfd-cli` and the builds of the test file. Partial is expected
because a miss of U17 is the likelier outcome. The one question foreseen is at the end.

## Decision rule

- **done** only if U17 is met (the time mean at 17 degrees within the band), S17 holds (settled by the registered
  rule), C15 and S15 hold (the control angle reproduces its steady value), R1, P2, T1 and T2 hold, FT17 passes, NC1 to
  NC3 reject, M1 to M4 are each caught, and E, F and P hold.
- **partial** if the control holds (C15, S15, R1) and the mean at 17 degrees is outside the band or not settled, with
  the numbers; or if a gate-side check or a mutation is not in place. The verdict at 17 degrees is then one of:
  "premature stall of this model and mesh" (settled, the mean below the band), "outside the band" (settled, above it),
  or "not settled in the registered time" (with `M`, `M1`, `M2`).
- **stopped and reported** if the control does not hold (C15 or S15 missed, or the control run not complete after the
  fallback), if P2 fails, or if a defect of the solver appears. The run at 17 degrees is then reported with its numbers
  and without a verdict. No solver source is changed by this work.

In each case the known miss of ADR 0760 stays as it is. No limit is changed after a run, no run is repeated to get
another number, and no test is ignored, skipped or retried to make a run pass.

## What is not covered, stated before any result

- **One model, one mesh, one step.** Spalart-Allmaras without DES in two dimensions on M1w-far500 with a wall law at y+
  about 40; no grid-convergence check, no second time step (unless the fallback runs), no SST, no DDES, no third
  dimension. A time-accurate RANS run in two dimensions gives, at most, coherent shedding; it resolves no turbulence.
- **The words of the acceptance.** The acceptance of F-868 says "incompressible steady RANS". A time mean of a
  time-accurate run with the same model is what the maintainer's answer of 2026-11-22 asks for at 17 degrees; if the
  flow comes out steady in time, the value is a steady solution of the same equations that the SIMPLEC iteration did
  not reach.
- **Hysteresis.** Both runs start from the free stream; a stalled or attached branch reached from another start is not
  looked for.
- **The control** shows that the set-up returns the steady value where the flow is attached and steady. It does not show
  that a shedding flow is resolved well; the report gives the steps per period for that.
- **Other angles.** 16 degrees and 18 degrees and above are not run. Drag is reported and not judged.
- **The gate** holds the record, its judgement and 50 steps of the same case on a coarse mesh; it does not re-run the
  two heavy runs. The record is made on Windows only.

## Question foreseen (rule 6: not taken, it changes the scope of the item)

1. If the mean at 17 degrees is outside the band: whether the headline of F-868 then ends at 16 degrees with the early
   stall stated as the model's limit (option (b) of ADR 0760's question 1), or another arm (a finer or wall-resolved
   mesh, SST on `pimple`, DDES) is registered. It is written out with options and a recommendation in the addendum,
   under the fixed heading, once the result is known.
