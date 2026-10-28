# ADR 0153: Job 48S, the time step is set when a run starts: "Simulate for N s" on the Run toolbar, an automatic step from the mesh and the case's speeds under the solver's Courant controller, a Courant estimate in the captions and a warning above Courant 2

Status: **registered 2026-10-28**, before any registered run (the code and the tests below were written and compiled
first; what had run before is listed under "What I already knew"); the registration was corrected in review before any
run (724d735 to the next commit: check (f2) added for "Refine x2" and the Start run dialog, the (a) control shortened to
1.25 s so that its results are surely written, the gravity scale and the ramped-lid case of D5 and (g), D3's wording, the
order of runs, CRLF-proof test helpers). Job 48S, on local main a64864e (waves 48A-48P and 48R
merged; this branch was started at 6867e40 and merged main before this registration). House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed here before the registered runs; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before the
corrected run; defaults of existing code stay bit-identical outside the Studio's run path; `rcfd run` of a case file and the
command line behave exactly as before; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new crate, no unsafe
code, the crate graph stays acyclic. Clean-room: our own Studio code and the egui public API.

## The maintainer's request (2026-10-28, relayed)

"We should set the dt automatically from the field resolution plus the seconds we want to simulate. It should stop and save
automatically once we've simulated that many seconds, and it should be set when we run the simulation, not on the case
settings." Evidence: the maintainer refined `example-projects/wind-tunnel/cases/cylinder-re100.toml` from 64 x 48 to 256 x 256
cells and kept `solver.dt = 0.0625`; the Courant number rose to about 10 and the run was garbage (noisy pressure streaks,
flat residuals, noisy `cl` and `cd`) without a warning. [ADR 0148](0148-job48n-selected-case-middle-drag-units-scale-refine.md)
question 4 had recommended no Courant estimate for "Refine x2" (decided so in ADR 0149); this ADR supersedes it.

Asked, Studio-side and at run time only: (1) a "Simulate for [N] [unit]" control next to Run (the case's own end time by
default) and an Advanced popover with the Courant target (and the step limits if needed); steady cases: an iteration limit
or hidden, decided here; (2) the step computed at Run from the mesh the run builds and a speed scale of the case, preferably
through the solver's own adaptive Courant controller if it is robust for PISO on the example cases, else a fixed step that
lands on N in whole steps (ADR 0145); the trade-off measured and registered, and what happens without a speed scale; (3)
run-time settings passed as run overrides and recorded with the run, never written into the case or the editor, the toolbar
showing the effective values with a hover and an indicator when the case's own values are overridden; (4) the run ends at
exactly N with a stored final state there, state finished; (5) a Courant estimate in the Mesh caption and the Run
caption/hover, a warning before a run when explicit settings give a Courant number above a registered limit (for example
2), and "Refine x2" consistent with it. Registered checks (a)-(h) with controls; compressible and explicit solvers' scope
stated.

## Context (read at 6867e40, the merge a64864e checked for the same facts)

* **Run's plan.** `quick_run::run_selected_case` (Run, F5) makes `StartPlan::new_route` (ADR 0149, 0152) on a control
  thread and submits it; `StartPlan::set()` gives the `rcfd run --set` entries (the store proposal of ADR 0140, then the
  user's overrides of the Start run dialog). The runner records them in `run.json` `settings.set`; `case.snapshot` is the
  case file's bytes (ADR 0130's contract; ADR 0148 N1 and ADR 0149 O10 assert it). The store proposal for a transient case
  is `output.store_time_interval = solver.end_time / 100` (ADR 0140 D2).
* **The solver's step paths** (`crates/rcfd-api/src/sim.rs`, `timectl.rs`, `crates/rcfd-fv/src/stepctl.rs`, `ddt.rs`):
  1. a fixed `solver.dt`, landing on `solver.end_time` by ADR 0145's rule (`fixed_step`: a remainder below 1e-6 of a step
     is absorbed, a larger one is a separate shorter step);
  2. `solver.max_courant` alone: the earlier controller (`CflController`): its largest step is `solver.dt` (so it never
     steps above the first step) and it lands through `fixed_step` with whatever remainder is left;
  3. any key of `timectl::requested` (`solver.max_dt`, `dt_growth`, `min_dt`, ...): the controller of ADR 0050:
     `solver.dt` is the first step and, unless `solver.max_dt` is given, the largest; after each step the proposal is
     `dt * 0.9 * max_courant / Co` (safety 0.9), at most 1.2 times and at least 0.5 times the step before; a step whose
     remainder to the end time is within 1.05 of it lands there and the time is snapped onto the end time (`land`,
     `LAND_STRETCH`); so a last step is between about 5 % and 105 % of the proposal.
  The controller is built for `Engine::Transient` only (incompressible `piso` and `pimple`); a variable-density run and a
  VOF run refuse it ("fixed time steps only"); the compressible solver is explicit with its own `compressible.cfl`.
* **The field store** writes the final state when the run ends unless the last step was stored (`fieldstore::Hook::finish`),
  so a run that ends at N stores a snapshot at N whatever the store interval.
* **The Courant number** of the solver is `0.5 sum|F| dt / V` over the faces of a cell that are not `empty` (`ddt::
  courant_numbers`), recorded per step as the monitor `courant` (and `dt`) in `output/monitors.csv`.
* **ADR 0069's stability test** (`crates/rcfd-fv/tests/w33b_stability.rs`): `dt = Co_t h_min / U`; required stable for PISO
  `Co_t` in {0.25, 0.5, 1} (24 of 24 stable), for PIMPLE in {0.5, 1, 2, 4} (PIMPLE-2 unstable at 4 at Re 100); its scan found
  PISO stable to `Co_t` 8 on the impulsive-start cavity. ADR 0069 also quotes a cylinder at 181 x 141, dt 0.05, that blew up
  at Courant 10 to 30 while dt 0.025 was stable.
* **The examples**: of the 10 example cases only the two cylinders are transient (PISO, dt 0.0625, end 80); the cavities,
  heated cavities, steps and aerofoils are steady SIMPLEC. The checks of this ADR therefore use registered PISO variants of the
  cavity, the step and the heated cavity (the shipped file with `algorithm = "piso"` and an `end_time`; `tests/ts/mod.rs`).
* **Another job**: job 48T (ADR 0154, on its own branch) investigates the pressure spokes at the wall of the refined O-grid
  that the maintainer saw also at dt 0.0156; this job changes no solver code and does not investigate them.

## What I already knew (rule d): probes before any registered run

Release build of this branch at 6867e40 (`rcfd`, `rcfd-runner`, `rcfd-studio`), single-threaded `rcfd run` unless said, on
a host shared with other agents' builds and runs. None of it is a registered check; the throwaway probe test of P11 was
deleted and never committed.

| Probe | What | Result |
|---|---|---|
| P1 | The committed 64 x 48 run of the example (`monitors.csv`) | Courant 1.78 at step 1, largest 2.06 (step 2), median 1.63, 1.93 at most after step 10; `res_p` about 1.9e-2 throughout |
| P2 | 20 steps at dt 0.0625, 64 x 48 and 256 x 256 | 0.33 s and 9.98 s; Courant of the first five steps 1.78, 2.06, 1.98, 1.96, 1.94 and 6.32, 9.39, 7.69, 8.42, 8.02; the 256 x 256 run reached Courant 500 within the 20 steps |
| P3 | 256 x 256, dt 0.007: threads 1, 4, 8 | 10 steps 4.35, 3.81, 3.91 s; 40 steps 12.81 (1 thread) and 11.84 s (4): about 0.3 s per step, 4.5 microseconds per cell and step, threads buy about 1.1x |
| P4 | 256 x 256 at fixed dt (early flow) | dt 0.007 (Courant 0.93-1.1): `res_p` grows from step 7, Courant 33 at step 39; dt 0.005 (0.67-0.79): grows from about step 20, Courant 21 within 100 steps; dt 0.0042 (0.56-0.66): `res_p` 3.5e-3 to 0.85 over 300 steps (slow growth); dt 0.0035 (0.45-0.55): `res_p` 2.7e-3 to 3.4e-4 over 600 steps (decaying) |
| P5 | 256 x 256 at dt 0.005, varied | `pressure_rel_tol` 1e-10 with `pressure_max_iter` 20000: the same blow-up (largest Courant 21.329 in both); `mesh.grading` 20: stable 100 steps (`res_p` decaying); `solver.n_correctors` 3: stable 100 steps |
| P6 | 128 x 128 | dt 0.0125 (Courant 0.86-0.93): 100 steps, `res_p` steady; dt 0.025 (1.6-1.8): `res_p` 1.1e-2 to 6.6e-2 over 100 steps (rising) |
| P7 | 64 x 48 at large dt | dt 0.125 (Courant 3.4-3.9) 80 steps and 0.25 (6.4-7.4) 40 steps, no blow-up |
| P8 | `rcfd run --check-only` (parse, build, mesh check) | 0.052 s (64 x 48), 1.567 s (256 x 256), 6.506 s (512 x 512) |
| P9 | The adaptive controller on 256 x 256 (`solver.dt` 0.00117, `max_courant` 0.5, `max_dt` 0.015) to 1.5 s | Courant 0.148 at step 1, 0.445 at step 6, then 0.447-0.450; dt 0.0029-0.0033; `res_p` decaying; 460 steps; final time 1.5 exactly; the last step 4.18e-4 s, 12.5 % of the step before, its `cd` and `cl` in line with their trend |
| P10 | P9 to 8 s | 2245 steps in 1085 s (shared host); Courant at most 0.450 after step 6; dt 0.0033 growing to 0.00375 as `cd` fell from 1.57 to 1.30; `res_p` 9.6e-4, 2.1e-4 (t 4), 3.7e-4 (t 8); `cl` -0.36 (t 0.5), +0.29 (t 3), -0.27 (t 6.3): it swings from the start (the example's kick) |
| P11 | `time_step::facts` and the default plan on the examples (this job's code) | see the table below; the facts of the 256 x 256 cylinder took 0.95 s (its simulation built), the steady cases 0.002 s |
| P12 | The committed 64 x 48 run's `cl` | half range about 0.2 from t = 5-10, 0.31 from t = 30-40 |

P11 (default run-time settings: Courant target 0.5, simulated time the case's own):

| Case | Cells | Thinnest cell (m) | Speed at the start (m/s), where | First step dt0 (s) | Estimate at the case's dt |
|---|---|---|---|---|---|
| cylinder 64 x 48 (shipped) | 3,072 | 0.013460 | 1.1165, the initial field (U plus the kick) | 0.00602 | 5.18 at 0.0625 |
| cylinder 128 x 96 | 12,288 | 0.006878 | 1.1177 | 0.00307 | 10.16 |
| cylinder 128 x 128 | 16,384 | 0.005186 | 1.1177 | 0.00231 | 13.47 |
| cylinder 256 x 256 | 65,536 | 0.002614 | 1.1179 | 0.00116 | 26.73 |
| cavity as PISO (32 x 32) | 1,024 | 0.03125 | 1.0, boundary `lid` | 0.0156 | 0.32 at 0.01 (default) |
| step Re 400 as PISO | 8,000 | 0.025 | 1.4963, boundary `xmin` (the inlet expression) | 0.00835 | 0.60 at 0.01 |
| heated cavity Ra 1e4 as PISO | 1,600 | 0.025 | 0 (none) | 0.01 (the case's) | none |

Read together: the estimate of D4 is on the high side for the cylinder by a factor of 2.5 (P1: 5.18 against 2.06 measured)
to 3.4 (P9's first step: 0.148 at an estimate of 0.5), because the thinnest cells lie at the wall where the flow runs along
them. On the 256 x 256 O-grid (grading 100) the PISO solver with its default two correctors is unstable well below Courant 1
(P4: growing at 0.56-0.66 and above, decaying at 0.45-0.55), which neither the pressure tolerance changes nor a Courant
target alone can be shown to cure (P5: a milder grading or a third corrector removes it at dt 0.005); 128 x 128 shows the
start of it at 1.6-1.8 (P6) and 64 x 48 none up to 7.4 (P7). Found, not fixed (a solver matter, question 2).

## Options for the step (registered before the decision runs)

| | A: adaptive (the solver's controller of ADR 0050) | F: fixed (whole equal steps) |
|---|---|---|
| Entries | `solver.end_time=N`, `solver.dt=dt0`, `solver.max_courant=C`, `solver.max_dt=N/100` | `solver.end_time=N`, `solver.dt=N/k`, k = ceil(N / (C h_min / u0)) |
| Step | conservative first step, then the largest step that keeps the measured Courant number at about 0.9 C | the estimate's step throughout: 2.5-3.4 times smaller than needed on the cylinder (P11 against P1, P9) |
| Flow faster than the estimate (jets, accelerating flow) | followed after one step | not followed |
| No speed scale | starts from the case's dt and follows the flow (D5) | cannot be computed: refused |
| End | lands on N, time snapped exactly; the last step 5-105 % of the proposal | N / k steps, ADR 0145's landing |
| Steps known before the run | an upper estimate (N / dt0) | exactly |
| Snapshot times | the first step after each store interval | on the interval when it is a multiple of N / k |
| Cost of the 256 x 256 cylinder to 40 s | about 11,000 steps (P10's dt) | about 34,000 steps |
| Scope | `piso`, `pimple` (constant density, no VOF) | the same here (VOF and variable density are out of scope, D1) |

**Decision rule** (the maintainer's "prefer the solver's controller if it is robust for PISO on the example cases"): A is
the Studio's step when D1-D4 below all meet their limits (A is robust and agrees with F on each example); otherwise F when
F meets R1 and its first-step Courant number is at most C on every case; otherwise neither is the default (an amendment and
the maintainer's question). The code carries both (`time_step::StepMode`); `time_step::DEFAULT_MODE` is A at registration.

## Decisions (as implemented; D2 subject to the decision rule)

1. **Scope** (`time_step::Kind`, `kind_of`). "Simulate for" and the automatic step apply to incompressible transient
   runs: `solver.algorithm` `piso` or `pimple`, constant density (`fluid.density` not `ideal_gas`/`real_gas`), no VOF
   (`multiphase.enabled`), the flow solved (not `solver.frozen_flow`), not a chain. A steady case gets the line `steady
   (<algorithm>): until converged, at most <N> iterations` (no control: the run-time iteration limit is question 4); a
   transient case outside the scope gets `<algorithm>: the case's own time step, to its end time <T> s` with the reason on
   hover (registered reasons: `the compressible solver is explicit and sets its own step from compressible.cfl`; `a
   variable-density run (fluid.density) takes fixed steps only, without the Courant controller`; `a VOF run (multiphase)
   takes fixed steps only, without the Courant controller`; `the flow is prescribed (solver.frozen_flow), so no Courant
   number follows it`; `a chain runs each stage with its own settings`). Run of those cases is unchanged.
2. **The step** (`time_step::plan`). A (adaptive, D-rule): `dt0 = C h_min / u0` rounded down to 3 significant digits
   (`round_down`; the first step's estimated Courant number is at most C), at most the largest step; `solver.max_dt = N /
   100` (`MIN_STEPS`; at least about 100 steps, and the key that selects ADR 0050's controller, never the earlier one);
   `solver.max_courant = C`; the other controller keys at their defaults (safety 0.9, growth 1.2, shrink 0.5, `min_dt` 1e-3
   dt0). F: `dt = N / k`, `k = ceil(N / (C h_min / u0) - 1e-9)`. Entries are written with Rust's shortest round-trip text.
3. **The Courant target** `C = 0.5` (`COURANT_TARGET`), editable in Advanced. Justification: inside the set ADR 0069's
   stability test requires PISO to be stable in (`Co_t` 0.25, 0.5, 1) and below PIMPLE-2's failure at 4; the controller aims
   at 0.9 C = 0.45. On the 256 x 256 cylinder the early-flow runs grew unstable at fixed steps of 0.0042 s and more (Courant
   0.56 and up) and decayed at 0.0035 s (0.45-0.55, P4); the controller's step drifted from 0.0033 to 0.00375 s as the flow
   developed (P10), and P5 shows the onset is not a Courant threshold alone, so the margin in the developed flow is not
   known: check (a) tests it over 40 s. Not a claim that 0.5 is stable on every mesh (P4-P6, question 2).
4. **The estimate** (`Facts`, `facts`, `Facts::courant`). The speed scale u0 is the largest speed of the case's initial
   state as the solver builds it (`Simulation::new` of the case: the cell velocities and `boundary_field("speed")`, so fixed
   values, free stream, moving walls, boundary expressions and user functions at t = 0, and the initial expressions); for a
   flow at rest under gravity (`physics.gravity` not zero) it is free fall across the domain, `sqrt(|g| L)` with `L` the
   largest extent of the mesh's bounding box (`GRAVITY_SCALE`; an upper bound of a buoyant flow's speed, so the first steps
   are short and the controller lengthens them). The thinnest cell `h_min` is the smallest cell volume over its largest face
   area, the faces of `empty` patches left out (as the solver's Courant number leaves them out); the estimate is `Co = u0 dt
   / h_min`: on the high side when the thin cells lie along walls (P11: 2.5 to 3.4 on the cylinder), close on a uniform
   grid. Time-dependent boundary values are taken at t = 0 only (a lid ramped from rest gives no speed: D5).
5. **No speed scale** (u0 = 0: nothing moves at the start and no gravity): A starts at the case's own `solver.dt` (at most
   the largest step) under the controller; the hover says `(the case's solver.dt: no inflow, moving wall, initial velocity or
   gravity gives a speed at the start)`; F refuses with `no speed scale for a fixed automatic step: the case has no inflow,
   moving wall, initial velocity or gravity at its start`. (P11 ran before the gravity rule was written: the heated cavity
   then had no scale.)
6. **The warning** (`time_step::warning`, limit `COURANT_WARN = 2` on the estimate). Explicit settings: the case's own
   `solver.dt` (Advanced: the automatic step off) whose estimate is above 2, or a Courant target above 2. Texts: `<case>:
   this run takes the case's own solver.dt = <dt> s, an estimated Courant number of <Co> (<u0> m/s through the thinnest cell,
   <h> m): above 2, the run may become unstable or noisy. Fix: use the automatic time step (Advanced), or a solver.dt of at
   most <dt2> s.` (dt2 = 2 h_min / u0 rounded down) and `<case>: the Courant target <C> is above 2: the run may become
   unstable or noisy. Fix: a target of at most 2 (Advanced; the default is 0.5).` Drawn under the toolbar's row with a
   button (`Use the automatic time step` or `Use the default Courant target`) that applies the fix; Run is not blocked. A case
   with its own step control (D11) gets no estimate and no warning.
7. **The captions** (supersedes ADR 0148 question 4). The Mesh side gets a line under its resolution line once the facts of
   the shown text are known (`time_step::mesh_line`, hover `MESH_HOVER`): `Courant estimate <Co> at the case's solver.dt
   <dt> s (<u0> m/s through the thinnest cell, <h> m); Run sets the time step: <dt0> s at the start for Courant <C>, then
   adaptive, so the case's solver.dt is overridden at run time.` (variants: `solver.dt (the schema default)`, no speed
   scale, the case's own step). The Run caption is the toolbar's row (D8). "Refine x2" does not touch the time step; its hover
   gains `The time step is not edited: Run sets it from the finer cells when it starts (about half of the <dt0> s it starts
   with now); the case's solver.dt is overridden at run time.` (`refine_note`), and after the refined text is built the Mesh
   line shows the finer mesh's estimate.
8. **The toolbar** (`quick_run_ui::time_row`, `quick_run::time_view`). A second row under Run's when a case is selected: for
   a transient case `Simulate for` [N] `s`, `Advanced` (a popover: the checkbox `automatic time step (Courant controller)`,
   `Courant target`, `Largest step (s)`), the step line (`label`: `dt auto: <dt0> s at the start, then adaptive (Courant
   <C>)`, or `dt <dt> s, the case's own`, or `the case's own step control (<keys>)`), and, when this run overrides the case
   text's own values, the indicator `overrides the case's solver.dt <dt> s [and solver.end_time <T> s] for this run` (warning
   colour; `solver.end_time` only when N differs). Run's hover is its text (`RUN_HOVER`, unchanged) and a second line, the
   plan's `hover`: `This run: <N> s of simulated time; dt = <dt0> s at the start, then the solver's Courant controller keeps
   the largest cell Courant number at about <0.9C> and at most <C> (steps of at most <max_dt> s) and lands on <N> s.
   Estimated Courant number of the first step ... About <k> steps or fewer (the step grows to the Courant target), estimated
   wall time about <w> or less (<cells> cells, at 4.7 microseconds per cell and step). This run overrides the case's ...: the
   case file and the case editor keep them; the run's settings record the values it used. The run stops at the end time and
   stores its final state there.` The facts come from the text the Mesh side shows, computed on a control thread
   (`quick_run::facts_of`, at most 16 kept); before they are known the row reads `time step: known once the mesh of the case
   is built; Run sets it when it starts`.
9. **Run-time only** (`StartPlan::time`, `with_time`, `set()`). Run computes the facts again from the file it runs (or takes
   the shown ones when the file hashes to them) and adds the plan's entries after the store keys and before any user
   override; nothing is written to the case file or the case editor; `run.json` `settings.set` records the entries and
   `case.snapshot` stays the file's bytes. The settings are per case, in memory, forgotten when the project closes
   (`QuickRun::run_time`). A case whose simulation does not build runs without time entries (the solver reports why, as
   before); a plan that cannot be made (a simulated time that is not positive, an end before the case's `output.write_times`)
   refuses the start with `<case> was not started: <reason>`.
10. **"Simulate for" default**: the case's resolved `solver.end_time`, what `rcfd run` of the file simulates (the schema
    default when the text sets none); unit `s` (`viewport::TIME_UNIT`; the settings are SI). A case without its own store gets
    ADR 0140's proposal for N (`output.store_time_interval = N / 100`); a case's own store is kept; the final state at N is
    stored by the field store's finish (verified by (c)).
11. **A case with its own step control** (any of `OWN_CONTROL`: `solver.max_courant`, the ADR 0050 keys, `output.write_times`,
    `output.write_time_interval`): Run keeps it and adds only `solver.end_time` (the Advanced checkbox is greyed out with
    the keys); an N before the case's last `output.write_times` entry is refused.
12. **The Start run dialog** ("Options...") keeps the case's own time settings (its plan has no time entries, so ADR 0140's
    registered dialog checks are unchanged) and shows, for a transient case whose facts are known, the line `Time step: this
    dialog's run takes solver.dt = <dt> s to solver.end_time = <T> s (the case's, unless an override sets them); Run on the
    toolbar sets the time step for you (Simulate for, Advanced).` and, above the limit, the warning `<case>: solver.dt = <dt> s
    gives an estimated Courant number of <Co> ...` (`dialog_lines`; question 3).
13. **Test switch** `QuickRun::switches.case_time`: the route before (no time row, no Mesh line, no time entries).

## Placement

`crates/rcfd-studio` only: new `src/time_step.rs`; hunks in `src/{lib,runs,quick_run,quick_run_ui,preview,preview_ui,ui}.rs`
(`runs.rs`: one boxed field, `set()`'s time entries, `with_time`, `text_tree` made crate-visible; `preview.rs`: `Built::text`).
`store_plan.rs` is not touched. No new crate, no new dependency, no new workspace edge; `Cargo.toml`, `Cargo.lock`,
`crates/rcfd-cli/tests/workspace_graph.rs` and `docs/architecture.md` unchanged. Nothing outside the Studio changes, so `rcfd
run`, the runner and the command line behave exactly as before.

## Registered checks (limits fixed here, before any of them ran)

Release builds, `--test-threads 1 --nocapture`, output to files, this host; `rcfd` and `rcfd-runner` of the same build next
to the test's `rcfd-studio` (`common::binaries`). Files: `crates/rcfd-studio/tests/time_step_s.rs` (D1-D4, S-E, S-G, S-H, S-K),
`run_time_s.rs` ((a)-(h)), helpers `tests/ts/mod.rs`. "Through Run": a served project (`RunnerProject`), the Studio model with
the watcher and the run worker on, the case selected and settled (its facts known), `run_selected_case` (F5's call), the run
waited for. Shedding: ADR 0142 amendment 1's rule (`ts::shedding`: upward crossings of `cl` through its mean over the window,
St = (k - 1) / (t_k - t_1), amplitude half the range). Every wait has an upper bound (60 s for the model, the run's own limit
below), not a timing claim.

| Id | Check | Limit | Negative control (must fail) |
|---|---|---|---|
| D1 | Decision: the shipped cylinder (64 x 48) to 80 s by `rcfd run` with the plan's entries, A and F | R1: both exit 0 and end at 80 bit for bit (`monitors.csv` time); A's largest step above its first (ADR 0050's controller ran: the earlier controller never steps above `solver.dt`); R2: A's first Courant number at most C and its largest after step 10 at most 1.1 C; R3: A's St over [40, 80] within 3 % of F's | (a decision run) |
| D2 | Decision: the cylinder at 256 x 256 to 3 s, A and F | R1, R2 as D1; R3: `cd` at 3 s within 2 % | (a decision run) |
| D3 | Decision: the quick start's cavity as PISO to 10 s, A and F | R1, R2; R3: `lid_drag` at 10 s within 1 % | (a decision run) |
| D4 | Decision: the wind tunnel's step Re 400 as PISO to 30 s, A and F | R1, R2; R3: `p_inlet` at 30 s within 2 % | (a decision run) |
| S-E | (e) the step follows the cell size (model): every count doubled ("Refine x2"): the cylinder 64 x 48 -> 128 x 96 to 80 s, the cavity as PISO 32 -> 64 to 10 s | dt0 (the first step, "the computed dt") of the refined case over the base case's in [0.47, 0.53]; the largest step unchanged bit for bit | the case's own step (`StepMode::Case`): a ratio outside [0.47, 0.53] (1) |
| S-G | (g) the speed scale's two fallbacks (model), 2 s: (g1) the heated cavity Ra 1e4 as PISO (at rest, gravity 7100 m/s^2, a 1 m box); (g2) the quick start's cavity as PISO with its lid ramped from rest (`velocity_expression_x = "min(1 [m/s], 1 [m/s] * t / 1 [s])"`) | (g1): `u0_from` exactly `GRAVITY_SCALE`, u0 = sqrt(7100) within 1e-9 relative, the plan's entries exactly `solver.end_time=2`, `solver.dt=0.000148`, `solver.max_courant=0.5`, `solver.max_dt=0.02`; (g2): u0 0, `u0_from` none, the entries exactly `solver.end_time=2`, `solver.dt=0.01`, `solver.max_courant=0.5`, `solver.max_dt=0.02`, no first-step estimate, the hover holds D5's sentence, F refused with D5's text exactly | (none: a new path's registered behaviour; F's refusal discriminates the two) |
| S-H | (h) JSON (model): the cylinder as JSON (`Case::to_json_string`) | the facts equal the TOML file's apart from the hash; the plans equal | (none) |
| S-K | Scope (model) | the quick start's cavity: `Steady { max_iterations: 2000 }`, the line `steady (simplec): until converged, at most 2,000 iterations`, no plan (`a steady run has no time step`); `cases/sod/sod.toml` and `cases/wedge/wedge.toml`: D1's compressible reason, no plan, a line starting `compressible: the case's own time step, to its end time`; the shipped cylinder: transient, its own dt and end time in its text, no step control, dt 0.0625, end 80, 3,072 cells, the speed from the initial field | (none) |
| (a) | The 256 x 256 cylinder through Run, the automatic step, "Simulate for" 40 s (heavy, ignored, run on purpose) | finished at 40 s bit for bit; every monitor finite; the first step's Courant number at most C; at most 1.1 C after step 10; over [10, 40] s at least 3 upward crossings, St in [0.14, 0.20], amplitude at least 0.1; within 3 hours | (a)'s control below |
| (a) control | The same case with the automatic step off (the case's own `solver.dt = 0.0625`), 1.25 s (20 steps, as P2, whose run ended with exit 0 and its results written) | must fail (a): its first Courant number above C (predicted about 6.3); predicted, asserted: the largest Courant number above 10 within the 1.25 s; the run's `set` exactly `solver.end_time=1.25` | |
| (b) | The shipped cylinder through Run with its defaults (80 s) | finished at 80 s bit for bit; the first Courant number at most C and at most 1.1 C after step 10; over [40, 80] s at least 3 upward crossings, St in [0.14, 0.20] (the band of the example; ADR 0142 registered none, its run gave 0.1517 at Courant about 2), amplitude at least 0.1 | the route before (`case_time`): its Courant number after step 10 above 1.1 C |
| (c) | "Simulate for" 7.3 s on the shipped cylinder (its own store every 1 s) through Run | finished, `status.json` time 7.3 bit for bit, reason exactly `finished: end time 7.3 reached`; the store holds 8 snapshots, the last at (the last step, 7.3) exactly, snapshot i (i = 1..7) at a time in [i, i + 0.1); the last step at least 5 % of the step before; `cd` and `cl` of the last step within [min - 0.1 range, max + 0.1 range] of the 50 steps before and changed by at most 5 times their largest step-to-step change there (ADR 0145 C1's form); the last snapshot's largest abs `p` at most twice the snapshot before's | the route before: it does not end at 7.3 |
| (d) | The run of (c) | the case file's bytes after the run equal those before; `case.snapshot` equals them; the case editor holds the case clean with `solver.dt` 0.0625 and `solver.end_time` 80; the plan's `set()` and `run.json` `settings.set` exactly `solver.end_time=7.3`, `solver.dt=0.00602`, `solver.max_courant=0.5`, `solver.max_dt=0.073`; the status bar's `queued run ...` ends with `(rcfd run --set <those entries>)`; the monitor `dt`: first 0.00602, largest above 1.2 times it (ADR 0050's controller ran; the earlier controller never steps above `solver.dt`) | the route before: no `solver.*` entry in `run.json` |
| (f) | The toolbar drawn (1280 x 800, tooltips at once): a served project with the shipped cylinder and the quick start's cavity; (i) the cylinder selected, a frame; (ii) the pointer on Run; (iii) "Simulate for" 7.3 (the model's call the field makes); (iv) a drawn click on Advanced, a drawn click on its checkbox, Escape (the popover closes); (v) a drawn click on the warning's fix; (vi) the cavity selected | (i) drawn: `Simulate for`, `80`, `Advanced`, `dt auto: 0.00602 s at the start, then adaptive (Courant 0.5)`, `overrides the case's solver.dt 0.0625 s for this run`, and the Mesh line exactly `Courant estimate 5.184 at the case's solver.dt 0.0625 s (1.116 m/s through the thinnest cell, 0.01346 m); Run sets the time step: 0.00602 s at the start for Courant 0.5, then adaptive, so the case's solver.dt is overridden at run time.`; (ii) `RUN_HOVER` and the plan's hover text drawn; (iii) `overrides the case's solver.dt 0.0625 s and solver.end_time 80 s for this run` and the 7.3 s plan's label drawn, the case file unchanged and the editor clean; (iv) the checkbox's text and `Courant target` drawn; then `dt 0.0625 s, the case's own` and the warning exactly `cases/cyl.toml: this run takes the case's own solver.dt = 0.0625 s, an estimated Courant number of 5.184 (1.116 m/s through the thinnest cell, 0.01346 m): above 2, the run may become unstable or noisy. Fix: use the automatic time step (Advanced), or a solver.dt of at most 0.0241 s.` with its fix button drawn; (v) the automatic step's label again, the warning not drawn; (vi) `steady (simplec): until converged, at most 2,000 iterations` drawn, `Simulate for` not | the route before: no `Simulate for`, no indicator, no plan hover, no Mesh line drawn |
| (f2) | Refine x2 and the Start run dialog, drawn (1280 x 800, tooltips at once): a served project with the shipped cylinder; (i) selected, the pointer on "Refine x2"; (ii) a drawn click on it, frames until the 128 x 96 text is shown and its facts known, Run's state; (iii) the edits discarded, the file's mesh shown again, the Start run dialog opened on the cylinder until its plan is drawn; (iv) its overrides `solver.dt=0.02` | (i) "Refine x2"'s hover draws `The time step is not edited: Run sets it from the finer cells when it starts (about half of the 0.00602 s it starts with now); the case's solver.dt is overridden at run time.`; (ii) the Mesh line of the 128 x 96 text (`mesh_line` of `ts::cylinder(128, 96)`'s facts) and `dt auto: 0.00307 s at the start, then adaptive (Courant 0.5)` drawn, Run refused exactly with `cases/cyl.toml has unsaved edits: save it first (Case > Save); a run uses the file on disk`; (iii) the dialog draws `Time step: this dialog's run takes solver.dt = 0.0625 s to solver.end_time = 80 s (the case's, unless an override sets them); Run on the toolbar sets the time step for you (Simulate for, Advanced).` and `cases/cyl.toml: solver.dt = 0.0625 s gives an estimated Courant number of 5.184 (1.116 m/s through the thinnest cell, 0.01346 m): above 2, the run may become unstable or noisy. Fix: Run on the toolbar (automatic time step), or an override solver.dt of at most 0.0241 s.`; (iv) the line with `solver.dt = 0.02 s`, no warning drawn | the route before: no note, no Mesh line, no dialog line drawn |
| (g1) | The heated cavity as PISO through Run, "Simulate for" 2 s | the toolbar's hover holds `GRAVITY_SCALE`; `run.json` `set` exactly S-G (g1)'s entries (the example keeps its own store); finished at 2 s bit for bit; every monitor finite; Courant at most 1.1 C after step 10 | (none: S-G's) |
| (g2) | The ramped-lid cavity as PISO through Run, "Simulate for" 2 s | the toolbar's hover holds `NO_SPEED` (`no inflow, moving wall, initial velocity or gravity`); `run.json` `set` exactly S-G (g2)'s entries (the example keeps its own store); finished at 2 s bit for bit; every monitor finite; Courant at most 1.1 C after step 10 | (none: S-G's) |
| (h) | `cases/c.json` (the cylinder as JSON) through Run, "Simulate for" 2.3 s | its `run.json` `set` equals the TOML twin's plan entries; finished at 2.3 s bit for bit; `case.snapshot` the JSON file's bytes | the route before: no `solver.*` entry |
| W | The whole `rcfd-studio` suite (fake-claude built first), `cargo clippy -p rcfd-studio --release --all-targets -- -D warnings`, `cargo fmt -p rcfd-studio -- --check`, `workspace_graph`, `scripts/third_party_licenses.py --check` | pass; the known misses ignored as before | (their own) |

Reported without a limit: the wall times and step counts of every run; the realized Courant band and the largest step of
the adaptive runs; the estimate against the measured first-step Courant number on each case; the last step's fraction of
the adaptive runs; (a)'s step count and dt range.

## Order of runs

All on one build of this registration's commit (`rcfd`, `rcfd-runner`, `fake-claude` and the test binaries built first).

1. (a) in a process of its own, started first (it is the longest, 60-90 minutes); in parallel D1-D4 (`cargo test --release
   -p rcfd-studio --test time_step_s -- --ignored d`, D2 in a process of its own). The decision by the rule above, in the
   addendum (or an amendment, if F; then (a) and every check below that runs the automatic step run again with F).
2. `time_step_s` (S-E, S-G, S-H, S-K), then `run_time_s` (all but (a)).
3. W.

## Amendment 1 (2026-10-28, after the first registered run of `run_time_s`, before the corrected run of (f))

The registered run of `run_time_s.rs` on bfc2600 (23:16-23:20) passed 12 of its 13 tests that are not ignored ((a)'s
control, (b) and its control, (c)/(d) and its control, (f)'s control, (f2) and its control, (g1), (g2), (h) and its
control); **(f) failed at its first assertion**, `(i) not drawn: Simulate for`. The texts of its frame (i), printed by the
test, held `80.0` and `Advanced` of the time row but not its labels. A throwaway probe (`tests/zz_diag_f.rs`, run once,
deleted, never committed) drew three frames of the same set-up: in the first frame of a fresh egui context the top panel
is laid out at its height of the frame before (one row: clip rectangle y 23-46), so the labels of the second row (y 46-64)
are not painted, while the field and the button are; from the second frame on the panel has its height (y 23-67) and every
text of the row is drawn (`Simulate for`, `80.0`, `Advanced`, the step line, the indicator, and the Mesh line). In the
window a second frame follows at once, so this is a defect of (f)'s procedure, which read the first frame of a new
context. The same frames show the field drawn as `80.0` where (f) registered `80`.

Corrected, before the corrected run (the same test names; every limit and control unchanged): (f)'s procedure draws one
frame before it reads (i); and the code formats the "Simulate for" and "Largest step" fields with the Studio's number
format (`viewport::fmt_num`, ADR 0148 decision 6: `80`, `7.3`, no trailing zeros) instead of egui's, so that the field
reads as every other time on the Run page; the registered `80` stays. The corrected (f) and its control run on a build of
this amendment's commit in a target folder of their own (`target-48s-a1`), because (a) and D2 still run from the test
binaries of the registration's build; (a) and D2 do not draw the field. What I knew when writing this: the output of the
first run (the passing tests' values are in the addendum) and the probe; the corrected (f) had not run.

## Amendment 2 (2026-10-28, after the corrected run of (f), before the run of f3)

The corrected run of (f) and its control on 3ae759b (amendment 1's build, `target-48s-a1`): the control passed; **(f)
missed (iv)**: every limit of (i)-(iii) was met (the row's texts with `80`, the Mesh line, the hover, the indicator after
7.3 s, the case file and the editor untouched), and in (iv) the popover's texts, `dt 0.0625 s, the case's own` and the
warning's exact text were drawn, but not the fix button `Use the automatic time step`: the warning is one line of about
2,000 points and the button sat right of it, outside the 1280-point window, so egui did not paint it. This is a defect of
the code (in the window the button cannot be clicked either), not of the procedure. By the house rule the test becomes the
ignored `f_the_toolbar_time_row_drawn_known_miss` with its limits intact, running the registration's layout through the
test switch `QuickRun::switches.warning_fix_last`.

The fix (the code of this amendment): the warning's row draws the fix button first and the warning after it, wrapped to the
window (`egui::Label::wrap`). New check **f3** (`f3_the_toolbar_time_row_drawn`), registered here before its run: (f)'s
procedure (as amendment 1 corrected it) and (f)'s limits, unchanged, on this layout; its control is (f)'s (the route
before) and the known-miss test itself (the registration's layout misses (iv)). f3 and (f)'s control run on a build of
this amendment's commit in `target-48s-a1`, as do W's suite and lints. What I knew when writing this: the output of that run
(its values are in the addendum); f3 had not run.

## Changes to earlier tests

None: Run of a steady case, the Start run dialog's plan and `StartPlan::new` are unchanged. Earlier tests that draw the
toolbar while a transient case is selected (ADR 0148 N9 selects the wind tunnel's cylinder runs, so the cylinder) now draw
the time row as well; their limits do not read it (checked by reading N9: its only absence limits are ` t = ` and `  t `,
which no new text holds).

## Not in CI (by hand)

A person at the real window: the row under Run on a transient and a steady case, the popover, the hover, the indicator, the
warning and its fix, the Mesh line before and after "Refine x2", a run that stops at the simulated time asked for, the Start
run dialog's note.

## View-and-report checklist for the maintainer

1. `powershell -ExecutionPolicy Bypass -File scripts\studio.ps1 example-projects\wind-tunnel` (or a copy).
2. Click `cases/cylinder-re100.toml`: under Run a row `Simulate for [80] s  Advanced  dt auto: 0.00602 s at the start, then
   adaptive (Courant 0.5)  overrides the case's solver.dt 0.0625 s for this run`. Hover Run: this run's dt, Courant number,
   steps and wall time. The Mesh side has a line `Courant estimate 5.184 ...`.
3. Type 7.3 into "Simulate for", press Run: the run stops at 7.3 s, finished, and the slider's last snapshot is at 7.3 s.
   The case file still says `dt = 0.0625` and `end_time = 80`; the case editor shows no unsaved edit.
4. "Refine x2" twice (256 x 192), save, then the Mesh line and the row: the step halves each time. Run: the run is stable.
5. Advanced: untick the automatic time step: an orange warning names the Courant estimate of `solver.dt = 0.0625` on the
   refined mesh and the fix; "Use the automatic time step" restores it.
6. Click a steady case: the row says it iterates until converged, with its iteration limit.
7. Report anything slow, confusing or wrong.

## Questions for the maintainer (open at registration; the addendum may add to them)

1. **The default Courant target 0.5**, with the controller's safety 0.9 (aim 0.45). Keep, or another value (1.0 halves the
   cost but P4 shows the 256 x 256 cylinder unstable from about 0.56)?
2. **PISO on the refined O-grid** (P4-P6, found here, not fixed): with two correctors the 256 x 256 cylinder (grading 100)
   grows unstable at Courant 0.56-1 while 64 x 48 runs at 7; a third corrector or a milder grading removed it at dt 0.005 in
   100 steps. Job 48T (ADR 0154) investigates the wall artefacts of that mesh. Should the PISO defaults change (a solver
   job), or should the Studio set `solver.n_correctors` for strongly graded meshes (a run-time setting, like this job's)?
3. **The Start run dialog** keeps the case's own time settings (with the note and the warning). Should it apply the
   toolbar's time plan too (its `set` would change for transient cases; ADR 0140's dialog checks SF4 (b) and SF9 (g) would be
   registered again)?
4. **Steady cases**: a run-time iteration limit ("Iterate at most [N]") instead of the read-only line?
5. **The wall-time estimate** uses a fixed 4.7 microseconds per cell and step (this host, single thread, laminar PISO) and,
   for the adaptive step, the first step's count (an upper estimate: 2.5 to 3.4 times the steps on the cylinder). Calibrate
   from the project's own finished runs instead?

## Files

This ADR; `crates/rcfd-studio/src/time_step.rs` (new); hunks in `crates/rcfd-studio/src/{lib,runs,quick_run,quick_run_ui,
preview,preview_ui,ui}.rs`; new tests `crates/rcfd-studio/tests/{time_step_s,run_time_s}.rs` and `tests/ts/mod.rs`; a dated
note in ADR 0148 (question 4 superseded) and ADR 0149 (its decision 4 on that question).
