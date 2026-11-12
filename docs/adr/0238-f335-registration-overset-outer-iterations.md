# ADR 0238: F-335 registration: outer iterations on the overset path (`OversetSolver`, opt-in)

Status: **REGISTERED 2026-11-12**, written before any code of F-335 and before any run of its checks. Not yet accepted by the
maintainer. Before this text nothing was compiled or run: the code and the documents listed under "What was read" were read,
and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Parents: [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (F-134: `OversetSolver`, the model
M1 to M11, the set-ups BF, U, TG_L, TGB_L, the checks Q0, U1, T1, T2, E1 and their records),
[ADR 0232](0232-f314-registration-overset-observed-order-miss.md) (F-314: the diagnosis, hypotheses H6a and H6b, the
measurement `max abs(d_200) = 4.49e-4`, question Q4 whose recommendation (a) is this item),
[ADR 0033](0033-wave20b-dynamic-mesh.md) (the ALE step), [ADR 0129](0129-test-time-limit-jobs5-measurement-limit-audit.md)
(test time limits, `limit_audit.py`), [ADR 0137](0137-linux-gate-platform-values.md) (platform values). Roadmap item F-335
(depends on F-134).

Number: the lead gave 0238 to F-335. Branch `f335-overset-outer-iterations` from main 65c36e60, worktree `w49i-work/f335`.

House rules (ADR 0130, ADR 0213, ADR 0219, ADR 0232):

- limits are fixed here, before the code and the runs, and are not changed after a run; the limits of F-134 and F-314 are
  not changed and their known-miss tests stay as recorded;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- defaults stay bit-identical: a run that does not opt in runs as before, bit for bit (records R1 and R2 below);
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded; no existing
  test is edited;
- each registered run is made once, and nothing is re-run to pick a result (a deterministic run that two tests share is the
  same run);
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. Outer iterations are the fixed-point (Picard) iteration of the linearised convection term within a time step,
wrapped around the pressure correctors of PISO: R. I. Issa, "Solution of the implicitly discretised fluid flow equations by
operator-splitting", J. Comput. Phys. 62 (1986) 40-65 (PISO); J. H. Ferziger and M. Perić, "Computational Methods for
Fluid Dynamics", Springer (outer iterations of implicit pressure-correction methods, the lagged convecting flux);
S. V. Patankar, "Numerical Heat Transfer and Fluid Flow", Hemisphere 1980 (implicit under-relaxation). They are cited from
general knowledge; no paper, book, code or vendor document was opened for this work. What is built is the outer loop that
the library's own `PimpleSolver::step_with` already has (read below), applied to the overset rows of ADR 0219.

## What F-335 asks

The roadmap entry (acceptance, quoted, binding): "OversetSolver accepts n_outer above 1 as an opt-in (defaults
bit-identical, Q0 of ADR 0219 still bit for bit); with three outer iterations the time error of TG_2 at dt 0.0025 (max
abs(d_200) of ADR 0232, 4.49e-4) falls below 1.2e-4 and T2 at level 2 of ADR 0219 holds at the registered step, or the ADR
names why not with a number." Note: "From ADR 0232 question 4, recommendation taken on the maintainer's standing
instruction of 2026-11-11."

**Reading (registered here).**

1. "Accepts `n_outer` above 1 as an opt-in": see "Open choices taken" O1. `OversetSolver::new` keeps its refusal of
   `n_outer != 1` (check E1 of ADR 0219 stays true and its test unedited); a new method of the solver switches the outer
   iterations on.
2. "The time error of TG_2 at `dt = 0.0025`" is `max abs(d_200)` of ADR 0232: the largest difference, over the non-hole
   cells at `t = 0.5`, over the decay `e^{-2 nu t}`, between the velocities of the run of TG_2 in 200 steps and the run in
   800 steps. Here both runs take three outer iterations. ADR 0232 measured `4.4936e-4` with one (largest in a calculated
   component cell; the maximum over all four classes of cells is that number).
3. "T2 at level 2 of ADR 0219 holds at the registered step": `e(TG_2) <= 2 e(TGB_2)` with TG_2 in 200 steps (`dt =
   0.0025`) and three outer iterations. ADR 0219 defines TGB_L as "the background alone with `PimpleSolver::step`, the same
   settings and conditions". Both readings of the reference are checked and both must hold: TGB_2 as recorded (one outer
   iteration, `1.94e-4`) and TGB_2 with the same three outer iterations (O3).
4. T1 of ADR 0219 (order `>= 1.7` at `dt = 0.0025`) is not in the acceptance: ADR 0232 showed that it needs `e(TG_2) <=
   3.32e-4`, below the `3.51e-4` left without any time error. It is printed, not checked.
5. The known-miss tests of ADR 0219 and ADR 0232 are not edited; what holds with the opt-in is shown by new tests.

## What was read (no run)

- ADR 0219 and ADR 0232 in full; the roadmap entries F-134, F-314, F-335.
- `crates/rcfd-fv/src/overset.rs`: `refusal`, `new`, `prepare`, `advance`, `piso`, `overset_pressure`.
- `crates/rcfd-fv/src/pimple.rs`, `PimpleSolver::step_with`, and `pimple/moving.rs`, `step_ale_with_walls`: how the
  library's moving-mesh step takes outer iterations.
- `crates/rcfd-fv/tests/overset_f134.rs`, `overset_f134_records.rs`, `overset_f314.rs` (the run helpers, `difference`,
  `h6_time_error_of_the_moving_cells`, and `sm_run`, which obtained outer iterations for a single moving mesh by setting
  `n_outer = 3` on the settings of a `PimpleSolver`; the settings PS have `alpha_u = alpha_p = 1`, so those iterations were
  not under-relaxed).

What the code read shows:

1. **The library's outer loop** (`step_with`, the subset the overset path supports). Before the loop, once per step: the
   ALE data (`mesh_flux`, the old volumes, the old-level Rhie-Chow defect) and the old velocity of the transient term. In
   every outer iteration: the pressure conditions (`pressure_bc`, the wall gradients), the boundary velocities, the
   Green-Gauss gradient of the current pressure; the convecting flux `phi - m` from the **current** flux `phi` (in the first
   outer iteration the flux of the old time, afterwards the flux of the previous outer iteration, which is a flux through
   the faces at their new position); per component the matrix `convection + laplacian`, `add_ddt`, `fix_ddt` (old level
   and old volume unchanged), the implicit under-relaxation `relax(alpha_u, current u)` on every outer iteration but the
   last, the solve from the current velocity; `a_P`, `rAU`, `gamma_p` from these matrices; the `n_correctors` correctors;
   the pressure update `p_old + a (p_new - p_old)` with `a = alpha_p` in the last corrector of every outer iteration but
   the last and `a = 1` otherwise. With `n_outer = 1` the loop body runs once with `alpha_u = alpha_p = 1`.
2. **The overset step** (`advance`, `piso`). `advance` moves nothing itself (`prepare` did): it installs the new geometry,
   the kinds and stencils of the new pose, refreshes the fringe and hole cells and the faces that do not keep their flux
   from the old values (M9), and calls `piso`, which is one pass of the loop body of item 1 with the overset rows (M5 to
   M8). The pressure matrix with its overset rows, its AMG hierarchy and the response `p1` to a unit offset are built in
   the first corrector and kept for the correctors of the step, because `gamma_p` does not change between them.
3. `refusal` returns "overset: one outer iteration (PISO) only" for `n_outer != 1`, both in `new` and in every `advance`.

## Arithmetic before this registration (disclosed, by hand)

- ADR 0232 SMc: three outer iterations leave `q = 0.153` of the time error of a single moving mesh. Applied to the overset
  value: `0.153 * 4.49e-4 = 6.9e-5`. The limit `1.2e-4` is `0.267` of `4.49e-4`.
- T2 at level 2 needs `e(TG_2) <= 2 * 1.94e-4 = 3.88e-4`. Known with one outer iteration: `6.43e-4` (200 steps), `3.83e-4`
  (400 steps), `3.51e-4` (800 steps). The time error with 400 steps and one outer iteration is about half of `4.49e-4 *
  4/3`, about `3.0e-4`, and leaves `3.83e-4`; a time error of `(4/3) * 6.9e-5 = 9e-5` to `(4/3) * 1.2e-4 = 1.6e-4` at 200
  steps should leave an error between `3.5e-4` and `3.8e-4`. The margin to `3.88e-4` is narrow.
- Cost. ADR 0232: TG_2 in 800 steps took 56 s. With three outer iterations about three times that; the registered runs of
  D below (200 and 800 steps with one and with three outer iterations, two background runs) about 5 minutes.

## Design

**D1. The switch.** `OversetSolver::new` and `refusal` are unchanged for a solver that does not opt in: `n_outer != 1` is
refused in `new` and in `advance`. New method

`OversetSolver::set_outer_iterations(&mut self, n_outer: usize, alpha_u: f64, alpha_p: f64) -> Result<(), String>`

sets `settings.n_outer`, `settings.alpha_u`, `settings.alpha_p` and a private mark that the solver has opted in; from then
on `advance` accepts `settings.n_outer >= 1`. It refuses, leaving the solver unchanged, `n_outer = 0` and a factor outside
`(0, 1]` (or not finite). `advance` on an opted-in solver refuses the same values (the settings are public fields). The
factors are those of the library: they act on every outer iteration but the last; with `n_outer = 1` they are not used.
No field of `PimpleSettings`, no case-schema key, no change in `pimple.rs`, `moving.rs`, `ale.rs`, `rcfd-api`.

**D2. Where the loop sits.** Once per step, before the loop, exactly as now: `prepare` (the motion of the component, the
mesh flux, the follow conditions of the moving walls, the kinds, donors and weights of the new pose) and, in `advance`,
the installation of the new geometry, the ALE data (old volumes, the old-level defect of the kept faces), the old
velocity `u_old`, and the refresh M9 of the fringe cells (velocity and pressure from the old values of the new donors,
plus the last `delta` on the background), the hole cells and the fluxes of the faces that do not keep theirs. The motion
is prescribed, so the pose, the geometry, the kinds, the donors and the weights are those of the new time in every outer
iteration and are not searched again.

The outer loop is inside `piso` and wraps its whole present body. Every outer iteration, with the overset rows of ADR 0219:

1. the pressure conditions and wall gradients, the boundary velocities, the Green-Gauss gradient of the current pressure
   (the fringe cells hold the pressure of the last solve, the background ones with its offset);
2. the convecting flux `phi - m` from the current flux: in the first iteration the old flux as refreshed by M9, afterwards
   the flux of the previous outer iteration on the new geometry (on the faces without a calculated cell the face
   interpolation of M8);
3. the momentum matrices (transient term with `u_old` and the old volumes, unchanged over the iterations), on every
   iteration but the last the implicit under-relaxation with the current velocity, then the rows of the fringe and hole
   cells replaced (M5) and the solve from the current velocity; so the fringe rows interpolate the other part implicitly in
   every outer iteration;
4. `a_P`, `rAU` (interpolated in the fringe cells, `dt` in the hole cells, M6), `gamma_p`;
5. the pressure system of **this** outer iteration: the matrix with its overset rows, its AMG hierarchy and `p1` are
   built anew in the first corrector of each outer iteration (they depend on `gamma_p`) and kept for its correctors; one
   pressure system for the composite mesh, so the pressures of both meshes and the offset `delta` (M7) are solved together
   in every corrector, as now;
6. the correctors: `HbyA` (interpolated in the fringe cells, M6), the pressure solve with `delta`, the corrected flux, the
   velocity, the fringe velocities from their stencils and the hole velocities (M8), the flux of the faces without a
   calculated cell, the pressure update with the factor `alpha_p` in the last corrector of a non-final outer iteration.

The reported `delta` and the flux into the hole region are those of the last corrector of the last outer iteration; the
pressure iterations and the unconverged solves are summed over the outer iterations.

**D3. Why `n_outer = 1` takes exactly the present code path.** The loop body is the present body of `piso`, statement
for statement. With `n_outer = 1` it runs once as the last iteration: the relaxation is skipped by the same condition as
in the library (`alpha_u < 1` only on a non-final iteration), the pressure update factor is the constant 1 as now, and
the pressure system starts empty at the top of the single iteration as it does now at the top of `piso`. No floating-point
operation is added, removed or reordered. A solver that never calls `set_outer_iterations` cannot reach `n_outer > 1`
(refused as now). This is checked, not only argued: Q0 with the hash R1, and the records R2 below, which are taken on the
unchanged library before the library is edited.

**D4. With every cell calculated** the step with `n` outer iterations is the library's `step_ale_with_walls` with the
same settings (check Q0o), including the under-relaxation.

## Order of the work

1. This registration, committed alone.
2. The record test `crates/rcfd-fv/tests/overset_f335_records.rs`, written and run on the library of main 65c36e60 with no
   source change; its hashes are entered as constants and the file is committed with `git diff 65c36e60 -- crates/*/src`
   empty (records R2). This is the only run before the library change, and it is no check of this item's result.
3. The library change and the tests `crates/rcfd-fv/tests/overset_f335.rs`; compiled, formatted and linted without running;
   then every registered check, once.

## Registered checks

Release, Windows. PS, BF, U, TG_L, TGB_L and the error `e` are those of ADR 0219; "three outer iterations" is
`set_outer_iterations(3, 1.0, 1.0)` unless factors are named; "relaxed" is `set_outer_iterations(3, 0.8, 0.5)` (the
factors of `PimpleSettings::pimple`). `d_200(n)` is the difference field of reading 2 with `n` outer iterations in both
runs. Tests that take more than about two minutes are `#[ignore]` with their run time as the reason and are run once.

### B. Defaults (preconditions of landing)

| Id | Check | Limit |
| --- | --- | --- |
| Q0 | the existing test of ADR 0219 Q0, unedited, after the code: the differences and the printed hash | `<= 1e-12` each; equal bit for bit; hash `af2c9e6bd3e5596d` (R1) |
| R2 | records taken on the unchanged library (step 2), after the code, with a solver that does not opt in: the bits of `u`, `p`, `phi`, the time, the step count and every step's `delta` of (a) U, 80 steps; (b) TG_1, 200 steps; (c) TR, the first 20 steps | equal (Windows; printed elsewhere, ADR 0137) |
| R2' | the same three runs after `set_outer_iterations(1, 1.0, 1.0)` | equal to R2 |
| D-S | the existing tests, unedited: `overset_f134` (12 run), `overset_f134_records`, the non-ignored tests of `overset_f314` | pass |

### Q. Equivalence with the library step

| Id | Check | Limit |
| --- | --- | --- |
| Q0o | BF for 20 steps, `advance_mesh` with three outer iterations against `step_ale_with_walls` with `n_outer = 3`: the largest difference of `u`, `p`, `phi` (scaled as Q0); printed: whether equal bit for bit | `<= 1e-12` each |
| Q0r | the same, relaxed on both sides (`alpha_u = 0.8`, `alpha_p = 0.5`) | `<= 1e-12` each |
| Q0n (negative control) | Q0o's library run against the overset run with **one** outer iteration: the largest difference of `u` | `>= 1e-8` (the check can tell one from three) |

### U. Uniform flow

| Id | Check | Limit |
| --- | --- | --- |
| U1o | U with three outer iterations, every step, over the non-hole cells: `max abs(u_c - U) / abs(U)`, `max abs(p_c) / abs(U)^2` | `<= 1e-10` each |
| U3o | the same run: the continuity of the calculated cells; `abs(Q_xmax - U_x A_xmax) / (U_x A_xmax)` | `<= 1e-9`; `<= 1e-10` |
| U1r, U3r | U relaxed: the same four numbers | the same limits |
| N2o (negative control) | U with three outer iterations and every stencil weight multiplied by 1.001 after `prepare`: `max abs(u_c - U) / abs(U)` | `>= 1e-6` |

### D. The acceptance: the time error of TG_2 and T2 at level 2

| Id | Check | Limit |
| --- | --- | --- |
| S1 (set-up, negative control) | `max abs(d_200(1))` over the non-hole cells: the number of ADR 0232, and above the limit of D1 | in `[4.47e-4, 4.52e-4]` |
| D1 (acceptance) | `max abs(d_200(3))` over the non-hole cells | `< 1.2e-4` |
| T2o-a (acceptance) | `e(TG_2)` in 200 steps with three outer iterations over `e(TGB_2)` in 200 steps as recorded by ADR 0219 (one outer iteration) | `<= 2` |
| T2o-b (acceptance) | the same over `e(TGB_2)` in 200 steps with `n_outer = 3` | `<= 2` |
| T3o | every step of the four TG_2 runs of this table: the continuity of the calculated cells | `<= 1e-9` |
| L1 | TG_1 in 200 steps with three outer iterations: `e(TG_1) / e(TGB_1)` (TGB_1 with one outer iteration) and the continuity of every step | `<= 2`; `<= 1e-9` |
| L1r | TG_1 in 200 steps, relaxed: the same two numbers | `<= 2`; `<= 1e-9` |

Printed, no limit: `max abs(d_200)` per class (CF, CC, BF, BC) and the cell of the maximum, with one and with three outer
iterations; `e(TG_2)` of the four runs; `e(TGB_2)` both ways; T1 with three outer iterations (`log2(e(TG_1) / e(TG_2))`,
limit of ADR 0219 `>= 1.7`, not expected to hold, reading 4); the range of `delta`; the pressure iterations per step and
the number of unconverged linear solves of every run; the run times. In a separate slow test, printed only and run once:
`max abs(d_200(2))` and `max abs(d_200(5))` (the trend over the number of outer iterations; no decision rests on them).

### E. Refusals

| Id | Check | Limit |
| --- | --- | --- |
| E1 | the existing test of ADR 0219 E1, unedited (`new` with `n_outer = 2` is `Err`) | pass |
| E4 | `set_outer_iterations` with `n_outer = 0`; with `alpha_u` 0, 1.5, NaN; with `alpha_p` 0, 1.5 | `Err` each; `settings` unchanged |
| E5 | a solver that did not opt in, `settings.n_outer = 3` written directly, then `advance` | `Err`; `u`, `p`, `phi`, time unchanged bit for bit |
| E6 | an opted-in solver with `settings.alpha_u = 0` written directly, then `advance` | `Err`; state unchanged |
| E7 | `set_outer_iterations(2, 1.0, 1.0)` and one step of U | `Ok` |

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final tree by an exact replacement that must occur once, the named
test is run once and must fail; the file is restored and compared byte for byte. The numbers go in the addendum.
`scripts/limit_audit.py` is run on the new test files and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MO1 | `piso`: the convecting flux of every outer iteration is the flux at the start of the step (not re-evaluated) | D1 |
| MO2 | `piso`: the under-relaxation of the momentum matrices is left out | Q0r |
| MO3 | `piso`: the pressure system (matrix, hierarchy, `p1`) of the first outer iteration is kept for the later ones | L1 (continuity) or U3o; which one is reported |
| MO4 | `piso`: the loop runs `n_outer + 1` times | Q0 (hash R1) and R2 |

### Classes

Q0, R2, R2', D-S and E1 are preconditions of landing: a miss means the change is not merged and the cause is recorded. A
miss of any other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A
set-up check or negative control (S1, Q0n, N2o) that does not hold, or a mutation that does not make its check fail, is a
defective set-up: corrected by an amendment before any re-run, never counted as a pass.

## Predictions (mine, before any run)

Q0, R2, R2': equal. Q0o and Q0r: equal bit for bit. U1o, U1r: near `1e-14`. S1: `4.4936e-4`. D1: `7e-5` to `1.0e-4`, met;
the largest value may move from the calculated component cells to the background fringe (`3.76e-4` with one iteration),
whose share that outer iterations remove was not measured. T2o: a ratio between 1.8 and 2.0; it may miss by a few percent
(the arithmetic above). L1: near 1.4. `d_200(2)` between `d_200(1)` and `d_200(3)`; `d_200(5)` within 10 percent of
`d_200(3)` (the remainder is the backward Euler truncation, which outer iterations do not remove). T1: about 1.5, missed.

## Decision rule

- **done**: (a) every registered check is met (D1, T2o-a and T2o-b among them), the preconditions hold, the controls held
  and the mutations failed their checks; or (b) the same except that D1 or T2o (either form) is missed, and each such miss
  has a cause confirmed by a measurement and a deciding number registered in an amendment before that measurement (the
  second branch of the acceptance: "the ADR names why not with a number"); the note then says which limit is missed, by
  how much and why.
- **partial**: the opt-in is built and the preconditions hold, but a miss of D1 or T2o is left without a confirmed cause,
  or another registered check is missed, or a control or mutation did not hold and was not corrected.
- **todo**: the opt-in could not be built, or a precondition does not hold (then nothing is to be merged).

## Open choices taken

Each is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none is a decision the
maintainer made in person, and each is listed again in the addendum. None changes a default.

- **O1. Where the opt-in lives.** (a) `PimpleSettings::n_outer` accepted by `OversetSolver::new`: one knob, but the
  registered check E1 of ADR 0219 asserts that `new` refuses `n_outer = 2`, so an existing test would have to be edited;
  (b) `new` unchanged and a method of the solver that opts in (D1 above): every existing test and record stays unedited.
  Taken: (b). Changing to (a) later is a small edit (the mark dropped, one clause of E1 removed).
- **O2. Under-relaxation.** (a) accept the factors `alpha_u`, `alpha_p` of the library on the non-final outer iterations;
  (b) refuse factors other than 1. Taken: (a): the library's loop has them, the equivalence Q0r checks them against the
  library and U1r, L1r on the overset rows.
- **O3. The reference of T2.** Both readings are checked and both must hold (reading 3).

## Limits of this registration

- The acceptance numbers are measured on the Taylor-Green set-up of ADR 0219 at level 2 only (uniform Cartesian parts, one
  component, `nu = 0.1`, `t = 0.5`, PS: central convection, two correctors, no relaxation). Nothing is concluded for the
  cylinder case TR with outer iterations; TR is run only without the opt-in (R2).
- No convergence control of the outer loop is added (the number of iterations is fixed, as in the library), and no
  residual of the outer iteration is reported.
- The relaxed path is checked for equivalence with the library, for the uniform flow and for the level-1 error only; no
  accuracy or stability claim at large Courant numbers is made.
- The backward Euler truncation along the cell paths is not removed by outer iterations (ADR 0232 H6b: `rho` moves to
  3.88); BDF2 stays refused on the overset path.
- The first-order term of the calculated component cells (ADR 0232, F-334) and the fringe truncation (T1c) are untouched.
- Not delivered: the case-tree, CLI and Studio wiring (ADR 0219 Q1); a flux predictor for the first outer iteration (ADR
  0232 Q4 option (b)).

## Open questions

Listed with options and a recommendation in the addendum, after the results.

## Amendment 1 (2026-11-12, written after the registered runs of B, Q, U, E, L1 and D and before any further run)

The records R2 were taken on the unchanged library (`9efe7ffe`, run twice, equal), the library change and the tests were
committed compiled and not run (`844357b0`), and then every registered check ran once, release. All numbers are in
addendum 1. Met: Q0 (bit for bit, hash R1), R2, R2', D-S, E1, Q0o and Q0r (bit for bit), Q0n, U1o, U3o, U1r, U3r, N2o, E4
to E7, L1, L1r, S1 (`4.4936e-4`), T2o-a (`1.862`), T2o-b (`1.862`), T3o. One check is missed:

| Id | Limit | Measured |
| --- | --- | --- |
| D1 (acceptance) | `< 1.2e-4` | `1.690e-4` (`0.376` of the `4.4936e-4` with one outer iteration; largest in a calculated component cell at `(1.27, 1.20)`; by class CF `8.03e-5`, CC `1.690e-4`, BF `1.639e-4`, BC `1.481e-4`) |

D1 stays as registered in the ignored test `d1_time_error_with_three_outer_iterations_known_miss` (limit intact); the slow
test of the acceptance keeps S1, T2o-a, T2o-b and T3o. My prediction of D1 (`7e-5` to `1.0e-4`) was wrong: it applied the
fraction `0.153` that ADR 0232 measured on a single moving mesh with exact boundary data (SMc) to the coupled meshes.
There three outer iterations left `5.7e-5` (`0.153 * 3.726e-4`); here they leave `1.69e-4`, three times as much, and
nearly as much in the background cells (which do not move) as in the component cells.

The trend test with two and five outer iterations (registered as printed only) was started with the other runs; its log
has not been read when this amendment is written and committed. Hypotheses for the miss, each with its measurement and
deciding number, fixed here:

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H12 | three outer iterations have not converged the step: more iterations would remove more | the trend run: `c = max abs(d_200(5)) / max abs(d_200(3))` | confirmed: `c <= 0.8`; ruled out: `c >= 0.9` (the remainder is not a lag that outer iterations remove) |
| H13 | the remainder is a first-order time error | TG_2 with three outer iterations in 200, 400 and 800 steps: `R = max abs(d_200(3)) / max abs(d_400(3))` over the non-hole cells (first order gives 3) | confirmed: `R` in `[2.5, 3.5]`; ruled out: outside |
| H14 | the remainder is the backward Euler truncation along the paths of the moving cells (ADR 0232 H6, there ruled out for the whole error with one iteration, and found for the remainder of SMc: `rho = 3.88`) | the same at half speed (`tg_pose(t/2)`, three outer iterations, 200 and 800 steps): `rho_3 = max abs(d_200(3))` at full speed over that at half speed, against the kinematic `rho_BE` of ADR 0232 (`4.783` as a maximum, `3.700` as a root mean square) and against 2 (a lag linear in the speed) | confirmed: `rho_3 >= 2.85` (nearer to `3.70` than to 2); ruled out: `rho_3 < 2.85` |
| H15 | the remainder is larger than on a single mesh because the two meshes exchange it: the background cells carry it though they do not move | from the runs of H13: `max abs(d_200(3))` over the background calculated cells (BC) against that over the component calculated cells (CC), and the same ratio with one outer iteration (`3.367e-4 / 4.494e-4 = 0.749`) | confirmed: BC over CC `>= 0.7` with three outer iterations (the background carries the remainder in full measure); ruled out: `< 0.5` |

Reading fixed here. If H12 is ruled out and H13 and H14 are confirmed, the cause of the D1 miss is named: what three
outer iterations leave is the first-order truncation of the implicit Euler scheme along the paths of the moving cells,
which no number of outer iterations removes, and the number is `max abs(d_200(3)) = 1.69e-4` with `c`, `R` and `rho_3`
beside it. If H12 is confirmed, the cause is the number of iterations and `d_200(5)` is reported against the limit (it is
not a registered check and the limit of D1 is not moved to it). Otherwise the cause stays open and the rule gives partial.
H15 does not decide the cause; it explains the difference to SMc.

Test `a1_remainder_of_the_time_error_with_three_outer_iterations` (ignored, slow, run once; it repeats the two runs of D
with three outer iterations, which are deterministic, and adds the run in 400 steps and the two at half speed). The run
helper takes the speed of the motion since this amendment; the registered runs are unchanged. Predictions (mine): H12
ruled out (`c` near 1), H13 confirmed, H14 confirmed with `rho_3` between 3 and 4.5, H15 confirmed. The decision rule is
unchanged.
