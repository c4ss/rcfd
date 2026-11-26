# ADR 0810: F-885 registration: why the relaxed and the unrelaxed outer iterations of a pimple step have different fixed points

Status: **REGISTERED 2026-11-26, written before any code of F-885 for this registration and before any run of its checks.**
Before this text no library code, test, harness or solver run of this work was written or made. What was done: reading
(below) and the algebra of this page. Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f885-outer-fixed-points.md`](briefs/f885-outer-fixed-points.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. A further registration of F-885 under rule 13 (a
diagnosis of the miss of [ADR 0767](0767-f885-registration-check-h-by-the-velocity-change-of-the-outer-iterations.md)),
taken as option (a) of its question 1 by the lead's decision of 2026-11-25. Branch `f885-outer-fixed-points` from main
f42518a3; scratch `w49i-work/f885-outer-fixed-points-scratch`. Written by the model Claude Opus 5.5.

Size (agent): done; 1; 1; 1; 230000

## What is asked

The acceptance of F-885 is unchanged and is not met by this work (check H still has to decide). The brief asks for a
diagnosis: where and why the relaxed outer iterations (1 to `N - 1`) and the unrelaxed final one (`N`) of a pimple step have
different fixed points, on the cavity of ADR 0767 and on 20 steps of H(dt), with no long run; the hypotheses from the code
with file and line, the controls that separate them and the numbers each control decides by, fixed now. The remedy is a
registration of its own.

What ADR 0767 measured (addendum 1): in every window step of both runs of check H, `d_5 / d_1` 0.93 (dt) and 0.96 (dt/2),
the medians of `d_5 / d_4` 6.70 and 19.4 while `d_2 / d_1` to `d_4 / d_3` stay at or below about 1; on the cavity `d_5 / d_4`
4 to 15 (step 1: 3.4197e-2, 7.1717e-3, 1.5369e-3, 3.3740e-4, 1.4550e-3 m/s); on the first step of H(dt) 36.09, 26.05,
4.316, 1.340, 0.916 m/s (S2).

## What was read

Whole: `AGENTS.md`; `docs/process/agent-rules.md` (version 5); the brief; ADR 0767 (registration, addenda 1 and 2, the
lead's reading, the lead's decisions of 2026-11-25); `crates/rcfd-api/tests/f885_check_h_outer_record.rs`.

In part: `crates/rcfd-fv/src/pimple.rs` lines 150 to 275 (the settings of the outer records, `OuterRecord`,
`OuterVelocity`, `max_velocity_change`), 1100 to 1118 (`PimpleSettings::pimple`), 1680 to 2640 (the whole of
`step_with`: the outer loop, the predictor, the relaxation, the correctors, the end of the step), 3050 to 3070 and 3100 to
3106 (`PeriodicLinks::relax` and `h_operator`); `crates/rcfd-linalg/src/matrix.rs` lines 86 to 95 (`relax`);
`crates/rcfd-fv/src/fvmatrix.rs` lines 50 to 52; `crates/rcfd-fv/src/pressure.rs` lines 631 to 661 (`corrected_flux`);
`crates/rcfd-api/src/sim.rs` lines 2100 to 2125 (the transient settings from the case), 2395 to 2410 (the rotating frame
and `coupled_body_pair_mean`), 4395 to 4425 (the hook of the transient step: sources, turbulence); `crates/rcfd-api/src/schema.rs`
lines 870 to 886 (`alpha_u` default 0.9, `alpha_p` default 1.0), the defaults of `component_diagonal` (`averaged`),
`n_correctors` (2) and `ddt_correction` (true); [ADR 0062](0062-wave31a-solver-defects.md) lines 150 to 158;
[ADR 0245](0245-f344-registration-bdf2-step-remainder.md) lines 30 to 40; `docs/process/tolerances.md`, the table;
`scripts/process_report.py`, the checks of registrations and citations.

The settings of the two cases. The cavity of ADR 0767 is built on `PimpleSettings::pimple(0.01, 5)`: `alpha_u = 0.8`,
`alpha_p = 0.5`, one corrector, BDF2, `ddt_correction` of `piso` (true). The fan case of check H takes the case defaults:
`alpha_u = 0.9`, `alpha_p = 1.0`, `n_correctors = 2`, `ddt_correction = true`, `component_diagonal` averaged, and the
rotating frame as the hook's source (Coriolis and centrifugal parts from the iterate, `sim.rs` 4400 to 4405) with
`coupled_body_pair_mean` on. So on the fan only the velocity is relaxed; ADR 0767's "alpha_p of the case default" is 1.

No published source outside the repository was read for this registration. The derivation below is the code's own
algebra. The repository already records the effect it leads to: ADR 0062 (line 156) measured a steady SIMPLE answer that
"depends on alpha_u ... the known relaxation dependence of the Rhie-Chow face flux", and ADR 0245 (lines 33 to 36) names
the literature on it, cited there from memory (S. Majumdar, Numer. Heat Transfer 13 (1988) 125-132); that paper was not
read for this work either, and nothing here rests on it.

## 1. The final iteration in the code

`step_with` (`crates/rcfd-fv/src/pimple.rs`) runs `for outer in 0..s.n_outer` (line 1816). The final iteration differs
from the others in these places only:

* line 1817 to 1818: `last` sets `(alpha_u, alpha_p) = (1.0, 1.0)`, else the settings' factors;
* line 2074 to 2080: the momentum matrix of each component is relaxed only when `alpha_u < 1`;
* line 2512: the pressure of the last corrector is relaxed by `alpha_p` (1 in the final iteration);
* lines 2162, 2170, 2188: the pressure-lag context and the adaptive corrector count, read only with
  `pressure_lag_check` and `corrector_control`, which neither case sets;
* line 2525: the loop ends.

Line 1911: the first iteration of a BDF2 step (not the final one) extrapolates the convecting flux and the
deferred-correction state in time. That makes the first iteration solve different equations from the 2nd to the `N`-th;
it cannot make the final one differ from the 2nd to the `(N - 1)`-th, and is not a hypothesis for the jump.

## 2. The algebra of a relaxed outer iteration (fixed now)

For one component and cell `c`: the momentum matrix before relaxation has the diagonal `a_c` (with the time term), the
off-diagonal coefficients `a_nb` and the source `b_c`. `relax` (`rcfd-linalg/src/matrix.rs` 86 to 95; with the periodic
links `pimple.rs` 3053 to 3066) takes `d = max(a_c, sum |a_nb|)` and sets the diagonal to `d / alpha` and the source to
`b_c + (d / alpha - a_c) u*_c`, with `u*` the iterate at the start of the outer iteration (`cells`, line 2076). With a time
term the diagonal dominates, so `d = a_c`. The relaxed matrix is kept in `mats[k]` (line 2110), its diagonal in `a_p`
(line 2101) and so in

    r_au = V / a_p           (line 2207),   a_p = mean over the active components of a_c / alpha,

so `r_au` of a relaxed iteration is `alpha` times that of the final one. The correctors form `H = b' - sum a_nb u_nb` from
the relaxed matrix (`h_operator`, line 2230), `HbyA = H / a_p` (2235 to 2238) and

* the face flux `phi = interp(HbyA) . S + (gamma_p / dt) D + gamma_p b_f - gamma_p |S| dp/dn` (lines 2244, 2247, 2264 to
  2279, 2455), with `gamma_p = interp(r_au)` (2211 to 2215) and `D` the old-level flux defect (1765 to 1785);
* the cell velocity `u = HbyA + r_au b - r_au grad p` (2490 to 2498).

**The cell momentum at a fixed point does not depend on alpha.** At a fixed point of the relaxed iteration, `u* = u` and
the corrector leaves `u` and `p` unchanged. Then `(a_c / alpha) u = b + (a_c / alpha - a_c) u - sum a_nb u_nb - V grad p +
V b_cell`, that is `a_c u = b - sum a_nb u_nb - V grad p + V b_cell`: the unrelaxed equation. This holds where the
components' diagonals are equal (then `a_p = a_c / alpha` for each component); where they differ, the averaged `a_p`
leaves `(a_p - a_c,k / alpha + a_c,k) u_k` on the left, which depends on alpha (hypothesis H6).

**The face flux at a fixed point does depend on alpha.** Substituting `HbyA = u + r_au grad p - r_au b_cell` into the face
flux gives `phi = interp(u) . S + R(alpha)` with

    R(alpha) = interp(r_au grad p - r_au b_cell) . S - gamma_p |S| dp/dn + gamma_p b_f + (gamma_p / dt) D,

every part of which carries `r_au` or `gamma_p`, so `R(alpha) = alpha R(1)` at the same state (with `d = a_c`). Discrete
continuity `sum_f phi = 0` is then a different equation for each alpha: the relaxed iterations converge (if they do) to a
state `F(alpha)` and the unrelaxed one to `F(1)`. The unrelaxed final iteration, started from `F(alpha)`, meets a mass
residual `(1 / alpha - 1) div R(alpha)` in every cell and moves the state towards `F(1)`; the next step's relaxed first
iteration moves it back towards `F(alpha)`. That is the pattern ADR 0767 recorded (`d_5` about `d_1`, both large, the step
change small). This is hypothesis H1.

**The size, to first order.** `F(alpha) - F(1)` is the response of the discrete problem to the flux perturbation `(1 - alpha)
R(1)`; for a smooth dependence it is linear in `s = 1 - alpha`, and so is the final iteration's change `d_N = |G_1(F(alpha))
- F(alpha)|` (`G_1` the unrelaxed outer map, the same map for both alpha). So `d_N(alpha = 0.95) / d_N(alpha = 0.9) = 0.5`
to first order in `s`; the neglected term is of relative order `s = 0.1`, and the band below allows twice that.

**The pressure relaxation.** `p = p_old + alpha_p (p_new - p_old)` (line 2517) acts after the flux (2455) and the velocity
(2496) were formed from `p_new`; at a fixed point `p_new = p_old`. By the algebra it changes neither fixed point, and in
the final iteration it touches only the stored pressure, not the velocity of that iteration. Hypothesis H2 says the
algebra is wrong here; the control is cheap.

## 3. The hypotheses (fixed now)

| Id | Hypothesis | Code |
| --- | --- | --- |
| H1 | the relaxed `r_au` enters the face flux (the face coefficient `gamma_p`, the transient term, the body-force terms) and does not cancel at a fixed point, so continuity depends on `alpha_u` | `pimple.rs` 2101, 2207, 2211 to 2215, 2247, 2264 to 2279, 2455 |
| H2 | the pressure relaxation leaves a relaxed term in the flux or the velocity | `pimple.rs` 2512, 2517 |
| H6 | the averaged momentum diagonal (`component_diagonal` averaged) leaves an alpha-dependent cell momentum where the components' diagonals differ | `pimple.rs` 2139 to 2141, 2235 to 2238 |
| Hc | something other than the relaxation factors makes the final iteration solve different equations | `pimple.rs` 1817, 2162, 2170, 2188, 2525 |
| H4 | the relaxed outer iterations have no fixed point to converge to (they stall or cycle), and the final iteration's change is part of that | the loop as a whole |

H6 is zero on the cavity by construction: its two active components have the same matrix (the same walls, all fixed
zero, the same convection, diffusion and time term), so H6 is tested on the fan only.

## 4. The instrumentation (fixed now)

One addition to the record of ADR 0767, in `crates/rcfd-fv/src/pimple.rs` only: `OuterVelocity` gains `cell: Vec<usize>`,
the cell of the largest change of each outer iteration (the first such cell on a tie; cell 0 when every change is zero),
formed in the same pass that forms `d_k` (line 2523), with the value `d_k` formed as before. It is read only with
`outer_velocity_record`, as the record is, and enters no equation. No other source file changes. The controls set the
existing settings `alpha_u`, `alpha_p`, `n_outer`, `ddt_correction` and `component_diagonal` on the built solver (the
cavity on `PimpleSolver::new`, the fan through `Simulation::pimple_solver_mut`, as ADR 0767 did); no case key, no
default, no golden changes.

## 5. The controls (fixed now)

**Cavity** (the cavity of ADR 0767: `box_2d(8, 8, 1, 1)`, lid `(1, 0, 0)`, `nu = 0.01`, `dt = 0.05`, BDF2, from rest, 3
steps, the record on):

| Run | Settings | Separates |
| --- | --- | --- |
| K0 | as ADR 0767: `alpha_u = 0.8`, `alpha_p = 0.5`, `N = 5` | the baseline |
| K1 | `alpha_u = alpha_p = 1`, `N = 5` | H1, H2 against Hc |
| K2 | `alpha_u = 0.8`, `alpha_p = 1`, `N = 5` | H1 against H2 |
| K3 | `alpha_u = 1`, `alpha_p = 0.5`, `N = 5` | H2 against H1 |
| K4 | as K0 with `N = 20` | H4; the jump with converged relaxed iterations |
| K5 | as K0 with `ddt_correction = false` | the term: the transient part of `R` against the pressure part |
| K6a, K6b | `alpha_p = 1`, `N = 20`, `alpha_u = 0.9` and `0.95` | the size of H1, linear in `1 - alpha_u` |

**Fan** (20 steps of H(dt) on B2: the case text of ADR 0766 and ADR 0767, `h_text(4.605e-5)` copied unchanged, the
uniform start, both records of ADR 0767 E-2 on):

| Run | Settings | Separates |
| --- | --- | --- |
| F0 | the case: `alpha_u = 0.9`, `alpha_p = 1`, `N = 5` | the baseline |
| F1 | `alpha_u = 1` (so every iteration unrelaxed) | H1, H6 against Hc |
| F2 | `N = 20` | H4; the jump with more relaxed iterations |
| F3 | `component_diagonal = true` (the `corrected` split) | H6 against H1 |
| F4 | `ddt_correction = false` | the term, as K5 |

Each run is made once, in one test process per case; the fan runs one after the other in one heavy test (about 6 min on
the shared machine at 0.34 s an outer iteration from ADR 0767's 1.71 s a step: `20 (5 + 5 + 20 + 5 + 5) = 800`
iterations; no long run).

**The measures (fixed now).** For a run with `N` outer iterations, over its steps `S` (the cavity: steps 1 to 3; the fan:
steps 11 to 20, the second half, so the start from the uniform state is left out), the median is the upper median
(`x[len / 2]` of the sorted values, as ADR 0767's spread):

* `J` = median of `d_N / d_(N-1)`; **PRESENT** if `J >= 4`, **ABSENT** if `J <= 2`, else **UNDECIDED**; a NaN, a run that
  did not complete or a guard trip gives UNDECIDED;
* for `N = 20`: `C` = median of `d_19 / d_2`, **CONVERGES** if `C <= 1e-2`, **STALLS** if `C > 1e-2` (NaN: UNDECIDED); and
  `Q` = median of `d_20 / d_1`, reported, with H1's prediction `Q > 1e-2` (more outer iterations do not meet ADR 0183's
  rule);
* the cavity's size: `L` = median over the steps of `d_20(K6b) / d_20(K6a)`, **IN** if `0.4 <= L <= 0.6`.

**What each hypothesis predicts (fixed now).** The cavity names a hypothesis when its row holds, the fan likewise:

| Id | Cavity | Fan |
| --- | --- | --- |
| H1 | K0 PRESENT, K1 ABSENT, K2 PRESENT, K3 ABSENT, K4 CONVERGES and PRESENT, L IN | F0 PRESENT, F1 ABSENT, F2 PRESENT, F3 PRESENT |
| H2 | K0 PRESENT, K1 ABSENT, K2 ABSENT, K3 PRESENT | (not tested: `alpha_p = 1` on the fan) |
| H6 | (zero by construction) | F0 PRESENT, F1 ABSENT, F3 ABSENT |
| Hc | K0 PRESENT, K1 PRESENT | F0 PRESENT, F1 PRESENT |
| H4 | K4 STALLS | F2 STALLS (reported beside the cause, see the rule) |

The term (K5, F4): PRESENT says the pressure and body-force parts of `R` carry the jump; ABSENT says the transient part
`(gamma_p / dt) D` carries it; reported with `J`.

**The result.** The cause is the one hypothesis among H1, H2, H6 and Hc whose row holds on the cavity and on the fan (H2
and H6 by their one case). It is "all ruled out" if no row of the four holds on either case. Anything else (two rows, the
cases disagree, a baseline UNDECIDED) names none. H4 is reported beside: a STALLS says the relaxed loop does not converge,
which bears on the remedy but does not by itself explain a jump that H1 or H6 explains.

**The cell.** Reported beside, without a limit: for each run the cell of the largest change of the final iteration in
each step of `S`, its centre, and whether it is the cell of the largest change of the first iteration. H1 expects it where
`div R` is largest (on the cavity, the lid corners; on the fan, not predicted).

## Table of checks

There is no clause of the acceptance of F-885 that this work can meet (check H is not run); the brief's task is the
measure.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| B1 | the brief's decision: every control of section 5 ran as registered (K0 to K6b complete their 3 steps, F0 to F4 their 20 steps or stop with the stop recorded) | as stated | the brief, decision rule | all run | n/a | solution verification |
| B2 | the result names one hypothesis or rules all out, by the table of section 5 | as stated | the brief, decision rule; section 2 of this registration (the code's algebra) | H1 named | n/a | solution verification |
| B3 | no solver default changes: `cargo test -p rcfd-api --release --test baseline_hashes` passes, `baseline_hashes.rs` unchanged | passes, file unchanged | the brief; rule 3 | passes | n/a | unchanged default output |
| C0c | K0 step 1, `d_1` to `d_5`, against ADR 0767 A2 (3.4197e-2, 7.1717e-3, 1.5369e-3, 3.3740e-4, 1.4550e-3 m/s) | relative `<= 1e-4` | `docs/process/tolerances.md`, "Same discrete problem by two routes", widened to the 5 printed digits as ADR 0767 S1 widened | 0 | n/a | unchanged default output |
| C0f | F0 step 1, `d_1` to `d_5`, against ADR 0767 S2 (36.09, 26.05, 4.316, 1.340, 0.916 m/s) | relative `<= 1e-3` | as C0c, the 4 printed digits | 0 | n/a | unchanged default output |
| P1 | the cell of the record against route B (the largest change formed in the test from the hook's copies, ADR 0767 A2), every outer iteration of 3 cavity steps (K0) | equal index | "Same discrete problem by two routes" (equal where only rounding separates; the same operations) | equal | n/a | code verification |
| P2 | the record off on the cavity: `outer_velocity` empty; `u`, `p` and the flux after 3 steps equal in bits to the run with it on | equal bits | "Bit-identity of default output" | equal | n/a | unchanged default output |
| P3 | the existing tests of the record path, `cargo test -p rcfd-api --release --test f885_check_h_outer_record` (T5, C2, S1 among them) | pass, file unchanged | rule 2; ADR 0767 | pass | n/a | unchanged default output |
| P4 | the measures and the table of section 5 on built records: `J` 6, 1 and 3 give PRESENT, ABSENT, UNDECIDED; a NaN gives UNDECIDED; `C` 1e-3 and 0.1 give CONVERGES and STALLS; `L` 0.5 and 0.7 give IN and not; the outcome sets of each row give its hypothesis and a mixed set names none | each as built | this registration, section 5 | as built | n/a | code verification |
| P5 | the fan's class from the committed record of F0 to F4 (`docs/investigations/f885-outer-fixed-points/fan_controls.csv`): 5 runs, 20 steps each where completed, the outcomes and the result as recorded after the run | equal; `J`, `C` within 1e-6 relative | "Same discrete problem by two routes", widened to the printed digits (10) | equal | n/a | unchanged default output |
| D1 | the source change: only `crates/rcfd-fv/src/pimple.rs` among the files under `crates/*/src`; `docs/roadmap.json`, the ledger, every golden and every existing test file unchanged | as stated | the brief, owned files; rule 3 | as stated | n/a | unchanged default output |
| P | `cargo test -p rcfd-fv --release --lib`; the new test file; `cargo fmt -p rcfd-fv -p rcfd-api --check`; `cargo clippy -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

The decision numbers of the controls (4, 2, 1e-2, [0.4, 0.6]) are not limits of a pass or a fail; they class a run. Their
sources: 4 is the smallest `d_5 / d_4` ADR 0767 recorded on the cavity (addendum 1, "4 to 15"); 1 bounds the ratio of a
contracting iteration, and 2 is the geometric mean of 1 and 4, so that a run is classed only when it is nearer one of the
two; 1e-2 is ADR 0183's outer-iteration ratio, as ADR 0767 E-1 states it; the band of L is section 2 (0.5 to first order,
with twice the neglected relative term `s = 0.1`). No row is of kind validation: nothing here is compared with a
measurement.

## Tests (fixed now)

All in `crates/rcfd-api/tests/f885_outer_fixed_points.rs` (header: ADR 0810, F-885).

* Run by the gate: `f885_fp_cell_two_routes` (P1, P2); `f885_fp_rules_on_built_records` (P4);
  `f885_fp_cavity_controls` (K0 to K6b, C0c, the cavity's outcomes; well under a minute); `f885_fp_fan_from_the_committed_record`
  (P5).
* Heavy record (`#[ignore]`, run once by me): `f885_fp_fan_controls_record`, F0 to F4 with C0f, writing the record to the
  folder named by `F885_FP_OUT`; the fast test on the same path is ADR 0767's `f885_short_run_holds_the_state` (20 steps
  of H(dt) with the records) and the cavity controls.

The outcomes of the registered runs are written into the test file after them (`CAVITY_RECORD`, `FAN_RECORD`, as ADR 0767
G1 did); before that the cavity test prints its outcomes and asserts only C0c, and the fan test fails for want of the
record. The form and the tolerance are fixed here. The record files go to
`docs/investigations/f885-outer-fixed-points/`: `fan_controls.csv` (one row per run, step and outer iteration: the run,
the step, `k`, `d_k`, the cell), and the logs of the two registered runs.

## Negative controls and planned mutations

Run on a scratch copy of the tree (`git archive` of the branch into the scratch folder, its own target folder), each
against the gate tests of the new file; the heavy record is not rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: the record keeps the cell of the smallest change | `f885_fp_cell_two_routes` (P1) |
| M2 | solver: the record keeps the cell index plus one (modulo the cell count) | `f885_fp_cell_two_routes` (P1) |
| M3 | solver: with the record on, the cell pass flips the last bit of one cell's velocity | `f885_fp_cell_two_routes` (P2) |
| M4 | test: PRESENT from `J >= 4` to `J >= 40` | `f885_fp_rules_on_built_records` (P4) |
| M5 | test: `J` reads `d_(N-1) / d_(N-2)` | `f885_fp_rules_on_built_records` (P4) |
| M6 | record: `fan_controls.csv` cut to its first half | `f885_fp_fan_from_the_committed_record` (P5) |

The committed patch (`crates/rcfd-api/tests/f885_outer_fixed_points.mutation.patch`) is M1, with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f885_outer_fixed_points f885_fp_cell_two_routes`.

## Decision rule

* **Done** for this registration if every control ran as registered (B1) and the result names one hypothesis or rules all
  out (B2), with B3, P1 to P5 and D1 met.
* **Partial** if a control did not run as registered, or the result names none (two rows hold, the cases disagree, or a
  baseline is UNDECIDED), with the reason and what it would take.
* **Stopped and reported** if the cell record cannot be made opt-in with the defaults bit-identical, or anything else in the
  solver would need changing.

F-885 itself stays partial whatever this gives, until check H decides (the brief). A defective set-up found before a
registered run is corrected by an amendment committed before the corrected run.

## What is not covered

* Any remedy (a registration of its own, decided by the lead on this result); any change of a default.
* Check H, H(dt/2), a long run, the window state of check H (20 steps from the uniform start only, as the brief says).
* The fractional step and the moving-mesh steps.
* A decomposition of `R` face by face, or a closed form of its size on either case.

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation: the remedy, and any question of scope.

## Addendum 1: results

Written 2026-11-26, after the runs. Commits on the branch, from main f42518a3: d0f8b82e (this registration, alone),
7010a3ec (the cell in the record, `crates/rcfd-fv/src/pimple.rs`, and the test file; both registered runs were made from
it), ae536cda (rustfmt of the two files and the recorded outcomes `CAVITY_RECORD`, `FAN_RECORD`), 848167f8 (the fan record
and the logs of the two runs), c36970c9 (the committed mutation M1), and the commit of this addendum with a type alias in
the test file for clippy, the mutation logs and the proposal file. Records in `docs/investigations/f885-outer-fixed-points/`:
`fan_controls.csv`, `cavity.log`, `fan.log`, `mutations.log`, `mutations-rerun.log`.

### The sentence

**The controls name H1.** The relaxed outer iterations and the unrelaxed final one have different fixed points because
`r_au = V / a_p` is formed from the relaxed momentum diagonal (`pimple.rs` line 2207 of the base) and enters the face flux,
so discrete continuity depends on `alpha_u` while the cell momentum does not. Every entry of the row of H1 holds on the
cavity and on the fan, and no other row holds on either.

### The controls

Cavity (3 steps; `J` the median of `d_N / d_(N-1)`; settings `alpha_u` / `alpha_p`):

| Run | Settings | `J` | Class |
| --- | --- | --- | --- |
| K0 | 0.8 / 0.5, N 5 | 9.825 | PRESENT |
| K1 | 1 / 1, N 5 | 0.0741 | ABSENT |
| K2 | 0.8 / 1, N 5 | 9.845 | PRESENT |
| K3 | 1 / 0.5, N 5 | 0.487 | ABSENT |
| K4 | 0.8 / 0.5, N 20 | 1.33e7 | PRESENT; `C` 4.78e-8, CONVERGES; `Q` 0.0872 |
| K5 | as K0, `ddt_correction` off | 6.863 | PRESENT |
| K6a, K6b | 0.9 and 0.95 / 1, N 20 | | `L` 0.4870, IN [0.4, 0.6] |

Fan (20 steps of H(dt), classed over steps 11 to 20; all five runs completed, no guard trip):

| Run | Settings | `J` | Class | median `d_N / d_1` |
| --- | --- | --- | --- | --- |
| F0 | the case, N 5 | 34.78 | PRESENT | 0.388 |
| F1 | `alpha_u = 1` | 0.934 | ABSENT | 0.0218 |
| F2 | N 20 | 105.3 | PRESENT; `C` 0.0366, STALLS | 0.388 (`Q`) |
| F3 | `component_diagonal` corrected | 34.78 | PRESENT | 0.388 |
| F4 | `ddt_correction` off | 9.791 | PRESENT | 0.286 |

Rows that hold: on the cavity H1 only; on the fan H1 only; H4 on the fan (reported beside, below). Result: **named H1**.

What the numbers say. On the cavity with 20 outer iterations the relaxed ones converge to about 1e-9 m/s or less (`d_19`
1.1e-9, 2.1e-10 and 3.0e-10 m/s in the three steps) and the 20th still moves the velocity by 2.8e-3 m/s, as the 5th of K0 does (2.8e-3): the final change is the
distance between the two fixed points, not unfinished relaxed work. It halves when `1 - alpha_u` halves (0.487 against
0.5 to first order). It is gone with `alpha_u = 1` whatever `alpha_p` is (K1, K3), and stays with `alpha_p = 1` (K2), so
the pressure relaxation is not it. On the fan the component diagonal changes nothing (F3 equals F0 to 1e-12 in `J`: the
components' diagonals are equal there too). Without the transient Rhie-Chow term the jump is smaller but present (K5 6.9
against 9.8, F4 9.8 against 34.8): that term carries a part, the pressure and body-force parts the rest.

The cell (reported). On the cavity the final iteration's largest change is at c63 (x 0.9375, y 0.9375), the cell at the
lid's downstream corner, where the pressure gradient is singular, in every step of K0, K2, K4 and K5. On the fan it is
c9824 (r 0.2476 m, z 0.1215 m: in the tip gap, between the blade tip at 0.2475 m and the shroud at 0.25 m, behind the
leading edge at z 0.1 m) in every classed step of F0, F2, F3 and F4; with `alpha_u = 1` (F1) the final change moves to
other tip-gap cells and is 16 times smaller (median 0.060 against 0.978 m/s).

H4 on the fan (beside). With 20 outer iterations the relaxed changes on the fan do not fall below about 5e-3 to 9e-3 m/s
(`C` 0.0366 > 1e-2): the relaxed loop stalls there, as ADR 0767's `d_4 / d_3` of about 1 suggested. It does not explain
the jump (F1 removes the jump with the stall in place), but it bounds what a remedy can give: in the window of check H,
where ADR 0767 measured a step change of 0.09 m/s, a stall of 5e-3 m/s is above 1e-2 of a first change of that size. The
cavity's relaxed loop does not stall.

### Table of checks

| Id | Measured | Met |
| --- | --- | --- |
| B1 | K0 to K6b: 3 of 3 steps each; F0 to F4: 20 of 20 steps each, no stop | yes |
| B2 | named H1 (one row on each case, the same) | yes |
| B3 | `baseline_hashes`: 3 passed, 1 ignored; `baseline_hashes.rs` unchanged | yes |
| C0c | K0 step 1 equal to ADR 0767 A2 at the 5 printed digits | yes |
| C0f | F0 step 1: 36.093, 26.055, 4.3157, 1.3395, 0.91601 m/s against 36.09, 26.05, 4.316, 1.340, 0.916; largest relative difference 3.7e-4 against 1e-3 | yes |
| P1 | the recorded cell equals route B in all 15 outer iterations of 3 steps (c63, c60, c60, c60, c63 in step 1) | yes |
| P2 | record off: `outer_velocity` empty; `u`, `p`, flux equal bits | yes |
| P3 | `f885_check_h_outer_record`: 4 passed, 2 ignored, file unchanged | yes |
| P4 | `f885_fp_rules_on_built_records`: every built case as built | yes |
| P5 | `f885_fp_fan_from_the_committed_record`: 5 runs of 20 steps, outcomes and "named H1" as recorded | yes |
| D1 | `git diff --name-only origin/main`: under `crates/*/src` only `crates/rcfd-fv/src/pimple.rs` (branch lines 261 to 285, 1824, 2531 to 2540, 2641: the struct field, the function, the cell vector, its push); no roadmap, ledger, golden or existing test changed | yes |
| P | see "The checks of rule 7" | yes |

### Mutations

| Id | What happened |
| --- | --- |
| K0 | unmutated: every gate test passes (both runs of the script) |
| M1 | the cell of the smallest change: caught by `f885_fp_cell_two_routes` (P1). Committed as the mutation patch |
| M2 | the cell plus one: caught by `f885_fp_cell_two_routes` (P1) |
| M3 | the cell pass flips a bit of the velocity: caught by `f885_fp_cell_two_routes` (P2) |
| M4 | PRESENT at `J >= 40`: caught by `f885_fp_rules_on_built_records`, and by the two tests of the recorded outcomes |
| M5 | `J` from `d_(N-1) / d_(N-2)`: caught by the same three |
| M6 | `fan_controls.csv` cut to its first half: caught by `f885_fp_fan_from_the_committed_record` |

### Deviations

1. The first mutation run (`mutations.log`) extracted each copy with the commit's file times, so after M3 cargo did not
   rebuild `rcfd-fv` for M4 and M5, which ran on the M3 solver (their `f885_fp_cell_two_routes` failed at P2 for that
   reason). M1 to M3 had rebuilt (their files were newer). The script was corrected (every file of the copy touched) and
   K0, M4, M5 and M6 were run with it (`mutations-rerun.log`); the table is from the rerun for M4 to M6 and from the first
   run for M1 to M3.
2. The registration said the cell would be reported "and whether it is the cell of the largest change of the first
   iteration"; it is reported in the logs. In these early steps the first iteration's change is dominated by the step's
   own change, so that comparison says little.
3. `cargo clippy` first failed on one `type_complexity` lint in the new test file (the return type of `fan_controls`); a
   type alias was added, clippy then ran clean, and the new test file's gate tests were run again (4 passed, 1 ignored).
   Nothing else in the file changed; the registered runs had been made before.
4. The fan record took 298 s (estimate about 6 min).

### What was not run

Check H, H(dt/2), any long run; any remedy; the whole workspace suite.

### The checks of rule 7

* `cargo test -p rcfd-api --release --test f885_outer_fixed_points`: 4 passed, 1 ignored (the heavy record, run once).
* `cargo test -p rcfd-api --release --test f885_check_h_outer_record`: 4 passed, 2 ignored.
* `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored.
* `cargo test -p rcfd-fv --release --lib`: 115 passed, 6 ignored.
* `cargo fmt -p rcfd-fv --check` and `cargo fmt -p rcfd-api --check`: clean.
* `cargo clippy -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`: clean (deviation 3).
* `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and
  `python scripts/process_report.py --check`: at the commit of this addendum, as reported to the lead.

### Size

Size (agent): done; 1; 1; 2; 240000

### Decision by the registered rule

**Done for this registration**: every control ran as registered (B1) and the result names one hypothesis (B2), with B3,
P1 to P5 and D1 met. F-885 stays **partial**: check H has not decided.

## Open questions for the maintainer (addendum 1)

1. **The remedy.** Options: (a) an opt-in face flux whose relaxed fixed point is the unrelaxed one: the face coefficient
   and the Rhie-Chow parts formed with the unrelaxed `r_au`, or the relaxed flux with a correction for the relaxation (the
   device the literature cited from memory in ADR 0245 is known for, not read here), registered under F-885 with the
   cavity's K0 as its check (`J` ABSENT with `alpha_u = 0.8`) and the defaults unchanged; then check H again; (b) check H
   with `alpha_u = 1` in every outer iteration (F1 shows the jump gone on 20 steps, but a run of hours without relaxation
   may not be stable, and the fan's median `d_5 / d_1` there was still 0.022); (c) relax the final iteration too, so all
   iterations share the relaxed fixed point, whose answer then depends on `alpha_u`; (d) leave F-885 and F-879 partial. I
   recommend (a): it removes the cause the controls named, keeps the damping of the relaxation, and serves every pimple run
   with `n_outer > 1`, not only check H.
2. **Scope: the stall of the fan's relaxed loop.** Without the jump, the fan's relaxed iterations still stall near 5e-3 to
   9e-3 m/s (H4, beside), which may keep ADR 0183's rule from being met in the window. Options: (a) the remedy's
   registration also records the stall on 20 steps, with its cell, before check H is run; (b) run check H after the remedy
   and read the rule as it falls; (c) a diagnosis of the stall as a registration of its own. I recommend (a): one more
   control in the same registration, and no long run until both are known.

## For the lead: roadmap entries

> Status proposed for F-885: partial.

> Note entry: ADR 0810, 2026-11-26, diagnosis done, item partial: the relaxed outer iterations of the pimple step converge to a different state from the unrelaxed final one because rAU = V/a_P is formed from the relaxed diagonal and enters the face flux (its pressure, transient and body-force Rhie-Chow parts), so discrete continuity depends on alpha_u while the cell momentum does not; with alpha_u = 1 in every iteration the jump of the final iteration is gone (d_N/d_(N-1) 0.074 on the cavity, 0.93 on 20 steps of H(dt), against 9.8 and 34.8 relaxed), it stays with 20 outer iterations (relaxed changes 1e-9 m/s or less on the cavity, final 1.5e-3 to 3.9e-3), it scales with 1 - alpha_u (ratio 0.487, 0.5 predicted), and neither the pressure relaxation nor the component diagonal carries it; held by crates/rcfd-api/tests/f885_outer_fixed_points.rs and docs/investigations/f885-outer-fixed-points, six of six mutations caught.

> Gap entry: Missing for done: a class by ADR 0183's rule. ADR 0810 found the cause: the relaxed rAU in the face flux gives the relaxed and unrelaxed outer iterations different fixed points (with alpha_u = 1 the final jump is gone). Needs the remedy chosen by the maintainer (ADR 0810 question 1: a face flux independent of alpha_u, opt-in, registered under F-885) and check H run again with it; the fan's relaxed loop also stalls near 5e-3 to 9e-3 m/s, which may still fail the 1e-2 rule.

The same entries are in `docs/roadmap/proposals/0810-F-885.json` (form 1 of ADR 0755, base f42518a3), checked with
`python scripts/roadmap_proposals.py check`. No follow-up item is proposed: the remedy adds no acceptance clause and is
registered under F-885 (rule 13).

Generated pages that this work may leave not current, for the lead (rule 9): `docs/adr/index.md` (ADR 0810 is new).

## Answers of the maintainer (2026-11-26), written by the lead

1. Option (a): an opt-in face flux that does not depend on alpha_u, registered under F-885 (ADR 0811), then check H again; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
2. The fan's relaxed-loop stall is recorded in ADR 0811's registration before any long run; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
