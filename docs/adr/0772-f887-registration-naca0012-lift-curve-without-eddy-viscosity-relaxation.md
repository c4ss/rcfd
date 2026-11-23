# ADR 0772: F-887 registration: steady Spalart-Allmaras without eddy-viscosity relaxation over the NACA 0012 lift curve

Status: **REGISTERED 2026-11-22, written before any code of F-887 and before any run of its checks.**
Before this text no solver iteration of this work ran, no test was run and no file of the repository was changed. What
was done: reading (below), the arithmetic below, reading the committed records of ADR 0760 and ADR 0770 by a script, and
a release build of `rcfd-cli` from the unchanged tree (main d6e44b27; a build is not a run of a check). Results go in
addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f887-naca0012-lift-curve-relax-nut.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f887-naca0012-lift-curve-relax-nut` from main d6e44b27, scratch
`E:/projects/cfd/w49i-work/f887-naca0012-lift-curve-relax-nut-scratch`. Parents:
[ADR 0770](0770-f884-registration-naca0012-17-degrees-steady-iteration-diagnosis.md) (the setting `relax_nut = 1.0`,
which reached the attached state at 17 degrees, and the maintainer's answer that made this item),
[ADR 0760](0760-f868-registration-naca0012-lift-curve-against-ladson.md) (the twelve angles, the band and the steady
set-up), [ADR 0764](0764-f868-registration-naca0012-17-degrees-by-an-unsteady-arm.md) (the time-accurate value 1.5516 at
17 degrees) and [ADR 0167](0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md) (the window rule D5
and the curve rule D7).

## What is asked

The acceptance of F-887 (`docs/roadmap.json`, read by a script): "Every limit not stated here is registered in the ADR
before the run. ADR 0760's twelve steady cases from 0 to 17 degrees with turbulence.relax_nut = 1.0 added each end steady
by ADR 0167's window rule with cl within 0.018 of ADR 0760's steady values at 0 to 16 degrees and of ADR 0764's 1.5516 at
17 degrees, or each angle that does not is recorded with its number; the result informs whether relax_nut = 1.0 can
become the default of steady Spalart-Allmaras."

The brief: register the twelve angles, the band and the window rule exactly as ADR 0760 and ADR 0167 have them, with the
one change `relax_nut = 1.0`, and what the result means for a default change (a question for the maintainer); run the
twelve angles and report each against ADR 0760's result with the default setting beside it; hold it by a fast test in
the gate with the runs as a heavy record; change no default. The sentence this work must be able to write: with
relax_nut 1.0 the twelve angles end steady and within the band of ADR 0760, or these do not, with their numbers.

## What was read

- `AGENTS.md`; `docs/process/agent-rules.md` (version 5) whole; the brief whole; the entries F-887 and F-884 of
  `docs/roadmap.json`, by a script.
- ADR 0770 whole; ADR 0760 whole; ADR 0167 lines 136 to 155 (D5 to D9) and 290 to 364 (the results at 13 and 16
  degrees, the deviation on D7); **ADR 0167 not read whole**. ADR 0764: its result U17 only as ADR 0770 states it (1.5516,
  the time mean of cl_eff at 17 degrees); **ADR 0764 not read**.
- `scripts/f884_naca0012_17_steady.py` whole; `scripts/f868_naca0012_lift_curve.py` lines 1 to 178 (the steady part:
  `judge`, `band`, `collect`); in `scripts/w49d_aoa_sweep_fixed.py` the function `summary` (lines 263 to 285), the constants
  (lines 46 to 69) and the curve of `analyse` (lines 374 to 392); in `scripts/w48y_aoa_sweep.py` `read_stream`.
- `crates/rcfd-api/tests/naca0012_17_degrees_steady_f884.rs` whole; `docs/investigations/f884-naca0012-17-steady/README.md`
  lines 1 to 40; the committed `SA-a17.toml` of ADR 0760; the committed records
  `docs/investigations/f868-naca0012-lift-curve/results.json` and `docs/investigations/f884-naca0012-17-steady/results.json`,
  by a script (the values quoted below).
- `crates/rcfd-turb/src/sa.rs` lines 876 to 905 (the update of the eddy viscosity) and the lines a search for "relax"
  gave (the defaults, lines 494 and 495); [tolerances.md](../process/tolerances.md) whole; `scripts/process_report.py`
  lines 440 to 523 (the reading of a table of checks).
- `git diff --stat 13b0937d d6e44b27 -- 'crates/*/src'` and the same from 21054b68: **empty**. No solver source has
  changed since the builds of ADR 0760 (13b0937d) and ADR 0770 (21054b68).

## Arithmetic done beforehand

1. **The relaxation of the eddy viscosity does not move a fixed point.** The steady route updates the eddy viscosity of
   every cell as `nut_new = a target + (1 - a) nut_old`, with `target` the eddy viscosity of the new nuTilda and `a` =
   `relax_nut` (`sa.rs` lines 893 to 902; the time-accurate route uses `a = 1`). At a fixed point `nut_new = nut_old`, so
   `a (target - nut_old) = 0`, and for any `a > 0` the fixed point is `nut = target`: the same for 0.5 and for 1.0. The
   momentum and the nuTilda relaxations, which do enter the fixed point through `a_P` (ADR 0770, arithmetic 1), are not
   changed. So at an angle where both iterations converge to the same branch, the difference of cl is the difference of
   two convergence errors. ADR 0770's KT-a15 against ADR 0760's SA-a15: 1.4932012 against 1.4932034, -2.3e-6. **Expected
   difference: at most 1e-3** at an angle where the run ends steady on the attached branch.
2. **What can change is the path.** Which basin the iteration falls into (ADR 0770: at 17 degrees the default never left
   the separated states and `relax_nut = 1.0` did, after about 2700 irregular iterations), how many iterations it takes,
   and the stop at the residual targets. The changed `change` measure (line 901, `upd - nut_old`) is twice as large with
   `a = 1` for the same lag; whether it enters the residual targets was not read, so no iteration count is predicted.
3. **D5 on a run that stops early.** The window is the last min(1000, n/2) iterations; a run that meets the residual
   targets after n iterations has a window that can hold the end of its start-up. ADR 0760's runs at 13 and 16 degrees met
   the targets (777 and 2008 iterations) and are not steady by D5 (window range of cl_eff 0.00519 and 0.0202). The
   acceptance asks for "steady by ADR 0167's window rule", so such an angle is a miss of row A even when it converged;
   ADR 0760's band (row V) counts a run that met the targets as a flow value. Both are reported for every angle.
4. **The tolerance cannot tell 16 from 17 degrees.** The references at 16 and 17 degrees (1.5420 and 1.5516) differ by
   0.0096, under 0.018; every other pair of neighbouring angles differs by at least 0.0488 (15 to 16) and up to 0.22
   (NC1).
5. **What the setting gives at 17 degrees, if the run follows ADR 0770's KT-a17** (same case body, same solver sources):
   1.5514, so A17 is met (-0.0002); against Ladson's 1.6057 the difference is -0.0543, inside ADR 0760's band 0.0803 (V17
   met) and outside the scatter 0.018 (VS17 a miss); the maximum lift would be 1.5514 at 17 degrees, 1.5514 / 1.6116 - 1
   = -3.7 % (VM met).

## The registered set-up (task 1)

**Angles (12):** 0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16 and 17 degrees, ADR 0760's. Not run: 18, 20, 22, 24 and negative
angles, as ADR 0760.

**Cases.** For each angle, ADR 0760's committed case `docs/investigations/f868-naca0012-lift-curve/cases/SA-aNN.toml`
with the head comment and the title reworded and **one line added** after `model = "spalart_allmaras"`:

    relax_nut = 1.0   # ADR 0772 RN: the eddy viscosity not relaxed (ADR 0770 KT)

Every other line (mesh, rotation, viscosity, initial velocity, boundaries, model, reports, `max_iterations = 8000`,
`min_iterations = 8000`, output) is byte for byte that file's. The ids are `RN-a00` to `RN-a17`. At 15 and 17 degrees the
body equals ADR 0770's `KT-a15.toml` and `KT-a17.toml` (that file adds the same key with the same value and another
comment), which gives the reproduction checks D15 and D17.

**Mesh, binary and runs.** M1w-far500 by `python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>`, sha256
`59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889` (P2). Binary: `cargo build --release --locked -p
rcfd-cli` of main d6e44b27 (no crate source differs on this branch), its sha256 recorded. ADR 0167's command line (one
thread, the status stream with monitors at every iteration, the last snapshot stored), each run made once, at most three
solver processes at a time (three other agents share the machine). If the machine restarts, a run cut short is made again
from its start with the same binary and case, and that is reported. No limit on wall time is registered.

**The window rule and the value (ADR 0167, unchanged; its driver's `summary`).** The window is the last min(1000, n/2)
iterations; the D5 value is the final iterate when the run met the residual targets (exit 0) and the window mean when it
ran to its cap (exit 5); **steady** when the window range of `cl_eff` is at most 0.005 and that of `cd_eff` at most
0.0005. The curve value (D7) is the D5 value of a steady angle and the window mean of one not steady, flagged.

**The references.** Row A takes ADR 0760's steady values, the `cl` of its `results.json` (`evaluation.angles`, D7's
values: at 13 and 16 degrees the window means of runs that met the residual targets, as ADR 0760 recorded them), and at 17
degrees ADR 0764's 1.5516. Rows V and VS take Ladson's 80-grit lift `cl_L`, interpolated linearly in the committed CSV
`validation-data/reference/w31c_naca0012_ladson1988_80grit_cl_cd.csv`, and ADR 0760's band `B = max(0.05 abs(cl_L),
0.018)`. The values, from the committed records:

| alpha | ADR 0760 cl (default setting) | its run: iterations (exit), D5 steady | reference of A | cl_L | B |
| --- | --- | --- | --- | --- | --- |
| 0 | -0.0000014 | 254 (0), yes | -0.0000014 | -0.0072 | 0.0180 |
| 2 | 0.220164 | 299 (0), yes | 0.220164 | 0.2071 | 0.0180 |
| 4 | 0.438753 | 332 (0), yes | 0.438753 | 0.4272 | 0.0214 |
| 6 | 0.653988 | 380 (0), yes | 0.653988 | 0.6448 | 0.0322 |
| 8 | 0.863827 | 459 (0), yes | 0.863827 | 0.8557 | 0.0428 |
| 10 | 1.065594 | 533 (0), yes | 1.065594 | 1.0586 | 0.0529 |
| 12 | 1.255161 | 648 (0), yes | 1.255161 | 1.2493 | 0.0625 |
| 13 | 1.342311 | 777 (0), no (window mean) | 1.342311 | 1.3384 | 0.0669 |
| 14 | 1.423470 | 8000 (5), yes | 1.423470 | 1.4189 | 0.0709 |
| 15 | 1.493203 | 8000 (5), yes | 1.493203 | 1.4938 | 0.0747 |
| 16 | 1.542041 | 2008 (0), no (window mean) | 1.542041 | 1.5563 | 0.0778 |
| 17 | 1.047622 | 8000 (5), no (iteration mean) | 1.5516 (ADR 0764) | 1.6057 | 0.0803 |

The test and the driver read these from the committed files, not from this table.

## Design

### The heavy record

- **Driver** `scripts/f887_naca0012_lift_curve_relax_nut.py` (new; the drivers of ADR 0167, 0760 and 0770 are imported
  and not changed): `ids`, `cases <dir>` (the twelve texts by the replacement above), `run <rcfd> <work> <ids>
  [--parallel 3]` (ADR 0167's command line, as ADR 0770's driver runs it), `collect <work> <outdir> [--binary]
  [--commit]` (D5 by ADR 0167's `summary`, the curve by D7, the rows A, V and VS, VM, L2, L4, R1, D15, D17, P2, the
  history of `cl_eff` and `cd_eff` at every 50th iteration, `results.json` and the case files) and `selftest`. It is
  committed with the twelve case texts before any run.
- **Record** `docs/investigations/f887-naca0012-lift-curve-relax-nut/`: `README.md`, `results.json`, `cases/*.toml`. Full
  status streams stay in the scratch folder.

This heavy record is not a cargo test: twelve runs of up to 8000 iterations on 32 768 cells.

### What the gate holds

A new test file `crates/rcfd-api/tests/naca0012_lift_curve_relax_nut_f887.rs`, header "ADR 0772 (F-887)". The test files
of ADR 0760, 0764 and 0770 are not changed; ADR 0760's known miss at 17 degrees stays as it is.

1. **The judgement on the record**: the test reads the committed `results.json`, the committed record of ADR 0760 and the
   committed 80-grit CSV, applies D5 and D7 to each run's window statistics with its own code, and asserts the rows A, V,
   VS (one test per angle each), VM, L2, L4, R1, D15, D17, P2, T1 and T2. A row that misses becomes an ignored
   `*_known_miss` test with the limit intact and the number recorded (rule 2).
2. **The fast tests on the same path** (FT0n, FT4n, FT4nu): the committed cases `RN-a00.toml` and `RN-a04.toml` with three
   lines replaced (the mesh, by the committed coarse O-grid `example-projects/wind-tunnel/meshes/naca0012-ogrid`, 6144
   cells; `max_iterations` and `min_iterations`, by 600), run in process by `Simulation::run(600)`, as ADR 0760's FT0 and
   FT4 with the setting in force. FT4n's reference `R4n` is the value of the first run of that test on the build of this
   work, written into the test as a constant in the commit that adds the test and stated in the addendum. ADR 0760's
   R4 (0.3292877732, default setting) is printed beside it, without a limit.
3. **Negative controls** NC1 to NC3 (table).

## Table of checks

Margin is the limit divided by the expected value. For each alpha in the twelve angles there is one row A-alpha, one
V-alpha and one VS-alpha; they are written once here with alpha as the variable.

| Id | Check | Limit | Source of the limit | Expected | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A-alpha | acceptance of F-887, each alpha: run RN-aNN steady by D5, and abs(cl - reference of A) with cl the D5 value | steady; <= 0.018 | acceptance of F-887; ADR 0760, `results.json` (0 to 16); ADR 0764, U17 (17); ADR 0167, D5 | 1e-3 where steady on the attached branch (arithmetic 1); whether each run ends steady by D5 is not known (arithmetic 2 and 3) | 18 | solution verification |
| V-alpha | ADR 0760's band, each alpha: the value a flow value (exit 0 or steady by D5) and abs(cl - cl_L) with cl the D7 value | flow value; <= B(alpha) (0.018 to 0.0803) | ADR 0760, rows V00 to V17 (its band and its flow-value rule) | ADR 0760's differences at 0 to 16 (0.0006 to 0.0143); -0.0543 at 17 (arithmetic 5) | 1.4 to 125 | validation |
| VS-alpha | the scatter alone, each alpha: flow value and abs(cl - cl_L) | flow value; <= 0.018 | ADR 0760, S1 (the scatter of the three grit zones), and the lead's reading of ADR 0760 (question 2: later registrations take 0.018) | as V; at 17 degrees 0.0543: **a miss is expected** | 1.3 to 30; 0.33 at 17 | validation |
| VM | maximum lift: abs(cl_max / 1.6116 - 1), cl_max the largest flow value | <= 0.05 | ADR 0760, VM | 0.037 (arithmetic 5) | 1.35 | validation |
| L2 | lift slope over 0 to 8 degrees (least squares with intercept), as a factor of 2 pi | 0.90 to 1.05 | ADR 0760, L2 (ADR 0167, L2) | 0.9855 | n/a | solution verification |
| L4 | symmetry: abs(cl) at 0 degrees | <= 0.005 | ADR 0760, L4 (ADR 0167, L4) | 1.4e-6 | 3571 | code verification |
| R1 | the twelve runs are made as registered: each ends with a finish record and an exit code, on the binary built from this work's HEAD with no crate source changed | 12 of 12 | acceptance of F-887 (the twelve cases); ADR 0760, R1 | 12 | n/a | code verification |
| D15, D17 | RN-a15 and RN-a17 reproduce ADR 0770's KT-a15 and KT-a17: iterations, exit code, and the window mean, minimum, maximum and final iterate of cl_eff and cd_eff | equal bits | [tolerances.md](../process/tolerances.md), row "Bit-identity of default output" (exact), for the same case body on the same solver sources | equal | n/a | code verification |
| P2 | the mesh is ADR 0167's: sha256 of M1w-far500 | equal | ADR 0167, P2 | equal | n/a | unchanged default output |
| T1 | the set-up: each committed case equals the text recorded in `results.json`, names its angle, and differs from ADR 0760's committed case of that angle in the title, the head comment and the one added line only | all twelve | the brief, task 1 (the one change `relax_nut = 1.0`) | holds | n/a | code verification |
| T2 | the record is consistent: window min <= mean <= max for cl_eff and cd_eff, the history inside the window lies in [min, max], the window length is min(1000, n/2), and the recorded verdicts follow from the statistics by D5 and D7 | all twelve | [tolerances.md](../process/tolerances.md), row "Same discrete problem by two routes" (the driver's and the test's judgement) | holds | n/a | code verification |
| FT0n | fast test, coarse mesh, 0 degrees, 600 iterations: abs(cl_eff) | <= 0.005 | ADR 0167, L4 (ADR 0760, FT0) | below 1e-4 | above 50 | code verification |
| FT4n | fast test, coarse mesh, 4 degrees, 600 iterations: abs(cl_eff - R4n), R4n recorded once as said above | <= 0.018 | ADR 0760, FT4 (the same construction and tolerance) | 0 on the same build | above 18 | code verification |
| FT4nu | fast test, 4 degrees: cl_eff positive and not above 1.05 x 2 pi x 4 degrees = 0.4606 | 0 < cl_eff <= 0.4606 | ADR 0760, FT4u (ADR 0167, L2) | 0.33 (ADR 0760's R4) | n/a | solution verification |
| NC1 | negative control: the tolerance 0.018 of row A rejects the reference of each angle judged against that of the next angle (eleven pairs) | >= 10 of 11 rejected | the tolerance of A, the other way round ([tolerances.md](../process/tolerances.md), negative controls) | 10 (all but 16 and 17, 0.0096: arithmetic 4) | n/a | solution verification (negative control) |
| NC2 | negative control: D5 rejects, each limit alone, a window with the cl_eff range of ADR 0760's SA-a17 (2.0202) and the cd_eff range of its SA-a10 (1.6e-4), and one with the cl_eff range of SA-a10 (0.0030) and the cd_eff range of SA-a17 (0.6835) | both not steady | D5, the other way round | not steady, both | 404 and 1367 | solution verification (negative control) |
| NC3 | negative control: ADR 0760's band applied to this work's curve with cl multiplied by 1.1 (the reference area wrong by 10 %) rejects the angles 0 to 16 | >= 8 of 11 rejected | ADR 0760, NC2 (the same control) | 10 | n/a | validation (negative control) |
| B1 | storage: `docs/investigations/f887-naca0012-lift-curve-relax-nut/` | <= 200 000 bytes | ADR 0760, B1 | 120 000 | 1.7 | unchanged default output |
| E | `git diff --stat d6e44b27` names no file under `crates/*/src/`, no existing test, no golden and no hash table | none | rule 3 of `docs/process/agent-rules.md`; the brief (change no solver source and no default) | holds | n/a | unchanged default output |
| F | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; the driver's selftest | clean; clean; passes; passes | rule 7 of `docs/process/agent-rules.md` | clean | n/a | code verification |
| P | `python scripts/process_report.py --check` and `python scripts/roadmap_proposals.py check` on the last commit | no failure of this work | rules 7 and 9 of `docs/process/agent-rules.md` | none | n/a | code verification |

`baseline_hashes` is run only if a file under `crates/*/src/` is changed, which is not planned (E).

## Negative controls and mutations

NC1 to NC3 run in the gate. Mutations, each on a scratch copy of the tree at the last code commit, release, each run once;
what failed is recorded.

- **M1** (committed as `crates/rcfd-api/tests/naca0012_lift_curve_relax_nut_f887.mutation.patch`, first line
  `# expect-fail: cargo test -p rcfd-api --release --test naca0012_lift_curve_relax_nut_f887`): in `RN-a04.toml` the added
  line is undone (`relax_nut = 1.0` to `relax_nut = 0.5`, the default). T1 must fail; whether FT4n fails too is recorded.
- **M2**: in `results.json`, the cl_eff final iterate, window mean, minimum and maximum of RN-a10 each raised by 0.02. A-10
  must fail (or V-10's or T2's assertion first).
- **M3**: in the test file, D5's cl_eff range 0.005 multiplied by 1000. NC2 must fail.
- **M4**: in the test file, the tolerance 0.018 of row A multiplied by ten. NC1 must fail.

## Size

Size (agent): done; 0; 1; 1; 230000

The one heavy run is the batch of twelve runs, three at a time (default runs took 62 s to 41 minutes, ADR 0770's KT runs
37 to 59 minutes), with one release build of `rcfd-cli`, the builds of the test file and one build of a scratch copy for
the mutations. Done is expected because the brief's rule counts a recorded miss as reported.

## Decision rule

- **done** if all twelve angles are run as registered (R1, P2, T1) and each is reported against A, V and VS with its
  number (a miss is recorded as a miss and held as an ignored `*_known_miss` test with its limit intact), and T2, D15 and
  D17 (see below), FT0n, FT4n, FT4nu, NC1 to NC3, L4, M1 to M4, E, F and P hold.
- **partial** if the runs cannot be made as registered (an angle not run, a run without a finish record), or a gate-side
  check or a mutation is not in place.
- **stopped and reported** if P2 fails or a defect of the solver appears; no solver source is changed by this work.

If D15 or D17 does not hold, the solver sources being unchanged, that is reported with the numbers and is not a stop; the
runs of this work are then the record. No limit is changed after a run, no run is repeated to get another number, and no
test is ignored, skipped or retried to make a run pass. ADR 0760's and ADR 0770's tests stay as they are.

The sentence of the brief is written in the addendum from rows A and V: "with relax_nut 1.0 the twelve angles end steady
and within the band of ADR 0760", or the angles that do not, each with its number.

## What the result means for a default change (stated now; the question is the maintainer's)

Whether `relax_nut = 1.0` becomes the default of the steady Spalart-Allmaras route is not decided here (rule 6: it would
change every steady SA result and the baseline hashes). What this record can say, fixed before the run:

- **For** a default change: every A-alpha met (the fixed point unchanged at 0 to 16 degrees, arithmetic 1, and the
  attached state reached at 17), with no angle that the default reaches and the setting does not.
- **Against**, or for a narrower use (the setting named in the aerofoil cases near stall only): any angle at which the
  setting ends not steady, ends on another branch, or diverges where the default converged; or an iteration count much
  larger than the default's at the attached angles (reported, no limit).
- **Not shown in any case**: other meshes, other cases, the SST model (the key is Spalart-Allmaras's), Linux, and the
  robustness of a path that at 17 degrees passed through about 2700 irregular iterations (ADR 0770).

## What is not covered, stated before any result

- One setting at one value, one mesh, one model; no combination with ADR 0770's other settings.
- The angles 18 to 24 and negative angles; drag, moment and pressure are reported only as values, without a limit.
- Why a run takes the number of iterations it takes; no study of the residual measure of the eddy viscosity.
- The fast tests check the code's own output on the coarse mesh with the setting in force; the gate does not re-run the
  32 768-cell runs.
- Linux, and any machine but the maintainer's shared Windows machine.

## Question foreseen (rule 6: not taken, it changes a default result)

1. Whether `relax_nut = 1.0` becomes the default of the steady Spalart-Allmaras route. Written out with options and a
   recommendation in the addendum, under the fixed heading, once the result is known.

## Addendum 1: results (2026-11-23)

Commits: e393996e (this registration, alone), e314fb2f (the driver and the twelve case texts; before any run), 7f16397d
(the test file with the recorded fast-test value; before the record), 700757bc (the record; VS17 as an ignored known-miss
record), 3fcd9c9d (the committed mutation), and the commit of this addendum with the proposal file. Machine: the
maintainer's Windows machine, shared with three other agents. Binary: `rcfd-cli`, release, built from main d6e44b27 (no
crate source differs on this branch), sha256 `6be42efdecd953b8208bf438ac0a02d52a0ab7f964067196f0d6a472f20a15bb`. Mesh
M1w-far500 built by ADR 0167's driver. Twelve runs, each made once, one thread each, three at a time, from 23:22 on
2026-11-22 to 01:10 local time: 90 s to 3953 s a run. Record: `docs/investigations/f887-naca0012-lift-curve-relax-nut/`
(README, `results.json`, `cases/`, 123 563 bytes); status streams in the scratch folder.

### The twelve angles

The difference of A is to the reference of A (ADR 0760's steady value, ADR 0764's 1.5516 at 17); that of V and VS to
Ladson's 80-grit `cl_L`. ADR 0760's run with the default `relax_nut = 0.5` is beside it.

| alpha | iterations (exit) | window range cl_eff / cd_eff | cl (D5 = D7) | A: difference (limit 0.018) | V: cl - cl_L (band) | VS (0.018) | default: iterations (exit), cl |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 244 (0) | 2.3e-4 / 1.1e-7 | -0.0000015 | -0.0000001, **met** | +0.0072 (0.0180), **met** | met | 254 (0), -0.0000014 |
| 2 | 298 (0) | 7.3e-4 / 1.6e-5 | 0.220164 | -0.0000001, **met** | +0.0130 (0.0180), **met** | met | 299 (0), 0.220164 |
| 4 | 331 (0) | 1.0e-3 / 4.4e-5 | 0.438753 | -0.0000001, **met** | +0.0116 (0.0214), **met** | met | 332 (0), 0.438753 |
| 6 | 378 (0) | 1.2e-3 / 7.3e-5 | 0.653988 | +0.0000000, **met** | +0.0092 (0.0322), **met** | met | 380 (0), 0.653988 |
| 8 | 452 (0) | 1.3e-3 / 9.4e-5 | 0.863827 | -0.0000001, **met** | +0.0081 (0.0428), **met** | met | 459 (0), 0.863827 |
| 10 | 536 (0) | 2.5e-3 / 1.5e-4 | 1.065594 | +0.0000002, **met** | +0.0070 (0.0529), **met** | met | 533 (0), 1.065594 |
| 12 | 645 (0) | 4.1e-3 / 4.3e-4 | 1.255160 | -0.0000002, **met** | +0.0058 (0.0625), **met** | met | 648 (0), 1.255161 |
| 13 | 8000 (5) | 9.1e-7 / 3.5e-7 | 1.342886 | +0.000576, **met** | +0.0045 (0.0669), **met** | met | 777 (0), 1.342311 (not steady) |
| 14 | 8000 (5) | 3.0e-6 / 1.3e-6 | 1.423469 | -0.0000006, **met** | +0.0045 (0.0709), **met** | met | 8000 (5), 1.423470 |
| 15 | 8000 (5) | 4.2e-6 / 2.2e-6 | 1.493201 | -0.0000023, **met** | -0.0006 (0.0747), **met** | met | 8000 (5), 1.493203 |
| 16 | 8000 (5) | 4.5e-6 / 2.7e-6 | 1.543568 | +0.001528, **met** | -0.0127 (0.0778), **met** | met | 2008 (0), 1.542041 (not steady) |
| 17 | 8000 (5) | 3.1e-6 / 2.2e-6 | 1.551398 | -0.000202, **met** | -0.0543 (0.0803), **met** | **miss** | 8000 (5), 1.047622 (not steady) |

**The sentence of the brief: with relax_nut 1.0 the twelve angles end steady by ADR 0167's window rule and within the
band of ADR 0760; all twelve also meet the acceptance's 0.018 about ADR 0760's steady values and ADR 0764's 1.5516.** The
one miss is VS17: at 17 degrees the steady lift 1.5514 is 0.0543 under Ladson's 1.6057, outside the measured scatter
0.018 and inside ADR 0760's band (0.0803); it is the ignored record `f887_vs17_within_the_scatter_known_miss`, which
fails when run.

What the runs show beside the limits:

- **0 to 12 degrees: the same fixed point, the same iteration counts.** The lift equals the default's to six digits and
  the counts differ by at most 10 (254 against 244 at 0). Arithmetic 1 holds: the setting does not move the fixed point.
- **13 and 16 degrees: steady, but no longer converged by the residual targets.** The default met the targets at 777 and
  2008 iterations (and was then not steady by D5, its window holding the start-up); with the setting the residuals stall
  and both run to the cap, steady by D5 with window ranges of 1e-6. Their lift is that of the default's final iterate
  (1.3429 and 1.5436, ADR 0167's table), so the differences of A (+0.0006, +0.0015) are those of the default's window
  means, not a shift of the state. Why the residuals stall was not examined (not covered).
- **14 and 15 degrees** run to the cap as with the default and agree to 2e-6; at 15 and 17 degrees the runs equal ADR
  0770's KT-a15 and KT-a17 bit for bit (D15, D17).
- **17 degrees: the attached state** (cd_eff 0.04352 against the default's iteration mean 0.29972). The maximum lift is
  now 1.5514 at 17 degrees, 3.7 % under Ladson's 1.6116 (default: 1.5420 at 16, 4.3 % under).
- **Cost:** angles 13 and 16 took 41 and 42 minutes against 3.5 and 9 minutes with the default (ADR 0760's record); the
  others about the same.

### The other checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| VM | abs(cl_max / 1.6116 - 1) <= 0.05 | 1.5514 at 17 degrees, -3.74 % | **yes** |
| L2 | 0.90 to 1.05 | 0.98552 | **yes** |
| L4 | abs(cl(0)) <= 0.005 | -1.5e-6 | **yes** |
| R1 | 12 of 12 | 12, each with a finish record: exit 0 at 0 to 12, exit 5 at 13 to 17 | **yes** |
| D15, D17 | equal bits | iterations, exit code and the window mean, minimum, maximum and final iterate of cl_eff and cd_eff all equal to KT-a15 and KT-a17 | **yes** |
| P2 | equal | `59e707a8...3a70c889` (M1w also equal) | **yes** |
| T1 | all twelve | all twelve | **yes** |
| T2 | all twelve | all twelve | **yes** |
| FT0n | abs(cl_eff) <= 0.005 | 5.9e-9; the run met the residual targets after 202 steps | **yes** |
| FT4n | abs(cl_eff - R4n) <= 0.018 | R4n = 0.3293069785, recorded from the first run (600 steps, targets not met), equal to ten digits on every later run; ADR 0760's R4 with the default 0.3292877732, +1.9e-5 | **yes** |
| FT4nu | 0 < cl_eff <= 0.4606 | 0.3293 | **yes** |
| NC1 | >= 10 of 11 rejected | 10 of 11 (16 and 17 not, 0.0096) | **yes** |
| NC2 | both not steady | both | **yes** |
| NC3 | >= 8 of 11 rejected | 10 of 11 | **yes** |
| B1 | <= 200 000 bytes | 123 563 bytes | **yes** |
| E | none | `git diff --stat d6e44b27`: this ADR, the driver, the record folder, the test file and its mutation patch, the proposal file | **yes** |

Test file, release: 49 passed, 1 ignored (VS17), about 20 s; with `--ignored` the known miss fails with "cl 1.55140,
-0.05430 from Ladson's 1.6057 (limit 0.0180)".

### Mutations (scratch copy of the tree at 700757bc, its own target folder, release, each run once)

| Id | Mutation | What failed |
| --- | --- | --- |
| M1 (committed patch) | `relax_nut = 1.0` to `0.5` in `RN-a04.toml` | T1. FT4n did not fail: at 4 degrees on the coarse mesh the two settings give the same lift to 2e-5 (arithmetic 1), so FT4n cannot see the setting |
| M2 | RN-a10's cl_eff final, mean, minimum and maximum raised by 0.02 in `results.json` | A-10 (+0.0200 against 0.018), VS-10 and T2 |
| M3 | D5's cl_eff range 0.005 multiplied by 1000 in the test | NC2 |
| M4 | the tolerance 0.018 of row A multiplied by ten in the test | NC1 (6 of 11 rejected) |

All four are caught. `git apply --check` accepts the committed patch on the tip.

### What the gate runs and what is a heavy record

- **In the gate** (`crates/rcfd-api/tests/naca0012_lift_curve_relax_nut_f887.rs`, 49 tests): rows A, V and VS at the
  twelve angles and VM, L2, L4, R1, D15, D17, P2, T1, T2 on the committed record; three negative controls; and two solver
  runs of the committed cases at 0 and 4 degrees on the 6144-cell example mesh (FT0n, FT4n), with the setting in force.
- **Heavy record, not a cargo test:** the twelve runs on 32 768 cells, made by the driver on demand (commands in the
  folder's README). The gate sees a change of the coarse lift at 4 degrees above 0.018, or any edit of the record or of
  the cases; as M1 shows, it does not see the setting itself in the fast runs, only in T1.

### Deviations and disclosures

- The binary was built before the commit of the registration, from the same tree (said in its status lines).
- The first fast-test run (constant unset, failing by construction) used the name filter `ft`, which also ran L2 and VM;
  they failed there because the record did not exist yet.
- `cargo fmt -p rcfd-api --check` could not run: on this machine it stops with "The filename or extension is too long
  (os error 206)", from the crate folder too. `rustfmt --check --edition 2021` on the new test file, with the workspace's
  `rustfmt.toml`, is clean; no other Rust file was changed.
- The status streams were read while the runs went, to see progress; no limit or run depended on it. A preliminary
  `collect` into the scratch folder was made when ten runs had finished.
- `RAYON_NUM_THREADS=2` was set for the test runs (the shared machine).
- ADR 0167 and ADR 0764 were not read whole (said in the registration).

### The house checks (F, P)

- `cargo test -p rcfd-api --release --test naca0012_lift_curve_relax_nut_f887`: 49 passed, 1 ignored.
- `cargo test -p rcfd-api --release --lib`: 159 passed, 4 ignored.
- `cargo clippy -p rcfd-api --all-targets -- -D warnings`: clean. fmt: see deviations.
- `python scripts/f887_naca0012_lift_curve_relax_nut.py selftest` and `python scripts/w49d_aoa_sweep_fixed.py selftest`:
  pass. `cargo test -p rcfd-cli --release --test docs_site` and `python scripts/process_report.py --check` on the last
  tree: given in the report.
- `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0772-F-887.json`: accepted (F-887: todo to done).

### Not run

The angles 18 to 24 and negative angles; any other value of `relax_nut` or a combination with other settings; another
mesh or model; Linux; `baseline_hashes` (no file under `crates/*/src/` changed); the whole integration tests of `rcfd-api`
and the workspace suite (rule 7).

### Size

Actual: done (registered: done); 0 further registrations, no amendment; 1 heavy batch (twelve runs, 1 h 48 min of wall
time, with one release build of `rcfd-cli`, the test-file builds and one build of a scratch copy for the mutations); 1
question; about 200 000 bytes read.

### Decision by the registered rule

All twelve angles are run as registered (R1, P2, T1) and reported against A, V and VS with their numbers: A met at all
twelve, V met at all twelve, VS missed at 17 degrees (recorded as a known miss, limit intact). T2, D15, D17, FT0n, FT4n,
FT4nu, NC1 to NC3, L4, M1 to M4 and E hold; F as stated (fmt by rustfmt on the one file). By the clause "done": **done**.

### Open questions for the maintainer

1. **Does `relax_nut = 1.0` become the default of the steady Spalart-Allmaras route?** By the registered reading: every
   A-alpha is met and no angle that the default reaches is lost, which is the case "for" a default change; against it
   stand the two angles (13 and 16) that now run to the cap without meeting the residual targets, ten times the wall
   time there, and that the evidence is one mesh and one case. Options: (a) make it the default now, which changes every
   steady SA result and needs the baseline hashes and any SA goldens re-recorded by the maintainer; (b) keep the default
   and name the setting in the aerofoil example cases and the topic page, with a follow-up that runs it on a second SA
   case set (for example ADR 0159's mesh M2w or another steady SA validation case of the repository) and diagnoses the
   stalled residuals at 13 and 16 degrees, before (a) is asked again; (c) keep the default and do nothing more.
   **Recommendation (b)**: the fixed point is unchanged (arithmetic 1, shown at ten angles to six digits), so a default
   change would cost only robustness and run time, but the stalled residuals are a change of behaviour not yet
   understood, and one case is too narrow for a default of every steady SA run.

### For the lead: roadmap entries

Status proposed: done.

> ADR 0772, 2026-11-23, done by its registered rule: ADR 0760's twelve steady SA cases on M1w-far500 with turbulence.relax_nut = 1.0 added all end steady by ADR 0167's window rule, within 0.018 of ADR 0760's steady values at 0 to 16 degrees (largest 0.0015 at 16) and of ADR 0764's 1.5516 at 17 (1.5514), and within ADR 0760's band of Ladson at all twelve angles (17 degrees -0.054, outside the scatter 0.018, a known miss); 13 and 16 degrees no longer meet the residual targets and run to the cap; no default changed; the gate holds the record, three negative controls and two coarse runs (crates/rcfd-api/tests/naca0012_lift_curve_relax_nut_f887.rs).

No follow-up item is proposed here: the follow-up of question 1 (b) depends on the maintainer's answer. The same entry is
in `docs/roadmap/proposals/0772-F-887.json`. `docs/adr/index.md` is not current and is left to the lead.
