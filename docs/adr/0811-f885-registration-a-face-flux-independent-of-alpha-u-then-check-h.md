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
