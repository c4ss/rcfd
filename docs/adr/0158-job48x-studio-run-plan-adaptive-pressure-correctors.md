# ADR 0158: Job 48X, the Studio's Run makes the pressure correctors adaptive (`solver.corrector_control=adaptive` as a run-time setting), with a toolbar line, a switch in Advanced and coherent pre-run messages

Status: **registered** 2026-10-29, after the code and the tests below were written and compiled and before any registered
run (what had run before is listed under "What I already knew"); amendment 1 (X2's check, a set-up defect) before its
corrected run; **results in the addendum**: every registered limit met but the registered X2 (set-up defect, corrected by
X2b, met), every control failed as required; amendment 2 (X6, the dialog line) after those runs and before its own, met;
not yet accepted by the maintainer. House rules of
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

## Amendment 2 (2026-10-29, after the runs of the addendum below, before X6's run)

D6's dialog line has an exact text but no registered check covered it. Added **X6** (`x6_dialog_corrector_line`, model):
for the shipped cylinder, `dialog_corrector_line` with no override gives exactly `Pressure correctors: this dialog's run
takes solver.corrector_control = "fixed", 2 per step (the case's, unless an override sets them); Run on the toolbar makes them
adaptive (Advanced).`; with `solver.corrector_control=adaptive` exactly `Pressure correctors: this dialog's run takes
solver.corrector_control = "adaptive" (at least 2, at most 10).`; with `=adaptive` then `=fixed` the fixed line (last wins);
with `solver.n_correctors=3` the fixed line with 3; none for the quick start's steady cavity and for the fractional-step
cylinder. Its control is in-test (the two cases without a line). What I knew when writing this: the results below and that
`store_plan::parse_entry` reads a bare `adaptive` as a JSON string (read in the code); X6 had not run.

## Addendum (2026-10-29): results against the registered limits

Code at 405e219 (registration; X1, X3, X4 ran on it) and b52505d (amendment 1: the test file only; X2b, X5, W and the lints
ran on it). Release builds in this worktree's `target/`; this host (Windows 11, 16 threads, 93.6 GB, 4 to 5 GB free during
the runs: another agent's WSL workload held 55 GB), so wall times are rough.

### The answer

Run now adds `solver.corrector_control=adaptive` to the run of every transient iterative piso/pimple case that does not
choose the key itself, records it in `run.json` `settings.set` and nowhere else, and says so in a toolbar line, Run's hover
and the Advanced popover, where it can be switched off. On the maintainer's 256 x 256 cylinder at the step that showed the
spokes (1/64) the Studio run is clean (M_hf 2.7e-7 / 9.1e-8 at t = 1 / 3, 5 correctors, no SV301), and the same run with the
switch off reproduces 48T's spokes (M_hf 42.6 / 48.3) and SV301 at 2.42. At ADR 0153's automatic step the opt-in chose 2
correctors (the estimate 0.406 at the first step's dt 0.00116 s) and the run equals the switched-off one: clean either way
over 3 s, no extra cost; but see "a finding" below.

### Results

| Check | Limit (registered) | Measured | Outcome |
|---|---|---|---|
| X1 | scope as registered; the 4 transient example files get the entry | 16 set-ups as registered: the 10 in scope `Adaptive` (n 2; n 3 with `n_correctors = 3`) and built by the solver with the key; `"adaptive"` `CaseAdaptive`; fractional_step, error_tolerance, the piston, the low-Mach box, the steady cavity and `sod` `Fixed { Scope }` with the D2 reasons, each refused by the solver at `solver.corrector_control`; files: 34 (12 in each example tree, 10 in `cases/`), 4 with the entry (the cylinders Re 100 and Re 200 of both trees, each built with it), 30 without (steady or compressible) | met |
| X2 | as registered | stopped at its first assertion: `time:` matched `end_time:` (set-up defect, amendment 1) | known miss (ignored) |
| X2b | X2's limits, the fields read | the five entries in order, the user's `solver.corrector_control=fixed` after them, four entries with the switch off, `correctors: Adaptive { n: 2 }` in a Run plan's text, neither `, time: ` nor `, correctors: ` in `StartPlan::new`'s | met |
| X3 | (i)-(v) as registered | every text drawn exactly; the line at [8, 67] to [670, 82] in the 1280 x 800 window; the switch off in the run-time settings after the drawn click; the file unchanged, the editor clean | met |
| X3 control | no corrector text in the route before | none drawn | fails as required |
| X4 | on: the five entries, finished at 2, 2 correctors per event, summary adaptive / max 2 / cap 0, file and snapshot; off: the four entries, no corrector keys; `monitors.csv` byte-identical | on: 138 steps, 2.0 s wall, every event 2, one estimate (step 1, rho 0.0401), `triggered` 0; off: 138 steps, no `correctors` in any event, no summary; both files and snapshots unchanged; `monitors.csv` byte-identical | met |
| X5 (a) | the five entries, finished at 3, adaptive summary, clean, no SV301, file and snapshot | `set` `solver.end_time=3`, `solver.dt=0.00116`, `solver.max_courant=0.5`, `solver.max_dt=0.03`, `solver.corrector_control=adaptive`; 891 steps, 297 s; one estimate (step 1: 2 correctors, rho 0.406), 2 at every step; M_hf 1.05e-7 (t = 1.0005) and 1.17e-7 (t = 3); no SV301; file and snapshot unchanged | met |
| X5 (a) control | the four entries, no corrector keys; reported | 891 steps, 310 s; no corrector keys; M_hf 1.05e-7 / 1.17e-7 (the same values as X5 (a): the count stayed 2); no SV301 | as registered (predicted, not discriminating) |
| X5 (b) | `solver.end_time=3` and the entry, finished at 3, count above 2, clean, no SV301, file and snapshot | 192 steps, 137 s; one estimate (step 1: 5 correctors, rho 0.361), 5 at every step; M_hf 2.68e-7 / 9.06e-8 (ADR 0156 TV256: the same); no SV301; file and snapshot unchanged | met |
| X5 (b) control | must fail (b): `set` exactly `solver.end_time=3`, SV301 in `check.json` and the events, spoky | 192 steps, 93 s; `check.json`'s `rules` hold SV301 (`amplified by 2.42 per step ... cell 129 at (-0.50093, -0.01845, 0.05), typically a thin wall cell`, fix `adaptive`) and `events.jsonl` its warning event; M_hf 42.63 / 48.33 (ADR 0154 R256 / ADR 0156 TR256: the same) | fails as required |
| X6 (amendment 2) | the dialog line exactly, last wins; none for the steady and fractional cases | every text as registered; none for the two (ad082e5); clippy (`-D warnings`) and fmt clean again on ad082e5 | met |
| W | 0 failures; ignored = main's plus this job's; lints; `workspace_graph` | the whole `rcfd-studio` suite on b52505d (`--test-threads 1`): 62 test binaries, **279 passed, 0 failed, 18 ignored**: main's 13 and this job's 5 (the four X5 and amendment 1's X2 known miss); the changed ADR 0153 tests ((a) control, (d), (g1), (g2), (h)) and the unchanged (b), (c), f3, (f2) and ADR 0152's DV6 among the passed; clippy and fmt clean (below); `workspace_graph` 3 passed | met |
| N | only the Studio and this ADR | `git diff --stat 4e795b1`: `crates/rcfd-studio/src/{quick_run,quick_run_ui,runs,time_step,ui}.rs`, `tests/{correctors_x,run_time_s}.rs`, this ADR | met |

`cargo clippy -j 2 -p rcfd-studio --release --all-targets -- -D warnings`: clean (b52505d). `cargo fmt -p rcfd-studio --
--check`: clean.

### A finding: the count is chosen at the automatic step's first dt

At ADR 0153's automatic step the first estimate is made at dt0 = 0.00116 s, where two correctors give 0.406 on the 256 x 256
cylinder; the controller then grows the step about threefold within 70 steps (to 0.0033 to 0.0039 s, ADR 0153 (a)), where
DG1 measured 1.166 for two correctors at 1/256 = 0.0039 s (ADR 0154): at or above 1 (lag-neutral or slowly growing), far
above ADR 0156's target 0.5; ADR 0153's P4 saw slow growth at a fixed 0.0042 s on this mesh. The adaptive control
re-estimates only every 1000 steps or when its growth monitor fires (three steps in a row growing by 1.5, which an
amplification near 1 does not do); a 3 s run has 891 steps, so it kept 2 throughout. So under the Studio's default automatic
step the opt-in did nothing on 256 x 256 for the first 1000 steps, and the coupling ran at a step whose two-corrector
estimate is about 1.0 to 1.17. The 3 s run was clean (M_hf 1.2e-7 at 3 s), but why it stayed clean was not measured, and
longer runs at the grown step (ADR 0153's (a) ran 40 s with two fixed correctors and shed normally; its M_hf was not
measured) are not covered by this job. ADR 0156 D8 item 5 named the gap; closing it is a solver change (`crates/rcfd-fv`),
outside this job: question 2. Until then, resolution robustness under the Studio's defaults rests on the solver's 1000-step
re-estimate and growth monitor, not on the first estimate's margin.

### Deviations

1. Amendment 1: X2's first assertion matched `end_time:` (a set-up defect of the check); the registered X2 is the ignored
   `x2_entries_in_order_and_the_plan_text_known_miss`, X2b is the corrected check (met).
2. ADR 0156 D8 item 4 put the corrector into ADR 0153's "overrides the case's ..." indicator; here it has a line of its own
   and a line in Run's hover (D4), the indicator unchanged: by ADR 0152's rule a `"fixed"` key is not the case's own, and
   ADR 0153's f3 checks the indicator's text exactly.
3. ADR 0156 D8 item 6 (iv) expected ADR 0152's DV6 record to change for transient entries; it does not: DV6 records the
   Start run dialog's plans (`StartPlan::new`), which get no corrector entry (D6), and the plan prints `correctors` only when
   set.
4. The brief's example wording "1.3x cost" is not used: the measured figures are 1.0 (count 2) to about 1.6 per step (5
   correctors on 256 x 256 against 2), ADR 0156 A7.
5. The Studio shows no SV301 text of its own (D5): with the switch on the solver makes no SV301 estimate at all; with it off,
   SV301 reaches the run's `check.json` and events (X5 (b) control), which the Log panel lists, and the corrector line's hover
   says so.
6. After N was measured, a dated note (docs only) was added to ADR 0156 D8 pointing here; so the branch's final diff against
   4e795b1 also names `docs/adr/0156-...`, which N's registered wording does not list. No code outside `crates/rcfd-studio`.

### Not run

* ADR 0153's heavy tests (`--ignored`: (a) 40 s, D1-D4), now with the corrector entry; ADR 0153's (a) at 40 s would make
  about 10 estimates (every 1000 steps) and may raise the count above 2 later in the run (not measured).
* The gate's other crates (nothing outside `crates/rcfd-studio` changed);
  `baseline_hashes` (untouched, and its `kepsilon_channel` fails on this machine at earlier commits too).
* A person at the real window; Linux.
* The developed shedding regime (t > 10) at 256 x 256 through the Studio (ADR 0156 C3 ran it through `rcfd run` with the
  fixed step 1/64).

### Questions for the maintainer (with recommendations)

1. **`residuals.jsonl` without the corrector count** (ADR 0156 question 6): `crates/rcfd-runner` `drain` copies only `u`, `p`,
   `continuity` and `monitors`, so the residual plot cannot show the count; the count is in `solver-status.jsonl` and the
   `finish` summary (forwarded into `events.jsonl`). Add `"coupling": {"correctors": n, "cap": c}` there, and a Plots series in
   the Studio? Recommendation: yes, a small runner job with its own check (no change for fixed runs), then a Studio line
   `correctors <n>` in the strip.
2. **A re-estimate when the step has grown** (the finding above; ADR 0156 D8 item 5): with ADR 0153's automatic step the
   count is chosen at the first step's dt (0.00116 s) and kept while the step grows threefold, so on 256 x 256 the opt-in
   does nothing for the first 1000 steps and the coupling runs where two correctors' estimate is about 1.0 to 1.17 (at or
   above lag-neutral) instead of the target 0.5. This is the most important open item for "stable whatever the resolution"
   under the Studio's defaults. Options: (a) keep (the 3 s run was
   clean); (b) a solver change in `PimpleSolver`: re-estimate when dt has changed by a factor 1.5 since the last estimate;
   (c) a Studio change: start the automatic step at the controller's later step size. Recommendation: (b), registered with
   ADR 0156's harness (V256 under the controller to 10 s, cost reported), since it is what makes "stable whatever the
   resolution" hold under the automatic step.
3. **The Start run dialog**: it keeps the case's own correctors (D6, with a line saying Run on the toolbar makes them
   adaptive). Should the dialog's plan add the entry too (its `set` and ADR 0140/0152's dialog checks would change)?
   Recommendation: yes together with ADR 0153 question 3 (the dialog taking the toolbar's time plan), in one registered job,
   so that the dialog and Run never differ in more than the user's overrides.
4. **SV301 in the Studio before the run**: the Studio could read `check.json` (the runner writes it before the launch) and
   show SV301 with a button that turns the switch on, for runs with the switch off or started from the dialog.
   Recommendation: yes, small (no estimate in the Studio process; the runner's check already made it).
5. **The cost line**: the hover quotes ADR 0156's measured counts and cost. Calibrate it from the project's own finished runs
   (`finish.correctors.mean`) when they exist? Recommendation: later, with ADR 0153 question 5's wall-time calibration.
