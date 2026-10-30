# ADR 0172: Job 49J, set-up warnings (queue items A2 and A3): SV301 at the settled step, one warning path for every run entry, a new static warning SV302 for unlimited linear-upwind on thin skewed wall cells, a guard hint that names the limiter, and the adaptive corrector count in the Studio plot and the MCP `read_residuals`

Status: **registered 2026-10-30**, before any implementation result of this job and before any run. Amendments follow
this text, each written before the runs it covers; results go in an addendum. Not yet accepted by the maintainer.

Job 49J, on local main aef580d (worktree branch `job49j-warnings`). Number: 0171 is the highest on main; checked just
before writing (job 49I took none).

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered tests and runs; a missed limit becomes an ignored `*_known_miss` test
  with the limit intact; a defective set-up gets an amendment before the corrected run;
- defaults of existing code stay bit-identical: warnings may be added, no numerical result, case hash or run output
  changes (the new `residuals.jsonl` reading is read-only; no file is written differently);
  `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no new third-party crate, the crate graph stays acyclic, no commit trailers, nothing pushed.

Clean-room: our own code and our own ADRs only.

## The request (the maintainer, queue of 2026-10-30, relayed)

A2, set-up warnings: (1) SV301 at the step the run will actually use, not only at the initial step; (2) the static
set-up warnings and SV301 in `run.headless` and in the chain driver, with one code path; (3) a new static warning for
unlimited `solver.linear_upwind_limiter` on wall cells with high aspect times tan(non-orthogonality) (ADR 0171 findings
and question 2); (4) the divergence-guard hint names the limiter when the failing cell is a wall cell under unlimited
`linear_upwind` (ADR 0171 question 3). A3: the adaptive corrector count and cap (`correctors`, `corrector_cap`, ADR 0160
D4) in the Studio's residual plot and in the MCP `read_residuals`.

## What I read (rule d), nothing run

- [ADR 0156](0156-job48v-adaptive-pressure-correctors.md) D7 / [ADR 0160](0160-job48z-end-time-stamp-corrector-count-record-sv301-wiring.md) D5: SV301
  (`rules::lag_preflight_rule`, level `SV301_LEVEL` 1.3) is made by `Simulation::pressure_lag_preflight()` at `self.dt`
  (`solver.dt`, the first step) with `LAG_PREFLIGHT_ITERATIONS` (60) cold power iterations on a copy of the solver. It is
  wired in `runctl::check_only` (and so `run.check`, MCP `check_case_headless`), in `rcfd run` (`rcfd-cli/src/main.rs`,
  after restart / continue-from) and in `run.headless` (only when `iteration() == 0`, into `start.info.warnings`).
- [ADR 0162](0162-job48ab-adaptive-correctors-follow-the-time-step.md) question 4: under the automatic step the
  preflight sees the first step's dt, "a gap, not a miss", and its recommendation is the follow-up of this item. The
  adaptive-corrector path already re-estimates when the step has changed by a factor 1.5 (ADR 0162 D2), but only with
  `corrector_control = "adaptive"`; with fixed correctors nothing watches the step.
- Static warnings (`rules::check_case`, `RuleSet::builtin()` plus MT101) are shown by `rcfd run` (`cmd_w19c::rules_gate`,
  errors give exit 3), `check_only`, the chain driver (`chain::run_stage`, errors refuse, warnings go in `info.warnings`)
  and the design/study paths; `run.headless` shows none of them, the chain driver shows no SV301.
- SV101 (non-orthogonality above 70 degrees), SV102, SV103 (aspect above 1000) exist as mesh-wide warnings; M1 trips SV101
  and SV103 today. They say "expect slower convergence" and do not name a remedy or a scheme.
- The divergence-guard message of the steady solvers ends in "lower the relaxation factors (solver.alpha_u, solver.alpha_p)
  or use a more dissipative convection scheme" (`SteadyDivergence::message`, `crates/rcfd-fv/src/simple.rs`).
  `Simulation::steady_divergence_message` appends a note for `linear_upwind` (ADR 0077 part B / ADR 0081) that already
  mentions the limiter, after that text.
- The limiter applies only to the steady incompressible `simple` / `simplec` engine with `solver.convection =
  "linear_upwind"` (anything else is an error at build time).
- `residuals.jsonl` lines carry `correctors` and `corrector_cap` only for runs with `corrector_control = "adaptive"`
  (`rcfd_runner::residual_line`); `rcfd_project::records::ResidualLine` has the two optional fields; the Studio worker
  (`poll.rs`) puts `residuals` and `monitors` into the plot stores and ignores the two; `rcfd_mcp::project::residual_series`
  returns `residuals.<eq>` and `monitors.<name>` only.
- ADR 0171 numbers: aspect times tan(max non-orthogonality) of the trailing-edge wall cell was about 21 000 on M1, 6 900 on
  M1w05, 1 600 on M1w15; the unlimited scheme diverged at 6 degrees on M1 (iteration 15, cell 0) and M1w05 (28), not on
  M1w15, and M1 at 0 degrees is clean; `barth_jespersen` made all five 300-iteration runs clean and costs +3.0 % of
  `cd_eff` at 0 degrees (LL4), all in the pressure drag.

## Decisions

**D1. The settled step of SV301.** `Simulation::settled_dt_estimate()` is the step the run will use after its controller
has settled, estimated before the run: with the adaptive controller of ADR 0050 (`timectl`, `error_tol` off) and with the
legacy `solver.max_courant` controller, `dt_ref * aim / Co(dt_ref)` (the controller's own rule: `aim = safety *
target` for the active Courant criteria, `Co` the largest / rms Courant number of the present flux at `dt_ref`, the
proposed next step) clamped to the controller's `min_dt` / `max_dt` (the legacy controller: at most `solver.dt`); when
the present flux gives no Courant number (rest start, `Co` = 0) or the controller is not Courant-driven, the next
proposed step; for a fixed step `solver.dt`, bit-identical to today. `pressure_lag_preflight()` uses it; a new
`pressure_lag_preflight_at(dt)` takes the step as an argument (the old `pressure_lag_preflight()` on a fixed step is
`..._at(self.dt)` exactly). The pre-run estimate cannot see a flow that has not developed, so a **run-time watch** backs
it: `runctl::drive` keeps a `LagWatch` for runs where the preflight applies (the same conditions as
`pressure_lag_preflight`: incompressible transient, `corrector_control` fixed, `n_correctors >= 2`, no dynamic mesh,
`pressure_lag_check != "off"`) and, when the step has changed by a factor of 1.5 up or down since the last evaluation, or
has stayed within 2 % for 10 consecutive steps after changing by more than 5 % since the last evaluation (the controller
has settled), evaluates `Simulation::lag_estimate_next_step(LAG_PREFLIGHT_ITERATIONS)` (a copy of the solver, ADR 0162 D5;
no change of the run) and emits a `warning` event, rule `SV301`, at most once per run, when the estimate is at least
`SV301_LEVEL`; the message names the step, the time step and the estimate, and repeats the remedy. If the pre-run SV301
already fired (`info.warnings` holds it) the watch is silent. Steps that landed on a write or end time (the fixed step's
short last step) are not evaluated.

**D2. One warning path.** `rules::setup_diagnostics(case, sim, mesh, q)` is `check_case` (static rules, MT101) followed by
`rules::dynamic_setup_diagnostics(sim, mesh)`: SV301 (`lag_preflight_rule`) and SV302 (D3). The caller picks the moment:
`rcfd run` calls `check_case` first for its error gate (exit 3, as before) and the dynamic part after the restart is applied
(as SV301 is today, so a continued run estimates its restarted state); `check_only` calls `setup_diagnostics` (the SV301
order after the static rules is unchanged); the chain driver calls it per stage after the stage's restart; `run.headless`
calls it at the start of a run (`iteration() == 0`) and puts the **warning-severity** diagnostics in `start.info.warnings`
and as `warning` events, as SV301 is today. `run.headless` does not refuse a case with a rule *error* (it never did; a
refusal would change what a call returns); the errors are not shown there (question 1). Output of `rcfd run`, `run.check`
and the chain driver is unchanged except for the added diagnostics.

**D3. SV302, the static warning.** Rule `SV302`, severity warning, category `solver`, path `solver.linear_upwind_limiter`,
no `fix` object (the limiter has a cost and is an accuracy decision, question 2). Metric: for every wall cell (a cell
with a face on a `PatchKind::Wall` patch) `W = aspect_ratio(c) * tan(theta_c)`, `aspect_ratio` and the face
non-orthogonality as `rcfd_mesh::quality::compute_metrics` defines them (aspect: longest over shortest edge of the cell,
the span edge included), `theta_c` the largest non-orthogonality over all faces of the cell, clamped to 89.9 degrees;
`W_max` is the maximum over wall cells. The rule fires when the engine is the steady incompressible `simple` / `simplec`
solver, `solver.convection = "linear_upwind"`, `solver.linear_upwind_limiter = "none"` and `W_max >= SV302_LEVEL = 3000`.
The level lies between the ADR 0171 values of M1w15 (1 600, clean) and M1w05 (6 900, diverged at 6 degrees) and is a
judgement, not a fit. The message is a risk notice: it gives `W_max`, the cell and its position and the two factors; says
that this combination (unlimited linear-upwind on thin, strongly skewed wall cells) diverged in ADR 0171's NACA 0012 runs at
y+ about 2 once the flow was off-axis, that it is a risk, not a prediction (the same metric value did not diverge on M1 at
0 degrees); names `solver.linear_upwind_limiter = "barth_jespersen"` and its cost (+3 % of `cd_eff` at 0 degrees in ADR
0171, all in the pressure drag); and says that lowering the relaxation factors did not help there. **Honest about false
positives:** the rule fires on M1 at 0 degrees, which ran clean for 8000 iterations, and on any thin-celled skewed wall mesh
that would have converged; it does not fire on a stable run whose sliver cells are not wall cells, nor on a diverging run
whose wall cells stay under the level. The default is unchanged (the limiter stays opt-in).

**D4. The guard hint.** `SteadyDivergence::message_with_hint(centres, hint)` replaces the tail "lower the relaxation factors
(solver.alpha_u, solver.alpha_p) or use a more dissipative convection scheme" by `hint` when it is `Some`;
`message(centres)` is `message_with_hint(centres, None)` and byte-identical to today. `Simulation::steady_divergence_message`
passes the hint "the failing cell is a wall cell and solver.convection = linear_upwind is unlimited: set
solver.linear_upwind_limiter = \"barth_jespersen\" (ADR 0171: lowering the relaxation factors does not help here)" when
the cell has a face on a wall patch, `solver.convection = "linear_upwind"`, the limiter is `none` and the engine is the
steady solver; otherwise the text is unchanged (including the existing ADR 0077 / 0081 / 0102 notes).

**D5. A3.** `rcfd_mcp::project::residual_series` adds the series `correctors` and `corrector_cap` (against the step) from
the lines that carry them; a run without them returns exactly what it returned (no new key). `read_residuals`' description
names them. The Studio worker (`poll.rs`) puts the same two series into the shown run's **monitor** store (the "Monitors"
plot, linear axis: the residual plot is log10 and a count of 2 to 10 would be squeezed into 0.3 to 1), under the names
`correctors` and `corrector_cap`; nothing changes for a run without them (question 3).

## Test cases (fixed now)

- **K1** (ADR 0160): the committed `example-projects/wind-tunnel/cases/cylinder-re100.toml` with `mesh.nx = 64`, `mesh.ny =
  256`, `solver.dt = 0.015625` (PISO, fixed correctors): the SV301 positive, step-1 rho 4.30 to 4.36 (ADR 0160 P1, V1).
  The committed 64 x 48, dt 1/16 case is the negative.
- **Controlled K1 variants** (through `settings.set`): C-small = K1 with `solver.max_courant = 0.01` (legacy controller);
  C-grow = K1 with `solver.dt = 0.0009765625` (1/1024), `solver.max_dt = 0.015625`, `solver.max_courant = 1000` (the
  adaptive controller grows the step by its factor 1.2 per step to `max_dt`; the Courant criterion never binds).
- **The 49H meshes** M1, M1w05, M1w15 (sha256 `5d5c8917...523f4`, `8e04edea...54451c`, `62b49fc8...b4aac`, ADR 0171), rebuilt
  by ADR 0171's scripts (`scripts/w48y_aoa_sweep.py`, `scripts/w49e_sa_wall.py`; verified here to the same hashes), the cases
  of `scripts/w49h_te_divergence.py` (RA: M1 at 0 degrees; RB: M1 at 6 degrees; RC, RD: M1w05; RF: M1w15 at 6 degrees; QA: RB
  with `barth_jespersen`). These tests read the meshes from the folder named by `RCFD_W49J_WORK` and are skipped (printing
  why) when it is unset; I run them with it set, once.
- **A sheared wall-stretched channel** (library mesh `channel` with geometric grading toward the walls, sheared by 70
  degrees, written as polyMesh) for the tests that need a thin skewed wall cell without the 49H meshes.

## Limits (fixed before any test or run)

### SV301 at the settled step

| Limit | Statement |
|---|---|
| LS1 | **Fixed step unchanged:** on K1, `pressure_lag_preflight()` equals `pressure_lag_preflight_at(solver.dt)` bit for bit (rho, cell, position); every existing SV301 test (`sv301_service_w48z`, `sv301_mcp_w48z`, `w48v_coupling_warning`, `w48v_adaptive_correctors`, `correctors_x`) passes unchanged |
| LS2 | **A transient large initial step does not warn:** C-small: `settled_dt_estimate() <= solver.dt / 10`, and `run.check` on it has no SV301 (K1 itself has one, LS1) |
| LS3 | **A step that grows into the risk warns before the run:** C-grow: `settled_dt_estimate()` equals `solver.max_dt` (1/64), the preflight is the K1 estimate within 1e-9 relative, `run.check` has SV301, whereas the estimate at the first step (`..._at(1/1024)`) is below `SV301_LEVEL` |
| LS4 | **The watch finds a risk the pre-run estimate did not:** `runctl::drive` on C-grow built with `Simulation::new` (no pre-run call, `info` without warnings) for 60 steps emits exactly one `warning` event with rule `SV301` after `start`, at a step after the first |
| LS5 | **The watch is silent after a pre-run SV301:** the same drive with `info.warnings` holding an SV301 object emits no further SV301 event |
| LS6 | **The watch changes no result:** the velocity and pressure fields after the 60 steps of LS4's drive are bit-identical to those of the same 60 steps taken by `Simulation::step` in a loop |
| LS7 | **The watch is cheap:** at most 12 evaluations in LS4's 60 steps (counted through a test counter of `lag_estimate_next_step` calls made by the watch) |

### One warning path

| Limit | Statement |
|---|---|
| LP1 | `run.headless` on a steady cavity session with `solver.tolerance_u = 0.1` (SV105) emits the SV105 `warning` event after `start` and before the first `iteration`, and `start.info.warnings` holds it; the committed example case emits none (as ADR 0160 P3) |
| LP2 | the chain driver: a chain whose stage case trips SV105 has that warning in the stage's `start.info.warnings` and as a `warning` event; a K1 stage has SV301 likewise (neither was so before) |
| LP3 | `run.check`, `rcfd run` (stderr and `start.info.warnings`) and the chain driver's lists of one case agree: for the sheared-channel case, the same SV302 text (rule id, path, message) in all four |
| LP4 | every existing test of `rcfd-api`, `rcfd-cli`, `rcfd-mcp` passes unchanged |

### SV302

| Limit | Statement |
|---|---|
| LW1 | **Fires on M1** (RB's case and RA's: `W_max >= 3000`) |
| LW2 | **Fires on M1w05** (RD's case) |
| LW3 | **Does not fire on M1w15** (RF's case) |
| LW4 | **Value:** `W_max` of M1, M1w05, M1w15 is within 30 % of ADR 0171's 21 000, 6 900, 1 600 (it checks that this metric is the one ADR 0171 used) |
| LW5 | **Documented false positive:** the rule fires on RA (M1 at 0 degrees, clean in ADR 0171); the test asserts that it fires and carries the comment that this is a registered false positive |
| LW6 | **Silent when the cause is removed:** QA (M1, `barth_jespersen`), and M1 with `convection = "upwind"` do not fire; a transient (piso) case on a high-`W` mesh does not fire |
| LW7 | **Message:** severity warning, no `fix` object, names `solver.linear_upwind_limiter`, `barth_jespersen`, "3 %" and the words "risk" and "0 degrees"; `rcfd check` exits 0 on M1's case (a warning, not an error) |
| LW8 | **Shipped examples:** `rcfd check` of every case under `example-projects/` exits as before (no example is refused or changes exit status); which ones carry SV302 is reported |
| LW9 | **Unit:** on the sheared-channel mesh `W_max` equals the hand value `aspect * tan(70 degrees)` of the wall cell within 1 % and the rule fires; on the unsheared stretched channel (non-orthogonality 0) it does not |

### Guard hint

| Limit | Statement |
|---|---|
| LH1 | `message(centres)` is byte-identical to the text before this job (a fixed string test on a constructed `SteadyDivergence`) |
| LH2 | **Names the limiter:** `Simulation::steady_divergence_message` for a divergence at a wall cell of an unlimited `linear_upwind` steady case contains `solver.linear_upwind_limiter` and `barth_jespersen` and does not contain "lower the relaxation factors" |
| LH3 | **Unchanged elsewhere:** the same divergence at a non-wall cell, with the limiter on, or with `convection = "upwind"`, gives the old text (still containing "lower the relaxation factors") |
| LH4 | **End to end:** RB (M1, 6 degrees, SA, `max_iterations` 300) stops at the guard before iteration 300 (ADR 0171: 15, cell 0) and its message passes LH2; not run if the stop does not reproduce, in which case it is reported |

### Correctors

| Limit | Statement |
|---|---|
| LC1 | MCP `read_residuals` on a `residuals.jsonl` with `correctors` / `corrector_cap` returns series `correctors` and `corrector_cap` with the file's steps and values; with `names` = `["correctors"]` it returns that series only; a file without the fields returns the same keys as before |
| LC2 | the Studio worker's `Series` update for such a file carries monitor columns `correctors` and `corrector_cap` with the file's values; a file without them carries neither |
| LC3 | Studio state after the update: `Studio::monitors` holds both series (the Monitors plot draws every series of that store, `report_names()` lists them); a run without them is unchanged |

### Bit-identical defaults

| Limit | Statement |
|---|---|
| LD1 | `baseline_hashes.rs` and `case_hashes_w48u.rs` pass unmodified |
| LD2 | no case file, no `output/` file and no `residuals.jsonl` line of any existing test run differs (the existing run tests pass) |

## Method and gates

Implementation first in this order: ADR (this text), then the solver-side library items (D1, D2, D3, D4), then A3. Tests as the
limits above name them. Builds with `CARGO_TARGET_DIR=E:\projects\cfd\w49j-work\target`; the Studio and GPU tests with
`--release --test-threads 1`; at most two heavy processes. Full-workspace gate on a git-archive copy per
`reference_gate_procedure` at the end, `--exclude rcfd-gui` for the Rust 1.90 job.

## Out of scope (stated now)

A change of any default, including making the limiter default; an automatic switch of the limiter; the C-grid generator;
rule errors in `run.headless`; the Studio's own pre-run SV301 display (it shows the runner's `check.json`).
