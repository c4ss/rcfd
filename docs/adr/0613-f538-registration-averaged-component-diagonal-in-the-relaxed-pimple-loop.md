# ADR 0613: F-538 registration: the averaged component diagonal in the relaxed PIMPLE loop

Status: **REGISTERED 2026-11-15, written before any code of F-538 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text the base `2bc0141c` (main) was not changed; no solver run was made for this item before this
commit (the arithmetic below is Python on the models, no solver call). Results go in the addendum at the end; nothing above it is
changed after a run.

Parent: [ADR 0455](0455-f531-registration-symmetry-plane-tangential-viscous-flux.md) (F-531: the averaged component diagonal of
the PISO correctors next to a `Symmetry` plane; its amendment 1 and remaining question 2: the relaxed loop shows no excess).
Also [ADR 0446](0446-f421-registration-aerodynamic-damping-energy-method.md) (the oscillating channel QF), [ADR 0451](0451-f530-registration-piso-stability-at-large-diffusion-numbers.md)
(the discrete impedance of the implicit-Euler profile), [ADR 0010](0010-component-diagonal.md). Roadmap item F-538 (deps
F-531). Number 0613 given by the lead. Branch `f538-work` from `2bc0141c`, worktree `w49i-work/f538`, scratch
`w49i-work/f538-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; a
missed limit becomes an ignored `*_known_miss` test with the limit intact; defaults bit-identical (no library line is planned);
no edit of `baseline_hashes.rs`, the hash tables or a golden file; existing tests and ignored records not edited (the channel
code of `symmetry_flux_f531.rs` is copied into a new file); no new dependency, no unsafe code; clean-room (every formula below is
derived here from the discrete operators of `PimpleSolver::step` as they stand in the repository; nothing is taken from
literature). Choices the maintainer would normally be asked about are taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation) and marked so.

## What F-538 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. The channel of ADR 0455 QF with Symmetry planes under
> PimpleSettings::pimple (alpha_u 0.8, alpha_p 0.5, one corrector, 10 outer iterations), whose excess over the zero-gradient
> planes was -0.0021 against the averaged two-corrector PISO step's 0.36, is explained by an exact linear model of the relaxed
> step that reproduces the measured impedance of both runs within registered limits; the dependence on alpha_u is stated.

Clauses: **c0** every other limit registered here; **c1** an exact linear model of the relaxed step; **c2** it reproduces the
measured impedance of both runs (the relaxed PIMPLE run and the averaged two-corrector PISO run) within registered limits;
**c3** the near-zero excess is explained by it; **c4** the dependence on `alpha_u` is stated (and checked).

**Reading of c2 (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation).** The channel is short
(`L_f = 20 h`); its impedance is a developed part proportional to the length plus a part of the two ends (the moving end wall,
where the plug profile develops, and the outlet), which is a 2-D flow. ADR 0455's diagnosis (`diag1_length.log`) separated the
two with a second length (`40 h`, 60 cells, the same `Expansion(5.5)`, whose end cells are within 1 % of the 20 h mesh's:
`0.250 h` and `0.2515 h`). The model below is of the developed channel, as ADR 0455's `channel_step_model.py`; it is compared
with the **per-length part** `Z(40 h) - Z(20 h)` of each run (one 20 h of developed channel), for each run separately and for the
excess `Delta Z = Z(Symmetry) - Z(zero gradient)`. The end parts `2 Z(20 h) - Z(40 h)` are recorded, not modelled. Recommended
over a 2-D dense model of the whole channel (the graded mesh, the moving wall, Rhie-Chow and the ALE terms reproduced by hand), which
is a larger item with its own risk; listed as a possible follow-up if the end part turns out to carry the effect.

## What I read, and the arithmetic done before this registration (disclosed)

Read: ADR 0455 (D1, D4, amendment 1, the addendum and its remaining questions), its test file
`crates/rcfd-post/tests/symmetry_flux_f531.rs`, its scratch (`channel_step_model.py`, `zz_f531_diag.rs`, `diag1_length.log`,
`run3_q.log`, `run4_q4.log`); the code `crates/rcfd-fv/src/pimple.rs` `PimpleSolver::step_with` (the outer loop lines 1709 to
2300 at `2bc0141c`: `(alpha_u, alpha_p) = (1, 1)` on the last outer iteration; the momentum matrices relaxed by
`PeriodicLinks::relax` before the predictor; `a_p` the mean of the **relaxed** component diagonals; `H` from the relaxed matrix;
`u = HbyA - rAU grad p_new`; `p <- p_old + alpha_p (p_new - p_old)` after the last corrector), `PeriodicLinks::relax` (line 2786)
and `rcfd-linalg/src/matrix.rs` `LduMatrix::relax` (`d = max(diag, sum |offdiag|)`, `diag <- d/alpha`, `b += (d/alpha - diag)
x_old`, `x_old` the iterate at the start of the outer iteration), `PimpleSettings::pimple` (`n_correctors = 1`, `alpha_u = 0.8`,
`alpha_p = 0.5`, the rest of `piso`).

Already measured and published (so any agreement with them is a **postdiction**, stated as such): ADR 0455 Q4 (relaxed, 20 h, N =
100) `QD10 = 3.0308399791722214 + 1.080025234703981 i`, `QZ10 = 3.032931844875428 + 1.0807530111304366 i`, `Delta Z = -0.00209 -
0.00073 i`; Q3 (PISO, 20 h, N = 100) `QD = 3.3926045570268286 + 1.079197068958323 i`, `QZ = 3.0318371762026075 +
1.0928385577536086 i`; the unregistered 40 h PISO runs of `diag1_length.log` `6.754904879701068 + 2.161364829243946 i` (Symmetry),
`6.032149424900445 + 2.188689265043991 i` (zero gradient), so the PISO per-length parts `3.36230 + 1.08217 i`, `3.00031 + 1.09585
i`, `Delta Z = 0.36199 - 0.01368 i`. The relaxed 40 h runs and every run of the `alpha_u` sweep and at `N = 50` are new: those
are predictions.

Arithmetic (Python 3.9 with numpy, `f538-scratch/relaxed_model.py`, `sweep1.py` to `sweep3.py`; no solver run): the models of
D1 to D3 below at the registered parameters. The numbers quoted in the table come from there.

## D1. The exact linear model of the step in the developed channel

The developed channel of ADR 0455 D4: `n_y = 8` cells across the gap `h = 0.02 m`, one cell deep (`z` planes `Symmetry` or zero
gradient), the flow uniform in `x`, driven by the piston flux `v` (per area, every corrector imposes `mean_y(u) = v`); `rho =
1000`, `Wo = 1` (`nu = w (h/2)^2`, `w = 2 pi`), `dt = 1/N`. Per volume, with `kap = nu/dy^2`: the `x` matrix of the own component
`D + O`, `D_i = 1/dt + kap (l_i + r_i)` (`l, r = 2` at the no-slip walls `y = 0, h`, else 1), `O` the off-diagonals `-kap`; right
side `b = u_old/dt`; the pressure enters as a uniform gradient `G` (per unit density). The averaged diagonal is `a_P = D + L`, `L =
lambda I`, `lambda = 4 nu/(3 h^2)` with `Symmetry` planes (ADR 0455 D1: the normal component's Dirichlet zero adds `2 nu/h^2` per
plane, `4 nu/h^2` in all, and the mean over the three components adds `lambda`), `lambda = 0` with zero-gradient planes. The component's own diagonal is the
`x` one in both cases (the developed flow has `u_y = u_z = 0`).

The step `(u_old, G_old) -> (u, G)` as `step_with` takes it, for outer iterations `o = 0 .. n_outer - 1` with `(a_u, a_p) =
(alpha_u, alpha_p)` except `(1, 1)` on the last, starting from `u = u_old`, `G = G_old`:

1. relax: `D_r = D/a_u`, `b_r = b + (D_r - D) u` (`u` the iterate at the start of the outer iteration);
2. predictor: `(D_r + O) u* = b_r - G`;
3. `n_corr` correctors, each: `h = b_r - O u`, `HbyA = h/a_P,r` (`a_P,r = (D + L)/a_u`; with the opt-in corrected split `HbyA =
   (h + (a_P,r - D_r) u)/a_P,r`), `G_new = (mean HbyA - v)/mean(1/a_P,r)`, `u = HbyA - G_new/a_P,r`; after the last corrector `G <-
   G + a_p (G_new - G)` (the earlier ones take `a = 1`).

The step is linear: `x_n = T x_{n-1} + g v_n`, `x = (u, G)`. The periodic state at `v = V e^{i w t}` is `X = (I - T e^{-i w
dt})^{-1} g`, and the impedance of the developed length `L` is `Z = -rho A L G_X s/(i w)`, `A = h^2`, `s = (1 - e^{-i w dt})/dt`
(the sign is that of `periodic_response`: positive real part for a resisting load; ADR 0455's Python printed the opposite sign).
With `n_outer = 1`, `n_corr = 2`, `a_u = a_p = 1` this is ADR 0455's `channel_step_model.py` (model of the averaged PISO step).
The test builds `T` column by column with this arithmetic and solves the complex system by Gaussian elimination (as ADR 0455).

Neglected, as in ADR 0455 (whose PISO model met the per-length part to 0.4 % of `Delta Z`): the ends; the mesh motion (its terms
are second order in the amplitude `1e-3 h`); the `x` grading of the developed part; convection (uniform in `x`).

## D2. Why the relaxed loop shows no excess: the converged loop in closed form

Ten outer iterations converge the loop (the model's `Delta Z` changes by `< 1e-7` from 10 to 200 outer iterations, `sweep1.py`).
At a fixed point of the relaxed iteration (`u` and `G` unchanged, `G_new = G`) step 3 gives `a_P,r u = h - G = b_r - O u* - G =
D_r u*` (by step 2), so

`u = D (D + L)^{-1} u*`, i.e. `u* = (I + D^{-1} L) u`:

the predictor overshoots the corrected velocity by `lambda/D`. Putting this into step 2 (`b_r = b + (D/alpha_u - D) u`):

`K u = b - G`, `K = D + O + L/alpha_u + O D^{-1} L`.

The relaxed iterations therefore converge to the own-component equation with a sink `L/alpha_u` (larger than D1 of ADR 0455,
as its amendment 1 found) **and** an anti-sink `O D^{-1} L`: the off-diagonal sum of `O` is `-(D - 1/dt - w)` on the profile (`w`
the wall drag rate), so the predictor's overshoot leaks out through the neighbour links. `alpha_p` drops out (it changes the path,
not the fixed point). The last, unrelaxed iteration then solves `(D + O) u** = b - G_fp` and makes one corrector with the averaged
diagonal, which lowers the pressure gradient by about `lambda` times the overshoot of `u**`. The converged loop is therefore the
linear map `(u_old) -> (u_f, G_f)` with `K u_fp = b - G_fp`, `mean(u_fp) = v`, `(D + O) u** = b - G_fp`, `u_f = (D u** + G_fp -
G_f)/(D + L)`, `mean(u_f) = v` ("model CL", `sweep2.py`); it agrees with the dense 10-outer model to `4e-4` at `alpha_u = 0.5`
and to `1e-6` from 0.6 to 1.0.

**A 0-D reduction (the statement of the `alpha_u` dependence).** Replace each operator by its action on the developed profile:
`D -> d = 1/dt + 2 nu/dy^2` (the interior diagonal), `D + O -> e = 1/dt + w` with the plane-Poiseuille drag rate `w = 12 nu/h^2`
(mean wall shear of the parabolic profile per mean velocity), `O -> -(d - e)`. The algebra above gives the final pressure
gradient `G_f = b - e v - sigma lambda v` with

`sigma(alpha_u) = r - (1/alpha_u - 1)(1/r - 1)`, `r = e/d = (1/dt + 12 nu/h^2)/(1/dt + 2 nu/dy^2)`,

the fraction of ADR 0455 D1's infinite-corrector sink that the relaxed loop leaves. It vanishes at `1/alpha_0 = 1 + r^2/(1 - r)`.
At QF's `N = 100` (`nu dt/dy^2 = 1.005`): `r = 0.3948`, `sigma(0.8) = 0.0115`, `alpha_0 = 0.7952`. **The excess is not removed by
the relaxed loop; at `alpha_u = 0.8` and this time step two effects of the averaged diagonal cancel**: the relaxed iterations'
sink `lambda (1/alpha_u - 1)`-amplified through the final iteration (negative, `-(1/alpha_u - 1)(1/r - 1)`) and the one-corrector
remainder `r` (positive). With `alpha_u = 1` the converged one-corrector loop keeps `sigma = r`; below `alpha_0` the excess is
negative (the `Symmetry` channel's impedance falls below the zero-gradient one); at `N = 50` (`r = 0.274`, `alpha_0 = 0.906`) the
same `alpha_u = 0.8` gives `sigma = -0.387`.

The dense model's `sigma` (its `Delta Z` divided by ADR 0455's sink model `Delta Z_D1 = 0.39152 - 0.01714 i` at `N = 100`, real
parts): `-0.589, -0.243, 0.017, 0.219, 0.381` at `alpha_u = 0.6, 0.7, 0.8, 0.9, 1.0`; the 0-D formula `-0.627, -0.262, 0.012,
0.224, 0.395`; zero of the dense model `0.7926`, of the 0-D formula `0.7952`.

## D3. Predictions of the dense model (per 20 h of developed channel, `Z` in N s/m, the periodic-response convention)

| Run | `Z` Symmetry | `Z` zero gradient | `Delta Z` |
|---|---|---|---|
| PISO (1 outer, 2 correctors), N 100 | `3.364169 + 1.081501 i` | `3.000657 + 1.095371 i` | `0.363511 - 0.013869 i` |
| relaxed `alpha_u 0.6`, N 100 | `2.773386 + 1.092046 i` | `3.004042 + 1.083964 i` | `-0.230656 + 0.008082 i` |
| relaxed `alpha_u 0.7`, N 100 | `2.908999 + 1.086995 i` | `3.004070 + 1.084150 i` | `-0.095070 + 0.002846 i` |
| relaxed `alpha_u 0.8`, N 100 (the `pimple` constructor) | `3.010720 + 1.083076 i` | `3.004071 + 1.084151 i` | `0.006650 - 0.001075 i` |
| relaxed `alpha_u 0.9`, N 100 | `3.089843 + 1.080027 i` | `3.004071 + 1.084150 i` | `0.085772 - 0.004124 i` |
| relaxed `alpha_u 1.0`, N 100 | `3.153147 + 1.077587 i` | `3.004071 + 1.084149 i` | `0.149076 - 0.006563 i` |
| relaxed `alpha_u 0.8`, N 50 | `2.921110 + 0.993948 i` | `3.071765 + 0.983672 i` | `-0.150655 + 0.010276 i` |

Relaxed runs: `PimpleSettings::pimple(nu, 10)` with `alpha_u` as stated (`alpha_p = 0.5`, one corrector), everything else as
ADR 0455 Q4. With the corrected split the relaxed `Symmetry` model equals the relaxed zero-gradient one to `2e-9`. Model variants
used as negative controls (`alpha_u = 0.8`, N 100): the final iteration relaxed too, `Delta Z = 0.24695`; relaxation ignored
(`alpha_u = 1` in the model), `0.14908`; ADR 0455 D1's sink, `0.39152`.

The postdiction for the relaxed 20 h run: the model's developed `0.0066` against the measured `-0.0021`; the difference `-0.0087`
would be the end part (ADR 0455's PISO end part was `-0.0012`). Expected from D2: in the end regions the profile is plug-like
(`w` small, `r` closer to `1/dt/d`), where the 0-D `sigma(0.8)` is negative, so a negative end part in the relaxed loop is
plausible; it is recorded (E1), not a limit.

## Set-up of the runs

The channel, its run and `periodic_response` are those of `symmetry_flux_f531.rs` (copied; that file is not edited): `n_x = 30`,
`L_f = 20 h` and `n_x = 60`, `L_f = 40 h`, `n_y = 8`, `Expansion(5.5)` in `x`, `Wo = 1`, `C = 3` cycles, central convection,
solvers `tol_abs 1e-16`, `tol_rel 1e-13`, `max_iter 5000`, moved by `ChannelMotion` through `FlowWall`, amplitude `1e-3 h`. The
relaxed settings are `PimpleSettings::pimple(nu, 10)` then `alpha_u` set; PISO is `PimpleSettings::piso(nu)`. Per-length part
`Z_pl = Z(40 h) - Z(20 h)`, compared with the model at `L = 20 h`; end part `Z_end = 2 Z(20 h) - Z(40 h)`.

Runs (each once, release): group P (PISO, N 100: Symmetry and zero gradient at 20 h and 40 h, 4 runs); group R (relaxed
`alpha_u = 0.8`, N 100: the same 4 runs, plus the corrected split with Symmetry at 20 h); group A (relaxed `alpha_u = 0.6, 0.7,
0.9, 1.0`, N 100: 16 runs); group N (relaxed `alpha_u = 0.8`, N 50: 4 runs).

## Registered checks

Test file `crates/rcfd-post/tests/relaxed_pimple_diagonal_f538.rs`. Every value is printed; a test fails at the end if any of its
checks missed. Per-part errors of a complex `Z` are `|Re Z/Re Z_ref - 1|`, `|Im Z/Im Z_ref - 1|`; "absolute" means the difference
in N s/m. Release profile.

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| A0 | c0 | this file committed before any code | holds | holds |
| R0 | c2 | the 20 h runs of groups P and R reproduce ADR 0455's printed `QD`, `QZ`, `QD10`, `QZ10`: `abs(Z - Z_0455)/abs(Z_0455)` | `<= 1e-9` each | `0` (the same code path; the base moved since) |
| C1 | c1, c2, c3 | relaxed `alpha_u 0.8`: `Delta Z_pl` against the model's `0.006650 - 0.001075 i`, absolute | real `<= 5e-3`, imaginary `<= 2e-3` | `1.5e-3`, `2e-4` (ADR 0455's PISO model met its per-length part to `1.5e-3`, `1.9e-4`) |
| C2 | c2 | PISO: `Delta Z_pl` against the model's `0.363511 - 0.013869 i`, absolute | real `<= 5e-3`, imaginary `<= 2e-3` | `1.5e-3`, `1.9e-4` (postdiction from `diag1_length.log`) |
| C3 | c2 | `Z_pl` of each of the four runs (relaxed and PISO, Symmetry and zero gradient) against the model, per part | `<= 2e-3` each | `6e-4` (PISO postdiction: `5.6e-4`, `6.2e-4`; `1.1e-4`, `4.4e-4`) |
| C4 | c4 | sweep `alpha_u = 0.6, 0.7, 0.9, 1.0`: `Re Delta Z_pl` against the model, absolute; `Z_pl` of each run per part | `<= 5e-3 + 0.02 abs(Re Delta Z_model)`; `<= 2e-3` | `1.5e-3 + 1 %`; `6e-4` |
| C5 | c4 | the zero of `Re Delta Z_pl(alpha_u)` by linear interpolation between the measured bracketing pair, against the same interpolation of the model (`0.7935` between 0.7 and 0.8) | `<= 0.02` | `2e-3` (an error `1.5e-3` in `Delta Z` over a slope of about `1.0` per unit `alpha_u`) |
| C6 | c3, c4 | `N = 50`, `alpha_u 0.8`: `Re Delta Z_pl` against the model's `-0.150655`, absolute; `Z_pl` per part | `<= 5e-3 + 0.02 x 0.150655`; `<= 3e-3` | `2e-3`; `1e-3` (`nu dt/dy^2 = 2`) |
| C7 | c1 | model CL (D2, closed form of the converged loop) against the dense 10-outer model: `abs(Delta Z_CL - Delta Z_dense)` at every `alpha_u` of the sweep and 0.8; `alpha_p = 0.3` and `0.7` against `0.5` in the dense model | `<= 1e-3`; `<= 1e-3` | `1e-6`; `1.3e-4` |
| C8 | c4 | the 0-D formula of D2 against the dense model: `abs(sigma_0D - sigma_dense)` at every `alpha_u` of the sweep and 0.8 (N 100), and the zeros `abs(alpha_0,0D - alpha_0,dense)` at N 100 and N 50 | `<= 0.06`; `<= 0.02` | `0.04`; `2.6e-3` (N 100: `0.7952` against `0.7926`), `3e-3` (N 50: `0.9061` against `0.9090`) |
| RC | c3 | relaxed `alpha_u 0.8`, the corrected split, Symmetry, 20 h, against the relaxed zero-gradient 20 h run, per part | `<= 1e-3` each | `1e-5` (ADR 0455 Q1 for PISO: `1.1e-5`, `1.1e-4`) |
| Q5 | c2 | every run periodic (`cycle_change`) | `<= 1e-6` | `<= 1e-12` |
| E1 | c3 | end parts `Delta Z_end = 2 Delta Z(20 h) - Delta Z(40 h)` of every group | record | relaxed `0.8`: about `-0.009` if C1 holds; PISO `-0.0012` |

### Negative controls

* **NC1** (the model discriminates): the measured `Re Delta Z_pl` of group R differs by at least `0.1` from each of three wrong
  models: the final outer iteration relaxed too (`0.24695`), relaxation ignored (`0.14908`), ADR 0455 D1's sink (`0.39152`).
* **NC2** (the cancellation is not structural): `Re Delta Z_pl` at `N = 50` and at `N = 100` (both `alpha_u = 0.8`) differ by at
  least `0.1` (model: `-0.1507` against `0.0066`).
* **NC3**: `Re Delta Z_pl` of the sweep is monotone increasing in `alpha_u` (model: yes) and changes sign between 0.7 and 0.8.

### Planned mutations (on a scratch copy with its own target directory; each run against C1 to C3 (groups P, R) and C7, C8)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `pimple.rs`: the last outer iteration relaxed too (`(alpha_u, alpha_p)` without the `if last`) | C1 (model 0.247), C3 |
| MU2 | `rcfd-linalg` `LduMatrix::relax`: the source `(d/alpha - diag) x_old` dropped | C3 |
| MU3 | `pimple.rs`: `alpha_p` not applied (`a = 1` after the last corrector) | C1 or C3 (the dense model with `alpha_p = 1` gives `-0.28`, an unconverged loop) |
| MU4 | the test's model: the final iteration relaxed (the model of NC1's first variant) | C1 |
| MU5 | the test's model: the relaxation source taken from `u_old` instead of the outer iterate | C1, C3, C7 |

## Decision rule

* **done**: every check of the table met on its registered run (C1 to C8, RC, Q5, R0), the negative controls hold, every mutation
  caught, nothing met only after an amendment.
* **partial**: a registered check missed (a `*_known_miss` record with the limit intact) or a mutation not caught, while C1 and C2
  are met (the model reproduces the developed part of both runs).
* **todo**: C1 or C2 missed and the cause not located (the model does not describe the step).

## Limits (what this does not cover)

* The end parts of the short channel (a 2-D flow at the moving wall and the outlet) are measured, not modelled; the model is of
  the developed channel.
* One mesh across (`n_y = 8`), one Womersley number, two time steps; the 0-D formula is a reduction with the plane-Poiseuille drag,
  checked against the dense model only where stated.
* `n_correctors > 1` with relaxation, the opt-in consistent correctors, the outer-iteration criterion of ADR 0458, the steady and
  variable-density kernels: not run.
* No library change; whether the default diagonal becomes `corrected` stays the maintainer's open decision of ADR 0455 remaining
  question 1 (`component-diagonal-default.md`).

## Amendment 1 (2026-11-15, written after the registered runs on commit `6857342e`, before the amended comparison)

**What the registered runs showed** (`cargo test -p rcfd-post --release --test relaxed_pimple_diagonal_f538 f538_registered_runs
-- --ignored --nocapture`, log `f538-scratch/run2_registered.log`, 135 s, all 29 runs made once): R0 (`0` at all four), C2, the PISO
part of C3, every zero-gradient part of C3, C4 and C6 (within `7.7e-5`), RC, Q5, NC1, NC2 and C5 are met. **Missed**: C1 (`Re`
`1.07e-2`, limit `5e-3`; measured `Delta Z_pl = -0.00407 - 0.00065 i` against the model's `0.00665 - 0.00107 i`), the Symmetry
parts of C3 (`3.6e-3`), C4 (`7.9e-3`, `5.4e-3`, `2.4e-3` at `alpha_u = 0.6, 0.7, 0.9`) and C6 (`5.2e-3`), the `Re Delta Z_pl` of
C4 at `0.6`, `0.7`, `0.9` (`2.2e-2`, `1.5e-2`, `7.1e-3`) and of C6 (`1.5e-2`), and NC3's bracket (the measured sign change lies
between 0.8 and 0.9, not 0.7 and 0.8). The test is kept as the ignored record
`f538_registered_runs_relaxed_piso_sweep_and_n50_known_miss` (attribute and name only; limits intact).

**Diagnosis (unregistered arithmetic on the logged numbers, `f538-scratch/diag_cx.py`; disclosed).** The misses are all on the
Symmetry side, and the model error of `Re Delta Z_pl` is linear in `1/alpha_u - 1`: `-0.0041, -0.0071, -0.0107, -0.0154, -0.0217`
at `alpha_u = 1.0, 0.9, 0.8, 0.7, 0.6`, i.e. about `-0.0041 - 0.0265 (1/alpha_u - 1)`. **A defect of D1's model**: it drops the
`x`-direction Laplacian of the momentum matrices because on an `x`-uniform field its diagonal `c` and its off-diagonals cancel
(`(c + O_x) u = 0`). They cancel in `D + O`, but not in the splitting: the relaxation scales the **whole** diagonal, `(D + c)/alpha_u`,
and the averaged `a_P` and `HbyA` divide by `D + c + L`. With `D' = D + c` and `O' = O - c` (on `x`-uniform fields) D2 holds with
`D'`: `D' + O' = D + O` (the zero-gradient fixed point does not move, hence the zero-gradient parts met), but `r = e/d` becomes
`e/(d + c)`, smaller, and the relaxed excess falls more steeply with `1/alpha_u`. Per volume `c_i = nu sum_x-faces |S_f|/(d_f V_i)`
(`d_f` the centre distance, half the cell at the piston, nothing at the zero-gradient outlet), which on the graded meshes runs
from `2 nu/(0.25 h)^2 = 50 /s` at the piston to about `1.7 /s` at the outlet; its length-weighted mean `2 nu 2.87/h^2 = 9.0 /s` is
`3 %` of `d = 301 /s`, the size the 0-D formula needs for the measured slope. The model with `c_i` (each `x` column of the mesh as a
developed column with its own `c_i`, `Z = sum_i z(c_i) dx_i`, the per-length part from the two meshes as for the runs; "model X")
was evaluated on the logged runs before this amendment: it reproduces every `Delta Z_pl` within `1e-5`. This is a postdiction, stated
as such; the item is **partial** by the registered rule whatever the amended comparison gives (checks met only after an amendment).

**Amended comparison (A-checks; the registered limits unchanged; no solver run is repeated).** A new fast test
`f538_amended_model_x_against_the_registered_runs` embeds the 29 logged impedances (full digits of `run2_registered.log`) and
compares them with model X, `c_i` computed from the point coordinates of the two `box_mesh` meshes (no free parameter):

| Id | Quantity | Limit (as registered) |
|---|---|---|
| C1A | relaxed `0.8`: `Delta Z_pl` against model X, absolute | real `<= 5e-3`, imaginary `<= 2e-3` |
| C2A | PISO: the same | real `<= 5e-3`, imaginary `<= 2e-3` |
| C3A | `Z_pl` of the four runs of C3 against model X, per part | `<= 2e-3` |
| C4A | sweep: `Re Delta Z_pl` against model X; `Z_pl` per part | `<= 5e-3 + 0.02 abs(Re Delta Z_X)`; `<= 2e-3` |
| C5A | the measured zero against model X's linear interpolation on the same bracket | `<= 0.02` |
| C6A | `N = 50`: `Re Delta Z_pl` against model X; `Z_pl` per part | `<= 5e-3 + 0.02 abs(Re Delta Z_X)`; `<= 3e-3` |
| NC3A | the measured bracket of the sign change equals model X's bracket | holds |
| C8A | the 0-D formula with `d -> d + c_bar` (`c_bar` the per-length mean of `c_i`, from the meshes) against the measured `sigma = Re Delta Z_pl/Re Delta Z_D1` at every `alpha_u` (N 100) and at N 50; its zero against the measured zero of C5A | `<= 0.06`; `<= 0.02` |
| E1A | the 20 h `Delta Z` of model X against the measured 20 h `Delta Z` (the ends' 2-D part not modelled) | record |

Planned mutations MU4 and MU5 are applied to model X as well (the test's own model); MU1 to MU3 (library) are run against the
registered record's fast parts as planned, the measured numbers being the solver's: they are run with the heavy record on a scratch
copy (each mutation once), and model X compared with the mutated runs' logged output by the same arithmetic.

## Addendum 1: results (2026-11-15)

Runs, each once, release profile, `CARGO_BUILD_JOBS=4`, logs in `w49i-work/f538-scratch`: on `6857342e` the model test
(`run1_models.log`, 0.01 s) and the 29 registered channel runs (`run2_registered.log`, 135 s); after amendment 1, on `0985fff5`,
the amended comparison (`run3_amended.log`, arithmetic on the logged impedances); after a clippy-only edit (`4a263303`: a type
alias and fixed-size arrays) the two fast tests once more (`run4_after_lint.log`, every printed number identical to run1 and
run3); the unregistered diagnosis `diag_cx.py`; the mutations (`MU*.log`, `mutations_summary.log`, copy `f538-scratch/copy`,
target `target-mut`, baseline `mut_baseline.log`).

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| A0 | registration first | `a384d6d5` precedes every code commit | yes |
| R0 | `<= 1e-9` | `0` at all four (ADR 0455's QD, QZ, QD10, QZ10 reproduced bit for bit) | yes |
| C1 | real `<= 5e-3`, imaginary `<= 2e-3` | `Delta Z_pl = -0.004069 - 0.000650 i` against `0.006650 - 0.001075 i`: `1.07e-2`, `4.3e-4` | **no** (record) |
| C2 | the same | `0.361988 - 0.013683 i` against `0.363511 - 0.013869 i`: `1.52e-3`, `1.9e-4` | yes |
| C3 | `<= 2e-3` | PISO `5.6e-4`, `6.2e-4` (Symmetry), `1.1e-4`, `4.4e-4`; relaxed zero gradient `7.1e-5`, `2.2e-6`; relaxed Symmetry real **`3.6e-3`**, imaginary `3.9e-4` | **no** (record) |
| C4 | `<= 5e-3 + 0.02 abs(model)`; `<= 2e-3` | `Re Delta Z_pl` errors `2.2e-2`, `1.5e-2`, `7.1e-3`, `4.1e-3` (`alpha_u` 0.6, 0.7, 0.9, 1.0; the first three missed); Symmetry `Z_pl` real `7.9e-3`, `5.4e-3`, `2.4e-3` missed, `1.4e-3` at 1.0; zero gradient `<= 7.7e-5` | **no** (record) |
| C5 | `<= 0.02` | measured zero `0.8049` (bracket 0.8, 0.9) against the model's interpolation on that bracket `0.7916`: `1.3e-2` | yes |
| C6 | `<= 8.0e-3`; `<= 3e-3` | `Re Delta Z_pl = -0.16553` against `-0.15065`: `1.5e-2`; Symmetry real `5.2e-3`; zero gradient `6.9e-5` | **no** (record) |
| C7 | `<= 1e-3`; `<= 1e-3` | CL against dense `2.1e-5` (`alpha_u` 0.6), `<= 1.7e-6` elsewhere; `alpha_p` 0.3, 0.7: `1.3e-4`, `1.3e-5` | yes |
| C8 | `<= 0.06`; `<= 0.02` | `sigma` within `3.8e-2`; zeros `0.7952` against `0.7926` (`2.6e-3`), `0.9061` against `0.9090` (`2.9e-3`) | yes |
| RC | `<= 1e-3` | `2.8e-6`, `3.8e-6` | yes |
| Q5 | `<= 1e-6` | every run `<= 5.1e-12` | yes |
| E1 | record | end parts `Delta Z_end`: PISO `-0.00122`; relaxed `+0.0046, +0.0031, +0.0020, +0.0011, +0.0004` (`alpha_u` 0.6 to 1.0), `+0.0039` (N 50) | recorded |
| NC1 | `>= 0.1` | `0.251`, `0.153`, `0.396` | holds |
| NC2 | `>= 0.1` | `0.161` | holds |
| NC3 | monotone; bracket (0.7, 0.8) | monotone; bracket **(0.8, 0.9)** | monotone holds, bracket missed (record) |
| C1A | real `<= 5e-3`, imaginary `<= 2e-3` | model X `-0.004067 - 0.000649 i`: `2.7e-6`, `3.8e-7` | yes (after amendment) |
| C2A | the same | `8.4e-6`, `6.0e-7` | yes (after amendment) |
| C3A | `<= 2e-3` | all eight parts `<= 7.2e-5` | yes (after amendment) |
| C4A | as C4 | `Re Delta Z_pl` within `5.0e-6`, `6.6e-7`, `5.5e-6`, `7.8e-6`; `Z_pl` `<= 8.0e-5` | yes (after amendment) |
| C5A | `<= 0.02` | `0.804916` against `0.804913`: `3.5e-6` | yes (after amendment) |
| C6A | as C6 | `2.0e-7`; `Z_pl` `<= 7.3e-5` | yes (after amendment) |
| NC3A | brackets equal | (0.8, 0.9) both | holds |
| C8A | `<= 0.06`; `<= 0.02` | `c_bar = 8.96 /s`; `sigma` within `4.4e-2` (0.6), `2.4e-2`, `8.4e-3`, `3.6e-3`, `1.3e-2`, N 50 `5.4e-3`; zero `0.8075` against `0.8049`: `2.6e-3` | yes (after amendment) |
| E1A | record | model X's 20 h `Delta Z` against the measured: relaxed 0.8 `-0.00448` against `-0.00209`; PISO `0.36194` against `0.36077`; the others within `5.5e-3` | recorded |

### What the numbers say

* **(c1, c2) An exact linear model of the relaxed step reproduces both runs.** Model X (D1 with the flow-direction Laplacian
  diagonal `c_i` of each column, amendment 1) gives the measured per-length impedance of the relaxed run and of the PISO run within
  `7.2e-5` per part, and their excess within `2.7e-6` (relaxed) and `8.4e-6` (PISO); across the sweep and at `N = 50` within
  `8e-6`. Even the 20 h `Delta Z`, ends included, comes within `2.4e-3`: the `-0.0021` of ADR 0455 Q4 is model X's `-0.0045` plus a
  2-D end correction. The registered model D1 missed on the Symmetry side by `1.07e-2` (C1), with an error linear in
  `1/alpha_u - 1`; its zero-gradient side met to `7e-5`, which located the defect in the splitting, not in the fixed point.
* **(c3) Explained.** The relaxed loop does not remove the averaged diagonal's sink; it cancels it by accident. At the fixed point of
  the relaxed outer iterations the corrector imposes `u = D (D + L)^{-1} u*`, which turns the predictor into `D + O + L/alpha_u + O
  D^{-1} L` (a sink `lambda/alpha_u` and an anti-sink through the neighbour links); the unrelaxed final iteration with one averaged
  corrector shifts the pressure gradient back by about `lambda` times the predictor's overshoot. The remainder is `sigma lambda`,
  `sigma = r - (1/alpha_u - 1)(1/r - 1)` (C8, C8A); at `alpha_u = 0.8`, `nu dt/dy^2 = 1` it is `-0.010` (measured; the registered
  D1 said `+0.017`). `alpha_p` drops out (C7). The corrected split removes the effect in the relaxed loop as well (RC: `4e-6`).
* **(c4) The dependence on `alpha_u`.** Measured `sigma = -0.64, -0.28, -0.010, +0.20, +0.37` at `alpha_u = 0.6, 0.7, 0.8, 0.9,
  1.0` (`N = 100`), zero at `alpha_u = 0.805` (the 0-D formula with `c_bar`: `0.8075`); at `N = 50` the same `alpha_u = 0.8` gives
  `sigma = -0.42` (`Delta Z_pl = -0.166`, the Symmetry channel's impedance per length 5.4 % **below** the zero-gradient one). The
  relaxed loop's error next to a `Symmetry` plane therefore changes sign with `alpha_u`, with the time step (`r` through `1/dt`)
  and with the flow-direction spacing (`c_x`); `alpha_u = 1` with one corrector keeps `sigma = r`.

### Mutations (copy `f538-scratch/copy`, own target `target-mut`, `mutate.py`; each applied once and reverted)

Library mutations against a scratch-only driver (`mut_test.rs`, appended to the copy only): group R's four runs against model X
with the limits of C1A and C3A; the driver met every limit on the unmutated copy (`mut_baseline.log`, `2.7e-6`). Test-model
mutations against the two fast tests.

| Id | Mutation | Caught by |
|---|---|---|
| MU1 | `pimple.rs`: the last outer iteration relaxed too | C1A (`0.247`), C3A Symmetry (`8.2e-2`): yes |
| MU2 | `LduMatrix::relax`: the source `(d/alpha - diag) x_old` dropped | C3A (`7.4` on both sides), C1A (`2.2e-2`): yes |
| MU3 | `pimple.rs`: `alpha_p` not applied | Q5 (the four runs not periodic, `2.5e-6` to `2.9e-4`): yes, by the periodicity check only |
| MU4 | the test's model: the final iteration relaxed | C7 (`0.24`), C8 (`0.62`), C1A (`0.247`), C3A: yes |
| MU5 | the test's model: the relaxation source from `u_old` | C7, C8, C3A, C4A (`8.4e-2`; `1.44` imaginary): yes |

### Deviations

* The registered model D1 omitted the flow-direction Laplacian (amendment 1); the A-checks compare the amended model with the
  logged impedances of the registered runs, which were not repeated.
* The amended comparison was evaluated in the unregistered diagnosis before amendment 1 was written (disclosed there); the A-checks
  keep the registered limits.
* Amendment 1 said the library mutations would be run "with the heavy record"; they were run with a reduced scratch driver (group
  R only, the four runs MU1 to MU3 act on), 34 s per mutation instead of 135 s.
* The two fast tests were run a second time after the clippy-only commit, as a check of that edit (identical numbers); no
  registered channel run was repeated.

### Not run

* The rest of the rcfd-post suite and `baseline_hashes`: no library line was changed (one new test file, the ADR, a topic page).
* A 2-D model of the ends (the end parts are recorded, E1, E1A); other meshes across the gap, other Womersley numbers,
  `n_correctors > 1` with relaxation.

### Decision by the registered rule

**Partial.** The registered checks C1, C3, C4, C6 and NC3's bracket missed on their registered runs (record
`f538_registered_runs_relaxed_piso_sweep_and_n50_known_miss`, limits intact) and were met only after amendment 1 (model X), which
corrected a defect of the registered model; every A-check is met (excesses within `8e-6`) and all five mutations are caught (one by
the periodicity check only). The registered rule's partial clause asks that C1 and C2 be met; C1 was met only after the
amendment. The cause is located and the amended model reproduces both runs, so the item is not todo; by the rule it is partial.

### Remaining questions

1. **The default.** F-538 adds that the averaged diagonal's error in a relaxed transient run can have either sign and depends on
   `alpha_u`, the time step and the flow-direction spacing. This strengthens ADR 0455 remaining question 1 (make `corrected` the
   default of the transient kernel and re-register `packed_bed`). Not taken here: it changes default results and a registered hash.
   Recommendation: as ADR 0455, option 2 for the transient kernel.
2. **The ends.** Model X leaves `2.4e-3` of the relaxed 20 h `Delta Z` (and `1.2e-3` of the PISO one) to the 2-D end flow. Options:
   (a) a 2-D dense model of the short channel; (b) nothing. Recommendation: (b): the per-length part is reproduced to `1e-5`, and
   the ends do not bear on the default question.
3. **ADR 0455's two-corrector model** (`channel_step_model.py`) has the same omission; with `c_x` its PISO per-length error goes from
   `1.5e-3` to `8.4e-6`. Nothing to decide; recorded for later estimates.

## For the lead: roadmap entries

> Proposed status: partial (F-538).

> Numerics. PARTIAL (ADR 0613, 2026-11-15; registered before the code, amendment 1 after the registered runs). Test-only, no library line changed. The near-zero Symmetry excess of ADR 0455's relaxed PIMPLE run (pimple(nu, 10): alpha_u 0.8, alpha_p 0.5, one corrector; -0.0021 against the PISO step's 0.36) is an accidental cancellation, not a removal: at the fixed point of the relaxed outer iterations the averaged corrector turns the predictor into D + O + L/alpha_u + O D^-1 L (a sink lambda/alpha_u and an anti-sink through the neighbour links), and the unrelaxed final one-corrector iteration shifts the pressure gradient back, leaving sigma = r - (1/alpha_u - 1)(1/r - 1) of the infinite-corrector sink, r = (1/dt + 12 nu/h^2)/(1/dt + 2 nu/dy^2 + c_x); alpha_p drops out. Measured per-length excess (QF channel at 20 h and 40 h, 29 runs): sigma -0.64, -0.28, -0.010, +0.20, +0.37 at alpha_u 0.6 to 1.0 (N 100), zero at alpha_u 0.805 (0-D formula 0.8075); at N 50 the same alpha_u 0.8 gives sigma -0.42 (the Symmetry channel 5.4 percent below the zero-gradient one). An exact linear model of the relaxed step on the developed channel, with each flow-direction column's Laplacian diagonal c_x (it enters through the relaxation and a_P), reproduces the per-length impedance of the relaxed and the PISO runs within 7.2e-5 per part and every excess of the sweep within 8e-6; ADR 0455's printed impedances reproduced bit for bit; the opt-in corrected split removes the effect in the relaxed loop too (4e-6). Partial: the registered model omitted c_x and missed C1 by 1.07e-2 (limit 5e-3) and the Symmetry parts of C3, C4, C6 (record f538_registered_runs_relaxed_piso_sweep_and_n50_known_miss); met only after amendment 1. Five of five mutations caught (one only by the periodicity check). Open: ADR 0613 Q1 (the transient default, as ADR 0455 Q1).

F-531 (note addition):

> ADR 0613 (F-538, 2026-11-15): the relaxed PIMPLE loop's near-zero Symmetry excess is an accidental cancellation at alpha_u 0.8 and nu dt/dy^2 = 1 (sigma = r - (1/alpha_u - 1)(1/r - 1)); at N 50 the excess is -0.42 of the sink; ADR 0455's two-corrector model gains the flow-direction Laplacian diagonal and then meets the PISO per-length part to 8.4e-6.

No follow-up is proposed (remaining question 2 recommends leaving the 2-D ends).

## Answers of the maintainer (2026-11-18), written into this ADR by the lead on 2026-11-19

The maintainer decided these on 2026-11-18 as sets of options with a recommendation; until now the decision stood in the roadmap note of the item only. Nothing above this section is changed.

- **F-538, remaining question 1 (the default; first asked in ADR 0455, remaining question 1).** Asked: `component_diagonal = corrected` as the default of the transient kernel, with the `packed_bed` hash re-registered. Answer: Held. The default change is not approved now: it would move existing results and waits until the fan path needs it. The other open questions of this ADR are closed by the lead on their written recommendations (lead-process.md, "Questions asked before 2026-11-11").
