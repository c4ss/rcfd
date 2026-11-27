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
