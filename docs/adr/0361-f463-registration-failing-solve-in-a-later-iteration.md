# ADR 0361: F-463 registration: the failing static solve in iteration 2 and in iteration 3 of a coupled step

Status: **REGISTERED 2026-11-12, written before any code of F-463 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-463 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script, no prototype and no library
code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-463 (deps F-385), follow-up 1 of [ADR 0286](0286-in-process-loads-and-static-structural-participant.md) (its
remaining question 1, option (b)). Branch `f463-failing-solve-later-iteration` from main fd9c968e, worktree `w49i-work/f463`,
scratch `w49i-work/f463-scratch`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run; each registered run is made once;
- tests only: one new file `crates/rcfd-post/tests/sb_solid_coupling_fail.rs`. `sb_solid_coupling.rs` (ADR 0286) is not edited;
  its mock fluid, wrapper and helpers are copied into the new file. No library file, no shared solver file, no golden, no hash
  table is touched, so every default result keeps its bits by construction; a library defect found is reported, not patched;
- no new dependency, no unsafe code, the crate graph stays acyclic;
- clean-room: the argument is the fixed-point arithmetic of two springs in parallel, derived below; no paper is used.

## What F-463 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run, which discloses that ADR 0286 ran the test once after its
> amendment 1. With the StaticSolid and the mock fluid of ADR 0286, a coupled step that starts away from its fixed point (the
> mock's F0 doubled after a converged step) and whose static solve fails in iteration 2 stops with the participant's name and
> iteration 2, the solver's message, and both participants bit-equal to the start of the step; a step whose solve fails in
> iteration 3 does the same.

Clauses: (c1) every limit not stated is registered here, before the run; (c2) disclosure that ADR 0286 ran the test once after
its amendment 1; (c3) the set-up is the StaticSolid and the mock fluid of ADR 0286, a converged step, then F0 doubled; (c4)
failure in iteration 2: the error names the participant and iteration 2, carries the solver's message, and both participants
are bit-equal to the start of the step; (c5) the same for a failure in iteration 3.

### Disclosure (c2)

ADR 0286 registered a check F2 (failing solve in iteration 2, after a converged step). On its registered run (commit 6f7122f7)
F2 missed because the step started at its fixed point and converged in iteration 1. Amendment 1 of ADR 0286 registered the
corrected test F2b (F0 doubled between the converged step and the failing one) and ran it once (commit 6b0008b7); it met its
checks. F2b stays in `sb_solid_coupling.rs` and is not edited. **This registration therefore does not run a test for the first
time**: the iteration-2 case below is a second, independent run of F2b's scenario in a new file, with more checks (the error's
time, the number of advances, a retry, an explicit control that the step does reach iteration 2, and the least-squares
update). The iteration-3 case has not been run before. ADR 0286's follow-up text asked for the check to be registered in an ADR
before the run; that is this ADR.

## What was read (main fd9c968e, read only)

1. `AGENT_RULES.md`; [ADR 0286](0286-in-process-loads-and-static-structural-participant.md) in full (design, arithmetic 8, checks
   C1 to C3 and F1 to F3, amendment 1, addendum 1, its roadmap text and the follow-up); the head of
   [ADR 0358](0358-f461-registration-hex8-patch-test-on-moved-mesh.md) (the style followed here).
2. Code: `crates/rcfd-post/src/solid_coupling.rs` in full (`StaticSolid::advance` solves first and only then updates its state;
   `checkpoint`, `restore`, `name`); `crates/rcfd-post/tests/sb_solid_coupling.rs` in full (the mock fluid `LinearWall`, `wall`,
   `setup`, `FailSecond`, `failing`, `solid_bits`, F1, F2, F2b); `crates/rcfd-fv/src/cosim.rs` lines 240 to 718 (`AtCap`,
   `CouplingSettings`, `CouplingError`, `SliceRelaxer` (Aitken), `LeastSquaresUpdate`, `coupled_step`).
3. **Not read:** `cosim/mapping.rs` beyond what ADR 0286 states; `rcfd-fem` solver code (the solver's message text is taken from
   the measurement recorded in ADR 0286 addendum 1, check F1: `solver cg-jacobi: not converged in 1 iterations`).

Findings that shape the design:

1. **One structure advance per iteration.** In `coupled_step` iteration `k` calls `structure.advance` exactly once, after (for
   `k > 1`) `fluid.restore()` and `structure.restore()`. The count of the structure's `advance` calls therefore equals the
   iteration number, and a wrapper that switches the solver to a failing one on its `n`-th call fails the solve in iteration `n`.
2. **The failing advance leaves the state unchanged** (`StaticSolid::advance` updates time, displacement and forces only after a
   successful solve), and in iteration `k > 1` both participants are restored before it. The coupler's own restore at the error
   (`match run()` arm) is thus redundant for a structure failure: both participants are already at the checkpoint when the solve
   fails. Consequence for the mutations: skipping only that final restore is not observable here (see M9 and M11).
3. **The error carries** `participant`, `time` (the start of the step), `iteration` and `message`; for a structure failure the
   message is `static solid 'solid': <FemError text>`.

## Arithmetic before this registration (disclosed, by hand)

Set-up, as ADR 0286 arithmetic 8 and F2b: bar `hex_bar()`, `k_s = E A/L = 5e10 N/m`, mock `LinearWall` with `k_f = 1.5 k_s =
7.5e10 N/m`, `F0 = 1e6 N`, `dt = 1e-3 s`, Aitken(0.5) with tolerance 1e-10, cap 20, stop at the cap. The first step converges to
`u* = F0/(k_s + k_f) = 8e-6 m` (ADR 0286: 3 iterations, error 1.9e-15). Then `F0' = 2 F0`. Every force is proportional to the
uniform end load, so the iteration is scalar. Write forces as totals in units of `F0` (`f`), the mock's output `g(f) = 2 - a f` with
`a = k_f/k_s = 1.5`, the solid's displacement `u = f F0/k_s`, and the residual `r = g(f) - f`.

1. **The start.** The mock's state still holds the wall displacement `u*` (the restore of ADR 0286 returns the end of step 1), so
   its force at the start of the next step is `2 - a (k_s u*/F0)`, with `k_s u* = F0/(1 + a) = 0.4 F0`: `f1 = 2 - 0.6 = 1.4`.
2. **Iteration 1.** The solid under `f1 = 1.4` moves `u = 1.4 F0/k_s = 2.8e-5 m`; the mock outputs `g(1.4) = 2 - 2.1 = -0.1`.
   `r1 = -0.1 - 1.4 = -1.5`. Test quantity `|r|/|ft| = 1.5/0.1 = 15`. Not converged.
3. **Update 1** (Aitken and least squares alike: the first update is the relaxed one with the initial factor 0.5): `f2 = 1.4 +
   0.5 (-1.5) = 0.65`.
4. **Iteration 2.** `g(0.65) = 2 - 0.975 = 1.025`; `r2 = 1.025 - 0.65 = 0.375`. Test quantity `0.375/1.025 = 0.36585...`. Not
   converged (tolerance 1e-10).
5. **Update 2.** Aitken (`SliceRelaxer::factor`): `w = -omega (r1 . (r2 - r1))/|r2 - r1|^2 = -0.5 r1/(r2 - r1) = -0.5 (-1.5)/1.875
   = 0.4`, which is `1/(1 + a)`, the secant of a linear map (exact; inside the cap 2). `f3 = 0.65 + 0.4 (0.375) = 0.8`. Least squares
   (one column `v = r2 - r1`, `w = ft2 - ft1`, `c = argmin |v c + r2|`): `next = ft2 - a r2/(1 + a) = 1.025 - 1.5 (0.375)/2.5 =
   0.8`. Both give `f3 = 0.8 = 2/(1 + a)`, the fixed point of `g`.
6. **Iteration 3.** `g(0.8) = 2 - 1.2 = 0.8`; `r3 = 0` up to rounding (about `1e-16` relative, as the third residual 4.4e-16 of
   ADR 0286 C1). Converged in iteration 3 with `u = 0.8 F0/k_s = 1.6e-5 m = 2 u*`, i.e. `F0'/(k_s + k_f)`.

**Reachability of iteration 3.** A step fails in iteration 3 only if iterations 1 and 2 both succeed and the step has not
converged before. After iteration 1 the test quantity is 15 and after iteration 2 it is 0.366, both far above 1e-10, so the
coupler goes on; it first converges at iteration 3 (residual about 1e-16). Iteration 3 is therefore reached, and it is the last
iteration of the healthy step: it exists exactly once, and an iteration 4 would not be reached (check N1). The reachability is
not assumed: the control run P1 on the healthy solid from the same start must show `iterations == 3` with the test quantities of
steps 2, 4 and 6, and the solid's `advance` must have been called three times. If the control does not show it, the iteration-3
check is not reachable by this set-up and the miss is reported as such (no change of limits).

Limits of the numbers: the start of step 1 holds `u*` only to the 1.9e-15 of ADR 0286 C1, so the quantities 15 and 0.36585 are
met to about 1e-14 relative; the limit for them is 1e-6 relative, a generous factor for the rounding of the solves.

## Design

One new file `crates/rcfd-post/tests/sb_solid_coupling_fail.rs`. Copied from `sb_solid_coupling.rs` (and unchanged in meaning):
the constants, `bar`/`hex_bar`, `wall`, `face_area`, `mean`, `map_for`, `solid`, `failing`, `solid_bits`, `LinearWall` (with
`bits`), `setup`, `with_tolerance`, the `Checks` recorder. New: the wrapper

```text
struct FailAt { inner: StaticSolid, fail_at: usize, calls: usize }
```

whose `advance` increments `calls` and, when `calls == fail_at`, sets `inner.options = failing()` (CG with tolerance 1e-14 and a
cap of 1 iteration, as ADR 0286) before delegating. `fail_at = usize::MAX` is a transparent wrapper.

Scenario `start(settings)`: `setup()`, one converged step with `settings` (tolerance 1e-10), `fluid.f0 = 2 F0`, the bits of fluid
(`LinearWall::bits`: time and wall displacement) and solid (`solid_bits`: time, displacement, interface forces) recorded as the
start of the step, `t0 = fluid.time()`. The registered runs are made on clones of this start state so that control and failing
runs begin from identical states.

## Checks

The "update" is Aitken(0.5) for tests A2, A3, P1, N1 and least squares(0.5) for L2, L3, P4 (the acceptance names no update; ADR
0286 used Aitken for F1 and F2b; the least-squares update is registered as well because both were registered for C1 and C2).

| Check | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| P1a | c3 control | healthy step, Aitken, from the start state: converged and `iterations` | converged, `== 3` | 3 (steps 2 to 6) |
| P1b | c3 control | test quantity of iteration 1, relative to 15 | `<= 1e-6` | `~1e-14` |
| P1c | c3 control | test quantity of iteration 2, relative to `0.375/1.025` | `<= 1e-6` | `~1e-14` |
| P1d | c3 control | test quantity of iteration 3 | `<= 1e-10` (the tolerance) | `~1e-16` |
| P1e | c3 control | number of `advance` calls of the solid in the step | `== 3` | 3 |
| P2 | c3 control | largest `\|u_x - 2 u*\|/(2 u*)` over the interface nodes of the control step | `<= 1e-8` | `<= 4e-11` |
| P3 | c4, c5 control | the control step changes the fluid bits and the solid bits relative to the start (so that the bit-equality after a failure is not vacuous) | both differ | differ |
| P4 | c3 control | healthy step, least squares: converged, `iterations == 3`, test quantities 1 and 2 as P1b, P1c | as P1 | as P1 |
| A2a | c4 | failure at the solid's 2nd advance, Aitken: the step is `Err`, `participant == "solid"`, `iteration == 2` | exactly | as stated |
| A2b | c4 | the message contains `static solid 'solid'` and `solver cg-jacobi: not converged in 1 iterations` and does not contain `restore failed` | contains | as stated |
| A2c | c4 | `error.time` bit-equal to `t0` | 0 bits | 0 |
| A2d | c4 | the solid's `advance` was called exactly twice | `== 2` | 2 |
| A2e | c4 | fluid bits and solid bits bit-equal to the start of the step | 0 bits differ | 0 |
| A2f | extension | after the failure, the solid's options set back to the default and the same step run again: converged, `iterations == 3`, `node_displacements` and `residuals` bit-equal to the control's | 0 bits | 0 |
| A3a to A3f | c5 | as A2a to A2f for a failure at the 3rd advance: `iteration == 3`, `calls == 3` | as A2 | as A2 |
| L2a to L2f, L3a to L3f | c4, c5 | as A2 and A3 with the least-squares update (control P4 for A2f's comparison) | as A2, A3 | as A2, A3 |
| N1 | control | wrapper with `fail_at = 4` (never reached), Aitken: `Ok`, converged, `iterations == 3`, `calls == 3`, `node_displacements` bit-equal to the control's | as stated | as stated |

Negative controls: P1 to P4 (the step really reaches iterations 2 and 3, and changes both states before a failure), N1 (the
failure is caused by the switch and by nothing else; a 4th advance does not exist). The wrapper is transparent when not
switched, which N1 and A2f/A3f show.

### Planned mutations (scratch copy of the final tree, each run once; `cargo test --release -p rcfd-post --test
sb_solid_coupling_fail`)

| Id | Where | Change | Expected to be caught by |
|---|---|---|---|
| M1 | `StaticSolid::restore` | the displacement not restored | A2e, A3e, L2e, L3e (solid bits) |
| M2 | `StaticSolid::restore` | the interface forces not restored | A2e, A3e, L2e, L3e (solid bits) |
| M3 | `StaticSolid::restore` | the time not restored | A2e, A3e, L2e, L3e; also P1 (time check of the healthy step) |
| M4 | `StaticSolid` `Participant::name` | returns `""` (participant name dropped) | A2a, A3a, L2a, L3a, A2b, A3b |
| M5 | `StaticSolid::advance` | the solver's text dropped from the error (`fail("solve failed")`) | A2b, A3b, L2b, L3b |
| M6 | `coupled_step`, error of the structure's `advance` | iteration reported as `k + 1` | A2a, A3a, L2a, L3a |
| M7 | `coupled_step`, same place | iteration reported as `k - 1` | A2a, A3a, L2a, L3a |
| M8 | `coupled_step` | the fluid not restored at `k > 1` nor at the error (both calls removed) | A2e, A3e, L2e, L3e (fluid bits); P1 (time check) |
| M9 | `coupled_step`, error arm only | the structure's restore at the error skipped | **expected to survive** (finding 2: already restored before the failing advance) |
| M10 | `coupled_step`, `k > 1` | the structure's restore skipped | P1, A3, L3, N1 (the solid then reports the time `t0 + 2 dt` after its 2nd advance, and the step stops in iteration 2 with `solid`); **expected to survive A2 and L2** (finding 2) |
| M11 | `coupled_step`, error arm only | the fluid's restore at the error skipped | **expected to survive** (already restored at `k > 1`) |
| M12 | test mock `LinearWall::restore` (new file) | the wall displacement not restored | A2e, A3e, L2e, L3e (fluid bits) |
| M13 | test mock `LinearWall::restore` (new file) | the time not restored | P1 (time check), A2e, A3e, L2e, L3e |
| M14 | test wrapper `FailAt` (new file) | switches on `calls == fail_at - 1` (off by one) | A2d, A3d, L2d, L3d, A2a, A3a, P1e is not affected |

M9 and M11 are registered as equivalent mutants for this scenario (finding 2): a structure failure never leaves the participants
off the checkpoint, because the restore of iteration `k` precedes the failing advance. They are run to confirm it; surviving is
the registered expectation and is not a miss. They would be caught by a fluid failure, which is outside F-463 (see limits).

## Decision rule

**done**: every check above (P1 to P4, A2a to A2e, A3a to A3e, L2a to L2e, L3a to L3e, N1; the extension A2f, A3f, L2f, L3f
and P1/P2 limits included) met on its registered run, and every mutation not marked "expected to survive" caught by the
checks named for it (M10 caught where named and, for A2 and L2, allowed to survive). **partial** (Rule I): a missed or unrun
check, a mutation caught only after a post-run amendment, a mutation expected to be caught that survives, or a control (P1,
P4) that does not reach iteration 3, since then the iteration-3 clause is not shown reachable. A miss of an extension (A2f,
A3f, L2f, L3f) is a `*_known_miss` record and alone does not block done unless its diagnosis shows that a clause check rests on
a defect. **todo**: the test file does not build.

## Limits (what is not covered)

- Only the failure of the static solve of the structure (a CG capped at one iteration), at the 2nd and 3rd advance, with Aitken
  and with the least-squares update; no failure of the fluid, of the time check or of the map in a later iteration (these would
  exercise the coupler's own restore at the error, M9 and M11, which a structure failure does not).
- Only the mock fluid of ADR 0286 (linear in the wall displacement, a scalar iteration); no flow solver (ADR 0286 c7).
- Only `fail_at` of 2 and 3 (and 4, never reached); iteration `n` for larger `n` follows the same argument but is not run.
- The bit comparison covers the state ADR 0286 compares (fluid: time and wall displacement; solid: time, displacement, interface
  forces); the fluid's checkpoint slot, `f0`, and the solid's checkpoint slot are not compared.
- The solver's message is checked by the substring recorded in ADR 0286 addendum 1, not by a text written independently of the
  library.

## Addendum 1: results (2026-11-12)

Registered run: commit 913947f1 (the test file, committed before the run), `cargo test --release -p rcfd-post --test
sb_solid_coupling_fail -- --nocapture --test-threads=1`, log `w49i-work/f463-scratch/registered_run.log`, run once: 3 passed,
0.12 s. Deviation from "each run once": the later `cargo test --release -p rcfd-post` (all targets, for the house rule) ran
this file a second time with the same result (3 passed); no result was picked from it. Release profile throughout.

### Checks

| Check | Limit | Measured | Met |
|---|---|---|---|
| P1a healthy step, Aitken: converged, iterations | converged, `== 3` | converged, 3 | yes |
| P1b test quantity of iteration 1 against 15 | `<= 1e-6` relative | 14.999999999998948 (7.0e-14) | yes |
| P1c test quantity of iteration 2 against 0.375/1.025 | `<= 1e-6` relative | 0.3658536585365908 (1.5e-14) | yes |
| P1d test quantity of iteration 3 | `<= 1e-10` | 5.8e-16 | yes |
| P1e advances of the solid in the step | `== 3` | 3 | yes |
| P2 largest `|u_x - 2u*|/2u*` | `<= 1e-8` | 1.9e-15 | yes |
| P3 the step changes both states | both differ | both differ | yes |
| P4 (a to e, P2, P3) as P1 with least squares | as P1 | residuals identical to P1's; 3 iterations, 3 advances | yes |
| A2a participant `solid`, iteration 2 | exactly | `solid`, 2 | yes |
| A2b message: solid named, `solver cg-jacobi: not converged in 1 iterations`, no `restore failed` | contains | `static solid 'solid': solver cg-jacobi: not converged in 1 iterations (residual 2.58e5 from 2.44e5, tolerance 1e-14 relative)` | yes |
| A2c error time bit-equal to the start | 0 bits | 0 | yes |
| A2d advances of the solid | `== 2` | 2 | yes |
| A2e fluid and solid bits equal to the start | 0 bits | 0 | yes |
| A2f retry reproduces the control (extension) | 0 bits | 0 | yes |
| A3a to A3f as A2 at the 3rd advance | as A2 (`iteration == 3`, `calls == 3`) | as stated; message residual 3.18e5 from 3.00e5 | yes |
| L2a to L2f, L3a to L3f (least squares) | as A2, A3 | as stated | yes |
| N1 failure at the 4th advance never reached | Ok, 3 iterations, 3 advances, bit-equal to control | as stated | yes |

The reachability argument of the arithmetic holds as measured: test quantities 15, 0.366, 5.8e-16, three advances. The
least-squares and the Aitken steps coincide here (same three residuals bit for bit), as steps 5 and 6 of the arithmetic predict
for a scalar linear iteration.

### Mutations (scratch copy `w49i-work/f463-scratch/mut` of 913947f1, script `mutate.py`, log `mutations.log`, each run once)

| Id | Change | Result |
|---|---|---|
| M1 | restore without the displacement | caught: A2e, A3e, L2e, L3e |
| M2 | restore without the interface forces | caught: A2e, A3e, L2e, L3e |
| M3 | restore without the time | caught by P1 (the healthy step's time check; the control's `unwrap` panics in all three tests, so A2e to L3e are not reached) |
| M4 | `name()` returns `""` | caught: A2a, A3a, L2a, L3a |
| M5 | solver text dropped from the error | caught: A2b, A3b, L2b, L3b |
| M6 | iteration reported `k + 1` | caught: A2a, A3a, L2a, L3a |
| M7 | iteration reported `k - 1` | caught: A2a, A3a, L2a, L3a |
| M8 | fluid not restored at `k > 1` nor at the error | caught by P1 (time check, panic in all three tests) |
| M9 | structure restore at the error skipped | survived, as registered (equivalent: finding 2) |
| M10 | structure restore at `k > 1` skipped | caught by P1 (time check) in all three tests, as registered; A2 and L2 are not reached because of that panic, so the registered allowance of surviving there was not tested |
| M11 | fluid restore at the error skipped | survived, as registered (equivalent) |
| M12 | mock `restore` without the wall displacement | caught: A2e, A2f, A3e, A3f, L2e, L2f, L3e, L3f |
| M13 | mock `restore` without the time | caught by P1 (time check) |
| M14 | wrapper switches one advance early | caught: A2a, A2d, A3a, A3d, L2a, L2d, L3a, L3d, and N1 |

All twelve mutations registered as "caught" were caught by a registered check; the two registered survivors (M9, M11)
survived. M3, M8, M10 and M13 are caught by the control P1 before the failing-step checks run, which is what the
registration named for M8, M10 and M13 and as an additional catcher for M3; the A-e checks for those four were therefore
not shown to catch them on their own (unmeasured, not a miss).

### Other checks run

- `cargo test --release -p rcfd-post`, all targets, existing tests unedited: every suite passed (log `post_all.log`);
  `sb_solid_coupling` 11 passed, 1 ignored (the F2 record), `sb_solid_coupling_fail` 3 passed.
- `cargo fmt -p rcfd-post --check`: clean. `cargo clippy -p rcfd-post --all-targets --release -- -D warnings`: clean.
- Defaults: no library, shared solver file, golden or hash table changed (tests only); `baseline_hashes` not run (rule 3).

### Deviations and what was not run

- The all-targets test run executed the new file a second time (above).
- Not run: a failure of the fluid or of the time check in a later iteration (the coupler's own restore at the error; M9 and
  M11 show that a structure failure does not exercise it), iterations beyond 3, a flow solver.

### Decision by the registered rule

**done.** Every check (P1 to P4, A2, A3, L2, L3, N1, extensions included) met on its registered run, the control reaches
iteration 3 as derived, and every mutation registered as caught was caught; the two registered survivors survived.

### Remaining questions

1. **The coupler's restore at the error is untested by structure failures.** M9 and M11 survive because both participants are
   already restored when the structure's solve fails. Options: (a) leave it; (b) a follow-up item with a failing fluid (a mock
   fluid returning an error in iteration 2 and 3) so that the error arm's restore is observable. Recommendation: (b), a small
   item, since the claim "both restored" then rests on both restore sites.
2. **A2e to L3e for M3, M8, M10, M13.** The control panics first. Options: (a) accept; (b) a follow-up that runs these
   mutations against the failing-step checks alone. Recommendation: (a): P1 is itself a registered check and the mutations are
   caught.
3. **F-385's status.** ADR 0286 is partial by Rule I (a missed registered F2); this item registers and meets the follow-up it
   asked for. Options: (a) leave F-385 partial and note F-463 in its note; (b) mark F-385 done. Recommendation: (a), by the rule
   as written; the lead decides.

### For the lead: roadmap entries

F-463: proposed status **done**. Note:

> Structural solver, basic tier. ADR 0361, 2026-11-12: tests only, crates/rcfd-post/tests/sb_solid_coupling_fail.rs (ADR 0286's test file is not edited; the single run of ADR 0286's corrected F2b after its amendment 1 is disclosed). With the StaticSolid and the mock fluid of ADR 0286 (k_f 1.5 k_s, Aitken and least squares, tolerance 1e-10), a converged step then the mock's F0 doubled: worked out by hand before the run that the healthy step takes exactly 3 iterations (test quantities 15, 0.366, then rounding) from the start f = 1.4 F0, so iteration 3 exists; measured 15 to 7e-14, 0.36585 to 1.5e-14, 5.8e-16, three solid advances, u = 2 u* to 1.9e-15. A wrapper switches the solid to a failing CG (one iteration) at its n-th advance: for n = 2 and n = 3 the step stops with participant 'solid', iteration n, the solver's text (solver cg-jacobi: not converged in 1 iterations), the error's time the start time, n advances, and fluid and solid state (time, displacement, forces) bit-equal to the start of the step; the same step run again with the healthy solver reproduces the control bit for bit. A failure switched at the 4th advance is never reached. Twelve of twelve mutations registered as caught were caught (restore without displacement, forces, time on the solid and on the mock fluid, participant name dropped, solver text dropped, iteration reported k+1 and k-1, coupler restore removed, wrapper off by one); the two registered equivalent mutants (the coupler's restore at the error, redundant for a structure failure) survived. Not covered: a failing fluid or time check in a later iteration, iterations beyond 3, a flow solver (open ADR 0361 Q1).

Other items: the F-385 note may add the sentence "The follow-up F-463 (ADR 0361, 2026-11-12) registers the failing solve in iterations 2 and 3 and meets it." (status unchanged; open ADR 0361 Q3).

Follow-up item (id by the lead):

1. Title: "Coupled step: a failing fluid in a later iteration as a registered check". Deps: F-463.

> Every limit not stated here is registered in the ADR before the run. With the StaticSolid and the mock fluid of ADR 0286 and the start of ADR 0361 (a converged step, then F0 doubled), a mock fluid whose advance returns an error in iteration 2, and in iteration 3, stops the step with the fluid's name, that iteration number and its message, and both participants bit-equal to the start of the step; mutations that skip the coupler's restore of the fluid or of the structure at the error, or at the start of an iteration, are each caught by at least one check (the pair of restores that ADR 0361 M9 and M11 could not observe).
