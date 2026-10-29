# ADR 0160: Job 48Z, three follow-ups: a fixed step's final time reported on `solver.end_time`, the corrector count in `residuals.jsonl`, SV301 in the run service and the MCP tools with the first-step viscosity

Status: **registered** 2026-10-29, before any code of this job was written or compiled and before any run of this job (set-ups,
criteria, limits, reading rules and the order of runs are fixed here); results in the addendum. House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed before the registered runs; a missed limit
becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before the
corrected run; defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no
unsafe code, no new crate, the crate graph stays acyclic; no edit of `crates/rcfd-studio`. Clean-room: first principles, the
repository's own ADRs and code. Parents: [ADR 0145](0145-job48k-end-time-remainder-step.md) (the fixed-step landing,
`fixed_step`), [ADR 0153](0153-job48s-time-step-set-when-a-run-starts.md) question 9 (a fixed step ends up to 1e-12 short),
[ADR 0156](0156-job48v-adaptive-pressure-correctors.md) (adaptive correctors, SV301; its questions 3, 5, 6, 7 and D8),
[ADR 0050](0050-wave27b-robustness-adaptive-stepping.md) (the adaptive controller's snapped landing),
[ADR 0143](0143-job48i-native-runner-start-unix-zombies-linux-known-misses.md) and
[ADR 0146](0146-job48l-runner-output-early-exit-windows-inherited-handles.md) (the runner's output).

## The request

Job 48Z, three items from the maintainer's queue:

1. **Fixed-step end time** (ADR 0153 question 9, recommendation (b)): a run with a fixed `solver.dt` can finish up to 1e-12 short
   of `solver.end_time` (job 48S: 2,567 steps of 3/2567 end at 2.999999999999983). Snap the final time onto `solver.end_time`
   when the end test is met within its tolerance; the fields must stay bit-identical to before (only the reported time stamp
   changes), existing baselines must hold. No opt-in when only the time stamp changes; if any field bit changes, stop, make it
   opt-in and report.
2. **Corrector count in `residuals.jsonl`** (ADR 0156 question 6): a new field written only with `solver.corrector_control =
   "adaptive"`, so that existing `residuals.jsonl` files and hashes of default runs are unchanged; schema doc and tests.
3. **SV301 wiring and viscosity** (ADR 0156 questions 5 and 7): the pre-run coupling warning into the run service and the MCP
   tools wherever `rcfd check` / `--check-only` equivalents are produced; the pre-run estimate gets the Simulation's own
   first-step viscosity, so that rheology / turbulence cases are not missed (the power-law channel: 0.011 in the preflight
   against 4.17 in ADR 0077's in-step estimate); none of the 14 qualified shipped/literal cases of ADR 0156 B2a may newly warn.

## Context (read at local main 444255c; the worktree started at 1248a76 and was fast-forwarded to 4e795b1 and then to 444255c, wave 48X merged, before any work)

### Item 1

* `crates/rcfd-api/src/sim.rs` `fixed_step(t, dt, end)` (ADR 0145 D1) serves four arms of `Simulation::step`: prescribed flow
  (`Engine::Transient` with `frozen_flow`), transient variable density, VOF, and the incompressible PISO/PIMPLE arm when the
  step is the solver's fixed `solver.dt` or the legacy `solver.max_courant` controller's (`fixed`: neither ADR 0050's planned
  step nor its step-doubling error control). Each arm ends the run when `last` (a stretched step) or `time >= end - 1e-12`.
  Every solver advances its own time by one addition, so a run whose summed steps fall short of `end` by less than 1e-12 ends
  there: 48S's 1.7e-14 (ADR 0153 deviation 2).
* Arithmetic (Python, the same IEEE additions as the solvers' `time += d`, `d = min(dt, max(end - t, 1e-12))`; not a solver run):
  10 steps of 0.05 to 0.5 end at 0.49999999999999994; 20 steps to 1.0 end at 1.0; a continuation of the first ten from exactly
  0.5 takes a last step of `0x1.9999999999960p-5` where the single run takes `0x1.9999999999970p-5`. So if the snapped time
  reached the solver's own time or a restart file, `chain_w19c` C7 (two stages 0 to 0.5 and 0.5 to 1.0 against one run,
  bit for bit) would differ in its fields: **the snap must reach the reported time only** (D2).
* Readers of the time inside `step()` before it returns: the scalar step's `StepCtx.time`, the monitor abscissae of every arm
  and of the shared monitors (`x = summary.time`), `stats.accumulate(summary.time, ..)` (time-weighted means: the width of each
  step is `t_end - t_prev`), `udf_monitor_step(x, ..)`, `particles.after_step(self)` (`dt = sim.time() - t_prev`).
  Readers after it returns: `runctl::drive` (`iteration` events: `step.time`; the field store's `after_step(.., step.time)`;
  `RunOutcome.time = sim.time()`, the `finish` event and `rcfd run`'s summary), `fieldstore::Hook::finish` (`sim.time()`),
  `Simulation::status_json` and the export metadata (`self.time()`), `particles::track_transient` (`dt = sim.time() - t_prev`
  per flow step), `save_restart` (the solver's `time` and its last `StepInfo`), the service's status and logs.
* `rcfd_post::monitors::MonitorStore` keeps `x` and `y` per series; `step_report_values` matches a report's last `x` against
  `res_u`'s last `x` bit for bit (ADR 0130 D5).

### Item 2

* `crates/rcfd-runner/src/runner.rs` `drain` turns every `iteration` event of the solver stream into a `residuals.jsonl` line
  `{"step", "time", "residuals": {"u", "p", "continuity"}, "monitors"}` (serde_json, keys in insertion order of `json!`, which
  serde_json sorts: the crate has no `preserve_order`). The `iteration` event carries `correctors` and `corrector_cap` only with
  `adaptive` (ADR 0156 D4, N5: a `fixed` run's events have neither key).
* `crates/rcfd-project` `ResidualLine` (step, time, residuals, monitors; tolerant numbers) is the reader; `crates/rcfd-studio`
  only deserialises it (`poll.rs`, two tests read it); no struct literal of it outside `crates/rcfd-project` (grep).
* ADR 0142's `scripts/make_example_projects.py --rerun-check` rebuilds the example projects in a temporary copy and compares
  `residuals.jsonl` (with `case.snapshot`, the store and `monitors.json`) byte for byte.

### Item 3

* `runctl::check_only` (behind `rcfd check`, `rcfd run --check-only` and so the runner's `check.json`) makes the preflight and
  appends SV301 since 48V. The run service's `run.check` calls `runctl::check_only`, and the MCP tool `check_case_headless`
  calls `run.check`: so these already give SV301, although ADR 0156 D7 lists the run service and the MCP tools as not wired.
  What is not wired: the service's `run.headless` (MCP `run_headless`), the `rcfd run` equivalent, whose `info` has no
  `warnings` (neither the static rules' nor SV301); `run.start` / `run.step` (interactive stepping, no status stream);
  `settings.validate` (the `rcfd validate` equivalent, excluded by D7); the chain driver.
* `Simulation::pressure_lag_preflight` steps a copy of the solver (`PimpleSolver::lag_preflight`, `scratch()`), whose
  `nu_face` is what the solver holds before the first step: `None` (the constant `physics.nu`) for every case, because only
  `Simulation::step` sets it. The transient incompressible arm sets it before every step from the turbulence model
  (`begin_step(dt)`, `update(u, phi)`, `nu_face()`, with the laminar-zone mask) and then, overriding it, from the rheology law
  (`nu_faces(geom, u, None, t_init)`, relaxed against its previous call). The property-law and user-function viscosities are
  set in the steady arm only. `Rheology` is `Clone`; `TurbState` and `Box<dyn TurbulenceModel>` are not.
* ADR 0156's census log (`E:\projects\cfd\w48v-work\b2b-census.jsonl`, 248 lines, read for this registration): turbulence
  models smagorinsky 12, rsm 9, spalart_allmaras 6, wale 3, plugin 1; rheology power_law 1 (0.011223), carreau 1 (0.200550,
  the `limit` arm: `nu = 0.9` with a Carreau law of zero-shear 0.1 and a vanishing time constant); every turbulent rho below
  0.0002 except the SA set-ups (0.0013). B2a's 14 applicable texts (ADR 0156 `b2a.log`, read): the two example cylinders
  (0.3578, 0.2713), des_tree#0 (0.0013), divergence_guard_w32c#0, dynamic_mesh#0, particles_case#14, registry_setups_w39a#9
  (0.9892), rsm_tree#0, sgs_tree#0, statistics#1, statistics#62 (0.9892), transient_energy#0 (0.6952), zone_sources#40,
  rcfd-studio store_plan_sf#60.
* Seen before this registration: nothing of this job's code (none written); 48V's census log and B2a list as quoted.

## Decisions

**D1. The snap (item 1).** On exactly the paths that call `fixed_step` (the four arms above; not the adaptive controller, not
its error control, not the compressible engine), when a step ends the run by the end time (`summary.done` from `last` or the end
test, and the run stops with "end time ... reached": no failure, divergence, non-finite residual, user-function error,
particle error or convergence condition), and the solver's time `t` differs from `end` with `end - 1e-12 <= t <=
end + 1e-12 max(1, |end|)`, the run's **reported** time becomes `end` exactly. Both sides: `t + (end - t)` can round above
`end` by an ulp. No setting, no schema key (so no case hash changes).

**D2. Reported, not computed.** The snap is applied at the very end of `step()`, after every reader inside it, to:
the returned `StepSummary.time` (so `iteration` events, the store's per-step snapshot time, `self.last`),
`Simulation::time()` (a recorded pair: the solver time's bits and `end`; `time()` returns `end` while the solver's time still
has those bits; cleared at the start of every step and by `reset_run_state`), and the last abscissa of every monitor series
whose last `x` is this step's time (`MonitorStore::retime_last`, new in `rcfd-post`). Unchanged (they read the solver's own
time): every engine's `time` and step history, restart files, the next step of a continued run, the scalar step's
`StepCtx.time`, `stats.accumulate`, `udf_monitor_step`'s `x`, `particles.after_step` (inside `step()`, before the snap), and
`particles::track_transient`, which is switched to a new `Simulation::solver_time()` (the unsnapped engine time) so that its
last particle step stays the flow's step. Consumers of the reported time: `runctl` (`RunOutcome.time`, `finish`), the field
store (per-step and final snapshot times), `status_json`, the export metadata, the service's status. The store's time rule
`floor(t / interval + 1e-9)` can put the final step in another bucket only when an interval boundary lies within 1e-12 of
`end`; the set of stored steps is the same (the final state is stored by `finish` otherwise), only its time stamp differs.

**D3. Not opt-in.** By D2 no field, flux, statistic, particle or restart bit depends on the snap. The S checks verify it; if
any S or N limit shows a changed field bit, the job stops there, makes the snap opt-in (a schema key with a hash-neutral
default, ADR 0156 D6) and reports, as the brief says.

**D4. The corrector count in `residuals.jsonl` (item 2).** `drain` builds each line with a new helper
`residual_line(event)`: the line as before, plus the event's `correctors` and `corrector_cap`, top-level and under the same names
as in the status stream, **only when the event has them** (only `adaptive` runs; a `fixed` line is byte-identical to before).
ADR 0156 D8 item 4's nested `"coupling": {"correctors", "cap"}` is not chosen: one name in the stream and in the record (question
6's recommendation names these two keys). `ResidualLine` gains `correctors: Option<u64>` and `corrector_cap: Option<u64>`
(`#[serde(default, skip_serializing_if = "Option::is_none")]`), so older lines read as before and a line without them writes
as before. Documented in `docs/run-control.md`.

**D5. SV301 wiring (item 3).** (a) `run.check` and MCP `check_case_headless`: already through `runctl::check_only`; stated here
and checked (P1, P2), no code. (b) `run.headless` (MCP `run_headless`): when the session's simulation has taken no step yet
(`iteration() == 0`: freshly initialised, continued from a checkpoint or interpolated in this call), the preflight is made and an
SV301 diagnostic, if it fires, is put in `info.warnings` (the `rules::warnings_json` form `rcfd run` uses), which
`runctl::drive` emits as a `warning` event after `start`; the key `warnings` is added to `info` only when it fires, so the
`start` event of a non-warning case is unchanged. A later call on a session that has already stepped (chunked runs) makes no
preflight. The static setup rules' warnings stay out of the service's runs (unchanged; question). (c) Not wired: `run.start` /
`run.step` (interactive stepping, no status stream), `settings.validate` (`rcfd validate`, ADR 0156 D7), the chain driver (not in
this brief; question). MCP tool descriptions are not changed (`tests/golden/tools_list_default.json`).

**D6. The first-step viscosity (item 3).** `pressure_lag_preflight` hands the trial step the face viscosity that
`Simulation::step` would set before the next step, computed without changing the simulation (new
`Simulation::next_step_viscosity()`): with a rheology law, the law on a **copy** of the `Rheology` (the transient call: no
temperature, `t_init`, relaxed against the copy's history) on the current velocity, which is exactly what the next step sets;
else with a turbulence model, `TurbState::nu_face()` of the model's **current** state (eddy viscosity of the present state,
laminar-zone mask included), which is not the state after the step's own sweep (`begin_step`, `update`; a model copy would
need `Clone` on the turbulence trait: question); else `None`, the existing call bit for bit. `rcfd-fv`:
`PimpleSolver::lag_preflight_with(dt, iterations, nu_face)` sets the copy's `nu_face` when given; `lag_preflight` calls it
with `None`. Under BDF2 the trial's two steps keep that viscosity. For the tests, `Simulation::solver_face_viscosity()` returns the
solver's current `nu_face` (read only).

**D7. Placement.** `crates/rcfd-api/src/sim.rs`, `particles.rs`, `service.rs`; `crates/rcfd-fv/src/pimple.rs`;
`crates/rcfd-post/src/monitors.rs`; `crates/rcfd-runner/src/runner.rs`; `crates/rcfd-project/src/records.rs`;
`docs/run-control.md`, `docs/case-schema.md` (one sentence each); tests below. No new crate or dependency, no workspace edge.

## Predictions

* Item 1: no field bit changes anywhere; the five paths' S variants report `end`; the 48S arithmetic reproduced on the cavity
  ends at 3.0 reported; continuations unchanged; ADR 0145's tests unchanged (their E, T, J, N variants end on `end` already).
* Item 2: `fixed` runs' lines byte-identical; `adaptive` lines carry the two keys.
* Item 3: P1 and P2 pass already on the unchanged tree; P3, P4 fail there and pass after; the power-law channel's preflight
  rises from 0.0112 to about ADR 0077's 4.17 (in-step, 20 iterations, the law's viscosity on the same initial velocity) and warns;
  the Carreau `limit` arm falls (its law gives 0.1 against `nu = 0.9`); law-free laminar estimates bit-identical; turbulent ones
  change (the eddy viscosity of the initial state enters) and stay below 1.3; no B2a text warns.

## Registered checks and limits

Common: release builds of this branch, `CARGO_TARGET_DIR=E:\projects\cfd\target-48z`; output to files. "Unchanged tree": the
registration commit (444255c with this ADR and the new test files, no code change). Hash = FNV-1a over the little-endian bit
patterns (as `baseline_hashes.rs`). The new test files hold no complete case text as a literal (every text is a `format!`
template or built by `settings.set`), so ADR 0156 B2a's literal scan finds no new applicable case of this job.

### N: defaults and baselines

| Id | Check | Limit |
|---|---|---|
| N1 | `baseline_hashes` | passes, file not edited; `shipped_cases_match_registered_hashes` may fail at `kepsilon_channel` only, and only if it fails identically on the unchanged tree (known on this machine at every commit) |
| N2 | `w33b_default_bits` (rcfd-fv) | passes (the five fingerprints of ADR 0156 N2) |
| N3 | ADR 0145's `end_time_landing_w48k` (C1, L1-L6, W1, B2) | passes unchanged |
| N4 | `chain_w19c` (C7: chained transient = single run bit for bit, time included), `restart_w19c`, `timectl_tree` | pass |
| N5 | `cargo test --release` of rcfd-post, rcfd-fv, rcfd-api, rcfd-project, rcfd-runner, rcfd-mcp, rcfd-cli, and rcfd-studio's `json_o` and `saved_store_r` (they read `ResidualLine`) | 0 failures except N1's known one; ignored = before plus this job's new ignored tests |
| N6 | `cargo clippy --all-targets -- -D warnings` of the changed crates and their dependents (workspace), `cargo fmt -p <crate> -- --check` per changed crate, `workspace_graph` | clean, acyclic |

### S: the snap (`crates/rcfd-api/tests/end_time_snap_w48z.rs`, existing API only, so it runs on the unchanged tree)

Variant S: `end = t_3 + 1e-13` with `t_3` the f64 sum of three steps (ADR 0145's set-ups and steps: PISO and PISO with the legacy
Courant controller on the 20 x 20 cavity at 0.01, prescribed flow at 0.0025, variable density at 0.05, VOF at 1e-3); the end time
written with 17 significant digits.

| Id | Check | Limit (fixed tree) | On the unchanged tree |
|---|---|---|---|
| S1 | The five paths, variant S, stepped in-process | 3 steps; `step().time` of steps 1, 2 = the partial sums, of step 3 = `end` bit for bit; `sim.time()` = `end`; the state hash (`ux uy p T alpha` that exist, face flux) and the hash of every monitor `y`, and of every monitor `x` but the last, equal the unchanged tree's (recorded in R2); every series' last `x` = `end` | predicted: 3 steps, final time `t_3` (not `end`); prints the hashes |
| S2 | 48S's arithmetic on the PISO cavity: `dt = 3/2567`, `end = 3` | 2567 steps; final reported time 3.0 bit for bit; state hash equal to the recorded one | predicted: 2567 steps ending at 2.999999999999983 |
| S3 | S1's PISO cavity with `[statistics]` (start 0, fields `ux`, `p`) | hash of `ux_mean ux_rms p_mean p_rms` and the state hash equal the recorded ones | records |
| S4 | Particles: the settling box of `particles_case.rs` PT14 (as a template) with `solver.end_time = t_3 + 1e-13` at dt 0.01, `particles.end_time` 1.0, through `particles::track_transient` | 3 flow steps; hash of every parcel's position and velocity and the tracker time equal the recorded ones | records |
| S5 | Continuation: the PISO cavity S run through `runctl::drive` with a checkpoint, then a fresh simulation with `end = t_6` (the exact sum) loaded from it and driven to the end, against one run from 0 to `t_6` | ux, uy, p, flux and `time()` bit-identical between the two; the chained final state hash equal to the recorded one | records; must already be equal |
| S6 | Through `runctl::drive` with an output folder and the field store every step (`ux`): the PISO cavity S | the last `iteration` event's `time`, the `finish` event's `time` and `RunOutcome.time` = `end`; the store holds 3 snapshots, the last at step 3 and time `end` bit for bit; the stored `ux` of every snapshot bit-equal to the recorded hash | predicted: the times are `t_3` |
| S7 | Unit test of the snap rule (`sim.rs`): `end - 1e-13`, `end + 1 ulp`, `end` with `end = 1e5` and one ulp above, `end - 2e-12`, `end` itself, NaN | `end`, `end`, `end`, none, none, none | (new code) |
| S8 | `crates/rcfd-api/tests/w48z_followups.rs` (new API): S1's PISO run, `solver_time()` after step 3 | `t_3` bit for bit (the solver's own time is not snapped) | (new API) |
| S9 | Probe (scratch copy, never committed): one line per firing of the snap (program, `t`, `end`) to `RCFD_W48Z_SNAP`, during the suites of rcfd-api, rcfd-cli, rcfd-runner, rcfd-mcp | reported: the firings outside this job's test files (which existing tests now see a changed time stamp), their largest `|t - end|`; and the probe run's failures, each counted as N5 counts them | |

### R: the corrector count in `residuals.jsonl`

| Id | Check | Limit |
|---|---|---|
| R1 | Unit (`runner.rs`): `residual_line` of three fixed-run events (with monitors, without, with `null` residuals) against the previous construction (the old `json!` expression, copied into the test) | serialised bytes equal; an event with `correctors` 5 and `corrector_cap` 0 gives the same line plus exactly those two keys |
| R2 | Runner end to end (`crates/rcfd-runner/tests/residuals_correctors_w48z.rs`): the 20 x 20 PISO cavity, 5 steps (`solver.end_time = 0.05`, dt 0.01), once with `solver.corrector_control = "adaptive"`, once without | adaptive: every line has `correctors` (an integer >= 2) and `corrector_cap` 0; fixed: no line has either key, every line's keys are exactly `monitors residuals step time` | 
| R3 | `rcfd-project` (`tests/records.rs`): `ResidualLine` round trip with and without the two fields; an old line (no fields) reads with `None` and writes back byte-equal | as stated |
| R4 | `python scripts/make_example_projects.py --rerun-check` (ADR 0142, this platform's tree, the branch's release binaries) | the comparison passes: `residuals.jsonl` byte-equal (and the script's other files); a failure is run again on the unchanged tree's binaries and counted only if it does not fail there |

### P: SV301 in the run service and the MCP tools

"K1": the committed `example-projects/wind-tunnel/cases/cylinder-re100.toml` with `mesh.nx = 64`, `mesh.ny = 256`,
`solver.dt = 0.015625` (ADR 0154 K1, the SV301 positive of ADR 0156 B1/B3, rho 4.36), set through `settings.set` on a session
created from the file's text.

| Id | Check | Limit | Unchanged tree |
|---|---|---|---|
| P1 | Service `run.check` on a K1 session and on the committed case's session (`crates/rcfd-api/tests/sv301_service_w48z.rs`) | K1: `exit_code` 0, `event.rules` holds exactly one SV301 (severity warning, category solver, path `solver.corrector_control`, fix value `adaptive`); committed: no SV301 | predicted pass |
| P2 | MCP `check_case_headless` on a K1 session (`crates/rcfd-mcp/tests/sv301_mcp_w48z.rs`, library server) | as P1 | predicted pass |
| P3 | Service `run.headless` with `max_steps` 2: K1, then a second call (`max_steps` 1) on the same session; the committed case | K1 first call: events `start`, then a `warning` with `rule` SV301 before the first `iteration`, and `start.info.warnings` holds it; second call: no SV301 event; committed: no `warning` event and no `warnings` key in `start.info` | predicted fail (no warning) |
| P4 | MCP `run_headless` (`max_steps` 1) on a K1 session | the returned events hold the SV301 warning | predicted fail |
| P5 | The preflight changes no result in the service: the committed case, `run.headless` 20 steps with `pressure_lag_check = "warn"` (preflight made, no warning) against `"off"` (none) | the 20 `iteration` events equal in index, time, res_u, res_p, continuity | predicted pass |

### V: the first-step viscosity

The rheology channel: `crates/rcfd-api/tests/rheology.rs` `transient_runs_use_the_law_too`'s set-up (its `channel` text, PISO
dt 0.02, 20 x 8) as a template: `thin` (power law K 0.1, n 0.4, cap 10, `nu = 0.1`), `limit` (Carreau, `nu = 0.9`), `plain`
(`nu = 0.1`, no law). Turbulence: the first case literals of `sgs_tree.rs` (Smagorinsky) and `des_tree.rs` (Spalart-Allmaras), as
templates.

| Id | Check | Limit | Unchanged tree |
|---|---|---|---|
| V1 | Preflight of `thin` (`sv301_service_w48z.rs`) | rho >= 1.3: SV301 fires (`check_only` gives it); value reported against ADR 0077's 4.17 | predicted fail (0.0112) |
| V2 | `next_step_viscosity()` before step 1 against `solver_face_viscosity()` after step 1 (`w48z_followups.rs`), `thin` and `limit` | bit-identical | (new API) |
| V3 | The same for the two turbulence set-ups | reported: largest relative difference (the size of D6's approximation) | (new API) |
| V4 | The preflight changes no result: `thin` and the SGS set-up, 5 steps after calling `pressure_lag_preflight` against a fresh simulation without it | ux, uy, p and flux bit-identical | predicted pass |
| V5 | ADR 0156 B2a (`w48v_b2a_...`, ignored, run explicitly) | 0 warnings; the applicable list is ADR 0156's 14 (any other applicable text reported) | |
| V6 | ADR 0156 B0 and B1 (`w48t_cylinder_spokes.rs`, ignored, run explicitly) | B0: each within 1e-3 of DG1 and equal to ADR 0156's printed six decimals; B1: 10 of 10 as registered there | |
| V7 | Census (S9's scratch run of rcfd-api, `RCFD_W48Z_B2B`): for every simulation the suite builds where the preflight applies, both the old estimate (`lag_preflight` with the constant viscosity, the unchanged code path) and the new one | (a) every set-up with neither turbulence nor rheology: new = old bit for bit; (b) reported: every set-up whose warning status (rho >= 1.3) differs between old and new, and every rho >= 1.3 | |
| V8 | ADR 0156 B2c's classification (`w48v_b2c_...`) of every set-up that newly warns in V7 (a log of those set-ups with the new rho) | zero false positives (ADR 0156's rule: g < 0.95) | |

## Order of runs

1. R1 (unchanged tree, after the registration commit): `end_time_snap_w48z` (prints S1-S6's values), `sv301_service_w48z`,
   `sv301_mcp_w48z`, `residuals_correctors_w48z`; N1 (to see `kepsilon_channel` on this machine).
2. R2: S's printed values written into the test file's `PRE_FIX` (a commit with no other change).
3. The code (a commit), then on the fixed tree: every test of S, R, P, V; N1-N6.
4. S9/V7 scratch run, then V8, V5, V6, R4.

Results go into an addendum; a miss or a defective set-up is handled by the house rules above.

## Amendment 1 (2026-10-29, after the registration commit 5e282d6 and before any run of this job)

P3's test counted the SV301 entries of `start.info.warnings` by the key `id`, but those entries are `rules::warnings_json`
objects, whose rule id is under `rule` (read in `rules.rs` while writing the code plan; nothing had been built or run): the set-up
would have reported 0 entries whatever the code does. Corrected in the test before its first run: the entries with
`rule = "SV301"` are counted. P3's limit is unchanged.

## Not changed by this job

The solver's time arithmetic and every engine's `time`, the adaptive controller and its error control (ADR 0050), the compressible
engine's own landing (relative tolerance, no projection; ADR 0145 question 4), the schema (no key), `crates/rcfd-studio`, the
example projects' committed files, `baseline_hashes.rs`, SV301's level and iterations, the ADR 0077 one-corrector check.
