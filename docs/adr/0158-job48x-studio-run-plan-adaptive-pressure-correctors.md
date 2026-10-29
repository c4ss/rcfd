# ADR 0158: Job 48X, the Studio's Run makes the pressure correctors adaptive (`solver.corrector_control=adaptive` as a run-time setting), with a toolbar line, a switch in Advanced and coherent pre-run messages

Status: **registered** 2026-10-29, after the code and the tests below were written and compiled and before any registered
run (what had run before is listed under "What I already knew"); not yet accepted by the maintainer. House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed here before the registered runs; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before the
corrected run; defaults of existing code stay bit-identical outside the Studio's run plan; `rcfd run`, the runner and the
command line behave exactly as before; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new crate, no unsafe
code, the crate graph stays acyclic. Clean-room: our own Studio code, the repository's ADRs and the egui public API. Parents:
[ADR 0156](0156-job48v-adaptive-pressure-correctors.md) D8 (the hand-off this job carries out) and its question 1,
[ADR 0153](0153-job48s-time-step-set-when-a-run-starts.md) (the Studio's run-time settings, `StartPlan::time`, the toolbar's
time row), [ADR 0152](0152-job48r-store-key-at-its-default-is-not-the-case-s-own.md) (a key at its schema default is not the
case's own), [ADR 0154](0154-job48t-cylinder-ogrid-pressure-spokes.md) (the spoke metric `M_hf` and the clean limit L3).

## The request

The maintainer approved turning ADR 0156's opt-in on by default in the Studio ("yes turn on adaptive correctors", ADR 0156
question 1); the core idea (ADR 0156, "The request"): the simulation must be stable whatever the resolution, without
hand-tuning. Asked: the Studio's run plan adds the run-time override `--set solver.corrector_control=adaptive` for transient
iterative piso/pimple cases with constant density, no VOF, no dynamic mesh and no `solver.error_tolerance`; after job 48S's
time-step entries and before the user's own overrides; only when the case's key is at its default (ADR 0152's rule); shown in
the toolbar's run row and hover next to the step label; recorded in `run.json` `settings.set` only (the case file and its
snapshot untouched); switchable off in the Advanced popover; the pre-run Courant/SV301 messages coherent (no SV301 nag when
adaptive is on). Only `crates/rcfd-studio` and `docs`; a `residuals.jsonl` field (`crates/rcfd-runner`) is a question, not
this job's change.

## Context (read at local main 4e795b1, the merge of wave 48V)

* **Run's plan** (ADR 0153 D9): `quick_run::start_job` makes `StartPlan::new_route` and, for a case whose time-step facts
  (`time_step::facts`, the case's simulation built on the control thread) are `Kind::Transient`, adds `TimePlan`
  (`with_time`); `StartPlan::set()` gives store keys, time entries, then `extra_set` (empty for Run; the Start run dialog's
  overrides for the dialog's own plan, which has no time entries). The runner records `set` in `run.json` and passes each
  entry as `--set` to the pre-launch check (`rcfd run --check-only --status-json` into `check.json`) and to the run; it
  forwards every solver status event but `iteration` into `events.jsonl` (`{"kind": "solver", "solver": ...}`), and the
  solver's own stream stays in `solver-status.jsonl`.
* **`Kind::Transient`** (ADR 0153 D1) is piso/pimple, not compressible, not variable density, not VOF, not a prescribed
  flow, not a chain: in `rcfd-api` `Simulation::new` exactly the cases that build `Engine::Transient` (`PimpleSolver`), minus
  the prescribed flow and the chain. ADR 0156 D5 (`sim.rs`): `adaptive` is refused at `solver.corrector_control` unless the
  engine is `Engine::Transient` with piso/pimple, and with `time_advance = "fractional_step"`, a dynamic mesh or
  `solver.error_tolerance > 0`.
* **SV301** (ADR 0156 D7) is made only with `corrector_control = "fixed"`, two or more correctors, the iterative advance,
  no dynamic mesh and `pressure_lag_check != "off"`; with `adaptive` the solver makes no preflight, so a run with the entry
  can show no SV301 (neither in `check.json` nor in `events.jsonl`). The Studio itself shows no SV301 text anywhere today
  (it shows the run's events in the Log panel).
* **The automatic step and the count.** ADR 0153's (a) ran the 256 x 256 cylinder with the automatic step at a wall
  diffusion number of 5.4 to 5.7, below ADR 0154's onset (6.5 to 11.4); the step starts at 0.00116 s and grows to about
  0.0035 s within 70 steps; a 3 s run is about 900 steps, below ADR 0156's 1000-step re-estimate, and the growth monitor
  fired in no run of ADR 0156. So at the automatic step the adaptive count is chosen at the first step's dt and is expected
  to stay at the case's 2, where it is bit-identical to fixed correctors (ADR 0156 A1, A4). The case where the opt-in is
  needed is the maintainer's own step 1/64 (or 0.0156) on 256 x 256: ADR 0156 TV256 (adaptive, 5 correctors, clean) against
  TR256 (default, spoky, M_hf 42.6 / 48.3 at t = 1 / 3), and SV301 fires there at 2.42 (B1).

## What I already knew (no registered run before this registration)

The release build of `rcfd`, `rcfd-runner`, `fake-claude` and the `rcfd-studio` test binaries of the code below (compilation
only). No test of this job and no run of a case was made before this registration; every number in the predictions is
quoted from ADR 0153, 0154 and 0156.

## Decisions

**D1. The entry and its place.** Run's plan gets `StartPlan::correctors: Option<time_step::CorrectorPlan>`
(`with_correctors`), set in `start_job` in the same arm as `with_time` (the facts known and `Kind::Transient`, not the route
before ADR 0153: `Switches::case_time` adds neither). `set()` order: store keys, time entries (ADR 0153), then
`solver.corrector_control=adaptive` (`time_step::ADAPTIVE_ENTRY`, only for `CorrectorPlan::Adaptive`), then `extra_set` (the
last `--set` of a key applies). `TimePlan` and its `entries` are not changed (ADR 0153 S-G and D1-D4 read them). The plan's
hand-written `Debug` prints `correctors` only when set, so a plan without Run's settings prints as before (ADR 0152 DV6; ADR
0153 amendment 3's lesson).

**D2. When** (`time_step::corrector_plan`, `Correctors`, `not_adaptive`). New `Facts::correctors`: `solver.n_correctors` and
`solver.corrector_control` as resolved, and `not_adaptive`, the reason the solver would refuse the key (ADR 0156 D5): not
`Kind::Transient` (`a steady run has no time steps`, or the kind's own reason), `solver.time_advance = "fractional_step" (a
non-iterative step has no correctors)`, `dynamic_mesh (the pressure-lag estimate is not defined on a moving mesh)`,
`solver.error_tolerance (step doubling restores the state of trial steps)`. The plan: `Fixed { why: Scope(reason) }` when
there is a reason; `CaseAdaptive` (no entry) when the case resolves `adaptive` itself; `Fixed { why: Off }` when the
run-time switch is off; else `Adaptive { n }`. ADR 0152's rule: `"fixed"` is the schema default, so a text that spells it out
(the Studio editor's full save; any JSON case) gets the entry like one without the key (ADR 0156 question 4's
recommendation). A prescribed flow and a chain are left alone (ADR 0153's scope) even where the solver might accept the key.

**D3. The switch** (`RunTime::adaptive_correctors`, default `time_step::DEFAULT_ADAPTIVE_CORRECTORS` = true). Per case, in
memory, forgotten when the project closes (as ADR 0153's run-time settings). The Advanced popover gets a section "Pressure
correctors of this case's runs" with the checkbox `adaptive pressure correctors (the solver picks the count)`
(`quick_run_ui::CORRECTORS_BOX`), greyed out with the reason when the case sets `adaptive` itself or the solver has none for
it.

**D4. What the toolbar shows** (`time_step::corrector_label`, `corrector_hover`). A line of its own under the time row
(wrapped to the window, so that ADR 0153's row keeps its width: ADR 0153 amendment 2), for a transient case whose time plan is
made: `pressure correctors: adaptive for this run (at least <n>, at most 10; the solver adds them where the mesh needs them)`
(weak), `pressure correctors: fixed, <n> per step (adaptive switched off in Advanced)` (warning colour),
`pressure correctors: adaptive (the case's own solver.corrector_control)`, `pressure correctors: the case's own (<n> per step;
adaptive is not available for this case)`. Its hover, also a third line of Run's hover after ADR 0153's plan hover: what the
solver does (ADR 0156 D2-D4 in words), the measured cost (`CORRECTOR_COST`: 2 on the shipped 64 x 48 cylinder, bit-identical
to fixed; 3 on 128 x 96; 5 on 256 x 256 at dt 1/64, about 1.6 times the time per step of 2 fixed correctors (ADR 0156 A7:
1.057 / 0.667 s), plus a first-step search of about 50 steps' time), that the count is in the run's events, and that it is a
run-time setting switchable in Advanced. ADR 0153's step label, indicator (`overrides the case's ...`), Mesh line and plan
hover are not changed: by ADR 0152's rule the case's `"fixed"` is not the case's own, so nothing of the case's own is
overridden (a deviation from ADR 0156 D8 item 4's wording, which put the corrector in the indicator).

**D5. Coherent pre-run messages.** With the entry the solver makes no SV301 preflight, so no SV301 reaches `check.json` or the
events (no nag). With the switch off the line is in warning colour and its hover says that the solver's check before the run
warns (SV301, in the run's events) when the fixed count is likely unstable, with the fix the switch turns on. ADR 0153's
Courant warning (an explicit step above Courant 2) is unchanged: a separate cause with its own fix. The Studio does not
compute the pressure-lag estimate itself (60 lag-map applications, 20 to 30 s on 65,536 cells: ADR 0156 question 3).

**D6. The Start run dialog** keeps the case's own settings (ADR 0153 D12; its plan has no time and no corrector entry, so ADR
0140's and ADR 0152's dialog checks are unchanged) and draws, for a transient case whose solver accepts the key, the line
(`time_step::dialog_corrector_line`, the dialog's overrides read last-wins): `Pressure correctors: this dialog's run takes
solver.corrector_control = "<c>", <n> per step (the case's, unless an override sets them); Run on the toolbar makes them
adaptive (Advanced).` (or `... takes solver.corrector_control = "adaptive" (at least <n>, at most 10).`). Whether the dialog
should get the entry too is question 3.

**D7. Placement.** `crates/rcfd-studio` only: hunks in `src/{time_step,runs,quick_run,quick_run_ui,ui}.rs`; new test
`tests/correctors_x.rs`; the registered expectation changes of `tests/run_time_s.rs` below. No new crate, dependency or
workspace edge; `Cargo.toml`, `Cargo.lock`, `workspace_graph.rs`, `crates/rcfd-runner` and everything outside the Studio
unchanged.

## Predictions (registered with the limits)

* The four example cylinders (both trees, Re 100 and 200) get the entry; every other example and `cases/` file none (steady or
  compressible). Every set-up given the entry is built by the solver with it; every D5 reason is refused by it.
* X4: the shipped cylinder at the automatic step keeps 2 correctors at every step, so on and off are byte-identical in
  `monitors.csv` (ADR 0156 A1, at a smaller step here).
* X5 (a): 2 correctors at the first estimate (3 at most), no later estimate within the 3 s, clean; the control (switch off)
  clean too and without SV301: at the automatic step the opt-in is not needed on this mesh (Context), so (a)'s control does
  not discriminate; (b)'s does.
* X5 (b): 5 correctors at every step (ADR 0156 TV256), clean (TV256 M_hf 2.68e-7 / 9.06e-8); its control spoky (TR256 42.6
  / 48.3) with SV301 at about 2.42 in `check.json` and in a warning event.

## Registered checks and limits

Release builds of this registration's commit, `--test-threads 1 --nocapture`, output to files; `rcfd` and `rcfd-runner` of
the same build next to the test binaries (`common::binaries`). "Through Run": a served project, the Studio model with the
watcher and the run worker, the case selected and its facts known, `run_selected_case` (F5's call), the run waited for (as
ADR 0153's `run_time_s.rs`). File: `crates/rcfd-studio/tests/correctors_x.rs`.

**Reading rule for M_hf** (ADR 0154's definition, copied verbatim as `rings`/`m_hf`, Q = 0.5): on the run's field store
(`fields/`, the case's own store: one snapshot per simulated second, f32), the pressure `p` of the snapshot at the first stored
time >= 1 s (within a step after it) and of the last snapshot (the final state at 3 s); the cell centres from the case's mesh
as `Simulation::new` builds it. Clean (L3): M_hf <= 0.01 at the first and <= 0.0114 at the last; spoky: >= 0.1.

| Id | Check | Limit | Negative control (must fail) |
|---|---|---|---|
| X1 | Scope (model): with the default run-time settings, the shipped cylinder, the cylinder with `corrector_control = "fixed"` spelled out, as JSON, with `n_correctors = 3`, at 256 x 256; the cavity, step, heated cavity and ramped-lid cavity as PISO (ADR 0153's `ts` variants); the cylinder with `"adaptive"`; the cylinder with `fractional_step`, with `error_tolerance = 1e-3`; ADR 0156's piston (dynamic mesh) and low-Mach box (variable density); the quick start's steady cavity; `cases/sod` (compressible). Then every case file of `example-projects*/*/cases/` and `cases/*/` | `Adaptive { n: 2 }` (n 3 for the `n_correctors = 3` set-up) and the entry for the first group, each built by `Simulation::new` with the key; `CaseAdaptive`, no entry, for `"adaptive"`; `Fixed { Scope }` with the D2 reason's text and no entry for the rest, each refused by the solver at `solver.corrector_control`; of the files, exactly the 4 transient ones get the entry (each built with it), the others none | (the D5 set-ups are the in-test negatives) |
| X2 | Order (model): Run's plan of the shipped cylinder (default settings, its own store) | `set()` exactly `solver.end_time=80`, `solver.dt=0.00602`, `solver.max_courant=0.5`, `solver.max_dt=0.8`, `solver.corrector_control=adaptive`; with the override `solver.corrector_control=fixed` it follows last; with the switch off the first four only; `StartPlan::new`'s `Debug` text has neither `time:` nor `correctors`, a Run plan's has `correctors: Adaptive { n: 2 }` | the switch off (in-test) |
| X3 | Drawn (1280 x 800, tooltips at once): the shipped cylinder selected; (i) the second frame; (ii) the pointer on Run; (iii) a drawn click on Advanced; (iv) a drawn click on the switch, Escape; (v) Advanced and the switch again | (i) ADR 0153's `dt auto: 0.00602 s at the start, then adaptive (Courant 0.5)` and `overrides the case's solver.dt 0.0625 s for this run` and exactly `pressure correctors: adaptive for this run (at least 2, at most 10; the solver adds them where the mesh needs them)` drawn, the line's rectangle inside the window; (ii) `RUN_HOVER`, the plan's hover and the corrector hover drawn; (iii) `CORRECTORS_BOX` drawn; (iv) the run-time switch off and exactly `pressure correctors: fixed, 2 per step (adaptive switched off in Advanced)` drawn, the adaptive line not; (v) the adaptive line again; the case file's bytes unchanged, the editor clean | the route before (`case_time`): no text starting `pressure correctors` / `Pressure correctors` drawn in (i) or (ii) |
| X4 | The shipped cylinder through Run, "Simulate for" 2 s, the automatic step, on and off | on: `run.json` `set` exactly `solver.end_time=2`, `solver.dt=0.00602`, `solver.max_courant=0.5`, `solver.max_dt=0.02`, `solver.corrector_control=adaptive`; finished at 2 bit for bit; every `iteration` event of `solver-status.jsonl` has `correctors` 2; the `finish` event's `correctors` has `mode` `adaptive`, `max` 2, `cap_bound` 0; the case file and `case.snapshot` the file's bytes; the two runs' `monitors.csv` byte-identical | off: `set` exactly the four time entries, finished, no `correctors` in any event and none in `finish`, the file and snapshot unchanged |
| X5 (a) | The 256 x 256 cylinder through Run, the automatic step, "Simulate for" 3 s, on (heavy, ignored, run on purpose) | `set` exactly `solver.end_time=3`, `solver.dt=0.00116`, `solver.max_courant=0.5`, `solver.max_dt=0.03`, `solver.corrector_control=adaptive`; finished at 3 bit for bit; `finish.correctors.mode` `adaptive`; every `iteration` event with `correctors`; clean (L3); no SV301 in `check.json`'s `rules` nor in `events.jsonl`; the case file and `case.snapshot` its bytes | (a) control: the switch off: `set` exactly the four time entries, no corrector keys; M_hf and SV301 reported (predicted clean, none: not discriminating) |
| X5 (b) | The 256 x 256 cylinder with its own `solver.dt = 0.015625` (1/64), the automatic step off (`StepMode::Case`), "Simulate for" 3 s, on (heavy, ignored) | `set` exactly `solver.end_time=3`, `solver.corrector_control=adaptive`; finished at 3 bit for bit; `correctors` above 2 in every `iteration` event; clean (L3); no SV301; the file and snapshot unchanged | (b) control: the switch off must fail (b): `set` exactly `solver.end_time=3`; SV301 in `check.json` and in `events.jsonl`; spoky at 3 s (M_hf >= 0.1) |
| W | The whole `rcfd-studio` suite (`rcfd`, `rcfd-runner`, `fake-claude` built first), `cargo clippy -p rcfd-studio --release --all-targets -- -D warnings`, `cargo fmt -p rcfd-studio -- --check`, `workspace_graph` (`rcfd-cli`) | 0 failures; the ignored set is main's plus this job's four X5 tests; lints clean; `workspace_graph` passes | (their own) |
| N | Nothing outside the Studio | `git diff --stat 4e795b1` names only `crates/rcfd-studio/` and `docs/adr/0158-...` | |

Reported without a limit: wall times, the correctors per step and the estimates' log of every run, M_hf at both times, the
SV301 estimate of (b)'s control.

### Changes to earlier tests (registered here, before any run)

ADR 0153's `run_time_s.rs` asserts the exact `set` of Run's plans; with this job Run adds the corrector entry to every
transient case in scope. Treatment, each decided before any run:

1. **(d)** (`cd_ends_at_the_time_asked_for_and_leaves_the_case_alone`, part (d)): the expected list gains
   `solver.corrector_control=adaptive` as its fifth entry (the plan, `run.json` and the status bar's `queued run ... (rcfd run
   --set ...)`): the registered difference.
2. **(g1), (g2)**: the expected `run.json` `set` gains the same fifth entry (the heated and the ramped-lid cavity as PISO are
   in scope).
3. **(h)**: `run.json` `set` must equal the TOML twin's time entries followed by the twin's corrector entry
   (`corrector_plan(...).entry()`), so (h) still compares the JSON case's run with its TOML twin's plan.
4. **(a) control** (`a_control_the_case_step_on_256_is_far_above_the_target`): runs with the switch off, so that its set-up
   is the registered one (`set` exactly `solver.end_time=1.25`, the case's own step, two fixed correctors); its limits
   unchanged. (a) itself (heavy, ignored, not run here) runs with the switch on.
5. Unchanged and expected to pass unchanged: (b) (the shipped cylinder to 80 s keeps 2 correctors, bit-identical), (c), f3,
   (f2), the route-before controls (no time plan, so no corrector entry), ADR 0152's DV6 (the dialog's plans, printed as
   before), ADR 0153's S-G and D1-D4 (`TimePlan.entries` unchanged).

## Order of runs

1. X1, X2, X3 and its control, X4 (`cargo test --release -p rcfd-studio --test correctors_x`).
2. X5 (a), (b) and their controls (`-- --ignored x5`), two at a time, after checking the free memory (the machine is shared:
   at registration about 5 GB of 94 GB free, most held by another agent's WSL workload).
3. W, then `workspace_graph` when no Studio run holds `rcfd.exe`.

## Amendment 1 (2026-10-29, after the first registered run of X1-X4 on 405e219, before X2's corrected run)

The run of step 1 (`correctors_x`, 405e219): X1, X3, X3's control and X4 passed; **X2 failed at its first assertion**: the
`Debug` text of `StartPlan::new`'s plan must hold neither `time:` nor `correctors`, and it holds `end_time: 80.0` (the plan's
`CaseFacts`), which contains `time:`. The plan has no `time` field there (the text printed by the test ends `extra_set: [] }`),
so this is a defect of the check, not of the code; the other assertions of X2 did not run. By the house rule the registered
test is kept as the ignored `x2_entries_in_order_and_the_plan_text_known_miss` (its limit intact). Corrected check **X2b**
(`x2b_entries_in_order_and_the_plan_text`): X2's procedure and every limit, the `Debug` text read for the fields (`, time: `
and `, correctors: `, the derived form's separator before a field name). What I knew when writing this: the output of that run
(the values of X1, X3 and X4 are in the addendum); X2b had not run.
