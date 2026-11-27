# ADR 0822: F-885 registration: why the pimple step's time-order case is first order, the remedy's order on a corrected case, then check H

Status: **REGISTERED 2026-11-27, written before any code of F-885 for this registration and before any run of its checks.**
Before this text no test, harness or solver run of this work was written or made. What was done: reading (below), one
reading script over `docs/roadmap.json` (F-885), `git apply --check` of the eight committed F-885 mutation patches at the
base efce0cf1 (all eight apply), and the arithmetic of this page. Results go in addenda at the end; nothing above them is
changed after a run.

It answers the brief [`docs/adr/briefs/f885-order-case.md`](briefs/f885-order-case.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. A follow-up of F-885's miss under rule 13, on the
line that answer 1 of [ADR 0821](0821-f885-registration-the-frozen-convection-gradient-its-time-order-a-steady-case-and-check-h.md)
named (a diagnosis of the first order, the wall-bounded mode `f2c` of `fractional_step.rs` as its control, then the order
check again on a corrected set-up). Branch `f885-order-case` from efce0cf1; scratch `w49i-work/f885-order-case-scratch`.
Written by the model Claude Opus 5.5.

Size (agent): done; 0; 3; 0; 160000

## What is asked

F-885's acceptance (read by script): "... Check H of ADR 0183 is then run as registered there and in ADR 0766, with the
outer-iteration rule of ADR 0183 as written (the velocity change of the 5th outer iteration at most 1e-2 of the 1st in at
least 99 % of the window's steps), and classes the hub pocket as steady, physical or numerical, or is recorded as not
deciding." The gap: ADR 0821's order check O did not decide (its control O2 without the setting was first order, `p_f`
1.015), so check H was not run with the remedy `outer_frozen_convection_gradient`.

## What was read

Whole: `AGENTS.md`; `docs/process/agent-rules.md` (version 5); the brief; ADR 0821 (registration, amendment 1, addendum 1,
answers); `crates/rcfd-api/tests/f885_frozen_convection_check_h.rs`.

In part: [ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) lines 70 to 95 (hypothesis H-COMM, the algebra of the
commutator) and 300 to 440 (A1, Y1, F-C, F-G, the final addendum's table);
[ADR 0223](0223-f301-registration-consistent-piso-correctors-moving-mesh-lag-estimate-case-key-and-benchmarks.md) lines 468
to 500 (the BO and BF orders); [ADR 0245](0245-f344-registration-bdf2-step-remainder.md), the lines found by a search for
Taylor-Green and the order (its roadmap note at line 625: the start, the measure, the outer loop and the correctors ruled
out by measurement; the opt-in `rhie_chow_step_consistent` acts on internal faces only); `crates/rcfd-fv/src/pimple.rs`
(base efce0cf1) lines 55 to 70, 135 to 160, 250 to 290, 1137 to 1232 (`piso`, `pimple`), 1476 to 1488 (`new`: the face flux
from the cell velocity, one history level), 1886 to 1912 (the old-level flux defects), 2440 to 2530 (the face coefficient,
the corrector's `phiHbyA`), 3752 to 3830 (`ddt_history_weights`, `PimpleState`, `snapshot`, `restore`);
`crates/rcfd-fv/src/ddt.rs` lines 24 to 56 (`History`, `push`); `crates/rcfd-fv/tests/fractional_step.rs` lines 1 to 40 and
380 to 425 (`mode`, `f2c_bdf2_order_without_periodic_faces`); `crates/rcfd-fv/tests/consistent_benchmarks_f301.rs` lines 1
to 345 and 366 to 392 (ADR 0069's order harness, its `Mode` problem, the BO rows); `docs/process/tolerances.md`, the table.

No published source outside the repository was read. The mechanism is the repository's own (ADR 0073's H-COMM, derived
there from the discrete equations); the arithmetic below is from first principles.

## 1. The hypotheses (fixed now)

The set-up of ADR 0821's O: `box_2d(16, 16, 2 pi, 2 pi, true, true)`, `nu = 0.1`, `piso(0.1)` with `n_outer` 10,
`alpha_u = alpha_p = 1`, BDF2, linear upwind without a limiter, solvers 1e-14 / 1e-12 / 5000, to `t = 1` with `dt` 0.1,
0.05, 0.025 against `dt_ref = 1/640` on the same mesh. Measured there: OFF `p_f` 1.0151, central 1.0118, `n_outer` 20
1.0151, the extrapolated periodic partner 1.0151, implicit Euler 1.0474.

**H1, H-COMM: an `O(dt)` part of the converged face flux, spatial in origin, which a same-mesh reference does not cancel.**
Where: `pimple.rs` lines 2454 to 2461 (`r_au = V / a_P`, `gamma_p` its linear face value) and 2496
(`phiHbyA = face_flux(HbyA)`). With `u = HbyA - rAU G p`, the face defect `d = phi - interp(u) . S` carries
`C(G p) = interp(rAU G p) . S - interp(rAU) interp(G p) . S`, and `rAU = dt / (c0 + a dt)` varies from cell to cell by
`O(dt^2 (a_N - a_P))`, so `C(G p) / dt` is `O(dt h^2)` for a smooth field (ADR 0073, lines 75 to 87; its Y1 removed the floor
and its F-G brought the order to 2.0 to 2.3; ADR 0223 BF at N = 16, 32, 64: PISO 1.036, 1.627, 2.359, PIMPLE-4 1.005,
1.031, 2.010 for the pair 0.025, 0.0125, central convection). It is part of the converged discrete equations, so more
outer iterations do not touch it (ADR 0821 D-b), and it does not depend on the convection scheme (D-a). Predicted order:
the error is `e = A dt^2 + B h^2 dt` (`B` saturating once `a dt > c0`), so the pair's order is 1 where `B h^2 dt`
dominates and rises to 2 as `h^2 / dt` falls. With ADR 0223's numbers the floor's share at N = 64 is small enough for
2.0 on the pair 0.025, 0.0125; on the coarser pair 0.05, 0.025 of this case its share is half of that (`h^2 / dt` halves),
so `p_f` about 2.0 at N = 64. At N = 32, `p_f` between 1.2 and 1.8. Controls: (a) refinement, D-32 and the corrected case
O' at N = 64; (b) the mode F2 below, whose pressure is uniform (`G p = 0`, so `C(G p) = 0`): BDF2's order predicted;
(c) ADR 0245's opt-in `rhie_chow_step_consistent` (D-rc, `pimple.rs` lines 2457 to 2461 and 2522 to 2525), which removes
`C(G p)` on internal faces only, the coupled faces being left: predicted a smaller error and a higher `p_f` than OFF, not
necessarily 1.9.

**H2, the start.** Where: `pimple.rs` lines 3752 to 3760 (the first step of BDF2 is implicit Euler, `c1 = 1, c2 = 0`) and
1476 (the first face flux is the face value of the exact cell field, `d^0 = 0`). First principles: one Euler step leaves a
local error `O(dt^2)`, which does not change the global order 2. ADR 0245 ruled the start out on its set-up by measurement
(its note, line 625). Predicted: no `O(dt)` part. Control D-s: a BDF2 first step from two exact levels (the older level
the exact field at `t = -dt`, its face flux from `PimpleSolver::new` on that field, `dt_old = dt`, put in by
`snapshot` / `restore`). H2 is ruled out if `abs(p_f(D-s) - p_f(OFF)) <= 0.05`.

**H3, the time level of the pressure in the correction and the outer loop.** Where: `pimple.rs` the correctors from line
2486. Ruled out by ADR 0821's D-b (20 outer iterations equal 10 to five digits); no new arm.

**H4, the time level of the periodic partner and the coupled faces' transient term.** Where: `pimple.rs` lines 2464 to
2466 and 2512 to 2514 (`coupled_ddt_correction`, default off: the coupled faces carry no transient Rhie-Chow term). The
partner's time level is ruled out by ADR 0821's D-c. Control D-cp: `coupled_ddt_correction` on, reported.

**H5, the error norm's time.** The norm is taken at `t = 1` in every run (`sol.time` asserted within 1e-9 of 1) against a
reference at the same time on the same mesh: it cannot make an `O(dt)` error. No arm.

**The reading (fixed now).** H1 stands if `p_f(D-32) >= p_f(OFF at N = 16) + 0.2`, O2' (N = 64, below) `>= 1.9` and F2
`>= 1.9`. H2 is ruled out by the rule above. D-rc and D-cp are reported against their predictions, without a limit. A solver
defect found here is a finding with its file and line and the remedy as an option; no solver file is changed.

## 2. The wall-bounded control F2 (fixed now)

The mode of `fractional_step.rs` `mode`: `v = sin(pi x) exp(-nu pi^2 t)` on `box_2d(32, 32, 1, 1, false, false)`, walls
fixed zero on `xmin`, `xmax`, zero gradient on `ymin`, `ymax` with the pressure fixed 0 there, `nu = 0.1`, the start
`(0, sin(pi x), 0)`, built on the pimple step with the settings of O (OFF): `piso(0.1)`, `n_outer` 10, `alpha` 1, BDF2,
linear upwind, the solvers of O. `dt` 0.1, 0.05, 0.025 to `t = 1` against `dt_ref = 1/640`; `e` the relative L2 of the
in-plane velocity as in O.

* **F2**: `p_f >= 1.9`. Source: `docs/process/tolerances.md`, "Observed order of accuracy". **Estimate**: about 2.0
  (`f2c` 2.07 on the fractional step; ADR 0223 BO O5, MODE PISO BDF2, 2.016 and 2.070). If missed, the solver's BDF2 order
  itself is in question: O' then does not decide either way and check H is not run.

The setting cannot act on the mode (its deferred correction `grad(v) . (x_f - x_U)` is zero: `grad v` is along x, `x_f -
x_U` along y on the faces that carry flux), so F2 has no ON arm.

## 3. The set-up check R0 and the diagnosis D (fixed now)

* **R0** (set-up): the harness of this file at N = 16, OFF, reproduces ADR 0821's OFF errors (5.1468e-4, 2.2644e-4,
  1.1204e-4): each relative difference `<= 1e-4` (the five printed digits; "Same discrete problem by two routes" widened to
  the printed digits, as ADR 0767 S1 did).
* **D** (a record of the readings of section 1, no limit beyond R0): the OFF arm with one change each, `e`, `p_c`, `p_f`
  printed: D-32 (N = 32); D-s (the exact BDF2 start, N = 16); D-rc (`rhie_chow_step_consistent`, N = 16); D-cp
  (`coupled_ddt_correction`, N = 16).

## 4. The corrected order check O' (fixed now)

ADR 0821's O with one change: N = 64 (`box_2d(64, 64, 2 pi, 2 pi, true, true)`), chosen now by H1 (the floor's share falls
as `h^2`). Arms OFF and ON (`outer_frozen_convection_gradient`), each against its own reference `dt_ref = 1/640`.

* **O1'**: `p_f(ON) >= 1.9`.
* **O2'** (the control): `p_f(OFF) >= 1.9`. If O2' is missed, O' does not decide.
* **O3'** (non-vacuity): the relative L2 difference of `u_ON` and `u_OFF` at `dt` 0.05, `t = 1`, `> 1e-9`. If not, O' does
  not decide.
* Beside: `p_c` of both arms, every `e`, `e_ON / e_OFF`, `abs(p_f(ON) - p_f(OFF))`.

**Limit and source**: as ADR 0821's O1, O2: `docs/process/tolerances.md`, "Observed order of accuracy" (BDF2's 2 less 0.1).
**Estimate**: `p_f` about 2.0 in both arms (section 1, H1); the reference leaves 1/256 of `e(0.025)` (ADR 0821 section 2).
Margin 0.1 in order, thin: a miss is recorded as a miss. **Cost**: 4 096 cells, 1 420 steps of 10 outer iterations and
two correctors, estimated 3 to 10 minutes on the shared machine: a heavy record (`#[ignore]`), run once. Its fast test on
the same path is OF: the same harness at N = 16, `dt` 0.05 to `t = 1`, ON against OFF, relative L2 `> 1e-9` (as O3 of ADR
0821, which measured 4.15e-4 there).

## 5. Check H (fixed now)

As ADR 0821 section 5 registered it, unchanged: B2, H(dt) 4.605e-5 s 5 498 steps and H(dt/2) 2.3025e-5 s 10 996 steps,
the window above 0.1266 s, the six site cells, A_STEADY 0.193 m/s, the classes of ADR 0183 in their order, the verdict of
F-879 with persistence 0.75, no wall-clock cap, both records on, `n_outer` 5, the outer rule of E-1 of
[ADR 0767](0767-f885-registration-check-h-by-the-velocity-change-of-the-outer-iterations.md) as written, the opt-ins
`alpha_free_face_flux`, `outer_frozen_limiter` and `outer_frozen_convection_gradient`. The case, the measures, `class_0183`,
`verdict`, `run_h`, the record form and C5 are copied unchanged from `f885_frozen_convection_check_h.rs` into the new test
file; the records go to `docs/investigations/f885-order-case/` (`H_dt.csv`, `H_dt2.csv`, their logs).

**Run only if O' holds** (O1', O2', O3' met) **and F2 is met.** Both runs started together, one process each, in the
background; no cap; stop rule 4 of ADR 0183 holds. Estimate (ADR 0821 section 5): about 1.6 h and 3 h; `d_5 / d_1` about
3e-3, margin 3.3; STEADY. If check H is not run, SR and G1 are marked `#[ignore = "not run: ..."]` in the same commit as the
results and recorded as not run (their files cannot exist); that is fixed now, not an amendment.

## Table of checks

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A3 | check H with the opt-ins: H(dt) completes 5 498 steps and H(dt/2) 10 996, no failure, no guard trip, every field finite | both complete | acceptance of F-885; ADR 0183 D5; ADR 0767 A3 | completes | n/a | solution verification |
| A4 | each run TIME-ACCURATE by E-1 of ADR 0767 | at least 99 % of the window's steps with `d_5 <= 1e-2 d_1` | acceptance of F-885; ADR 0183, check H, the outer-iteration rule | `d_5 / d_1` about 3e-3 | 3.3 | solution verification |
| A5 | the class by ADR 0183's rule and the verdict of F-879 by ADR 0766's table | one of the four, by the registered numbers of ADR 0767 A5 | acceptance of F-885; ADR 0183; ADR 0766 | STEADY | n/a | solution verification |
| A6 | a run that does not decide is recorded as not deciding | as stated | acceptance of F-885 (last clause) | as stated | n/a | solution verification |
| O1' | TG at N = 64, BDF2, the setting on: `p_f` | `>= 1.9` | `docs/process/tolerances.md`, "Observed order of accuracy" | 2.0 | 0.1 in order | solution verification |
| O2' | TG at N = 64, the setting off (control): `p_f` | `>= 1.9` | as O1' | 2.0 | 0.1 in order | solution verification |
| O3' | non-vacuity at N = 64, `dt` 0.05 | relative L2 `> 1e-9` | ADR 0821 O3 (the setting must act on the case) | order 1e-4 | n/a | code verification |
| OF | the fast test of O' (N = 16, `dt` 0.05, ON against OFF) | relative L2 `> 1e-9` | as O3' | 4.2e-4 | n/a | code verification |
| F2 | the wall-bounded mode on the pimple step, OFF: `p_f` | `>= 1.9` | `docs/process/tolerances.md`, "Observed order of accuracy" | 2.0 | 0.1 in order | solution verification |
| R0 | the harness at N = 16, OFF, against ADR 0821's three OFF errors | relative `<= 1e-4` each | "Same discrete problem by two routes", widened to the five printed digits | 0 | n/a | code verification |
| D | the diagnosis arms D-32, D-s, D-rc, D-cp and the reading of section 1 | recorded; the reading by its rule | section 1 | H1 stands, H2 ruled out | n/a | solution verification |
| SR | 20 steps of the check H path against the first 20 rows of `H_dt.csv` | relative `<= 1e-6`; finite; `d_1 > 0` | as ADR 0821 SR | 0 | n/a | code verification |
| C5 | the rule of E-1 on built records (ADR 0767 C5, copied) | each as built | ADR 0767 E-1 | as built | n/a | code verification |
| G1 | the class and verdict from the committed `H_dt.csv` and `H_dt2.csv` as recorded | counts exact; class and verdict equal; 1e-6 relative | as ADR 0821 G1 | 0 | n/a | unchanged default output |
| B3 | `cargo test -p rcfd-api --release --test baseline_hashes` passes, `baseline_hashes.rs` unchanged | passes, file unchanged | rule 3; the brief | passes | n/a | unchanged default output |
| D1 | no file under `crates/*/src` changes; `docs/roadmap.json`, the ledger, every golden and every existing test file unchanged; the eight committed F-885 patches apply | as stated | the brief, owned files; rule 3 | as stated | n/a | unchanged default output |
| P | `cargo test -p rcfd-api --release --lib`; the new file's gate tests; `--test f885_frozen_convection_check_h` and `--test f885_alpha_free_face_flux` (the tests of the harness copied and of the opt-ins); `rustfmt --check` of the new file (for `cargo fmt -p rcfd-api --check`, which refuses here with os error 206); `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

No row is of kind validation: F-885 claims no agreement with a measured case.

## Tests (fixed now)

All in `crates/rcfd-api/tests/f885_order_case.rs` (header: ADR 0822, F-885). Gate tests: `f885_oc_setting_acts` (OF),
`f885_oc_wall_bounded_mode` (F2), `f885_oc_rules_on_built_records` (C5), `f885_oc_short_run_on_the_check_h_path` (SR),
`f885_oc_class_from_the_committed_records` (G1; `RECORD` written after the runs). Records, `#[ignore]`, run once by me:
`f885_oc_diagnosis_record` (R0 asserted, D printed; seconds to a minute), `f885_oc_time_order_record` (O', heavy),
`f885_oc_h_dt_record`, `f885_oc_h_dt2_record` (check H, heavy, writing to `F885_OC_OUT`). The fast tests on the heavy
paths: OF for O', SR for check H.

## Negative controls and planned mutations

On a scratch copy of the full commit (`git archive` into the scratch folder, its own target folder), each against the
named test; the check H records are not rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: `outer_frozen_convection_gradient` not read | `f885_oc_setting_acts` (OF) |
| M2 | solver: the kept gradient formed from `cells` (`u^n`) instead of `conv_cells` | `f885_oc_time_order_record` (O1') |
| M3 | solver: `ddt_history_weights` returns `c2` times 0.9 | `f885_oc_wall_bounded_mode` (F2) |
| M4 | test: the ratio of E-1 1e-1 for 1e-2 | `f885_oc_rules_on_built_records` (C5) |
| M5 | record: `H_dt.csv` cut to its first half | `f885_oc_class_from_the_committed_records` (G1); not run if check H is not run |

The committed patch (`crates/rcfd-api/tests/f885_order_case.mutation.patch`) is M1, first line
`# expect-fail: cargo test -p rcfd-api --release --test f885_order_case f885_oc_setting_acts`.

## Decision rule

* **Done** for F-885 if O' (O1', O2', O3'), F2, R0, OF, A3 and A4 in both runs, A5 with the verdict steady, physically
  unsteady or numerical, SR, C5, G1, B3, D1 and P are met, and each of M1 to M5 is caught.
* **Partial** otherwise, with the reason by the rule: O' or F2 missed or not deciding (then check H is not run); check H run
  and not deciding; a row or a mutation missed.
* **Stopped and reported** if a check needs a change of the solver or check H cannot be run as registered on this base.

A defective set-up found before a registered run is corrected by an amendment committed before the corrected run.

## What is not covered

* A remedy of H-COMM in the solver (F-G of ADR 0073 was not accepted; ADR 0245's opt-in leaves the coupled faces): a
  finding at most, an option in the open questions.
* The setting as a default or a case key; its order with the limiter or variable steps; arm 1, arm 2 and D9 of ADR 0183;
  any agreement with a measurement.

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation: the remedy of a solver defect if the diagnosis names
one, and any question of scope.

## Amendment 1 (2026-11-27): M3 corrected, before any code commit and before any run

Written before the test file is committed and before any test or mutation of this registration is run. Nothing above is
changed. M3 as registered (`ddt_history_weights` returning `c2` times 0.9, `pimple.rs` lines 3752 to 3760) cannot change
what F2 reads: those weights feed only the transient Rhie-Chow carry-over of the old face defects (lines 1888 to 1910),
and on the mode the pressure is uniform and the discrete divergence of `(0, v(x))` is zero, so the defects are zero to
rounding. The brief asks that a mutation be able to change what its test reads. **M3 is now**: `crates/rcfd-fv/src/ddt.rs`
line 76, the BDF2 cell weight `c2` of the time derivative times 0.9 (`0.9 * r * r / (1.0 + r)`), which every BDF2 step of
the mode reads; caught by `f885_oc_wall_bounded_mode` (F2). The other mutations, the committed patch (M1) and every limit
are unchanged.

## Addendum 1: results

Written 2026-11-27, after the runs. Commits on the branch, from efce0cf1: 654b2364 (this registration, alone), debebd72
(amendment 1, alone, before any code or run), 1da1fd85 (the test file; every registered run was made from it), e102244f
(O' as a known miss, SR and G1 marked not run, the records, the mutation log, the committed mutation M1), and the commit of
this addendum with the rule 7 log and the proposal file. Records in `docs/investigations/f885-order-case/`: `gate.log`,
`diagnosis.log`, `order.log`, `mutations.log`, `rule7.log`. No file under `crates/*/src` changed.

### The sentence

**The first order of ADR 0821's case is the Rhie-Chow commutator floor of ADR 0073 (H-COMM), not the remedy, and on the
corrected case the remedy misses BDF2's order, so check H was not run.** On the Taylor-Green set-up without the setting
`p_f` is 1.015 at N = 16, 1.452 at 32 and 2.035 at 64; at N = 16 the step-consistent face coefficient of ADR 0245 alone
gives 2.022; the wall-bounded mode, whose pressure is uniform, gives 2.032; the exact BDF2 start leaves the order unchanged
(1.064). On the corrected case (N = 64) the control O2' holds (2.035) and O3' acts (4.1e-5), but with the setting on `p_f`
is 1.822 against 1.9: O1' is missed and `e_ON / e_OFF` rises from 1.36 to 1.66 as `dt` falls, a lag of lower order than
BDF2's that the argument of ADR 0821 section 1 did not foresee. By the registered rule F-885 stays **partial**.

### Table of checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| A3 to A6 | as registered | check H not run (section 5: it runs only if O' holds and F2 is met) | not run |
| O1' | `p_f(ON) >= 1.9` | 1.8222 (`e` 3.7568e-4, 9.7105e-5, 2.7461e-5; `p_c` 1.9519) | **no** |
| O2' | `p_f(OFF) >= 1.9` | 2.0345 (`e` 2.7615e-4, 6.7885e-5, 1.6570e-5; `p_c` 2.0243) | yes |
| O3' | `> 1e-9` | 4.1083e-5 | yes |
| OF | `> 1e-9` | 4.1524e-4 | yes |
| F2 | `p_f >= 1.9` | 2.0320 (`e` 4.4497e-3, 1.0603e-3, 2.5926e-4; `p_c` 2.0693) | yes |
| R0 | relative `<= 1e-4` each | 3.4e-6, 1.5e-5, 3.3e-5 | yes |
| D | recorded; the reading by its rule | H1 stands, H2 ruled out (below) | recorded |
| SR | as registered | not run (no record of check H; section 5) | not run |
| C5 | each as built | each of the seven as built | yes |
| G1 | as registered | not run (no record of check H; section 5) | not run |
| B3 | passes, file unchanged | `baseline_hashes`: 3 passed, 1 ignored; `baseline_hashes.rs` unchanged | yes |
| D1 | as stated | `git diff --name-only efce0cf1` names this ADR, the new test file, its patch, the five logs and the proposal file; nothing under `crates/*/src`, no roadmap, ledger, golden or existing test file; the eight earlier F-885 patches and the new one apply | yes |
| P | rule 7 | see "The checks of rule 7" | yes |

Beside, without a limit: `e_ON / e_OFF` at N = 64 1.3604, 1.4304, 1.6572; `abs(p_f(ON) - p_f(OFF))` 0.2123. Wall time:
the three gate tests 17.1 s, the diagnosis 69.8 s, O' 158.5 s, on the shared machine.

### The diagnosis (section 1)

| Arm | `e` at 0.1, 0.05, 0.025 | `p_c` | `p_f` |
| --- | --- | --- | --- |
| R0, N = 16 (ADR 0821's OFF) | 5.1468e-4, 2.2644e-4, 1.1204e-4 | 1.1846 | 1.0151 |
| D-32, N = 32 | 2.7200e-4, 7.1771e-5, 2.6230e-5 | 1.9222 | 1.4522 |
| O2', N = 64 | 2.7615e-4, 6.7885e-5, 1.6570e-5 | 2.0243 | 2.0345 |
| D-s, the exact BDF2 start, N = 16 | 9.7210e-4, 4.6515e-4, 2.2248e-4 | 1.0634 | 1.0640 |
| D-rc, `rhie_chow_step_consistent`, N = 16 | 3.8292e-4, 9.3308e-5, 2.2976e-5 | 2.0370 | 2.0219 |
| D-cp, `coupled_ddt_correction`, N = 16 | 5.1468e-4, 2.2644e-4, 1.1204e-4 | 1.1846 | 1.0151 |
| F2, the wall-bounded mode, N = 32 | 4.4497e-3, 1.0603e-3, 2.5926e-4 | 2.0693 | 2.0320 |

By the registered reading: **H1 (H-COMM) stands**: `p_f(D-32) - p_f(16)` = 0.437 (>= 0.2), O2' 2.035 and F2 2.032 (>= 1.9).
**H2 is ruled out**: `abs(p_f(D-s) - p_f(16))` = 0.0489 (<= 0.05, by 0.001; the exact start changes the error by a factor
of about 2 but not its order). H3 and H5 as registered (no arm). H4: D-cp equals OFF to the printed digits, so the coupled
faces' transient term is not the cause. D-rc goes beyond its prediction (registered: higher than OFF, not necessarily 1.9):
at N = 16 removing `C(G p)` on the internal faces alone restores the order, so on this box the coupled faces do not carry a
floor of their own.

**The finding (no solver file changed).** The default face flux of the pimple step (`pimple.rs` lines 2454 to 2461, the
linear face value of `rAU`, and 2496, `phiHbyA` from the cell `HbyA`) carries ADR 0073's commutator `C(G p)`, an `O(dt h^2)`
term of the converged equations: on a mesh as coarse as N = 16 it dominates BDF2's error at these steps, so every default
BDF2 order check on such a mesh with a pressure gradient reads first order. It is a known property (ADR 0073, ADR 0245), not
a new defect; ADR 0245's opt-in `rhie_chow_step_consistent` removes it on internal faces. Remedy options are question 2.

**The remedy's miss, not diagnosed.** With the setting on at N = 64, `e_ON / e_OFF` grows as `dt` falls (1.36, 1.43, 1.66),
which a lag of order `dt^2` would not do: the kept gradient adds a part of lower order. Its source is not shown by a run
of this registration. Read from the code only: the kept gradient is formed in the first outer iteration from `conv_cells`
(the extrapolated cells) but with `bv`, the boundary values of the current iterate (`pimple.rs` lines 2191 to 2200), so on
the periodic seam its partner values lag by `O(dt)`, as ADR 0073's H-SEAM-CONV found for the convection's partner value;
M2 (the cells unextrapolated as well) moves `p_f(ON)` to 1.175, so the extrapolation is read and most of the order kept.

### Mutations

From `mutations.log` (`git archive` of 1da1fd85 in the scratch folder, its own target folder):

| Id | What happened |
| --- | --- |
| K0 | unmutated: `f885_oc_setting_acts`, `f885_oc_wall_bounded_mode`, `f885_oc_rules_on_built_records` pass |
| M1 | the setting not read: caught by `f885_oc_setting_acts` (OF 0 against 1e-9). Committed as the mutation patch |
| M2 | the kept gradient from `u^n`: **not caught (the test fails unmutated)**. It changed what the test reads: `p_f(ON)` 1.1753, `e_ON / e_OFF` 3.35, 5.73, 10.39 |
| M3 | the BDF2 cell weight `c2` times 0.9 (amendment 1): caught by `f885_oc_wall_bounded_mode` (the mode's run stopped on the divergence guard) |
| M4 | the ratio of E-1 1e-1: caught by `f885_oc_rules_on_built_records` ("d5/d1 = 0.05 everywhere" got Steady) |
| M5 | not run (no `H_dt.csv`; check H not run) |

Three of five caught, one not caught, one not run.

### Deviations

1. Amendment 1 (debebd72) replaced M3 before any code or run, because the registered M3 could not change what F2 reads.
2. O1' was missed. By rule 2 `f885_oc_time_order_record` became the ignored record `f885_oc_time_order_known_miss`, its
   limits intact and the measured numbers in its ignore reason; SR and G1 were marked not run as section 5 fixed in advance.
   No other test change.
3. The mutation script failed twice before running a mutation (a `tar` path and a Python 3.9 call); K0 ran in the second
   attempt and M1 to M4 in the third, each once. No limit or test changed.
4. `cargo fmt -p rcfd-api --check`: exit 0 on this run (it ran). `rustfmt --check --edition 2021` of the new file is the registered replacement.
5. The new file's gate tests were not run again for rule 7: their registered run (`gate.log`, at 1da1fd85) is the run; the
   later commit only adds ignore attributes to other tests.

### What was not run

Check H (A3 to A6), SR, G1, M5; a diagnosis of the remedy's miss; the whole workspace suite.

### The checks of rule 7

From `rule7.log`, at e102244f: `cargo fmt -p rcfd-api --check` exit 0 (it ran on this machine this time); `rustfmt --check` of the new file clean; `cargo clippy -p rcfd-api --all-targets -- -D warnings` clean; `cargo test -p rcfd-api --release --lib` 160 passed, 4 ignored; `--test f885_frozen_convection_check_h` 3 passed, 6 ignored; `--test f885_alpha_free_face_flux` 5 passed, 3 ignored; `--test baseline_hashes` 3 passed, 1 ignored (`baseline_hashes.rs` unchanged); `cargo test -p rcfd-cli --release --test docs_site` 2 passed; `python scripts/roadmap_proposals.py check` of the proposal file exit 0; the nine F-885 patches apply. `python scripts/process_report.py --check` run at the commit of this addendum, as reported to the lead.

### Size

Size (agent): partial; 1; 1; 2; 170000

### Decision by the registered rule

**Partial.** O1' is missed (1.822 against 1.9), so O' does not hold and check H was not run; M2 is not caught and M5, SR,
G1 were not run. O2', O3', OF, F2, R0, C5, D1 are met, and the diagnosis names the first order of ADR 0821's case
(H-COMM). What it would take: a diagnosis of the remedy's lag on the N = 64 case, registered under F-885 (rule 13), the
remedy's order met, then check H.

## Open questions for the maintainer

1. **The remedy's order.** Options: (a) a diagnosis under F-885 of the lag of the kept gradient, with arms fixed before the
   run: the boundary values of the kept gradient extrapolated as the cells are (`periodic_partner_extrapolated` with the
   setting, and a scratch variant of `bv`), on the N = 64 case and on N = 16 with `rhie_chow_step_consistent` (2.02 there
   without the setting, a cheaper order case); then O again; (b) accept `p_f` 1.82 against 1.9 and run check H, which changes
   the registered rule and is the maintainer's to make; (c) drop this remedy and look for another cause of the stalled
   outer loop. I recommend (a): the miss is small, has a likely source in two lines, and the N = 16 case with
   `rhie_chow_step_consistent` makes each order check take seconds.
2. **The commutator floor of the default step (a known property, not new).** Options: (a) keep it as it is, recorded,
   and use `rhie_chow_step_consistent` or a fine mesh in order checks; (b) a new item to make `rhie_chow_step_consistent` the
   default of the pimple step, which changes every hash of a pimple run. I recommend (a) for F-885; (b) is a question of
   scope for the maintainer, not of this item.
3. **Scope.** No new item proposed; the diagnosis of question 1 adds no acceptance clause and is registered under F-885.

## For the lead: roadmap entries

> Status proposed for F-885: partial.

> Note entry: ADR 0822, 2026-11-27, partial: the first order of ADR 0821's Taylor-Green case is the O(dt h^2) Rhie-Chow commutator floor of ADR 0073 (H-COMM; p_f 1.015 at N = 16, 1.452 at 32, 2.035 at 64; 2.022 at N = 16 with rhie_chow_step_consistent; the wall-bounded mode 2.032; the start ruled out), but on the corrected case at N = 64 the remedy outer_frozen_convection_gradient misses BDF2's order (p_f 1.822 against 1.9, the control 2.035), so check H was not run by the registered rule; held by crates/rcfd-api/tests/f885_order_case.rs and docs/investigations/f885-order-case.

> Gap entry: Missing for done: a class by ADR 0183's rule. The order case now decides (N = 64, control p_f 2.035; ADR 0821's N = 16 case is held by the Rhie-Chow commutator floor), but the remedy outer_frozen_convection_gradient is below BDF2's order on it (p_f 1.822 against 1.9, e_ON / e_OFF rising 1.36 to 1.66 as dt falls). Needs a diagnosis of that lag, the remedy's order met, then check H with the remedy.

The same entries are in `docs/roadmap/proposals/0822-F-885.json` (form 1 of ADR 0755, base efce0cf1), checked with
`python scripts/roadmap_proposals.py check`. No follow-up item is proposed (question 3).

Generated pages that this work may leave not current, for the lead (rule 9): `docs/adr/index.md` (ADR 0822 is new).

## Answers of the maintainer (2026-11-26), written by the lead

1. Option (a): a diagnosis under F-885 of the kept gradient's lag, with its arms fixed in advance, ADR 0824; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
2. Option (a): the commutator floor of the default step stays recorded; order checks use the opt-in `rhie_chow_step_consistent` or a finer mesh; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
3. The recommendation: no new item; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
