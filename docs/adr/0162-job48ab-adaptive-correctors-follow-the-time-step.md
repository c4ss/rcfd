# ADR 0162: Job 48AB, the adaptive pressure-corrector count is re-estimated when the time step has changed by a factor 1.5 (the dt trigger)

Status: **registered** 2026-10-29, before any code of this job was written and before any run of this job (set-ups, limits,
reading rules and the cost bound are fixed here); **results in the addendum** (2026-10-29: every registered limit met except N3 and
N6 as worded, both only because of a failure of `case_hashes_w48u` that is identical at 444255c; the control B in class
B-iii, the caveat not reproduced as an instability); no amendment; not yet accepted by the maintainer. House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed before the registered runs; a missed limit
becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before the corrected
run; defaults stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no unsafe code, no new crate,
the crate graph stays acyclic; no edit of `crates/rcfd-studio` or of the `rcfd-runner` schema. Clean-room: first principles,
the repository's own ADRs and code. Parents: [ADR 0156](0156-job48v-adaptive-pressure-correctors.md) (the adaptive count, D2-D4,
its harness and records), [ADR 0158](0158-job48x-studio-run-plan-adaptive-pressure-correctors.md) (the Studio turns it on; its
finding "the count is chosen at the automatic step's first dt" and question 2, option (b)),
[ADR 0153](0153-job48s-time-step-set-when-a-run-starts.md) (the Studio's automatic step),
[ADR 0154](0154-job48t-cylinder-ogrid-pressure-spokes.md) (DG1, `M_hf`, L3). Number: no ADR 0159 or above exists in this
worktree or any sibling worktree on disk (checked 2026-10-29); the brief names 0162 (0159-0161 are expected from the concurrent
jobs).

## The request

ADR 0158 question 2 (b), as the brief states it: re-run the pressure-lag estimate and re-choose the corrector count whenever
dt has changed by a factor >= 1.5 (up or down) since the last estimate, in addition to the existing triggers; only with
`solver.corrector_control = "adaptive"`; a run whose dt never changes stays bit-identical to before (adaptive runs with a fixed
dt included). The maintainer's requirement: stable whatever the resolution and whatever dt the controller picks, without
hand-tuning.

## Context (read at local main 444255c; the worktree started at 1248a76 and was fast-forwarded to 444255c before any work)

* ADR 0156 D3: estimates at the first step (step 2 under bdf2), every 1000 steps, and after the growth monitor fired (it fired
  in no run). ADR 0158 X5 (a): the Studio plan on 256 x 256 (`solver.end_time=3`, `solver.dt=0.00116`,
  `solver.max_courant=0.5`, `solver.max_dt=0.03`, adaptive) took 891 steps with one estimate (step 1: 2 correctors, rho 0.406)
  and stayed clean (M_hf 1.17e-7 at t = 3); ADR 0153 D2: dt 0.00116 to 0.00354 s over the same 3 s. DG1 (ADR 0154): two
  correctors at dt 1/256 = 0.0039 s give 1.166 at step 1. So the run spends most of its steps at a dt where its count's
  estimate is near or above 1 (not measured at the grown step itself; this job measures it).
* The controller (`rcfd-fv` `stepctl`): `plan` lands a step on the next **write** time or the end time when it lies within 1.05
  of the proposal (`land`); `output.store_time_interval` is not a landing target (`next_target` reads `write_times` and
  `write_time_interval` only); the next proposal is computed from the unshortened proposal `dt_next` (`propose_courant(st, d,
  dt_prop)` scales from `dt_prop`), so a shortened landing step is a single short step and the step after it is of the
  ordinary size. The fixed step (`sim.rs` `fixed_step`, ADR 0145) can make the last step short when `end_time` is not a whole
  number of steps.
* The shipped cylinder case keeps its own `output.store_time_interval = 1.0` (the Studio adds no store key for it, ADR 0152),
  so a Studio run of it stores once per D / U.
* TV256 (ADR 0156 A7): the first estimate's search over 2, 3, 4, 5 correctors (14 correctors x 20 iterations = 280 lag-map
  corrector applications) took 52.4 s on one thread, where a 5-corrector step took 1.057 s: about 0.19 s per application, about
  the cost of one corrector of a real step.

## Decisions

**D1. The dt trigger.** `CorrectorControl` gets `dt_factor` (registered **1.5**; 0 = no dt trigger, ADR 0156's behaviour).
At a step at or after the first estimate, the control makes an estimate when the step's dt satisfies `dt >= 1.5 dt_e` or
`dt_e >= 1.5 dt`, where `dt_e` is the dt of the step at which the last estimate (of any kind) was made. Checked after ADR 0156's
first-step, 1000-step and monitor triggers (an estimate due for one of those keeps its own reason). Only with
`corrector_control = "adaptive"`: with `fixed` the control is `None` and no code of this job runs.

**D2. Landing steps are skipped.** A step whose dt differs from the step size the time control proposed for it (a step shortened
or stretched by the controller's `land` onto a write time or the end time; the fixed step's short last step; the min with the
end time of the legacy Courant control) is not compared: `PimpleSolver::correctors.mark_landing_step()` before the step, called
by `rcfd-api` `Simulation::step` when the step's dt is not bit-equal to the proposal; `PimpleSolver::run_fixed` marks its own
shortened last step. Such a step neither fires the dt trigger nor becomes `dt_e`; the next ordinary step is compared. (A
landing step that is also a first-step, 1000-step or monitor estimate is estimated as in ADR 0156.) Why: a landing step at the
end of a run would otherwise re-estimate on its last step (cost, and a different last step for a fixed-dt adaptive run whose
`end_time` is not a whole number of steps), and write-time landings would cost two estimates each.

**D3. The search of a dt-fired estimate.** As ADR 0156 D3.3's later estimates (from the count in use: count up while above
0.5; else one step down when `n - 1` is at or below 0.5), except that an estimate fired by an **increase** of dt makes no
step-down check (the step grew, so the count in use is the floor; saves one evaluation). 20 power iterations, warm start from
the previous estimate's mode, target 0.5, cap 10: unchanged.

**D4. Records.** `CorrectorEstimate` gains `dt` (the step's dt), `dt_triggered` (fired by D1) and `work` (lag-map corrector
applications of the search: the sum over its evaluations of count x iterations; deterministic). `CorrectorRecord` gains
`dt_triggered` (count). `triggered` keeps meaning the growth monitor only (a dt-fired estimate does not set its gap). The run
summary (`Simulation::corrector_summary`, the `finish` event's `correctors`) gains `dt_triggered`, and each `log` entry gains
three elements after the existing five: `[step, correctors, rho, cap_bound, triggered, dt, dt_triggered, work]` (readers that
index positions 0 to 4 are unchanged). No wall-clock time enters `rcfd-fv` or any event (cost is measured in the harness, E).
A `fixed` run's events are unchanged (ADR 0156 N5).

**D5. Test switch and diagnostic (`rcfd-api` `Simulation`).** `set_corrector_dt_factor(f) -> bool` sets `dt_factor` of an
adaptive run (0 turns the dt trigger off; the control B); `lag_estimate_next_step(iterations) -> Option<LagPreflight>`: on a
copy of the solver (the simulation is not changed), the ADR 0073 estimate at the next step (the controller's planned dt, else
the last step's, else `solver.dt`) with the count in use (the adaptive control's current count, else `n_correctors`) and
`iterations` cold power iterations (`PimpleSolver::lag_trial`, built on ADR 0156's `lag_preflight`). Neither is reachable from a
case file, the command line or the Studio.

**D6.** `CorrectorControl::registered()` includes `dt_factor` 1.5, so every adaptive run (the case key, the Studio's override)
gets the trigger; its doc names ADR 0162. Nothing else of ADR 0156's control changes.

## Predictions

* Fixed-dt adaptive runs (every ADR 0156 record) and every `fixed` run: bit-identical (no dt change; short last steps skipped).
* 256 x 256 under the Studio plan: step 1 keeps 2 (0.406, as X5); the controller grows dt by 1.2 per step, so the trigger fires
  about step 4 (dt about 1.7 dt0) and about step 7 (about 3 dt0), taking 3 and then 3 or 4; afterwards dt stays in about
  0.0029-0.0036 s (within 1.5 of the last `dt_e`), so no further dt-fired estimate: **2 to 3 dt-fired estimates, all in the first
  20 steps**. The 1000-step estimates confirm or lower by one.
* 64 x 48 and 128 x 96: 2 and 2 or 3 at the grown step (64 x 48 at 1/16 took 2 with 0.358; the automatic step there is 0.006 to
  0.018 s, smaller).
* The control (trigger off): 2 correctors to step 1000 (t about 3.3), then the 1000-step estimate re-chooses (expected 3 or
  4). Its cold 60-iteration estimate with 2 correctors at the grown step: about 0.9 to 1.2 (DG1's 1.166 at 0.0039 s on the
  step-1 flow; the grown step is 0.0029-0.0036 s). Spokes within t <= 3.3: **not expected** (X5 (a) was clean at t = 3; an
  amplification just above 1 needs many steps to lift the mode from round-off, and the ADR 0154 failing arms had >= 2.4).
  This is the least certain prediction; B's reading rule below covers each outcome.

## Registered checks and limits

Common: release build of this branch; in-process through `Simulation` (the control needs D5's switch, so every run of A-C uses
the same harness, `crates/rcfd-api/tests/w48ab_dt_trigger.rs`); the case is the shipped `cylinder-re100.toml` with the
**Studio plan's full `--set` list** applied as tree overrides: `solver.end_time=N`, `solver.dt=dt0`, `solver.max_courant=0.5`,
`solver.max_dt=N/100`, `solver.corrector_control=adaptive`, with dt0 ADR 0153's values (64 x 48: 0.00602, 128 x 96: 0.00307,
256 x 256: 0.00116), plus `mesh.nx`, `mesh.ny`. The case's own `store_time_interval` 1.0 stays, so the **stored times** are the
steps at which the time first reaches k x 1.0 (where `rcfd run` would store). `M_hf` is the Rust `m_hf` of
`w48t_cylinder_spokes.rs` (copied into the harness), on the f64 pressure. Each run writes a status-like stream
(`iteration` events with `index`, `time`, `dt`, `monitors.cd`, `monitors.cl`, `correctors`, `wall_s`; `store` events with
`M_hf`; `lag_trial` events; a `finish` event with the summary) that `scripts/w48v_shedding.py` reads unchanged. Thread counts
do not change results (`docs/run-control.md`).

### N: defaults and fixed-dt adaptive runs unchanged

| Id | Check | Limit |
|---|---|---|
| N1 | `w33b_default_bits` | passes unchanged |
| N2 | `baseline_hashes` | as at 444255c on this machine (its `kepsilon_channel` case is known to fail locally at every commit; any other case passes); the file is not edited |
| N3 | `case_hashes_w48u` | passes |
| N4 | Fingerprints (FNV-1a of the bits of ux, uy, p and the face flux, plus positions 0-4 of the corrector log) printed by `crates/rcfd-api/tests/w48ab_fingerprints.rs` (an ignored printer that builds on 444255c unchanged), run on a `git archive` copy of 444255c and on this branch: F1 the shipped case adaptive at dt 1/16 to t = 10 (ADR 0156 A1); F2 K1's set-up adaptive (64 x 256, 1/64) to t = 3 (A2); F3 the shipped case adaptive at dt 1/16 to `end_time` 1.03 (a short last step, D2); F4 the shipped case (fixed) to t = 3; F5 the shipped case, fixed correctors, under the Studio's time entries (automatic dt) to t = 3 | the five fingerprints equal; F2 also equals ADR 0156's record (M_hf 5.707e-3, count 8, 0.4707) |
| N5 | ADR 0156's own tests: `crates/rcfd-fv/tests/w48v_corrector_control.rs` (5), `crates/rcfd-api/tests/w48v_adaptive_correctors.rs` (not ignored), `w48t_cylinder_spokes.rs` A1, A2, A3 (not ignored) and A4 (ignored, run on purpose) | pass unchanged |
| N6 | `cargo test --release -p rcfd-fv` and `-p rcfd-api` | 0 failures other than N2's known local one; the ignored set is the earlier one plus this job's ignored tests |

### L: library checks (`crates/rcfd-fv/tests/w48ab_dt_trigger.rs`, not ignored; the 16 x 16 cavity of ADR 0156's tests)

| Id | Check | Limit |
|---|---|---|
| L1 | dt held constant for 20 steps | no dt-fired estimate; fingerprint equal to `dt_factor` 0 |
| L2 | dt x 1.4 at step 6 | no dt-fired estimate |
| L3 | dt x 2 at step 6 | a dt-fired estimate at step 6 with `dt` the new step, `triggered` count 0 |
| L4 | dt x 0.5 at step 6 (from a count above `n_correctors`, target lowered so the count is above 2) | a dt-fired estimate at step 6 |
| L5 | `run_fixed` with a short last step | no dt-fired estimate; equal to `dt_factor` 0 bit for bit |
| L6 | `dt_factor` 0 with L3's steps | no dt-fired estimate |

### A: 256 x 256 under the Studio plan, N = 10 (AB256)

| Id | Check | Limit |
|---|---|---|
| A1 | run | not stopped by the guard; t = 10 reached |
| A2 | clean | `M_hf <= 0.0114` at every stored time (t = 1 ... 10) and also at every 0.1 s mark (the first step at or after it) |
| A3 | the count follows the step | at least one dt-fired estimate chooses more than 2 correctors |
| A4 | estimates | every estimate's 20-iteration amplification <= 0.5, none cap-bound |
| A5 | effective amplification between estimates: `lag_estimate_next_step(60)` with the count in use at the first step at or after t = 1, 3, 10 (10: after the last step) | each < 1 (value reported; predicted <= 0.7) |
| A6 | how often | at most **6** dt-fired estimates over (0, 10] (predicted 2 to 3, all in the first 20 steps) |

### B: the control, AB256 with `set_corrector_dt_factor(0)` (AB256-off), N = 10

Measured: `M_hf` at the same times as A2; the count; `lag_estimate_next_step(60)` at the first step at or after t = 1, 3, 10.
**Reading rule (fixed now):** (B-i) `M_hf > 0.0114` at any time: the caveat reproduced as spokes; (B-ii) not (B-i), but the
60-iteration estimate with 2 correctors >= 1 at t = 1 or t = 3 (before the 1000-step estimate): the caveat reproduced as a
missing margin (at or above lag-neutral at the grown step), not as spokes; (B-iii) neither: **the caveat is not reproduced**;
reported as such; the job's change then restores the registered margin 0.5 at the grown step but no instability is shown at
t <= 10. The control is not a pass/fail limit of this job; its outcome is reported with its class.

### C: resolution sweep under the Studio plan with automatic dt, N = 100 (ADR 0156 C3, C5 bands)

| Id | Run | Limit |
|---|---|---|
| C1 | C64 (64 x 48, dt0 0.00602), C128 (128 x 96, 0.00307), C256 (256 x 256, 0.00116), to t = 100 | each: not stopped; `M_hf <= 0.0114` at every stored time |
| C2 | shedding by `scripts/w48v_shedding.py` over [50, 100] | each St in [0.145, 0.175], mean cd in [1.25, 1.45]; \|St(C256) - St(C128)\| <= 0.008, \|cd(C256) - cd(C128)\| <= 0.04 |
| C3 | counts | reported: the count history, dt-fired estimates of each run |

A run stopped by its wall cap (C64 2 h, C128 6 h, C256 14 h; harness limit) is evaluated on what it reached: St over [50,
t_end] when at least three periods lie there, else "not reached" (ADR 0156's reading rule; not a pass).

### E: cost of the dt-fired estimates

Measured in the harness: the wall time of every `Simulation::step`; the cost of an estimate step = its time minus the median of
the 10 nearest steps without an estimate. Deterministic proxy: `work` (D4) against the run's corrector applications (sum of the
counts of all steps).

| Id | Check | Limit |
|---|---|---|
| E1 | AB256: cost of the dt-fired estimates over (0, 3] against the wall time of all steps over (0, 3] | <= **10 %** (from TV256: about 0.19 s per application; predicted about 240 applications, about 45 s against about 700 s, 6 %) |
| E2 | the same over (0, 10] | <= **4 %** |
| E3 | all estimates (every trigger) over (0, 10] | reported |

## Run matrix (work folder `E:\projects\cfd\w48ab-work`, outside the repository; one process per run)

| Id | Mesh | N | Trigger | Threads | Wall cap |
|---|---|---|---|---|---|
| AB256 | 256 x 256 | 10 | on | 1 | 6 h |
| AB256-off | 256 x 256 | 10 | off (B) | 1 | 6 h |
| C64 | 64 x 48 | 100 | on | 1 | 2 h |
| C128 | 128 x 96 | 100 | on | 1 | 6 h |
| C256 | 256 x 256 | 100 | on | 2 | 14 h |

C256 is started first (the longest). Free RAM is checked before starting (other agents share the machine). L and N run from
`cargo test`; N4 on a `git archive` copy of 444255c with its own target folder.

## Tools (written after this registration, before their first use)

* `crates/rcfd-fv/tests/w48ab_dt_trigger.rs` (L1-L6).
* `crates/rcfd-api/tests/w48ab_dt_trigger.rs`: the harness of A, B, C (one ignored test per run id, selected by name; the work
  folder from `W48AB_WORK`, wall cap from the table).
* `crates/rcfd-api/tests/w48ab_fingerprints.rs`: N4's printer (ignored; only APIs of 444255c).

## Not changed by this job

`crates/rcfd-studio`, `crates/rcfd-runner`, every default, ADR 0156's target, cap, iteration count and other triggers, the ADR
0077 check, SV301, `baseline_hashes.rs`, the example projects and the user's files.

## Addendum (2026-10-29): results against the registered limits

Everything below was measured after the registration (fedae40) on the code of c0dc6d8 (no amendment was needed: no set-up
defect was found and no limit or rule was changed after a result was seen). Runs from copies of c0dc6d8's test binaries
(`E:\projects\cfd\w48ab-work\bin\`), work folder `E:\projects\cfd\w48ab-work` (run streams `runs/<id>/solver-status.jsonl`, logs
`logs/`, the 444255c copy and its fingerprints). The machine was shared with other agents' workloads (CPU 85 % busy before
these runs started, 26 to 35 GB free), so wall times are rough; E's shares compare steps of the same run.

### The answer

With the dt trigger the adaptive count follows the automatic step. On the 256 x 256 cylinder under the Studio's plan the
count is 2 at step 1 (dt 0.00116, 0.406), the trigger fires at step 4 (dt 0.00200, 1.73 dt0: 3 correctors, 0.242) and at step
14 (dt 0.00302, 1.51 times the step-4 dt: 3 kept, 0.358), and the run then holds 3 to t = 10 (the 1000-step estimates at steps
1014 and 2014: 3, 0.419 and 0.432). The 60-iteration estimate of the count in use stays 0.39 to 0.45 at t = 1, 3, 10; the run is
clean (largest M_hf 5.7e-7). The extra estimates cost 3.3 % of the steps' wall time to t = 3 and 1.1 % to t = 10. Without the
trigger (the control) the count stays 2 for 1000 steps and the 60-iteration estimate with 2 correctors at the grown step is
0.938 (t = 1) and 0.997 (t = 3): below 1, but at lag-neutral rather than at the registered margin 0.5; that run is clean too
(largest M_hf 5.7e-7), and its own 1000-step estimate at step 1001 (t = 3.44) chooses 3 (0.392), the count the trigger chose at
step 4. **So the caveat of ADR 0158 is not reproduced as an instability at t <= 10 (class B-iii)**: on this mesh at the Studio's
Courant 0.5 the two-corrector coupling sits just under lag-neutral, and what the trigger buys here is the registered margin (from
about 1.0 to about 0.4) for the first 1000 steps, not the removal of a visible defect. The resolution sweep under the Studio's
plan to t = 100 (64 x 48, 128 x 96, 256 x 256) is clean at every stored time and converges (St 0.1573, 0.1649, 0.1665; mean cd
1.3264, 1.3532, 1.3578), with counts 2, 2, 3. Fixed-dt adaptive runs and default runs are bit-identical to 444255c; the only
failing test (`case_hashes_w48u`) fails identically at 444255c.

### Results

| Check | Limit (registered) | Measured | Outcome |
|---|---|---|---|
| N1 | `w33b_default_bits` passes unchanged | passed (also run on its own with its output): e9dca234e513bec8, 72d935a8d4993955, e46fbe511183e9fc, c21a6553acb2a7a6, 2c71144215f1d245, ADR 0156's five values | met |
| N2 | `baseline_hashes` as at 444255c, file not edited | 3 passed (shipped cases, cavity, heated cavity), 1 ignored (its existing `rotating_channel` known difference); the local failure the brief expected (`kepsilon_channel`) did not occur; no diff of the file | met |
| N3 | `case_hashes_w48u` passes | **fails**: H1 "30 differ": every difference is in the written `toml`/`json` text hashes, the settings and mesh hashes are equal; the identical test on the `git archive` copy of 444255c fails with the same 46 lines (compared line by line: 0 differences). Its record was made on 6867e40, before ADR 0156 added `solver.corrector_control` to the schema (the written texts list every key with its default), which is the likely cause; ADR 0160 (job 48Z) found the same failure identical at 444255c and gives the same reading ("toml/json text hashes of the 48U record against the merged schema") | **not met as worded; pre-existing at 444255c, unchanged by this job** (deviation 6) |
| N5 | ADR 0156's tests pass unchanged | rcfd-fv `w48v_corrector_control` 5 of 5; rcfd-api `w48v_adaptive_correctors` (not ignored: key/hash, A9b, N5/A10, B3, A11) ok; `w48t_cylinder_spokes` A1, A2 (K1 adaptive), A3 (K2 adaptive) ok; A4 (ignored, run on purpose): count 8, equal to `n_correctors = 8` bit for bit, 150 s | met |
| N6 | 0 failures but N2's known one | rcfd-fv: 101 test binaries, all `ok`; rcfd-api (`--no-fail-fast`): 147 binaries, 740 passed, 1 failed (N3's H1, pre-existing), 273 ignored (6 of them this job's new ignored tests; no existing test file is edited, so no `#[ignore]` was added or removed elsewhere) | not met as worded: the one failure is N3's, pre-existing at 444255c |
| N4 | five fingerprints equal; F2 = ADR 0156's record | 444255c / branch: F1 afd495792c2b99fe / same (160 steps, count 2, 0.3628); F2 cc671f5cf533a58f / same (192 steps, count 8, 0.47074: ADR 0156's 8 and 0.4707); F3 825d0690211cbd20 / same (17 steps, the last 0.03 s after 16 of 0.0625: the landing rule keeps it bit-identical); F4 e14dcf87ed39866d / same; F5 93857919dcc30131 / same (201 steps, fixed correctors, automatic dt). Not registered, printed for contrast: F6 (adaptive under the Studio's time entries, 64 x 48) differs, 6f08f81a1f3d5b4c / 6f92929a044c68ee: the branch's log has the two dt-fired estimates (steps 4, 135; count 2 kept), the hash includes the log | met |
| L1-L6 | as registered | `crates/rcfd-fv/tests/w48ab_dt_trigger.rs`: 5 tests (L2 and L3 in one) passed: constant dt no dt-fired estimate and bit-equal to factor 0; x1.4 none; x2 one at step 6 with dt 0.1, `triggered` 0; x0.5 one at step 6 (count 10 at target 0); the short last step of `run_fixed` (0.02 after 0.05) none and bit-equal; factor 0 none | met |
| A1 | not stopped, t = 10 | 2767 steps, t = 10 exactly | met |
| A2 | M_hf <= 0.0114 at every stored time and every 0.1 s | largest of 110 values 5.72e-7 (stored: 1.02e-7 at t = 1, 1.10e-7 at t = 3) | met |
| A3 | a dt-fired estimate above 2 | step 4 (dt 0.00200448 = 1.73 dt0): rho(2) above 0.5, 3 correctors at 0.2425 | met |
| A4 | every estimate <= 0.5, none cap-bound | 0.4061 (step 1, 2), 0.2425 (step 4, 3), 0.3579 (step 14, 3), 0.4186 (step 1014, 3), 0.4324 (step 2014, 3); no cap | met |
| A5 | 60-iteration estimate of the count in use < 1 at t = 1, 3, 10 | 3 correctors: 0.3934, 0.4214, 0.4460 (predicted <= 0.7) | met |
| A6 | at most 6 dt-fired estimates | 2 (steps 4 and 14; predicted 2 to 3, all in the first 20 steps) | met |
| E1 | dt-fired estimates <= 10 % of the steps' wall time over (0, 3] | 29.7 s of 910.7 s: 3.3 % (step 4: 18.9 s for 100 applications; step 14: 10.9 s for 60) | met |
| E2 | <= 4 % over (0, 10] | 29.7 s of 2745.2 s: 1.1 % | met |
| E3 | all estimates over (0, 10], reported | 87.3 s of 2745.2 s: 3.2 % (step 1: 6.6 s; steps 1014, 2014: 24.4 s and 26.6 s); deterministic proxy: 400 lag-map corrector applications (160 dt-fired) against 8298 corrector solves of the steps | reported |
| B | reported by its reading rule | AB256-off: 2767 steps to t = 10, largest M_hf 5.73e-7 (no spokes); 2 correctors to step 1000; 60-iteration estimate with 2 correctors 0.9379 (t = 1, the wall cell at (0.499, 0.043)) and 0.9970 (t = 3); its own 1000-step estimate at step 1001 (t = 3.44) chose 3 (0.3923), step 2001 3 (0.4260); at t = 10, 3 correctors 0.4460 | **class B-iii: not reproduced** |
| C1 | C64, C128, C256 not stopped, M_hf <= 0.0114 at every stored time | C64: 5824 steps, largest 8.03e-3 (the 64-around physical part, as V64's 7.4e-3); C128: 11822 steps, largest 4.42e-5; C256: 26747 steps (18 115 s, 2 threads), largest 1.66e-7 (the 100 stored values 4.4e-8 to 1.66e-7) | met |
| C2 | St in [0.145, 0.175], mean cd in [1.25, 1.45]; C256 against C128 | C64 St 0.1573, cd 1.3264; C128 St 0.1649, cd 1.3532; C256 St 0.1665, cd 1.3578 (cl peak to peak 0.673); \|St(C256) - St(C128)\| 0.0016, \|cd(C256) - cd(C128)\| 0.0046 | met |
| C3 | counts, reported | C64: 2 throughout (estimates 0.040 to 0.085; dt-fired at steps 4 and 135); C128: 2 throughout (0.111 at step 1, 0.137 and 0.227 at the dt-fired steps 4 and 333, then 0.247 to 0.255); C256: as AB256 to t = 10 (the same five estimates with the same values: `max_dt` 1 against 0.1 never binds), then 3 at every 1000-step estimate to step 26014 (0.425 to 0.440, dt 0.00371 to 0.00381); dt-fired only at steps 4 and 14; mean 3.000 | reported |

### Against the predictions

* The count followed the prediction (2, then 3 from step 4); the second dt-fired estimate came at step 14, not about step 7,
  because the controller's step levelled off at Courant 0.45 near 0.0029 to 0.0030 s (1.42 to 1.50 times the step-4 dt) and
  crossed 1.5 only at step 14; it kept 3. No dt-fired estimate after step 14 on any 256 x 256 run (the step stayed within
  0.0030 to 0.0039 s, under 1.5 times 0.00302).
* 64 x 48 and 128 x 96 kept 2 throughout (predicted 2, and 2 or 3); their grown-step estimates 0.085 and 0.25.
* The control: the 2-corrector estimate at the grown step was 0.94 to 1.00 (predicted 0.9 to 1.2), no spokes (predicted).

### Deviations

1. The worktree started at 1248a76, the merge base, and was fast-forwarded to local main 444255c (48S, 48V, 48W, 48X merged)
   before any work.
2. D2 names `Simulation::step` and `run_fixed` as the places that mark a landing step; the library's `ControlledRun::advance`
   (`stepctl.rs`, the controller's own driver, not used by `Simulation` for these runs) marks its landed steps the same way, for
   consistency of the library. No registered run goes through it.
3. The harness's `wall_s` includes its own diagnostics (the 60-iteration trials, 30 to 42 s each on 256 x 256, and the M_hf
   evaluations); E uses the per-step `step_s` only, as registered.
4. `scripts/w48v_shedding.py` prints `cd(3) None` for C64 and `cd(10) None` for C128: its nearest-step tolerance is taken from the
   first step's size, which is small under the automatic step. Not a registered quantity (C2 uses its St and mean cd only).
5. N2 and N3 (`baseline_hashes`, `case_hashes_w48u`) ran inside N6's `cargo test -p rcfd-api`, N1 (`w33b_default_bits`) inside
   N6's `cargo test -p rcfd-fv`; N5's ignored A4 was run on its own afterwards. A first rcfd-api run without `--no-fail-fast`
   stopped at N3's failure; the reported N6 is the second, full run.
6. N3 as registered ("passes") is not met, but the failure is the same at 444255c, before this job, and concerns written case
   texts, which this job does not touch. It was **not** turned into a `*_known_miss` test: the test belongs to ADR 0155 and the
   miss is not this job's; question 6 asks for its owner's decision.
7. While this job ran, local main advanced to 1a9c566 (jobs 48Y, 48Z, 48AA). The branch stays on 444255c; a trial merge with
   1a9c566 (aborted afterwards) has no conflict and `cargo check -p rcfd-fv -p rcfd-api --all-targets` of it is clean; no run or
   test of this job was repeated on the merged tree. Read of 1a9c566's `sim.rs`: `let proposed = dt;` still precedes `fixed_step` and the end clamp, and 48Z's end-time snap changes the reported time after the step, not the step, so D2's landing mark sees the same values there.

### Not run

* Linux; the Studio itself (not edited) and the runner (not edited): the registered runs drive `Simulation` in-process, which is
  what `rcfd run` (launched by the runner for a Studio Run) calls step by step; no Studio Run of the 256 x 256 case was made.
* The control B beyond t = 10, and a control of the sweep C (the trigger off at N = 100).
* A write-time landing with the adaptive count (the shipped case has no `write_times`; the end-time landing of the controller and
  the fixed step's short last step were exercised: AB256 and the control end on a landed step without an estimate, F3).
* The test suites of crates other than rcfd-fv and rcfd-api (their code was compiled by `cargo clippy --workspace --all-targets
  -- -D warnings`, clean); `cargo fmt -p rcfd-fv -p rcfd-api` was run (the other crates are unchanged).

### Questions for the maintainer (with recommendations)

1. **Keep the dt trigger at a factor 1.5 in every adaptive run?** Measured cost on 256 x 256: 3.3 % of the steps' time to t = 3,
   1.1 % to t = 10; on 64 x 48 and 128 x 96 two extra estimates of about 0.24 s and 1.1 s. Recommendation: yes; it makes the
   count follow the controller's step, which "stable whatever dt the controller picks" needs, and it costs little.
2. **The caveat was not reproduced as an instability (B-iii).** At the Studio's Courant 0.5 the 256 x 256 two-corrector coupling
   sits at 0.94 to 1.00 (just below lag-neutral) and stayed clean to t = 10, as ADR 0158's 3 s run did. Nothing in this job shows
   a case where the old schedule produced spokes under the automatic step. Recommendation: keep the trigger anyway (the registered
   margin 0.5 is what ADR 0156 D2 argued for; at 0.94 to 1.00 a different flow state, mesh or Courant target could cross 1), and do
   not spend a job on demonstrating the failure.
3. **With the margin restored, the automatic step on 256 x 256 runs 3 correctors instead of 2 (about 1.3 times the work per step).**
   Keep the target 0.5 (ADR 0156 question 2), or accept a target near 0.9 under the automatic step? Recommendation: keep 0.5; one
   rule for every mesh and step is simpler to reason about, and the 1000-step estimate chose 3 here anyway.
4. **SV301 under the automatic step.** The pre-run warning estimates at the first step's dt (dt0), where 256 x 256 gives 0.406,
   while the grown step gives 0.94 to 1.00; with the Studio's switch off (fixed correctors) SV301 cannot see the grown step. Here
   it would not fire either way (both below its level 1.3), so this is a gap, not a miss. Recommendation: a follow-up that makes
   the preflight estimate at the controller's settled step (or at `max_dt` capped by the Courant target) when a Courant
   controller is on.
5. **The diagnostic `Simulation::lag_estimate_next_step` and the switch `set_corrector_dt_factor`** are public for tests only.
   Expose the diagnostic anywhere (MCP, run log)? Recommendation: no, not until a use is asked for.
6. **`case_hashes_w48u` (ADR 0155's H1) fails on main 444255c** (and identically here): its record (6867e40) predates ADR 0156's
   schema key, so every written case text hashes differently while the settings hashes are equal. Re-record the fixture's text
   hashes, or narrow H1 to the settings and mesh hashes? Recommendation: a small job of ADR 0155's owner that registers the
   expected difference (the one new key) and re-records the text columns only; not done here (not this job's file).
