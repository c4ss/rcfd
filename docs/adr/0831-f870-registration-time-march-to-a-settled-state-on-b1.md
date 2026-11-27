# ADR 0831: F-870 registration: FAN-01 design point, a time march to a settled state on B1 at two steps, and the compared quantities read from it

Status: **REGISTERED 2026-11-27, written before any code of F-870 for this registration and before any run of its checks.**
Before this text no test, harness or solver run of this work was written or made. What was done: reading (below), one
reading script over `docs/roadmap.json` (F-870), the first two lines of ADR 0824's record `H_dt.csv` and the last lines of
its logs (the wall time), a check of the free memory, and the arithmetic of this page (`arith.py` in the scratch folder).
Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f870-time-march-b1.md`](briefs/f870-time-march-b1.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. Branch `f870-time-march-b1` from 39d689c6; scratch
`w49i-work/f870-time-march-b1-scratch`. Written by the model Claude Opus 5.5. No solver source is changed.

Size (agent): partial; 0; 2; 1; 400000

## What is asked

F-870's acceptance (read by script): pressure rise and shaft power at the design point (1.4032 m^3/s; measured 126.46 Pa
and 333.30 W) each within a band registered before the run from the measurement's stated uncertainty and a
mesh-convergence estimate, on a mesh that meets the registered quality gates, with the solve converged by its registered
rule; beside them the Euler head and the kinetic energy leaving the rotor over the measured span (220.02 and 99.20 Pa),
the circumferential-mean profiles of `c_x` and `c_t` at the measured plane, and the time-mean casing pressure behind the
rotor (113.90 Pa at x = 90 mm), each within a band registered before the run. The convergence rule is now option (c) of
[ADR 0827](0827-f879-registration-check-h-class-torque-and-pressure-rise-in-time.md) section 4, taken by the lead on
2026-11-27 (its answer 1).

## What was read

Whole: `AGENTS.md`; the rules (version 5); the brief; ADR 0827. Of
[ADR 0762](0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md): the registration, amendment 1, addendum
1 and the answers (lines 1 to 280 and 333 to 600). Of
[ADR 0765](0765-f878-registration-fan01-lda-profiles-euler-work-and-the-division-of-the-missing-pressure-rise.md): the design,
the rule of division, the checks, amendment 1 and the results to "Every check" (lines 149 to 570). Of
[ADR 0769](0769-f883-registration-fan01-wall-pressure-behind-the-rotor.md): what is asked, the identity, the results, the
question and the answer. Of [ADR 0766](0766-f879-registration-fan01-check-h-the-hub-pocket-in-time.md): the set-up and the
runs (lines 60 to 80). Of [ADR 0824](0824-f885-registration-the-lag-of-the-kept-convection-gradient-its-arms-a-corrected-variant-and-check-h.md):
section 5 by search. Code: `crates/rcfd-api/tests/f885_kept_gradient_lag.rs` lines 1 to 300 and 598 to 783 (`h_text`,
`run_h`, the record form); `crates/rcfd-api/tests/f878_fan01_lda_profiles.rs` lines 19 to 177, 300 to 700 and 786 to 860
(the sampler, the integrals, the recorded B1/B2 profiles). No published source outside the repository is used: the
measures are from first principles (a mean, a half range, the trapezoid rule) and every limit is a number of a record of
the repository, named per row.

## 1. The case (fixed now)

* **Mesh.** B1 of ADR 0086 (`camber_seam_h`, counts 12, 24, 12, 15, 6, 21, 10; 38 184 cells), the text of `h_text` of
  `f885_kept_gradient_lag.rs` with the B1 counts in place of B2's and no other change (SST, `rotating_frame` with the
  centrifugal part, seam diffusivity `pair`, Barth-Jespersen-limited `linear_upwind`, `pimple`, `n_outer = 5`, BDF2,
  `end_time = 0.2532`).
* **The four opt-ins** of ADR 0824 as `run_h` sets them: `alpha_free_face_flux`, `outer_frozen_limiter`,
  `outer_frozen_convection_gradient`, `outer_frozen_convection_gradient_extrapolated_boundary`; with both outer records on
  (`outer_pressure_tolerance = Some(-1.0)`, `outer_velocity_record`), so every step makes its five outer iterations, as in
  check H.
* **Start**: the uniform state of the case text (ADR 0766, "Start state").
* **Steps**: `dt = T_c / 100 = 4.605e-5 s` (ADR 0766: hub chord 0.099 m over the hub relative speed 21.50 m/s; a property
  of the blade and the speed, not of the mesh) and `dt/2 = 2.3025e-5 s`. 5 498 and 10 996 steps, 0.2531829 s each: six
  flow-through times `T_ft` = 42.197 ms (ADR 0183 D5, as ADR 0827 section 1). The **last flow-through time** is the steps
  with `t > 5 T_ft` = 0.2109858 s: steps 4 582 to 5 498 and 9 164 to 10 996 (917 and 1 833 steps).
* **Records**, one row per step (step, t, torque, `dp_ts`, `res_u`, continuity, `d1` to `d5` of the outer velocity record,
  step change), `M_dt.csv` and `M_dt2.csv`; and the **rings**, `M_dt_rings.csv` and `M_dt2_rings.csv`: at five steps of
  the last flow-through time, 4 582, 4 811, 5 040, 5 269, 5 498 (dt) and 9 164, 9 622, 10 080, 10 538, 10 996 (dt/2; the
  same times), the circumferential means of ADR 0765 (360 points, the cell's absolute velocity of its amendment 1) at the
  26 measured radii and at one casing ring `r = 249.5 mm`, on the planes x = 88.7, 90 and 91.3 mm, with `p_in`. The
  sampler (`Locator`, `ring_mean`, `trapezoid`, `head`, `solved`) is copied from `f878_fan01_lda_profiles.rs` without a
  change, where V2 to V4 of ADR 0765 verify it. The casing ring takes the pressure of the wall-adjacent cell: at a wall
  the normal gradient of the pressure vanishes to the order of the boundary layer (first principles), and the first cell
  is about 0.57 mm thick (ADR 0762), so 249.5 mm lies in it.
* **Wall clock and load**: each run records its seconds per step every 250 steps; a logger in the scratch folder writes
  every 15 minutes the machine's CPU load and each run's CPU time and memory; the log is committed with the records. No
  wall-clock cap (ADR 0766 D-3). Estimate: B2 took 1.136 and 0.90 s per step (ADR 0824's logs); B1 has 2.80 times the
  cells, so about 3.2 and 2.5 s per step alone, 5 and 8 hours, more with both together on a shared machine.
* **Stops**: a failure of the simulation or a non-finite value ends a run; it is then recorded as **not deciding**, with
  the step it reached. Each run is made once; a run is not lengthened.

## 2. The convergence rule (option (c) of ADR 0827, fixed now)

Over the last flow-through time of each run, for the shaft torque `-9 (m_blade + m_hub)` and
`dp_ts = rho (p_out - (p_in + u_in^2 / 2))`: mean `m`, half range `A = (max - min) / 2`.

* **S1 (settled, each run)**: `A / abs(m) <= 1 %` for torque and for `dp_ts`, in M(dt) and in M(dt/2) (four numbers).
* **S2 (the two steps)**: `abs(m(dt/2) / m(dt) - 1) <= 1 %` for torque and for `dp_ts`.

Converged iff S1 and S2 are met. Source of both limits: ADR 0762, "The tolerance of the state", as ADR 0827 section 4
and its answer 1 fixed them. A march that is not converged is recorded as **not converged**, with its numbers; the
compared quantities below are formed and reported all the same, as read from a state that is not converged, and the
acceptance is then missed on its convergence clause. Reported beside, no limit: the share of the steps of the last
flow-through time with `d5 <= 1e-2 d1` (E-1 of ADR 0767), the half range over each of the six flow-through times, the
Richardson value `m(dt/2) + (m(dt/2) - m(dt)) / 3`.

## 3. The compared quantities and their bands (fixed now)

Every compared value is **of M(dt/2)**: for torque and `dp_ts` its mean over the last flow-through time; for the sampled
quantities the mean over its five samples. M(dt) is reported beside. The band's form is ADR 0762's and ADR 0765's: terms
added, `u_meas = 0` where no source states an uncertainty (ADR 0762, open question 1; F-877 is not done).

| Id | Quantity | Measured | Band |
| --- | --- | --- | --- |
| Q1 | total-to-static pressure rise | 126.46 Pa | 29.75 Pa (ADR 0762, `U_num`) |
| Q2 | shaft power `Omega m` of torque | 333.30 W | 32.98 W (ADR 0762, `U_num`) |
| Q3 | Euler head `e`, pointwise reading, measured span at x = 90 mm | 220.02 Pa (formed from the constants) | `U_num` 14.229 Pa + `S` + `T` + `u_data` 1.20 Pa |
| Q4 | kinetic energy leaving `q`, pointwise reading, the same span | 99.20 Pa (the same) | `U_num` 12.936 Pa + `S` + `T` + `u_data` 2.07 Pa |
| Q5 | `c_x` at the 26 radii at x = 90 mm | `ux_mean` | at each radius `U_num(r) + S(r) + T(r)` |
| Q6 | `c_t` at the 26 radii | `uphi_mean` | the same |
| Q7 | casing pressure `rho (p_ring(249.5 mm) - p_in - u_in^2 / 2)` at x = 90 mm | 113.90 Pa (ADR 0769) | `U_num` 21.280 Pa + `S` + `T` + `u_stat` 0.5674 Pa + `u_x` 0.4901 Pa |

* `U_num` is the mesh term of ADR 0765, `1.25 abs(f_B2 - f_B1) / (r_21 - 1)`, `r_21 = 1.40981`, from its recorded B1 and
  B2 values (`B1_SCALARS`, `B2_SCALARS`, `B1_CX` and the like of `f878_fan01_lda_profiles.rs`): for `e` 14.229, for `q`
  12.936, for `Pi_s` 21.280 Pa (`arith.py`). The time march has one mesh, so its mesh term is the family's; this is the
  only mesh-convergence estimate the repository has for these quantities. For Q7 no casing value of B1 and B2 was ever
  sampled; its mesh term is that of `Pi_s`, the static pressure at the same plane against the same inlet total pressure.
* `S`: half the range over the five samples of M(dt/2) (the spread of the settled state; ADR 0765's `S` over the cycle).
* `T`: half the range of the sample mean over the planes 88.7, 90 and 91.3 mm (ADR 0765's place of the plane).
* `u_data` (Q3, Q4): the difference of the two readings of the measured value (ADR 0765: 221.22 against 220.02, 97.13
  against 99.20). `u_stat` and `u_x` (Q7): the two terms of ADR 0769's reading (two standard errors of the transducer
  means, the place between the transducers); the record states no uncertainty of the transducers.
* Q5, Q6 are met iff every radius is within its band; the count of radii within is reported.

**Comparison with ADR 0762's B1 SIMPLEC sample** (row K): `abs(m(dt/2) - m_SIMPLEC)` with `m_SIMPLEC` 2.098164 N m and
73.9881 Pa, limit `U_num` 0.21190 N m and 29.75 Pa (ADR 0762, the band arithmetic; the limit of ADR 0827 T4). The
differences of `e`, `q`, `Pi_s` and of the profiles against ADR 0765's B1 record are reported, no limit.

## Table of checks

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| R | both marches run as registered, complete | 5 498 and 10 996 rows, steps 1 to N; 5 ring samples each | the brief; ADR 0766 | complete | n/a | solution verification |
| S1 | `A / abs(m)` of torque and `dp_ts`, last flow-through time, each run | `<= 1 %` | ADR 0762, tolerance of the state; ADR 0827 answer 1 | 0.1 % and 0.2 % (dt), 0.03 % (dt/2), as on B2 | 10 to 30 | solution verification |
| S2 | means, dt against dt/2 | `<= 1 %` each | the same | 0.15 % and 0.3 % (B2) | 3 to 7 | solution verification |
| Q1 | `abs(dp_ts - 126.46)` | `<= 29.75 Pa` | acceptance of F-870; ADR 0762 | 45 Pa (82 Pa: B1 SIMPLEC plus the route's 11 % on B2) | 0.66: expected MISS | validation |
| Q2 | `abs(P - 333.30)` | `<= 32.98 W` | the same | 8 W (325 W) | 4 | validation |
| Q3 | `abs(e - 220.02)` | `<= 15.43 Pa + S + T` | acceptance of F-870; ADR 0765's band form | 7 Pa | 2 | validation |
| Q4 | `abs(q - 99.20)` | `<= 15.01 Pa + S + T` | the same | 3 Pa | 5 | validation |
| Q5 | `c_x`, every one of 26 radii | `<= U_num(r) + S(r) + T(r)` | the same | about 12 radii miss, as in ADR 0765 | below 1: expected MISS | validation |
| Q6 | `c_t`, every one of 26 radii | the same | the same | the same | below 1: expected MISS | validation |
| Q7 | `abs(p_c - 113.90)` | `<= 22.34 Pa + S + T` | acceptance of F-870; ADR 0769 answer; ADR 0765's band form | 30 Pa (`Pi_s` about 65 Pa plus the radial rise of about 20 Pa that ADR 0769 found) | 0.75: expected MISS | validation |
| G | the other clauses of the acceptance: B1's mesh gates (83.37 deg against 70, skewness 3.765 against 2.0) and a registered `u_meas` | met | acceptance of F-870; ADR 0762 A3, open question 1 | missed, as recorded (no new mesh, F-877 not done) | 0 | solution verification |
| K | `abs(m(dt/2) - m_SIMPLEC)` | `<= 0.21190 N m`, `<= 29.75 Pa` | ADR 0762 band arithmetic; ADR 0827 T4 | 0.01 N m, 8 Pa | 20, 4 | solution verification |
| C1 | known signals: mean and half range of `2 + 0.37 sin` (`A` to 1e-3 relative, mean to 1e-12); the last flow-through time of a series of 5 498 steps is 917 steps from 4 582; `2 + 0.001 sin` settled, `2 + 0.05 sin` not (N1); `U_num` of `e`, `q`, `Pi_s` from the constants to 5e-4 Pa of 14.229, 12.936, 21.280; ADR 0765's band at r / 250 mm = 0.532 from its constants, 0.890 and 0.251, to 5e-4 m/s | as stated | `docs/process/tolerances.md`, "Agreement with a closed form"; ADR 0765's printed digits | exact | n/a | code verification |
| SR | 20 steps of M(dt) on the heavy path against the first 20 rows of `M_dt.csv` | `1e-6` relative | as ADR 0824 SR | 0 | n/a | code verification |
| H | every number of the rows above, as first computed, written as `RECORDED` after the first run of the rows test and held at `1e-6` relative, with the verdicts | `1e-6`; verdicts equal | as ADR 0827 G2 | 0 | n/a | unchanged default output |
| D1 | no file under `crates/*/src`; `docs/roadmap.json`, the ledger, goldens, existing tests unchanged | 0 files | rule 3; the brief | 0 | n/a | unchanged default output |
| B3 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes, file unchanged | rule 3; the goal | passes | n/a | unchanged default output |
| P | `rustfmt --check` of the new file (or `cargo fmt -p rcfd-api --check`); `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `rcfd-api --release --lib`; the new file; `--test f885_kept_gradient_lag` (the path that `run_h` copies); `baseline_hashes`; `docs_site`; `roadmap_proposals.py check`; `process_report.py --check` | clean; passes; only a generated file not current | rule 7 | as stated | n/a | code verification |

## Tests (fixed now)

One new file, `crates/rcfd-api/tests/f870_time_march_b1.rs` (header: ADR 0831, F-870), std and the crates `rcfd-api` uses:

* in the gate: `f870_tm_measures_on_known_signals` (C1, N1); `f870_tm_rows_from_the_committed_records` (R, S1, S2, Q1 to
  Q7, K, H; reads the four committed record files and prints every row; its output is saved as `rows.log`); in its first
  run `RECORDED` is `None`: it prints, asserts R and does not assert H; that is the registered run of the rows, made once;
  `f870_tm_short_run_holds_the_record` (SR, B1, about two minutes in release).
* `f870_tm_acceptance`: from the records, asserts S1, S2 and Q1 to Q7 with the limits above. If it misses it is kept,
  ignored, as `f870_tm_acceptance_known_miss` with its numbers in the reason (rule 2); its limits are not changed.
* heavy (`#[ignore]`), run once each, in the background from a copied test binary: `f870_tm_march_dt_record` and
  `f870_tm_march_dt2_record`, writing to the folder `F870_TM_OUT`.

The records go to `docs/investigations/f870-time-march-b1/` with the logs of the two runs and the load log.

## Negative controls and planned mutations

On a `git archive` copy of the test commit in the scratch folder, its own target folder; the heavy runs are not repeated.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | test: the last flow-through time taken from `t > 4 T_ft` | `f870_tm_measures_on_known_signals` (the window count) |
| M2 | test: the 1 % of S1 and S2 made 10 % | `f870_tm_measures_on_known_signals` (N1) |
| M3 | test: `dp_ts` read as torque | `f870_tm_rows_from_the_committed_records` (H) |
| M4 | record: `M_dt2.csv` cut to its first half | `f870_tm_rows_from_the_committed_records` (R) |
| M5 | test: `T` left out of the band | `f870_tm_measures_on_known_signals` (ADR 0765's band) |
| M6 | test set-up: `outer_frozen_convection_gradient_extrapolated_boundary` off | `f870_tm_short_run_holds_the_record` |
| M7 | test: the casing ring at 247.5 mm | `f870_tm_rows_from_the_committed_records` (H) |

Negative control N1: `2 + 0.05 sin` (2.5 %) is not called settled. The committed patch is M1
(`crates/rcfd-api/tests/f870_time_march_b1.mutation.patch`), first line
`# expect-fail: cargo test -p rcfd-api --release --target-dir target/mutation --test f870_time_march_b1 f870_tm_measures_on_known_signals`.

## Decision rule

* **Done** for F-870 if the march is converged by section 2 (S1, S2), every compared quantity Q1 to Q7 is within its
  band, the other clauses of the acceptance (G) are met, R, K, C1, SR, H, D1, B3 and P are met, and each of M1 to M7 is
  caught. G is part of the rule because rule 8 asks for partial where a clause of the acceptance is not met; the brief's
  rule names the march, the compared quantities and the mutations.
* **Partial** otherwise, with each miss recorded with its number. Expected: partial (Q1, Q5, Q6, Q7 and G missed).
* **Not deciding** for the convergence clause if a march fails or does not complete; the rows it does give are recorded.

## What is not covered

The mesh gates and a new mesh; `u_meas` and the density (F-877); a third step or a longer run; the default `pimple`
step; the time-marched B2 and B3 (the mesh term is the SIMPLEC family's); the off-design point; the outlet model (F-880).

## Open questions for the maintainer

To be completed in the addendum: any question of scope.
