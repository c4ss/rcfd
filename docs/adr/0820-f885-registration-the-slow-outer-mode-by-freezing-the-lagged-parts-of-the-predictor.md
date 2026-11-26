# ADR 0820: F-885 registration: the slow outer mode, by freezing the lagged parts of the momentum predictor one at a time

Status: **REGISTERED 2026-11-26, written before any code of F-885 for this registration and before any run of its checks.**
Before this text no library code, test, harness or solver run of this work was written or made. What was done: reading
(below), `git apply --check` of the six committed F-885 mutation patches at the base 05cb93d7 (all six apply), and the
arithmetic of this page. Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f885-slow-outer-mode.md`](briefs/f885-slow-outer-mode.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. A further diagnosis of F-885 under rule 13 (the slow
mode that remains with the opt-ins of [ADR 0811](0811-f885-registration-a-face-flux-independent-of-alpha-u-then-check-h.md)
and [ADR 0819](0819-f885-registration-a-limiter-factor-kept-from-the-steps-first-outer-iteration-and-mode-b-with-the-term-off.md)
on), taken as option (a) of ADR 0819's question 1 by the lead's decision of 2026-11-26. Branch `f885-slow-outer-mode` from
05cb93d7; scratch `w49i-work/f885-slow-outer-mode-scratch`. Written by the model Claude Opus 5.5.

Size (agent): done; 1; 1; 1; 150000

## What is asked

With ADR 0819's `outer_frozen_limiter` the fan's outer loop no longer cycles at c11837, but it still stalls on 20 steps of
H(dt): `C` 0.0143 against 1e-2, a factor `rho` of 0.834 an iteration (run F of ADR 0819). The brief asks for hypotheses for
that factor from the code, each with its file and line, the factor it predicts and a control that separates it, run on 20
steps of H(dt) with the cavity as a control, no long run, and the spectral radius of the outer map from successive
differences with the place of its dominant vector.

## What was read

Whole: `AGENTS.md`; `docs/process/agent-rules.md` (version 5); the brief; ADR 0819 (registration, addenda 1 to 3,
amendment 1, answers); `crates/rcfd-api/tests/f885_frozen_limiter.rs`.

In part: ADR 0815, addendum 1 (the controls GT, GS, GL, GP, GR, GA and their result); ADR 0816, addendum 1 (mode B: B1 to
B4, HB1 to HB4) and section 1 (the blend); `docs/process/tolerances.md`, the table; `crates/rcfd-fv/src/pimple.rs` (base
05cb93d7) lines 195 to 330 (the F-885 settings and records), 1100 to 1197 (`piso`, `pimple`, the solver tolerances), 1755 to
2870 (`step_with`: the outer loop, the predictor, the correctors, the finish); `crates/rcfd-fv/src/convection.rs` lines 125
to 260 (`face_value`, `convection_impl`); `crates/rcfd-fv/src/diffusion.rs` lines 1 to 160 (`laplacian`, `laplacian_opt`);
`crates/rcfd-api/src/sim.rs` lines 2100 to 2160 (the transient settings from the case); `crates/rcfd-mesh/src/quality.rs`
lines 450 to 475 (a sheared box built from a box's topology); the schema entry `solver.viscous_stress` (default
`laplacian`: the fan's case has no transposed stress term).

No published source outside the repository was read. The hypotheses and their estimates come from the code and from first
principles (sections 1 and 2); nothing rests on a published result.

## 1. What is already ruled out, and what of the step is still lagged (from the code and the records)

Ruled out by the committed records, not run again: the turbulence sweep's viscosity, the rotating frame's source, the linear
solvers' tolerances, the rotational seam made implicit (ADR 0815: GT, GS, GP, GR, GA all stall); a lost dominance, the
corrector count (two against four: `rho` 0.853789 and 0.853740), a blend of the relaxation with a slow unrelaxed map (ADR
0816: HB1 to HB4); ADR 0811's term (ADR 0819: HC2); the limiter's cycle (ADR 0819: mode A gone). The corrector count rules
out the splitting of the pressure-velocity coupling within an outer iteration: the correctors' `H` iteration contracts by
about `s / a_c` = 1 / 1.38 = 0.72 a corrector at c6245, so four correctors would have left 0.72^2 = 0.52 of what two
leave, and `rho` did not move.

What each outer iteration of the fan still takes from the iterate of the previous one, in the momentum predictor
(`crates/rcfd-fv/src/pimple.rs`, base 05cb93d7; the fan's case: BDF2, linear upwind with Barth-Jespersen, no
non-orthogonal pressure correctors, the Laplacian viscous term only):

* **The convecting flux** (Picard linearisation): `phi_conv` (lines 2025 to 2034) is `self.phi`, the flux after the
  previous outer iteration's correctors (in the first outer iteration of a BDF2 step, the extrapolated old fluxes). It
  multiplies the velocity in `convection` (line 2159) and builds the periodic links (`PeriodicLinks::build`).
* **The deferred correction of linear upwind**: `convection` (`convection.rs` lines 209 to 219) is the implicit upwind
  operator plus the explicit face term `flux (phi_ho - phi_ud)`, and for linear upwind `phi_ho - phi_ud` is
  `grad_U . (x_f - x_U)` (`convection.rs` lines 132 to 135): the iterate's least-squares gradient (`pimple.rs` lines 2130 to
  2134), times the kept factor (lines 2136 to 2157).
* **The deferred non-orthogonal correction of the momentum Laplacian**: `laplacian` with `Some(grad)` (`pimple.rs` lines
  2169 to 2181; `diffusion.rs` line 24) adds the explicit flux of `k . grad_f` with the same gradient.

The rest of the outer map is linear in the iterate once these three and the hook's two inputs (frozen by ADR 0815's
opt-ins) are kept: the relaxation, ADR 0811's term, the correctors' split, the predictor's lagged pressure gradient, the
boundary values of the iterate.

## 2. The hypotheses (fixed now)

| Id | Hypothesis | Where | Control (section 3) | Factor it predicts with its control |
| --- | --- | --- | --- | --- |
| HF | the Picard lag of the convecting flux carries the slow mode | `pimple.rs` 2025 to 2034, 2159 | GF: `phi_conv` of the step's first outer iteration kept | `rho(GF) <= 0.7627` |
| HD | the deferred correction of linear upwind (the iterate's gradient) carries it | `convection.rs` 132 to 135, 209 to 219; `pimple.rs` 2130 to 2165 | GD: the convection's gradient of the step's first outer iteration kept | `rho(GD) <= 0.7627` |
| HN | the deferred non-orthogonal correction of the momentum Laplacian carries it | `pimple.rs` 2169 to 2181; `diffusion.rs` 24 | GN: the Laplacian's gradient of the step's first outer iteration kept | `rho(GN) <= 0.7627` |
| HX | the three together carry it, none alone | the three places | GX: GF, GD and GN, with ADR 0815's viscosity and source freezes | `rho(GX) <= 0.7627`, no single control at or below it |
| HL | the linear rest of the map carries it (section 1, last paragraph) | the relaxation, the correctors, the lagged pressure gradient and boundary values | by elimination: GX | `abs(rho(GX) - rho(F0)) <= 0.015` |

**The threshold 0.7627.** ADR 0183's outer rule as ADR 0810 section 5 classes it: `C = d_19 / d_2 <= 1e-2`. For a loop that
contracts by `rho` an iteration, `d_19 / d_2 = rho^17`, which is at most 1e-2 when `rho <= 10^(-2/17)` = 0.7627. A
hypothesis carries the slow mode if removing its lag brings the factor to where 20 outer iterations meet the rule.

**The band 0.015.** ADR 0816 section 2 (less than half the smallest separation of two of its predictions; the spread of the
factor in ADR 0815 is a tenth of it), as ADR 0819 section 3 used it.

**Estimates (not limits), from first principles.** For HD, the 1D Fourier symbol of the deferred correction of linear upwind
with a centred gradient on the BDF2 upwind operator, `g(theta) = abs(C/4 (2i sin theta - 1 + e^(-2i theta))) / abs(1.5 + C (1 - e^(-i theta)))`,
has its largest value 0.42 at the local Courant number `C` = 4 (the value that the dominance ratio 1.38 at c6245 gives if
all of the off-diagonal sum is convection: `1.5 / (1.38 - 1)` = 3.9), and tends to 0.50 as `C` grows. So on a smooth mesh
HD alone predicts a factor near 0.42 to 0.5, below the 0.834 seen; it reaches 0.83 only through the skewed cells of the C
grid. For HF, the lag is the term `(delta phi) (u_f - u_P)` of the cell, of size `dt abs(grad u)` against the diagonal, which
the shear at a blade wall can make of order one; I have no number for the cell before the run. For HN, the deferred
correction of a face is of the order of the tangent of its non-orthogonality times the diffusion part of the diagonal; no
number before the run. The relaxation alone at c6245 (`alpha_u` 0.9, `a_c / s` = 1.38) gives `(1/9) / (1 - 1/1.38 + 1/9)` =
0.29 on the smoothest mode, the floor a run with the three lags removed would approach. My expectation: HF.

## 3. The opt-ins and the record (fixed now)

All in `crates/rcfd-fv/src/pimple.rs`, read only in `step_with`'s outer loop (not the fractional step, not the moving-mesh
lag map), each a local of `step_with` created empty before the outer loop of each step (reset at every step), default
`false` or empty so that no branch is taken and no copy made: every default bit-identical, `baseline_hashes` unchanged. No
case key, schema, golden or hash table; the runs set them on the built solver as ADR 0815 to ADR 0819 did.

* `PimpleSettings::outer_frozen_convecting_flux: bool` (GF): `phi_conv` of the step's first outer iteration (after the BDF2
  extrapolation and the moving-mesh map) is kept and used as `phi_conv` in every later outer iteration of the step, the
  final one included: in `convection`, the periodic and rotational links and the probe of ADR 0816. Not the correctors'
  flux, not ADR 0811's term (which reads `self.phi`).
* `PimpleSettings::outer_frozen_convection_gradient: bool` (GD): per component, the least-squares gradient that enters
  `convection` (and the limiter factor where one is formed) in the step's first outer iteration is kept and used in its
  place in the later ones. Read only where a gradient is formed.
* `PimpleSettings::outer_frozen_laplacian_gradient: bool` (GN): per component, the gradient that enters the Laplacian's
  non-orthogonal correction in the step's first outer iteration is kept and used in the later ones. Read only where a
  gradient is formed and `coupled_non_orth` is off (with it, the partner-point gradients are used as now).
* `PimpleSettings::outer_field_record: Vec<usize>` (the record): for each outer iteration named here (1-based), push to
  `PimpleSolver::outer_fields` an `OuterField { step, iteration, u, phi_conv, grad_convection, grad_laplacian }`: the
  velocity after the iteration's correctors, the convecting flux the predictor used, and per component (`None` where not
  formed or inactive) the gradient that entered `convection` (before the limiter factor) and the one that entered the
  Laplacian. Read-only: the copies enter no equation.

## 4. The fan runs (fixed now)

Harness: that of ADR 0819's run F (`h_text(4.605e-5)` copied unchanged from `f885_frozen_limiter.rs`, `alpha_free_face_flux
= true`, the limiter on with `outer_frozen_limiter = true`, `alpha_u` 0.9, two correctors, `n_outer = 20`, the record on,
`outer_pressure_tolerance = Some(-1.0)`), 20 steps from the uniform start, `outer_field_record = [15, 16, 17, 18, 19]`, the
records of each step drained by the test as the step ends.

| Run | added to F of ADR 0819 |
| --- | --- |
| F0 | nothing (F reproduced, with the field record) |
| GF | `outer_frozen_convecting_flux` |
| GD | `outer_frozen_convection_gradient` |
| GN | `outer_frozen_laplacian_gradient` |
| GX | the three, and `outer_freeze_viscosity`, `outer_freeze_source` (ADR 0815) |

Each run once, in one heavy test in one background process, the record of each written as it ends. Time estimate: ADR
0819's F took 79 s; about 7 min for the five; no cap. No long run, no check H.

**Measures.** As ADR 0819 (upper median, steps 11 to 20 classed): `rho` = median `(d_19 / d_16)^(1/3)`; `C` = median
`d_19 / d_2`, CONVERGES if `<= 1e-2`, STALLS if above, UNDECIDED if NaN or the run is not complete. From the field record,
with `delta_k = u_k - u_(k-1)` over all cells and active components and `n2` its Euclidean norm (cells unweighted):
`rho2` = median `(n2(delta_19) / n2(delta_16))^(1/3)`, the spectral radius of the outer map estimated by the power
iteration that the outer loop itself is; `cos` = median of the cosine of `delta_19` and `delta_18`; the place of
`delta_19`: its squared norm's share in each class of cell (a cell with a face on the patch `blade`, else `shroud`, else
`hub`, else a coupled (periodic) patch, else `inlet`, else `outlet`, else interior), per step, and the median share per
class; `n90`, the number of cells that hold 90 % of the squared norm (median); the cell of the largest change of `delta_19`
with its radius and `z` (the step of the median `rho`).

**The class of each control** `X` in GF, GD, GN, GX, with `D = rho(X) - rho(F0)`: CARRIES if `rho(X) <= 0.7627`; PART if
not and `D < -0.015`; NONE if `abs(D) <= 0.015`; SLOWS if `D > 0.015`; UNDECIDED if a run is not complete or a factor NaN.

**Result D.** If a class is UNDECIDED: **undecided**. Else, `S` the set of GF, GD, GN that CARRY: one member: **names** its
hypothesis (HF, HD or HN); more than one: **names the set**. `S` empty and GX CARRIES: **names HX**. `S` empty, GX NONE or
SLOWS and every single control NONE or SLOWS: **rules HF, HD, HN and HX out** and names HL by elimination. Otherwise
(`S` empty, some PART): **parts named**, with the set of PART.

**One dominant mode (beside, classed, no pass or fail).** F0 has one real dominant mode if `cos >= 0.99` and
`abs(rho2 - rho) <= 0.015` (the power iteration has settled on one real eigenvalue, and the field's factor is the largest
cell change's).

## 5. The cavity rows (fixed now)

The 8 x 8 lid-driven cavity of ADR 0767 (`box_2d(8, 8, 1, 1)`, lid `(1, 0, 0)`, from rest), and its sheared form `Sh`: the
same topology with every point moved by `x += 0.3 y` (as `quality.rs` 456 builds a sheared box), so that the faces between
columns are at `atan(0.3)` = 16.7 degrees to the line of the centres and the Laplacian's non-orthogonal correction is not
zero. Linear upwind with Barth-Jespersen and `outer_frozen_limiter` unless stated.

* **T1, what is kept**: `Sh`, `pimple(0.01, 5)` (implicit Euler), `dt` 0.05, 3 steps, GF, GD and GN on, the field record at
  iterations 1 to 5. In each step and iteration: `grad_convection` and `grad_laplacian` of each active component against
  `least_squares` of the state before the step (formed in the test with the crate's public functions), and `phi_conv`
  against the solver's flux before the step; the measure `max_c abs(a - b) / max_c abs(b)` (vector entries), limit
  `<= 1e-12`; `None` for the inactive component; the record's `u` of iteration 5 equal in bits to the solver's velocity
  after the step. Non-vacuity: the same run with the three off has, in step 2 or 3, `grad_convection`, `grad_laplacian` and
  `phi_conv` of iteration 5 differing from iteration 1 by more than 1e-9 by the same measure; and with the three off the
  record's gradients of iteration `k >= 2` equal `least_squares` of the record's `u` of iteration `k - 1` (`<= 1e-12`), so
  the record holds what was used. If the non-vacuity fails, T1 does not decide.
* **L, the frozen map solves one problem**: `Sh`, implicit Euler, `dt` 0.05, 3 steps, GF, GD and GN on, `alpha_u = alpha_p =
  1`, `n_correctors` 60, momentum and pressure `tol_rel` 1e-12, 5 outer iterations, the velocity record on. With every lag
  of the predictor kept and no relaxation, each outer iteration's correctors converge to the solution of the same linear
  system (the correctors' fixed point does not depend on the predictor's pressure), so `d_k / d_1 <= 1e-8` for `k` = 2 to 5
  in each step. Estimate: the correctors contract by about the dominance `s / a_c`, about 0.48 on this cavity (`V / dt` =
  0.3125, four diffusion links of 0.01, convection up to 0.125 a face), so 0.48^60 = 7e-20, with the solvers' 1e-12 left:
  about 1e-11, margin 1e3. Non-vacuity: with one of GF, GD, GN off at a time (three runs), `d_2 / d_1 > 1e-6` in some step;
  if not, L does not decide for that opt-in.
* **R0, the record read-only, and T2, each opt-in read only where it acts**: KL of ADR 0816 (`pimple(0.01, 20)`, BDF2, `dt`
  0.05, 3 steps, linear upwind with the limiter, the record and the switch on) with the field record at all 20 iterations
  against without: `u`, `p`, flux and the record of `d_k` equal bits. GD and GN with upwind convection and no
  non-orthogonal correctors (no gradient formed) against off, and GF with one outer iteration (`piso(0.01)`) against off:
  equal bits.
* **K, the control**: KL of ADR 0816 with `outer_frozen_limiter` and each of GF, GD, GN and the three together: `C` (median
  `d_19 / d_2` over steps 1 to 3) `<= 1e-2` in each of the four.

## Table of checks

No clause of the acceptance of F-885 can be met here (check H is not run); the brief's task is the diagnosis.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | F0 against F of ADR 0819 (`docs/investigations/f885-frozen-limiter/fan_d.csv`): every `d_k` of 20 steps | relative `<= 1e-6` | "Same discrete problem by two routes", widened to the 10 printed digits as ADR 0816 R0 | 0 | n/a | unchanged default output |
| FD | GF, GD, GN, GX complete 20 steps or stop with the stop recorded; result D names a hypothesis, names HX, or rules HF to HX out (section 4) | as stated | the brief, decision rule; section 4 | names HF | n/a | solution verification |
| FC | `C` of each of the five runs classed | classed: CONVERGES or STALLS, not UNDECIDED (recorded, not a limit) | ADR 0815, "The measures"; ADR 0810 section 5 | F0 STALLS 0.0143 | n/a | solution verification |
| SM | F0: `rho2`, `cos`, the place of `delta_19`, one dominant mode classed | classed yes or no (recorded, not a limit) | section 4 | yes, at the blade | n/a | solution verification |
| T1 | what is kept, and the record, on `Sh` (section 5) | `<= 1e-12`; equal bits; non-vacuous | "Same discrete problem by two routes", `<= 1e-12` where only rounding separates; "Bit-identity of default output" | 0 | n/a | code verification |
| L | the frozen map, `d_k / d_1` | `<= 1e-8` each; non-vacuous | "Same discrete problem by two routes", `<= 1e-8` (iterations `k` and 1 solve one linear system) | 1e-11 | 1e3 | code verification |
| R0 | the field record read-only (KL) | equal bits | "Bit-identity of default output" | equal | n/a | unchanged default output |
| T2 | each opt-in read only where it acts | equal bits | "Bit-identity of default output" | equal | n/a | unchanged default output |
| K | KL with each opt-in, and the three | `C <= 1e-2` each | ADR 0810 section 5 (ADR 0183's outer rule, as ADR 0767 E-1 states it) | 1e-8 | 1e6 | code verification |
| T3 | the rules of section 4 on built records: each class, each outcome of result D, `rho2`, `cos`, the shares and `n90` on built fields | each as built | this registration, section 4 | as built | n/a | code verification |
| G2 | the outcomes from the committed records: `rho`, `C` and class of the five runs, `rho2` and `cos` of F0, the classes of the controls, result D, R1 | equal; numbers within 1e-6 relative | "Same discrete problem by two routes", widened to the printed digits | 0 | n/a | unchanged default output |
| B3 | no solver default changes: `cargo test -p rcfd-api --release --test baseline_hashes` passes, `baseline_hashes.rs` unchanged | passes, file unchanged | the brief; rule 3 | passes | n/a | unchanged default output |
| D1 | under `crates/*/src` only `crates/rcfd-fv/src/pimple.rs` changes; `docs/roadmap.json`, the ledger, every golden and every existing test file unchanged; the six committed F-885 mutation patches still apply (`git apply --check`), re-based in this branch with the mutation unchanged if not | as stated | the brief, owned files; rule 3 | as stated | n/a | unchanged default output |
| P | `cargo test -p rcfd-fv --release --lib`; `cargo test -p rcfd-api --release --lib`; the new test file; the existing tests of the changed step `f885_frozen_limiter`, `f885_two_outer_modes`, `f885_outer_stall`, `f885_alpha_free_face_flux`, `f885_outer_fixed_points`, `f885_check_h_outer_record`; `cargo fmt -p rcfd-fv -p rcfd-api --check` (where it refuses with os error 206, `rustfmt --check` of the touched files with the repository's `rustfmt.toml`); `cargo clippy -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

The classes of section 4 and the class of `C` class a run; they are not pass or fail limits of a fan run. No row is of
kind validation.

## Tests (fixed now)

All in `crates/rcfd-api/tests/f885_slow_outer_mode.rs` (header: ADR 0820, F-885).

* Run by the gate: `f885_so_two_routes` (T1); `f885_so_linear_map` (L); `f885_so_read_only` (R0, T2);
  `f885_so_cavity_control` (K); `f885_so_rules_on_built_records` (T3); `f885_so_fan_from_the_committed_record` (G2, and
  R1, FD, FC, SM from the committed records).
* Heavy record (`#[ignore]`, run once by me): `f885_so_fan_record`, F0, GF, GD, GN, GX, writing to the folder named by
  `F885_SO_OUT`; the fast tests on the same code path are T1, L, R0, T2 and K (the opt-ins and the record) and ADR 0811's
  `f885_af_short_run_on_the_check_h_path` (the fan harness).

Records in `docs/investigations/f885-slow-outer-mode/`: `fan_d.csv` (run, step, `k`, `d_k`, cell; all steps),
`fields.csv` (run, classed step, `n2(delta_k)` for `k` = 16 to 19, `cos`, the seven shares, `n90`, the cell of the largest
change), the run log, the cavity log and the mutation log. The outcomes are written into the test file after the
registered run (`FAN_RECORD`), as ADR 0819 did; the form and the tolerance are fixed here.

## Negative controls and planned mutations

On a scratch copy of the branch (`git archive` of the full commit into the scratch folder, its own target folder, every
source file touched after extraction), each against the gate tests of the new file; the heavy record is not rerun for a
mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| N1 | solver: the kept convecting flux not reset at a new step (kept from the first step of the run) | `f885_so_two_routes` (T1) |
| N2 | solver: with GD, the convection takes the gradient formed anew while the kept one is still kept and recorded | `f885_so_linear_map` (L) |
| N3 | solver: with GN, the kept Laplacian gradient of component 0 used for every component | `f885_so_two_routes` (T1) |
| N4 | solver: `outer_frozen_convection_gradient` not read | `f885_so_two_routes` (T1) |
| N5 | solver: the record's `u` taken at the start of the iteration | `f885_so_two_routes` (T1) |
| N6 | solver: with the field record, the momentum solve starts from zero | `f885_so_read_only` (R0) |
| N7 | test: the band of section 4 0.15 for 0.015 | `f885_so_rules_on_built_records` (T3) |
| N8 | record: `fan_d.csv` cut to its first half | `f885_so_fan_from_the_committed_record` (G2) |

The committed patch (`crates/rcfd-api/tests/f885_slow_outer_mode.mutation.patch`) is N1, with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f885_slow_outer_mode f885_so_two_routes`.

## Decision rule

* **Done** for this registration if every run was made as registered, R1, T1, L, R0, T2, K, T3, G2, B3, D1 and P are met,
  the five runs are classed (FC), each mutation of the table is caught, and result D names a hypothesis, names HX or rules
  HF to HX out (FD).
* **Partial** if a row misses or does not decide, a run was not made as registered, a mutation is not caught, or result D
  is "names the set", "parts named" or undecided, with the reason and what it would take.
* **Stopped and reported** if an opt-in cannot be made with the defaults bit-identical, if F0 does not reproduce F of ADR
  0819 (R1), or if anything else in the solver would need changing.

F-885 itself stays partial whatever this gives, until check H decides (the brief). A defective set-up found before a
registered run is corrected by an amendment committed before the corrected run.

## What is not covered

* Check H, H(dt/2), a long run; any remedy; any change of a default; the opt-ins in the steady solver or the fractional step.
* The periodic faces' central deferred correction in `convection` (it reads the lagged partner, not the gradient), the
  boundary values of the iterate, the predictor's lagged pressure gradient one by one (they are in HL together).
* A freeze of a field other than the step's first outer iteration's.

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation: the remedy the result points to, whether check H can
be run with it, and any question of scope.
