# ADR 0770: F-884 registration: why the steady iteration does not reach the attached NACA 0012 state at 17 degrees

Status: **REGISTERED 2026-11-22, written before any code of F-884 and before any run of its checks.**
Before this text no solver iteration of this work ran, no test was run and no file of the repository was changed. What
was done: reading (below, with the status streams of ADR 0760's runs at 16 and 17 degrees and of ADR 0764's run at 17
degrees, which are records of earlier work), the arithmetic below, a release build of `rcfd-cli` from the unchanged tree
(main 21054b68, sha256 `087038c6b19963fd854693c3bc3821179ac05cdb7a443cc8127f3e9a5462ec89`), the mesh M1w-far500 built by
ADR 0167's driver (a conversion; its checksum is the registered one), and `rcfd validate` and `rcfd run --check-only` of
the five changed case texts given below, written by a scratch script (the parser's check and the build of the simulation;
neither takes an iteration). Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f884-naca0012-17-steady.md` and follows `docs/process/agent-rules.md`, version 5.
Branch `f884-naca0012-17-steady` from main 21054b68, scratch `E:/projects/cfd/w49i-work/f884-naca0012-17-steady-scratch`.
Parents: [ADR 0760](0760-f868-registration-naca0012-lift-curve-against-ladson.md) (the steady set-up and its known miss at
17 degrees), [ADR 0764](0764-f868-registration-naca0012-17-degrees-by-an-unsteady-arm.md) (the time-accurate state, cl
1.5516, and the maintainer's answer that made this item) and
[ADR 0167](0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md) (the window rule D5).

## What is asked

The acceptance of F-884 (`docs/roadmap.json`, read by a script): "On the mesh M1w-far500 with the set-up of ADR 0760 a
steady solve at 17 degrees ends steady by ADR 0167's window rule with a lift coefficient within 0.018 of 1.5516, the
time-accurate value of ADR 0764, by a registered change of the steady route that leaves every default bit-identical, or
the cause of the non-convergence of the SIMPLEC iteration there is diagnosed and recorded with its evidence; a run that
does not decide is recorded as not deciding." The brief: register the candidate causes and one run for each that
changes it alone, with the amount by which a run must move the iteration before its cause is named; fix the route if the
diagnosis shows how; hold the result by a fast test in the gate with the heavy runs as records.

## What was read

- `AGENTS.md`; `docs/process/agent-rules.md` (version 5) whole; the brief whole; ADR 0760 whole; ADR 0764 whole; ADR 0167
  lines 90 to 170 (its decisions D1 to D10, D5 the window rule) and the lines a search for "D5", "window" and "relax"
  gave; **ADR 0167 not read whole**.
- `crates/rcfd-fv/src/simple.rs` lines 1 to 230 (settings and their defaults) and 770 to 1050 (the pseudo time step and
  the momentum predictor with its relaxation); **not read whole**: the pressure equation and the corrector part
  (lines 1050 to 1975) were not read.
- `crates/rcfd-api/src/schema.rs`: the fields `alpha_u`, `alpha_p`, `non_orthogonal_correctors`,
  `simplec_non_orthogonal_compensation`, `pseudo_mode`, `pseudo_dt`, `pseudo_cfl`, `pseudo_factor`, `pseudo_control`,
  `min_iterations`, `initial.method`, `turbulence.relax`, `turbulence.relax_nut` and the Spalart-Allmaras group.
  `crates/rcfd-turb/src/sa.rs`: the lines a search for "relax" gave (the defaults 0.7 and 0.5, line 494; the implicit
  relaxation of the nuTilda matrix, line 874; the explicit relaxation of the eddy viscosity, line 894; the time-accurate
  route relaxes neither, lines 664 to 676 and 865 to 871). `crates/rcfd-api/src/init.rs` lines 150 to 215 (the
  potential-flow start); `crates/rcfd-api/src/runctl.rs` lines 340 to 375 and `docs/case-schema.md` lines 2279 to 2318
  (restart files: not written for a run with a turbulence model).
- `crates/rcfd-api/tests/naca0012_lift_curve_f868.rs` lines 1 to 100, 327 to 340 and 447 to 520 (the fast test's
  construction); `scripts/f868_naca0012_lift_curve.py` lines 1 to 80; `scripts/w49d_aoa_sweep_fixed.py` lines 180 to 300
  (run command, D5 `summary`); the committed case `SA-a17.toml` and the runs SA-a15 and SA-a17 of
  `docs/investigations/f868-naca0012-lift-curve/results.json`; [tolerances.md](../process/tolerances.md) whole;
  `scripts/process_report.py` lines 424 to 523.

## What the earlier runs show (records read, no run of this work)

**ADR 0760's steady run at 17 degrees** (status stream in ADR 0760's scratch folder): from the free stream cl_eff rises
to 1.78 at iteration 40 with cd_eff 0.58, falls to 0.32 at iteration 70, and from then on swings between about -0.15 and
1.95 to the cap of 8000 iterations. It never approaches a steady value. Over the last 1000 iterations (D5's window) the
range of cl_eff is **2.0202** (-0.0655 to 1.9547) and of cd_eff **0.6835**, the window mean of cl_eff 1.0476.

**ADR 0760's steady run at 16 degrees** has the same start: cl_eff 1.73 at iteration 50 with cd_eff 0.59, then swings of
the same size for about 800 iterations; from iteration 850 it settles, and it meets the residual targets at iteration
2008 with cl_eff 1.5436 (window mean 1.5420, flagged by D5). So the attached basin is found at 16 degrees after a long
irregular start, and is not found at 17 degrees in 8000 iterations.

**ADR 0764's time-accurate run at 17 degrees** (`pimple`, BDF2, dt 0.02, three outer iterations, the nuTilda equation
and the eddy viscosity not relaxed): from the free stream cl_eff rises monotonically, 0.93 at t = 0.1, 1.21 at t = 1,
1.49 at t = 4, 1.571 at t = 10, then falls slowly to 1.5511 at t = 200; cd_eff falls from 0.155 to 0.044. Its path never
passes through the strongly separated states (cd_eff 0.3 to 0.7) that the steady iteration visits from iteration 30 on.

So the attached state is a solution of the steady equations that a time-accurate path reaches from the same start, and
the SIMPLEC iteration leaves the neighbourhood of that path within its first 50 iterations. The candidate causes below
are the ways in which the steady iteration differs from the time-accurate path.

## Arithmetic done beforehand

1. **What under-relaxation is as a pseudo time step.** The momentum matrix is relaxed as `a_P / alpha` with the source
   `(1 - alpha)/alpha a_P u_old` (`m.relax(s.alpha_u, ...)`, `simple.rs` line 1029), which is an implicit Euler pseudo
   time step with `V / dt_c = a_P (1 - alpha) / alpha` in every cell. For the upwind part of a divergence-free flux,
   `a_P` is about `0.5 sum |phi_f| + sum nu_f lap_f`, the rate that the `local` pseudo mode divides by (line 791), so the
   relaxation is a local pseudo time step of Courant (and diffusion) number `alpha / (1 - alpha)`: **9 at the default
   0.9, 2.33 at 0.7**. A pseudo step of `local` mode with number `c` on top of the relaxation gives `alpha / (1 - alpha +
   1/c)`: 4.5 for c = 10 at alpha 0.9, so `local` mode tests the same thing as a lower alpha and is not run.
2. **A global pseudo time step.** `pseudo_mode = "physical"` adds `V / dt` with one dt to every cell (lines 1022 to 1028),
   before the relaxation. With dt = 0.02 c/U (ADR 0764's step) the iteration is implicit Euler in a time that is the same
   in every cell, plus the relaxation; 8000 iterations are 160 c/U. The starting-vortex relation of ADR 0764 (arithmetic
   1 there, `delta cl = cl c / (2 U t)`, first principles) gives a lift lack of 0.0048 at t = 160 and a change over the
   last 1000 iterations (t = 140 to 160) of `cl/2 (1/140 - 1/160)` = 0.0007, under D5's 0.005, if the iteration follows a
   time-like path.
3. **The relaxation of the eddy viscosity.** `nut = nut_old + 0.5 (nut(nuTilda) - nut_old)` each iteration (sa.rs line
   894, default 0.5): the eddy viscosity of the boundary layer lags its equation by a factor 2 per iteration, while the
   momentum moves with a pseudo Courant number of 9 (item 1). The time-accurate route does not relax it.
4. **The tolerance cannot tell 16 from 17 degrees.** ADR 0760's steady value at 16 degrees, 1.5420, is 0.0096 from
   1.5516, inside the target 0.018; that at 15 degrees, 1.4932, is 0.0584 away. So the target shows the attached branch
   near 17 degrees, not the angle; the case file's angle is held by T1.

## The candidate causes and their runs (task 1)

Every run is ADR 0760's committed case `SA-a17.toml` (`docs/investigations/f868-naca0012-lift-curve/cases/`) with its
title and head comment reworded and **only the lines named here added**; every other line (mesh, rotation, viscosity,
initial velocity, boundaries, model, reports, `max_iterations = 8000`, `min_iterations = 8000`, output) is byte for byte
that file's. The added lines and their sources:

| Run | Cause it tests | Lines added | Why this value |
| --- | --- | --- | --- |
| K0 | none (the reproduction on this build) | none: the committed `SA-a17.toml` unchanged | ADR 0760's run on main 13b0937d; main has moved to 21054b68 |
| KU | the velocity under-relaxation (a pseudo Courant number of 9, arithmetic 1) | `[solver]` `alpha_u = 0.7` | the library's default for SIMPLE, `SimpleSettings::new`, `simple.rs` line 200: pseudo Courant number 2.33 |
| KS | the start state (the impulsive start from the free stream) | `[initial]` `method = "potential"` | the one other start the steady route offers on a mesh read from a file (ADR 0019's potential-flow start; `coarse` needs the generator); a start from an attached state is not possible, see "Not covered" |
| KT | the coupling of the turbulence model (the eddy viscosity lags, arithmetic 3) | `[turbulence]` `relax_nut = 1.0` | no relaxation of the eddy viscosity, as on the time-accurate route (sa.rs lines 664 to 676) |
| KC | the corrector settings (no non-orthogonal correction on a mesh above 70 degrees, warning SV101) | `[solver]` `non_orthogonal_correctors = 1` and `simplec_non_orthogonal_compensation = true` | one corrector, the least; the compensation is the one ADR 0050 adds for SIMPLEC with correctors and alpha_u near 1 on strongly non-orthogonal meshes (the schema's text of that key), so the two lines are one setting |
| KG | the local character of the pseudo time step (arithmetic 1 and 2) | `[solver]` `pseudo_mode = "physical"` and `pseudo_dt = 0.02` | one global step, that of ADR 0764's run, which reaches the attached state; the two lines are one setting (the mode and its step) |

All five changed texts were accepted by `rcfd validate` and `rcfd run --check-only` on the build named in the status
lines (warning SV101 only, as for the unchanged case). Each run is made once, one thread each, at most three solver
processes at a time (three other agents share the machine), to its cap of 8000 iterations or its own stop. If the
machine restarts, a run cut short is made again from its start with the same binary and case, and that is reported.

**The amount by which a run must move the iteration (registered now).** Every run is judged by ADR 0167's D5, unchanged:
the window is the last min(1000, n/2) iterations; the value is the final iterate when the run met the residual targets
(exit 0) and the window mean when it ran to its cap; **steady** when the window range of `cl_eff` is at most 0.005 and that
of `cd_eff` at most 0.0005. **A cause is named only if its run ends steady by D5**: the window range of cl_eff moves from
the 2.0202 of the baseline to 0.005 or less, by a factor above 400, and that of cd_eff from 0.6835 to 0.0005 or less. A run
whose window ranges fall but stay above those limits is reported with its numbers and names no cause. A run that ends
steady names its change as **a** cause of the non-convergence (a change that removes it alone); several may be named.

**The target (task 2) on every run.** A run meets the acceptance when it is steady by D5 and its D5 value satisfies

    abs(cl - 1.5516) <= 0.018

with 1.5516 ADR 0764's time mean at 17 degrees (its addendum, row U17) and 0.018 the acceptance's tolerance. A run that
ends steady elsewhere (a stalled steady state) names its cause and does not meet the target.

**The control at 15 degrees, for every change that meets the target.** A change of the iteration settings can move the
fixed point (the relaxation and the pseudo step enter the face-flux interpolation through `a_P`). For each change that
meets the target at 17 degrees the same change is run once at 15 degrees: ADR 0760's committed `SA-a15.toml` with the
same lines added (ids `KU-a15` and so on). The change is a fix only when that run is steady by D5 and

    abs(cl - 1.4932034) <= 0.018

with 1.4932034 ADR 0760's steady value at 15 degrees (its `results.json`) and 0.018 the tolerance of ADR 0764's control
C15. The five control texts are written and committed with the others before any run; only those needed are run.

**What a fix is in this work.** The five settings are existing opt-in keys of the steady route, so a fix by one of them
changes no source and leaves every default bit-identical by construction. No change of `crates/rcfd-fv/src/simple.rs` is
planned: a new behaviour there would need a new case key in `crates/rcfd-api/src/schema.rs` and its wiring in
`crates/rcfd-api/src/sim.rs`, which another agent owns (brief). If the runs show that only such a change would reach the
state, this work stops there and reports it; it does not change those files.

## Design

### The heavy record

- **Build** as in the status lines; copied out as `rcfd-f884.exe` in the scratch folder.
- **Mesh** by `python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>`; the sha256 must be
  `59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889` (P2).
- **Driver** `scripts/f884_naca0012_17_steady.py` (new; ADR 0167's and ADR 0760's drivers are imported and not changed):
  `cases <dir>` (the eleven case texts: K0 is the committed `SA-a17.toml`, the five changed texts at 17 degrees and the
  five at 15 degrees, by the replacement above), `run <rcfd> <work> <ids> [--parallel 3]` (`rcfd run` with ADR 0167's
  command line: one thread, the status stream with monitors at every iteration, the last snapshot stored),
  `collect <work> <outdir> [--binary] [--commit]` (D5 by ADR 0167's `summary`, unchanged; the verdicts above; the history
  of `cl_eff` and `cd_eff` at every 50th iteration; `results.json` and the case files) and `selftest`. It is committed
  with the case texts before any run.
- **Record** `docs/investigations/f884-naca0012-17-steady/`: `README.md`, `results.json` (per run: exit code, iterations,
  finish reason, warnings, wall time, D5's window statistics of every monitor, the value, the verdicts, the thinned
  history, the case text), `cases/*.toml` (the ten changed texts; K0's is ADR 0760's committed file). Full status streams
  stay in the scratch folder.

This heavy record is not a cargo test: up to eleven runs of 8000 iterations on 32 768 cells, about 40 minutes each.

### What the gate holds

A new test file `crates/rcfd-api/tests/naca0012_17_degrees_steady_f884.rs`, header "ADR 0770 (F-884)". The test files of
ADR 0760 and ADR 0764 are not changed; ADR 0760's known miss at 17 degrees stays as it is.

1. **The judgement on the record.** The test reads the committed `results.json`, applies D5 to each run's committed window
   statistics with its own code, and asserts for each run the rows N-x and F17-x below (and C15-x for each control run).
   A row that misses becomes an ignored `*_known_miss` test with the limit intact and the number recorded (rule 2); a
   diagnosis run that names no cause is such a miss and is the record of that cause's test.
2. **The fast tests on the same path** (FT-U, FT-S, FT-T, FT-C, FT-G): each committed changed case at 17 degrees with
   three lines replaced (the mesh, by the committed coarse O-grid `example-projects/wind-tunnel/meshes/naca0012-ogrid`,
   6144 cells, far field at 20 chords; `max_iterations` and `min_iterations`, by 600), run in process by
   `Simulation::run(600)`. Each asserts that `cl_eff` is finite and within 0.018 of `R_x`, the value of the first run of
   that test on the build of this work, written into the test as a constant in the commit that adds the test and stated
   in the addendum (ADR 0760's FT4, the same construction). This mesh is too coarse for the attached state; the fast tests
   check the code's own earlier output on the path the record uses, with each setting in force.
3. **Negative controls** (NC1 to NC3, table).

## Table of checks

Margin is the limit divided by the expected value.

| Id | Check | Limit | Source of the limit | Expected | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| F17-x | acceptance of F-884, for each x in U, S, T, C, G: run Kx-a17 steady by D5 and abs(cl - 1.5516) | steady; <= 0.018 | acceptance of F-884; ADR 0764, U17 (the value); ADR 0167, D5 (the rule) | not known: 0.005 for a change that reaches the attached state, 0.5 otherwise; **a miss is likely for most x** | 3.6 or 0.04 | solution verification |
| N-x | the cause is named, for each x: run Kx-a17 steady by D5, window range of cl_eff and of cd_eff | <= 0.005 and <= 0.0005 | ADR 0167, D5 (ADR 0159 D4); the brief, task 1 | not known | n/a | solution verification |
| C15-x | the control, for each x that meets F17-x: run Kx-a15 steady by D5 and abs(cl - 1.4932034) | steady; <= 0.018 | ADR 0764, C15 (the tolerance); ADR 0760, `results.json` (the value) | 0.002 | 9 | solution verification |
| D0 | K0 reproduces ADR 0760's SA-a17: iterations, exit code, and the window mean, minimum, maximum and final iterate of cl_eff and cd_eff | equal bits | [tolerances.md](../process/tolerances.md), row "Bit-identity of default output" | equal | n/a | unchanged default output |
| P2 | the mesh is ADR 0167's: sha256 of M1w-far500 | equal | ADR 0167, P2 | equal | n/a | unchanged default output |
| T1 | the set-up is the registered one: each committed changed case equals the text recorded in `results.json`, names its angle, and differs from ADR 0760's committed case of that angle in the title, the head comment and the registered lines only | all ten files | this registration (the table of causes); the brief, task 1 (each run changes one cause alone) | holds | n/a | code verification |
| T2 | the record is consistent: for every run, window min <= mean <= max for cl_eff and cd_eff, the history points inside the window lie in [min, max], and the recorded value and verdicts follow from the statistics by D5 | all runs | [tolerances.md](../process/tolerances.md), row "Same discrete problem by two routes" (the driver's and the test's judgement) | holds | n/a | code verification |
| FT-x | fast test, coarse mesh, 17 degrees, 600 iterations, each setting x: cl_eff finite, abs(cl_eff - R_x) | <= 0.018 | ADR 0760, row FT4 (the same construction and tolerance) | 0 on the same build | above 18 | code verification |
| NC1 | negative control: D5 applied to ADR 0760's committed SA-a17 window (cl_eff range 2.0202, cd_eff 0.6835) says not steady | not steady | D5, the other way round ([tolerances.md](../process/tolerances.md), negative controls) | not steady | 404 | solution verification (negative control) |
| NC2 | negative control: the target rejects ADR 0760's steady value at 15 degrees, 1.4932034, and its iteration mean at 17 degrees, 1.0476 | both > 0.018 | the target of F17-x, the other way round | 0.0584 and 0.504 | 3.2 | solution verification (negative control) |
| NC3 | negative control: the control's tolerance rejects ADR 0760's steady values at 14 and at 16 degrees (1.4235 and 1.5420) against 1.4932034 | both > 0.018 | the tolerance of C15-x, the other way round | 0.070 and 0.049 | 2.7 | solution verification (negative control) |
| B1 | storage: `docs/investigations/f884-naca0012-17-steady/` | <= 200 000 bytes | ADR 0760, B1 | 90 000 | 2.2 | unchanged default output |
| E | `git diff --stat 21054b68` names no file under `crates/*/src/`, no existing test, no golden and no hash table | none | rule 3 of `docs/process/agent-rules.md`; the brief (opt-in only, defaults bit-identical) | holds | n/a | unchanged default output |
| F | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; the driver's selftest | clean; clean; passes; passes | rule 7 of `docs/process/agent-rules.md` | clean | n/a | code verification |
| P | `python scripts/process_report.py --check` and `python scripts/roadmap_proposals.py check` on the last commit | no failure of this work | rules 7 and 9 of `docs/process/agent-rules.md` | none | n/a | code verification |

`baseline_hashes` is run only if a file under `crates/*/src/` is changed, which is not planned (E).

## Negative controls and mutations

NC1 to NC3 run in the gate. Mutations, each on a scratch copy of the tree at the last code commit, release, run once; what
failed is recorded. The case named "the decisive case" is that of the first change in the order U, S, T, C, G that is a
fix; if none is, the first that names a cause; if none does, KU.

- **M1** (committed as `crates/rcfd-api/tests/naca0012_17_degrees_steady_f884.mutation.patch`, first line
  `# expect-fail: cargo test -p rcfd-api --release --test naca0012_17_degrees_steady_f884`): in the decisive case file at
  17 degrees the added setting is undone (its key set to the default). T1 must fail; whether its fast test fails too is
  recorded.
- **M2**: in `results.json`, the cl_eff window maximum of the decisive run raised by 0.01. T2 must fail (the verdict no
  longer follows), or its N-x row.
- **M3**: in the test file, D5's cl_eff range 0.005 multiplied by 1000. NC1 must fail.
- **M4**: in the test file, the tolerance 0.018 of F17-x and C15-x multiplied by ten. NC2 and NC3 must fail.

## Size

Size (agent): done; 0; 1; 1; 230000

The one heavy run is the batch: six runs at 17 degrees, three at a time (about 80 minutes), and the controls at 15
degrees for the changes that meet the target (about 40 minutes each, side by side), with one release build of `rcfd-cli`
and the builds of the test file. Done is expected because the brief's rule counts a diagnosed cause as done and at least
one of the five changes is expected to end steady; a fix is less likely than a named cause.

## Decision rule

- **done (fixed)** if at least one change x meets F17-x and C15-x, and D0, P2, T1, T2, FT-U to FT-G, NC1 to NC3, M1 to M4,
  E, F and P hold. The statement: the steady solver reaches the attached state at 17 degrees with the opt-in setting x.
- **done (diagnosed)** if no change is a fix but at least one names its cause (N-x), and the same gate-side checks hold.
  The statement: the non-convergence goes away with the setting x alone (the state reached, attached or not, with its
  number); the attached state is or is not reached by it.
- **partial** if no run ends steady by D5 (the runs do not decide: each run's window ranges and mean are recorded), or a
  gate-side check or a mutation is not in place.
- **stopped and reported** if P2 fails, if a defect of the solver appears, or if the only way the runs point to is a
  change outside the files of this work (`schema.rs`, `sim.rs`, `pimple.rs`).

If D0 does not hold (main no longer reproduces ADR 0760's run), K0 is the base of the comparison and that is reported; it
is not a stop. No limit is changed after a run, no run is repeated to get another number, and no test is ignored,
skipped or retried to make a run pass. ADR 0760's known miss stays as it is in every case.

## What is not covered, stated before any result

- **A start from an attached state** (the 16-degree solution, or ADR 0764's time-accurate field). The steady route writes
  no restart file for a run with a turbulence model (`runctl.rs`, "this run cannot be restarted"), and the stored fields
  hold `nut` and not nuTilda; such a start needs a change of `sim.rs`, outside this work. KS tests the other start the
  route has.
- **One value of each setting, one setting at a time.** No combination, no search over values; a setting that would work
  at another value is not found here. The local pseudo mode is not run (arithmetic 1: it repeats KU's mechanism).
- **The discretisation is not changed** (convection schemes, the wall law, the far field): a change there would change the
  steady equations and so the state to be reached.
- **One mesh, one model, one angle** with its control; no Linux run. The target cannot tell 16 from 17 degrees
  (arithmetic 4); T1 holds the angle.
- **Why a named setting works** is shown only by its run and the comparison with K0; no eigenvalue analysis of the
  iteration is made.

## Question foreseen (rule 6: not taken, it changes a default result)

1. Whether a setting found here becomes a default of the steady route (or of the aerofoil cases). Written out with
   options and a recommendation in the addendum, under the fixed heading, once the result is known.

## Amendment 1 (2026-11-22): FT-U, before the commit of the test file and before any record

The first run of the five fast tests (constants unset, so they failed by construction) gave after 600 iterations on the
coarse mesh: FT-S 1.4197453512, FT-T 0.6849084672, FT-C -0.0781516830, FT-G 0.0571143866, and for **FT-U 2.3e55**: the
coarse run with `alpha_u = 0.7` diverges. A tolerance of 0.018 about such a number is not a record of the path (it
depends on the last bits of a divergent sequence). The set-up of that row is defective, so by rule 2 it is corrected
here, before the commit of the test file: **FT-U asserts that the coarse run at 17 degrees with `alpha_u = 0.7` has
diverged after 600 iterations, `abs(cl_eff) > 100` or not finite**, which is what the first run shows; the gate then
sees a change of that path that stops the divergence. The limit 100 is two orders above any lift of this aerofoil
(ADR 0760's largest swing is 1.95). The other four rows stand as registered, with the values above as `R_x`. The
divergence on the coarse mesh is reported in the addendum with the heavy run KU; it is not diagnosed further here.

## Amendment 2 (2026-11-22): a reported run KU-a15, before it is run

KU-a17 ended at iteration 3517 with exit code 0 (the residual targets met), steady by D5, at cl_eff 1.5768: it names its
cause and misses the target by +0.025. The registration runs a control at 15 degrees only for a change that meets the
target. To tell whether the 0.025 is a shift of the fixed point by `alpha_u` (arithmetic 1 and "the control at 15
degrees": the relaxation enters the face-flux interpolation through `a_P`) or another state at 17 degrees, the committed
case `KU-a15.toml` is run once as well, **reported without a limit**: its difference from 1.4932034 beside KU-a17's
difference from 1.5516. It decides no row: F17-U stays a miss whatever it gives, and C15-U is not a row of the decision
because KU does not meet F17-U. The test of the C15 rows (`f884_c15_controls_hold_for_every_change_that_meets_the_target`)
applies its limit to every control run in the record; it is corrected, in the commit after this amendment and before
the run, to apply it only to the changes that meet the target, so that this reported run is not judged by it.

## Amendment 3 (2026-11-22): NC1b, after mutation M3 was not caught and before its corrected run

Mutation M3 (D5's cl_eff range 0.005 multiplied by 1000 in the test file) was run once on the scratch copy and **no test
failed**: NC1 judges ADR 0760's window at 17 degrees, whose cd_eff range (0.68) alone already makes it not steady, so
NC1 does not control the cl_eff limit by itself. That is a defect of NC1's set-up. By rule 2 a corrected control is added
here, before its run; NC1 stays as it is. **NC1b**: D5 must call not steady a window with the cl_eff range of ADR
0760's run at 17 degrees (2.0202) and the cd_eff range of KT-a17 (2.2e-6), and a window with the cl_eff range of
KT-a17 (3.1e-6) and the cd_eff range of ADR 0760's run (0.6835); each limit of D5 is so controlled alone. Limit: both
not steady; source: D5, the other way round; kind: solution verification (negative control). M3 is then run once more
on a scratch copy with NC1b in place (M3b), and both outcomes are reported.

## Addendum 1: results (2026-11-22)

Commits: 46f01e24 (this registration, alone), 66739a37 (the driver and the ten case texts; before any run), 056c8471
(amendment 1), 330ba502 (the test file with the recorded fast-test values; before the record), c716f408 (amendment 2,
before KU-a15 ran) and e4205434 (the C15 test as amendment 2 says), 01c01418 (the record; the five misses as ignored
known-miss records; the driver's hash helper, see deviations), 1dd384a9 (the committed mutation), b5cb019d (amendment 3)
and 01515a8f (NC1b), and the commit of this addendum with the proposal file. Machine: the maintainer's Windows machine,
shared with three other agents. Binary as in the status lines. Runs, each made once, one thread each, three at a time: the
six at 17 degrees from 18:35 to 20:15 local time (1117 s to 3587 s each), the three at 15 degrees from 20:15 to 20:54.
Record: `docs/investigations/f884-naca0012-17-steady/` (README, `results.json`, `cases/`); status streams in the scratch
folder.

### The runs

| Run | Iterations (exit) | Window range cl_eff / cd_eff | cl_eff (D5 value) | Difference | N-x (steady) | F17-x or C15-x |
| --- | --- | --- | --- | --- | --- | --- |
| K0-a17 | 8000 (5) | 2.0202 / 0.6835 | 1.04762 (mean) | -0.5040 | no | no |
| KU-a17 | 3517 (0) | 4.3e-4 / 4.0e-5 | 1.57677 | +0.0252 | **yes** | **miss** |
| KS-a17 | 3730 (0) | 0.0232 / 0.0042 | 1.50932 | -0.0423 | **miss** | **miss** |
| KT-a17 | 8000 (5) | 3.1e-6 / 2.2e-6 | 1.55140 | -0.0002 | **yes** | **met** |
| KC-a17 | 8000 (5) | 2.0105 / 0.6680 | 1.02599 (mean) | -0.5256 | **miss** | **miss** |
| KG-a17 | 8000 (5) | 1.8e-4 / 8.4e-5 | 1.56603 | +0.0144 | **yes** | **met** |
| KT-a15 (C15-T) | 8000 (5) | 4.2e-6 / 2.2e-6 | 1.493201 | -0.000002 | yes | **met** |
| KG-a15 (C15-G) | 8000 (5) | 4.5e-4 / 1.5e-4 | 1.491724 | -0.0015 | yes | **met** |
| KU-a15 (reported, amendment 2) | 8000 (5) | 1e-10 / 1e-10 | 1.501414 | +0.0082 | yes | reported |

The difference is to 1.5516 at 17 degrees and to 1.4932034 at 15. Drag (D5 value of cd_eff, reported without a limit):
KT-a17 0.04352, KG-a17 0.04349, KU-a17 0.04101, against ADR 0764's time mean 0.04369; KT-a15 0.02676 and KG-a15 0.02713
against ADR 0760's 0.02676.

**What the runs show.**

- **The eddy-viscosity relaxation is a cause, and its removal is a fix (KT).** With `relax_nut = 1.0` and nothing else
  changed, the iteration goes through the same irregular start as K0 (cl_eff between about 0 and 1.8 to iteration
  2700), then falls into the attached state and stays there: 1.5469 at 3000, 1.5504 at 3500, 1.5514 from 4000 on, the
  residuals stalled at 1.4e-7 (so exit 5 at the cap, the window mean the value). cl_eff 1.55140 is 0.0002 from the
  time-accurate 1.5516, cd_eff within 0.0002 of its time mean, and at 15 degrees the same setting gives ADR 0760's
  steady value to six digits. The default `relax_nut = 0.5` lets the eddy viscosity of the boundary layer lag the
  momentum (arithmetic 3); with the lag the iteration does not leave the separated states in 8000 iterations, without it
  it does.
- **The local pseudo time step is a cause too, and a global step is a fix (KG).** With `pseudo_mode = "physical"`,
  `pseudo_dt = 0.02`, the iteration follows a time-like path from the first iteration (1.557 at 500, 1.579 at 1000,
  then a slow fall), as ADR 0764's time-accurate run does, and never visits the separated states. At the cap it is
  steady by D5 at 1.56603, 0.0144 above the target (within 0.018) and still falling by about 2e-5 per 100 iterations; at
  15 degrees it is 0.0015 low. The slow fall at the end is of the size arithmetic 2 gives for the starting vortex.
- **The velocity under-relaxation is a cause, but the state it reaches misses the target (KU).** With `alpha_u = 0.7`
  the iteration rises without the separated excursions and meets the residual targets at 3517 iterations, steady, at
  1.57677: 0.0252 above the target. The reported run at 15 degrees (amendment 2) is 0.0082 above ADR 0760's steady value
  there, so `alpha_u` moves the converged fixed point (arithmetic 1: the relaxation enters the face-flux interpolation
  through `a_P`), by more at 17 degrees than at 15; KU names its cause and is not a fix.
- **The start state and the corrector are not causes by this test.** KS (the potential-flow start) begins with lift
  values of order 1e5 and an eddy viscosity whose volume mean stays at 1.667e-2 (1e5 nu) from iteration 10 to the end;
  it met the residual targets at 3730 iterations with cl_eff still falling (window range 0.023, not steady) and cd_eff
  0.35, a separated state. That start is not a usable test of the start state on this case; why the eddy viscosity sits
  at 1e5 nu was not examined. KC (one non-orthogonal corrector with the compensation) behaves as K0: window range
  2.0105, mean 1.026.

So the sentence the brief asks for: **the steady solver reaches the attached state at 17 degrees by the registered
opt-in setting `turbulence.relax_nut = 1.0` (cl 1.5514 against 1.5516), and also by `pseudo_mode = "physical"` with
`pseudo_dt = 0.02` (cl 1.5660); the non-convergence of the default iteration is caused by the lag of the relaxed eddy
viscosity behind a momentum iteration with a local pseudo step, shown by these two runs, each of which removes it alone.**
One trajectory per setting: KT left the irregular phase after about 2700 iterations, as ADR 0760's run at 16 degrees left
it after about 850; whether it always does at 17 degrees, or at other angles, is not shown.

### The other checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| D0 | equal bits | K0-a17 equals ADR 0760's SA-a17 in iterations, exit code and the mean, minimum, maximum and final iterate of cl_eff and cd_eff | **yes** |
| P2 | equal | `59e707a8...3a70c889` | **yes** |
| T1 | all ten files | all ten | **yes** |
| T2 | all runs | all nine | **yes** |
| FT-S, FT-T, FT-C, FT-G | abs(cl_eff - R_x) <= 0.018 | equal to the ten printed digits on every later run | **yes** |
| FT-U (amendment 1) | diverged, abs(cl_eff) > 100 | 2.3e55 on every run | **yes** |
| NC1 | not steady | not steady | **yes** |
| NC1b (amendment 3) | both not steady | both | **yes** |
| NC2 | both > 0.018 | 0.0584 and 0.504 | **yes** |
| NC3 | both > 0.018 | 0.0697 and 0.0504 (16 degrees met the residual targets, so its final iterate 1.5436) | **yes** |
| B1 | <= 200 000 bytes | 155 601 bytes | **yes** |
| E | no file under `crates/*/src/` | `git diff --stat 21054b68`: this ADR, the driver, the record folder, the test file and its mutation patch, the proposal file | **yes** |
| F, P | see "The house checks" | | see below |

Test file, release: 19 passed, 5 ignored (the known misses N-S, N-C, F17-U, F17-S, F17-C, each with its number in its
comment; with `--ignored` all five fail), about 32 s, of which the five coarse runs take most.

### Mutations (scratch copy of the tree, own target folder, release, each run once)

| Id | Mutation | What failed |
| --- | --- | --- |
| M1 (committed patch) | `relax_nut = 1.0` to `0.5` in `KT-a17.toml` (the decisive case: KT is the first fix in the order U, S, T, C, G) | T1; and FT-T: cl_eff 0.3166, 0.368 from R_T |
| M2 | KT-a17's cl_eff window maximum raised by 0.01 in `results.json` | T2, N-T and F17-T |
| M3 | D5's cl_eff range 0.005 multiplied by 1000 in the test | **nothing** (NC1 is carried by the cd_eff limit alone; amendment 3) |
| M3b | the same, with NC1b in place | NC1b |
| M4 | the tolerance 0.018 multiplied by ten in the test | NC2, NC3 and T2 |

`git apply --check` accepts the committed patch on the tip.

### What the gate runs and what is a heavy record

- **In the gate** (`crates/rcfd-api/tests/naca0012_17_degrees_steady_f884.rs`, 19 tests): the judgement of all nine runs
  by D5 from the committed record, the reproduction D0, the set-up of the ten cases, four negative controls, and five
  solver runs of 600 iterations of the changed cases on the 6144-cell example mesh (one of them, KU, diverges there and
  is held as diverging).
- **Heavy record, not a cargo test:** the nine runs of up to 8000 iterations on 32 768 cells, made by the driver on
  demand (commands in the folder's README). The gate does not see a change of these runs directly; it sees a change of
  a coarse run's lift above 0.018, or any edit of the record or the cases. ADR 0760's known miss is unchanged.

### Deviations and disclosures

- Amendment 1: the coarse run with `alpha_u = 0.7` diverges, so FT-U asserts the divergence; why a lower relaxation
  diverges on the coarse mesh was not examined.
- Amendment 2: a reported run KU-a15, not registered at first; it decides no row.
- Amendment 3: M3 was not caught by the registered NC1; NC1b was added and M3 was run again (M3b) and caught. By the
  decision rule's words ("M1 to M4 ... hold") the registered M3 missed; it is reported here for the lead to weigh.
- `collect` failed at its first call on the record because the driver called a hash function that ADR 0167's driver
  does not have; a local `sha256_of` was added (01c01418) and `collect` was run again. Nothing of the judgement changed.
  A preliminary `collect` into the scratch folder was made before the controls ran, to read the verdicts at 17 degrees.
- The first fast-test run, with the constants unset, failed by construction; `RAYON_NUM_THREADS=2` was set for the test
  runs (the shared machine); the values were equal to ten digits on every later run.
- While the runs went, their status streams were read to see progress; that KT and KG would meet the target was seen
  before they finished; no limit or run depended on it.
- The second fast-test run used a name filter that also ran D0, which failed there because the record did not yet exist.
- ADR 0167 and the pressure part of `simple.rs` were not read whole (said in the registration).

### The house checks (F, P)

- `cargo test -p rcfd-api --release --test naca0012_17_degrees_steady_f884`: 19 passed, 5 ignored.
- `cargo test -p rcfd-api --release --lib`, `cargo fmt -p rcfd-api --check`, `cargo clippy -p rcfd-api --all-targets --
  -D warnings`, `python scripts/f884_naca0012_17_steady.py selftest`, `python scripts/w49d_aoa_sweep_fixed.py selftest`,
  `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and
  `python scripts/process_report.py --check` on the last tree: given in the report.

### Not run

Any combination of settings or another value of one; the local pseudo mode; a start from an attached state (needs
`sim.rs`); other angles than 15 and 17; Linux; `baseline_hashes` (no file under `crates/*/src/` changed); the whole
integration tests of `rcfd-api` and the workspace suite (rule 7).

### Size

Actual: done (registered: done); 0 further registrations and three amendments; 1 heavy batch (nine runs, about 2.3
hours of wall time, with one release build of `rcfd-cli`, the test-file builds and one build of a scratch copy for the
mutations); 1 question; about 240 000 bytes read.

### Decision by the registered rule

KT meets F17-T (1.55140 against 1.5516, tolerance 0.018) and C15-T (1.493201 against 1.4932034); KG meets F17-G and C15-G;
D0, P2, T1, T2, the five fast tests (FT-U as amended), NC1 to NC3 and NC1b hold; M1, M2 and M4 are caught, M3 only
after amendment 3 (M3b); E holds; no source of the solver was changed, so every default is bit-identical by
construction. By the clause "done (fixed)": **done**, with the M3 deviation stated, for the lead to decide.

### Open questions for the maintainer

1. **Does a setting found here become a default?** Options: (a) no default changes: the case files of this aerofoil near
   stall (and the topic page) name `turbulence.relax_nut = 1.0` as the setting that reaches the attached state, and a
   follow-up runs ADR 0760's twelve angles with it to see whether it is safe as a default; (b) make `relax_nut = 1.0` the
   default for Spalart-Allmaras on the steady route now, which changes every steady SA result and would need the
   baseline hashes re-recorded; (c) make the global pseudo step the default for aerofoil cases. **Recommendation (a)**:
   the evidence is one trajectory at each of two angles of one case; a default change alters every steady SA result,
   and KT's escape from the irregular phase took 2700 iterations, so its robustness is not shown.

### For the lead: roadmap entries

Status proposed: done.

> ADR 0770, 2026-11-22, done by its registered rule: on M1w-far500 with ADR 0760's set-up the steady SIMPLEC solve at 17 degrees with the one opt-in setting turbulence.relax_nut = 1.0 ends steady by ADR 0167's window rule at cl 1.5514, 0.0002 from the time-accurate 1.5516 (control at 15 degrees 1.493201 against 1.4932034), and with a global pseudo step (pseudo_mode physical, dt 0.02) at 1.5660; alpha_u 0.7 converges but 0.025 high, the potential start and a non-orthogonal corrector do not converge; the unchanged case reproduces ADR 0760 bit for bit; no solver source changed; the gate holds the record, four negative controls and five coarse runs (crates/rcfd-api/tests/naca0012_17_degrees_steady_f884.rs).

Follow-up proposed (a new item, with its own acceptance):

> Title: Steady Spalart-Allmaras without eddy-viscosity relaxation over the NACA 0012 lift curve. Deps: F-884. Acceptance: Every limit not stated here is registered in the ADR before the run. ADR 0760's twelve steady cases from 0 to 17 degrees with turbulence.relax_nut = 1.0 added each end steady by ADR 0167's window rule with cl within 0.018 of ADR 0760's steady values at 0 to 16 degrees and of ADR 0764's 1.5516 at 17 degrees, or each angle that does not is recorded with its number; the result informs whether relax_nut = 1.0 can become the default of steady Spalart-Allmaras.

The same entries are in `docs/roadmap/proposals/0770-F-884.json`. `docs/adr/index.md` is not current and is left to the
lead.

## The lead's reading (2026-11-22), at the merge

Merged at fe1e4d31 (the report read at 21:06 UTC, 5 minutes of reading, proposed done, written done). Checked by the
lead before the merge: the registration is the first commit and alone, and amendments 1 to 3 are each committed before
the change they allow; no file under `crates/*/src/` is changed, so the settings tried are existing options and the
defaults are untouched; the tests of ADR 0760 and ADR 0764 are unchanged; the five misses are ignored known-miss tests
with their limits intact. The push script ran the committed mutation and it failed as expected.

Status: **done**, as proposed: the acceptance allows either the target met by a registered change of the route or the
cause diagnosed, and both are given, with one run per setting. The lead notes the caveat the agent names: the mutation
M3 was caught only after amendment 3 added NC1b. The follow-up names itself F-886; that number is already taken on the
maintainer's checkout, so it is entered under the next free number if the maintainer takes it. The open question, a
default change, is put to the maintainer with the agent's recommendation (a), which the lead shares: a default change
would move every Spalart-Allmaras result and the hashes, and the twelve angles of ADR 0760 should be run with the setting
first.

## Answer of the maintainer (2026-11-22)

The lead put the open question above to the maintainer on 2026-11-22 with the agent's recommendation (a), which the
lead shares. The maintainer answered: "Yes, option (a)".

1. Option (a): no default is changed. The follow-up of the proposal file, the steady Spalart-Allmaras solve without
   eddy-viscosity relaxation over the twelve angles of ADR 0760, is entered as F-887 (the proposal file calls it F-886,
   a number already used on the maintainer's checkout) with the title, dependencies and acceptance written there.
   Whether the setting becomes a default is asked again when F-887 reports.
