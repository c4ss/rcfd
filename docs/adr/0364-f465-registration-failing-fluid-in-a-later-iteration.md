# ADR 0364: F-465 registration: a failing fluid in iteration 2 and in iteration 3 of a coupled step

Status: **REGISTERED 2026-11-12, written before any code of F-465 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-465 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script, no prototype and no library
code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-465 (deps F-463), follow-up of [ADR 0361](0361-f463-registration-failing-solve-in-a-later-iteration.md) (its
remaining question 1, option (b)). Branch `f465-failing-fluid-later-iteration` from main a82347ab, worktree `w49i-work/f465`,
scratch `w49i-work/f465-scratch`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded; a defective set-up gets a corrected test by an amendment
  written and committed before the corrected run; each registered run is made once;
- tests only: one new file `crates/rcfd-post/tests/sb_solid_coupling_fail_fluid.rs`. `sb_solid_coupling.rs` (ADR 0286) and
  `sb_solid_coupling_fail.rs` (ADR 0361) are not edited; the set-up of the latter is copied. No library file, shared solver
  file, golden or hash table is touched, so every default result keeps its bits by construction. A library defect found is
  reported, not patched. The library mutations below are run on a scratch copy only;
- no new dependency, no unsafe code; clean-room: the argument is the fixed-point arithmetic of ADR 0361, reused, no paper.

## What F-465 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. With the StaticSolid and the mock fluid of ADR 0286 and
> the start of ADR 0361 (a converged step, then F0 doubled), a mock fluid whose advance returns an error in iteration 2, and in
> iteration 3, stops the step with the fluid's name, that iteration number and its message, and both participants bit-equal to
> the start of the step; mutations that skip the coupler's restore of the fluid or of the structure at the error, or at the
> start of an iteration, are each caught by at least one check (the pair of restores that ADR 0361 M9 and M11 could not
> observe).

Clauses: (c1) every limit not stated is registered here, before the run; (c2) the set-up is the StaticSolid and the mock fluid
of ADR 0286 with the start of ADR 0361; (c3) the fluid fails in iteration 2: the error names the fluid, iteration 2 and carries
the mock's message, and both participants are bit-equal to the start of the step; (c4) the same for iteration 3; (c5) mutations
that skip the coupler's restore of the fluid or of the structure at the error, or at the start of an iteration, are each caught
by at least one check.

## What was read (main a82347ab, read only)

1. `AGENT_RULES.md`; [ADR 0361](0361-f463-registration-failing-solve-in-a-later-iteration.md) in full (arithmetic, design,
   checks, mutations M9 and M11, addendum 1); `crates/rcfd-post/tests/sb_solid_coupling_fail.rs` in full (the set-up copied).
2. `crates/rcfd-fv/src/cosim.rs` lines 338 to 360 (`CouplingError`) and 559 to 700 (`coupled_step`).
3. **Not read:** `solid_coupling.rs` again (ADR 0361's reading of it is relied on: `StaticSolid::advance` and `restore`),
   `cosim/mapping.rs`, `FlowWall`.

Findings from `coupled_step` that shape the design:

1. **Order inside an iteration `k`**: for `k > 1` the fluid is restored, then the structure; then `structure.advance`, the
   structure's time check, `fluid.advance`, the fluid's time check, the update. A fluid `advance` that returns `Err` is wrapped
   as `fail(fluid_name, k, message)`: participant the fluid's name, iteration `k`, the message unchanged, time `t0`.
2. **At that moment both participants are off the checkpoint.** The structure has advanced `k` times in the step and the
   fluid, if it fails after taking its new state (a mesh moved, a solver diverged after the wall displacement was set), is
   advanced too. The only restore that follows is the error arm of `match run()`, which restores the fluid and then the
   structure and appends `; and the restore failed: ...` to the message if one of them fails. M9 and M11 of ADR 0361 are thus
   observable here: with either restore skipped, that participant's bits stay at the failed iteration.
3. **The restore at the start of iteration `k > 1`** is observable through the iterations that follow it: without the fluid's
   restore, the fluid's time after iteration 2's advance is `t0 + 2 dt` and the coupler stops in iteration 2 with the fluid's
   name (`at_end`); without the structure's, the structure stops iteration 2 with its name. A failing fluid with the call
   count as the only trigger would not show a skipped restore in iteration 2 (it fails anyway, and the error arm restores);
   so the mock reports in its message the time at which the failing advance was entered (design below).

## Arithmetic before this registration (disclosed, by hand)

The trajectory is that of ADR 0361's arithmetic, which is reused as is because the fluid fails in call `n` only, and calls `1`
to `n - 1` are those of the healthy mock. In units of `F0` (`f` the nodal total force, `a = k_f/k_s = 1.5`, `g(f) = 2 - a f`):
the step starts at `f1 = 1.4`; iteration 1 gives `g(1.4) = -0.1`, residual `-1.5`, test quantity `|r|/|ft| = 15`; update to
`f2 = 0.65`; iteration 2 gives `g = 1.025`, residual `0.375`, test quantity `0.36585...`; update (Aitken `w = 0.4`, least
squares `f3 = 0.8`) to `f3 = 0.8`; iteration 3 gives `g = 0.8`, residual about `1e-16`, converged. (ADR 0361 measured 15 to
7e-14, 0.3658536585365908, 5.8e-16.)

**Reachability of iterations 2 and 3 with a failing fluid.** The fluid's `advance` in iteration `n` runs after
`structure.advance` of iteration `n` and before the test quantity of iteration `n` is formed.

- Failure in the fluid's 2nd advance: iteration 1 is complete (its test quantity 15 exceeds the tolerance 1e-10, no
  convergence, cap 20 not reached), so the loop enters `k = 2`, restores both participants, advances the structure under
  `f2 = 0.65` (a healthy solve, the same solid as the healthy step), and then calls the fluid for the 2nd time. Reached.
- Failure in the fluid's 3rd advance: iterations 1 and 2 are complete; the test quantities 15 and 0.366 exceed 1e-10; the loop
  enters `k = 3`, restores both, advances the structure under `f3 = 0.8` and calls the fluid for the 3rd time. Reached. The
  healthy step converges in exactly this iteration (residual about 1e-16), so iteration 3 exists and a 4th does not (check
  N1 with the fluid failing at its 4th call).
- The structure advances `n` times and the fluid `n` times in each failing step (`n` fluid calls, the last of them failing).

The control runs P1 (Aitken) and P4 (least squares) on the healthy mock from the same start must show `iterations == 3`, the
test quantities 15, 0.36585 (relative 1e-6) and `<= 1e-10` for the third, and 3 advances of the structure and of the fluid. If
the control does not, the reachability fails and it is reported as such, with no change of limits.

## Design

One new file `crates/rcfd-post/tests/sb_solid_coupling_fail_fluid.rs`. Copied unchanged in meaning from
`sb_solid_coupling_fail.rs`: the constants, `bar`, `hex_bar`, `wall`, `face_area`, `mean`, `map_for`, `solid`, `solid_bits`,
`LinearWall` (with `bits`), `setup`, `with_tolerance`, `start`, `bits_of`, `residual_bits`, `Checks`. The wrapper `FailAt` of
ADR 0361 is replaced by a transparent counting form of the structure wrapper that counts the structure's advances and records
the solid's bits after each advance (`last_after`). New, the fluid wrapper

```text
struct FailFluid { inner: LinearWall, fail_at: usize, calls: usize, at_failure: Option<(u64, Vec<u64>)> }
```

delegating `Participant` and `Fluid` to `inner`. Its `advance` increments `calls`; when `calls == fail_at` it takes the
entry time `t_in = inner.t`, then takes the new state as the healthy mock does (`inner.d = displacements`, `inner.t += dt`),
records `at_failure = inner.bits()` and returns `Err(format!("linear wall: injected failure at advance {n}, entered at t =
{t_in:?}"))` with `{:?}` of the f64 (which round-trips). A fluid that fails after its state moved is the case that needs
the coupler's restore; it is chosen on purpose and is what makes M9 and M11 observable. `fail_at = usize::MAX` is a
transparent wrapper.

`control(settings)` returns the healthy step as an `Option` (no panic when a mutation breaks the healthy step), so that the
failing-step checks are evaluated even when the control check fails; the control's own numbers are asserted in the tests P1
and P4 only. Every test runs on clones of the same `start(settings)`.

## Checks

The update is Aitken(0.5) for P1, F2, F3, N1 and least squares(0.5) for P4, G2, G3 (as ADR 0361). `n` is 2 or 3.

| Check | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| P1a | c2 control | healthy step, Aitken: converged, `iterations` | converged, `== 3` | 3 |
| P1b | c2 control | test quantity 1 relative to 15 | `<= 1e-6` | `~1e-13` |
| P1c | c2 control | test quantity 2 relative to `0.375/1.025` | `<= 1e-6` | `~1e-14` |
| P1d | c2 control | test quantity 3 | `<= 1e-10` | `~1e-15` |
| P1e | c2 control | structure advances and fluid advances in the step | both `== 3` | 3, 3 |
| P1f | c2 control | the step changes the fluid bits and the solid bits relative to the start | both differ | differ |
| P4a to P4f | c2 control | as P1 with least squares | as P1 | as P1 |
| F2a, F3a, G2a, G3a | c3, c4 | the step is `Err`; `participant == "fluid"` and `iteration == n` | exactly | as stated |
| F2b, F3b, G2b, G3b | c3, c4 | the error's message equals `linear wall: injected failure at advance n, entered at t = {t0:?}` exactly (so the mock's text is carried, the fluid entered the failing advance at the start time, and no `; and the restore failed` is appended) | string equal | equal |
| F2c, F3c, G2c, G3c | c3, c4 | `error.time` bit-equal to `t0` | 0 bits | 0 |
| F2d, F3d, G2d, G3d | c3, c4 | the structure advanced exactly `n` times and the fluid exactly `n` times | `== n`, `== n` | as stated |
| F2e, F3e, G2e, G3e | c3, c4 | fluid bits (time, wall displacement) and solid bits (time, displacement, interface forces) bit-equal to the start of the step | 0 bits differ in each | 0 |
| F2f, F3f, G2f, G3f | c3, c4 control | the states at the failure were off the start: `at_failure` differs from the fluid's start bits and the solid's bits after its last advance differ from the solid's start bits (so e is not vacuous) | both differ | differ |
| F2g, F3g, G2g, G3g | extension | the same step run again with the fluid healthy: converged, `iterations == 3`, `node_displacements` and `residuals` bit-equal to the control's | 0 bits | 0 |
| N1 | control | fluid wrapper with `fail_at = 4`: `Ok`, converged, `iterations == 3`, structure and fluid advanced 3 times, `node_displacements` bit-equal to the control's | as stated | as stated |

Negative controls: P1, P4 (iterations 2 and 3 are reached; both states change before a failure), f (the states are dirty at
the failure), N1 (the failure is caused by the switch alone; there is no 4th advance).

### Planned mutations (c5)

Library mutations are made on a scratch copy of the final tree in `w49i-work/f465-scratch/mut` (not in the worktree), by a
script that applies one textual change, builds and runs `cargo test --release -p rcfd-post --test
sb_solid_coupling_fail_fluid -- --test-threads=1`; each is run once. Test-side mutations are made on the same copy in the new
file. Line numbers are those of `crates/rcfd-fv/src/cosim.rs` at a82347ab.

| Id | Where | Change | Expected to be caught by |
|---|---|---|---|
| M9 | `coupled_step`, error arm (l. 707 to 717) | the structure's restore skipped | F2e, F3e, G2e, G3e (solid bits) |
| M11 | same | the fluid's restore skipped | F2e, F3e, G2e, G3e (fluid bits) |
| M15 | same | both skipped | F2e, F3e, G2e, G3e (both) |
| M16 | `k > 1` block (l. 641 to 645) | the fluid's restore skipped at every `k > 1` | F2b (entry time `t0 + dt`), F3a (stops in iteration 2 by the fluid's time check), P1 |
| M17 | same | the structure's restore skipped at every `k > 1` | F2a, F3a, G2a, G3a (the solid stops iteration 2: participant `solid`), P1 |
| M18 | same | the fluid's restore skipped only for `k > 2` | F3b (entry time `t0 + dt` in iteration 3), G3b, P1; **F2 and G2 cannot see it** (iteration 2 is restored) |
| M19 | same | the structure's restore skipped only for `k > 2` | F3a, G3a (the solid stops iteration 3), P1; **F2 and G2 cannot see it** |
| M20 | fluid `advance` error (l. 654) | iteration reported `k + 1` | F2a, F3a, G2a, G3a |
| M21 | same | iteration reported `k - 1` | F2a, F3a, G2a, G3a |
| M22 | same | participant `"coupler"` instead of the fluid's name | F2a, F3a, G2a, G3a |
| M23 | same | the message dropped (`String::new()`) | F2b, F3b, G2b, G3b |
| M24 | mock `LinearWall::restore` (new file) | the wall displacement not restored | F2e, F2g, F3e, F3g, G2e, G2g, G3e, G3g |
| M25 | mock `LinearWall::restore` (new file) | the time not restored | P1 (the healthy step stops on the time check), F2e, F3e, G2e, G3e |
| M26 | `FailFluid` (new file) | switches on `calls == fail_at - 1` | F2a, F2d, F3a, F3d, G2a, G2d, G3a, G3d |

M9, M11, M15, M16, M17, M18, M19 are the mutations the acceptance names (the coupler's restores of the fluid and of the
structure at the error, and at the start of an iteration; M18 and M19 separate the start of iteration 3 from that of
iteration 2). All are registered as "to be caught"; none is registered as an equivalent survivor. If a library mutation turns
out equivalent on inspection after the run, it is reported as a survivor (Rule I: partial), not explained away.

## Decision rule

**done**: every check above (P1, P4, F2, F3, G2, G3 with a to g, N1) met on its registered run and every mutation M9 to M26
caught by at least one of the checks named for it in the table (a catch by P1 or P4 alone counts only for M25 and where the
table names P1 together with another check; for the others a failing-step check must catch it). **partial** (Rule I): a missed
or unrun check, a registered mutation that survives or is caught only by a check other than the registered ones, a mutation
caught only after a post-run amendment, or a control (P1, P4) that does not reach iteration 3, since then the iteration-3
clause is not shown reachable. A miss of the extension (g) is a `*_known_miss` record and alone does not block done unless its
diagnosis shows that a clause check rests on a defect. **todo**: the test file does not build.

## Limits (what is not covered)

- Only a mock fluid failing by a returned `Err` after taking its new state, in its 2nd and 3rd advance, with Aitken and least
  squares. Not covered: a fluid that fails before touching its state (the error arm's fluid restore is then redundant for
  `n >= 2`), the fluid's time check, a non-finite or wrongly sized force array from the fluid, a failing map, a failure of
  `restore` itself (the appended `; and the restore failed` text), a failure in iteration `n > 3`, the at-cap error.
- Only the mock fluid of ADR 0286 (linear, scalar iteration); no flow solver, `FlowWall` is not exercised.
- The bit comparison covers fluid time and wall displacement and solid time, displacement, interface forces; the checkpoint
  slots and `f0` are not compared.
- The message check compares the mock's own text written in this file, not the text of a library error.

## Addendum 1: results (2026-11-12)

Registered run: commit 2dcd4bd3 (the test file; an earlier commit a84fde78 of the same file differed only by a type alias added for clippy's `type_complexity`, no check or limit changed, both before any run), `cargo test --release -p rcfd-post --test sb_solid_coupling_fail_fluid -- --nocapture --test-threads=1`, log `w49i-work/f465-scratch/registered_run.log`, run once: 5 passed, 0.10 s. The later all-targets `cargo test --release -p rcfd-post` ran this file a second time with the same result; nothing was picked from it. Release profile throughout.

### Checks

| Check | Limit | Measured | Met |
|---|---|---|---|
| P1a healthy step, Aitken | converged, `== 3` | converged, 3 | yes |
| P1b test quantity 1 against 15 | `<= 1e-6` relative | 7.0e-14 | yes |
| P1c test quantity 2 against 0.375/1.025 | `<= 1e-6` relative | 1.5e-14 | yes |
| P1d test quantity 3 | `<= 1e-10` | 5.8e-16 | yes |
| P1e advances of structure and fluid | both 3 | 3, 3 | yes |
| P1f both states change | both differ | differ | yes |
| P4a to P4f (least squares) | as P1 | identical numbers to P1 | yes |
| F2a, F3a, G2a, G3a participant `fluid`, iteration n | exactly | as stated | yes |
| F2b, F3b, G2b, G3b message equals the mock's text with entry time `t0` | string equal | equal, no appended restore text | yes |
| F2c, F3c, G2c, G3c error time | 0 bits | 0 | yes |
| F2d, F3d, G2d, G3d structure and fluid advanced n times | `== n` each | n, n | yes |
| F2e, F3e, G2e, G3e both states bit-equal to the start | 0 bits | 0 | yes |
| F2f, F3f, G2f, G3f states were off the start at the failure | both differ | differ | yes |
| F2g, F3g, G2g, G3g retry reproduces the control | 0 bits | 0 | yes |
| N1 failure at the 4th fluid advance never reached | Ok, 3 iterations, 3 advances each, bit-equal | as stated | yes |

The reachability argument holds as measured (15, 0.366, 5.8e-16, three advances each); the Aitken and least-squares trajectories coincide bit for bit, as in ADR 0361. Example failing error: `co-simulation: 'fluid' at t = 0.001, iteration 3: linear wall: injected failure at advance 3, entered at t = 0.001`.

### Mutations (scratch copy `w49i-work/f465-scratch/mut` of 2dcd4bd3, script `mutate.py`, log `mutations.log`, each run once)

| Id | Change | Result |
|---|---|---|
| M9 | error arm: structure restore skipped | caught: F2e, F3e, G2e, G3e (and F2g to G3g) |
| M11 | error arm: fluid restore skipped | caught: F2e, F3e, G2e, G3e (and F2g to G3g) |
| M15 | error arm: both skipped | caught: F2e, F3e, G2e, G3e (and F2g to G3g) |
| M16 | fluid restore skipped at every `k > 1` | caught only by a panic in the set-up (see below): all 5 tests fail in `start()`; F2b and F3a were not reached |
| M17 | structure restore skipped at every `k > 1` | same, set-up panic (`solid`, iteration 2) |
| M18 | fluid restore skipped for `k > 2` | same, set-up panic (`fluid`, iteration 3) |
| M19 | structure restore skipped for `k > 2` | same, set-up panic (`solid`, iteration 3) |
| M20 | fluid error iteration `k + 1` | caught: F2a, F3a, G2a, G3a |
| M21 | iteration `k - 1` | caught: F2a, F3a, G2a, G3a |
| M22 | participant `coupler` | caught: F2a, F3a, G2a, G3a |
| M23 | message dropped | caught: F2b, F3b, G2b, G3b |
| M24 | mock restore without the wall displacement | caught: F2e, F2g, F3e, F3g, G2e, G2g, G3e, G3g |
| M25 | mock restore without the time | set-up panic (the converged first step stops on the time check), as M16 |
| M26 | wrapper switches one call early | caught: F2a, F2d, F3a, F3d, G2a, G2d, G3a, G3d |

**Defect of the set-up found by the run.** The start state (`start()`) is itself a converged coupled step of three iterations, so it uses the coupler's restores at the start of iterations 2 and 3. Any mutation of those restores (M16 to M19) or of the mock's restore (M25) makes `start()` fail with an `unwrap` panic in every test, before P1 or any failing-step check runs. These five mutations are therefore detected (every test of the file fails) but not by the checks registered for them (F2b, F3a, G3b, P1 and so on). The registration expected the failing-step checks to see them; they cannot. By Rule I this is a registered mutation not caught by its registered check: **the status is partial**. The limits are not changed. The mutations that the acceptance names for the error arm (M9, M11, M15) are caught by the registered checks.

### Other checks run

- `cargo test --release -p rcfd-post`, all targets, existing tests unedited: every suite passed (log `post_all.log`); `sb_solid_coupling` 11 passed, 1 ignored; `sb_solid_coupling_fail` 3 passed; the new file 5 passed.
- `cargo fmt -p rcfd-post --check`: clean. `cargo clippy -p rcfd-post --all-targets --release -- -D warnings`: clean.
- Defaults: tests only; no library, shared solver file, golden or hash table touched; `baseline_hashes` not run (rule 3).

### Not run

A fluid failing before it takes its state, the fluid's time check, wrong-size or non-finite fluid forces, a failing `restore`, iterations beyond 3, a flow solver. A corrected set-up for M16 to M19 and M25 (below).

### Decision by the registered rule

**partial.** All checks met and M9, M11, M15, M20 to M24, M26 caught by registered checks, but M16, M17, M18, M19 (the start-of-iteration restores, a clause of the acceptance) and M25 were caught only by the set-up panic, not by a registered check.

### Remaining questions

1. **Isolate the set-up from the mutations of the start-of-iteration restores.** Options: (a) leave it, accepting that a test-file-wide failure is a detection; (b) a follow-up item whose start state is built without a restore at `k > 1` (for example a loose-coupling first step, `max_iterations = 1` with `AtCap::Keep`, so that the coupler's iterations 2 and 3 first occur in the checked step), with its arithmetic registered beforehand, so that F2b, F3a, G3a, G3b and P1 are the catching checks. Recommendation: (b), a small item; (a) does not meet the acceptance as registered. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): listed as a follow-up item below.
2. **M9, M11 and M15 are also caught by g.** The registered catchers (e) already catch them. No action.

### For the lead: roadmap entries

F-465: proposed status **partial**. Note:

> Structural solver, basic tier. ADR 0364, 2026-11-12: tests only, crates/rcfd-post/tests/sb_solid_coupling_fail_fluid.rs. With the StaticSolid and the mock fluid of ADR 0286 and the start of ADR 0361 (a converged step, then F0 doubled), a mock fluid whose advance takes its new state and then returns an error in its 2nd and its 3rd call (Aitken and least squares) stops the step with participant 'fluid', that iteration number, the mock's message (entered at the start time) and the start time, after n advances of the structure and of the fluid; fluid and solid state (time, displacement, forces) bit-equal to the start of the step, with a check that both were off the start at the failure; the retry with a healthy fluid reproduces the healthy step bit for bit; the control reaches iteration 3 as derived (15 to 7e-14, 0.36585 to 1.5e-14, 5.8e-16) and a failure at the 4th call is never reached. The coupler's restores at the error, the equivalent mutants M9 and M11 of ADR 0361, are caught: skipping the structure's, the fluid's or both fails the bit-equality checks; a wrong iteration, participant or message and a wrong mock restore are caught by the registered checks. Partial: the mutations that skip the restores at the start of an iteration (fluid or structure, from iteration 2 or from iteration 3) and a mock restore without the time were caught only by a panic in the set-up, which is itself a converged 3-iteration step that uses those restores, not by the registered failing-step checks. Not covered: a fluid failing before it takes its state, the fluid's time check, bad force arrays, a failing restore, iterations beyond 3, a flow solver.

Follow-up item (id by the lead):

1. Title: "Coupled step: failing-participant checks with a start state that does not use the restores at the start of an iteration". Deps: F-465.

> Every limit not stated here is registered in the ADR before the run. With the StaticSolid and the mock fluid of ADR 0286, a start state reached without any coupler iteration beyond the first (a loose-coupling first step, then F0 doubled), registered with its arithmetic, a mock fluid failing in iteration 2 and in iteration 3 stops the step with the fluid's name, the iteration number and its message and both participants bit-equal to the start; mutations that skip the coupler's restore of the fluid or of the structure at the start of iteration 2 or of iteration 3 (ADR 0364 M16 to M19) and the mock's restore of the time (M25) are each caught by a failing-step or control check, not by a panic in the set-up.
