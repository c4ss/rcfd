# ADR 0767: F-885 registration: the velocity change of each outer iteration, and check H read by its own rule

Status: **REGISTERED 2026-11-22, written before any code of F-885 and before any run of its checks.** Before this text no
library code, test, harness or solver run of F-885 was written or made. What was done: reading (below), one reading
script over `docs/roadmap.json`, `git diff` of the source folders since the base of ADR 0766, and the arithmetic of this
page. Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f885-check-h-outer-record.md`](briefs/f885-check-h-outer-record.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. Branch `f885-check-h-outer-record` from main
21054b68; scratch `w49i-work/f885-check-h-outer-record-scratch`.

Size (agent): done; 0; 2; 1; 300000

## What is asked

Roadmap item F-885. Its acceptance: "Every limit not stated here is registered in the ADR before the run. The pimple step
keeps, opt-in and with defaults unchanged bit for bit (baseline hashes unchanged), the largest velocity change of each
outer iteration of a step; a closed-form or two-route check registered in the ADR shows the recorded values are the
changes of the iterations. Check H of ADR 0183 is then run as registered there and in ADR 0766, with the outer-iteration
rule of ADR 0183 as written (the velocity change of the 5th outer iteration at most 1e-2 of the 1st in at least 99 % of
the window's steps), and classes the hub pocket as steady, physical or numerical, or is recorded as not deciding."

The sentence this work must be able to write (the brief): the hub pocket is steady, physically unsteady or numerical by
ADR 0183's rule as written, or the run does not decide.

## What was read

Whole: the brief; `AGENTS.md`; `docs/process/agent-rules.md` (version 5);
[ADR 0766](0766-f879-registration-fan01-check-h-the-hub-pocket-in-time.md) (registration, amendment 1, addendum 1, the
lead's reading, the answers of the maintainer of 2026-11-22); `crates/rcfd-api/tests/f879_fan01_check_h.rs`;
`docs/process/tolerances.md`, the table.

In part: [ADR 0183](0183-exp-rot-registration.md) lines 150 to 350 (D5, D8, D9, the registered limits, the section
"Check H (Q-POCKET; report only)", the predictions, the budgets and stop rules); `crates/rcfd-fv/src/pimple.rs` lines
150 to 260 (the settings of the outer criterion and `OuterRecord`), 1005 to 1095 (`PimpleSettings::piso` and `pimple`),
1180 to 1205 (`OuterUpdate`, `OuterHook`), 1265 to 1405 (the solver fields and its two constructors), 1640 to 1810 and
2470 to 2603 (the step: the outer loop, its criterion, the end of the step and its records), 3515 to 3530
(`reinitialise`); the list of functions of `crates/rcfd-fv/src/pimple/moving.rs`; `crates/rcfd-api/src/sim.rs` lines
2100 to 2125 (the transient settings from the case) and the accessors `pimple_solver` and `pimple_solver_mut`;
`crates/rcfd-api/src/schema.rs` lines 870 to 880 (`solver.alpha_u`); the head of
`crates/rcfd-fv/tests/consistent_value_f322.rs` (a box solver built on `PimpleSolver::new`); the head of
`docs/investigations/f879-fan01-check-h/H_dt.csv`; `docs/roadmap/proposals/0766-F-879.json`. Of `docs/roadmap.json`, by
script: F-885 in full.

`git diff --stat 2bab1863 21054b68` over `crates/rcfd-fv/src`, `crates/rcfd-api/src`, `crates/rcfd-mesh/src` and
`crates/rcfd-core/src` is empty: the solver, the case tree and the mesh generator on this branch's base are those that
made the two records of ADR 0766.

No published source outside the repository is cited by this registration. The record is defined from first principles
(a difference of two iterates and its largest Euclidean norm over the cells); every limit of check H is that of ADR 0183
and ADR 0766.

## 1. The record (task 1; fixed now)

**What the step does now.** `PimpleSolver::step_with` runs `n_outer` outer iterations; each runs the hook (if any), the
momentum predictor (relaxed by `alpha_u` except in the last iteration) and the pressure correctors, which leave the
iterate in `self.u`. With `n_outer = 5` the 5th is the final, unrelaxed one. Nothing of the velocity change of an
iteration is kept; `OuterRecord` (ADR 0458) keeps one pressure ratio per step.

**The record.** A new setting `PimpleSettings::outer_velocity_record: bool`, default `false`. With it the step keeps, for
every outer iteration `k = 1 .. used` of the step,

    d_k = max over the cells c of |u_k(c) - u_(k-1)(c)|      (Euclidean norm of the cell velocity difference)

where `u_k` is `self.u` at the end of outer iteration `k` (after its last corrector) and `u_0` is `self.u` at the start of
the step's first outer iteration. Each iterate is copied at the start of its outer iteration, at the place where the
pressure copy of ADR 0458 is taken (before the hook), and `d_k` is formed after the correctors, before the loop ends or
goes on. A step that completes pushes `OuterVelocity { step, change: Vec<f64> }` (one entry per outer iteration run) to
the new field `PimpleSolver::outer_velocity`; a step that trips the guard pushes nothing, as for `outer_record`. The new
setting is read nowhere else: the copies and the norms read the iterate and do not enter any equation. Without the
setting no copy is made and nothing is pushed, so every default run takes the same operations as before.

The fractional step (`step_fractional`) and the moving-mesh steps of `pimple/moving.rs` are not given the record: check
H runs neither, and the brief asks for the least change. The setting is not read there (documented on the field).

**Switching it on.** No case key: the setting is set on the built solver through `Simulation::pimple_solver_mut`, as ADR
0766 set `outer_pressure_tolerance`. So `crates/rcfd-api/src/sim.rs` and `schema.rs` are not changed, and no golden
(schema, CLI help, tools list) or case hash table is touched.

**The check that the values are the changes of the iterations (two routes).** On a small case, an 8 x 8 lid-driven cavity
built on `PimpleSolver::new` (`box_2d(8, 8, 1, 1)`, walls `fixed` zero, the lid `ymax` at `(1, 0, 0)`, `nu = 0.01`,
`PimpleSettings::pimple(0.01, 5)`, `dt = 0.05`, BDF2, from rest), three steps are run with `step_with` and a hook that
returns `OuterUpdate::default()` (no source: the case has none, so the hook changes nothing) and copies the velocity it
is handed at the start of every outer iteration. Route A is the record. Route B is formed in the test from the hook's
copies `v_1 .. v_5` and the velocity after the step `v_6`: `d_k = max_c |v_(k+1) - v_k|`. The hook is called with
`&self.u` at the start of each outer iteration, so route B agrees with route A if and only if the record's iterates are
the ones the outer iterations hand on. Also run:

* the closed form: from rest with the lid at rest (`ymax` fixed zero), no source: every `d_k` is exactly zero;
* `n_outer = 1`: the one value equals `max_c |u^(n+1) - u^n|` from the velocities read before and after the step, without
  a hook;
* the negative control: the cumulative change `max_c |v_(k+1) - v_1|` differs from the record for some `k >= 2` of every
  step by more than 1e-6 relative (so a record of the change from the step start, the planned mutation M1, cannot pass
  route B);
* the defaults: the same three steps with the setting off give an empty `outer_velocity` and the same bits of `u`, `p`
  and the flux as with it on.

## 2. Check H again (task 2; fixed now)

**As registered in ADR 0183 and ADR 0766, unchanged:** B2 (13 627 cells, C-grid `camber_seam_h`), the case text of ADR
0766 (`h_text`: SST, `rotating_frame` with the centrifugal part, Barth-Jespersen-limited `linear_upwind`,
`algorithm = "pimple"`, `n_outer = 5`, `time_scheme = "bdf2"`, the walls and inlet of ADR 0086, the seam diffusivity
`pair` of D-1, the six reports of D-4); the uniform start (D-7, and the acceptance of F-879 as reworded by the maintainer
on 2026-11-22); H(dt) with 4.605e-5 s, 5 498 steps, and H(dt/2) with 2.3025e-5 s, 10 996 steps; the window, the steps
whose time is above 0.1266 s (2 749 and 5 498 steps); the six site cells c7, c57, c107, c157, c621, c1235; the signal,
the amplitude, the frequency by upward zero crossings with linear interpolation, the DFT peak (D-5); the STEADY threshold
0.193 m/s; the classes of ADR 0183 in their order with every number in them; the verdict of F-879 with persistence 0.75
(the table "The three classes as numbers" of ADR 0766); no wall-clock cap (D-3), a divergence, a guard trip or a
non-finite value is a stop and gives NOT EVALUABLE; the two runs made once, started together (ADR 0183 D8). Stop rule 4
of ADR 0183 holds: after the two runs nothing more is run in this part.

**The departures from ADR 0766, each with its reason (fixed now, before the run):**

**E-1. The outer-iteration rule is ADR 0183's, as written, on the new record.** It replaces D-2 of ADR 0766:

> a step meets the rule iff `d_5 <= 1e-2 d_1` (the velocity change of the 5th outer iteration at most 1e-2 of that of the
> 1st, from the record of section 1); the run is TIME-ACCURATE iff at least 99 % of the window's steps meet it.

No floor and no other quantity: the maintainer chose ADR 0183's own rule over a reading of the existing records (ADR
0766, answer 1). A step with `d_1 = d_5 = 0` meets it (`0 <= 0`); a NaN does not. Reason: the brief and the acceptance.
The rule of D-2 is computed from the same run (E-2) and reported beside it, with no limit and no part in the class.

**E-2. Both records on.** The runs set `outer_pressure_tolerance = Some(-1.0)` (the pressure record of D-2, never met, so
every step runs its 5 outer iterations) and `outer_velocity_record = true`. Reason: then the run is the computation of
ADR 0766 with one more read-only record, and its site signal can be checked against the committed records of ADR 0766
(R0). That both records together leave the solution unchanged bit for bit is check C2.

**E-3. The records.** Per run one file `docs/investigations/f885-check-h-outer-record/H_dt_outer.csv` and
`H_dt2_outer.csv`: one row per step with the step, `d_1` to `d_5` and the step change `max_c |u^(n+1) - u^n|` of the
cell velocity (read in the test between steps; reported only), printed to 10 significant digits. The full rows of ADR
0766's form (the site speeds, torque, pressure rise, the pressure ratio, residual, continuity) are written to the scratch
folder. If R0 passes, the class is read from the committed ADR 0766 records for the site signal and from the new files
for the outer rule, so the evidence is not committed twice; if R0 fails, the full rows of the new runs are committed as
`H_dt.csv` and `H_dt2.csv` in the new folder and the class is read from them alone. Reason: the brief prefers the least
committed volume that is the evidence.

**E-4. A new test file.** `crates/rcfd-api/tests/f885_check_h_outer_record.rs` (header: ADR 0767, F-885). The brief
forbids changing `f879_fan01_check_h.rs`; the case text, the measures, the class and the verdict are copied from it
unchanged, the outer rule is replaced by E-1, and the copy is held by the ADR 0766 pairs of known signals run again in
the new file (C4).

**What is not departed from:** everything in the paragraph "As registered" above.

**Estimate, before the run (not a limit).** ADR 0766 recorded the pressure ratio `c_4 = |p_4 - p_3| / |p_1 - p^n|` over
the window with a median of 0.0109 in H(dt) and 0.0060 in H(dt/2). If the velocity change of an outer iteration
contracts at the same rate `r` as the pressure change (`c_4 = r^3`), then `r = 0.0109^(1/3) = 0.222` and `0.0060^(1/3) =
0.182`, and `d_5 / d_1 = r^4 = 2.4e-3` and `1.1e-3`: margins 4.1 and 9 against 1e-2. Two reasons the estimate may fail,
said now: the 5th iteration is unrelaxed (`alpha_u = 0.9` of the case default in the 1st to 4th, 1 in the 5th), so its
change is not the geometric continuation of the relaxed ones; and if the state becomes steady, `d_1` may fall towards
the level that the linear solvers leave while `d_5` stays at that level, which the rule as written does not exempt. ADR
0766 found that the pressure's first change did not vanish in the steady window (160 and 301 m^2/s^2); whether the
velocity's does is not known before the run.

## 3. Class and the convergence rule of F-870 (task 3)

The class by ADR 0183's rule and the verdict of F-879 by ADR 0766's table, with the outer rule of E-1. The addendum
states what the class means for the convergence rule of F-870, as a proposal to the maintainer with ADR 0766's caution
(the time-marched pressure rise on B2 is 8 to 9 per cent above the SIMPLEC sample), under the fixed heading of open
questions. Not decided here.

## 4. The tests (task 4; fixed now)

All in `crates/rcfd-api/tests/f885_check_h_outer_record.rs`.

* Run by the gate (not ignored):
  * `f885_record_two_routes`: section 1 on the cavity (A2, T2 to T5).
  * `f885_rule_and_classes_on_known_signals`: the measures on a sampled sine and the ADR 0766 pairs (C3, C4), and built
    pairs for the rule of E-1 (C5).
  * `f885_short_run_holds_the_state`: 20 steps of H(dt) on the path of the heavy records, with both records and with
    neither (C2, S1, S2).
  * `f885_class_from_the_committed_records`: the class and the verdict from the committed records (G1).
* Heavy records (`#[ignore]`, run once by me, one process each, started together): `f885_h_dt_record` and
  `f885_h_dt2_record`, written to the folder named by `F885_OUT`; each compares its rows with the committed ADR 0766
  record of the same run (R0).

The values of G1 are those of the two registered runs and are written into the file after them; the form and the
tolerance are fixed here.

## Table of checks

Every clause of the acceptance first.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | the record is opt-in and the defaults are bit-identical: `cargo test -p rcfd-api --release --test baseline_hashes` passes with `baseline_hashes.rs` unchanged | passes, file unchanged | acceptance of F-885 ("baseline hashes unchanged"); rule 3 | passes | n/a | unchanged default output |
| A2 | route A (the record) against route B (the hook's iterates) on the cavity, every `d_k` of three steps | relative difference `<= 1e-12` (expected equal bits) | acceptance of F-885 ("two-route check"); `docs/process/tolerances.md`, "Same discrete problem by two routes" (`<= 1e-12` where only rounding separates) | 0 | n/a | code verification |
| A3 | check H run as registered there and in ADR 0766 with E-1 to E-4: H(dt) completes 5 498 steps and H(dt/2) 10 996, no failure, no guard trip, every field finite | both complete | acceptance of F-885; ADR 0183 D5; ADR 0766 A1 | completes (ADR 0766 A1, the same computation) | n/a | solution verification |
| A4 | each run is TIME-ACCURATE by E-1 | at least 99 % of the window's steps with `d_5 <= 1e-2 d_1` | acceptance of F-885; ADR 0183, check H, outer-iteration rule | `d_5 / d_1` about 2.4e-3 (dt) and 1.1e-3 (dt/2), by the estimate of section 2 | 4.1; 9 | solution verification |
| A5 | the class by ADR 0183's rule and the verdict of F-879 by ADR 0766's table | one of the four, by the registered numbers (0.193 m/s; `n >= 6`; 20 steps per period; [0.9, 1.1]; [0.75, 1.33]; 10 %; 4 steps; persistence 0.75) | acceptance of F-885; ADR 0183, check H, class; ADR 0766, the table of verdicts | STEADY if A4 holds (ADR 0766 measured `A` 5.4e-3 and 6.5e-3 m/s) | n/a | solution verification |
| A6 | a run that does not decide is recorded as not deciding: `Verdict::DoesNotDecide` is an outcome of the pure function and held by G1 if it is the result | as stated | acceptance of F-885 (last clause) | as stated | n/a | solution verification |
| T2 | the closed form on the cavity with the lid at rest: every `d_k` of three steps | exactly 0 | `docs/process/tolerances.md`, "Agreement with a closed form" (exact for the discrete problem); a state at rest with no source is a fixed point of every outer iteration | 0 | n/a | code verification |
| T3 | `n_outer = 1` on the cavity: the one value against `max_c |u^(n+1) - u^n|` from outside | relative difference `<= 1e-12` | "Same discrete problem by two routes" | 0 | n/a | code verification |
| T4 | negative control: the cumulative change from the step start against the record, `k >= 2` | differs by more than 1e-6 relative in at least one `k` of each step | this registration (a check that route B can tell the two apart) | differs at order 1 | n/a | code verification |
| T5 | the setting off on the cavity: `outer_velocity` empty; `u`, `p` and the flux after three steps equal to the run with it on | equal bits | `docs/process/tolerances.md`, "Bit-identity of default output" | equal | n/a | unchanged default output |
| C2 | 20 steps of H(dt) with both records (E-2) and with neither: the six site speeds, torque and pressure rise at every step | equal bits | "Bit-identity of default output" | equal | n/a | unchanged default output |
| S1 | after those 20 steps the largest site speed, torque and pressure rise against ADR 0766's first values (13.824327 m/s, 2.607352 N m, 110.1842 Pa, printed to 8 and 7 digits) | relative `<= 1e-6` | "Same discrete problem by two routes" (`<= 1e-8`), widened to the rounding of the printed digits as ADR 0766 G1 did; the solver source is unchanged since that run | 0 | n/a | unchanged default output |
| S2 | the velocity record of those 20 steps: 20 entries, each of 5 finite values with `d_1 > 0` | as stated | this registration (the record exists on the path of the heavy runs) | as stated | n/a | code verification |
| C3 | measures on a sampled sine (200 samples a period, 10.5 periods) | relative error `<= 1e-3` for `A` and `f` | "Agreement with a closed form" (exact only for the continuum) | 1.2e-4 | 8 | code verification |
| C4 | the ten pairs and the cut series of ADR 0766, on the copied functions | each verdict as ADR 0766 built it | ADR 0766, C4 | as built | n/a | code verification |
| C5 | built pairs for E-1: `d_5 / d_1 = 1e-3` everywhere (meets); `0.05` everywhere (NOT TIME-ACCURATE); `0.05` in 2 % of the window (fails 99 %); in 0.5 % (meets); `d_4 / d_1 = 0.5` with `d_5 / d_1 = 1e-3` (meets: the 5th is read, not the 4th); `d_1 = d_5 = 0` (meets); a NaN `d_5` in 2 % (fails) | each as built | this registration, E-1 | as built | n/a | code verification |
| R0 | each heavy run against the committed ADR 0766 record of the same run, every step: the six site speeds, the smallest axial velocity, torque, pressure rise, the pressure ratio and the first pressure change | relative `<= 1e-8` | "Same discrete problem by two routes" (`<= 1e-8`; the record is printed to 10 digits, a rounding of 5e-10) | 0 (the same source, the same set-up; E-2) | n/a | unchanged default output |
| G1 | `f885_class_from_the_committed_records`: step counts 5 498 and 10 996, window 2 749 and 5 498; the share of the window's steps meeting E-1, the class and the verdict as recorded; `A` and `f` of each run | counts exact; class and verdict equal; 1e-6 relative on the shares, `A` and `f` | "Same discrete problem by two routes", widened to the printed digits as ADR 0766 G1 | 0 | n/a | unchanged default output |
| D1 | the source change: only `crates/rcfd-fv/src/pimple.rs` among the files under `crates/*/src`; `simple.rs`, `f879_fan01_check_h.rs`, `f870_fan01_design_point.rs` and every golden unchanged | as stated | the brief; rule 3 | as stated | n/a | unchanged default output |
| P | `cargo test -p rcfd-fv --release --lib`; `cargo test -p rcfd-api --release --test f879_fan01_check_h` (the existing tests of the outer record's path); `cargo fmt -p rcfd-fv -p rcfd-api --check`; `cargo clippy -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

No row is of kind validation: F-885 claims no agreement with a measured case.

**Reported beside, without a limit:** per run, over the window, the smallest, median and largest of `d_5 / d_1`, of
`d_1`, of `d_5`, and of the contraction `d_(k+1) / d_k` for `k = 1 .. 4`; the share by the rule of D-2 of ADR 0766 on the
same run; the step change against `d_1`; everything ADR 0766 reported beside its class (torque and pressure rise with
their amplitude, frequency and means against the steady and measured values, the smallest axial relative velocity, the
five cells of largest amplitude, the seam-pair flux sum, the mass balance, wall seconds per step).

## Negative controls and planned mutations

Run on a scratch copy of the tree (`git archive` of the branch into the scratch folder, its own target folder), each
against the gate tests of the new file; the heavy records are not rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: the record takes the iterate at the step start only, so `d_k` is the cumulative change `max |u_k - u_0|` | `f885_record_two_routes` (A2) |
| M2 | solver: the record takes the x component only, `max |du_x|` | `f885_record_two_routes` (A2) |
| M3 | solver: with the record on, one cell's velocity is changed in its last bit at the copy (the record changes the solution) | `f885_record_two_routes` (T5), `f885_short_run_holds_the_state` (C2) |
| M4 | test: the rule's ratio from 1e-2 to 1e-1 | `f885_rule_and_classes_on_known_signals` (C5) |
| M5 | test: the rule reads `d_4` in place of `d_5` | `f885_rule_and_classes_on_known_signals` (C5) |
| M6 | record: `H_dt_outer.csv` cut to its first half | `f885_class_from_the_committed_records` |
| K1 | negative control: T4 in the test itself (the cumulative change is told apart) | as T4 |

The committed patch (`crates/rcfd-api/tests/f885_check_h_outer_record.mutation.patch`) is M1, with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f885_check_h_outer_record f885_record_two_routes`.

## Decision rule

* **Done** only if the record is shown correct (A2, T2 to T5 met), the defaults are bit-identical (A1, C2 met) and check H
  classes the pocket by ADR 0183's rule (A3 and A4 met in both runs, and the verdict is steady, physically unsteady or
  numerical).
* **Partial** if the record is in and shown correct, and the run does not decide (a run not TIME-ACCURATE by E-1, or a
  class NOT EVALUABLE, or PHYSICAL without persistence), with the reason by the rule and what it would take.
* **Stopped and reported** if the record cannot be made opt-in with the defaults bit-identical, if anything else in the
  solver needs changing, or if check H cannot be run as registered on this base (a refused case, a failed S1 or C2, a
  divergence).

A miss of R0 does not stop the class (E-3 names what is then committed); it is recorded with its number. A defective
set-up found before the registered runs is corrected by an amendment committed before the corrected run.

## What is not covered

* The record in the fractional step and the moving-mesh steps (section 1).
* A case key for the record (section 1): a library setting only.
* The scheme and the mesh in time, B1, a run from the stalled SIMPLEC state, a third time step or a longer run (stop rule
  4 of ADR 0183; ADR 0766, "What is not covered").
* Arm 1, arm 2 and the decision mapping D9 of ADR 0183.
* The convergence rule of F-870: a question for the maintainer, not decided here.
* Any agreement with the measurement.

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation; not decided by this work: the convergence rule of
F-870, after the class.

## Addendum 1: results

Written 2026-11-22, after the runs. Commits on the branch, from main 21054b68: 94335e42 (this registration, alone),
235d8479 (the record in `crates/rcfd-fv/src/pimple.rs` and the test file; the two registered runs were made from it),
3ded7768 (rustfmt of the test file and the committed mutation patch), d083a836 (the outer records, the logs, the
mutation log and the recorded class of G1), and the commit of this addendum with the proposal file. The records are in
`docs/investigations/f885-check-h-outer-record/`: `H_dt_outer.csv` and `H_dt2_outer.csv` (one row per step: `d_1` to
`d_5` and the step change), `H_dt.log`, `H_dt2.log`, `mutations.log`. R0 was met, so by E-3 the site signal is read from
the committed records of ADR 0766 and the full rows of the new runs stay in the scratch folder.

### The sentence

**The run does not decide.** By ADR 0183's outer-iteration rule as written, no step of the window of either run meets
it (0 of 2 749 in H(dt), 0 of 5 498 in H(dt/2), against 99 %): both runs are NOT TIME-ACCURATE, the class of ADR 0183 is
NOT EVALUABLE and the verdict of F-879 is "does not decide". No class is asserted.

The record shows why, and it is not the flow: in every step of both runs the velocity change of the 5th outer iteration
is about as large as that of the 1st (`d_5 / d_1` 0.93 and 0.96), while the 2nd to the 4th contract. The 5th outer
iteration is the unrelaxed one (alpha 1 against the relaxed `alpha_u = 0.9`, `alpha_p` of the case default in the 1st
to 4th), and it moves the velocity by about 1 m/s away from where the relaxed iterations went, and the next step's first
iteration moves it about as far back. The rule measures that switch, not the time accuracy of the state.

### The record (section 1)

| Id | Measured | Met |
| --- | --- | --- |
| A1 | `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored; `baseline_hashes.rs` unchanged | yes |
| A2 | cavity, three steps of 5 outer iterations: record and route B equal to the 13 printed digits at every `k` (e.g. step 1: 3.4197e-2, 7.1717e-3, 1.5369e-3, 3.3740e-4, 1.4550e-3 m/s), relative difference `<= 1e-12` asserted | yes |
| T2 | lid at rest: every `d_k` of three steps exactly 0 | yes |
| T3 | `n_outer = 1`: 4.303299147270e-2, 4.069470398797e-2, 3.756081223779e-2 m/s, equal to the change read from outside | yes |
| T4 | the cumulative change differs from the record at some `k >= 2` of each step | yes |
| T5 | setting off: `outer_velocity` empty, `u`, `p` and flux equal bits after three steps | yes |
| C2 | 20 steps of H(dt) with both records and with neither: site speeds, torque, pressure rise equal bits at every step | yes |
| S1 | after 20 steps: within 1e-6 of 13.824327 m/s, 2.607352 N m, 110.1842 Pa | yes |
| S2 | 20 entries of 5 finite values, `d_1 > 0` (step 1: 36.09, 26.05, 4.316, 1.340, 0.916 m/s) | yes |

The cavity shows the same pattern as the fan on a case with no rotation, no turbulence and no periodic seam: `d_5` is 4
to 15 times `d_4`.

### Check H (section 2)

Both runs completed (A3): 5 498 and 10 996 steps, no failure, every field finite, mass balance 1.6e-14 and 1.4e-14,
156.8 and 256.8 min (1.71 and 1.40 s per step on the shared machine).

| | H(dt) | H(dt/2) |
| --- | --- | --- |
| A4: window steps meeting `d_5 <= 1e-2 d_1` | **0.0000** | **0.0000** |
| `d_5 / d_1` over the window: smallest, median, largest | 0.9309, 0.9333, 0.9356 | 0.9601, 0.9644, 0.9685 |
| `d_1` (m/s): smallest, median, largest | 1.0125, 1.0146, 1.0160 | 0.9705, 0.9735, 0.9768 |
| `d_5` (m/s) | 0.9451, 0.9469, 0.9485 | 0.9367, 0.9390, 0.9411 |
| medians of `d_2/d_1`, `d_3/d_2`, `d_4/d_3`, `d_5/d_4` | 0.209, 0.602, 1.014, 6.70 | 0.199, 0.662, 0.919, 19.4 |
| `d_5 / d_1` by flow-through time 2 to 6 (median) | 0.938, 0.934, 0.933, 0.933, 0.933 | 0.967, 0.965, 0.964, 0.964, 0.964 |
| step change `max_c |u^(n+1) - u^n|` (m/s), median | 0.093 | 0.259 |
| share by the rule of ADR 0766 D-2, same run (reported only) | 0.4376 | 1.0000 |
| R0: values beyond 1e-8 of the ADR 0766 record; largest relative difference | 0; 4.9e-10 | 0; 4.8e-10 |
| site signal: cell, `A`, crossings | c157, 5.364938e-3 m/s, 0 | c157, 6.493334e-3 m/s, 0 |
| torque, pressure rise: window means | 2.007096 N m, 69.6172 Pa | 2.010156 N m, 70.0729 Pa |

R0 was met at every step of both runs: the solution is that of ADR 0766, so everything ADR 0766 reported beside its
class (amplitudes and frequencies of torque and pressure rise, the means against the steady and measured values, the
smallest axial velocity at the site, +0.29 and +0.28 m/s, the five cells of largest amplitude at the tip, the seam-pair
sum 2.3e-15) holds unchanged and is not repeated here; the logs carry it.

| Id | Measured | Met |
| --- | --- | --- |
| A3 | both runs complete | yes |
| A4 | 0 % and 0 % against 99 % | **no, in both runs** |
| A5 | class NOT EVALUABLE (first clause); verdict "does not decide" | a verdict by the rule, the one that does not decide |
| A6 | held by `f885_class_from_the_committed_records` | yes |
| C3, C4, C5 | the sine, the ten pairs and the cut series of ADR 0766, the seven pairs of E-1: each as built | yes |
| R0 | above | yes |
| G1 | counts 5 498 and 10 996, windows 2 749 and 5 498, shares 0 and 0, class and verdict, `A` as recorded | yes |
| D1 | `git diff --name-only 21054b68` names `crates/rcfd-fv/src/pimple.rs` only under `crates/*/src`; `simple.rs`, the two F-879 and F-870 test files and every golden unchanged | yes |

### What the record says, and what it does not

Three facts from the records. (1) `d_1` and `d_5` are about 1 m/s in every step of the window at both time steps; they
do not shrink as the state settles and do not scale with the step (1.01 against 0.97 m/s at a halved step). (2) The step
change, 0.09 and 0.26 m/s, is smaller than either: the 1st iteration moves the field by about 1 m/s and the 5th moves it
most of the way back. (3) The relaxed iterations contract (`d_4 / d_1` about 0.12 in both runs) and the unrelaxed 5th
jumps (`d_5 / d_4` 7 and 19). Together: the relaxed and the unrelaxed outer iterations do not share a fixed point, and
each step oscillates between the two by about 1 m/s at some cell. The same pattern on the cavity (`d_5 / d_4` 4 to 15)
says it belongs to the relaxed PIMPLE outer loop, not to the fan.

Read from the code and not shown by a run: the relaxation of the momentum predictor is implicit about the current
iterate (`FvMatrix::relax`), so it alone leaves the fixed point unchanged; but the relaxed diagonal also enters the
correctors' `rAU` and the face interpolation built on it, which change when alpha goes to 1 in the final iteration. That
is a candidate cause, not a diagnosis. It is consistent with ADR 0766's pressure record (a first pressure change that did
not vanish in the steady window). Where the largest change sits was not recorded (the record keeps the norm, not the
cell). No solver change was made beyond the record: the brief says to stop and report if anything else needs changing.

Said plainly: on this set-up ADR 0183's rule cannot be met by any state, steady or not, because the quantity it reads is
dominated by the switch from relaxed to unrelaxed iterations. Neither run's "no" says that the march is time-inaccurate
in the sense ADR 0183 meant; it says that the outer iterations of a step do not converge to one state.

### What this means for F-870 (task 3; a proposal, nothing is changed)

The class is not decided, so nothing follows for the convergence rule of F-870 from check H. The observation of ADR
0766 stands (the site signal decays to below 1e-4 m/s and the time means settle to 0.15 % in torque and 0.65 % in
pressure rise between the steps), but it now rests on a march whose outer iterations do not converge within a step, so
the time means themselves carry an error of the outer loop of unknown size. ADR 0766's caution stands with it: the
time-marched pressure rise on B2 is 8 to 9 % above the SIMPLEC sample on the same mesh, and part of that gap may be this
unconverged outer loop.

### Mutations and negative controls

From `mutations.log`, on a scratch copy with its own target folder; M6 in the worktree with the file restored after.

| Id | What happened |
| --- | --- |
| K0 | unmutated: the two tests pass |
| M1 | the change from the step start: caught by `f885_record_two_routes` (A2). Committed as the mutation patch |
| M2 | x component only: caught (A2) |
| M3 | the record flips the last bit of one cell's velocity: caught by T5 and by C2 of `f885_short_run_holds_the_state` |
| M4 | ratio 1e-1: caught ("the outer rule fails in H(dt/2)" got PHYSICAL) |
| M5 | `d_4` for `d_5`: caught ("small oscillation in both" got NOT EVALUABLE) |
| M6 | `H_dt_outer.csv` cut to 2 749 rows: caught (2 749 against 5 498 steps) |
| K1 | T4 holds in the unmutated test |

### Deviations

1. In C4 the pair "steady, with the outer ratio large where the step's first change is below the floor" of ADR 0766 is
   about D-2's floor, which E-1 removes; it is kept with its pressure columns and a good velocity record, so it shows
   that the pressure record does not enter the class. The other nine pairs carry their outer argument as `d_5 / d_1`.
2. `cargo fmt -p rcfd-api --check` fails on this machine with "The filename or extension is too long (os error 206)"
   (the command line of the crate's many files), so the new test file was checked with `rustfmt --check --edition 2021`
   on the file itself; `cargo fmt -p rcfd-fv --check` ran as written.
3. The first mutation run did not apply its mutations (a Python version error of the script) and reported every
   mutation as passing; it was corrected and run again, and `mutations.log` is the second run.
4. `cargo test -p rcfd-cli --release --test docs_site` was run on the registration's text before its commit, to check
   its links.

### What was not run

* Any run with another relaxation, another `n_outer`, or a longer run (stop rule 4 of ADR 0183); any search for the
  cell or the cause of the change of the final iteration.
* The full workspace test run; only the checks of rule 7 named below.

### The checks of rule 7

* `cargo test -p rcfd-api --release --test f885_check_h_outer_record`: 4 passed, 2 ignored (the heavy records, run
  once each).
* `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored.
* `cargo test -p rcfd-fv --release --lib`: 115 passed, 6 ignored.
* `cargo test -p rcfd-api --release --test f879_fan01_check_h`: 4 passed, 2 ignored.
* `cargo fmt -p rcfd-fv --check`: clean; the new test file by `rustfmt --check`: clean (deviation 2).
* `cargo clippy -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`: clean.
* `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and
  `python scripts/process_report.py --check`: at the commit of this addendum, as reported to the lead.

### Size

Size (agent): partial; 1; 2; 2; 330000

One further registration is expected if the maintainer takes option (a) of question 1.

### Decision by the registered rule

**Partial.** The record is in and shown correct (A2, T2 to T5), the defaults are bit-identical (A1, C2), the two runs
were made, and the run does not decide: neither run is TIME-ACCURATE by E-1. What it would take: an outer loop whose
iterations converge to one state within a step, so that the rule reads the time accuracy of the march; or the
maintainer's reading of the rule for a relaxed PIMPLE loop (question 1).

## Open questions for the maintainer (addendum 1)

1. **The outer-iteration rule on a relaxed PIMPLE loop.** On this set-up the 5th, unrelaxed outer iteration moves the
   velocity as far as the 1st in every step, at both time steps, so ADR 0183's rule as written cannot be met. Options:
   (a) a diagnosis registered under F-885 (rule 13), on the cavity and on 20 steps of H(dt), that finds where and why the
   relaxed and the unrelaxed iterations have different fixed points (the record with the cell of its largest change;
   the same steps with alpha 1 in every outer iteration), with no long run, and then a decision on the remedy; (b) check
   H run again with `alpha_u = alpha_p = 1` in every outer iteration, so that every iteration solves the same equations,
   which departs from the case default and may not be stable, about seven hours of the shared machine; (c) the rule read
   on the relaxed iterations only (`d_4 / d_1`), which is a rule written after the result and would fail as well (about
   0.12); (d) leave F-885 and F-879 partial and take the class from the observation of ADR 0766 as the maintainer's
   reading. I recommend (a): it is cheap, it tells whether the loop or the rule is at fault, and it bears on every
   pimple result with `n_outer > 1` and relaxation, not only on check H.
2. **The convergence rule of F-870.** Still not to be decided: no class exists. Options as in ADR 0766 question 2: (a)
   keep `is_converged()` of the steady solve at 1e-5; (b) allow a time march to a settled state, registered with the
   terms of ADR 0766; (c) a rule on a time mean. I recommend (a) for now, and (b) only after question 1 shows the outer
   loop converges within a step, since the time means of a march whose outer iterations do not converge carry an error
   of unknown size; ADR 0766's caution (the time-marched pressure rise on B2 is 8 to 9 % above the SIMPLEC sample) stands.

## For the lead: roadmap entries

> Status proposed for F-885: partial.

> Note entry: ADR 0767, 2026-11-22, partial: the pimple step keeps, opt-in (PimpleSettings::outer_velocity_record, defaults bit-identical, baseline hashes unchanged), the largest velocity change of each outer iteration, shown equal to the changes of the iterates by a second route on a cavity; check H was run again on B2 (both time steps, reproducing the ADR 0766 records to 5e-10 at every step) with ADR 0183's outer rule as written, and no window step of either run meets it (d5/d1 0.93 and 0.96 against 1e-2), because the unrelaxed 5th outer iteration moves the velocity about 1 m/s from the relaxed ones in every step, so the class is NOT EVALUABLE and the run does not decide; held by crates/rcfd-api/tests/f885_check_h_outer_record.rs and docs/investigations/f885-check-h-outer-record, six of six mutations caught.

> Gap entry: Missing for done: a class by ADR 0183's rule. The record is in and correct, but in every step the unrelaxed final outer iteration changes the velocity about as much as the first (d5/d1 0.93 at dt, 0.96 at dt/2; the same pattern on a cavity), so the relaxed and unrelaxed iterations do not share a fixed point and the rule cannot be met by any state. Needs a diagnosis of the outer loop (proposed under F-885), or a run without relaxation, or the maintainer's reading of the rule.

The same entries are in `docs/roadmap/proposals/0767-F-885.json` (form 1 of ADR 0755, base 21054b68), checked with
`python scripts/roadmap_proposals.py check`. No follow-up item is proposed: the diagnosis of question 1 (a) adds no
acceptance clause and would be registered under F-885 (rule 13).

Generated pages that this work may leave not current, for the lead (rule 9): `docs/adr/index.md` (ADR 0767 is new).

## Addendum 2 (2026-11-22): row T3 of the table of checks is malformed

Found by `python scripts/process_report.py --check` after addendum 1. In row T3 of the table of checks the formula of
the check and of the limit, written in code spans, holds the character that separates the cells of a table, so the row
splits into more cells than the header and its kind is read as "0". The row as it was meant, with no number changed:
check "`n_outer = 1` on the cavity: the one value against the largest norm over the cells of `u^(n+1) - u^n`, read from
outside"; limit "relative difference `<= 1e-12`"; source "Same discrete problem by two routes"; estimate 0; margin n/a;
kind **code verification**. T3 was run as meant and met (addendum 1). The table cannot be edited (rule 18), so the
process report keeps naming the row; the lead's list `docs/process/exceptions/registration-kind.txt` is where such a row
is passed over, and that is left to the lead.
