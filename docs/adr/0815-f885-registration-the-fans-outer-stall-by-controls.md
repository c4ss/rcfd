# ADR 0815: F-885 registration: why the fan's outer loop does not contract within a step, by controls fixed in advance

Status: **REGISTERED 2026-11-26, written before any code of F-885 for this registration and before any run of its checks.**
Before this text no library code, test, harness or solver run of this work was written or made. What was done: reading
(below), `git apply --check` of the three committed F-885 mutation patches at the base, and the reasoning of this page.
Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f885-fan-outer-stall.md`](briefs/f885-fan-outer-stall.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. A further registration of F-885 under rule 13 (a
diagnosis of the miss of check H in [ADR 0811](0811-f885-registration-a-face-flux-independent-of-alpha-u-then-check-h.md)),
taken as option (a) of ADR 0811's question 2 by the lead's decision of 2026-11-26. Branch `f885-fan-outer-stall` from main
3b6cc7cc; scratch `w49i-work/f885-fan-outer-stall-scratch`. Written by the model Claude Opus 5.5.

Size (agent): done; 1; 1; 1; 210000

## What is asked

The acceptance of F-885 is unchanged and this work does not meet it (check H still has to decide). ADR 0811 removed the
jump between the relaxed and the unrelaxed fixed points; check H with its switch completed both runs and did not decide,
because in the window `d_2` to `d_5` stay at about `d_1` (median `d_5 / d_1` 0.95 at dt, 1.08 at dt/2). Recorded before
that run: on 20 steps of H(dt) with 20 outer iterations and the switch on (F2s) the relaxed changes do not contract,
`C = median d_19 / d_2` 0.0258 (STALLS), median `d_19` 6.36e-3 m/s, in every classed step at the tip-gap cell c11837
(r 0.2492 m, z 0.2306 m). The brief asks for the hypotheses of that stall from the code, each with its control, on 20
steps of H(dt) with the switch on and on the cavity as a control that contracts, the numbers fixed now, and no long run.

## What was read

Whole: `AGENTS.md`; `docs/process/agent-rules.md` (version 5); the brief; ADR 0811 (registration, addendum 1, answers);
ADR 0810 (registration, addendum 1, answers).

In part: `crates/rcfd-fv/src/pimple.rs` (base 3b6cc7cc) lines 1 to 320 (the settings and the records), 1040 to 1135
(`piso`, `pimple`), 1220 to 1330 (`OuterUpdate`, `OuterHook`, the solver state), 1693 to 2720 (the whole of `step_with`);
`crates/rcfd-api/src/sim.rs` lines 1940 to 1990 (the linear controls from the case), 2100 to 2175 (the transient settings:
`amg_pressure` above 2000 cells, the limiter), 2225 to 2260 (`outer_turb`: `begin_time_step` and the face viscosity at the
step's start), 4385 to 4420 (the hook: the sources' `evaluate` and one turbulence sweep per call), 2970 to 2995;
`crates/rcfd-api/src/schema.rs` lines 884 to 890 (`non_orthogonal_correctors` default 0) and 1230 to 1260 (the linear
tolerances, default 0 = the algorithm's); `crates/rcfd-fv/src/pressure.rs` lines 420 to 432 (the reference cell only
without a Dirichlet pressure); `crates/rcfd-fv/src/gradient.rs` lines 344 to 374 (`limiter`); `crates/rcfd-fv/src/simple.rs`
lines 48 to 60 (`BodyForce`); `crates/rcfd-turb/src/sst.rs` lines 20 to 40 and 270 to 282 (the Spalding wall law in the
wall cells; the transient form: one sweep per outer iteration, the eddy viscosity not relaxed); `crates/rcfd-api/tests/f885_alpha_free_face_flux.rs`
lines 1 to 270 and 820 to 1300 (the case text, the measures, the cavity, the fan harness, the record); `docs/process/tolerances.md`,
the table.

No published source outside the repository was read for this registration. The hypotheses come from the code: each names
a place where the outer iteration's map takes a value that is either frozen at the start of the outer iteration (lagged)
or a non-smooth function of the iterate. Nothing here rests on a published result.

## 1. What the outer loop of the fan iterates (from the code)

In outer iteration `k` of a step, `step_with` (`pimple.rs`) does, in order: the hook (line 1842) with the iterate `u_(k-1)`
and flux `phi_(k-1)`, which on the fan (`sim.rs` 4396 to 4418) evaluates the rotating-frame source from `u_(k-1)`
(`up.source_force`, line 1844; Coriolis `-2 Omega x u` and the centrifugal part) and makes one k-omega SST sweep from
`(u_(k-1), phi_(k-1))` whose eddy viscosity becomes the face viscosity of the predictor (`up.nu_face`, lines 1850 to 1854;
`sst.rs` 32 to 35: not relaxed in the transient form; the wall cells by Spalding's law, lines 23 to 30); the momentum
predictor with the convecting flux `phi_(k-1)`, the linear-upwind deferred correction from the least-squares gradient of
`u_(k-1)` limited by Barth-Jespersen (line 2026), the rotational periodic partners lagged at `u_(k-1)`
(`PeriodicLinks::build`, line 2058; implicit only with `rotational_periodic_implicit`, line 202), the relaxation
`alpha_u = 0.9`, solved by BiCGStab to `tol_rel 1e-8` and at most 500 iterations (line 2114; the defaults at 1063 to 1067,
the case sets none); two correctors, each a pressure solve by AMG-CG to `tol_rel 1e-7`, at most 3000 iterations (lines
1068 to 1072, 2424; `sim.rs` 2166). With the switch of ADR 0811 the relaxed iterations' flux has the term
`(1 - alpha_u)(phi* - F(u*))`.

A fixed-point iteration whose map is smooth near its fixed point contracts geometrically there; the cavity's does (ADR
0810 K4, ADR 0811 K4s: `C` 9.1e-9). A map that does not contract but stays at a level (the fan: `d_k` 4e-3 to 1e-2 m/s
from the 5th to the 20th iteration, ADR 0811 F2s) points to one of: a lagged quantity whose coupling to the iterate is
strong enough that the lag does not contract (a spectral radius near 1), a non-smooth function of the iterate that
switches between iterations (the map is then piecewise and can cycle), or a floor that the linear solvers leave.

## 2. The hypotheses (fixed now)

| Id | Hypothesis | Code | Control |
| --- | --- | --- | --- |
| HT | the turbulence sweep in the hook: the eddy viscosity (with the Spalding wall law in the wall cells: the tip-gap cells touch the blade tip and the moving shroud) changes with each outer iteration and that feedback does not contract | `sim.rs` 4414 to 4417; `pimple.rs` 1850 to 1854; `sst.rs` 23 to 35 | GT: the face viscosity of the step's first outer iteration kept for its later ones |
| HS | the rotating frame's Coriolis source, explicit from the iterate, does not contract | `sim.rs` 4400 to 4404; `pimple.rs` 1844, 1884 | GS: the source force of the step's first outer iteration kept for its later ones |
| HL | the Barth-Jespersen limiter switches between outer iterations (a `min` over the neighbours, piecewise in the iterate) | `pimple.rs` 2023 to 2030; `gradient.rs` 344 | GL: no limiter (`linear_upwind_limiter = None`; the deferred correction stays) |
| HP | the linear solvers' tolerances leave a floor within an outer iteration | `pimple.rs` 1063 to 1072, 2114, 2424 | GP: momentum `tol_rel 1e-12`, at most 5000 iterations; pressure `tol_rel 1e-11`, at most 20000 iterations |
| HR | the rotational periodic seam, lagged in the predictor, does not contract | `pimple.rs` 2058, 202 | GR: `rotational_periodic_implicit = true` (ADR 0664) |

Ruled out from the code, without a run (fixed now):

* A non-orthogonal correction that switches: the case sets no `non_orthogonal_correctors` (default 0, `schema.rs` 887),
  so the pressure equation has none; `gradient_momentum` is `None`, so the momentum Laplacian is the plain one
  (`pimple.rs` 99 to 102).
* The pressure reference: the outlet pressure is fixed, so the reference cell is not used (`pressure.rs` 426 to 431).
* The switch of ADR 0811: the stall was there without it (ADR 0810 F2, `C` 0.0366).

**The combined control GA**: GT, GS, GL, GP and GR together. It separates "one of the five" and "none of the five, alone
or together" from "the five only together".

What each control can show: a control that removes the cause makes the relaxed loop contract (CONVERGES); one that does
not leaves it stalled. A frozen viscosity or source changes the problem the step iterates (the step then solves with the
first iteration's value), and the unlimited scheme changes the discretisation; neither is a remedy, both are controls of
contraction only.

## 3. The instrumentation (fixed now)

Two settings in `crates/rcfd-fv/src/pimple.rs` only, both default `false`, read only in the hook block of `step_with`
(lines 1842 to 1855 of the base):

* `PimpleSettings::outer_freeze_source`: in outer iterations after the first of a step, the hook's `source_force` is not
  taken; the one of the first outer iteration stays.
* `PimpleSettings::outer_freeze_viscosity`: in outer iterations after the first of a step, the hook's `nu_face` is not
  taken; the face viscosity of the first outer iteration stays (the hook still runs, so the turbulence model makes its
  sweeps as before; only the momentum does not see them within the step).

With both off no line runs differently (bit-identical; `baseline_hashes` unchanged). Without a hook neither is read. No
case key, no schema, no golden, no hash table. GL, GP and GR use existing settings, set on the built solver as ADR 0811
set its switch.

## 4. The controls (fixed now)

**Fan**: 20 steps of H(dt) on B2 from the uniform start, `h_text(4.605e-5)` of ADR 0811 copied unchanged, both records
of ADR 0767 E-2 on, `alpha_free_face_flux = true`, `n_outer = 20`; settings changed on the built solver. Runs, one after
the other in one heavy test, each once: G0 (as F2s of ADR 0811), GT, GS, GL, GP, GR, GA.

**Cavity** (`box_2d(8, 8, 1, 1)`, lid `(1, 0, 0)`, `nu = 0.01`, `PimpleSettings::pimple(0.01, 20)`, BDF2, `dt = 0.05`,
from rest, 3 steps, the record on, the switch on): KC0 (K4s of ADR 0811); KCh, with a test hook that returns the source
`-2 Omega x u` with `Omega = (0, 0, 1) 1/s` (cell values, face values `a_f . S_f` with `a_f` the mean of the two cells,
the owner value on boundary faces) and the face viscosity `nu (1 + 0.5 |u_o|)` (`u_o` the owner cell velocity); KCf, KCh
with both freezes on.

**The measures** (ADR 0810 section 5, ADR 0811 section 3, upper median): `C` = median of `d_19 / d_2` over steps 11 to 20
(fan) or 1 to 3 (cavity); **CONVERGES** if `C <= 1e-2`, **STALLS** if `C > 1e-2`, **UNDECIDED** if NaN or the run did not
complete. Reported beside, without a limit: `J`, `Q`, the median `d_19`, its cell and its centre, the patches the cell's
faces belong to, seconds per step.

**The result (fixed now).** G0 must reproduce F2s of ADR 0811 (check R0 below); if it does not, the instrumentation is
wrong and the work stops and reports. Let `S` be the set of the five single controls GT, GS, GL, GP, GR that CONVERGE.
* `S` has one member, and no single control is UNDECIDED: the result **names** its hypothesis.
* `S` is empty, no single control is UNDECIDED, and GA STALLS: the result **rules all out** (the five, alone and together;
  the cause is elsewhere in the step).
* Otherwise (two or more converge; `S` empty and GA CONVERGES or UNDECIDED; a single control UNDECIDED): the result
  **names none**, with the reason.

Estimates (none is a limit): HT is the one I expect (the sweep is not relaxed and the tip-gap wall cells take a law of the
wall that is nonlinear in the cell velocity); HL second; HS not (the Coriolis lag `2 Omega dt / 1.5` is about 1e-2 per
iteration for `Omega` 155.6 1/s); HP not (a relative floor of 1e-7 is far below `C` 0.026); HR uncertain.

**Time, stated before the start.** ADR 0811 F2s took 73 s for 20 steps of 20 outer iterations on this machine (16 logical
processors, shared). GP and GR cost more per iteration (more linear iterations; a block solve); GA both. Estimate: about
12 min for the seven runs, in one background process; no cap.

## Table of checks

There is no clause of the acceptance of F-885 that this work can meet (check H is not run); the brief's task is the
diagnosis and its decision rule.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| B1 | every control of section 4 ran as registered: G0 to GA complete 20 steps or stop with the stop recorded; KC0, KCh, KCf complete 3 steps | as stated | the brief, decision rule | all run | n/a | solution verification |
| B2 | the result names one hypothesis or rules all out, by the rule of section 4 | as stated | the brief, decision rule | HT named | n/a | solution verification |
| B3 | no solver default changes: `cargo test -p rcfd-api --release --test baseline_hashes` passes, `baseline_hashes.rs` unchanged | passes, file unchanged | the brief; rule 3 | passes | n/a | unchanged default output |
| R0 | G0 against F2s of ADR 0811 (`docs/investigations/f885-alpha-free-face-flux/fan_controls.csv`): every `d_k` of 20 steps | relative `<= 1e-6` | `docs/process/tolerances.md`, "Same discrete problem by two routes", widened to the 10 printed digits as ADR 0811 S1 | 0 | n/a | unchanged default output |
| K1 | KC0, KCh, KCf: `C` | `<= 1e-2` (CONVERGES) each | ADR 0810 section 5 (ADR 0183's outer ratio, as ADR 0767 E-1 states it) | KC0 9e-9 (ADR 0811 K4s); KCh, KCf below 1e-6 | above 1e4 | code verification |
| T1 | two routes, the cavity with the test hook, 3 steps of 5 outer iterations: freezes on with the hook as it is, against freezes off with a hook that returns, in every outer iteration of a step, its value of that step's first call; one route pair per freeze (source, viscosity) | `u`, `p`, flux equal bits | `docs/process/tolerances.md`, "Bit-identity of default output" (the same operations by two routes) | equal | n/a | code verification |
| T2 | negative control of T1: each freeze on against off, both with the hook as it is | flux differs after some step | this registration (the hook's values change with `u`; a freeze that is not read leaves them equal) | differs | n/a | code verification |
| T3 | the freezes on without a hook (the cavity, 3 steps, `N = 5`): `u`, `p`, flux, record equal bits to both off | equal bits | "Bit-identity of default output" | equal | n/a | unchanged default output |
| T4 | the result rule of section 4 and the class of `C` on built records: `C` 1e-3, 0.1, NaN give CONVERGES, STALLS, UNDECIDED; the sets of section 4 give named, all ruled out, none | each as built | this registration, section 4 | as built | n/a | code verification |
| G2 | the fan's outcomes from the committed record (`docs/investigations/f885-outer-stall/fan_controls.csv`): seven runs, their classes and `C`, the result | equal; `C` within 1e-6 relative | "Same discrete problem by two routes", widened to the printed digits as ADR 0811 G2 | 0 | n/a | unchanged default output |
| D1 | under `crates/*/src` only `crates/rcfd-fv/src/pimple.rs` changes; `docs/roadmap.json`, the ledger, every golden and every existing test file unchanged; the committed mutation patches of `f885_check_h_outer_record`, `f885_outer_fixed_points` and `f885_alpha_free_face_flux` still apply (`git apply --check`), re-based in this branch with the mutation unchanged if not | as stated | the brief, owned files; rule 3 | as stated | n/a | unchanged default output |
| P | `cargo test -p rcfd-fv --release --lib`; the new test file; the existing tests of the changed step `f885_alpha_free_face_flux`, `f885_outer_fixed_points`, `f885_check_h_outer_record`; `cargo fmt -p rcfd-fv -p rcfd-api --check`; `cargo clippy -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

The class number 1e-2 of `C` is not a pass or fail limit of a fan control; it classes a run (ADR 0810 section 5). No row
is of kind validation: nothing here is compared with a measurement.

## Tests (fixed now)

All in `crates/rcfd-api/tests/f885_outer_stall.rs` (header: ADR 0815, F-885).

* Run by the gate: `f885_os_freeze_two_routes` (T1, T2); `f885_os_freeze_without_hook` (T3); `f885_os_cavity_contracts`
  (K1); `f885_os_rules_on_built_records` (T4); `f885_os_fan_from_the_committed_record` (G2, and R0 from the two committed
  records).
* Heavy record (`#[ignore]`, run once by me): `f885_os_fan_controls_record`, G0 to GA, writing `fan_controls.csv` to the
  folder named by `F885_OS_OUT`; the fast tests on the same code path are T1 to T3 (the freezes) and ADR 0811's
  `f885_af_short_run_on_the_check_h_path` (the fan harness).

The outcomes are written into the test file after the registered run (`FAN_RECORD`), as ADR 0811 did; the form and the
tolerance are fixed here. Records: `docs/investigations/f885-outer-stall/` (`fan_controls.csv` in the form of ADR 0811's:
run, step, `k`, `d_k`, cell; the log of the heavy run; the mutation log).

## Negative controls and planned mutations

On a scratch copy of the branch (`git archive` into the scratch folder, its own target folder, every file touched after
extraction), each against the gate tests of the new file; the heavy record is not rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: `outer_freeze_source` not read (the hook's source taken in every outer iteration) | `f885_os_freeze_two_routes` (T1) |
| M2 | solver: `outer_freeze_viscosity` not read | `f885_os_freeze_two_routes` (T1) |
| M3 | solver: both freezes read as always on | `f885_os_freeze_two_routes` (T2) |
| M4 | solver: the freeze also in the first outer iteration (the source of the previous step kept) | `f885_os_freeze_two_routes` (T1) |
| M5 | test: CONVERGES at `C <= 1e-4` | `f885_os_rules_on_built_records` (T4) |
| M6 | record: `fan_controls.csv` cut to its first half | `f885_os_fan_from_the_committed_record` (G2) |

The committed patch (`crates/rcfd-api/tests/f885_outer_stall.mutation.patch`) is M1, with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f885_outer_stall f885_os_freeze_two_routes`.

## Decision rule

* **Done** for this registration if every control ran as registered (B1) and the result names one hypothesis or rules all
  out (B2), with B3, R0, K1, T1 to T4, G2 and D1 met.
* **Partial** if a control did not run as registered, or the result names none, with the reason and what it would take.
* **Stopped and reported** if the freezes cannot be made opt-in with the defaults bit-identical, if G0 does not reproduce
  F2s (R0), or if anything else in the solver would need changing.

F-885 itself stays partial whatever this gives, until check H decides (the brief). A defective set-up found before a
registered run is corrected by an amendment committed before the corrected run.

## What is not covered

* Any remedy (a registration of its own); any change of a default; check H, H(dt/2), a long run.
* A hypothesis other than the five and the three ruled out from the code; a decomposition of a cause by cell or term.
* The window state of check H (20 steps from the uniform start only, as the brief says).

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation: the remedy, and any question of scope.
