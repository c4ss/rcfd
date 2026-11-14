# ADR 0444: F-469 registration: failing-participant checks from a start state that uses no restore at the start of an iteration

Status: **REGISTERED 2026-11-14, written before any code of F-469 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-469 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script and no library code was run.
Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-469 (deps F-465, low priority, size XS), the follow-up of
[ADR 0364](0364-f465-registration-failing-fluid-in-a-later-iteration.md) (its remaining question 1, option (b)). Branch
`f469-coupled-step-isolation` from main 25f46045, worktree `w49i-work/f469`, scratch `w49i-work/f469-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits are fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test; a defective set-up gets a corrected test by a committed amendment before the corrected run; each registered
run is made once. Tests only: one new file `crates/rcfd-post/tests/sb_solid_coupling_fail_fluid_loose.rs`. The file of ADR 0364
(`sb_solid_coupling_fail_fluid.rs`) and the older ones are not edited and stay as they are. No library file, shared solver file,
golden or hash table is touched, so every default result keeps its bits by construction. A library defect found is reported, not
patched. Library mutations run on a scratch copy only. No new dependency, no unsafe code; clean-room: first-principles
fixed-point arithmetic, no paper.

## What F-469 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. With the StaticSolid and the mock fluid of ADR 0286, a
> start state reached without any coupler iteration beyond the first (a loose-coupling first step, then F0 doubled), registered
> with its arithmetic, a mock fluid failing in iteration 2 and in iteration 3 stops the step with the fluid's name, the iteration
> number and its message and both participants bit-equal to the start; mutations that skip the coupler's restore of the fluid or
> of the structure at the start of iteration 2 or of iteration 3 (ADR 0364 M16 to M19) and the mock's restore of the time (M25)
> are each caught by a failing-step or control check, not by a panic in the set-up.

Clauses: (c1) every limit not stated is registered here, before the run; (c2) the start state is a loose-coupling first step
(`max_iterations = 1`, `AtCap::Keep`) followed by F0 doubled, with its arithmetic registered below, and uses no restore at the
start of an iteration (no coupler iteration beyond the first); (c3) the fluid failing in iteration 2: the error names the fluid,
iteration 2 and carries the mock's message, both participants bit-equal to the start; (c4) the same for iteration 3; (c5)
M16, M17, M18, M19 and M25 are each caught by a failing-step or control check, and not by a panic in the set-up (so the set-up
must not panic under any of them).

## What was read (main 25f46045, read only)

1. `AGENT_RULES.md`; ADR 0364 in full (checks, mutations, addendum 1, remaining question 1); the test file
   `crates/rcfd-post/tests/sb_solid_coupling_fail_fluid.rs` in full (its set-up is copied); `crates/rcfd-fv/src/cosim.rs`
   `CouplingSettings`, `AtCap` and `coupled_step`.
2. Finding that shapes the design: in `coupled_step` the restores of both participants run only for `k > 1` and in the error arm.
   A step with `max_iterations = 1` never reaches `k > 1`; with `AtCap::Keep` a non-converged first iteration returns `Ok` (the
   cap error is only for `Stop`). So the loose first step calls neither the coupler's start-of-iteration restores nor, on
   success, the mock's `restore`. The mutations M16 to M19 (all at `k > 1`) and M25 (the mock's restore) then cannot reach the
   set-up.
3. **Not read:** `solid_coupling.rs` (the readings of ADR 0361 and 0364 are relied on), `cosim/mapping.rs`, `FlowWall`.

## Arithmetic before this registration (disclosed, by hand)

Units of `F0`; `f` the nodal total force (x); `a = k_f/k_s = 1.5`. The static solid is linear: the mean displacement under `f`
is `f F0/k_s`, so after a structure advance under `f` the mock fluid answers `g(f) = c - a f`, with `c` the mock's `f0` in units
of `F0`.

**Start state.** A fresh mock (d = 0, `c = 1`): the loose first step starts from `fluid.forces() = F0` (`f = 1`), advances the
structure under `f = 1`, then the fluid, which takes `d` for `f = 1` and time `dt`. Its test quantity `|r|/|ft|` has
`ft = g(1) = 1 - 1.5 = -0.5` and `r = ft - f = -1.5`, so it is `3`, above the tolerance `1e-10`; the cap 1 is reached and
`Keep` returns `Ok`, `converged = false`, `iterations = 1`. Then `c := 2`. Both participants stand at time `dt`. The mock's
force at the start of the checked step is `g(1) = 2 - 1.5 = 0.5` (the start of ADR 0361 gave `1.4`).

**Healthy checked step from this start** (`g(f) = 2 - 1.5 f`, fixed point `f* = 0.8`, tolerance `1e-10`, Aitken(0.5) or least
squares(0.5)):

- `f1 = 0.5`. Iteration 1: `g = 1.25`, `r = 0.75`, test quantity `0.75 / 1.25 = 0.6`. The first update is relaxed by 0.5 in
  both methods: `f2 = 0.5 + 0.5 * 0.75 = 0.875`.
- Iteration 2: `g = 2 - 1.3125 = 0.6875`, `r = -0.1875`, test quantity `0.1875 / 0.6875 = 0.272727...`. Aitken:
  `w = -w0 (r0 (r1 - r0)) / (r1 - r0)^2 = 0.5 * 0.75 / 0.9375 = 0.4` (below the cap 2), `f3 = 0.875 - 0.4 * 0.1875 = 0.8`.
  Least squares on a linear map: the secant through `(0.5, 1.25)` and `(0.875, 0.6875)` hits the fixed point exactly,
  `f3 = 0.8`. Both methods give `f3 = 0.8`, as in ADR 0361.
- Iteration 3: `g(0.8) = 0.8`, `r` about `1e-16`, converged. Iteration 3 exists and a 4th does not.

So the control has `iterations = 3`, test quantities `0.6`, `0.272727...`, `<= 1e-10`, and three advances of the structure and
of the fluid. The fluid's advance in iteration `n` runs after the structure's advance of iteration `n`, so a fluid failing in
its `n`-th call is reached for `n = 2, 3` by the argument of ADR 0364.

## Design

New file `crates/rcfd-post/tests/sb_solid_coupling_fail_fluid_loose.rs`, a copy of the ADR 0364 file with these differences
only: (1) `start(settings)` runs the loose first step with the same settings but `max_iterations: 1` and
`at_cap: AtCap::Keep` (built field by field if `CouplingSettings` is not `Clone`), unwraps it, then sets `fluid.f0 = 2.0 * F0`;
(2) the registered numbers of the controls are those above; (3) the new checks S1 to S3 on the start. The wrappers `CountSolid`
and `FailFluid` (fails after taking its state; the message carries the entry time), `failing_step`, `control` and the retry (g)
are as in ADR 0364.

The set-up does not call the coupler's start-of-iteration restores nor the mock's restore, so it must not panic under M16 to
M19 and M25. If a mutation still broke the set-up, that is reported as a panic and counts as not caught by a check.

## Checks

Update Aitken(0.5) for P1, F2, F3, N1; least squares(0.5) for P4, G2, G3. `n` is 2 or 3.

| Check | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| S1 | c2 | the loose first step: `Ok`, `iterations == 1`, `converged == false`, test quantity against 3 | as stated; relative `<= 1e-6` | 1, no, about 1e-13 |
| S2 | c2 | the mock's total x force at the start of the checked step (`forces()` summed) against `0.5 F0` | relative `<= 1e-6` | about 1e-13 |
| S3 | c2 | the first step left both participants at `dt` and moved the mock's wall displacement off zero | times equal `dt` within `1e-12 dt`; bits differ from zero | as stated |
| P1a | c2 control | healthy step, Aitken: converged, `iterations` | converged, `== 3` | 3 |
| P1b | c2 control | test quantity 1 against `0.6` | relative `<= 1e-6` | about 1e-13 |
| P1c | c2 control | test quantity 2 against `0.1875/0.6875` | relative `<= 1e-6` | about 1e-14 |
| P1d | c2 control | test quantity 3 | `<= 1e-10` | about 1e-15 |
| P1e | c2 control | advances of the structure and of the fluid in the step | both `== 3` | 3, 3 |
| P1f | c2 control | the step changes both states relative to the start | both differ | differ |
| P4a to P4f | c2 control | as P1 with least squares | as P1 | as P1 |
| F2a, F3a, G2a, G3a | c3, c4 | the step is `Err`; `participant == "fluid"` and `iteration == n` | exactly | as stated |
| F2b, F3b, G2b, G3b | c3, c4 | message equals `linear wall: injected failure at advance n, entered at t = {t0:?}` exactly (no restore text appended) | string equal | equal |
| F2c, F3c, G2c, G3c | c3, c4 | `error.time` bit-equal to `t0` | 0 bits | 0 |
| F2d, F3d, G2d, G3d | c3, c4 | structure and fluid advanced exactly `n` times each | `== n` | as stated |
| F2e, F3e, G2e, G3e | c3, c4 | fluid bits (time, wall displacement) and solid bits (time, displacement, interface forces) bit-equal to the start | 0 bits differ | 0 |
| F2f, F3f, G2f, G3f | c3, c4 control | the states at the failure were off the start (so e is not vacuous) | both differ | differ |
| F2g, F3g, G2g, G3g | extension | the retry with the healthy fluid reproduces the control: converged, 3 iterations, `node_displacements` and `residuals` bit-equal | 0 bits | 0 |
| N1 | control | `fail_at = 4`: `Ok`, converged, 3 iterations, 3 advances each, `node_displacements` bit-equal to the control's | as stated | as stated |

Negative controls: S (the start is what is registered), P1 and P4 (iterations 2 and 3 are reached; both states change), f (the
states are dirty at the failure), N1 (no 4th advance).

### Planned mutations (c5)

Run on a scratch copy of the committed tree (git archive) in `w49i-work/f469-scratch/mut`, by a script that applies one textual
change, builds and runs `cargo test --release -p rcfd-post --test sb_solid_coupling_fail_fluid_loose -- --nocapture
--test-threads=1` with `CARGO_BUILD_JOBS=4`; each is run once. The ids are those of ADR 0364.

| Id | Where | Change | Registered catchers (at least one, and no set-up panic) |
|---|---|---|---|
| M16 | `coupled_step`, `k > 1` block | the fluid's restore skipped at every `k > 1` | P1a, P4a (the control stops in iteration 2 by the fluid's time check), F2b, F3a, G2b, G3a |
| M17 | same | the structure's restore skipped at every `k > 1` | P1a, P4a, F2a, F3a, G2a, G3a (the solid stops iteration 2: participant `solid`) |
| M18 | same | the fluid's restore skipped only for `k > 2` | P1a, P4a, F3b, G3b (entry time `t0 + dt` in iteration 3); F2 and G2 cannot see it |
| M19 | same | the structure's restore skipped only for `k > 2` | P1a, P4a, F3a, G3a (the solid stops iteration 3); F2 and G2 cannot see it |
| M25 | mock `LinearWall::restore` (test file) | the time not restored | P1a, P4a, and the failing-step checks of F2, F3, G2, G3 |
| M9 | error arm | the structure's restore skipped | F2e, F3e, G2e, G3e (regression of ADR 0364) |
| M11 | error arm | the fluid's restore skipped | F2e, F3e, G2e, G3e (regression) |
| M24 | mock restore (test file) | the wall displacement not restored | F2e, F2g, F3e, F3g, G2e, G2g, G3e, G3g (regression) |

M16 to M19 and M25 are the mutations of the acceptance; the other three show that the new start did not lose the catches of
ADR 0364. All are registered as "to be caught"; none as an equivalent survivor. S1 to S3 are expected to pass under M16 to M19
and M25 (they do not use the mutated code); the result is recorded for each.

## Decision rule

**done**: every check above met on its registered run and each of M16 to M19 and M25 caught by at least one of the checks named
for it and not by a panic in the set-up (every test of the file starts and runs its checks; S1 to S3 pass under the mutations),
and M9, M11, M24 caught by their registered checks. **partial** (Rule I): a missed or unrun check; a registered mutation that
survives, is caught only by a check other than the registered ones, or only by a set-up panic; a mutation caught only after a
post-run amendment; or a control (P1, P4) that does not reach iteration 3. A miss of the extension (g) is a `*_known_miss`
record and alone does not block done unless its diagnosis shows a clause check rests on a defect. **todo**: the test file does
not build.

## Limits (what is not covered)

- Only a mock fluid failing by a returned `Err` after taking its new state, in its 2nd and 3rd advance, Aitken and least
  squares. Not covered: a fluid that fails before touching its state, the fluid's time check, bad force arrays, a failing
  `restore` itself, a failure in iteration `n > 3`, the at-cap error, a flow solver (`FlowWall` is not exercised).
- The start differs from that of ADR 0361 (the force at the start is `0.5 F0`, not `1.4 F0`): the failing-step checks are those
  of ADR 0364 on another trajectory.
- The bit comparison covers fluid time and wall displacement and solid time, displacement and interface forces; the checkpoint
  slots and `f0` are not compared.
