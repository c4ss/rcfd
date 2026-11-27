# ADR 0827: F-879 registration: check H's class from ADR 0824's records, torque and pressure rise in time, and what it means for F-870

Status: **REGISTERED 2026-11-27, written before any code of F-879 for this registration and before any run of its checks.**
Before this text no test or script of this work was written or run. What was done: reading (below), one reading script over
`docs/roadmap.json` (F-879 in full), the first two lines of each record file (its header, no number of the window), and
the arithmetic of this page. Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f879-check-h-class.md`](briefs/f879-check-h-class.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. Branch `f879-check-h-class` from 2cbdfa5f; scratch
`w49i-work/f879-check-h-class-scratch`. Written by the model Claude Opus 5.5. No solver source is changed; no new run of
the solver is made (the records decide every row, below).

Size (agent): done; 0; 0; 2; 170000

## What is asked

F-879's acceptance (read by script): check H of ADR 0183 "is run as registered there, ... from the start state registered
there, and classes the reversed-flow pocket at the hub trailing edge as steady, physical unsteadiness or numerical by the
rule registered before the run, with the time step, the time run and the measured amplitude and frequency of torque and
pressure rise; the report states what the class means for the convergence rule of F-870. A run that does not decide is
recorded as not deciding." Its gap: a class by the registered rule. F-885 (ADR 0824) ran check H with the outer rule of
ADR 0183 as written (ADR 0767 E-1) and the opt-ins `alpha_free_face_flux`, `outer_frozen_limiter`,
`outer_frozen_convection_gradient`, `outer_frozen_convection_gradient_extrapolated_boundary`; its records are
`docs/investigations/f885-kept-gradient-lag/H_dt.csv` and `H_dt2.csv` (21 columns, one row per step).

## What was read

Whole: `AGENTS.md`; `docs/process/agent-rules.md` (version 5); the brief; [ADR 0766](0766-f879-registration-fan01-check-h-the-hub-pocket-in-time.md);
[ADR 0824](0824-f885-registration-the-lag-of-the-kept-convection-gradient-its-arms-a-corrected-variant-and-check-h.md).
In part: `crates/rcfd-api/tests/f885_kept_gradient_lag.rs` lines 1 to 62, 245 to 480, 596 to 640, 1198 to 1323 (the
constants, `Row`, `measure`, `summarise`, the record form, `read_full`, G1); [ADR 0762](0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md)
lines 165 to 235 (the band, the convergence rule, the tolerance of the state) and its R1 and R2 rows;
[ADR 0767](0767-f885-registration-check-h-by-the-velocity-change-of-the-outer-iterations.md) section 3 and E-1;
`docs/process/tolerances.md`, the table; `docs/validation-data.md` lines 82 to 111 (FAN-01's design point).
Not read: the logs of the two records, and every number of the records' rows. What ADR 0824 printed of them, and so known
before this registration: share of the outer rule 1.000000 in both runs, site amplitude 1.2557e-3 and 2.2775e-3 m/s,
shaft power 313.56 and 313.96 W (2.01499 and 2.01756 N m), class STEADY and verdict Steady.

No published source outside the repository is used. The measures are from first principles (a half range, a least-squares
line, a discrete Fourier sum); the limits are rows of the repository's records, named per row.

## 1. The time scales (arithmetic, fixed now)

`Omega = 1486 x 2 pi / 60 = 155.6136 rad/s`; a revolution 40.377 ms; blade passing `9 x 1486 / 60 = 222.90 Hz`, period
4.4863 ms; a flow-through time `T_ft` = the run's length over six (ADR 0183 D5) = 42.197 ms. Each run is 0.2531829 s
(5 498 steps of 4.605e-5 s; 10 996 of 2.3025e-5 s): 6.00 flow-through times, 6.270 revolutions, 56.43 blade-passing
periods. The window (ADR 0766: the rows with `t > 0.1266 s`) is 2 749 and 5 498 steps, length `L = n dt` = 0.126591 s:
3.00 flow-through times, 3.135 revolutions, 28.22 blade-passing periods. A single passage with axisymmetric inflow in the
rotating frame has no blade-passing forcing; the period is given as a scale only.

## 2. The measures (fixed now)

For each run, over the window, for the shaft torque (`torque`, N m, column 9), the total-to-static pressure rise
(`dp_ts`, Pa, column 10) and the site cell of largest half range (ADR 0766's signal):

* mean `m`; half range `A = (max - min) / 2`; peak-to-peak `2A`; relative half range `A / abs(m)` (torque and `dp_ts`);
* `f_x`: upward crossings of the mean-removed signal and `(n - 1) / (t_n - t_1)` (ADR 0766 D-5, unchanged);
* the periodogram: the signal less its least-squares line over the window, `P_k = abs(sum_i x_i exp(-2 pi i k i / n))^2`
  for `k = 1 .. floor(n / 2)`; the dominant frequency `f_p = k* / L` of the largest `P_k`; resolution `1 / L` = 7.8994 Hz;
  highest frequency `1 / (2 dt)` = 10 858 Hz (dt) and 21 716 Hz (dt/2); the share `P_k* / sum P_k` reported. The line is
  removed so that a slow drift, which ADR 0766 found at the first bin, does not stand for the frequency; the drift is
  reported as the line's change across the window;
* the decay: `A` of each signal over each flow-through time `j = 1 .. 6` (`t` in `((j - 1) T_ft, j T_ft]`), and `A / abs(m)`
  over the last;
* shaft power `Omega m` of torque.

## 3. The rows and their limits (fixed now)

* **G (class).** From the two records: rows 5 498 and 10 996 with step `1 .. N`, window 2 749 and 5 498; the share of window
  steps with `d5 <= 1e-2 d1` (E-1) at least 0.99 in both (TIME-ACCURATE), and the site `A <= 0.193 m/s` in both: class
  STEADY by ADR 0183 (its first two clauses, in order), verdict steady by ADR 0766's table. Any other outcome is recorded
  as it comes (NOT STEADY is a row decided, and the verdict is then read by ADR 0824's G1, which stays).
* **T1, T2 (torque, pressure rise steady to the state).** `A / abs(m) <= 1 %` in each run. Source: ADR 0762, "The tolerance
  of the state" (1 % for a state of this passage). Met: the signal has no oscillation of the flow above the tolerance of
  the state, and its `f_p` is the frequency of a residual below it, reported as such. Missed in both runs: row **TF**
  applies, `f_p(dt/2) / f_p(dt)` in [0.9, 1.1] (ADR 0183, check H, the frequency band); missed in one run only: recorded
  as not deciding for that signal.
* **T3 (time step).** `abs(m(dt/2) / m(dt) - 1) <= 1 %` for torque and for `dp_ts`. Source: ADR 0762, the tolerance of the
  state (two samples each defined to 0.5 %). The Richardson value `m(dt/2) + (m(dt/2) - m(dt)) / 3` (BDF2, order 2) is
  reported beside, no limit.
* **T4 (route).** `abs(m(dt/2) - m_B2)` against the steady B2 run of ADR 0762 (R1: 2.028692 N m, 64.2329 Pa): at most
  `U_num` = 0.21190 N m and 29.75 Pa. Source: ADR 0762, the band arithmetic (the numerical uncertainty of the
  B1/B2/B3 family). Read: the time-marched and the SIMPLEC states differ by less than the mesh family's uncertainty.
* **T5 (measured).** `m(dt/2)` beside FAN-01's 126.46 Pa and 333.30 W (`docs/validation-data.md`), the difference reported.
  No limit: B2 is not F-870's mesh and ADR 0762 found no stated uncertainty of the measurement.
* **K1 (two routes).** The measures applied to ADR 0766's records (`docs/investigations/f879-fan01-check-h/`, 15 columns)
  give its printed torque and `dp_ts` values: means 2.007096 and 2.010156 N m, 69.6172 and 70.0729 Pa; `A` 2.83e-4 and
  3.53e-4 N m, 0.0212 and 0.0364 Pa; crossings 42 and 161, 41 and 106; `f_x` 1009.5 and 1307.6 Hz, 395.1 and 957.5 Hz.
  Limit: each within half a unit of its last printed digit, crossings exact. Source: "Same discrete problem by two
  routes", widened to the printed digits (as ADR 0766 G1).
* **K2.** ADR 0824's printed values from its records: share 1.000000 in both, site `A` 1.2557e-3 and 2.2775e-3 m/s, power
  313.56 and 313.96 W, each within half a unit of its last printed digit. Same source.
* **C1 (known signals).** On `n = 2 749`, `dt = 4.605e-5`: (a) `0.37 sin(2 pi 17 t / L)`: `f_p = 17 / L` to `1e-12`
  relative (exact for the discrete sum), `A` to `1e-3` (sampled maximum, exact for the continuum only); (b) the same plus a
  line rising `10 x 0.37` across the window: `f_p = 17 / L` to `1e-12`; (c) `2 + 0.001 sin` meets the 1 % of T1, `2 + 0.05
  sin` (2.5 %) does not; (d) a constant: `A = 0`. Source: `docs/process/tolerances.md`, "Agreement with a closed form".
* **G2 (hold).** Every number of the rows above, as first computed, written into the test as `RECORDED` after its first
  run and held at `1e-6` relative (the same files read twice; as ADR 0766 G1). In the first run `RECORDED` is `None`: the
  test prints, asserts the limited rows and does not assert G2; that is the registered run, made once.
* **D1.** No file under `crates/*/src` changes; `docs/roadmap.json`, the ledger, goldens and existing tests unchanged.
* **B3.** `cargo test -p rcfd-api --release --test baseline_hashes` passes, the file unchanged (the goal asks for it; no
  shared solver file is touched).

## Table of checks

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | check H run as registered (ADR 0183, 0766, 0767 E-1), both complete | 5 498 and 10 996 rows, steps 1 to N | acceptance of F-879; ADR 0824 A3 | complete | n/a | solution verification |
| G | class and verdict from the records | TIME-ACCURATE (0.99) and site `A <= 0.193` in both: STEADY | acceptance of F-879; ADR 0183; ADR 0766; ADR 0767 E-1 | STEADY | 85 | solution verification |
| A3 | time step, time run, amplitude and frequency of torque and pressure rise | reported, section 2 | acceptance of F-879 | as stated | n/a | solution verification |
| T1 | torque `A / abs(m)` each run | `<= 1 %` | ADR 0762, the tolerance of the state | 0.02 % | 50 | solution verification |
| T2 | `dp_ts` `A / abs(m)` each run | `<= 1 %` | the same | 0.05 % | 20 | solution verification |
| TF | only if T1 or T2 is missed in both runs: `f_p` ratio | [0.9, 1.1] | ADR 0183, check H | not expected to apply | n/a | solution verification |
| T3 | means, dt against dt/2 | `<= 1 %` each | ADR 0762, the tolerance of the state | 0.15 %, 0.65 % (ADR 0766) | 6.7, 1.5 | solution verification |
| T4 | `m(dt/2)` against steady B2 | `<= 0.21190 N m`, `<= 29.75 Pa` | ADR 0762, the band arithmetic | 0.02 N m, 6 Pa | 10, 5 | solution verification |
| T5 | `m(dt/2)` against FAN-01 | reported | none (no stated uncertainty) | about 6 % low in power, 45 % in pressure rise | n/a | solution verification |
| A5 | what the class means for F-870 | stated, section 4 | acceptance of F-879 | as stated | n/a | solution verification |
| K1 | ADR 0766's printed torque and `dp_ts` values from its records | half a unit of the last digit; crossings exact | "Same discrete problem by two routes" at the printed digits | 0 | n/a | code verification |
| K2 | ADR 0824's printed values from its records | half a unit of the last digit | the same | 0 | n/a | code verification |
| C1 | known signals (a) to (d) | `1e-12`; `1e-3`; as built | "Agreement with a closed form" | 1e-14; 2e-6 | 1e2; 5e2 | code verification |
| G2 | the rows as recorded | `1e-6` relative | as ADR 0766 G1 | 0 | n/a | unchanged default output |
| D1 | no source, roadmap, ledger, golden or existing test changed | 0 files | rule 3; the brief | 0 | n/a | unchanged default output |
| B3 | `baseline_hashes` | passes, file unchanged | rule 3; the brief | passes | n/a | unchanged default output |
| P | fmt and clippy of `rcfd-api`; `rcfd-api --lib`; the new file; `--test f885_kept_gradient_lag` (it reads the same records); `baseline_hashes`; `docs_site`; `roadmap_proposals.py check`; `process_report.py --check` | clean; passes; only a generated file not current | rule 7 | as stated | n/a | code verification |

No row is of kind validation: F-879 claims no agreement with a measured case (T5 is reported only).

## Tests (fixed now)

One new file, `crates/rcfd-api/tests/f879_check_h_class.rs` (header: ADR 0827, F-879), std only, no solve:
`f879_hc_measures_on_known_signals` (C1), `f879_hc_reproduces_the_printed_values` (K1, K2),
`f879_hc_rows_from_the_committed_records` (A1, G, T1 to T5, G2; it prints every row, and its output is saved as
`docs/investigations/f879-check-h-class/rows.log`). All three in the gate (a few seconds).

## Negative controls and planned mutations

On a `git archive` copy of the test commit in the scratch folder, its own target folder:

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | test: the least-squares line not removed before the periodogram | `f879_hc_measures_on_known_signals` (b) |
| M2 | test: the 1 % of T1 and T2 made 10 % | `f879_hc_measures_on_known_signals` (c) |
| M3 | test: the window start 0.1266 made 0.0633 s | `f879_hc_rows_from_the_committed_records` (window count) |
| M4 | test: the periodogram's frequency `(k + 1) / L` | `f879_hc_measures_on_known_signals` (a) |
| M5 | test: `dp_ts` read as torque (column 10 for 9) | `f879_hc_reproduces_the_printed_values` |
| M6 | record: ADR 0824's `H_dt2.csv` cut to its first half | `f879_hc_rows_from_the_committed_records` |

Negative control N1: C1 (c), the 2.5 % signal, is not called steady. The committed patch is M1
(`crates/rcfd-api/tests/f879_check_h_class.mutation.patch`), first line
`# expect-fail: cargo test -p rcfd-api --release --target-dir target/mutation --test f879_check_h_class f879_hc_measures_on_known_signals`.

## 4. What the class means for F-870 (options fixed now)

F-870's rule (ADR 0762): `is_converged()` at 1e-5 within 8000 iterations; the B1 SIMPLEC solve misses it (`res_u`
1.208e-3), a limit cycle of the steady iteration. Options, each with what it needs:

* (a) keep the residual rule: needs a steady iteration that converges on B1; none exists on main.
* (b) the residual rule replaced by the cap rule of ADR 0075 and 0086 (functional change over the last 1000 iterations
  within 0.5 %) on the SIMPLEC sample: needs the sample shown within a stated band of the time-marched state of the same
  mesh; T4 gives that number on B2 only.
* (c) a time march to a settled state on B1 (`pimple` with the four opt-ins of ADR 0824, uniform start, `dt` and `dt/2`):
  converged iff `A / abs(m)` of torque and `dp_ts` over the last flow-through time `<= 1 %` and the two steps' means within
  1 % (T1 to T3 as limits); needs two B1 runs (hours) under a registration.
* (d) a time mean over a stated number of periods: needed only for a class PHYSICAL.

Recommendation rule, fixed now: G STEADY with T1 to T3 met: (c), with (b) named as the cheaper route where T4 is met;
T3 missed: (c) with a halved step pair; G not STEADY: no recommendation beyond ADR 0824's G1.

## Decision rule

* **Done** for F-879 if every row is decided (G, A1, A3, A5, T1 to T5, TF if it applies), the limited rows G, T1 to T4, K1,
  K2, C1, G2, D1, B3 and P are met, and each of M1 to M6 is caught.
* **Partial** otherwise, with the row or mutation that misses.
* **Stopped and reported** if the records cannot be read as their headers say.

## What is not covered

A new run of any kind; B1; the default `pimple` step (the class holds with the four opt-ins); the scheme and the mesh in
time (ADR 0766); any agreement with the measurement.

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation: F-870's convergence rule (section 4) and the scope of
the class (the opt-ins).

## Addendum 1: results

Written 2026-11-27, after the run. Commits on the branch, from 2cbdfa5f: 1a7ec717 (this registration, alone), b81bdfdd
(the test file; the registered run was made from it, once, with `RECORDED` `None`), f392a716 (`RECORDED` written from that
run's printed keys, and its output as `rows.log`), 2842efc3 (the committed mutation M1 and `mutations.log`), and the commit
of this addendum with `rule7.log` and the proposal file. Records in `docs/investigations/f879-check-h-class/`.

### The sentence

**From ADR 0824's records the hub pocket classes STEADY by ADR 0183's rule, and torque and pressure rise are steady to the
1 % tolerance of the state in both runs; the time step changes their means by 0.13 % and 0.29 %. For F-870 a steady state
exists to converge to.** One finding beside the class: H(dt) keeps a small oscillation of torque and pressure rise at
2583 Hz (8.4 steps a period, half range 0.10 % and 0.18 % of the mean) that does not decay over the last three
flow-through times; at dt/2 it is absent and every signal decays. It is tied to the time step and below the tolerance of
the state; it is a residual of H(dt), not a class of the pocket (the site signal is steady in both).

### Torque and pressure rise in time (A3)

Time step 4.605e-5 s and 2.3025e-5 s; time run 0.2531829 s each (6.00 flow-through times, 6.270 revolutions, 56.43
blade-passing periods); window 2 749 and 5 498 steps, 0.126591 s (3.00 flow-through times, 3.135 revolutions), periodogram
resolution 7.8994 Hz, highest 10 858 and 21 716 Hz.

| | H(dt) | H(dt/2) |
| --- | --- | --- |
| torque: mean; shaft power | 2.0149759 N m; 313.558 W | 2.0175481 N m; 313.958 W |
| torque: half range (peak-to-peak); share of the mean | 2.0905e-3 (4.1809e-3) N m; 1.0375e-3 | 4.7505e-4 (9.5010e-4) N m; 2.3546e-4 |
| torque: crossings; `f_x`; `f_p` (share of the periodogram) | 327; 2583.55 Hz; 2583.11 Hz (0.197) | 13; 5642.3 Hz; 7.90 Hz (0.774, the drift) |
| torque: drift across the window | 7.27e-4 N m | 6.49e-4 N m |
| torque: half range in flow-through times 1 to 6 | 30.3; 4.97e-3; 2.55e-3; 1.87e-3; 1.81e-3; 1.71e-3 N m | 60.3; 3.80e-3; 1.33e-3; 3.94e-4; 8.17e-5; 3.15e-5 N m |
| pressure rise: mean | 71.25507 Pa | 71.46433 Pa |
| pressure rise: half range (peak-to-peak); share of the mean | 0.12759 (0.25518) Pa; 1.7906e-3 | 0.015676 (0.031353) Pa; 2.1936e-4 |
| pressure rise: crossings; `f_x`; `f_p` (share) | 327; 2582.55 Hz; 2583.11 Hz (0.226) | 19; 1833.8 Hz; 7.90 Hz (0.817, the drift) |
| pressure rise: half range in flow-through times 1 to 6 | 680; 0.250; 0.119; 0.116; 0.111; 0.112 Pa | 1387; 0.159; 0.0279; 0.0108; 4.81e-3; 2.35e-3 Pa |
| last flow-through time, half range over the window mean: torque; pressure rise | 8.46e-4; 1.58e-3 | 1.56e-5; 3.29e-5 |
| site: cell; half range; `f_p` | c157; 1.2557e-3 m/s; 7.90 Hz (drift) | c157; 2.2775e-3 m/s; 7.90 Hz (drift) |

The 2583 Hz of H(dt) is 11.6 times the blade-passing frequency and has no forcing in a single passage; its period is
8.41 steps of H(dt), and H(dt/2) shows nothing at 2583 Hz or its double. The 7.90 Hz peaks are the first bin, the
curvature of a slow drift that the least-squares line does not remove: no oscillation. Where the largest unsteadiness of
H(dt) sits is not in these records; ADR 0824 found its five cells of largest amplitude at the tip (1.54 m/s in H(dt),
0.126 m/s in H(dt/2)), which fits a step-tied tip oscillation; not diagnosed here.

### Every check

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| A1 | 5 498 and 10 996 rows, steps 1 to N | 5 498 and 10 996, complete; windows 2 749 and 5 498 | yes |
| G | TIME-ACCURATE and site `A <= 0.193` in both | share 1.000000 and 1.000000; `A` 1.2557e-3 and 2.2775e-3 m/s: STEADY, verdict steady | yes |
| A3 | reported | the table above | yes |
| T1 | `<= 1 %` | torque 0.104 % and 0.0235 % | yes |
| T2 | `<= 1 %` | pressure rise 0.179 % and 0.0219 % | yes |
| TF | only if T1 or T2 missed in both | does not apply (ratios 0.0031, printed only) | n/a |
| T3 | `<= 1 %` each | torque 0.128 %, pressure rise 0.294 %; Richardson 2.0184055 N m (314.09 W), 71.534 Pa | yes |
| T4 | `<= 0.21190 N m`, `<= 29.75 Pa` | torque -0.01114 N m (-0.55 %), pressure rise +7.231 Pa (+11.26 %) | yes |
| T5 | reported | power 313.958 W against 333.30 W (-5.80 %); pressure rise 71.464 Pa against 126.46 Pa (-43.49 %) | reported |
| A5 | stated | "What this means for F-870" below | yes |
| K1 | half a unit of the last digit; crossings exact | all sixteen values of ADR 0766 reproduced (for example 2.007096405 N m against 2.007096; 395.139 Hz against 395.1) | yes |
| K2 | the same | share 1.000000; site 1.2557175e-3 and 2.277452e-3 m/s; 313.5576 and 313.9578 W | yes |
| C1 | `1e-12`; `1e-3`; as built | (a) `f_p` equal to `17 / L` to the 13 printed digits, `A` 0.36999994; (b) equal; (c) 5.0e-4 steady, 2.5e-2 not (N1); (d) 0 | yes |
| G2 | `1e-6` relative | the 28 keys as recorded (the rule 7 run) | yes |
| D1 | 0 files | `git diff --name-only 2cbdfa5f`: the new test, its patch, ADR 0827, the records folder and the proposal file only | yes |
| B3 | passes, unchanged | 3 passed, 1 ignored; file unchanged | yes |
| P | rule 7 | below | yes |

### What this means for F-870 (A5)

The class is STEADY, so a steady state of B2 exists, and a time march reaches it: the steady iteration's limit cycle (ADR
0110, ADR 0762) is a property of the iteration. By the recommendation rule registered in section 4 (G STEADY, T1 to T3
met) the recommendation is option (c), a time march to a settled state on B1 at two steps, with (b) named as the cheaper
route because T4 is met. Against (b): the route moves the pressure rise by 7.2 Pa (11 %) on B2, inside the family's
numerical band but a quarter of it, so the compared number depends on the route. The residual of H(dt) argues for
running (c) at two steps and reading the settled rule at both.

### Mutations

From `mutations.log` (`git archive` of f392a716, one copy, the touched file restored between mutations):

| Id | What happened |
| --- | --- |
| M1 | no least-squares line: caught by `f879_hc_measures_on_known_signals` (b). Committed as the patch |
| M2 | 10 % for 1 %: caught by the same test (c) |
| M3 | window from 0.0633 s: caught by `f879_hc_rows_from_the_committed_records` (A1: the window) |
| M4 | `(k + 1) / L`: caught by the known signals (a), 142.19 against 134.29 Hz |
| M5 | `dp_ts` read as torque: caught by `f879_hc_reproduces_the_printed_values` (69.617 against 2.007096) |
| M6 | ADR 0824's `H_dt2.csv` cut to its first half: caught by the rows test (an empty window fails the measure's assertion) |

Six of six caught.

### Deviations

1. M6 was caught by the measure's assertion on an empty window, not by the count assertion: the kept half of H(dt/2)
   ends at 0.1266 s, so its window is empty.
2. `cargo clippy` ran in the dev profile (its default); the tests in release.
3. No limit was changed and no test was run again to pass; the registered run was made once, and G2 was asserted first in
   the rule 7 run, as registered.

### What was not run

No solver run (none was registered). `docs_site`, `roadmap_proposals.py check` and `process_report.py --check` run at
the commit of this addendum and are reported to the lead.

### The checks of rule 7

`rule7.log` (at 2842efc3; the later commit changes only this ADR, the proposal file and `rule7.log`): `cargo fmt -p
rcfd-api --check` exit 0; `rustfmt --check` of the new file exit 0; `cargo clippy -p rcfd-api --all-targets -- -D
warnings` clean; `rcfd-api --release --lib` 160 passed, 4 ignored; `--test f879_check_h_class` 3 passed;
`--test f885_kept_gradient_lag` 6 passed, 4 ignored; `--test baseline_hashes` 3 passed, 1 ignored.

### Size

Size (agent): done; 0; 0; 2; 170000

### Decision by the registered rule

**Done.** Every row is decided (G, A1, A3, A5, T1 to T5; TF does not apply), the limited rows G, T1 to T4, K1, K2, C1, G2,
D1, B3 and P are met, and each of M1 to M6 is caught.

## Open questions for the maintainer (addendum 1)

1. **The convergence rule of F-870.** Options: (a) keep `is_converged()` at 1e-5 (needs a steady iteration that converges
   on B1; none exists); (b) the cap rule on the SIMPLEC sample, with the time-marched state of the same mesh beside it
   (T4: 0.55 % in torque, 11 % in pressure rise on B2); (c) a time march to a settled state on B1, `pimple` with the four
   opt-ins of ADR 0824 from the uniform start at `dt` and `dt/2`, converged iff the half range of torque and pressure rise
   over the last flow-through time is at most 1 % of the mean and the two steps' means agree within 1 % (two runs of B1,
   hours); (d) a time mean (only for a class PHYSICAL; not supported). I recommend (c), by the rule registered in section 4.
2. **Scope of the class.** The class STEADY holds with the four opt-ins of the pimple step, not with the default step
   (ADR 0767 found the default step not TIME-ACCURATE by E-1). Options: (a) F-879 is done with the class under the
   opt-ins, as F-885 was (ADR 0824, answer 2); (b) F-879 waits for a default step that meets E-1. I recommend (a).

## For the lead: roadmap entries

> Status proposed for F-879: done.

> Note entry: ADR 0827, 2026-11-27, done: from the committed records of ADR 0824 (check H of ADR 0183 on B2, uniform start, pimple with 5 outer iterations and the four opt-ins of F-885, dt 4.605e-5 s and dt/2, 0.2532 s or 6.27 revolutions each, window the last 3 flow-through times), the hub pocket classes STEADY (outer rule met in 100 % of both windows, site amplitude 1.26e-3 and 2.28e-3 m/s against 0.193); torque 2.01498 and 2.01755 N m (313.56 and 313.96 W), half range 0.10 % and 0.024 % of the mean, pressure rise 71.255 and 71.464 Pa, half range 0.18 % and 0.022 %, so both steady to the 1 % state tolerance of ADR 0762; H(dt) keeps a non-decaying residual at 2583 Hz (8.4 steps a period) that is absent at dt/2, where everything decays and the periodogram peak is the drift at 7.9 Hz; the two steps agree within 0.13 % and 0.29 %, and against the steady B2 run of ADR 0762 by -0.55 % in torque and +11.3 % in pressure rise, inside its numerical band; for F-870 a steady state exists, and the recommended convergence rule is a settled time march on B1 at two steps; held by crates/rcfd-api/tests/f879_check_h_class.rs and docs/investigations/f879-check-h-class.

The same entries are in `docs/roadmap/proposals/0827-F-879.json` (form 1 of ADR 0755, base 2cbdfa5f), checked with
`python scripts/roadmap_proposals.py check`. No gap (done) and no follow-up item.

Generated pages that this work may leave not current, for the lead (rule 9): `docs/adr/index.md` (ADR 0827 is new).

## Answers of the maintainer (2026-11-27), written by the lead

1. F-870's convergence rule is option (c): a time march to a settled state on B1 at dt and dt/2, by the limits of section 4; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
2. F-879 is done with the class STEADY under the four opt-ins of ADR 0824, as for F-885; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).

Merged and pushed at d7209cc2.
