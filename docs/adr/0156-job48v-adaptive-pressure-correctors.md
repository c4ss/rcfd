# ADR 0156: Job 48V, adaptive PISO/PIMPLE pressure correctors chosen by the pressure-lag estimate (`solver.corrector_control`), a pre-run coupling warning (SV301) and resolution robustness of the refined cylinder

Status: **registered** 2026-10-28, before any code of this job was compiled and before any run of this job (set-ups,
criteria, limits and reading rules are fixed here); two amendments before the runs they govern; **results in the addendum**
(2026-10-29: every registered limit met except the registered A9, a set-up defect corrected by A9b); not yet accepted by the
maintainer. House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed before the registered runs; a missed limit
becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before the corrected
run; defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no unsafe
code, no new crate, the crate graph stays acyclic; no edit of `crates/rcfd-studio` (job 48S works there). Clean-room: first
principles, the repository's own ADRs and code; nothing of any vendor's software is decompiled or read. Parents:
[ADR 0154](0154-job48t-cylinder-ogrid-pressure-spokes.md) (job 48T: the spokes are the PISO coupling failing in the thin wall
cells; DG1, K1, K2), [ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) (the pressure-lag model and its run-time estimate),
[ADR 0077](0077-wave35b-solver-robustness-under-automation.md) part C (the estimate on the case path, one corrector only),
[ADR 0031](0031-wave19c-restart-chain.md) (setup rules), [ADR 0130](0130-wave48a-overview-and-run-record-contract.md) (status
stream, run records). Related, concurrent: job 48S (ADR 0153, Studio run plan: time step set at Run), job 48U (ADR 0155).

## The request

The maintainer, after ADR 0154 (verbatim): "i'm not looking for you to fix the settings - we shouldn't have to get to a point
where AI is having to solve our settings, the simulation should be stable when changing resolution by adjusting settings around
it - it's a core feature of the CFD [solver]". A user who raises `mesh.nx`, `mesh.ny` must get a stable, artifact-free run
without hand-tuning `n_correctors`, `time_advance` or `grading`; the solver (and the Studio's run plan) adapts its own numerics
to the mesh. Approved by the maintainer (ADR 0154 questions 1 and 2): (1) a pre-run warning when two or more pressure
correctors are likely unstable, from the existing pressure-lag estimate, warning level between 1.17 and 1.68 (for example 1.3),
changing no result; (2) an opt-in "correctors until converged" PISO option, accepted against the known misses K1 and K2. Not
wanted: a settings workaround note for the user.

## Context (read at local main aad08fe; the worktree started at 1248a76 and was fast-forwarded to aad08fe before any work)

### What 48T measured (quoted, not re-measured)

* The default transient coupling (`solver.algorithm = "piso"`: `PimpleSettings::piso`, n_outer 1, `n_correctors` 2) loses the
  pressure-velocity coupling in the thin wall cells of the graded O-grid once the wall diffusion number nu dt / dr1^2 passes an
  onset between 6.5 and 11.4. The ADR 0073 pressure-lag map (power iteration, 60 iterations, the step's frozen matrices) at
  step 1 (DG1): G64 (64 x 48, 1/16) 0.358; G128 (128 x 96, 1/32) 0.746; G256x48 0.221; G256g (grading 10.88) 0.219; G256c4
  (256 x 256, 1/64, **4 correctors**) 0.563; G256s (256 x 256, 1/256) 1.166; G256 (256 x 256, 1/64) 2.418; G64x256 (64 x 256,
  1/64) 4.359; G256p2 (PIMPLE n_outer 2, alpha 0.9) 2.433. Twins: every arm with rho(step 1) <= 1.166 ran clean, every arm with
  rho(step 1) >= 2.418 was spoky or diverged (near t = 1 the failing arms had 1.68 to 1.95). The leading mode sits in the
  first cell layer at the wall in every failing arm.
* Fixed corrector counts on 256 x 256 at dt 1/64: 2 spoky (M_hf 48.3 at t = 3); 3 and 4 clean (9.2e-8, 9.1e-8); on 64 x 256 at
  dt 1/64 (in-plane wall aspect 18.8): 2 diverges at step 30 (K1), 3 is spoky (K2, M_hf 282 at t = 3).
* Workarounds (one thread, a shared machine): fractional step (W3) 29 s per simulated second, cd 1.466 (t = 3) / 1.291 (t = 10);
  three correctors (W5) 49 s, 1.466 / 1.296; the user's own 256 x 256 PISO-2 run 22.8 s per simulated second. The spoke metric
  `M_hf` and the clean limit L3 (`M_hf <= max(2 M_hf(R64, t), 0.01)`: 0.01 at t = 1, 0.0114 at t = 3, 0.01 at t = 10) are ADR
  0154's.

### Data seen before this registration (postdictions where they enter a limit)

* The user's committed-case run U1 (64 x 48, dt 1/16, PISO-2, to t = 80; `E:\projects\cfd\w48t-work\user-runs\U1`, read here):
  cl peak to peak 0.45 at t = 10 to 15, 0.60 to 0.62 from t = 40; upward cl zero crossings after t = 40 at 42.30, 48.89, 55.49,
  62.08, 68.67, 75.27: period 6.59, **St 0.1517**; mean cd over 5-unit windows 1.32 from t = 30. So the case sheds from about
  t = 10 (the push), not from t = 40 as ADR 0154 assumed.
* Job 48S (ADR 0153, its probes P2-P6, read in its worktree): 256 x 256 PISO-2 is unstable at dt 0.007 and 0.005 (Courant about
  1), slowly growing at 0.0042, decaying at 0.0035; `n_correctors` 3 stable at 0.005; its Studio plan runs with
  `solver.max_courant = 0.5`.
* **ADR 0077 amendment 3 (C4)**: when the in-step estimate (20 iterations, step 1) was first enabled for every corrector count, the
  instrumented `cargo test --workspace --release` run logged 233 transient simulations with an estimate and 10 warnings (rho >= 1)
  in 3 existing tests, all with two correctors: `user_scalars` u3 (8 runs of a fluid at rest in a closed 128 x 2 or 256 x 2 box,
  rho 1.46 to 8.14; ADR 0077 calls them false positives because nothing excites them), `rheology`
  `transient_runs_use_the_law_too` (rho 4.17, not classified) and `transient_sources` t5 (rho 5.37, not classified; its test says
  the T5b set-up at dt 0.02 is "beyond the stability bound"). The check was then restricted to one corrector. At the level 1.3
  every one of these would warn. Whether they are false positives (the estimate over-predicts) or true ones (a lag-unstable
  coupling that the test never excites) has not been measured; B2c measures it.
* No run of this job's code exists; nothing below has been computed with it.

### The code (orientation, no run)

* `crates/rcfd-fv/src/pimple.rs` `step_with`: per outer iteration the momentum matrices are assembled and solved, then
  `for corr in 0..s.n_correctors` (HbyA from the current velocity, transient Rhie-Chow carry-over, pressure solve, flux and
  velocity correction; `alpha_p` on the last corrector of non-final outer iterations). The ADR 0073 check
  (`pressure_lag_check: Option<LagCheck>`) keeps a copy of the final outer iteration's matrices (`LagCtx`) and estimates `rho`
  by power iteration of `lag_map` (homogeneous conditions, `s.n_correctors` correctors per outer iteration, `iterations`
  iterations, the second half's mean log growth), on copies: the solution is unchanged (ADR 0073 B4e).
* `crates/rcfd-api/src/sim.rs` enables that check only with `n_correctors == 1` (ADR 0077 amendment 3) at step 1 (step 2 for
  bdf2), warn by default (`solver.pressure_lag_check`, values warn | raise | off); warnings reach the status stream as `warning`
  events through `take_runtime_warnings`.
* Setup rules (`rules.rs`, `rules/setup_rules.json`, ADR 0031) are static conditions on the resolved settings and the mesh
  quality, evaluated by `rcfd check`, `rcfd validate`, `rcfd run` and `--check-only` (`runctl::check_only`, which also builds
  the `Simulation`); `rcfd run` passes their warnings to `runctl::drive` in `info.warnings`, which emits them as `warning` events
  after `start`. `rcfd-runner` runs `rcfd run --check-only --status-json` before a launch when a run declares no memory need and
  keeps the `check` event in `check.json`.
* **How defaults enter the case hash.** `case_hash` (the `start` event, `journal.get`) and the settings hash of `rcfd-io` case
  containers are `SettingsTree::content_hash` = FNV-1a of `serde_json::to_string(&data)`, and `data` holds every schema key with
  its default filled in (`coerce` inserts `default_value()` for every absent field). Adding any key with a default therefore
  changes the hash of every case (ADR 0133 question 5 recorded the same for ADR 0124's keys). A new key needs a hash-neutral
  default to leave every existing case hash unchanged (D6).
* `PimpleSettings` is built only by `PimpleSettings::piso` and `::pimple` (no struct literal elsewhere), `StepInfo` is built in
  `rcfd-api` too (so it is not changed here), `Schema` is built only through `Schema::of` (no literal outside `settings.rs`).

## Decisions (D1-D8)

**D1. Key.** `solver.corrector_control`, enumeration `"fixed"` (default) | `"adaptive"`. `fixed`: exactly `solver.n_correctors`
correctors per outer iteration, the existing code path. `adaptive`: at least `solver.n_correctors`, as many more as the stopping
criterion D2 asks for, at most 10 (D4). The name says what the user changes (the control of the corrector count), `adaptive` says
what the solver then does; "converged" was not chosen for the value because the criterion is the coupling's stability margin, not
a residual target (D2).

**D2. Stopping criterion: the pressure-lag amplification with a safety factor 2.** The correctors of a step are counted up until
the ADR 0073 estimate of the per-step amplification of a lagged pressure error, made with that many correctors on the step's
frozen matrices, is at most **0.5** (safety factor 2 below the instability at 1). Why this and not a per-step residual tolerance:
(a) the 48T evidence is in exactly this quantity: every failing arm had rho >= 2.418 at step 1, every clean arm <= 1.166, and four
correctors on 256 x 256 gave 0.563; a target of 0.5 is below the largest clean value by a factor 2.3 and below the failing ones by
a factor 4.8 or more; (b) it is amplitude-independent: the lag mode starts at round-off level, invisible in any residual of the
step next to the physical pressure change, so a residual-controlled loop only reacts once the mode is large enough to show and
then holds it at the tolerance level with an amplification near 1 (no margin); (c) PISO's correctors are a non-iterative
splitting by design (two correctors keep the splitting error below the time error), so a per-step "until converged" tolerance would
add correctors to every healthy case (the coarse cases' second-corrector change is not small compared with any tolerance that
also catches the spokes). The search is the "iterate correctors until the lag-amplification estimate falls below a registered
safety factor" of the request. Predicted counts from DG1: 64 x 48 at 1/16: 2 (0.358); 128 x 96 at 1/32: 3 (0.746 > 0.5);
256 x 256 at 1/64: 5 (4 correctors gave 0.563); 64 x 256 at 1/64: 5 or more, possibly the cap.

**D3. When the count is chosen (per step, from estimates).**
1. The count `n` in use starts at `n_lo = solver.n_correctors`.
2. **Estimate steps**: the first step this solver takes at or after step `start` (1; 2 under `bdf2` / `bdf2_bounded`, whose step 1
   is implicit Euler, as the ADR 0077 check); then whenever 1000 steps have passed since the last estimate; and at the step after
   the growth monitor (5) fired, when at least 25 steps have passed since the last triggered estimate.
3. At an estimate step, after the final outer iteration's momentum matrices are assembled and before its correctors, the ADR 0073
   map with N correctors is estimated with **20** power iterations (the `LagCheck::new` default), warm-started from the previous
   estimate's mode (the ADR 0073 start vector for the first one). First estimate: N = n_lo, then N + 1, ... until rho(N) <= 0.5 or
   N = 10. Later estimates: rho(n); above 0.5, count up as before; at or below 0.5 and n > n_lo, rho(n - 1), and n - 1 is taken
   when it is at or below 0.5 (at most one step down per estimate).
4. Every outer iteration of the step from the estimate on, and the following steps, use `n` correctors (`alpha_p` of a non-final
   outer iteration stays on its last corrector). The estimates run on copies: a step's solution is the fixed-count step's with
   `n_correctors = n` (A1, A4 check this bit for bit).
5. **Growth monitor (per step, cheap)**: `d_s` = largest cell change of the pressure in the last corrector of the step's final outer
   iteration, `R_s` = pressure range after the step. A step with `d_s > 1.5 d_(s-1)` and `d_s > 1e-10 R_s` counts one; three such
   steps in a row fire the monitor (the count restarts after an estimate). The ADR 0073 lag mode grows by rho per step, so once it
   dominates `d_s` (rho >= 1.68 in every failing 48T arm) it fires within three steps; physical growth (the shedding onset, a few
   per cent per time unit) and the decaying impulsive start do not.

**D4. Cap and reports.** Cap **10** correctors (the schema's upper bound of `n_correctors`). An estimate whose search ends at 10
with rho > 0.5 binds the cap: a `warning` status event (`kind` `corrector_cap`, the step, rho, the cell and its position, the
remedy "lower solver.dt"); the count of cap-bound estimates is in every `iteration` event (`corrector_cap`) and in the `finish`
event. With `adaptive` every `iteration` event carries `correctors` (the count of the step's final outer iteration) and
`corrector_cap`; the `finish` event carries `correctors` = {mode, steps, mean, max, last, estimates, cap_bound, triggered}. With
`fixed` no event gains a key (N5). `rcfd run`'s human iteration line appends `correctors <n>` when the event has it. The runner's
`residuals.jsonl` copies only `u`, `p`, `continuity` and `monitors` (`crates/rcfd-runner`, not changed here, D8).

**D5. Scope.** `adaptive` applies to the incompressible transient kernel (`PimpleSolver`, `solver.algorithm` piso or pimple,
`time_advance = "iterative"`). It is a located error at `solver.corrector_control` with a steady or coupled algorithm, with
`time_advance = "fractional_step"`, with variable density, VOF or compressible runs (their own corrector loops), with a dynamic mesh
(the lag map is not defined there, as for the ADR 0077 check) and with `solver.error_tolerance > 0` (step doubling restores
states, which would replay estimates). With `adaptive` the ADR 0077 one-corrector check is not enabled (the adaptive count
supersedes it; a case with `n_correctors = 1` starts its search at 1).

**D6. Hash-neutral default.** `rcfd-core` `Schema` gets `hash_omit_default: bool` (default false; builder
`Schema::hash_omit_default()`); `SettingsTree::content_hash` leaves out every object field so flagged whose value equals its
default (fields rebuilt in the data's own order, so the serialised text equals the text without the key), and hashes exactly as
before when no field of the schema is flagged. Only `solver.corrector_control` is flagged. So every existing case keeps its
`case_hash`; a case that sets `"adaptive"` has another hash; one that writes `"fixed"` explicitly has the hash of the case
without it (its data are the same). `to_json`, `--print-settings` and the JSON schema list the key (with its default) like any
other key.

**D7. Pre-run warning SV301 (the maintainer's approved item 1).** A computed setup rule (it needs a built simulation, so it is not
in `setup_rules.json`): id `SV301`, severity warning, category `solver`, path `solver.corrector_control`, fix
`{"path": "solver.corrector_control", "value": "adaptive"}`. Made for the incompressible transient kernel with
`time_advance = "iterative"`, no dynamic mesh, `n_correctors >= 2`, `corrector_control = "fixed"` and
`pressure_lag_check != "off"`: a copy of the solver at its current state (the real solver is not touched) takes the next step
(two steps from step 0 under bdf2 / bdf2_bounded) with the ADR 0073 check at that step, **60** power iterations (DG1's, on which
the level is registered), the case's `n_correctors`. It fires when rho >= **1.3** (between the largest clean step-1 value 1.166
and the smallest failing one 2.418). Message: the cause (the estimate, its value, the cell and position, "typically a thin wall
cell", what the run would show) and the one-key remedy (`solver.corrector_control = "adaptive"`). Shown by `rcfd check`,
`rcfd run --check-only` (the `check` event's `rules`, so also the runner's `check.json`) and `rcfd run` (stderr, and a `warning`
status event with `rule` `SV301` after `start`, like the static rules' warnings). Not made by `rcfd validate`, the chain driver,
the run service (`service.rs`) or the MCP tools (stated, not wired). Cost: one step plus 60 lag-map applications (about 61
step-equivalents of linear algebra).

**D8. Studio integration (prepared, not wired; no edit of `crates/rcfd-studio`).** The hand-off is written in the addendum after the
results (which override, where, what the user sees, which tests).

## Predictions (registered with the limits; from DG1 and the code)

* `fixed` (every existing case): results, events, case hashes unchanged.
* `adaptive`: 64 x 48 at 1/16 keeps 2 correctors throughout and is bit-identical to `fixed` (rho 0.358); 128 x 96 at 1/32 takes 3;
  256 x 256 at 1/64 takes 5; 64 x 256 at 1/64 takes 5 to 10; all clean.
* SV301 fires on 256 x 256 at 1/64 and at 0.0156 and on 64 x 256 at 1/64; not on 64 x 48, 128 x 96, 256 x 256 at 1/256 (1.166, the
  closest call), nor on grading 10.88; the preflight reproduces DG1's step-1 values (same step, same estimate).
* No user-facing case warns (B2a); the existing tests' set-ups warn where ADR 0077 C4 saw two-corrector estimates above 1.3 and on
  ADR 0154's 64 x 256 set-ups (B2b); prediction for B2c: the u3 boxes, the rheology channel at nu up to 10 and the t5 Hartmann set-up
  are lag-unstable when perturbed (true positives: large diffusion numbers on cells of in-plane aspect 6 to 20, the cylinder wall
  mechanism), but this is the least certain prediction of this ADR.

## Registered checks and limits

Common: release build of this branch; `rcfd run` through the command line `rcfd-runner` builds (as ADR 0154's
`scripts/w48t_run_matrix.py`, with `--status-every 1 --status-monitors --store-dir`); in-process checks through `Simulation`
(ADR 0154's harness). `M_hf` by `scripts/w48t_spoke_metrics.py` on the stored p (store dumps), or by the Rust `m_hf` of
`w48t_cylinder_spokes.rs` in-process. The clean limit L3 at t = 1, 3, 10 is 0.01, 0.0114, 0.01.

### N: defaults unchanged

| Id | Check | Limit |
|---|---|---|
| N1 | `baseline_hashes` | passes; the file is not edited |
| N2 | `w33b_default_bits` (five fingerprints) | passes unchanged |
| N3 | `content_hash` of every case file under `cases/`, `example-projects/*/cases/`, `example-projects-linux/*/cases/` (read as `rcfd run` reads them) at aad08fe and on this branch | identical for every file (0 differences); on the branch, a case with `corrector_control = "fixed"` written out hashes equal to the same case without it, `"adaptive"` differently |
| N4 | `cargo test --release` of rcfd-core, rcfd-fv, rcfd-api, rcfd-cli, rcfd-runner, and of the rcfd-studio tests that parse, edit or plan case settings (`editor_b5`, `drafts_o`, `json_o`, `case_starter_g2`, `saved_store_r`, `start_plan_r10`, `store_plan_sf`) | 0 failures; the ignored set is the earlier one plus this job's new ignored tests |
| N5 | status stream of a `fixed` run (the committed cylinder, 20 steps) | no `correctors` / `corrector_cap` key in any event; the `finish` event has no `correctors` |

### A: the opt-in

| Id | Check | Limit |
|---|---|---|
| A1 | The committed 64 x 48 case (dt 1/16) with `adaptive` to t = 10 against `fixed`, in-process | count 2 at every step; ux, uy, p and the face flux bit-identical at t = 10 |
| A2 | K1's set-up (64 x 256, dt 1/64, `n_correctors` 2) with `adaptive`, in-process to t = 3 (`w48v_k1_...`, **not ignored**) | not stopped by the guard, t = 3 reached, `M_hf <= 0.0114` |
| A3 | K2's set-up (`n_correctors` 3) with `adaptive` (`w48v_k2_...`, **not ignored**) | as A2 |
| A4 | A2's run against `fixed` with `n_correctors` = the count A2 chose (when it did not change over the run) | bit-identical at t = 3 |
| A5 | Effective amplification: the count chosen at the first estimate of A1, the 128 x 96 and 256 x 256 runs (V128, V256 set-ups) and A2, and a 60-iteration cold estimate (DG1 harness) of the lag map with that count | the chosen count's 20-iteration estimate <= 0.5 (or the cap reported); the 60-iteration estimate < 1 |
| A6 | Cost: mean correctors per step over t in (0, 10] for V64, V128, V256, V64x256 (`correctors` of the iteration events); extra = mean - 2 | reported (no limit) |
| A7 | Wall time per simulated second over (0, 3], one thread each, the four runs started together: TV256 (`adaptive`), TW5 (`n_correctors` 3), TW3 (fractional step), TR256 (default, the control) on 256 x 256 at 1/64; and against 48T's W3 (29 s) and W5 (49 s) | reported (no limit); TR256 spoky by L3 at t = 1 and 3 (the control fails without the opt-in) |
| A8 | PIMPLE: K1's mesh with `algorithm = "pimple"`, `n_outer` 2, `alpha_u` = `alpha_p` = 0.9 (W1's coupling) and `adaptive`, in-process to t = 3 (ignored, heavy) | as A2 |
| A9 | Located errors (D5): `adaptive` with simplec, `fractional_step`, a dynamic mesh, `error_tolerance` 1e-3, a variable-density pimple case | each refused at `solver.corrector_control` |
| A10 | Reports: an `adaptive` run's events (V64x256's stream) | every `iteration` event has `correctors` (2 to 10) and `corrector_cap`; `finish` has `correctors` with `estimates >= 1`; a cap-bound estimate, if any, has its `warning` event |

### B: the pre-run warning

| Id | Check | Limit |
|---|---|---|
| B0 | Preflight rho against DG1's step-1 values: G64, G128, G256 (1/64), G64x256, G256s, G256g | each within 1e-3 of DG1 (0.358, 0.746, 2.418, 4.359, 1.166, 0.219) |
| B1 | Fires / not: SV301 on 256 x 256 at 1/64 and at 0.0156, on 64 x 256 at 1/64; none on 64 x 48, 128 x 96 at 1/32, 256 x 256 at 1/256, grading 10.88, `adaptive`, `fractional_step`, `pressure_lag_check = "off"` | exactly as listed |
| B2a | User-facing and literal cases: every case text of ADR 0031's E2 collection (`cases/**/*.toml`, the fenced TOML blocks of `docs/case-schema.md` and `docs/run-control.md`, the non-template case literals of `crates/*/tests/*.rs`) and every `example-projects*/*/cases/*` file, through the preflight where it applies | 0 warnings; the list of cases checked (applicable and skipped, with the reason) in the addendum |
| B2b | Census of the existing tests' set-ups (ADR 0077 C4's method): a scratch tree (not committed) whose `Simulation::new` makes the preflight for every applicable case and logs rho, the test thread and the resolved settings; `cargo test --release -p rcfd-api` once | reported: the number of applicable simulations, every one with rho >= 1.3 (predicted: the u3, rheology and t5 set-ups of ADR 0077 C4, and ADR 0154's 64 x 256 set-ups) |
| B2c | Each distinct B2b set-up with rho >= 1.3 is classified by a perturbation run: the logged settings, the fixed coupling, from the case's initial state with and without p0 += 1e-6 x the ADR 0073 start vector (normalised to max 1), 60 steps; g = (\|dp(60)\| / \|dp(30)\|)^(1/30), dp the difference of the two runs' p (volume mean removed), stopped runs counted as growth | true positive when g > 1.05 (or the perturbed run is stopped by the guard), false positive when g < 0.95, else undetermined; **limit: zero false positives** |
| B3 | `rcfd check --json` on the 64 x 256 set-up | `rules` holds SV301 (warning, `solver`, path `solver.corrector_control`, fix `adaptive`); exit code 0 as before |
| B4 | `rcfd run` of the 64 x 256 set-up (2 steps) | a `warning` event with `rule` `SV301` after `start` and before the first `iteration`; stderr names `solver.corrector_control` |
| B5 | The preflight changes no result: the committed case by `rcfd run`, 20 steps, `pressure_lag_check = "warn"` (preflight made, no warning) against `"off"` (no preflight) | every `iteration` event equal in index, time, res_u, res_p, continuity and monitors; `store.rts` byte-identical |

### C: the user's case family (all `adaptive`, through `rcfd run`)

| Id | Run | Limit |
|---|---|---|
| C1 | V256 (256 x 256, grading 100, dt 1/64) | clean at t = 1, 3, 10 (L3) |
| C2 | V256 | cd(3) in [1.444, 1.488] (1.466 +- 1.5 %), cd(10) in [1.2716, 1.3154] (1.291 to 1.296, +- 1.5 %) |
| C3 | V256 to t = 100 (shedding; wall cap 10 h by `--max-elapsed`) | `M_hf <= 0.0114` at every stored time; cl peak to peak over [50, 100] >= 0.3; St and mean cd as C5 |
| C4 | V512 (512 x 512, dt 1/128) to t = 10 (stretch; wall cap 10 h) | clean at t = 1, 3, 10 |
| C5 | Resolution sweep V64 (64 x 48, 1/16), V128 (128 x 96, 1/32), V256 to t = 100; St = 1 / (mean period of the upward zero crossings of cl - mean(cl) in [50, 100]), mean cd over the whole periods between the first and the last crossing | each St in [0.145, 0.175] and mean cd in [1.25, 1.45]; \|St(V256) - St(V128)\| <= 0.008 and \|cd(V256) - cd(V128)\| <= 0.04 (V64's St 0.1517 is a postdiction from U1, for which A1 predicts bit-identity) |

A run stopped by its wall cap is evaluated on what it stored and reached (St over [50, t_end] when it holds at least three
periods, else "not reached"); that is a reading rule, not a pass.

### K: controls

K1 and K2 of ADR 0154 (`w48t_k1_..._known_miss`, `w48t_k2_..._known_miss`) run once on this branch: they must still fail as
registered (the opt-in is off there); they stay ignored with their limits. TR256 (A7) is the 256 x 256 control.

## Run matrix (`scripts/w48v_run_matrix.py`, work folder `E:\projects\cfd\w48v-work`, outside the repository)

| Id | Overrides of the committed case | End | Threads | Wall cap |
|---|---|---|---|---|
| V64 | `corrector_control = adaptive` | 100 | 1 | 2 h |
| V128 | nx 128, ny 96, dt 1/32, adaptive | 100 | 2 | 6 h |
| V256 | nx 256, ny 256, dt 1/64, adaptive | 100 | 4 | 10 h |
| V512 | nx 512, ny 512, dt 1/128, adaptive | 10 | 4 | 10 h |
| V64x256 | nx 64, ny 256, dt 1/64, adaptive | 10 | 1 | 2 h |
| TV256, TW5, TW3, TR256 | 256 x 256, dt 1/64: adaptive; `n_correctors` 3; `time_advance` fractional_step; none | 3 | 1 each, started together | 2 h |

B3, B4 and B5 run through the binary in `crates/rcfd-cli/tests/w48v_coupling_warning.rs` (temporary folders); A1-A5, A8, A9, B0,
B1 in-process. Free RAM is checked before the long runs (other agents share the 94 GB machine); runs of the matrix go 4 to 6 at a time.

## Tools (written after this registration, before their first use)

* `crates/rcfd-api/tests/w48v_adaptive_correctors.rs`: A1, A4, A5 (DG1-style harness), A8, A9, N3's hash printer (ignored), N5,
  A10's event checks on a short run, B0, B1, B2a (ignored, prints the list), B2c (ignored, reads B2b's log), B3/B4's library parts,
  the store dumper for the metrics (the same as ADR 0154's `w48t_dump_stores`, so that `crates/rcfd-studio` is not built).
* B2b's instrumentation: a scratch copy of the tree outside the repository (`E:\projects\cfd\w48v-work\b2b`), an environment
  variable read in `Simulation::new`, never committed (as ADR 0077's A3a/C4 instrumentation).
* `crates/rcfd-api/tests/w48t_cylinder_spokes.rs`: A2 and A3 next to K1 and K2, not ignored.
* `crates/rcfd-cli/tests/w48v_coupling_warning.rs`: B3, B4, B5 through the binary.
* `crates/rcfd-fv/tests/w48v_corrector_control.rs`: the library's own checks (adaptive = fixed when the count stays; the search to
  the cap with a target of 0 equals a fixed run with 10 correctors; the growth monitor's counting).
* `scripts/w48v_run_matrix.py`, `scripts/w48v_shedding.py` (St, mean cd, cl amplitude, the correctors per step from a status
  stream).

## Amendment 1 (2026-10-29, after the code commit 56473b8 and before any registered run): threads of the long runs

Before the first registered run the machine was saturated by another agent's WSL/Docker workload (total CPU 92 to 94 %,
`vmmemWSL` 39.7 GB, 14.5 GB free), and job 48S measured that threads buy about 1.1x on this solver (ADR 0153 P3). Results do
not depend on the thread count (`docs/run-control.md`, measured; ADR 0154 ran with 1 or 2). Changed, for wall time only: V256
and V512 run with **2** threads (registered 4), V128 with **1** (registered 2); every other entry of the run matrix (overrides,
end times, wall caps) and every limit is unchanged. The A7 runs keep one thread each, started together. No run of this job had
been made when this was written (the development runs before it were compilation and the library tests of
`crates/rcfd-fv/tests/w48v_corrector_control.rs`, which are not registered checks, and `w33b_default_bits`, N2, which passed).

## Amendment 2 (2026-10-29, after A9's first run and before its corrected run): A9's simplec arm

A9 as registered stopped at its first arm: the example case with `algorithm = "simplec"` is refused by the case parser before
any simulation is built, because the example sets `output.store_time_interval`, a key of transient runs ("a steady run
(solver.algorithm = "simplec") has no simulated time; use output.store_interval"). That is a defect of the set-up (the arm
never reached the key under test); the other four arms did not run. Seen before this amendment: only that message. The
registered test is kept as `w48v_a9_..._known_miss` (ignored, limit unchanged, the defect named). Corrected test
`w48v_a9b_...`: the same five arms and limit (each refused at `solver.corrector_control`), the simplec arm's text with
`store_time_interval = 1.0` replaced by `store_interval = 50`.

## Amendment 3 (2026-10-29, after the addendum's runs and before any run of A11): the cap report end to end

Every registered run ended with no cap-bound estimate, so the cap's reports (the `corrector_cap` warning event, the
`corrector_cap` count of the `iteration` events, `finish.correctors.cap_bound`) were checked only at library level
(`w48v_corrector_control.rs`, target 0); A10 passed them vacuously. Added: **A11**, `w48v_a11_...` in
`crates/rcfd-api/tests/w48v_adaptive_correctors.rs`: K1's mesh (64 x 256) with `adaptive` at dt 1/16, 1/4 and 1, in this order;
each arm is driven through `runctl::drive` for 3 steps (fewer if the guard stops it); the first arm whose first estimate is
cap-bound is the checked arm. Limit on that arm: a `warning` event with `kind = "corrector_cap"`, `correctors` 10 and `rho` above
0.5; `corrector_cap >= 1` in every `iteration` event after it; `finish.correctors.cap_bound >= 1`. If no arm binds the cap, A11 is
reported as not exercised and no other arm is added. Seen before this amendment: only that the registered runs never bound the
cap; 48T's B2 (64 x 256 at dt 1/16, two correctors) diverged at step 19.

## Not changed by this job

`crates/rcfd-studio`, `crates/rcfd-runner`, the default of every key, the ADR 0077 one-corrector check, `baseline_hashes.rs`, the
example projects and the user's files.

## Addendum (2026-10-29): results against the registered limits

Everything below was measured after the registration and its two amendments; no limit or rule was changed after a result was
seen. Code at 56473b8 (the A9 test changed in d2ee3d2, amendment 2); the runs through `scripts/w48v_run_matrix.py` with
`E:\projects\cfd\w48v-work\rcfd-bbc34b4.exe` (56473b8's code); work folder `E:\projects\cfd\w48v-work` (runs, store dumps,
`metrics-*.json`, `shedding-final.json`, the census log). The machine was shared with another agent's WSL/Docker workload (total
CPU 92 to 100 %, 8 to 15 GB free) for the whole job, so wall times are rough and comparable only within the A7 group.

### The answer

`solver.corrector_control = "adaptive"` makes the solver choose its own corrector count from the mesh and the step. It took 2 on
the committed 64 x 48 (and is then bit-identical to today's default), 3 on 128 x 96, 5 on the user's 256 x 256, 6 and then 5 on
512 x 512, and 8 on the stretched 64 x 256 wall layer (in-plane wall aspect 18.8) where no hand-picked count of 3 worked; 4 per
outer iteration for PIMPLE with 2 outer iterations on that mesh (W1's coupling, which diverged in 48T). At the chosen counts the
estimated per-step amplification of a lagged pressure error is 0.29 to 0.48, against 2.42 to 4.36 with the default two
correctors. Every refined run is clean at every stored time, K1 and K2 are passing tests, the cap was never needed, and the
resolution sweep converges (St 0.1517, 0.1614, 0.1646 and mean cd 1.324, 1.349, 1.355 for 64 x 48, 128 x 96, 256 x 256). The
pre-run warning SV301 fires exactly on the refined set-ups of the user's family, on no user-facing case, and reproduces 48T's
DG1 values; the census of the test suite found it on two families of existing test set-ups (B2b, B2c below).

### Results

| Check | Limit (registered) | Measured | Outcome |
|---|---|---|---|
| N1 | `baseline_hashes` passes, file not edited | `baseline_hashes`: 3 passed (shipped cases, cavity, heated cavity), 1 ignored (its existing `rotating_channel` known difference); the file has no diff against aad08fe | met |
| N2 | five fingerprints unchanged | `w33b_default_bits` passed (e9dca234e513bec8, 72d935a8d4993955, e46fbe511183e9fc, c21a6553acb2a7a6, 2c71144215f1d245), also in the N4 run | met |
| N3 | 0 differences between aad08fe and the branch | 36 entries (`cases/**`, `example-projects*/*/cases/*`) equal: 30 case files with their hash, 6 `cases/*/case.json` registry descriptors (not case files) refused identically; the example case: 6ac54a8827aedab0 with the key absent, written `"fixed"` or set `"fixed"`; 5dad1499c3c6a5cb with `"adaptive"` | met |
| N4 | 0 failures | rcfd-core, rcfd-fv, rcfd-cli, rcfd-runner: 690 passed, 0 failed, 242 ignored (140 test binaries); rcfd-api: 726 passed, 0 failed, 267 ignored (143); rcfd-studio's seven listed files: 63 passed, 0 failed; the only `#[ignore]` lines added are this job's 11 new tests, none removed | met |
| N5 | no corrector key in a fixed run's events | none (the committed case, 3 steps through `runctl::drive`) | met |
| A1 | count 2 at every step; bit-identical at t = 10 | 160 steps, count 2 (0.3628 at step 1); ux, uy, p and face flux bit-identical | met |
| A2 | K1 + adaptive: t = 3, not stopped, M_hf <= 0.0114 | 192 steps, M_hf 5.707e-3, count 8 (0.4707) | **met: K1 is a passing test** |
| A3 | K2 + adaptive: as A2 | 192 steps, M_hf 5.707e-3, count 8 (0.4705) | **met: K2 is a passing test** |
| A4 | A2 equals fixed at its count | count constant (8); equal to `n_correctors = 8` bit for bit | met |
| A5 | chosen count's 20-iteration estimate <= 0.5; 60-iteration cold estimate < 1 | 64 x 48: 2 (0.3628; 0.3578); 128 x 96: 3 (0.2863; 0.2851); 256 x 256: 5 (0.3612; 0.3576); 64 x 256: 8 (0.4707; 0.4714) | met (margin 2.1 to 3.5) |
| A6 | reported | mean correctors per step over (0, 10]: V64 2.000 (extra 0), V128 3.000 (+1.0), V256 5.000 (+3.0), V64x256 8.000 (+6.0), V512 5.781 (+3.8) | reported |
| A7 | reported; TR256 spoky | below | reported; TR256 spoky (the control fails without the opt-in) |
| A8 | PIMPLE n_outer 2, alpha 0.9 + adaptive on K1's mesh | 192 steps, M_hf 7.540e-3, 4 correctors per outer iteration (0.4714) | met |
| A9 | each arm refused at `solver.corrector_control` | registered test stopped at its simplec arm (set-up defect, amendment 2); A9b: simplec, fractional_step, dynamic mesh, error_tolerance, low-Mach pimple refused there, the piston and low-Mach texts without the key build | A9: known miss (set-up defect, ignored); A9b met |
| A10 | correctors in every iteration event, finish summary | V64x256: 640 of 640 events (`correctors` 8, `corrector_cap` 0), `finish` summary (1 estimate); also V256 (6400 events, 7 estimates) and V512 (1280 events, counts 6 and 5); no cap-bound estimate anywhere, so no `corrector_cap` warning | met |
| B0 | within 1e-3 of DG1 | 0.357819, 0.745564, 2.418431, 4.358667, 1.165589, 0.219239 (G64, G128, G256, G64x256, G256s, G256g); largest difference 4.4e-4 (DG1 quoted to 3 decimals) | met |
| B1 | SV301 exactly on the three refined set-ups | fires on 256 x 256 at 1/64 and 0.0156 (2.42) and 64 x 256 (4.36); not on 64 x 48, 128 x 96, 256 x 256 at 1/256, grading 10.88, adaptive, fractional_step, `pressure_lag_check = "off"` | met (10 of 10) |
| B2a | 0 warnings | 302 distinct texts; 14 with the preflight (the example cylinders 0.358 and 0.271; 12 test literals, the largest 0.989); 288 skipped (120 not a case, 104 simplec, 48 templates, 5 compressible, 5 simple, 3 dynamic mesh, 2 adaptive, 1 coupled) | met |
| B2b | reported | 248 applicable simulations (101 tests, 173 distinct set-ups); 10 distinct set-ups with rho >= 1.3 (eight u3 boxes 1.47 to 8.16, t5's Hartmann step 31.3, this job's B3 cylinder 4.36); none between 1 and 1.3 (below) | reported |
| B2c | zero false positives | 5 true positives (t5 and two u3 boxes stopped by the guard once perturbed, one u3 box growing at g 1.17, the B3 cylinder), 5 undetermined (u3 boxes, g 0.965 to 1.022), 0 false positives | met |
| B3 | SV301 in `rcfd check --json`, exit 0 | SV301 (warning, solver, path, fix `adaptive`), 4.36, exit 0; human line printed; none on the committed case | met |
| B4 | warning SV301 after `start`, before the first `iteration` | `start`, `warning:SV301`, `iteration`, `iteration`, `checkpoint`, `finish`; stderr names the key | met |
| B5 | events equal, stores byte-identical | 20 = 20 iteration events equal; `store.rts` 892 365 bytes, byte-identical | met |
| C1 | V256 clean at t = 1, 3, 10 | M_hf 2.68e-7, 9.06e-8, 5.31e-8 | met |
| C2 | cd(3) in [1.444, 1.488], cd(10) in [1.2716, 1.3154] | 1.4657, 1.2956 | met |
| C3 | V256 to t = 100: M_hf <= 0.0114 at every stored time; cl peak to peak >= 0.3 over [50, 100]; St and cd as C5 | largest M_hf of 100 snapshots 2.68e-7 (t = 1), 8.6e-8 over t >= 50; cl peak to peak 0.679; St 0.1646, mean cd 1.3550; finished in 5609 s (cap 10 h) | met |
| C4 | V512 clean at t = 1, 3, 10 | 3.23e-8, 1.94e-8, 1.59e-8; cd(3) 1.4650, cd(10) 1.2997; 5266 s | met |
| C5 | each St in [0.145, 0.175], mean cd in [1.25, 1.45]; \|dSt\| <= 0.008, \|dcd\| <= 0.04 between V256 and V128 | St 0.1517 / 0.1614 / 0.1646; mean cd 1.3237 / 1.3488 / 1.3550; \|dSt\| 0.0032, \|dcd\| 0.0062 | met |
| K | K1, K2 fail as registered with the opt-in off | K1 stopped at step 30 (t = 0.453); K2 t = 3 with M_hf 282.35 | the controls fail, as registered |

### The resolution sweep (C5, A6; all `adaptive`, grading 100, Courant-matched step)

| Run | Mesh, dt | Correctors (estimates) | M_hf largest stored | cd(3), cd(10) | St, mean cd over [50, 100] | Wall per simulated second |
|---|---|---|---|---|---|---|
| V64 | 64 x 48, 1/16 | 2 (0.363 at step 1, 0.262 at 1001) | 7.4e-3 (t = 2; the physical part at 64 cells around: the run is the fixed one, A1) | 1.474, 1.272 | 0.1517, 1.3237 | 0.49 s (1 thread) |
| V128 | 128 x 96, 1/32 | 3 (0.286) | 4.4e-5 | 1.467, 1.285 | 0.1614, 1.3488 | 5.7 s (1 thread) |
| V256 | 256 x 256, 1/64 | 5 (0.361, then 0.325 to 0.333 at the six later estimates) | 2.7e-7 | 1.466, 1.296 | 0.1646, 1.3550 | 56 s (2 threads) |
| V512 | 512 x 512, 1/128 | 6, from step 1001 5 (0.354; 0.480) | 3.2e-8 (to t = 10) | 1.465, 1.300 | (to t = 10 only) | 526 s (2 threads) |
| V64x256 | 64 x 256, 1/64 | 8 (0.471) | 8.5e-3 (t = 2; the 64-around physical part) | 1.470, 1.296 | (to t = 10 only) | 54 s (1 thread) |

The Strouhal number and the mean drag converge with refinement (the differences fall from 0.0097 to 0.0032 and from 0.025 to
0.006). 48T's values for comparison: R128 cd(10) 1.286, the 256 x 256 workarounds 1.466 / 1.291 to 1.303.

### Cost (A6, A7)

Paired runs on 256 x 256 at dt 1/64, one thread each, started together, steps 2 to 192:

| Run | Coupling | s per step | s per simulated second | M_hf t = 1 / 3 |
|---|---|---|---|---|
| TV256 | adaptive (5 correctors) | 1.057 | 67.6 | 2.68e-7 / 9.06e-8 |
| TW5 | `n_correctors = 3` | 0.780 | 49.9 (48T: 49) | 2.72e-7 / 9.20e-8 |
| TW3 | `time_advance = "fractional_step"` | 0.459 | 29.4 (48T: 29) | 1.92e-7 / 7.88e-8 |
| TR256 | default PISO-2 (control) | 0.667 | 42.7 | 42.6 / 48.3 (spoky) |

TV256's first step, with the search over 2, 3, 4, 5 correctors, took 52.4 s (about 50 of its steps; 0.8 % of a 100-unit run).
The five correctors cost 1.36 x the three-corrector workaround and 2.3 x the fractional step per step; the factor-2 margin is what
makes it five rather than four (DG1: four correctors 0.563). A fixed count can make 64 x 256 stable: 8 (A4), which the control
finds by itself; 3 cannot (K2).

### The census and its classification (B2b, B2c)

**B2b.** The instrumented copy (`E:\projects\cfd\w48v-work\b2b`, bbc34b4 plus the logging in `Simulation::new`) ran
`cargo test --release -p rcfd-api` once: 725 passed, 266 ignored, 1 failed (the registered A9, whose set-up defect amendment 2
records; the copy predates A9b). It built 248 simulations to which the preflight applies (101 tests, 173 distinct set-ups). Ten
distinct set-ups have rho >= 1.3: the eight closed boxes at rest of `user_scalars` u3 (1.466, 1.894, 2.624, 3.222, 4.143, 4.819
on 256 cells; 6.438, 8.156 on 512), the Hartmann set-up of `transient_sources` t5 at dt 0.02 (31.31, 320 cells; its own file calls
that step "beyond the stability bound"), and the 64 x 256 cylinder of this job's B3 test (4.359, the intended positive). None
lies between 1 and 1.3, and the largest below 1 is 0.989 (a 12 800-cell registry set-up). Against the prediction: u3, t5 and the
64 x 256 cylinder found; ADR 0077 C4's rheology channel not (0.011 here, see the limitation below); ADR 0154's K tests are ignored,
so only B3's instance was in the suite.

**B2c.** Five true positives: t5 (the perturbed run stopped by the guard at step 24 while the unperturbed one runs 60 steps), u3
at 8.156 and 6.438 (stopped at steps 14 and 16), u3 at 1.466 (g 1.171, |dp| 3.6e-5 to 4.1e-3 over steps 30 to 60) and the B3
cylinder (both runs stopped at step 30, as K1). Five undetermined: u3 at 4.819, 4.143, 3.222, 2.624, 1.894 (g 1.000, 1.006, 1.022,
1.000, 0.965). No false positive: the limit is met. Reading: where the warning fires on the cylinder family it is confirmed (B0,
B1, DG1, K1); on five of the eight diffusion-only boxes (cells of in-plane aspect 6.4 and 12.8, nu dt / dx^2 from 4 to 33) a
perturbation neither grows nor decays within 60 steps, so there the estimate is an upper indication, not borne out. None of the
ten is a user-facing case, and the tests themselves drive `Simulation` directly and see no warning; SV301 would show for them only
through `rcfd check` or `rcfd run`.

### A limitation found by the census (not a registered check)

The pre-run estimate runs the solver as built. Where the Simulation sets the solver's face viscosity before every step from a law
(rheology, eddy viscosity of a turbulence model, a user-function or temperature-dependent viscosity), the trial step of the
preflight uses the solver's viscosity before that law is applied (the constant `physics.nu` at step 0). Example: the power-law
channel of `rheology.rs` `transient_runs_use_the_law_too` gives 0.011 in the preflight, where ADR 0077's in-step estimate (with
the law's viscosity, up to its cap 10) gave 4.17. So SV301 can miss such cases (a false negative). The adaptive control itself is
not affected: its estimates run inside the real step, on the matrices built with the viscosity the Simulation has just set.
Recommendation (question 7): give the preflight the Simulation's own first-step viscosity (a read-only version of the per-step
preparation in `Simulation::step`), or make the estimate in-step at step 1 for those cases.

### D8. Studio hand-off (prepared, not wired; `crates/rcfd-studio` and `crates/rcfd-runner` unchanged)

For the follow-up job that turns the opt-in on by default in the Studio's run plan, so that a Studio user who refines a mesh gets
the adaptive count without touching a setting:

1. **The override.** One run-time `--set` entry, `solver.corrector_control=adaptive`, added by the Studio's run plan
   (`StartPlan::set()`, whose entries the runner records in `run.json` `settings.set`; ADR 0149, 0152, 0153). It is never written
   into the case file or the case editor (ADR 0153's rule for run-time settings).
2. **When.** For a case whose plan facts are a transient incompressible `piso` or `pimple` run with the iterative time advance
   (not `time_advance = "fractional_step"`), constant density, not VOF, not `compressible`, no `dynamic_mesh`, no
   `solver.error_tolerance`: exactly the cases `adaptive` accepts (D5; any other case would be refused with a located error at
   `solver.corrector_control`). Only when the case does not choose the key itself; by ADR 0152's rule a key written at its schema
   default (`"fixed"`, which the Studio editor's full save writes out) is not the case's own, so a saved case still gets the
   proposal, and a user who wants fixed correctors sets `solver.corrector_control=fixed` in the Start run dialog, whose overrides
   come later in `set()` and win (the last `--set` of a key applies). Question 4 asks whether to keep that rule here.
3. **Order in `set()`.** After the store proposal (ADR 0140) and after 48S's time-step entries (`solver.dt`, `solver.max_courant`,
   `solver.max_dt`, `solver.end_time`), before the Start run dialog's overrides.
4. **What the user sees.** The Run hover and ADR 0153's "overrides the case's ..." indicator name it ("pressure correctors adaptive
   for this run"). Without the override, the pre-launch `check.json` (the runner's `rcfd run --check-only`) and `events.jsonl` (the
   runner forwards every `warning` event) already carry SV301 with its fix, and `corrector_cap` when it happens; the run log can
   show each `iteration` event's `correctors`. `residuals.jsonl` does not carry the count: `crates/rcfd-runner` `drain` copies only
   `u`, `p`, `continuity` and `monitors`; the follow-up adds `correctors` and `corrector_cap` there (for example `"coupling":
   {"correctors": n, "cap": c}`), so the residual plot can show them.
5. **With 48S's automatic step.** 48S's plan runs the Courant controller (`max_courant` 0.5, the step growing to `max_dt`). The
   count is chosen at the first step's dt and re-estimated every 1000 steps and when the growth monitor fires; a growing step
   raises the wall diffusion number and so the needed count: the monitor covers a count that becomes too small, the 1000-step
   estimates a count that becomes too large. A re-estimate when the step has changed by a factor (for example 1.5) since the last
   one would be a small addition (not registered here).
6. **Tests for the follow-up (to register there).** (i) the plan of the shipped cylinder case holds the entry; the quick start's
   cavity (steady) and a `fractional_step` variant do not; a case that sets `adaptive` itself gets no second entry; (ii) `run.json`
   `settings.set` records it and `case.snapshot` stays the file's bytes; (iii) a Run of the 256 x 256 refine through the Studio ends
   `finished`, clean at t = 1 and 3 (`M_hf`), with `finish.correctors.mode = "adaptive"`; (iv) ADR 0152's DV6 record
   (`plans_6867e40.txt`, never re-recorded) changes for its transient entries: register the expected difference, as ADR 0152 did.

### Deviations

1. The worktree started at 1248a76, the merge base with local main; it was fast-forwarded to local main aad08fe (48A to 48T, 48P,
   48Q, 48R merged) before any work.
2. Amendment 1: V128, V256 and V512 ran with 1, 2, 2 threads instead of 2, 4, 4 (wall time only).
3. Amendment 2: A9's simplec arm kept the example's transient store key (set-up defect); the registered A9 stays an ignored known
   miss, A9b is the corrected test.
4. Tool placement: A1, A4, A5, A8, B0, B1 and the user-mesh twin are in `w48t_cylinder_spokes.rs` (registered:
   `w48v_adaptive_correctors.rs`), next to ADR 0154's harness and `m_hf`; the N3 printer is its own file
   (`w48v_case_hashes.rs`) so that it builds on aad08fe unchanged.
5. `Simulation::perturb_pressure`, a public hook not named in the registration, was written for B2c (the schema has no initial
   pressure field and the restart route excludes the census's cases).
6. The census tree was cut at bbc34b4, before d2ee3d2: its run includes the registered A9 (the set-up defect, failing as expected)
   and not A9b.
7. The registration predicted no count for 512 x 512; it took 6 and from step 1001 5.
8. The analysis scripts ran with the user's site packages (numpy) without `python -I`; they read only this job's own files.

### Not run

* The whole gate (workspace fmt, clippy, tests); run: rustfmt on the touched crates, `cargo clippy -p rcfd-core -p rcfd-fv
  -p rcfd-api -p rcfd-cli --all-targets -- -D warnings` (clean), N4's suites.
* Linux.
* The Studio itself (not edited; only N4's listed tests); SV301 in the run service, the MCP tools, the chain driver and
  `rcfd validate` (stated as not wired in D7).
* V512 and V64x256 into the shedding phase (registered to t = 10).
* The census covered rcfd-api's suite only (as registered); the simulations built by other crates' tests (rcfd-studio,
  rcfd-runner, rcfd-cli through the binary) were not instrumented.
* The developed shedding of the user's own dt 0.0156 with the opt-in (V256 used the Courant-matched 1/64, which differs from
  0.0156 by 0.16 %; B1 shows the same estimate, 2.42, for both).

### Questions for the maintainer (with recommendations)

1. **Turn the opt-in on by default in the Studio's run plan (D8)?** Recommendation: yes, as the follow-up of D8; it is what makes a
   refinement in the Studio stable without a setting, and on the committed case it changes nothing (A1: bit-identical). Keep the
   library default `fixed` until a registered benchmark of a default flip.
2. **The target 0.5 (safety factor 2) gives five correctors on the user's mesh (1.36 x three correctors per step).** Keep it, or
   lower the margin (0.75 would take four there, DG1's 0.563)? Recommendation: keep 0.5: 48T's clean and failing step-1 estimates are
   separated only by the band 1.17 to 2.42, the estimate's spread is small (20 against 60 iterations within 1.5 %), and the margin
   also covers later flow states; `fractional_step` stays the cheaper choice for a user who wants speed on a fixed mesh.
3. **SV301 costs one step plus 60 lag-map applications per `rcfd check`, `--check-only` and `rcfd run` of a transient two-corrector
   case (20 to 30 s on 65 536 cells on this loaded machine, B0 and B1; twice when the runner makes its pre-launch check).** Accept, or use
   20 iterations (A5: within 1.5 %)? Recommendation: keep 60 for now (the level is registered on 60-iteration data); drop the copy in
   `rcfd run` once the Studio reads SV301 from `check.json`.
4. **An explicit `corrector_control = "fixed"` in a case text:** should the Studio's plan honour it (no proposal), or treat it as a
   key at its default (ADR 0152's rule, so the editor's full save does not lock users out of the fix)? Recommendation: ADR 0152's
   rule; fixed correctors through the Start run dialog's override.
5. **Wire SV301 into the run service, the MCP tools and the chain driver?** Recommendation: yes, in a follow-up with its own checks;
   `Simulation::pressure_lag_preflight` and `rules::lag_preflight_rule` are ready for it.
6. **`residuals.jsonl` without the corrector count:** add `correctors` and `corrector_cap` in `crates/rcfd-runner` `drain` with the
   Studio follow-up? Recommendation: yes (no change for fixed runs).
7. **The preflight's viscosity (the limitation above):** give it the Simulation's first-step viscosity? Recommendation: yes, as a
   registered follow-up with the rheology and RANS set-ups of the census as its checks.
8. **SV301 on existing test set-ups:** it would fire on nine of them (u3's eight closed boxes at rest, t5's Hartmann step at
   dt 0.02) if they were run through `rcfd check` or `rcfd run`; four are shown unstable when perturbed, five undetermined. Raise
   the level, or accept? Recommendation: accept: none is a user-facing case, the warning says "likely", and its remedy is harmless
   (on such a box `adaptive` just adds correctors); revisit the level only with a registered study of non-cylinder families.
