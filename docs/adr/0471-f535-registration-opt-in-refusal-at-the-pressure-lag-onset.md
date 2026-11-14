# ADR 0471: F-535 registration: opt-in refusal of the PISO step at the pressure-lag onset, naming the diffusion number

Status: **REGISTERED 2026-11-14, written before any code of F-535 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0451](0451-f530-registration-piso-stability-at-large-diffusion-numbers.md) (F-530: the fixed-mesh channel FC, the
onset of the divergence between `D = 2.5` and `3.0`, the consistent correctors with 8 outer iterations as the remedy),
[ADR 0458](0458-f536-registration-outer-iteration-criterion.md) (F-536: the outer count),
[ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) (the pressure-lag estimate B4),
[ADR 0214](0214-f299-registration-piso-instability-at-large-cell-diffusion-number-across-a-thin-gap.md) (the corrector
instability and the consistent correctors). Roadmap item F-535 (deps F-530). Number 0471 given by the lead. Branch `f535-work`
(from main `fc026814`), worktree `w49i-work/f535`, scratch `w49i-work/f535-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; defaults bit-identical; no edit of `baseline_hashes.rs`, the
hash tables or a golden file; existing tests are not edited; no new dependency, no unsafe code; clean-room (the refusal reuses
the estimate of ADR 0073 and a cell length from the cell volume and face areas; nothing is taken from literature or another
code).

## What F-535 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. An opt-in
setting makes the PISO/PIMPLE step refuse at step 1, through the divergence guard and with the state left as it was, when the
pressure-lag estimate of ADR 0073 for the step's settings is at least one, with a message naming the largest cell diffusion
number nu dt/dx^2, the estimate and the remedies (consistent correctors with outer iterations, a smaller step); the fixed-mesh
channel of ADR 0451 is refused at nu dt/dy^2 = 25 and 100 and runs at 2.5; the default results of every existing case stay
bit-identical."

Clauses: (c0) every other limit registered here; (c1) an opt-in setting; (c2) refusal at step 1 through the divergence guard
with the state left as it was; (c3) when the estimate for the step's settings is at least one; (c4) the message names the
largest cell diffusion number, the estimate and the remedies; (c5) FC refused at `D = 25` and `100` and run at `2.5`; (c6) the
default results of every existing case bit-identical.

## What was read (before any Rust change)

`PimpleSettings::piso` and its fields, `LagCheck`, `LagAction::{Warn, Raise}`, `LagWarning`, the pre-run estimate
`PimpleSolver::lag_preflight` / `lag_preflight_with` / `preflight_run` / `scratch`, `Divergence` and its `message`, `trip`,
`guard_end`, and `step_with` up to the outer loop (the F0 refusal of the consistent correctors, which also does `step_no += 1`
and then `trip`) and the end of the step where `LagAction::Raise` trips (`"lag amplification"`). `sim.rs` of `rcfd-api` (the
`kind` of a transient divergence) and `FvGeometry` (`active`, `empty`). ADR 0451's records of the estimate and
`crates/rcfd-post/tests/piso_diffusion_f530.rs` (`FC`, `settings`, `step`, `b4`, `outer_settings`).

**Finding of the read.** The code already has `LagAction::Raise`, which trips the guard when `rho >= 1`, but (i) only **after**
the step has been taken (the velocity, pressure and flux of the step are in the fields, only the history is not advanced), (ii)
only through `pressure_lag_check` at a chosen step and without a number of the diffusion in the message, and (iii) its message
names neither the cell diffusion number nor the remedy of ADR 0451. The pre-run estimate `lag_preflight` takes the step on a
copy of the solver (`scratch`) with the same check, so the estimate for the step's settings can be made **before** any change of
the state. The refusal therefore reuses `lag_preflight` (the ADR 0073 estimate, not re-derived) and the guard's `trip`.

**Probes before this registration (disclosed).** Two throw-away ignored tests (not committed, removed again) on FC
(`box_mesh([120, 40, 1], [100 h, h, h], [Expansion(21), Uniform, Uniform])`, `PimpleSettings::piso`, central convection, tight
solvers, inflow of step 1 set; `h = 0.02`, `Wo = 1`, `nu = 6.28319e-4`), release, `iterations` = power iterations of the estimate:

| `N` | `D = nu dt/dy^2` | iterations | estimate `rho` (mode cell) |
| --- | --- | --- | --- |
| 1000 | 2.513 | 20 | 0.9618 (56) |
| 1000 | 2.513 | 60 | 0.9871 (64) |
| 1000 | 2.513 | 200 | **1.0003** (4743) |
| 838, 718, 628 | 3.0, 3.5, 4.0 | 20 | 1.172, 1.388, 1.603 |
| 500 | 5.027 | 20 | 2.036 |
| 100 | 25.13 | 20 | 9.232 |
| 50 | 50.27 | 20 | 16.158 |
| 25 | 100.53 | 20 | 25.938 |

and `lag_preflight(dt, 20).rho` equals the `rho` of `lag_estimates[0]` of the same step taken with
`pressure_lag_check = LagCheck::new(0, Warn)` **bit for bit** (difference `0` at `N = 25, 100, 1000`), and the B4 of the remedy
(`consistent_correctors`, `n_outer = 8`, 20 iterations) at step 1 is `0.9201` (`N = 25`), `0.7308` (`N = 100`), `0.0595`
(`N = 1000`). **The estimate is a geometric mean over the second half of a power iteration, so it depends on the iteration
count.** At `D = 2.51` it creeps from 0.962 (20 iterations) to 1.0003 (200 iterations, a slowly growing mode in cell 4743, not
the thin-gap mode of the larger `D`). The acceptance "runs at 2.5" is therefore met with the estimate of ADR 0073's default of
20 iterations, with a margin of 0.038, and would not be met with 200. This is recorded as a limit and an open question, not
tuned: the refusal takes the iteration count as part of the setting, and the registered runs use 20.

## Design

### D1. The setting (opt-in)

`PimpleSettings::lag_onset_refusal: Option<usize>`, default `None` in `piso` (hence in `pimple`): `Some(n)` = refuse the first
step of the solver when the estimate with `n` power iterations (`n >= 2` is used; below that 2) is at least one. `None` does
nothing and no line of the step runs for it beyond one `Option` test. Whether to expose it in the case file is the maintainer's
call (recommendation in the addendum); a key would need `hash_omit_default` and changes the schema golden, so none is added
here (house rule 3).

### D2. The refusal

At the start of `step_with`, after the existing refusals and before any work (before `step_no` is advanced and before any field
is touched): when `lag_onset_refusal = Some(n)`, `step_no == 0` (the first step the solver takes), the mesh is at rest and the step
is not the fractional step, the solver forms `w = lag_preflight(dt, n)` on a copy (the copy runs with the option cleared, so the
estimate does not recurse). If `w.rho >= 1` the step is refused: `step_no += 1` (as the other pre-step refusals of the guard do,
so that `Divergence::step` is 1), then `trip` with stage `"pressure-lag onset"`, quantity `"lag onset"`, cell the cell of the
largest diffusion number, value `rho`, and a new field `Divergence::diffusion` (the largest cell diffusion number; `NaN` for every
other trip). The velocity, pressure and flux fields, the time, the histories, `lag_estimates`, `lag_warnings` and every record
stay as they were; later `step` calls return the recorded information without work (the guard's existing behaviour). Under BDF2
the preflight takes the second step of the trial copy (its existing rule), so that is the step estimated; the refusal is still
at the first call. Not defined (no refusal, the step runs as without the option): the moving-mesh steps (they do not go through
this entry for the check; `lag_preflight` is `None` there), the fractional step, a stopped run, and any step after the first.

### D3. The largest cell diffusion number

`PimpleSolver::max_cell_diffusion(dt) -> (f64, usize)`: for each cell `c`, a length `h_c = V_c / max_f |S_f|` over the faces of
the cell, leaving out the faces whose normal lies within `0.01` of an inactive (empty) direction (`FvGeometry::active`, the 2D
case: the empty planes would make the depth the length) and a viscosity `nu_c` = `settings.nu`, or the largest of `nu_face` over
the cell's faces when `nu_face` is set; `D_c = nu_c dt / h_c^2`; the maximum and its cell. For a hexahedron `V / max |S|` is the
shortest edge (so `nu dt/dy^2` for the channel), for the others a representative width. It names the number the roadmap asks for;
it does not enter the decision (the estimate decides).

### D4. The message

For quantity `"lag onset"`, `Divergence::message` returns: `PIMPLE pressure-lag onset refusal (opt-in, ADR 0471): the largest
cell diffusion number nu dt/dx^2 of the step is {D:.3}{at}, and the estimated amplification of the lagged pressure error (ADR
0073) is {rho:.3} per step (>= 1), so the step would diverge; the run is stopped at step {step} before any work and the state is
as it was: use the consistent correctors with outer iterations (solver.consistent_correctors.enabled with solver.n_outer >= 8,
ADR 0214, ADR 0451) or a smaller solver.dt`. `{at}` is the existing cell position suffix.

## Set-ups

FC, `settings(false)` and `outer_settings()`, `step`, `inflow`, `b4`, `Checks` as in
`crates/rcfd-post/tests/piso_diffusion_f530.rs`, copied into the new file `crates/rcfd-post/tests/piso_lag_refusal_f535.rs`
(that file is not edited). `N` steps per period: `N = 25, 50, 100` is `D = 100.53, 50.27, 25.13`; `N = 1000` is `D = 2.513`;
`N = 838, 500` is `D = 3.0, 5.03`. The option for every run is `lag_onset_refusal = Some(20)`. "A step" is the call of `step` of
ADR 0451's test (which sets the inflow of step `n`, then `fl.step(dt)`).

Library-level tests (new file `crates/rcfd-fv/tests/lag_refusal_f535.rs`, small and fast): FQ of ADR 0451 (`box_mesh([30, 16, 1],
[20 h, h, h], [Expansion(5.5), Uniform, Uniform])`, `N = 16`, `D = 25.13`, estimate `5.0` at 20 iterations by ADR 0451), and a 2D
cavity (`rcfd_mesh::gen::cavity_2d`) for the diffusion number.

## Registered checks

Every check is run once on the registered set-up.

| Id | Clause | What | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| R1 | c2, c3, c5 | FC default settings with the option, `N = 25, 50, 100`: after the first `step` call `diverged` is `Some` with quantity `"lag onset"`, stage `"pressure-lag onset"`, step 1; its value (the estimate) | value `>= 1` | probes: 25.94, 16.16, 9.232 |
| R2 | c3, c5 | the same at `N = 838` and `N = 500` (`D = 3.0`, `5.03`) | refused, value `>= 1` | probes 1.172, 2.036 |
| R3 | c3 | the value of R1 against `lag_estimates[0].rho` of the same step taken with `pressure_lag_check = LagCheck::new(0, Warn)` (no option), each of `N = 25, 50, 100` | relative difference `<= 1e-12` | probe: exactly 0 (the same operations on a copy) |
| R4 | c2 | the state after R1's refusal: `u.values`, `p.values`, `phi` bit for bit those before the call, `time == 0.0`, `history` empty, `lag_estimates`, `lag_warnings`, `outer_record` empty; three further `step` calls return the first call's `StepInfo` and leave the same fields unchanged | every one exact | D2 (no field is touched before the trip) |
| R5 | c4 | the largest diffusion number: the field `diffusion` of the divergence against `nu dt/dy^2` of the channel, each of `N = 25, 50, 100` | relative difference `<= 1e-9` | `V/max|S|` for the cells of FC is `dy` to rounding `1e-15`; every `dx >= 0.126 h = 5 dy` and `dz = 40 dy` |
| R6 | c4 | the message from `Divergence::message(None)`: contains the substrings `format!("{:.3}", D)`, `format!("{:.3}", rho)`, `nu dt/dx^2`, `consistent_correctors`, `n_outer`, `solver.dt`, `step 1` and `state is as it was` | all present | D4 |
| R7 | c5 | FC with the option at `N = 1000` (`D = 2.51`): `lag_preflight(dt, 20).rho` (step 1 set as above) | `< 1` | probe 0.9618 |
| R8 | c5 | the same run: 20 steps, no trip (`diverged` is `None`), `max |u| <= 10 V0` after every step | bounded | ADR 0451 A5: the default is stable at `D = 2.51` |
| R9 | c6 | the 20 steps of R8 against the same 20 steps without the option: `u.values`, `p.values`, `phi` and `time` after every step | exactly equal (bit for bit) | the preflight runs on a copy; no field is written |
| R10 | c2 | the first step only: after the passed step of R8, a step with `dt = 1/25` (`D = 100.5`) is not refused by the option | `diverged` is `None` or its quantity is not `"lag onset"` | D2 (`step_no == 0` only) |
| R11 | c4 | the remedies pass the refusal: `outer_settings()` (consistent correctors, `n_outer = 8`) with the option at `N = 25, 50, 100`: the first step and three more steps are not refused and `diverged` stays `None` | no trip | probes of the remedy: B4 `0.920` (`N = 25`), `0.731` (`N = 100`), 0.852 recorded by ADR 0458 for `N = 50` |
| R12 | c6 | negative control of R1: FC at `N = 100` with the option `None` takes the first step: `diverged` is `None` and `u.values` differ from the initial (rest) | both | the default path has no refusal |
| L1 | c2, c3 | `rcfd-fv`, FQ with the option, `N = 16`: refused at step 1, value `>= 1`, quantity `"lag onset"`; fields as before (bitwise); `diffusion` against `nu dt/dy^2` | all; `1e-9` | ADR 0451: 5.0 at `D = 25.1` |
| L2 | c6 | `rcfd-fv`, FQ at `N = 400` (`D = 1.0`): option `Some(20)`; the first 5 steps bit for bit equal to the run without the option; no trip | exactly equal | the estimate is below one at `D = 1` (ADR 0451: the default is stable below 2.5; the run is measured, not assumed: if it were refused L2 is reported as a miss) |
| L3 | c4 | `rcfd-fv`, the 2D cavity (`cavity_2d(16, 1.0)`, `nu = 1e-3`, `dt = 0.1`, a viscosity-only `D` of `nu dt n^2 = 25.6`): `max_cell_diffusion(dt)` against `nu dt n^2` | relative `<= 1e-9` | the empty planes are left out, so `h = 1/16` |
| L4 | c3 | `rcfd-fv`, FQ with the option and the fractional step (`fractional = Some(..)` as the existing tests build it) is not refused: the first step returns without `diverged` and equals the run without the option bit for bit | exactly equal | D2 (not defined) |
| L5 | c1 | `PimpleSettings::piso(nu).lag_onset_refusal` is `None` | is none | D1 |
| N1 | c6 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes | only an `Option` test is added to the default path |
| N2 | c6 | the `rcfd-fv` suite and `rcfd-post`'s `piso_diffusion_f530`, `piso_outer_criterion_f536` fast tests, release | pass | D1 to D4 |
| N3 | rules | `git diff fc026814 -- crates/*/tests` adds files only | adds only | rules |

Printed records, no limit: per level the estimate, the diffusion number, the time of the refusal call.

## Negative controls

* R12 is the control of R1 (no option: no refusal).
* R7 and R8 are the control of R1 and R2 at the other side of the onset: the same option, the same settings, accepted.
* R11 is the control for the message's remedy: the remedy it names removes the refusal.

## Planned mutations (scratch copy, not committed)

* MU1: the threshold `rho >= 1.0` replaced by `rho >= 1e9` (never refuses). Expected: caught by R1, R2, L1.
* MU2: the threshold replaced by `rho >= 0.0` (always refuses). Expected: caught by R7, R8, R9, L2.
* MU3: the refusal made after the step (the preflight replaced by taking the real step, then the trip). Expected: caught by R4 (the fields changed).
* MU4: the refusal also sets `self.time += dt`. Expected: caught by R4.
* MU5: `h_c = V_c / min_f |S_f|` in `max_cell_diffusion`. Expected: caught by R5 (and L1, L3).
* MU6: the empty-direction filter dropped. Expected: caught by L3 (the depth becomes the length).
* MU7: the diffusion number left out of the message. Expected: caught by R6.
* MU8: the cleared option dropped from the preflight copy (the copy refuses itself). Expected: the copy recurses without end: caught by every refusal test as a stack overflow or a hang; recorded as what it does.
* MU9: the check made at every step (`step_no == 0` dropped). Expected: caught by R10.
* MU10: `lag_preflight` called with `iterations = 2` instead of `n`. Expected: caught by R3 (different value) or R1/R7 limits; recorded as what caught it.

## Decision rule

* **done**: R1 to R12, L1 to L5, N1 to N3 met on their registered runs, every mutation caught, no amendment made after a run.
* **partial**: the refusal is built and opt-in with N1 to N3 met, but a check missed, a mutation was not caught, or a check was met
  only after an amendment made after its run.
* **todo**: the refusal cannot be built without changing a default (N1 or N2 fails and cannot be mended opt-in).

## Limits (not covered)

* Only the first step of the solver is examined; a later change of `dt`, of the viscosity or of the boundary conditions that
  pushes the estimate above one is not refused (R10 records it). `Option<usize>` for the iteration count keeps the setting
  open to the pre-run estimate's sensitivity, which the probes show (0.962 at 20, 1.0003 at 200 iterations at `D = 2.51`).
* The estimate is the linearised, frozen-matrix power iteration of ADR 0073 (a heuristic, not a bound); a refused run is one the
  estimate says diverges, an accepted one is not proven stable. FC is the one set-up tested at a given `Wo`.
* Not defined: moving meshes (`step_ale`, `step_sliding` have their own pre-run estimate `lag_preflight_moving`), the fractional
  step, a stopped run. A hook (`OuterHook`) is not applied by the trial copy.
* The preflight costs one more step and the power iterations (about 0.8 s on FC); it is opt-in.
* The diffusion number is a cell number (`V / max |S|` length); on a strongly non-orthogonal or skewed cell it is a
  representative, not a stencil, value. It is a report, not the decision.
* No case key and no change of the `rcfd-api` `kind` of a transient divergence (it would report `non_finite`); both are open
  questions in the addendum.

## Amendment 1 (2026-11-14, after the registered runs of the checks and of MU1 to MU10, before any run of L3b)

The registered mutation MU6 (the empty-direction filter dropped) was **not caught** by any registered check: L3 uses `cavity_2d`,
whose depth equals the cell width, so the largest face area of a cell is the same with and without the filter. (Also, the
registered table of L3 gave `dt = 0.1`, `nu = 1e-3` and `D = 25.6`; these are inconsistent, `nu dt n^2 = 0.0256`. The test uses the
registered `nu` and `dt` and the limit is against `nu dt n^2`, as registered; the 25.6 was an arithmetic slip.) The first six
mutations were run once; MU6 is left in the record as uncaught by L3.

New check **L3b** (library test, added now, before it is run): a 2D box with a thin depth: `box_mesh([8, 8, 1], [1, 1, 0.01])`,
`zmin` and `zmax` set to `PatchKind::Empty` through `MeshTopology` and `Mesh::from_topology`, `nu = 1e-3`, `dt = 0.1`:
`max_cell_diffusion(dt)` against `nu dt / (1/8)^2 = 6.4e-3`, relative difference `<= 1e-9`. Estimate: with the filter `h = V / (dx
dz) = 1/8`; without it `h = dz = 0.01` and the number is 156 times larger. MU6 is to be run again with L3b (expected caught by L3b
only). By the decision rule as registered, a check that is met only after an amendment made after a run gives **partial**; the
addendum reports that and says what the amendment changed.

## Addendum 1: results (2026-11-14)

Commits on `f535-work`: `b2442863` (this registration), `0b74cb0e` (the library change and the registered tests; every registered
run was made on it, with `cargo fmt` applied to the test files before the commit, layout only), `02cf6974` (amendment 1 and the
dynamic-mesh page), `9e60fd05` (L3b), and the last commit (this addendum and the roadmap entries). Release profile throughout
(`CARGO_TARGET_DIR=w49i-work/target-f535`, `target-f535b`, `f535-scratch/target-mut`); logs in `w49i-work/f535-scratch`
(`run1_post.log`, `fv_suite.log`, `fv_subset.log`, `post_subset.log`, `baseline_hashes.log`, `clippy.log`, `docs_site.log`,
`mutlogs/`). Each registered run was made once. One amendment (L3b) was made after the runs, for the surviving mutation MU6.

**The library change** (`crates/rcfd-fv/src/pimple.rs`, all opt-in, 86 lines added, none removed): `PimpleSettings::lag_onset_refusal:
Option<usize>` (`None` in `piso`), `Divergence::diffusion` (`NaN` in `trip`, set by the refusal), the `"lag onset"` branch of
`Divergence::message`, `PimpleSolver::max_cell_diffusion`, the clearing of the option in the trial copy of `preflight_run`, and
the gate at the start of `step_with` (after the F0 and immersed refusals and the fractional return, before `self.geom.clone()`).
With `None` the gate is one `Option` test.

### Results

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| R1 FC refused at step 1 as `lag onset`, `N = 100, 50, 25` | value `>= 1` | `9.2322`, `16.1584`, `25.9376` | yes |
| R2 `N = 838, 500` (`D = 3.00, 5.03`) | refused, `>= 1` | `1.1722`, `2.0356` | yes |
| R3 against the Warn estimate, `N = 100, 50, 25` | `<= 1e-12` | `0` each | yes |
| R4 state as it was (fields, time, history, records), three further calls | exact | exact at every level | yes |
| R5 diffusion number against `nu dt/dy^2` | `<= 1e-9` | `2.55e-14` at each level | yes |
| R6 message substrings (8 per level) | all present | 24 of 24 | yes |
| R7 estimate at `D = 2.51` | `< 1` | `0.96178` | yes |
| R8 20 steps at `D = 2.51`: no trip, `max abs(u)` | `<= 10 V0` | none; `1.439 V0` | yes |
| R9 bit for bit equal to the run without the option, every step | exact | equal | yes |
| R10 a later step with `D = 100` not refused by the option | not `lag onset` | not refused | yes |
| R11 remedies (consistent, `n_outer = 8`), `N = 100, 50, 25`, 4 steps | no trip | no trip | yes |
| R12 no option: step taken at `N = 100` | no trip, `u` moved | both | yes |
| L1 FQ refused, fields as before, `diffusion` | `>= 1`; exact; `1e-9` | `5.0100`; exact; `25.1327` (`nu dt/dy^2` to 1e-9) | yes |
| L2 FQ `N = 400`, 5 steps equal to the run without the option | exact | equal, no trip | yes |
| L3 `cavity_2d(16, 1)` diffusion number | `1e-9` | `2.5600000000000008e-2` against `2.56e-2` | yes |
| L3b thin 2D box (amendment 1) | `1e-9` | `6.4e-3` against `6.4e-3` | yes |
| L4 fractional step not refused, equal to the run without the option | exact | equal | yes |
| L5 default is `None` | none | none | yes |
| N1 `baseline_hashes` | passes | 3 passed, 1 ignored | yes |
| N2 `rcfd-fv` suite, release | passes | see "What was not run" | **partly** |
| N2 subset (`w34b_lag`, `piso_thin_gap_f299`, `consistent_correctors_f301`, `consistent_defaults_f301`, `outer_criterion_f536`, `lag_refusal_f535`) and `rcfd-post` `piso_diffusion_f530`, `piso_outer_criterion_f536` | pass | fv: 12, 6, 6, 3, 18, 3 passed (0 failed, 9 ignored records); post: 3 + 3 passed (0 failed, 8 ignored records) | yes |
| N3 tests: new files only | adds only | two files added, none edited | yes |

Clippy `-D warnings` on `rcfd-fv` and `rcfd-post` (all targets): clean. `cargo fmt -p rcfd-fv -p rcfd-post --check`: clean.
`cargo test -p rcfd-cli --release --test docs_site`: 2 passed (the page `docs/dynamic-mesh.md` was changed).

Records (no limit): the refusal call takes about `0.85 s` on FC (one trial step with 20 power iterations).

**Deviations.** (i) L3's registered `dt` and `nu` were inconsistent with the quoted `D = 25.6` (amendment 1); the test uses the
registered values against `nu dt n^2`. (ii) `cargo fmt` was applied to the test files after the registered runs and before their
commit; layout only, the runs were not repeated. (iii) L3b was added after the runs (amendment 1).

### Mutations

Applied one at a time to a `git archive` copy of `0b74cb0e` by `f535-scratch/mutate.py` (exact replacement, one occurrence
asserted); both new test files were run in release in `f535-scratch/target-mut`; logs `mutlogs/MU1.log` to `MU10.log`.

| Mutation | Caught by |
| --- | --- |
| MU1 threshold never reached | L1; R1, R2 |
| MU2 threshold always reached | L2; R7, R8, R9, R11 |
| MU3 state touched before the trip | L1; R1 (R4) |
| MU4 `time += dt` in the refusal | L1; R1 (R4) |
| MU5 `V / min S` as the length | L1; R1 (R5) |
| **MU6 empty-direction filter dropped** | **not caught by the registered checks** (L3's cavity has depth equal to the cell width, so the largest face is the same with and without the filter); **caught by L3b** (amendment 1, `mutlogs/MU6_amend1.log`) |
| MU7 the diffusion number replaced by the estimate in the message | L1; R1 (R5, R6) |
| MU8 the option not cleared in the trial copy | every refusal test: stack overflow in both files |
| MU9 refusal tried at every step | R10 only (the fv tests take no second step) |
| MU10 `iterations = 2` | R1, R2 (the estimate at 2 iterations is below one at these levels, so nothing is refused) |

### Reading

* The estimate depends on the power-iteration count. At `D = 2.51` it is `0.962` with 20 iterations and `1.0003` with 200 (probe
  before the registration; the slowly growing mode sits in cell 4743, not the thin-gap mode). So "runs at 2.5" holds for the
  registered 20 iterations with a margin of 0.038; with 200 iterations the same step would be refused. The estimate is a
  diagnostic of the linearised map, not a stability bound.
* The refusal is a statement about the first step's settings. A solver started at a safe step that later raises `dt` is not
  refused (R10).
* Under BDF2 the estimate is that of the second step of the trial copy; the refusal is still at the first call.
* `rcfd-api` turns a transient trip into a `kind` by `quantity`; `"lag onset"` falls under `non_finite` there. Not changed (the
  option is not reachable from a case file, and the change would touch the event JSON).

### What was not run

The complete `cargo test -p rcfd-fv --release` was started (log `fv_suite.log`). At the time of writing 65 of about 187 test
binaries had run, with 448 passed, 0 failed and 159 ignored; it was then held up by a heavy existing test of
`immersed_piso_stop_f355`. The remaining binaries were not completed by this agent; the lead's gate covers the whole suite. The
registered N2 is therefore partly run, with the subset above as the tests that cover the changed lines.

### Decision by the registered rule

R1 to R12, L1 to L5, L3b, N1 and N3 are met on their registered runs, and the acceptance clauses c1 to c6 hold with the estimate
at 20 iterations. By the letter of the rule the status is **partial**, for two reasons: MU6 survived the registered checks and
was caught only by a check added by amendment after the runs (L3b), and N2 was not completed. No limit was missed or changed, and
no default result changed. Recommendation: accept as done once the lead's gate has run the whole `rcfd-fv` suite, because both
reasons concern the evidence, not a missed limit.

### Open questions (each with a recommendation)

* Q1. Expose the option in the case file (`solver.lag_onset_refusal`)? Options: (a) no key, Rust API only; (b) a key with
  `hash_omit_default` (boolean, count fixed at 20), which still changes the schema golden, the CLI help and the MCP list, so it
  needs the maintainer's approval of the golden edit. Recommendation: (b) after that approval, because a case-file user meets the
  divergence and needs the message. The maintainer decides; the Rust API option is enough for this item.
* Q2. Report `"lag onset"` in the `rcfd-api` divergence event as `lag_amplification` with limit 1 (a few lines in `sim.rs`)?
  Recommendation: yes, together with Q1.
* Q3. The estimate depends on the iteration count (0.962 at 20, 1.0003 at 200 at `D = 2.51`): keep 20 or raise it?
  Recommendation: keep 20 (ADR 0073's default, what ADR 0451 measured) and keep the sensitivity on record.
* Q4. Refuse at later steps as well (when `dt` or the viscosity changes)? Recommendation: no; the pre-run setup rule SV301
  (`lag_preflight`) and the adaptive control `corrector_control` cover the run.

## For the lead: roadmap entries

> F-535 status: partial. Note: Numerics. PARTIAL (ADR 0471, 2026-11-14; registered before the code, amendment 1 after the registered runs). The opt-in PimpleSettings::lag_onset_refusal = Some(n) makes the first PISO/PIMPLE step refuse, through the divergence guard and before any field is touched, when the ADR 0073 estimate for the step's settings (n power iterations on a copy of the solver) is at least one; the message names the largest cell diffusion number nu dt/dx^2 (PimpleSolver::max_cell_diffusion), the estimate and the remedies. The fixed-mesh channel of ADR 0451 is refused at nu dt/dy^2 = 100, 50, 25, 5.03 and 3.00 (estimates 25.9, 16.2, 9.23, 2.04, 1.17), with the state bit for bit as it was, and runs at 2.51 (estimate 0.962, 20 steps bit for bit equal to the run without the option); the consistent correctors with 8 outer iterations pass. Defaults bit-identical (baseline_hashes passes; one Option test on the default path). Ten mutations: nine caught by the registered checks, the empty-direction filter only by a check added by amendment (L3b). Partial because of that amendment and because the complete rcfd-fv suite was not finished by the agent. Limits: the estimate depends on the iteration count (0.962 at 20, 1.0003 at 200 iterations at 2.51); first step only; not on moving meshes or the fractional step; no case key. Open: ADR 0471 Q1 to Q4.
