# ADR 0821: F-885 registration: the convection's gradient kept from the step's first outer iteration as the remedy, its time order, a steady case, then check H

Status: **REGISTERED 2026-11-27, written before any code of F-885 for this registration and before any run of its checks.**
Before this text no test, harness or solver run of this work was written or made. What was done: reading (below), one
reading script over `docs/roadmap.json` (F-885), `git apply --check` of the seven committed F-885 mutation patches at the
base e60fd829 (all seven apply), and the arithmetic of this page. Results go in addenda at the end; nothing above them is
changed after a run.

It answers the brief [`docs/adr/briefs/f885-frozen-convection-check-h.md`](briefs/f885-frozen-convection-check-h.md) and
follows [`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. A remedy of F-885's miss under rule 13, on the
line that [ADR 0820](0820-f885-registration-the-slow-outer-mode-by-freezing-the-lagged-parts-of-the-predictor.md) named
(its answers 1 to 3 of 2026-11-26). Branch `f885-frozen-convection-check-h` from e60fd829; scratch
`w49i-work/f885-frozen-convection-check-h-scratch`. Written by the model Claude Opus 5.5.

Size (agent): done; 0; 2; 1; 210000

## What is asked

F-885's acceptance (read by script): "... Check H of ADR 0183 is then run as registered there and in ADR 0766, with the
outer-iteration rule of ADR 0183 as written (the velocity change of the 5th outer iteration at most 1e-2 of the 1st in at
least 99 % of the window's steps), and classes the hub pocket as steady, physical or numerical, or is recorded as not
deciding." The record of the acceptance's first clauses is in (ADR 0767). The gap: check H does not decide because the fan's
outer loop does not contract within a step; ADR 0820 named the cause, the deferred correction of linear upwind formed anew
from each outer iterate's gradient. The brief: register `outer_frozen_convection_gradient` as the remedy, with a BDF2
time-order check and a steady check first, then check H as ADR 0767 and ADR 0183 state it, with the cavity as a control.

## What was read

Whole: `AGENTS.md`; `docs/process/agent-rules.md` (version 5); the brief; ADR 0820 (registration, addendum 1, answers);
[ADR 0767](0767-f885-registration-check-h-by-the-velocity-change-of-the-outer-iterations.md) (registration, addenda 1 and
2, the lead's reading, the decisions); `crates/rcfd-api/tests/f885_slow_outer_mode.rs`.

In part: [ADR 0811](0811-f885-registration-a-face-flux-independent-of-alpha-u-then-check-h.md), the sentence and the
lines naming its check H runs (94.7 and 181.0 min); `crates/rcfd-api/tests/f885_alpha_free_face_flux.rs` lines 1 to 124 and
244 to 843 (check H's case, measures, class, verdict, `run_h`) and 1311 to 1370 (its heavy runs and the class from the
committed records); `crates/rcfd-api/tests/f885_check_h_outer_record.rs` lines 997 to 1160 (C3 to C5); `docs/process/tolerances.md`,
the table; `crates/rcfd-fv/src/pimple.rs` (base e60fd829) lines 195 to 230 (the F-885 settings), 1888 and 1940 to 1951 (the
weights of the old levels; the kept vectors, locals of `step_with`), 2040 to 2262 (the outer iteration: the extrapolation,
the kept flux, the gradient, the kept convection gradient, the limiter, `convection`, the Laplacian's gradient), 3750 to
3760 (`ddt_history_weights`); `crates/rcfd-fv/tests/consistent_benchmarks_f301.rs` lines 1 to 295 (the Taylor-Green set-up
of ADR 0223 and its order harness, with `nu = 0.1` and the reference step 1/1280).

No published source outside the repository was read. The order argument is from first principles (section 1); BDF2's
formal order 2 is the scheme's own, held in the repository by the order rows of ADR 0223 and `crates/rcfd-fv/src/ddt.rs`
(`bdf2_variable_step_second_order`).

## 1. The setting (fixed now)

**What it is.** `PimpleSettings::outer_frozen_convection_gradient` of ADR 0820, as it stands on the base: per component,
the least-squares gradient that enters `convection` (the deferred correction of linear upwind, and the limiter factor where
one is formed) in the step's first outer iteration is kept in `frozen_grad_conv[k]` and used in its place in the step's
later outer iterations, the final one included (`pimple.rs` lines 2196 to 2208 at e60fd829). The kept vectors are locals of
`step_with`, created empty before the outer loop of each step (line 1950), so they are reset at every step. Default
`false`: no branch is taken and no copy made, so every default is bit-identical. No case key, schema, golden or hash table
(answer 3 of ADR 0820): runs set it on the built solver.

**No library change.** The setting is promoted to the remedy by this registration, not rebuilt: no file under
`crates/*/src` changes. If a check showed that the setting needs a change of the solver, I stop and report (decision rule).

**Why it keeps BDF2's order (the argument, first principles).** In the first outer iteration of a BDF2 step with constant
steps the solver forms the gradient from the extrapolated field `u* = (1 + r) u^n - r u^(n-1)`, `r = dt / dt_old` = 1
(`conv_cells`, lines 2191 to 2196; `extrap = c1 - 1`, line 2074; `c1 = 1 + r`, line 3756), so `u* = u^(n+1) + O(dt^2)`. The
deferred term `phi_f grad(u*) . (x_f - x_U)` then differs from the same term at `u^(n+1)` by `O(dt^2)` per unit time, which
is the order of BDF2's own truncation error: the global error stays `O(dt^2)`. The first step of a run is implicit Euler
(no older level: `c2 = 0`, no extrapolation) and its gradient is that of `u^0`: a local error `O(dt^2)` in one step, which
does not change the global order. Without the extrapolation (the gradient of `u^n`) the lag would be `O(dt)` and the order
1 on a fixed mesh: that is the committed mutation M2, so the order check can tell the two apart.

**Why a steady answer is the same.** At a steady state `u^(n+1) = u^n = u^(n-1)`, so `u*` is the steady field and the kept
gradient is the gradient of the state the outer iterations converge to: the steady discrete equations are the same with the
setting on or off. A march that reaches a steady state with both gives the same answer up to how close each is to it.

## 2. The order check O (fixed now)

The Taylor-Green vortex of ADR 0223's harness (copied): a periodic box `2 pi` by `2 pi`, `box_2d(16, 16, 2 pi, 2 pi, true,
true)`, `nu = 0.1`, the exact velocity and pressure at `t = 0` as the start; `PimpleSettings::piso(0.1)` with `n_outer = 10`,
`alpha_u = alpha_p = 1`, BDF2, `convection = linear_upwind` without a limiter, momentum and pressure solvers `tol_abs` 1e-14,
`tol_rel` 1e-12, `max_iter` 5000. To `t = 1` with `dt` 0.1, 0.05, 0.025 and the reference `dt_ref = 1/640`, each arm
against its own reference on the same mesh (so the spatial error cancels). Two arms: OFF (the setting off) and ON (on).
`e(dt)` is the relative L2 norm over the cells of `u(dt) - u(dt_ref)`; `p_f = log2(e(0.05) / e(0.025))`, `p_c =
log2(e(0.1) / e(0.05))`.

* **O1**: `p_f(ON) >= 1.9`.
* **O2** (the control): `p_f(OFF) >= 1.9`. If O2 is missed, O does not decide.
* **O3** (non-vacuity): the relative L2 difference of `u_ON` and `u_OFF` at `dt` 0.05, `t = 1`, `> 1e-9`. If not, O does not
  decide.
* Beside, without a limit: `p_c` of both arms, every `e`, `e_ON / e_OFF`, `abs(p_f(ON) - p_f(OFF))`.

**Limit and source.** `docs/process/tolerances.md`, "Observed order of accuracy" (at least the formal order less 0.1), with
BDF2's formal order 2. Ten outer iterations without relaxation so that the outer loop of either arm leaves little of a step
unconverged; the fan's five are check H's, not the order check's. **Estimate**: `p_f` about 2.0 in both arms; the reference
step leaves `(0.025 / (1/640))^-2` = 1/256 of `e(0.025)`, which moves `p_f` by about 0.006. Cost: about 1 400 steps of 256
cells, seconds.

## 3. The steady check S (fixed now)

The 8 x 8 lid-driven cavity of ADR 0767 (`box_2d(8, 8, 1, 1)`, walls fixed zero, lid `(1, 0, 0)`, `nu = 0.01`, from rest),
`PimpleSettings::pimple(0.01, 5)` with its relaxation, BDF2, `linear_upwind` without a limiter, `alpha_free_face_flux` (as
check H runs; its fixed point does not depend on `alpha_u`, ADR 0811), solvers `tol_abs` 1e-15, `tol_rel` 1e-12, `max_iter`
5000, `dt` 0.2. Marched until the step change `max_c abs(u^(n+1) - u^n)` (Euclidean per cell) is at most 1e-12 m/s, at most
5 000 steps. Arms OFF and ON.

* **S1**: both arms reach the criterion within 5 000 steps. If not, S does not decide.
* **S2**: `max_c abs(u_ON - u_OFF) / max_c abs(u_OFF) <= 1e-8`, and the same measure of the face flux `<= 1e-8`.
* **S3** (non-vacuity): after 5 steps the velocity measure of S2 is `> 1e-6`. If not, S does not decide.
* Beside, without a limit: the same with Barth-Jespersen and `outer_frozen_limiter` in both arms (SL): steps to the
  criterion, or the step change reached at 5 000 steps, and the measure of S2.

**Limit and source.** `docs/process/tolerances.md`, "Same discrete problem by two routes" (`<= 1e-8`): at a steady state
both arms solve the same discrete equations (section 1). **Estimate**: the slowest mode decays by about 4 % a step at `dt`
0.2 (`nu pi^2 2 dt` = 0.04), so a step change of 1e-12 leaves about 1e-12 / 0.04 = 2.5e-11 of distance to the steady state;
S2 about 1e-10, margin 100. Cost: seconds.

## 4. The cavity control KC (fixed now; classed, not a limit)

The cavity of section 3 with check H's settings: `pimple(0.01, 5)`, BDF2, Barth-Jespersen with `outer_frozen_limiter`,
`alpha_free_face_flux`, the velocity record, `dt` 0.05, 20 steps from rest; ON and OFF. Per step E-1 of ADR 0767
(`d_5 <= 1e-2 d_1`); recorded: the share of the 20 steps that meet it and the median `d_5 / d_1` of each arm. It shows the
remedy on a case with no rotation, turbulence or seam; it has no part in the decision.

## 5. Check H (fixed now)

As registered in ADR 0183 and [ADR 0766](0766-f879-registration-fan01-check-h-the-hub-pocket-in-time.md) with E-1 to E-4 of
ADR 0767, and run as ADR 0811 ran it (its `run_h`, the case text, the measures, `class_0183` and `verdict` copied unchanged
from `f885_alpha_free_face_flux.rs`): B2 (13 627 cells), H(dt) 4.605e-5 s 5 498 steps and H(dt/2) 2.3025e-5 s 10 996 steps,
the window above 0.1266 s, the six site cells, the STEADY threshold 0.193 m/s, the classes of ADR 0183 in their order, the
verdict of F-879 with persistence 0.75, no wall-clock cap, both records on (`outer_pressure_tolerance = Some(-1.0)`,
`outer_velocity_record`), `n_outer` 5 of the case, the outer rule of E-1 as written, the decision rule of ADR 0767
unchanged. Stop rule 4 of ADR 0183 holds: after the two runs nothing more is run in this part.

**The opt-ins (the only departure from ADR 0811's runs):** `alpha_free_face_flux` (ADR 0811), `outer_frozen_limiter` (ADR
0819) and `outer_frozen_convection_gradient` (this remedy), as GD of ADR 0820 had them, with five outer iterations.

**Run only if O and S hold** (O1, O2, O3, S1, S2, S3 met). Both runs started together, one process each, in the background.
Expected duration: about 1.6 h and 3 h (ADR 0811's 94.7 and 181.0 min; GD's step was 14 % faster than F0's in ADR 0820), no
cap. **Records** in `docs/investigations/f885-frozen-convection-check-h/`: `H_dt.csv` and `H_dt2.csv` (the full rows of ADR
0811's form: the solution differs from ADR 0766's, so R0 of ADR 0767 does not apply and the rows are committed, E-3), and
their logs.

**Estimate (not a limit).** GD of ADR 0820 had `d_5 / d_1` median 2.7e-3, largest 5.8e-3 over steps 11 to 20 of H(dt) (with
20 outer iterations, the 5th relaxed); with 5 the 5th is the final, unrelaxed one, whose jump `alpha_free_face_flux` removes
(ADR 0811). A4: `d_5 / d_1` about 3e-3, margin 3.3, in both runs; A5 STEADY if A4 holds (ADR 0811 measured `A` 1.45e-3 and
2.29e-3 m/s against 0.193).

**SR, the fast test on the same path.** 20 steps of `run_h` with the opt-ins, against the first 20 rows of the committed
`H_dt.csv`: every column but the time relative `<= 1e-6`; 20 rows, each `d_k` finite, `d_1 > 0`.

## Table of checks

Every clause of the acceptance first (A3 to A6 are check H; the acceptance's record clauses are held by ADR 0767 and are not
run again).

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A3 | check H with the opt-ins of section 5: H(dt) completes 5 498 steps and H(dt/2) 10 996, no failure, no guard trip, every field finite | both complete | acceptance of F-885; ADR 0183 D5; ADR 0767 A3 | completes | n/a | solution verification |
| A4 | each run TIME-ACCURATE by E-1 of ADR 0767 | at least 99 % of the window's steps with `d_5 <= 1e-2 d_1` | acceptance of F-885; ADR 0183, check H, the outer-iteration rule | `d_5 / d_1` about 3e-3 | 3.3 | solution verification |
| A5 | the class by ADR 0183's rule and the verdict of F-879 by ADR 0766's table | one of the four, by the registered numbers of ADR 0767 A5 | acceptance of F-885; ADR 0183, check H; ADR 0766 | STEADY | n/a | solution verification |
| A6 | a run that does not decide is recorded as not deciding | as stated | acceptance of F-885 (last clause) | as stated | n/a | solution verification |
| O1 | TG, BDF2, the setting on: `p_f` | `>= 1.9` | `docs/process/tolerances.md`, "Observed order of accuracy" (formal order 2 less 0.1) | 2.0 | 0.1 in order | solution verification |
| O2 | TG, BDF2, the setting off (control): `p_f` | `>= 1.9` | as O1 | 2.0 | 0.1 in order | solution verification |
| O3 | non-vacuity: `u_ON` against `u_OFF`, `dt` 0.05 | relative L2 `> 1e-9` | this registration (the setting must act on the case) | order 1e-4 | n/a | code verification |
| S1 | cavity, steady march: both arms reach a step change `<= 1e-12` m/s | within 5 000 steps | this registration (the criterion that makes S2 meaningful, section 3) | about 700 steps | 7 | code verification |
| S2 | the steady answers, ON against OFF: velocity and face flux | relative `<= 1e-8` | `docs/process/tolerances.md`, "Same discrete problem by two routes" | 1e-10 | 100 | code verification |
| S3 | non-vacuity: after 5 steps | relative `> 1e-6` | this registration (the setting must act in the transient) | order 1e-3 | n/a | code verification |
| KC | the cavity with check H's settings, ON and OFF: share of 20 steps meeting E-1, median `d_5 / d_1` | classed (recorded, not a limit) | section 4 | ON meets in most steps | n/a | solution verification |
| SR | 20 steps of the check H path against the first 20 rows of `H_dt.csv` | relative `<= 1e-6`; finite; `d_1 > 0` | "Same discrete problem by two routes", widened to the 10 printed digits as ADR 0767 S1 | 0 | n/a | code verification |
| C5 | the rule of E-1 on built records (ADR 0767 C5, copied) | each as built | ADR 0767 E-1 | as built | n/a | code verification |
| G1 | the class and verdict from the committed `H_dt.csv` and `H_dt2.csv`: counts 5 498 and 10 996, windows 2 749 and 5 498, the shares of E-1, class, verdict, `A` and `f` as recorded | counts exact; class and verdict equal; 1e-6 relative | "Same discrete problem by two routes", widened to the printed digits as ADR 0767 G1 | 0 | n/a | unchanged default output |
| B3 | no solver default changes: `cargo test -p rcfd-api --release --test baseline_hashes` passes, `baseline_hashes.rs` unchanged | passes, file unchanged | rule 3; the brief | passes | n/a | unchanged default output |
| D1 | no file under `crates/*/src` changes; `docs/roadmap.json`, the ledger, every golden and every existing test file unchanged; the seven committed F-885 mutation patches apply (`git apply --check`) | as stated | the brief, owned files; rule 3 | as stated | n/a | unchanged default output |
| P | `cargo test -p rcfd-api --release --lib`; the new file's gate tests; `--test f885_slow_outer_mode`, `--test f885_frozen_limiter`, `--test f885_alpha_free_face_flux` (the gate tests of the three opt-ins); `rustfmt --check` of the new test file (for `cargo fmt -p rcfd-api --check`, which refuses on this machine with os error 206); `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

No row is of kind validation: F-885 claims no agreement with a measured case. **Reported beside, without a limit:** per check
H run, as ADR 0811 reported it (the spread of `d_5 / d_1`, `d_1`, `d_5` and of the contractions, by flow-through time; the
share by ADR 0766 D-2; torque and pressure rise with amplitude, frequency and means; the five cells of largest amplitude;
the seam-pair sum; the mass balance; seconds per step), and the beside values of O, S and SL.

## Tests (fixed now)

All in `crates/rcfd-api/tests/f885_frozen_convection_check_h.rs` (header: ADR 0821, F-885). Gate tests:
`f885_fc_time_order` (O), `f885_fc_steady` (S, SL), `f885_fc_cavity_control` (KC), `f885_fc_rules_on_built_records` (C5),
`f885_fc_short_run_on_the_check_h_path` (SR), `f885_fc_class_from_the_committed_records` (G1; values written into `RECORD`
after the runs, the form and the tolerance fixed here). Heavy records (`#[ignore]`, run once by me, writing to the folder
named by `F885_FC_OUT`): `f885_fc_h_dt_record`, `f885_fc_h_dt2_record`; their fast test on the same path is SR.

## Negative controls and planned mutations

On a scratch copy of the full commit (`git archive` into the scratch folder, its own target folder, every touched source
file's time set after writing), each against the named gate test; the heavy records are not rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: `outer_frozen_convection_gradient` not read (the gradient formed anew is always used) | `f885_fc_time_order` (O3) |
| M2 | solver: the kept gradient formed from the iterate before the BDF2 extrapolation (`cells`, `u^n`) instead of `conv_cells` | `f885_fc_time_order` (O1) |
| M3 | solver: the kept gradient scaled by 0.99 where it is used | `f885_fc_steady` (S2) |
| M4 | test: the ratio of E-1 1e-1 for 1e-2 | `f885_fc_rules_on_built_records` (C5) |
| M5 | record: `H_dt.csv` cut to its first half | `f885_fc_class_from_the_committed_records` (G1) |

The committed patch (`crates/rcfd-api/tests/f885_frozen_convection_check_h.mutation.patch`) is M2, with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f885_frozen_convection_check_h f885_fc_time_order`.

## Decision rule

* **Done** for F-885 if O (O1, O2, O3), S (S1, S2, S3), A3 and A4 in both runs, A5 with the verdict steady, physically
  unsteady or numerical, G1, SR, C5, B3, D1 and P are met, and each of M1 to M5 is caught.
* **Partial** otherwise, with the reason by the rule: O or S missed or not deciding (then check H is not run); check H run
  and not deciding (a run not TIME-ACCURATE by E-1, NOT EVALUABLE, or PHYSICAL without persistence); a row or a mutation
  missed.
* **Stopped and reported** if the setting cannot be used without a change of the solver, if anything else in the solver
  would need changing, or if check H cannot be run as registered on this base.

A defective set-up found before a registered run is corrected by an amendment committed before the corrected run.

## What is not covered

* The setting as a default, a case key, the setting in the steady solver or the fractional step.
* Arm 1, arm 2 and D9 of ADR 0183; a third time step or a longer run (stop rule 4); any agreement with a measurement.
* The order of the setting with the limiter (SL is beside only) or with variable steps.
* The convergence rule of F-870 (ADR 0767 question 2): a question after the class.

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation: whether the setting becomes a default (not on this
branch), and any question of scope.

## Amendment 1 (2026-11-27): O does not decide; check H is not run; what follows, fixed before any further code or run

Written after the one registered run of O, S, KC and C5 at 36c9be6b (`f885_fc_time_order`, `f885_fc_steady`,
`f885_fc_cavity_control`, `f885_fc_rules_on_built_records`, together, 20.9 s), and before any other code or run. Nothing
above is changed. The run, as it came out:

* O: OFF `e` = 5.1468e-4, 2.2644e-4, 1.1204e-4, `p_c` 1.1846, `p_f` 1.0151; ON `e` = 1.1046e-3, 4.8429e-4, 2.3214e-4,
  `p_c` 1.1896, `p_f` 1.0609; O3 4.1524e-4. **O2 missed (1.0151 against 1.9): O does not decide.** O1 is missed as well
  (1.0609); O3 is met. The control arm, without the setting, is first order on this set-up, so the set-up cannot measure
  BDF2's order with or without the setting.
* S: both arms reach the criterion (211 and 210 steps); velocity 3.38e-13, flux 3.34e-13 against 1e-8; after 5 steps
  8.31e-3. S1, S2 and S3 met. SL (beside): 207 and 206 steps, 5.39e-13 and 5.02e-13.
* KC (classed): OFF 20 of 20 steps meet E-1, `d_5 / d_1` median 3.02e-3; ON 20 of 20, median 2.95e-3.
* C5: each as built.

By section 5 and the decision rule, **check H is not run** (it runs only if O and S hold), and F-885 stays **partial**. The
following is fixed now:

1. **The known miss (rule 2).** `f885_fc_time_order` becomes the ignored record `f885_fc_time_order_known_miss`, its limits
   intact and the measured numbers in its ignore reason. No other change to it.
2. **The tests that read check H's records.** `f885_fc_short_run_on_the_check_h_path` (SR) and
   `f885_fc_class_from_the_committed_records` (G1) read files that exist only after check H. They are marked
   `#[ignore = "not run: check H was not run by ADR 0821's rule (amendment 1)"]` and are recorded as not run, unchanged
   otherwise, so the gate does not fail on a missing file. This is not a pass: SR, G1, A3 to A6 are "not run".
3. **The mutations.** M1 and M2 are read by O, which fails without a mutation: both are run once and recorded as **not
   caught (the test fails unmutated)**. M5 needs `H_dt.csv` and is not run. M3 and M4 are run as registered. The committed
   patch is M3 (`crates/rcfd-api/tests/f885_frozen_convection_check_h.mutation.patch`, first line `# expect-fail: cargo test
   -p rcfd-api --release --test f885_frozen_convection_check_h f885_fc_steady`), in place of M2, whose test is now ignored.
4. **A diagnosis of O2, a record without limits** (rule 2: a miss is diagnosed). One ignored test
   `f885_fc_order_diagnosis_record`, the OFF arm of O with one change each, `p_c` and `p_f` printed, run once:
   * D-a: `convection = central` (ADR 0223's harness, whose PIMPLE-4 Taylor-Green rows show about order 2);
   * D-b: linear upwind, `n_outer` 20 (whether the outer loop leaves an `O(dt)` part);
   * D-c: linear upwind, `periodic_partner_extrapolated` (ADR 0687 amendment 1: the periodic partner of the extrapolated field);
   * D-d: linear upwind, implicit Euler (a first-order reference).

   Reading, fixed now: if D-a gives `p_f >= 1.9` and D-b, D-c do not, the first order belongs to the linear-upwind path of
   the pimple step on periodic faces and is not a property of the remedy; if D-c gives `p_f >= 1.9`, the lagged periodic
   partner carries it; if D-b does, the outer loop. The diagnosis decides nothing for this registration. It informs the
   open questions.

## Addendum 1: results

Written 2026-11-27, after the runs. Commits on the branch, from e60fd829: dc737791 (this registration, alone), 36c9be6b
(the test file; the registered run of O, S, KC and C5 was made from it), 1d023bd8 (amendment 1, alone), 557fd6e1 (the
amendment's test changes and the diagnosis record), bea7c041 (the records, the mutation log, the committed mutation M3),
d41f7e19 (a type alias for clippy, deviation 2), and the commit of this addendum with the rule 7 log and the proposal file.
Records in `docs/investigations/f885-frozen-convection-check-h/`: `gate.log`, `diagnosis.log`, `mutations.log`,
`rule7.log`. No file under `crates/*/src` changed.

### The sentence

**The order check does not decide, so check H was not run.** On the registered Taylor-Green set-up the pimple step with
BDF2 is first order in `dt` with the setting off (`p_f` 1.015), and the same with it on (1.061), against 1.9: the control
O2 is missed, so O does not decide (amendment 1). The diagnosis shows that the first order belongs to the set-up, not to
the remedy: central convection gives 1.012, and 20 outer iterations or the extrapolated periodic partner leave the OFF
numbers unchanged to five digits. The steady check holds: with the setting on and off the cavity reaches the same steady
state to 3.4e-13 (velocity) and 3.3e-13 (flux), against 1e-8. By the registered rule F-885 stays **partial**.

### Table of checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| A3 to A6 | as registered | check H not run (section 5: it runs only if O and S hold) | not run |
| O1 | `p_f(ON) >= 1.9` | 1.0609 (`e` 1.1046e-3, 4.8429e-4, 2.3214e-4; `p_c` 1.1896) | no |
| O2 | `p_f(OFF) >= 1.9` | 1.0151 (`e` 5.1468e-4, 2.2644e-4, 1.1204e-4; `p_c` 1.1846) | no: O does not decide |
| O3 | `> 1e-9` | 4.1524e-4 | yes |
| S1 | within 5 000 steps | OFF 211, ON 210 steps | yes |
| S2 | `<= 1e-8` | velocity 3.3828e-13, flux 3.3426e-13 | yes |
| S3 | `> 1e-6` | 8.3136e-3 | yes |
| KC | classed | OFF: 20 of 20 steps meet E-1, `d_5 / d_1` median 3.02e-3; ON: 20 of 20, median 2.95e-3 | classed |
| SR | as registered | not run (no record of check H; amendment 1) | not run |
| C5 | each as built | each of the seven as built | yes |
| G1 | as registered | not run (no record of check H; amendment 1) | not run |
| B3 | passes, file unchanged | `baseline_hashes`: 3 passed, 1 ignored; `baseline_hashes.rs` unchanged | yes |
| D1 | as stated | `git diff --name-only e60fd829` names the new test file, its patch, this ADR, the four logs and the proposal file; nothing under `crates/*/src`, no roadmap, ledger, golden or existing test file; the seven F-885 patches and the new one apply | yes |
| P | rule 7 | see "The checks of rule 7"; clippy failed at the first run (deviation 2) | met after deviation 2 |

Beside, without a limit: `e_ON / e_OFF` 2.15, 2.14, 2.07; `abs(p_f(ON) - p_f(OFF))` 0.046. SL (the limiter with
`outer_frozen_limiter` in both arms): 207 and 206 steps, velocity 5.39e-13, flux 5.02e-13. Wall time of the four gate
tests together: 20.9 s on the shared machine.

### The diagnosis (amendment 1, item 4)

| Arm | `e` at 0.1, 0.05, 0.025 | `p_c` | `p_f` |
| --- | --- | --- | --- |
| D-a central | 5.0673e-4, 2.2794e-4, 1.1304e-4 | 1.1526 | 1.0118 |
| D-b linear upwind, `n_outer` 20 | 5.1468e-4, 2.2644e-4, 1.1204e-4 | 1.1846 | 1.0151 |
| D-c linear upwind, `periodic_partner_extrapolated` | 5.1468e-4, 2.2644e-4, 1.1204e-4 | 1.1846 | 1.0151 |
| D-d linear upwind, implicit Euler | 2.3297e-3, 1.1465e-3, 5.5472e-4 | 1.0229 | 1.0474 |

None of the three readings fixed in amendment 1 applies. D-a is also first order, so the first order does not belong to the
linear-upwind path. D-b and D-c equal the OFF arm to five digits, so neither the outer loop nor the periodic partner
carries it. BDF2's error is 4.5 times smaller than Euler's at equal `dt` but falls at the same rate: an `O(dt)` error common
to every arm dominates it. That D-b equals OFF exactly suggests that more outer iterations change nothing on this set-up.
One reading, from the code and not shown by a run: the outer loop may end early on a tolerance of the `piso` settings, or
the start (an implicit Euler first step from an exact field with no consistent face flux) leaves an error of order `dt`.
I do not know which; ADR 0223's harness reported about order 2 for its PIMPLE-4 Taylor-Green rows. That harness measures
its errors in its own way, which I did not read in full.

### Mutations

From `mutations.log` (git archive of 557fd6e1 in the scratch folder, its own target folder, the touched files' times set):

| Id | What happened |
| --- | --- |
| K0 | unmutated: `f885_fc_steady` and `f885_fc_rules_on_built_records` pass |
| M1 | the setting not read: **not caught** (the test fails unmutated). It changed what the test reads: O3 0 against 4.15e-4 |
| M2 | the kept gradient from `u^n`: **not caught** (the test fails unmutated). It changed what the test reads: `e_ON` 2.8 times larger, `p_f(ON)` 1.0955 |
| M3 | the kept gradient times 0.99: caught by `f885_fc_steady` (S2 2.95e-3 against 1e-8). Committed as the mutation patch |
| M4 | the ratio of E-1 1e-1: caught by `f885_fc_rules_on_built_records` ("d5/d1 = 0.05 everywhere" got Steady) |
| M5 | not run (no `H_dt.csv`; amendment 1) |

Two of five caught, two not caught, one not run.

### Deviations

1. The registered run of O missed its control. Amendment 1 (1d023bd8) was written and committed before any further code or
   run. It turned the order test into the ignored record `f885_fc_time_order_known_miss`, limits intact. It marked SR and G1
   as not run, because their record files cannot exist. It moved the committed patch from M2 to M3. It fixed the diagnosis.
2. The first run of clippy (at bea7c041) named one lint of the new test file, `type_complexity` in the diagnosis arms. The
   fix in d41f7e19 is a type alias with no change of behaviour. `rustfmt --check` and clippy were run again on it and are
   clean. No test was rerun for it.
3. `cargo fmt -p rcfd-api --check` refuses on this machine (os error 206) and checks nothing. `rustfmt --check --edition
   2021` of the new test file is the registered replacement.
4. The rule 7 run of the new file's gate tests (S, KC, C5, at bea7c041) is the rule 7 check, not a second registered run.
   It gave what the registered run gave.

### What was not run

Check H (A3 to A6), SR, G1, M5; any order case other than the registered one and its four diagnosis arms; the whole
workspace suite.

### The checks of rule 7

From `rule7.log`, at bea7c041 unless stated: `cargo test -p rcfd-api --release --lib` 160 passed, 4 ignored; `--test
f885_frozen_convection_check_h` 3 passed, 6 ignored; `--test f885_slow_outer_mode` 6 passed, 1 ignored; `--test
f885_frozen_limiter` 7 passed, 1 ignored; `--test f885_alpha_free_face_flux` 5 passed, 3 ignored; `--test baseline_hashes` 3
passed, 1 ignored. `rustfmt --check` of the test file clean; clippy as in deviation 2, clean at d41f7e19. `cargo test -p
rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py
--check` run at the commit of this addendum, as reported to the lead.

### Size

Size (agent): partial; 1; 0; 2; 230000

### Decision by the registered rule

**Partial.** O does not decide (O2 missed), so check H was not run. M1 and M2 are not caught, and M5, SR and G1 were not
run. S, C5, B3 and D1 are met, P after deviation 2. What it would take: an order case on which the pimple step shows BDF2's
order without the setting, then O on it as a new registration under F-885 (rule 13), then check H.

## Open questions for the maintainer

1. **The order case.** The registered Taylor-Green set-up is first order in `dt` with every scheme tried. Options:
   * (a) A diagnosis registered under F-885 that finds why this harness is first order. The candidates are the start, an
     early end of the outer loop, and how ADR 0223's harness measured its errors. Then O again on a corrected set-up,
     registered before its run.
   * (b) O on a case with no periodic faces whose BDF2 order the repository already holds: the wall-bounded mode of
     `fractional_step.rs` `f2c` (order 2.07 there), built on the pimple step.
   * (c) Accept the steady check and the first-order equality of the two arms (`p_f` 1.06 against 1.02) as enough for
     check H. That changes the registered rule and is the maintainer's to make.

   I recommend (a), with (b) as its control case in the same registration. A first-order pimple step on a smooth
   periodic case bears on every BDF2 result of the transient solver, not only on this remedy.
2. **The setting as a default.** Options: (a) keep it opt-in until check H has decided with it; (b) make it a default now.
   I recommend (a). Its order is not shown, and as a default it would change every hash of a linear-upwind pimple run.
3. **Scope.** No new item is proposed. The order case of question 1 adds no acceptance clause of F-885's own and would be
   registered under it (rule 13).

## For the lead: roadmap entries

> Status proposed for F-885: partial.

> Note entry: ADR 0821, 2026-11-27, partial: the remedy outer_frozen_convection_gradient (ADR 0820's opt-in, no library change, baseline hashes unchanged) gives the same steady answer on the cavity (velocity 3.4e-13, flux 3.3e-13 against 1e-8), but the registered BDF2 order check on the Taylor-Green vortex does not decide: its control without the setting is first order (p_f 1.015; 1.061 with it, against 1.9), and so is a central-convection arm, so check H was not run by the registered rule; held by crates/rcfd-api/tests/f885_frozen_convection_check_h.rs and docs/investigations/f885-frozen-convection-check-h.

> Gap entry: Missing for done: a class by ADR 0183's rule. The remedy (outer_frozen_convection_gradient) holds a steady answer, but its BDF2 order check did not decide: on ADR 0821's Taylor-Green set-up the pimple step is first order with or without it (p_f 1.015 and 1.061, central 1.012). Needs an order case on which the pimple step shows BDF2's order without the setting, the remedy's order on it, then check H with the remedy.

The same entries are in `docs/roadmap/proposals/0821-F-885.json` (form 1 of ADR 0755, base e60fd829), checked with
`python scripts/roadmap_proposals.py check`. No follow-up item is proposed (question 3).

Generated pages that this work may leave not current, for the lead (rule 9): `docs/adr/index.md` (ADR 0821 is new).
