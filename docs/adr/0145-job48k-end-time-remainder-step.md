# ADR 0145: Job 48K, the end-time remainder step: a fixed-step transient run ends on a whole step

Status: **PROPOSED 2026-10-28** (set-up, limits and the order of runs registered here before any run of this job).
Written at main 0a69af4 (wave 48H merged, ADR 0142). Job 48K fixes the defect ADR 0142 found and did not fix (its
addendum "Found, not fixed" and question 6). House rules of ADRs 0110-0142 apply: the set-up and the limits below are
fixed before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a
defective set-up is corrected by an amendment registered before the corrected run; defaults of existing code stay
bit-identical for every existing case and test (bit-identity is per platform, ADR 0137);
`crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new crate; the crate graph stays acyclic.
Parents: ADR 0142 (the smoke that found the defect), ADR 0050 (adaptive time stepping: `LAND_STRETCH`, snapped
landings), ADR 0077 part C (run-time warnings of the solver), ADR 0124 (the field store's time rule), ADR 0137
(per-platform values).

## Problem (found in ADR 0142; the maintainer's decision)

ADR 0142's smoke ran the cylinder at Re 100 (64 x 48 O-grid, PISO, kick 0.5 at (1.5, 0)) with `solver.dt = 0.05` to
`solver.end_time = 100`. The last step was 3.5e-12 s long; C_D and C_L of that step were -4e5 and 9e5, and that state
was the run's final result, its last stored snapshot and its final report values. The maintainer decided: fix it, with
defaults bit-identical for every existing case and test, and evaluate before any run this approach: when the remainder
is below a registered tiny fraction of dt, take no separate step (absorb it by stretching the previous step, or treat
the end time as reached within a tolerance); for larger remainders keep today's behaviour. Reproduce the defect with a
failing test first (the control), then fix, then show that the control passes. Add a Studio- or runner-visible warning
only if it is cheap.

## Context (read at 0a69af4)

1. **The landing code.** `Simulation::step` (`crates/rcfd-api/src/sim.rs`) has four fixed-step transient paths with the
   same rule: the step is `d = min(dt, max(end - t, 1e-12))` and the run is done when the new time is
   `>= end - 1e-12` (an absolute tolerance):
   * incompressible PISO/PIMPLE (`Engine::Transient`) with the fixed `solver.dt` or the legacy `solver.max_courant`
     controller (`rcfd_fv::ddt::CflController`), whichever of the dynamic-mesh, momentum-source/component/two-way
     particle, or plain step takes it;
   * prescribed (frozen) flow, energy only (`Engine::Transient` with `solver.frozen_flow`);
   * transient variable density (`Engine::VarDensity`, ADR 0009);
   * VOF (`Engine::Vof`, ADR 0049).

   Every solver advances its time by one addition, `time += d` (`rcfd-fv` `pimple.rs`, `pimple/fractional.rs`,
   `varden.rs`, `vof.rs`; the frozen path in `sim.rs`), so the time after n steps is the floating-point sum of the steps.
2. **Why a remainder appears.** Each addition rounds, so n steps of dt can fall short of the end time by more than
   1e-12. Arithmetic in Python (the same IEEE operations; not a solver run): with dt 0.05, 2000 additions give
   99.99999999999646, so a step 2001 of 3.538502824085299e-12 s (7.08e-11 dt) follows: ADR 0142's "3.5e-12 s". The
   smallest end times `k dt` (as typed) that leave such a step: k = 929 for dt 0.1, 1107 for 0.05, 1388 for 0.04, 4813
   for 0.01, 5313 for 0.005; so runs of about a thousand steps and more. With dt 0.0625 (a power of two) the sums are
   exact.
3. **Why the pressure is spoiled.** The pressure of a projection step balances the divergence of the predicted velocity
   over that step, so its part from the divergence error of the prediction scales with 1/d: a step of 7e-11 dt
   multiplies it by about 1.4e10 (ADR 0142: C_D -4e5, C_L 9e5 at that step).
4. **Why it becomes the last stored snapshot.** The field store's time rule (`fieldstore.rs`,
   `floor(t / interval + 1e-9)`) stores step 2000 (t = 100 - 3.5e-12) in the t = 100 slot; step 2001 is not due by
   time, but `Hook::finish` stores the final state because its step differs from the last stored one. So the store
   holds 101 snapshots and the last is the spoiled state.
5. **Paths with their own landing (not changed here).** The adaptive controller of ADR 0050 lands a planned step on
   the end time or a write time when the remainder is within `LAND_STRETCH = 1.05` of the step and snaps the time; its
   step-doubling error control lands the same way inside `ControlledRun::advance`. The compressible engine
   (`comp.rs` `advance(t_end)`) is explicit, has the relative end tolerance `1e-12 max(end, 1)` and no pressure
   projection. Particle tracking has its own `particles.end_time`; the tracker follows the step the flow actually took
   (`particle_run.rs`: `after_step` uses `t - t_prev`, `pre_step` receives the flow's step), so a stretched flow step is
   tracked as taken.
6. **Existing golden and shipped cases.** `baseline_hashes.rs` runs three transient cases: the cavity with PISO and with
   PIMPLE, dt 0.01 to 0.3 (30 steps), and the heated cavity with PISO, dt 0.005 to 0.2 (40 steps). By the arithmetic of
   item 2 their sums reach 0.3 and 0.2 within the tolerance: no remainder step. Shipped transient case files: the two
   example cylinders (dt 0.0625, end 80: exact); `cases/` holds no incompressible transient case (`sod` and `wedge` are
   compressible). The schema defaults (dt 0.01, end time 1): 100 additions reach 1.0000000000000007, no remainder step.
7. **The Studio already counts whole steps.** `crates/rcfd-studio/src/store_plan.rs` estimates `ceil(end/dt - 1e-9)`
   steps: 2000 for dt 0.05 and end time 100, where today's run takes 2001.
8. **Warnings.** `Simulation::take_runtime_warnings` (ADR 0077 part C, the pressure-lag check) hands `warning` event
   bodies to `runctl::drive`, which emits them as `warning` events: `rcfd run` prints `warning: ...` on stderr, the run
   service records the solver's stream in the run's `events.jsonl` (kind `solver`), and the Studio lists a run's events
   (`ui.rs` `event_text`, a solver event as its JSON cut at 160 characters). The setup rules (`rules/setup_rules.json`)
   are declarative comparisons; a remainder cannot be computed in them without a new operator.

## Options (evaluated before any run)

| Option | What | Verdict |
|---|---|---|
| A | Absorb: a step that would leave a remainder below a fraction f of itself is stretched to end on `solver.end_time` | **chosen** (D1, D2) |
| B | The end time counts as reached within a tolerance: done when `t >= end - f dt` | rejected: the run ends short of its end time (at 99.99999999999646 for 100), so its final time, the time of its last stored snapshot and the abscissa of its last monitor point are not `solver.end_time`; the store's t = 100 slot holds a time below 100; ADR 0142 S5 and the Studio compare against `solver.end_time` |
| C | Time from a step counter (`t0 + k dt`, as ADR 0050's landing targets) | rejected: changes the time, and so the result, of every step of every run (not bit-identical) |
| D | A larger fraction, such as ADR 0050's 5 % | rejected: it also changes runs whose end time leaves a deliberate short last step; the decision keeps today's behaviour for larger remainders |

## Decisions (none is a new maintainer decision; the open points are questions at the end)

1. **D1 Landing rule.** On the four paths of Context 1, `d = min(dt, max(end - t, 1e-12))` as today, with `dt` the step
   proposed for this step (`solver.dt`, or the legacy Courant controller's step). If `t + d < end - 1e-12` (today the
   run would not be done after this step) and `end - (t + d) < f d`, the step taken is `end - t` and the run is done
   after it. Otherwise nothing changes: the same `d`, the same done test. The new time `t + (end - t)` equals `end`
   exactly whenever `t >= end / 2` (the subtraction is then exact, Sterbenz's lemma), which holds for every run whose
   last two steps are of similar size; otherwise it is within a few units in the last place of `end`, and the run is
   done by the rule itself, not by the tolerance. On the incompressible path D1 applies only to the solver's own fixed
   or legacy-Courant step: not to a planned step of the adaptive controller and not when the step-doubling error
   control is configured (both land by ADR 0050's rule; the local clamp of the error-control branch stays as it is).
2. **D2 The fraction f = 1e-6** (`REMAINDER_ABSORB` in `sim.rs`).
   * It lies above every rounding remainder of realistic runs. Each addition `t + dt` errs by at most `u t`
     (u = 2^-53, the unit round-off), so n steps fall short by at most about `u n^2 / 2` steps of dt: 5.6e-11 dt at
     n = 1000 and 2.2e-10 dt at n = 2000 (measured by the arithmetic of Context 2: 7.08e-11 dt). The bound stays below
     1e-6 dt up to n = 1.3e5 steps when every rounding error has the same sign, and up to several million steps when
     the signs mix (the sum then grows like n^1.5).
   * It is small enough that the stretched step is an ordinary step: its length, its Courant number and its
     time-discretisation error change by a relative 1e-6 at most.
   * An end time chosen on purpose within 1e-6 dt of a whole number of steps needs about seven more significant digits
     than dt; such a run now ends on its end time with one step at most 1e-6 longer.
   * A remainder of 1e-6 dt and more is still a separate step (the decision); its pressure carries the 1/d
     amplification of Context 3, which D4 reports.
3. **D3 Scope.** Changed: the four paths of Context 1. Unchanged: the adaptive controller and its error control
   (ADR 0050), the compressible engine, particle tracking (`particles.end_time`), the reactor crate, the setup rules,
   the Studio, the schema (no new key, so no settings hash changes; ADR 0133 question 5).
4. **D4 Warning** (cheap: about twenty lines on the existing channel of Context 8). When a separate last step is taken
   that is shorter than 5 % of the step proposed for it (`d < dt` and `d < 0.05 dt`; 0.05 is ADR 0050's
   `LAND_STRETCH - 1`, the remainder that controller already avoids by stretching), the paths that solve a pressure
   (incompressible, variable density, VOF; not prescribed flow) queue a warning that `take_runtime_warnings` hands out
   with the pressure-lag warnings: `kind` `short_last_step`, `step`, `time`, `dt` (the step taken), `dt_proposed`,
   `fraction` (`dt / dt_proposed`), `end_time`, and a message that names the step, its fraction, `solver.end_time` and
   the remedy (choose `solver.end_time` as a whole number of steps of `solver.dt`). It reaches `rcfd run`'s stderr, the
   run's `events.jsonl` and the Studio's event list. An absorbed remainder (D1) gives no warning. Not done: a
   check-time warning (it needs a new rule operator and changes the rule output of existing cases; question 2).
5. **D5 Placement.** `crates/rcfd-api/src/sim.rs`: two constants, one private helper (`fixed_step`), the four call
   sites, the warning queue; `crates/rcfd-api/tests/end_time_landing_w48k.rs` (new); `docs/case-schema.md` (one
   paragraph); this ADR. No other crate, no dependency, no workspace edge.
6. **D6 Defaults.** No setting: the rule is always on. It changes a run only when D1 fires, that is only a run that today
   ends with a separate step shorter than 1e-6 of the step proposed for it. B1-B4 are the evidence that no existing case
   or test is such a run. If B4 finds one, the job reports the test and the change to the maintainer before any merge
   (the fix necessarily changes such a run); no limit or test is edited for it.

## Registered checks (limits fixed here, before any run)

Release builds, output to files. The new test file is `crates/rcfd-api/tests/end_time_landing_w48k.rs`, committed with
this ADR. "Unfixed tree": the registration commit (0a69af4 with this ADR and the test file). "Fixed tree": the commit
that adds D1-D4.

Set-ups of the landing tests (L, W1, B2), each run to its end:

| Path | Set-up | dt | n |
|---|---|---|---|
| PISO | the `baseline_hashes.rs` cavity: `cavity_2d` 20 x 20, nu 0.01, lid 1, `linear_upwind`, PISO | 0.01 | 3 |
| PISO Courant | the same with `solver.max_courant = 1e6` (the legacy controller then proposes its cap, `solver.dt`) | 0.01 | 3 |
| Prescribed flow | the ADR 0024 contact case of `cht_case.rs` with 32 cells (BDF2, a solid zone) | 0.0025 | 3 |
| Variable density | the ADR 0009 closed-box heating case of `transient_variable_density.rs`, `low_mach` | 0.05 | 3 |
| VOF | the ADR 0049 still pool of `vof_tree.rs` | 1e-3 | 3 |

Variants: `end = t_n + f dt`, with `t_n` the f64 sum of n steps of dt and the end time written with 18 significant
digits (it reads back exactly): E f = 0, T f = 0.9e-6, J f = 1.1e-6, N f = 0.3; on PISO also W- f = 0.049 and
W+ f = 0.051.

| Id | Check | Limit | Negative control |
|---|---|---|---|
| C1 | **The reported defect** (`c1_cylinder_dt_0_05_to_100_ends_on_a_whole_step`): `example-projects/wind-tunnel/cases/cylinder-re100.toml` with `solver.dt = 0.05` and `solver.end_time = 100` (ADR 0142 smoke row 2), driven by `runctl::drive` with an output folder (the case's own store: every 1.0, f32, `ux uy p vorticity_z`) | (a) exit 0, state `finished`, reason "end time 100 reached"; (b) 2000 steps; (c) final time 100.0 bit for bit; (d) monitor `dt`: steps 1 to 1999 equal 0.05 and step 2000 equals `100 - t_1999` bit for bit; (e) `cd` and `cl`: the last value lies within [min - 0.1 range, max + 0.1 range] of the 200 values before it, and its change from the previous value is at most 5 times the largest step-to-step change among those 200; (f) the store: 100 snapshots, the last at step 2000 and time 100.0 bit for bit, its largest abs `p` at most twice that of the snapshot before it; (g) no `short_last_step` warning | the same test on the unfixed tree is **predicted to fail**: (b) with 2001 steps, (d) with a last step of 3.5385e-12 s, (e) and (f) (101 snapshots, the last one spoiled); it prints its measured values before asserting |
| L1-L5 | Landing on each path (`l1_piso_...` to `l5_vof_...`), from the times of the step summaries | E: n steps, t_1 to t_n equal the f64 partial sums bit for bit; T: n steps, t_1 to t_(n-1) the partial sums and t_n = end bit for bit; J and N: n + 1 steps, t_1 to t_n the partial sums and t_(n+1) = end bit for bit (today's remainder step) | T on the unfixed tree is **predicted to fail** on all five paths (n + 1 steps) |
| L6 | Warnings of `take_runtime_warnings` after the L runs (in the same tests) | exactly one `short_last_step` for J on PISO, PISO Courant, variable density and VOF, its `fraction` within 1e-6 relative of 1.1e-6; none for E, T and N; none on prescribed flow | J on the unfixed tree is **predicted to fail** (no such warning exists) |
| W1 | The warning as a run event (`w1_short_last_step_warning_is_a_run_event`, `runctl::drive`, PISO) | exit 0 for every variant; J and W-: exactly one `warning` event of kind `short_last_step`; E, T, N and W+: none | predicted to fail on the unfixed tree |
| B1 | `baseline_hashes.rs` on the fixed tree | its three tests pass unchanged (the ignored one stays ignored) | (the file is not edited) |
| B2 | Pre-fix hashes (`b2_final_states_equal_the_pre_fix_hashes_where_no_step_is_absorbed`): steps and FNV-1a (as `baseline_hashes.rs`) of the final `ux uy p T alpha` (those the run has) and the face flux of every variant on every path, printed by run R1 on the unfixed tree and written into the test file before any fix code (R2) | on the fixed tree, Windows x86_64: E, J, N on every path and W-, W+ equal their pre-fix values; other platforms print (ADR 0137) | T must differ from its pre-fix value on every path (the hash sees the stretched step); B2 on the unfixed tree after R2 reproduces the E, J, N, W values (determinism) and fails on T |
| B3 | `timectl_tree.rs` (ADR 0050 S11: the case-tree adaptive controller equals the library driver bit for bit) on the fixed tree | passes | (D1 excludes those paths) |
| B4 | **Probe of the existing tests**: a `git archive` of the fixed tree in a scratch folder with one statement added in `fixed_step` that appends a line to the file named by `RCFD_W48K_PROBE` when D1 fires and when a remainder step is taken (the program and its arguments, t, the step, the proposed step, the end time); nothing else changed, never committed; its own target folder; `python scripts/gate_test.py --jobs 3 -p rcfd-api -p rcfd-cli -p rcfd-mcp -p rcfd-studio -p rcfd-runner -p rcfd-gui -p rcfd-assistant` (every crate that links rcfd-api or starts the `rcfd` binary in its tests) | (a) D1 fires in no process other than this job's test file: 0 lines; (b) 0 failed tests, where a failing test is run alone on the unfixed tree and reported as pre-existing if it fails there too | the new test file's own D1 lines (C1, the T variants) show that the probe logs |
| F1 | `cargo fmt -p rcfd-api -- --check`, `cargo clippy -p rcfd-api --all-targets -- -D warnings`, `python scripts/limit_audit.py` | clean; 0 hits in the new file, or each hit explained | |

Reported without a limit: the remainder steps that existing tests take (B4 lines): their number, the smallest fraction,
and those below 0.05 (they now carry the D4 warning); C1's wall time; C1's values of the last step on the unfixed tree.

## Order of runs

1. R1, unfixed tree: the new test file. Predicted: C1, the T variants of L1-L5, J of L6 and W1 fail; B2 prints its
   values and fails for want of recorded values.
2. R2: B2's printed values written into `PRE_FIX` (a commit with no other change); B2 again on the unfixed tree:
   E, J, N, W equal, T fails its "differs" check.
3. R3: the fix (a commit), then on the fixed tree the new test file, `baseline_hashes.rs` and `timectl_tree.rs`.
4. R4: B4.
5. R5: F1.

Results go into an addendum; a miss or a defective set-up is handled by the house rules above.

## Not in CI (for the maintainer)

`cargo build --release -p rcfd-cli`, then `rcfd run` the cylinder case with `--set solver.dt=0.05 --set
solver.end_time=100` (or start it from the Studio with those values): the run ends at step 2000, `metrics.json`'s `cd`
is about 1.3, the last stored snapshot is at t = 100. With `solver.end_time = 100.001` it ends with a step of 0.001 s
(2 % of the step) and the warning `short_last_step` on stderr and in the run's events.

## Out of scope

The compressible engine's landing; the adaptive controller; a check-time warning; the Studio's store-count estimate
(question 3); regenerating the ADR 0142 examples (their dt 0.0625 lands exactly, so they are unchanged by
construction).

## Questions for the maintainer (open at registration; the addendum may add to them)

1. f = 1e-6 is registered. A larger fraction (up to ADR 0050's 5 %) would also absorb deliberate short last steps, which
   changes such runs. Keep 1e-6?
2. The warning comes with the last step. A warning before the run (a setup rule) needs a new rule operator and changes
   the rule output of existing cases. Wanted?
3. The Studio's store-count estimate allows a slack of 1e-9 steps (`ceil(end/dt - 1e-9)`). An end time between 1e-9 and
   1e-6 steps past a whole step is now absorbed, so the estimate counts one step (and possibly one snapshot) more than
   the run takes. Align it with f (a Studio change)?
4. The compressible engine has the same kind of landing (relative tolerance) but no projection. Leave it?
5. The Studio's event list cuts a solver event's JSON at 160 characters, so the warning's message may be cut there.
   Show solver warning messages in full (a Studio change)?

## Files

This ADR; `crates/rcfd-api/src/sim.rs`; `crates/rcfd-api/tests/end_time_landing_w48k.rs`; `docs/case-schema.md`.
