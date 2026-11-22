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
