# ADR 0388: F-483 registration: the pressure-lag estimate at an immersed body

Status: **REGISTERED 2026-11-13, written before any code of F-483 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-483 was written, compiled or run: the documents and code under "What was read" were
read and the arithmetic below was done by hand. Results go in the addendum at the end; a defective set-up is corrected by an
amendment written and committed before the corrected run; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0374](0374-f363-registration-immersed-flux-with-one-piso-corrector.md) (F-363: the five forcings, the set-up R,
the opt-in map with the reconstruction `PimpleSolver::immersed_lag`, the numbers of N0 and L1, question Q4 from which F-483 was
made), [ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) B4 (the pressure-lag map `lag_map`, its power iteration
`lag_amplification_n` and the run-time check), ADR 0156 D7 and ADR 0162 D5 (`lag_preflight`, `lag_trial`). Roadmap item F-483
(deps F-363). Branch `f483-immersed-pressure-lag` from main ecf12329, worktree `w49i-work/f483`, scratch `w49i-work/f483-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the number recorded; each registered run is made once; defaults stay
bit-identical; no hash table, golden or existing test file is edited (the helpers of `immersed_single_corrector_f363.rs` are
copied, that file is not edited); no new third-party crate, no unsafe code; unsigned commits without trailers; no push.

Clean-room: the pressure-lag map and its power-iteration estimate are rcfd's own (ADR 0073 B4, after the PISO of Issa, J. Comput.
Phys. 62 (1986) 40-65, cited there from memory). The power iteration (the dominant eigenvalue of a linear map from repeated
application and normalisation) and the property of conjugate gradients that the iterate stays in `x_0 + K_k(A, r_0)`, so that
a component of the start that the residual does not see is kept, are textbook linear algebra (Golub and Van Loan, Matrix
Computations, cited from memory; nothing of it is used beyond what is derived below). No vendor code, text, manual or binary.

## What F-483 asks

Acceptance (`docs/roadmap.json`), binding, clause by clause:

> The pressure-lag estimate of ADR 0073 for DFG 2D-2 on M2 with the first-order immersed forcing and one corrector at dt 1e-3
> (ADR 0374: 1.13 to 1.30 for a run that runs, the mode in a solid cell at the surface) is explained by a registered measurement
> (the solid cells, the start and the tolerance of the pressure solve), and either the estimate leaves out what is not a fluid
> mode, opt-in or with the defaults bit-identical, so that it is below one for that run and above one for a setting that stops,
> or the limitation is documented with its numbers.

1. *explained by a registered measurement*: (1a) the solid cells (check A1), (1b) the start of the pressure solve (A2),
   (1c) its tolerance (A3); the number itself reproduced first (R1).
2. *the estimate leaves out what is not a fluid mode, opt-in, defaults bit-identical* (D1, D2; checks U1 to U4, N1);
3. *below one for that run* (B1: the first-order forcing F, one corrector, `dt = 1e-3`, which runs 200 steps, ADR 0374 R0);
4. *above one for a setting that stops* (B2: the bounded forcing B with one corrector at `dt = 1e-3`, which stops at step 34,
   ADR 0374 R0; reproduced here, R2);
5. *or the limitation documented with its numbers*: the fallback if 3 or 4 is missed (then the item is partial, see the rule).

## What was read (disclosed)

ADR 0374 in full (registration, amendments 1 and 2, addendum 1); ADR 0073 B4 and its results; the roadmap entries F-483 and
F-363; `crates/rcfd-fv/src/pimple.rs`: `LagWarning`, `LagCtx`, the struct `PimpleSolver` and its initialisers in `new` and
`scratch`, `lag_preflight`, `lag_preflight_with`, `lag_trial`, `lag_preflight_moving`, the place of the check in the step (the
capture of `LagCtx` and the estimate after the step, lines about 1779 and 2116), `lag_map` (2171 to 2296), `lag_amplification_n`
(2301 to 2387) and `pick_correctors`; `crates/rcfd-fv/src/pressure.rs` `solve_pressure`, `solve_pressure_src`,
`solve_pressure_inner` (the assembly, `res0`, the AMG-CG call); `rcfd-linalg` `SolveControl` and the stopping test of `cg` and
AMG (`res <= tol_abs || res <= tol_rel * res0`, `res0` the residual of the start); `immersed.rs`: `ImmersedForcing` (the fields,
`body_of_cell`, `solid_mask`); `PimpleSettings::piso` (pressure solver `tol_abs` 1e-14, `tol_rel` 1e-7, `max_iter` 3000; the set-up
R sets `tol_rel` 1e-6); `crates/rcfd-fv/tests/immersed_single_corrector_f363.rs` (the DFG and circular Couette helpers, `fp_lag`,
`estimates`, `record`); `crates/rcfd-fv/tests/w34b_lag.rs` (`box_run`, the periodic Stokes box of ADR 0073 B2).

Facts read from the code (no run):

- `lag_map` starts every pressure solve of the map from the pressure error itself (`p_new = p_old.clone()`, `p_old` the error
  before the corrector) and stops it at `tol_rel` times the residual of that start (or at `tol_abs`).
- `lag_amplification_n` normalises with the volume-weighted 2-norm over **all** cells (the level removed over all cells when no
  boundary fixes it), takes `rho` as the exponential of the mean log growth over the second half of the iterations, and the mode
  cell as the argmax of `abs(e)` over all cells. The mode itself is not returned by any public method (`LagWarning` holds the
  cell only; ADR 0374 addendum could not print the share of the mode in the solid cells for that reason).
- The penalty of the immersed forcing is in the momentum diagonal of the solid cells (`coeff`, `K = 1e7` 1/s in R), so
  `rAU = V / a_P` there is about `1500 + 1240` against `1e7` per unit volume of a fluid cell: a ratio of about `2.7e-4`.

## Arithmetic before this registration (disclosed): the hypothesis HS

Set-up R (DFG 2D-2, M2, 176 x 104 = 18304 cells, core spacing `h = 0.0025`): the cylinder of diameter 0.1 holds about
`pi 0.05^2 / h^2 = 1257` cell centres, 0.87 percent of the volume (`0.00785 / (2.2 x 0.41)`).

**The exact map in a solid cell.** With the momentum predictor driven by `-V grad e`, `HbyA = H(u*)/a_P`; in a solid cell the
neighbour coefficients are those of a fluid cell (about 1240 per unit volume) and `a_P` is about `1e7`, so `HbyA_s` is about
`1e-4` of the neighbouring velocities: the solid cell returns almost nothing of the lagged gradient. The pressure equation in
the solid interior has face coefficients about `2.7e-4` of the fluid ones and a right-hand side of that order, so its exact
solution is close to a weighted harmonic extension of the pressure of the first solid layer: **the exact map carries no
error of the solid interior into the next step** (gain about `1e-4`). The first solid layer is tied to its fluid neighbour by a
face of coefficient about half the fluid one (the face value of `rAU` is the linear interpolation), and its row says that the
corrected flux of that face is (almost) zero: `p_s - p_f = h HbyA_f . n / rAU_f`, about `-D (e_s - e_f2) / 2` with `D` the
off-diagonal share (ADR 0073), so its gain is of the order of `D / 2`, below one.

**The stopped solve.** CG (AMG-preconditioned) from the start `x_0 = e` stops at `tol_rel res0`. The residual of a solid-interior
error is about `2.7e-4` of that of a fluid error of the same size, so once the fluid part of the residual has fallen by
`tol_rel` the solid interior has been reduced only as much as its small share of the residual required: a part of the start in
the solid interior is kept, unchanged, in `p_new`. A kept part is a gain of one; the CG polynomial of the step is not a linear
function of the start, so the "map" there is not linear either, and a power iteration on it can return a ratio slightly above
or below one. The numbers of ADR 0374 fit this reading without proving it: 1.13 to 1.30 at step 2 and 0.998 to 1.010 at step
11 for all five forcings, the two that run included; the mode cell in a solid cell for F (cell 11855, 0.0015 inside; 11681 with
40 iterations). The fluid part of an error decays by about `D` per application (with two correctors ADR 0374 measured 0.04 to
0.10, which suggests `D` about 0.25 with one, an estimate I do not rely on), so after 20 to 40 applications the iterated vector
would be almost all solid.

**HS.** The estimate above one of the first-order forcing at the body is not a fluid mode: it is the part of the error in the
solid cells that the pressure solve of the map keeps from its start because the solve stops at a relative tolerance before it
reaches the weakly coupled solid cells. Consequences, used as checks: (i) the mode of the present estimate lies mostly in the
solid cells (A1); (ii) started from zero, the solves of the map keep nothing of the error, and the estimate falls below one (A2);
(iii) with a tolerance tight enough to reach the solid cells (`tol_rel = 1e-12`, `tol_abs = 0`: the stop at `1e-6` leaves the
solid part at about `1e-6 / 2.7e-4`, about `4e-3` of the residual scale; `1e-12` is six orders below) the estimate from the error
also falls below one (A3). (iv) Measuring the norm over the fluid cells alone does not by itself remove a kept solid part that
leaks into the fluid through the gradient of the first fluid cells: the ratio of the fluid norms then tends to the same kept gain.
(iv) is printed, not a limit, because the leak is not estimated.

The value of `rho` for B with the change is **not predicted**: ADR 0374 found the frozen map at 1.18 for B at step 2 (the mode
at a fluid cell with two image faces) and about 1.0 at step 11 (after the growth began near step 9, G1); whether a fluid mode
above one remains once the kept solid part is removed is what B2 measures.

## Design (D1 to D3)

**D1. The fluid-mode estimate (opt-in).** A public type in `pimple.rs`:

```rust
pub struct LagFluid { pub zero_start: bool, pub fluid_norm: bool, pub tol_rel: Option<f64> }
impl LagFluid { pub const FLUID: LagFluid = LagFluid { zero_start: true, fluid_norm: true, tol_rel: None }; }
```

and a public field `PimpleSolver::lag_fluid: Option<LagFluid>` (`None` from `new`; copied by `scratch`). When `Some(o)`:

- `zero_start`: every pressure solve of `lag_map` starts from zero instead of from the error (the start of the linear map with
  an exact solve does not matter; from zero, the solve keeps nothing of the error that it does not reach). The relaxation
  `p = p_old + alpha_p (p_new - p_old)` is unchanged.
- `fluid_norm`: in `lag_amplification_n` the norm, the level (when no boundary fixes it) and the mode cell are taken over the
  fluid cells, those with `body_of_cell[c] == None` of the immersed forcing (every cell without a forcing). The vector itself is
  iterated in full: the solid entries stay part of the state of the map (they enter the gradient of the first fluid cells).
  For a linear map the ratio of the fluid norms tends to the same dominant eigenvalue as the full norm whenever that mode has a
  fluid part, so this removes only the weight of the solid entries in the estimate.
- `tol_rel = Some(t)`: the pressure solves of the map stop at `t` relative, with `tol_abs = 0` (the absolute tolerance 1e-14 of
  `piso` would stop a homogeneous map of unit volume-weighted norm, whose residuals are about 1e-5 on M2, at about 1e-9
  relative). A diagnostic of the measurement A3, not part of `FLUID`.

`LagFluid::FLUID` (zero start and fluid norm) is the estimate offered: **"the estimate leaves out what is not a fluid mode"**.
It is reached through every path of the estimate (`pressure_lag_check`, `corrector_control`, `lag_preflight`, `lag_trial`),
as `immersed_lag` is.

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the change in the estimate (an opt-in
field, as `immersed_lag`) rather than in the step (the step's pressure solve is not changed: the run is stable and the kept
part is harmless there); the zero start rather than a projection that zeroes the solid entries (the projection changes the map
by dropping the influence of the first solid layer, which is a real part of the step); the fluid norm on top (removes the weight
of the solid entries without changing the dominant eigenvalue).

**D2. The mode.** `PimpleSolver::lag_trial_mode(&self, dt, iterations, n_correctors) -> Option<(LagWarning, Vec<f64>)>`: the
estimate of `lag_trial` with the normalised mode of the power iteration (needed for A1). Built by a private field
`lag_mode: Option<Vec<f64>>` (`None` from `new` and `scratch`) that the check of the step fills with the mode only when it is
`Some`; `lag_preflight_with` is split into its guard and a private runner on the copy, operation for operation the same.

**D3. Defaults.** With `lag_fluid = None` and without `lag_trial_mode`, every operation of a step, of the lag map and of the
estimate is the present one. Hunks: `pimple.rs` only (the type, two fields and their initialisers, one argument and two
branches in `lag_map`, the weights and the mode cell in `lag_amplification_n`, the mode kept at the check, the split of
`lag_preflight_with`, the new method). `immersed.rs`, `pressure.rs`, `simple.rs`, `sim.rs`, the schema and the API are not
touched. No case key: the estimate with `FLUID` is a library option.

## The runs

Set-up R of ADR 0374 (copied): DFG 2D-2 on M2, `U_m = 1.5`, `nu = 1e-3`, `K = 1e7`, the start field of ADR 0249,
`PimpleSettings::piso` with BDF2, `LinearUpwind`, AMG pressure, momentum and pressure `tol_rel` 1e-6, `dt = 1e-3`, one
corrector, `flux_weight_check = false`. The forcings F, Sa, Sb, B1, B of ADR 0374. Two instants as in ADR 0374: the state at
step 0 (estimate at step 2, the first BDF2 step) and the state after step 10 (estimate at step 11). Every estimate is
`lag_trial` (or `lag_trial_mode`) with 40 power iterations. For Sa, Sb, B1 and B the estimates are made with
`immersed_lag = true` (the map with the reconstruction, the map of the step that runs); for F that flag changes nothing.

Variants of the estimate: **P** plain (`lag_fluid = None`); **Z** `{zero_start}` only; **T** `{tol_rel: 1e-12}` only (start from
the error); **N** `{fluid_norm}` only; **FL** `LagFluid::FLUID`.

## Registered checks

New file `crates/rcfd-fv/tests/immersed_lag_estimate_f483.rs`. The heavy record H carries `#[ignore]` and is run once by me in
release; the fast checks run in release in the gate (U1 to U4 and C1 also in debug).

| Id | Clause | Check | Limit |
| --- | --- | --- | --- |
| R1 (reproduction; record H) | 1 | P of F, one corrector, at step 2 and step 11 | `abs(rho - 1.1986) <= 1e-4` and `abs(rho - 0.9983) <= 1e-4` (ADR 0374 L1 F; a different number is a defective copy of the set-up) |
| R2 (reproduction; record H) | 4 | B, one corrector, check off, run on after step 10 | stops at step 34 by the Courant guard (ADR 0374 R0) |
| A1 (the solid cells; record H) | 1a | the mode of P of F at both instants: `s = sum_solid e^2 V / sum e^2 V` | `s >= 0.5` at both |
| A2 (the start; record H) | 1b | Z of F at both instants | `rho < 1` at both |
| A3 (the tolerance; record H) | 1c | T of F at both instants | `rho < 1` at both |
| B1 (record H) | 3 | FL of F, one corrector, at both instants | `rho < 1` at both |
| B2 (record H) | 4 | FL of B, one corrector, `immersed_lag = true` | `max(rho(2), rho(11)) > 1` |
| B3 (record H) | (control) | FL of F and of B with two correctors, both instants | `rho < 1` for all four |
| C2 (record H; negative control) | (control) | the channel of R without the cylinder (no forcing), one corrector, step 2: P and FL | `abs(rho_FL - rho_P) <= 0.02` and `rho_P < 1` |
| C1 (fast; negative control) | 2 | the periodic Stokes box of ADR 0073 B2 (N 32, BDF2, `pimple(nu, 2)`, pressure and momentum `tol_rel` 1e-10), `D = 1` and `D = 4`, after 5 steps: `lag_trial(0.01, 20, 1)` with P and with FL | `abs(rho_FL / rho_P - 1) <= 1e-3` for both `D` |
| U1 (fast) | 2 | circular Couette at N 24 (set-up of ADR 0374 U2, `build`, `theta_min = 1e-2`): `lag_fluid = Some({false, false, None})` against `None` | `rho` and cell bit-equal |
| U2 (fast) | 2 | the same set-up: Z, N and T each against P | `rho` differs in its bits for each |
| U3 (fast) | 2 | the same set-up: `lag_trial_mode` against `lag_trial`, P and FL | the warning bit-equal (step, `rho`, cell); the cell the argmax of `abs(mode)` (over the fluid cells for FL), the cell of FL a fluid cell; the mode of unit volume-weighted norm over the cells of its norm within 1e-12 |
| U4 (fast) | 2 | the same set-up, FL in the run-time check: `pressure_lag_check = LagCheck::new(0, Warn)` with `lag_fluid = FLUID`, one step; against `lag_trial` with FL on a fresh solver (Euler, so the estimate of step 1) | the estimate of the check bit-equal to that of `lag_trial` |
| N1 | 2 | unchanged existing tests, release: `immersed_single_corrector_f363` (not ignored: N0 with the fingerprints FP3 of the lag estimates, U1, U2, RF), `w34b_lag`, `w48v_corrector_control`, `consistent_correctors_f301`, `cargo test -p rcfd-fv --release --lib`, `cargo test -p rcfd-api --release --test baseline_hashes` | every test passes |

Printed, no limit: every `rho` with its cell, whether the cell is solid and its distance to the surface; for P of F also the
share of the mode in the first solid layer (solid cells with a fluid face neighbour) and the number of solid cells; N of F at
both instants; FL of Sa, Sb and B1 at both instants (with `immersed_lag = true`); P of B at both instants.

**Negative controls.** C1 (no body, converged solves: the change must not move a fluid mode), C2 (no body, the tolerance of R),
U1 (the option with every part off is the default), B3, and N1.

### Mutations (scratch copy of the final tree, not committed)

Each applied alone by an exact text replacement checked to occur once, the named test run once in release, the file restored
and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `zero_start` ignored (the map always starts from the error) | U2 (Z equal to P) |
| M2 | `fluid_norm` ignored in the norm (all cells weighted) | U2 (N equal to P) |
| M3 | `tol_rel` ignored | U2 (T equal to P) |
| M4 | `scratch` does not copy `lag_fluid` | U2 and U4 |
| M5 | the zero start applied with `lag_fluid = None` | N1 (`n0_fingerprints_of_the_defaults`, FP3) |
| M6 | the mode cell of FL taken over all cells | U3 (may survive if the largest entry of the FL mode on circular Couette is a fluid cell anyway; then recorded as survived) |

## Decision rule

- **done**: R1, R2, A1, A2, A3, B1, B2, B3, C1, C2, U1 to U4 and N1 met, and every mutation caught by its named check (M6 may
  survive as stated).
- **partial**: U1 to U4, C1 and N1 met (the opt-in estimate delivered with the defaults bit-identical) and any of A1 to A3, B1 to
  B3, C2 missed, or a mutation (other than M6) not caught: the limitation documented with its numbers (clause 5).
- **todo**: U1 to U4, C1 or N1 missed and not repaired: the library part is then not proposed.

A set-up check that does not hold (R1, R2) is a defective set-up: an amendment before any corrected run, never a pass.

## Limits (not covered)

One flow (DFG 2D-2) on one mesh (M2), BDF2, one convection scheme, one penalty, AMG-preconditioned CG; two dimensions, a body
at rest. The estimate stays a linearisation with frozen matrices and no velocity error at the start of the step (ADR 0073); it
says nothing of the period-two growth of the bounded form (ADR 0374 Q3, a separate item). The step's own pressure solve is not
changed. Moving meshes (`lag_preflight_moving`) take the option as every estimate does but are not measured.

## Open questions

- **Q1. FL as the default of the estimate with an immersed forcing.** Options: (a) opt-in only (as registered); (b) the default
  whenever `immersed` is set. (b) would change the estimate, and so `pressure_lag_check` warnings and the adaptive corrector
  count, of existing immersed runs: not taken (it changes a default result); left open for the maintainer.
- **Q2. A case key.** Not added (a schema golden would change). Open.
