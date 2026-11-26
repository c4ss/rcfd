# ADR 0811: F-885 registration: an opt-in face flux whose converged value does not depend on alpha_u, then check H

Status: **REGISTERED 2026-11-26, written before any code of F-885 for this registration and before any run of its checks.**
Before this text no library code, test, harness or solver run of this work was written or made. What was done: reading
(below), one reading script over `docs/roadmap.json` (F-885 only) and the algebra of this page. Results go in addenda at
the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f885-alpha-free-face-flux.md`](briefs/f885-alpha-free-face-flux.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. A further registration of F-885 under rule 13 (the
remedy of the miss of [ADR 0767](0767-f885-registration-check-h-by-the-velocity-change-of-the-outer-iterations.md), whose
cause [ADR 0810](0810-f885-registration-the-fixed-points-of-the-relaxed-and-unrelaxed-outer-iterations.md) named), taken as
option (a) of ADR 0810's question 1 by the lead's decision of 2026-11-26. Branch `f885-alpha-free-face-flux` from main
d1c731b2; scratch `w49i-work/f885-alpha-free-face-flux-scratch`. Written by the model Claude Opus 5.5.

Size (agent): partial; 1; 3; 2; 250000

## What is asked

The acceptance of F-885 (read by script): the record of the velocity change of each outer iteration (in, ADR 0767), and
"Check H of ADR 0183 is then run as registered there and in ADR 0766, with the outer-iteration rule of ADR 0183 as written
(the velocity change of the 5th outer iteration at most 1e-2 of the 1st in at least 99 % of the window's steps), and
classes the hub pocket as steady, physical or numerical, or is recorded as not deciding." ADR 0767 ran check H and it did
not decide (0 % of the window's steps met the rule); ADR 0810 named the cause H1: `r_au = V / a_p` is formed from the
relaxed momentum diagonal and enters the face flux, so the relaxed and the unrelaxed outer iterations have different fixed
points. The brief asks for an opt-in face flux that does not depend on alpha_u, its checks on the cavity and on 20 steps
of H(dt), the fan's relaxed-loop stall recorded first with a rule fixed in advance, then check H with the switch on.

## What was read

Whole: `AGENTS.md`; `docs/process/agent-rules.md` (version 5); the brief; ADR 0810 (registration, addendum 1, the
answers of 2026-11-26); `crates/rcfd-api/tests/f885_outer_fixed_points.rs`.

In part: ADR 0767 lines 1 to 420 (the registration: the record, check H's E-1 to E-4, the table of checks, the decision
rule; addendum 1: the results, the times); `crates/rcfd-api/tests/f885_check_h_outer_record.rs` lines 1 to 24, 335 to
1000 and 1160 to 1330 (the measures, the class, the verdict, `run_h`, the records, the heavy tests) and its committed
mutation patch; `crates/rcfd-fv/src/pimple.rs` lines 36 to 46, 140 to 200 (settings), 1041 to 1130 (`piso`, `pimple`),
1760 to 1860 (the step's start, the transient defect, the outer loop's head), 1888 to 1935 (`u_iter`, the convecting
flux), 1960 to 2000 and 2060 to 2560 (the predictor, the relaxation, `r_au`, `gamma_p`, the correctors, the flux, the
velocity, the pressure relaxation, the end of the outer iteration); `crates/rcfd-fv/src/convection.rs` lines 300 to 330
(`face_flux`); `crates/rcfd-fv/src/pressure.rs` lines 625 to 700 (`corrected_flux`: fixed-velocity faces take `ub`) and
760 to 795 (`boundary_hby_a`); `crates/rcfd-api/src/sim.rs`, the lines that set `consistent_correctors`,
`rhie_chow_step_consistent` and `coupled_body_pair_mean` from the case (2115 to 2147, 2401); `docs/process/tolerances.md`,
the table. Of ADR 0183 and ADR 0766 nothing new was read: check H is used exactly as ADR 0767 copied it (E-1 to E-4 of
ADR 0767, the functions of `f885_check_h_outer_record.rs`).

**The published source, read.** N. Kühl, "On the Removal of Solver-Induced Dependencies in Momentum-Weighted
Interpolation for Primal and Continuous-Adjoint Flow Solvers", arXiv:2607.21736v1 (23 July 2026),
<https://arxiv.org/abs/2607.21736>, read in its HTML form: Appendix A, Eqs. 19 to 25, and Section 5. Eq. 19 is the relaxed
momentum equation `(A_P / omega) v^k = H - Omega_P [dp/dx_i - f_i]^k + ((1 - omega) / omega) A_P v^(k-1)`; Eq. 25 the
relaxed momentum-weighted interpolation in recursive form,

    V_F^k = Vtilde_F^k - omega b_F^k [ [dp/dx_i - f_i]_F^k - overline([dp/dx_i - f_i])_F^k ] dGamma_F,i + (1 - omega) dV_F^(k-1),

with `dV_F^(k-1) = V_F^(k-1) - Vtilde_F^(k-1)` the stored correction of the previous outer iteration (Eqs. 15, 24) and
`b_F` the face mobility `Omega_P / A_P` (Eq. 7); Section 5: "The resulting formulation yields a face flux that is
independent of both the relaxation factor and the time-step size." The time-step part of that paper (Eq. 16, its `d_F`
terms) is not taken: the code's transient term (ADR 0062, `ddt_correction`) is kept as it is. Nothing here rests on the
paper beyond the form of the added term; its independence on this code is shown by the derivation of section 1, from the
code's own algebra (ADR 0810 section 2). Majumdar (1988), cited from memory in ADR 0245, was not read and is not used.

## 1. The interpolation (fixed now)

**Notation** (ADR 0810 section 2). In an outer iteration with factor `alpha = alpha_u < 1` the predictor's matrix of a
component has the relaxed diagonal `d / alpha` and the source `b + (d / alpha - a_c) u*`, `u*` the iterate at the start of
the outer iteration (`u_iter`, `pimple.rs` line 1920 of the base); `r_au = V / a_p` (line 2219) with `a_p` the mean of the
relaxed diagonals; the correctors form `HbyA` and the face flux

    phi = F(HbyA) + (gamma_p / dt) D + gamma_p b_f - gamma_p |S| dp/dn          (lines 2252 to 2296, 2455)

where `F(v)` is the face flux of a cell field `v` as the correctors form it (`face_flux` of `v` with the boundary values
`boundary_hby_a(v)`: the prescribed velocity on fixed-velocity faces, the owner value elsewhere, the weighted pair mean on
coupled faces), `gamma_p = interp(r_au)`, `D` the old-level flux defect. ADR 0810 showed that at a fixed point (`u = u*`,
`phi` unchanged) `phi = F(u) + R(alpha)` with `R(alpha) = alpha R(1)` where `d = a_c` (every part of `R` carries `r_au` or
`gamma_p`), so continuity differs for each alpha.

**The switch.** A new setting `PimpleSettings::alpha_free_face_flux: bool`, default `false`. With it, in every outer
iteration with `alpha_u < 1` (so never in the final one, whose `alpha_u` is 1), every corrector adds to `phi_hby_a`, on
every face that is neither empty nor a fixed-velocity boundary face (where `corrected_flux` takes the prescribed flux),

    T_f = (1 - alpha_u) (phi*_f - F(u*)_f),

with `u*` = `u_iter` and `phi*` = `self.phi` at the start of the outer iteration (the flux the previous iteration left,
the pair the previous corrector made together). `T` is formed once per outer iteration and is the same in each corrector.
This is Eq. 25 of Kühl's paper in this code's terms: `(1 - omega) dV_F^(k-1)` with `dV = phi - F(u)` of the previous outer
iteration, the other terms being the code's relaxed `F(HbyA)` and `gamma_p` parts, which carry `omega b_F`.

**Why the converged face flux does not depend on alpha_u (the derivation).** At a fixed point of the relaxed iteration
`u = u*` and `phi = phi*`, and the corrector gives `phi = F(u) + R(alpha) + (1 - alpha)(phi - F(u))`, so
`alpha (phi - F(u)) = R(alpha) = alpha R(1)` and

    phi = F(u) + R(1),

the face flux of the unrelaxed iteration at the same state, for every `alpha` in (0, 1). The cell momentum at the fixed
point is already the unrelaxed equation (ADR 0810 section 2), and the pressure relaxation leaves both fixed points
(ADR 0810, K2 and K3). So with the switch the relaxed iterations and the unrelaxed final one share their fixed point, and
the final iteration, started at it, adds `T = 0` and has nothing to move. Conditions, stated now: `d = a_c` in every cell
(the time term makes the diagonal dominant in both cases; where `sum |a_nb| > a_c` the factor `alpha` does not come out
of `r_au` and a remainder stays), and the components' diagonals equal where the averaged diagonal is used (ADR 0810 H6,
ruled out on the fan by F3).

**Where it does not act (fixed now).** The switch is refused (a panic with a message, as `coupled_non_orth` refuses an
immersed source) with the consistent correctors (`consistent_flags().enabled`: their split forms the flux with `rAtU`,
which does not scale with alpha), with a face jump (its face division follows the flux), with an immersed solid (its
face reconstruction) and on a moving mesh (`self.ale`); it is not read by the fractional step or the steps of
`pimple/moving.rs`. Neither case of this work uses any of them (the fan case sets none of the keys, `sim.rs` 2115 to
2147). No case key: the setting is set on the built solver, as ADR 0767 and ADR 0810 set theirs; `sim.rs` and
`schema.rs` are not changed, and no golden or case hash table is touched.

**Defaults.** Without the setting no copy is made and no term is added: every default run takes the same operations as
before (bit-identical; `baseline_hashes` unchanged). With the setting and `alpha_u = 1` the term is not formed either, so
such a run equals the run without the setting in every bit (check A2).

## 2. The fan's relaxed-loop stall, and the rule for reading check H (fixed now, before any long run)

ADR 0810 addendum 1 (F2): with 20 outer iterations the fan's relaxed changes do not fall below about 5e-3 to 9e-3 m/s
(`C = d_19 / d_2` 0.0366, STALLS), while the cavity's converge. The switch removes the cause of the jump; it is not
expected to remove the stall, which F1 (`alpha_u = 1`, no jump, median `d_5 / d_1` 0.022) suggests is a property of the
fan's outer loop itself.

**Recorded first.** Before check H is started, the fan controls of section 3 (F0s and F2s, 20 steps of H(dt), the switch
on) are run and their record committed: `J` of F0s, `C` of F2s with its class, the median `d_19` of F2s over steps 11 to
20 and its cell, and `P20`, the share of F0s's steps 11 to 20 with `d_5 <= 1e-2 d_1`.

**What it means for ADR 0183's rule if it persists (fixed now).** If F2s STALLS, the 5th outer iteration of a step cannot
be expected to fall below the stall level; with `d_1` of the order of the step change, which ADR 0767 measured as 0.093
(dt) and 0.259 m/s (dt/2) in the window, a stall of 5e-3 m/s gives `d_5 / d_1` of about 0.05 and 0.02, above 1e-2: the
prediction is then NOT TIME-ACCURATE for H(dt) and possibly for H(dt/2), and the class NOT EVALUABLE.

**The rule for running and reading check H (fixed now).** Check H is run with the switch on if and only if the switch's
rows hold: A1, A2, K0s, K4s, TR, TRn, F0s and S1 met. If any is missed, check H is not run (it would be a run of a remedy
that is not shown to work) and the item is partial with that reason. When it is run, it is read by ADR 0183's rule as
written (E-1 of ADR 0767) and by nothing else: a stall does not exempt a step, and no other quantity enters the class. The
prediction from P20 and F2s is written beside the result and has no part in it. Check H is run even if the prediction is
NOT TIME-ACCURATE, because the window (steps after 0.1266 s) is not steps 11 to 20, and the brief asks for the run.

## 3. The checks on the cavity and on 20 steps of H(dt) (fixed now)

**Cavity**: the cavity of ADR 0767 and ADR 0810 (`box_2d(8, 8, 1, 1)`, lid `(1, 0, 0)`, `nu = 0.01`,
`PimpleSettings::pimple(0.01, N)`, BDF2, `dt = 0.05`, from rest, 3 steps, the record on). **Fan**: 20 steps of H(dt) on
B2 from the uniform start, the case text `h_text(4.605e-5)` of ADR 0767 and ADR 0810 copied unchanged, both records of
ADR 0767 E-2 on, settings changed on the built solver. The measures `J` (median of `d_N / d_(N-1)`; ABSENT if `J <= 2`,
PRESENT if `J >= 4`), `C` (median of `d_19 / d_2`, CONVERGES if `<= 1e-2`) and `Q` (median of `d_20 / d_1`) are those of
ADR 0810 section 5, with its upper median, over steps 1 to 3 (cavity) and 11 to 20 (fan).

| Run | Settings | Registered outcome |
| --- | --- | --- |
| K0s | K0 of ADR 0810 (`alpha_u = 0.8`, `alpha_p = 0.5`, `N = 5`), switch on | `J` ABSENT |
| K4s | as K0s with `N = 20` | `J` ABSENT, `C` CONVERGES, `Q <= 1e-2` |
| TR | `N = 30`, `alpha_p = 1`, switch on, `alpha_u = 0.8` (route a) and `0.9` (route b) | flux and velocity after each step equal within `1e-8` relative |
| TRn | TR with the switch off | flux after some step differs by more than `1e-5` relative |
| F0s | F0 of ADR 0810 (the case: `alpha_u = 0.9`, `alpha_p = 1`, `N = 5`), switch on | `J` ABSENT |
| F2s | as F0s with `N = 20` | recorded (section 2): `C` with its class, median `d_19`, its cell |

TR's measure: after each of the 3 steps, `max_f |phi_a - phi_b| / max_f |phi_a|` and `max_c |u_a - u_b| / max_c |u_a|`;
the largest over the steps is the value. With 30 outer iterations both routes converge to the fixed point (ADR 0810 K4:
the relaxed changes contract by about 0.37 an iteration, `C` 4.8e-8 over 17 iterations), so the two routes then differ
only by what the outer and linear iterations leave.

## Table of checks

Every clause of the acceptance first (A3 to A5 are check H; A1 the defaults).

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | defaults bit-identical: `cargo test -p rcfd-api --release --test baseline_hashes` passes, `baseline_hashes.rs` unchanged | passes, file unchanged | acceptance of F-885 ("baseline hashes unchanged"); rule 3 | passes | n/a | unchanged default output |
| A2 | cavity, 3 steps: the switch on with `alpha_u = alpha_p = 1` against the switch off: `u`, `p`, flux and the record equal bits; and K0 with the switch off: step 1 `d_1` to `d_5` against ADR 0767 A2 (3.4197e-2, 7.1717e-3, 1.5369e-3, 3.3740e-4, 1.4550e-3 m/s) | equal bits; relative `<= 1e-4` | "Bit-identity of default output"; "Same discrete problem by two routes" widened to the 5 printed digits as ADR 0810 C0c | equal; 0 | n/a | unchanged default output |
| K0s | `J` of K0s | `<= 2` (ABSENT) | ADR 0810 section 5 (the class ABSENT); the brief ("J ABSENT by ADR 0810's limits") | 0.3 to 0.5 (ADR 0810 K3, the final unrelaxed iteration without the jump: 0.487) | 4 | code verification |
| K4s | `J`, `C` and `Q` of K4s | `J <= 2`; `C <= 1e-2`; `Q <= 1e-2` | ADR 0810 section 5; `Q`: ADR 0183's outer ratio 1e-2, as ADR 0767 E-1 states it | `J` about 0.4; `C` about 5e-8 (ADR 0810 K4); `Q` about 1e-8 (`d_20` at the relaxed level, 1e-10 against `d_1` 3e-2) | 5; 2e5; 1e6 | code verification |
| TR | the two routes of section 3 with the switch on | `<= 1e-8` relative | `docs/process/tolerances.md`, "Same discrete problem by two routes" (`<= 1e-8`) | 1e-11 | 1e3 | code verification |
| TRn | TR with the switch off | `> 1e-5` relative after some step | ADR 0810 addendum 1 (the final change, the distance of the two fixed points, 1.5e-3 to 3.9e-3 m/s at a lid speed of 1 m/s; the limit is 1e-2 of that) | 1e-3 | 100 | code verification |
| F0s | `J` of F0s on the fan | `<= 2` (ABSENT) | ADR 0810 section 5; the brief | about 1 (ADR 0810 F1, no jump: 0.934) | 2 | code verification |
| F2s | F2s completes 20 steps; `C`, median `d_19`, cell recorded | completes | the brief (the stall recorded before any long run) | completes; STALLS (`C` about 0.04) | n/a | solution verification |
| S1 | 20 steps of H(dt) on the path of the check H runs (`run_h` with the switch on), `d_1` to `d_5` of every step against F0s of the committed fan record | relative `<= 1e-6` | "Same discrete problem by two routes" (`<= 1e-8`), widened to the 10 printed digits and the rounding of the record as ADR 0810 P5 | 0 (the same computation by two harnesses) | n/a | code verification |
| A3 | check H with the switch on: H(dt) completes 5 498 steps and H(dt/2) 10 996, no failure, no guard trip, every field finite | both complete | acceptance of F-885; ADR 0183 D5; ADR 0766 A1; ADR 0767 A3 | complete | n/a | solution verification |
| A4 | each run TIME-ACCURATE by E-1 of ADR 0767 | at least 99 % of the window's steps with `d_5 <= 1e-2 d_1` | acceptance of F-885; ADR 0183, check H, the outer-iteration rule | not met if the stall persists (section 2: `d_5 / d_1` about 0.05 and 0.02) | below 1 | solution verification |
| A5 | the class by ADR 0183's rule and the verdict of F-879 by ADR 0766's table, as ADR 0767 A5 | one of the four, by the registered numbers of ADR 0767 A5 | acceptance of F-885; ADR 0183, check H; ADR 0766 | NOT EVALUABLE if A4 is missed; STEADY if met (ADR 0766's site amplitudes 5.4e-3 and 6.5e-3 m/s) | n/a | solution verification |
| G1 | the class and the verdict from the committed records of the two runs: step counts 5 498 and 10 996, windows 2 749 and 5 498, the shares, `A` and `f`, class and verdict as recorded | counts exact; 1e-6 relative; class and verdict equal | "Same discrete problem by two routes", widened to the printed digits as ADR 0767 G1 | 0 | n/a | unchanged default output |
| G2 | the fan controls from their committed record: F0s and F2s, 20 steps each, the outcomes as recorded | equal; `J`, `C` within 1e-6 relative | as G1 | 0 | n/a | unchanged default output |
| D1 | under `crates/*/src` only `crates/rcfd-fv/src/pimple.rs` changes; `docs/roadmap.json`, the ledger, every golden and every existing test file unchanged; the committed mutation patches of `f885_check_h_outer_record` and `f885_outer_fixed_points` still apply (`git apply --check`), re-based in this branch with the mutation unchanged if not | as stated | the brief, owned files; rule 3; the lead's instruction | as stated | n/a | unchanged default output |
| P | `cargo test -p rcfd-fv --release --lib`; the new test file; `f885_outer_fixed_points` and `f885_check_h_outer_record` (the existing tests of the changed step); `cargo fmt -p rcfd-fv -p rcfd-api --check`; `cargo clippy -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

No row is of kind validation: F-885 claims no agreement with a measured case.

**Reported beside, without a limit:** P20 and the prediction of section 2; per check H run, over the window, the spread
of `d_5 / d_1`, `d_1`, `d_5` and of `d_(k+1) / d_k`, the step change, the site signal, torque and pressure rise and
their means against ADR 0766's (the solution changes with the switch, so ADR 0767's R0 does not apply); the cell of the
final iteration's largest change in the fan controls; seconds per step.

## 4. Check H (fixed now)

As ADR 0767 section 2 ran it, unchanged but for one line: the switch on (`alpha_free_face_flux = true` on the built
solver, beside E-2's two records). The case, the time steps (4.605e-5 s, 5 498 steps; 2.3025e-5 s, 10 996 steps), the
window (t above 0.1266 s), the six site cells, the measures, the STEADY threshold 0.193 m/s, the classes in their order,
the verdict with persistence 0.75, the outer rule E-1, no wall-clock cap, a stop on a failure, a guard trip or a
non-finite value (NOT EVALUABLE), the two runs made once and started together (ADR 0183 D8), stop rule 4 of ADR 0183.
The functions are copied from `f885_check_h_outer_record.rs` unchanged; ADR 0767's R0 (agreement with ADR 0766's record)
is not run, since the switch changes the solution, and so by ADR 0767 E-3 the full rows of both runs are committed
(`H_dt.csv`, `H_dt2.csv`) with the outer record, and the class is read from them alone.

**Time and machine, stated before the start.** The machine: this Windows 11 Pro workstation, 16 logical processors, 94
GB of memory (25 GB free at registration), shared with other agents' builds and runs. ADR 0767 recorded 156.8 and 256.8
min (1.71 and 1.40 s a step) for the same two runs started together on it; the switch adds one face flux of the iterate
per relaxed outer iteration (under 1 % of a step). Estimate: about 2.7 h and 4.5 h, both started together in the
background; longer under load, with no cap.

## Tests (fixed now)

All in `crates/rcfd-api/tests/f885_alpha_free_face_flux.rs` (header: ADR 0811, F-885).

* Run by the gate: `f885_af_defaults_and_bits` (A2); `f885_af_cavity` (K0s, K4s, TR, TRn);
  `f885_af_fan_from_the_committed_record` (G2); `f885_af_short_run_on_the_check_h_path` (S1, 20 steps of H(dt), about one
  minute); `f885_af_class_from_the_committed_records` (G1, after the runs).
* Heavy records (`#[ignore]`, run once by me): `f885_af_fan_controls_record` (F0s, F2s; about 3 min), and
  `f885_af_h_dt_record`, `f885_af_h_dt2_record` (check H), writing to the folder named by `F885_AF_OUT`.

The values of G1 and G2 are written into the file after the registered runs; the form and the tolerance are fixed here.
Records: `docs/investigations/f885-alpha-free-face-flux/` (`fan_controls.csv`, `H_dt.csv`, `H_dt2.csv`, the logs).

## Negative controls and planned mutations

On a scratch copy of the branch (`git archive` into the scratch folder, its own target folder, every file touched after
extraction), each against the gate tests of the new file; the heavy records are not rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: the term with the factor `alpha_u` in place of `1 - alpha_u` | `f885_af_cavity` (K0s, TR) |
| M2 | solver: the term without `F(u*)` (`(1 - alpha_u) phi*`) | `f885_af_cavity` |
| M3 | solver: the term also in the final iteration, with `1 - s.alpha_u` | `f885_af_cavity` (K4s, TR) |
| M4 | solver: the switch read as always on | `f885_af_defaults_and_bits` (A2, the ADR 0767 values with the switch off) |
| M5 | test: TR compares route a with itself | `f885_af_cavity` (TRn) |
| M6 | record: `fan_controls.csv` cut to its first half | `f885_af_fan_from_the_committed_record` |

The committed patch (`crates/rcfd-api/tests/f885_alpha_free_face_flux.mutation.patch`) is M1, with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f885_alpha_free_face_flux f885_af_cavity`.

## Decision rule

* **Done** for F-885 if the switch's rows hold (A1, A2, K0s, K4s, TR, TRn, F0s, F2s, S1, D1 met) and check H decides a
  class by ADR 0183's rule with the switch on: A3 met, A4 met in both runs, and the class STEADY, PHYSICAL or NUMERICAL
  (with the verdict of ADR 0766's table), held by G1.
* **Partial** otherwise, with the reason: a switch row missed (then check H is not run, section 2), or check H run and not
  deciding (a run not TIME-ACCURATE, or NOT EVALUABLE, or PHYSICAL without persistence), with what it would take.
* **Stopped and reported** if the switch cannot be made opt-in with the defaults bit-identical, or anything else in the
  solver would need changing.

A defective set-up found before a registered run is corrected by an amendment committed before the corrected run.

## What is not covered

* A default change: the switch stays off by default (a question for the maintainer).
* The switch with the consistent correctors, a face jump, an immersed solid, a moving mesh or the fractional step.
* The time-step independence of Kühl's Eq. 16 (the transient term is the code's own).
* A diagnosis of the fan's relaxed-loop stall (recorded, not explained), H6 beyond ADR 0810 F3, a third time step, B1,
  a longer run.
* Any agreement with the measurement.

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation: whether the switch becomes the default (a default
change, never in this work), and any question of scope (the stall).

## Addendum 1: results

Written 2026-11-26, after the runs. Commits on the branch, from main d1c731b2: 1389abe2 (this registration, alone),
11840b32 (the switch in `crates/rcfd-fv/src/pimple.rs` and the test file; every registered run was made from it),
2979b6b3 (the fan controls' record, the cavity and fan logs, the recorded outcomes `FAN_RECORD`), 83d6a6d3 (the committed
mutation M1), 074b7e47 (the mutation log), 3e9ba395 (check H: the full rows and logs of both runs, the recorded class
`RECORD`, deviation 1), and the commit of this addendum with a stray comment line removed from the test file (deviation
4) and the proposal file. Records in `docs/investigations/f885-alpha-free-face-flux/`: `fan_controls.csv`, `H_dt.csv`,
`H_dt2.csv`, `cavity.log`, `fan.log`, `s1.log`, `H_dt.log`, `H_dt2.log`, `mutations.log`.

### The sentence

**The switch removes the jump; check H still does not decide.** With `alpha_free_face_flux` the relaxed outer iterations
and the unrelaxed final one share their fixed point: on the cavity two values of `alpha_u` reach the same face flux to
9.9e-12 (6.8e-4 without the switch), and the final iteration's jump is gone on both cases (`J` 0.27 on the cavity, 0.66
on the fan, against 9.8 and 34.8 without it in ADR 0810). Check H was run with the switch on as registered and completed
both runs, but no step of either window meets ADR 0183's outer-iteration rule (0 % and 0 % against 99 %), so both runs
are NOT TIME-ACCURATE, the class is NOT EVALUABLE and the verdict of F-879 is "does not decide". The reason is the one
recorded first in section 2: the fan's outer loop does not contract. In the window the 2nd to 5th outer iterations move
the velocity as much as the 1st (median `d_5 / d_1` 0.95 and 1.08).

### The switch's rows

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| A1 | passes, file unchanged | `baseline_hashes`: 3 passed, 1 ignored; `baseline_hashes.rs` unchanged | yes |
| A2 | equal bits; `<= 1e-4` | switch on with `alpha_u = alpha_p = 1`: `u`, `p`, flux, record equal bits after 3 steps; K0 switch off, step 1: 3.4197e-2, 7.1717e-3, 1.5369e-3, 3.3740e-4, 1.4550e-3 m/s, equal to ADR 0767 A2 at the printed digits | yes |
| K0s | `J <= 2` | `J` 0.2658 (ABSENT); step 1 `d_k` 3.4197e-2, 7.2227e-3, 1.5538e-3, 3.4198e-4, 9.7890e-5 m/s (the 5th was 1.4550e-3 without the switch) | yes |
| K4s | `J <= 2`; `C <= 1e-2`; `Q <= 1e-2` | `J` 0.5230; `C` 9.149e-9; `Q` 1.008e-9 | yes |
| TR | `<= 1e-8` | 9.863e-12 (largest of flux and velocity over 3 steps) | yes |
| TRn | `> 1e-5` | 6.773e-4 (switch off: flux 9.6e-5 to 2.0e-4, velocity 3.3e-4 to 6.8e-4) | yes |
| F0s | `J <= 2` | `J` 0.6585 (ABSENT) | yes |
| F2s | completes | 20 of 20 steps; `C` 0.02581 (STALLS); median `d_19` 6.361e-3 m/s, in every classed step at c11837 (r 0.2492 m, z 0.2306 m: the tip gap, downstream of the blade); `Q` 3.063e-3 | yes |
| S1 | `<= 1e-6` | 4.18e-10 (`run_h` with the switch against F0s of the record, every `d_k` of 20 steps) | yes |
| G2 | equal, `1e-6` | `f885_af_fan_from_the_committed_record`: as recorded | yes |
| D1 | as stated | `git diff --name-only d1c731b2` (the base) names under `crates/*/src` only `crates/rcfd-fv/src/pimple.rs` (branch lines 181 to 188, 1095, 2244 to 2260, 2291 to 2295); no roadmap, ledger, golden or existing test file changed; the committed patches of `f885_check_h_outer_record` and `f885_outer_fixed_points` apply unchanged (`git apply --check`), so neither was re-based | yes |

The stall recorded first (section 2): F2s STALLS with `C` 0.026 (0.037 without the switch, ADR 0810), the relaxed changes
of steps 11 to 20 staying at 4.3e-3 to 1.1e-2 m/s at one tip-gap cell; `P20` = 0.50 (5 of the 10 classed steps of F0s
meet `d_5 <= 1e-2 d_1`, where `d_1` is still large, 0.6 to 3 m/s). The prediction written before the run, NOT
TIME-ACCURATE if the stall persists, is what came out.

### Check H (A3 to A5, G1)

Started together at 16:01 UTC; H(dt) ended at 17:36 (94.7 min, 1.03 s a step), H(dt/2) at 19:02 (181.0 min, 0.99 s a
step), against the estimate of about 2.7 h and 4.5 h (the machine was less loaded than for ADR 0767).

| | H(dt) | H(dt/2) |
| --- | --- | --- |
| A3: steps, failure, fields | 5 498, none, finite; mass balance 6.0e-16 | 10 996, none, finite; 6.0e-16 |
| A4: window steps meeting `d_5 <= 1e-2 d_1` | **0.0000** | **0.0000** |
| `d_5 / d_1`: smallest, median, largest | 0.147, 0.953, 5.82 | 0.861, 1.085, 1.41 |
| `d_1` (m/s): median | 0.105 | 0.0485 |
| `d_5` (m/s): median | 0.115 | 0.0491 |
| medians of `d_2/d_1`, `d_3/d_2`, `d_4/d_3`, `d_5/d_4` | 0.967, 0.902, 1.029, 1.22 | 1.55, 0.849, 0.862, 0.977 |
| step change (m/s): median | 0.113 | 0.0344 |
| site signal: cell, `A`, crossings | c157, 1.451348e-3 m/s, 1 | c157, 2.288205e-3 m/s, 1 |
| torque, pressure rise: window means | 2.014952 N m, 71.2453 Pa | 2.017549 N m, 71.4638 Pa |
| five cells of largest amplitude | tip region, r 0.244 to 0.247 m, A 0.070 to 0.100 m/s | tip region, r 0.246 to 0.247 m, A 0.060 to 0.061 m/s |

| Id | Measured | Met |
| --- | --- | --- |
| A3 | both runs complete | yes |
| A4 | 0 % and 0 % against 99 % | **no, in both runs** |
| A5 | class NOT EVALUABLE (first clause: not TIME-ACCURATE); verdict "does not decide" | a verdict by the rule, the one that does not decide |
| G1 | `f885_af_class_from_the_committed_records`: counts 5 498 and 10 996, windows 2 749 and 5 498, shares 0 and 0, `A` as recorded, class and verdict as recorded | yes |

What the record says. Against ADR 0767 (no switch) the window's first change fell from about 1.01 and 0.97 m/s to 0.105
and 0.049 m/s: the switch removed the 1 m/s swing between the two fixed points, and `d_1` is now of the order of the step
change (0.11 and 0.034 m/s). But the outer iterations of a window step do not contract: `d_2` to `d_5` stay at the level
of `d_1`, and in H(dt/2) the 2nd iteration moves the field more than the 1st. That is the stall of F2s at the scale of
the window. A pattern read from the logs, without a limit: the largest changes sit in the tip region (F2s c11837 in the
tip gap; the cells of largest amplitude at r 0.244 to 0.247 m). By the rule fixed in section 2 nothing else enters the
class. Reported only: the site amplitudes (1.5e-3 and 2.3e-3 m/s) are far below the STEADY threshold 0.193 m/s, and the
window means are within 0.4 % in torque and 2.3 % in pressure rise of ADR 0767's, but no class is asserted.

### Mutations

From `mutations.log` (git archive of 2979b6b3 in the scratch folder, its own target folder, the restored and mutated
files touched before each build):

| Id | What happened |
| --- | --- |
| K0 | unmutated: the three gate tests pass |
| M1 | factor `alpha_u` for `1 - alpha_u`: caught by `f885_af_cavity` (K0s `J` 6.72, K4s, TR 4.9e-3). Committed as the mutation patch |
| M2 | without `F(u*)`: caught by `f885_af_cavity` (K0s `J` 9.86, K4s, TR 4.3e-3) |
| M3 | the term also in the final iteration: caught by `f885_af_cavity` (K0s `J` 9.23, K4s, TR 3.7e-2) |
| M4 | the switch always on: caught by `f885_af_defaults_and_bits` (K0 switch off no longer ADR 0767 A2) and by `f885_af_cavity` (TRn 9.9e-12) |
| M5 | TR compares route a with itself: caught by `f885_af_cavity` (TRn 0) |
| M6 | `fan_controls.csv` cut to its first half: caught by `f885_af_fan_from_the_committed_record` |

### Deviations

1. `read_full` (the reader of G1) first asserted 22 columns per row; the rows have 21 (15 of ADR 0766's form, 5 of the
   outer record, the step change), so G1's first run failed on the count before reading anything. The count and its
   comment were corrected (the column indices were right); no limit, measure, record or registered run changed. G1 was
   then run once and passed.
2. The fan controls took 19 s and 73 s (estimate about 3 min); check H took 94.7 and 181.0 min (estimate 2.7 and 4.5 h).
3. Rule 7's run of the new test file was made test by test, as each registered step came due (A2 and the cavity; G2 and
   S1; G1), each once, not as one command over the file.
4. `cargo clippy` first failed on one lint in the new test file ("empty line after doc comment": a comment line left by
   the copy from `f885_check_h_outer_record.rs`, above no item). The line was removed, `cargo fmt -p rcfd-api --check`
   and clippy then ran clean. Nothing else in the file changed; the registered runs had been made before.

### What was not run

Any change of a default; the switch with the consistent correctors, a face jump, an immersed solid, a moving mesh or the
fractional step; any diagnosis of the stall; a third time step or a longer run (stop rule 4 of ADR 0183); the whole
workspace suite.

### The checks of rule 7

* The new test file: `f885_af_defaults_and_bits`, `f885_af_cavity` (2 passed); `f885_af_fan_from_the_committed_record`,
  `f885_af_short_run_on_the_check_h_path` (2 passed); `f885_af_class_from_the_committed_records` (1 passed); the three
  heavy records run once each.
* `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored.
* `cargo test -p rcfd-fv --release --lib`: 115 passed, 6 ignored.
* `cargo test -p rcfd-api --release --test f885_outer_fixed_points`: 4 passed, 1 ignored;
  `--test f885_check_h_outer_record`: 4 passed, 2 ignored.
* `cargo fmt -p rcfd-fv --check`, `cargo fmt -p rcfd-api --check`: clean; `cargo clippy -p rcfd-fv -p rcfd-api
  --all-targets -- -D warnings`: clean (deviation 4).
* `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and
  `python scripts/process_report.py --check`: at the commit of this addendum, as reported to the lead.

### Size

Size (agent): partial; 1; 3; 2; 260000

### Decision by the registered rule

**Partial.** Every row of the switch holds (A1, A2, K0s, K4s, TR, TRn, F0s, F2s, S1, D1), check H was run as registered
with the switch on and both runs completed (A3), and it does not decide: A4 is missed in both runs (0 % against 99 %), so
the class is NOT EVALUABLE. What it would take: an outer loop on the fan that contracts within a step in the window (the
stall diagnosed and removed), or the maintainer's word on how ADR 0183's rule reads a loop that does not.

## Open questions for the maintainer (addendum 1)

1. **Whether the switch becomes the default.** Options: (a) keep it opt-in; (b) make it the default for pimple runs with
   `n_outer > 1` and `alpha_u < 1`, in a registration of its own with the baseline hashes re-recorded on the maintainer's
   approval (every relaxed pimple run changes its bytes); (c) remove it. I recommend (a) for now: it is shown to do what it
   is for, but by itself it does not make check H decide, and a default change of every relaxed pimple run should wait
   until it is shown to help a case end to end.
2. **Scope: the stall of the fan's outer loop.** Options: (a) a diagnosis of the stall registered under F-885 (rule 13),
   on 20 steps of H(dt) with the switch on and the cell record (F2s's c11837 in the tip gap at z 0.23 m), with controls
   fixed in advance (the momentum and pressure solver tolerances, the deferred linear-upwind correction, the turbulence
   update in the hook, the seam), before any further long run; (b) a reading of ADR 0183's rule for a loop that does not
   contract (a change of the rule, the maintainer's to make); (c) leave F-885 partial with check H recorded as not
   deciding. I recommend (a): the jump is now gone, and the stall is the one thing the record names as standing in the way.

## For the lead: roadmap entries

> Status proposed for F-885: partial.

> Note entry: ADR 0811, 2026-11-26, partial: an opt-in face flux whose converged value does not depend on alpha_u (PimpleSettings::alpha_free_face_flux, the term (1 - alpha_u)(phi* - F(u*)) of Kuhl, arXiv:2607.21736 Eq. 25; defaults bit-identical) gives the relaxed and unrelaxed outer iterations one fixed point (two alpha_u reach the same face flux to 9.9e-12 on the cavity; the final jump d_N/d_(N-1) 0.27 on the cavity and 0.66 on 20 steps of H(dt)), but check H run with it completes and does not decide: 0 % of window steps meet d5 <= 1e-2 d1 (median d5/d1 0.95 and 1.08) because the fan's outer loop does not contract within a step; held by crates/rcfd-api/tests/f885_alpha_free_face_flux.rs and docs/investigations/f885-alpha-free-face-flux, six of six mutations caught.

> Gap entry: Missing for done: a class by ADR 0183's rule. ADR 0811 removed the jump of the final outer iteration (opt-in alpha_free_face_flux) and ran check H with it: both runs complete, 0 % of window steps meet d5 <= 1e-2 d1, because the fan's outer loop does not contract within a step (d2 to d5 about d1, 0.05 to 0.1 m/s; the relaxed loop stalls near 6e-3 m/s at a tip-gap cell). Needs a diagnosis of that stall (ADR 0811 question 2) or the maintainer's reading of the rule.

The same entries are in `docs/roadmap/proposals/0811-F-885.json` (form 1 of ADR 0755, base d1c731b2), checked with
`python scripts/roadmap_proposals.py check`. No follow-up item is proposed: a diagnosis of the stall adds no acceptance
clause and would be registered under F-885 (rule 13).

Generated pages that this work may leave not current, for the lead (rule 9): `docs/adr/index.md` (ADR 0811 is new).

## Answers of the maintainer (2026-11-26), written by the lead

1. Option (a): the switch stays opt-in; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
2. Option (a): the fan's stall is diagnosed under F-885 (rule 13), on 20 steps with the cell record and the controls fixed in advance, ADR 0815; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
