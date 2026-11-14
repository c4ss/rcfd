# ADR 0451: F-530 registration: stability of the non-iterated PISO step at large diffusion numbers

Status: **REGISTERED 2026-11-14, written before any code of F-530 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0446](0446-f421-registration-aerodynamic-damping-energy-method.md) (F-421: the plane channel driven by an
oscillating inflow, amendment 2: the divergence of the PISO step above `nu dt/dy^2` of about 3.5, not located there),
[ADR 0214](0214-f299-registration-piso-instability-at-large-cell-diffusion-number-across-a-thin-gap.md) (F-299: a PISO
instability across a thin gap with the same onset, its mechanism M1, M2 and the opt-in consistent correctors F0),
[ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) (the pressure-lag estimate B4), [ADR 0434](0434-f401-registration-piston-on-an-elastic-bar.md)
(the viscous splitting VS mentioned by ADR 0446 D4). Roadmap item F-530 (deps F-421). Number 0451 given by the lead. Branch
`f530-piso-stability` (from main `f3ab9970`), worktree `w49i-work/f530`, scratch `w49i-work/f530-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; defaults bit-identical; no edit of `baseline_hashes.rs`, the
hash tables or a golden file; existing tests are not edited; no new dependency, no unsafe code; clean-room (the corrector algebra
is that of ADR 0214, derived there from the SIMPLEC idea of Van Doormaal and Raithby, Numer. Heat Transfer 7 (1984) 147, cited from
memory as ADR 0214 does; the oscillating-channel impedance is derived in ADR 0446 D3; nothing else is taken from literature).

## What F-530 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. The divergence
of the PISO step above nu dt/dy^2 of about 3.5 found by ADR 0446 (a plane channel driven by an oscillating inflow on a fixed mesh)
is located in the step and either removed, so that the same channel at nu dt/dy^2 = 25 and 100 stays bounded over 3 periods and
matches the oscillating-channel impedance as the stable runs do, or refused by the step with a message naming the diffusion
number; the default results of every existing case stay bit-identical."

Clauses: (c0) every other limit registered here; (c1) the divergence is located in the step; (c2a) removed: the same channel at
`D = 25` and `D = 100` bounded over 3 periods, and (c2b) matching the oscillating-channel impedance as the stable runs do; or (c2')
refused by the step with a message naming the diffusion number; (c3) the default results of every existing case bit-identical.
This registration takes the branch (c2a, c2b) for the reason given in "The choice" below; (c2') is not built.

## What was read (no Rust run)

ADR 0446 (whole, with amendments 2 and 3 and the diagnosis code `f421-scratch/sa_aerodamp_with_diag.rs`: `zz_diag4` to `zz_diag6`,
the fixed-mesh inflow runs), ADR 0214 (the code read, M1 to M3, F0, the set-ups, the results and the reading), the comments of
`LagCheck` and `PimpleSettings::consistent_correctors` in `crates/rcfd-fv/src/pimple.rs`, `PimpleSettings::piso`, the momentum
assembly of `step_with` (the convection matrix plus `laplacian_opt`, the transposed stress only with `full_viscous_stress`), the
B4 estimate (`lag_due`, `lag_amplification_n`, `lag_estimates`), `crates/rcfd-post/tests/sa_aerodamp.rs` (the channel, the
impedance formula, `Checks`), `rcfd_post::solid_aerodamp::periodic_response`, `rcfd_fv::sixdof::patch_face_forces`,
`FlowWall::advance`. Nothing was built or run in Rust for this item.

**Arithmetic before this registration (disclosed).** Two Python scripts in `w49i-work/f530-scratch` (`prearith.py`,
`prearith2.py`; numpy; they reuse the models `Zc` and `Zfv` of `f421-scratch/prearith.py`): the discrete impedance of D2 below
at `Wo = 1`, 40 cells across, `N = 25, 50, 100, 1000, 2000` steps per period and its extrapolations; the fast set-up's value; the
step counts for given diffusion numbers; the first cell of the grading (`Expansion(21)` on 120 cells over `100 h`: `0.126 h`,
last `2.65 h`). The tests compute their own predictions; no printed number is pasted into a test.

## The code read: what the step does and what it does not

The PISO step of `PimpleSolver::step_with` with `PimpleSettings::piso` (`n_outer = 1`, `n_correctors = 2`, `alpha_u = alpha_p
= 1`, implicit Euler): the momentum predictor with the viscous term **implicit** (`laplacian_opt` assembled into the matrix with
the half-cell wall coefficient; the explicit transposed stress only with `full_viscous_stress`, off; the channel's mesh is
orthogonal, so no deferred non-orthogonal part, and `non_orth_correctors = 0`), then two correctors with `rAU = V/a_P` (ADR 0214's
code read, item 1). There is no under-relaxation in the PISO step (`alpha = 1`). Terms that scale with `dt`: the transient
Rhie-Chow term `gamma_p/dt * defect` (ADR 0446 amendment 2 ruled it out: `ddt_correction = false` diverges as well) and the
diagonal `a_P = V/dt + viscous links`, through which `rAU` and the corrector's pressure depend on the diffusion number. So of the
candidates of the brief (an explicit viscous term, deferred correction, under-relaxation, the Rhie-Chow term, the viscous
splitting), only the corrector algebra with `rAU` remains in the step for a fixed orthogonal mesh at rest, and that is exactly the
mechanism ADR 0214 located across a thin gap:

* **M1** (ADR 0214): the first corrector over-corrects a pressure error by about the cross-gap diffusion number `D`, because it
  uses `rAU = 1/a`, `a = (1 + 2 D)/dt`, while the column answers a pressure gradient uniform across the gap with about `dt`.
* **M2** (ADR 0214): on the collocated mesh the cell update with the Green-Gauss gradient leaves the Rhie-Chow part `sin^2(k dx/2)`
  of a short pressure wave along the gap in the cell velocity, which the second corrector amplifies again through the cross-gap
  links.

ADR 0214's measured onset with two correctors lies between `D = 2.98` and `3.97` (its uniform gap of 16 cells across and its
falling-block mesh); ADR 0446 found the channel stable at `D = 3` and divergent at `D = 4` on 40 cells, and the viscous splitting VS
of ADR 0434 is an accuracy effect of the step, not a growth. **Hypothesis H (this registration):** the divergence of ADR 0446 is
the instability H-RC of ADR 0214 (M1 with M2), with the gap of the channel the gap of ADR 0214 and the long direction `x` the
direction along it. The diagnosis checks DG below test it on the solver itself before the choice between removal and refusal is
used (their outcome decides the item, not the choice, which is made now from the code read).

## The choice: removal by the existing opt-in, no library change

If H holds, the remedy already exists: `PimpleSettings::consistent_correctors` (ADR 0214 F0, default `false`, bit-identical), whose
corrector error for a mode of symbol `lambda` is `lambda dt/(c0 + lambda dt)` in `(0, 1)` for any `D` (ADR 0214 R5 with the
option), and which is defined on fixed and moving meshes (ADR 0214 F0) and in the B4 estimate. The removal branch (c2a, c2b) is
then met by the step with that option, and no library line is changed, so (c3) holds by construction (no file of `rcfd-fv`,
`rcfd-api` or another library crate is edited; if a library edit turns out to be needed it is made opt-in under an amendment and
`baseline_hashes` is run). Making the option the default would change the default results of every PISO case: not taken; recorded
as an open question. A refusal (c2') by default would fire only where the default diverges, but "the step diverges" is not a
property of `D` alone (ADR 0214: the onset depends on the corrector count and the mode needs the thin-gap structure; a cell
diffusion number of 25 is harmless along a coarse direction), so a refusal keyed on `D` would stop cases that run today. Not
built; an opt-in refusal is listed as an open question.

Documentation: the PISO limit and the option go into the topic documentation that describes the coupling (see "Not delivered" if
no such page applies); `rcfd-cli`'s `docs_site` test is run if a page under `docs/` is changed.

## Set-ups

Constants of ADR 0446 D6: `h = 0.02 m`, depth `h` (one cell), `rho = 1000 kg/m^3`, `w = 2 pi rad/s` (`T = 1 s`), `Wo = 1`, `nu =
w (h/2)^2/Wo^2 = 6.28319e-4 m^2/s`, amplitude `q0 = 1e-3 h`, `V0 = q0 w`.

**FC (the fixed-mesh channel; "the same channel").** `box_mesh([120, 40, 1], [L_f, h, h], [Expansion(21), Uniform, Uniform])`, `L_f
= 100 h`, not moved; `ymin`, `ymax` velocity `FixedValue` zero; `zmin`, `zmax` no condition (the default zero gradient, ADR 0446
amendment 3); `xmax` pressure `FixedValue` 0, velocity the default zero gradient; `xmin` the inflow: before step `n` the velocity
`FixedValue` `(v_n, 0, 0)`, `v_n = (q_n - q_{n-1})/dt`, `q_n = q0 sin(w n dt)` (the flux of the piston of ADR 0446 D4, so the
discrete model D2 is that of ADR 0446). Settings: ADR 0446's `settings(nu)`: `PimpleSettings::piso(nu)`, central convection,
momentum and pressure solvers `tol_abs 1e-16`, `tol_rel 1e-13`, `max_iter 5000`; the option: the same with
`consistent_correctors = true`. From rest; `dt = T/N`; `D = nu dt/dy^2 = (pi/2) 40^2/(Wo^2 N)`: `N = 25` gives `D = 100.53`, `N =
50` `50.27`, `N = 100` `25.13`, `N = 500` `5.03`, `N = 1000` `2.51`, `N = 2000` `1.26`.

The force: after step `n`, `Q_n` = the `x` component of the sum of `rcfd_fv::sixdof::patch_face_forces(geom, u, p, rho, nu,
["xmin"], None)` (the load of the flow on the inflow plane, `rho [p S - nu (grad u + grad u^T) . S]`; the pressure there is the
cell value, the default zero gradient: half a first cell `0.063 h` off the plane, a pressure difference of `6e-4` of the force per
`100 h`, not corrected). `periodic_response(Q, N, q0, w, 1e-6)` of `rcfd_post::solid_aerodamp` gives `Z = c + i w m_a` from the
last cycle (ADR 0446 D1; refused if the first harmonic of the last two cycles changes by more than `1e-6`). 3 periods (`3 N`
steps). The inflow plane replaces the piston: on the fixed mesh the domain does not move by `q0 = 1e-3 h`, a geometric effect of
order `q0/h` in the second harmonic and `(q0/h)^2` in the first (ADR 0446 D6), and the force is the same integral.

"Bounded" (A1): every step returns without a trip of the divergence guard (`diverged` stays `None`), every velocity finite, and
`max_c |u_c| <= 10 V0` after every step (the developed oscillating profile at `Wo = 1` peaks near `1.5 V0`; the plug at the inflow
`V0`). "Not bounded": any of these fails within the 3 periods.

**FQ (the fast set-up).** As FC with `box_mesh([30, 16, 1], [20 h, h, h], [Expansion(5.5), Uniform, Uniform])` (ADR 0446's
diagnosis channel), `N = 16` (`D = 25.13`), 2 periods.

**AP (the piston of ADR 0446, a record).** `channel_run` of `sa_aerodamp.rs` (copied: `FlowWall` with `ChannelMotion`, the
prescribed wall gradient, `drive_mode`) on the channel of D6 with the `z` planes at the default (amendment 3), `Wo = 1`, 40 cells
across, `N = 50` and `100`, 3 cycles, with `consistent_correctors = true`. ADR 0446's registered W1 level `N = 100` (`D = 25.13`)
diverged there.

**B4 estimates.** `settings.pressure_lag_check = Some(LagCheck::new(0, LagAction::Warn))` (step 1 only, 20 power iterations),
one step of FC from rest, `rho = lag_estimates[0].rho`.

## D2. The discrete model (ADR 0446 D4, recomputed in the test)

Implicit Euler on the cell-centred profile across the gap (half-cell wall coefficient, `n_y` cells): with `s = (1 -
e^{-i w dt})/dt`, `u = G (s I - nu L_y)^{-1} 1`, `G` such that the mean of `u` is one; per unit piston velocity `Z_dt = A rho L_f
G(s) s/(i w)`. The test solves the tridiagonal complex system itself (Thomas algorithm on a pair of reals). It is exact for the
developed parallel flow with the step's time discretisation; it leaves out the ends (ADR 0446 D3, measured 0.2 % at `Wo = 5`,
amendment 3) and the splitting of the correctors. Values against the formula `A i w rho L_f/(1 - tanh(kappa)/kappa)` (FC):

| `N` | `D` | real | imaginary |
| --- | --- | --- | --- |
| 25 | 100.5 | `+8.68e-2` | `-3.51e-1` |
| 50 | 50.3 | `+4.60e-2` | `-1.68e-1` |
| 100 | 25.1 | `+2.31e-2` | `-8.21e-2` |
| 1000 | 2.51 | `+1.28e-3` | `-8.85e-3` |
| 2000 | 1.26 | `+3.3e-5` | `-4.89e-3` |

Extrapolations (Richardson of order 1, ADR 0446 D4): `R_c = 2 Z_100 - Z_50`: `+2.59e-4`, `+3.48e-3`; `R_d = 2 Z_2000 - Z_1000`:
`-1.21e-3`, `-9.27e-4`; `|R_c - R_d|/|R_d| = 2.1e-3`; observed order from `N = 25, 50, 100`: 1.028. FQ at `N = 16`: the model
`Z_dt` is the reference (its distance from the formula, `+0.117`, `-0.577`, is the time step's).

## Registered checks

Every check is run once on the registered set-up. The DG checks are run on the library of `f3ab9970` (no library change is
planned at all).

| Id | Clause | What | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| DG0 | c1 | FC, default, `N = 100` (`D = 25.1`) and `N = 25` (`D = 100.5`), 3 periods | not bounded, each | ADR 0446 amendment 2: divergent at `D = 25` |
| DG1 | c1 | B4 estimate, FC default, `N = 100` | `3.5 <= abs(rho) <= 6.5` | ADR 0214 M3/D3: `4.99` at `D = 23.8` (gap of 16 cells, rows `2h`); the rate depends little on rows above `h/2`; +-30 % for 40 cells across and the graded rows |
| DG2 | c1 | B4 estimate, FC default, `N = 1000` (`D = 2.51`) and `N = 500` (`D = 5.03`) | `abs(rho) < 1` and `abs(rho) >= 1` | ADR 0214: onset between `D = 2.98` and `3.97`; ADR 0446: stable at 3, divergent at 4 |
| DG3 | c1 | growth in DG0's `N = 100` run: `e_n = |u_n - u_{n-1}|_2` over the cells; the 5 ratios `e_n/e_{n-1}` after the first step with `max |u| >= 100 V0` | geometric mean within 25 % of DG1's `abs(rho)`; `<u_n - u_{n-1}, u_{n-1} - u_{n-2}> < 0` at each of the 5 steps (alternating) | ADR 0214: twin-run radius against B4 within 4.4 % (3.159 against 3.302); the mode of H-RC alternates every step |
| DG4 | c1 | B4 estimate, FC with the option, `N = 100` and `N = 25` | `abs(rho) < 1` each | ADR 0214 F1/F3: 0.92 to 0.95; R5: in `(0, 1)` for any `D` |
| A1 | c2a | FC with the option, `N = 25` (`D = 100.5`) and `N = 100` (`D = 25.1`), and `N = 50`, 3 periods | bounded, each | ADR 0214 F2: bounded at `D = 11.9` with the option |
| A2 | c2b | the same runs periodic | cycle change `<= 1e-6` (refusal of `periodic_response` otherwise) | transient at `Wo = 1`: `1e-55` per period (ADR 0446 D4) |
| A3 | c2b | each of the three levels against the discrete model `Z_dt` (D2) | real and imaginary parts within `3e-2` each | ends (`< 1 %`, ADR 0446 amendment 3), pressure half-cell `6e-4`; the corrector splitting not derived (ADR 0214 A1c: `+0.55 %` on its block at `D = 11.9`) |
| A4 | c2b | `R_c = 2 Z_100 - Z_50` against the formula | real and imaginary parts within `3e-2` each (ADR 0446's limit for the stable runs) | D2: `+2.6e-4`, `+3.5e-3`, plus A3's parts |
| A5 | c2b | the stable runs: FC default, `N = 1000` and `2000`, 3 periods; `R_d = 2 Z_2000 - Z_1000` against the formula | bounded; periodic; within `3e-2` each | D2: `-1.2e-3`, `-9.3e-4` |
| A6 | c2b | "as the stable runs do": `abs(R_c - R_d)/abs(R_d)` | `<= 1e-2` | D2: `2.1e-3`; the remainder the corrector splitting at large `D` |
| O | companion | observed order from `N = 25, 50, 100`, real and imaginary parts, `log2(abs(Z_25 - Z_50)/abs(Z_50 - Z_100))` per part | `0.8 <= p <= 1.25` | D2: 1.028 (modulus) |
| QF1 | fast | FQ default | not bounded within 2 periods; B4 `abs(rho) >= 1` | DG0, DG1 |
| QF2 | fast | FQ with the option | bounded; B4 `abs(rho) < 1`; periodic `<= 1e-6`; real and imaginary parts within `5e-2` of its `Z_dt` | A1 to A3; the ends at `20 h` five times those at `100 h` |
| AP | record | the piston (ADR 0446's W1 at `N = 100`) with the option, `N = 50, 100` | bounded (no error from `drive_mode`), periodic; `2 Z_100 - Z_50` within `3e-2` of the formula each part | as A4 (the moving wall adds `(q0/h)^2`) |
| N1 | c3 | `git diff f3ab9970 -- crates/*/src` empty (no library line changed) | empty | the choice |
| N2 | c3 | the existing tests that cover the option are untouched; `sa_aerodamp.rs` untouched | `git diff` of `crates/*/tests` adds files only | rules |

Printed records, no limit: B4 at FC default `N = 100` with 3 and 4 correctors and with `ddt_correction = false`; B4 at FC default
`N = 838, 718, 628` (`D = 3.0, 3.5, 4.0`, the onset); the per-level `Z` and run times.

If N1 has to fail (a library edit becomes necessary under an amendment), N1 is replaced by `cargo test -p rcfd-api --release
--test baseline_hashes` passing and the `rcfd-fv` suite of the touched file passing.

## Negative controls

* DG0 and QF1 are the negative controls of A1 and QF2 (the same set-up without the option must not be bounded).
* NC1: the A3 comparison of the `N = 25` level against the formula instead of `Z_dt` must miss (D2: `-0.351` imaginary): shows that
  A3 tests the model and not a loose limit.
* NC2: DG3's sign test on the base run of A1 at `N = 100` (the consistent run, smooth in time) must not show alternation at 5
  successive steps of its last period.

## Planned mutations (scratch copy, not committed)

* MU1 (library, `pimple.rs`): in the consistent correctors, the coefficient of the new pressure gradient in the cell velocity
  `rAtU` replaced by `rAU` (the default's). Expected: caught by A1 or A3 (QF2 for the fast test).
* MU2 (library): `rAtU` computed with `H1 = 0` (so `rAtU = rAU`: the option reduces to the default algebra plus a zero
  compensation). Expected: caught by DG4 / QF2 (B4 and boundedness).
* MU3 (test): `s = i w` in the discrete model (no time discretisation). Expected: caught by A3 (and QF2).
* MU4 (test): the force taken on `xmax` instead of `xmin`. Expected: caught by A3, A4, QF2.
* MU5 (test): the inflow `v_n = q0 w cos(w n dt)` (the analytic velocity at `t_n` instead of the step's flux). Expected: caught by
  A3 at `N = 25` (a phase of `w dt/2 = 0.126 rad` rotates part of the real into the imaginary part, larger than `3e-2` of the
  imaginary part); a mutation not caught is reported as such.

## Decision rule

* **done**: DG0 to DG4 met (H holds: the divergence located), A1 to A6 met, O met, QF1 and QF2 met, N1 and N2 met, every
  mutation caught, every check met on its registered run without an amendment.
* **partial**: the divergence located (DG0 to DG4 met) and removed (A1 met), but a check of c2b missed, O missed, a mutation not
  caught, or a check met only after an amendment made after its run.
* **todo**: DG does not locate it (H rejected and no other cause shown), or A1 misses (the option does not remove it).

## Limits (not covered)

* Only `Wo = 1`, 40 cells across, one length; the low-frequency case of ADR 0446 (LF, `D = 100` to `400` on its registered levels)
  is not run.
* The cost of the option at large `D` is printed (run times), not limited.
* No change to the default numerics; the onset of the default stays (documented, not removed).
* The moving piston (AP) is a record; F-421's registered W1 and LF results are not revisited.
* The `Symmetry` dissipation of ADR 0446 amendment 3 (F-531) is not addressed; the set-ups avoid `Symmetry`.

## Amendment 1 (2026-11-14, written after the registered runs QF, DG, A and AP on commit ae8fa02f and four unregistered diagnoses, before any corrected run)

**What the registered runs showed** (each made once, `cargo test -p rcfd-post --release --test piso_diffusion_f530 <name> --
[--ignored] --nocapture`; logs `f530-scratch/run1_fast.log`, `run2_dg.log`, `run3_a.log`, `run4_ap.log`):

* **QF**: QF1 met (the default not bounded at step 7, B4 `5.010`); QF2 bounded (max `|u|` `1.444 V0`), B4 with the option `0.952`
  (met), but **not periodic**: the change between cycles 1 and 2 is `3.89e-3` (limit `1e-6`; missed, no impedance).
* **DG**: DG0 met (default not bounded at step 5 at `D = 25.1`, step 4 at `D = 100.5`); **DG1 missed** (B4 `9.232`, limit `3.5` to
  `6.5`); **DG3 missed** in its growth part (geometric mean of the 5 ratios `4.13` against B4 `9.23`: `0.553`, limit `0.25`), its
  sign part met (every dot negative: the step's growth alternates); DG2 met (`0.962` at `D = 2.51`, `2.036` at `D = 5.03`); DG4
  met (`0.956` at `D = 25.1`, `0.985` at `D = 100.5`). Records: 3 correctors `5.187`, 4 correctors `3.794`, no Rhie-Chow time
  term `9.232`; `D = 3.0, 3.5, 4.0`: `1.172, 1.388, 1.603`.
* **A**: A1 met at every level (bounded over 3 periods at `D = 100.5, 50.3, 25.1`, max `|u|` `1.493, 1.499, 1.498 V0`); A2 met at
  `N = 50` (`1.43e-7`) and `100` (`4.2e-9`), **missed at `N = 25`** (`1.005e-6`, limit `1e-6`), so A4, A6, O and NC1 were not
  formed; **A3 missed** in the imaginary part (`N = 50`: `-1.428e-1`, `N = 100`: `-3.083e-2`; the real parts `1.65e-2` and
  `9.5e-3` met); NC2 held (longest run of negative dots 1); A5 met (`R_d` `+4.68e-3`, `-1.39e-3`). From the logged `N = 50, 100`
  values (not a registered measurement): `R_c` would be `+2.5e-3`, `+6.6e-2` against the formula, `|R_c - R_d|/|R_d| = 2.5e-2`.
* **AP**: bounded and periodic at `N = 50, 100` (the registered W1 level of ADR 0446 runs with the option); `2 Z_100 - Z_50`
  real `+9.2e-4` (met), **imaginary `+6.53e-2` (missed)**.

**Unregistered diagnoses** (temporary tests, removed again; the code is kept in `f530-scratch/piso_with_diag.rs` and
`piso_with_diag2-4.rs`, logs `diag1.log` to `diag4.log`):

1. B4 with 20, 60, 200 power iterations: `9.232, 9.357, 9.399` on FC, `5.010, 5.004, 5.003` on FQ (16 cells across, the same
   `D`), `8.95, 9.00, 9.06` on FC with uniform `x` cells; the largest entries in the wall rows. The radius depends on the number
   of cells across, not on `D` alone: ADR 0214's 16-cell value was not a valid estimate for 40 cells (**DG1's estimate was
   wrong; the miss stays**). The run's ratios `e_n/e_{n-1}` over the whole run: FC `0.64, 0.81, 1.47, 3.27, 3.78, 3.95, 4.03, ...,
   4.52, 5.43` and then the non-linear blow-up; uniform `x` reaches `8.1` to `8.3`. The 5 steps after `100 V0` (step 7) are not a
   converged power iteration of the step map: **DG3's growth part was a defective set-up** (corrected as DG3c below).
2. The option with 2, 3, 4, 8 correctors at `N = 50`: imaginary part against `Z_dt` `-14.3, -14.0, -13.7, -12.7 %`, B4 `0.975,
   0.965, 0.956, 0.919`; at `N = 100` `-3.08` to `-2.59 %`.
3. At `N = 50` with the option: without the Rhie-Chow time term unchanged (`-14.28 %`); uniform `x` `-13.99 %`; the channel at
   `200 h` gives per length `Z(200 h) - Z(100 h)` `-14.0 %` and ends `2 Z(100 h) - Z(200 h)` of `0.5 %` of `Z`: the loss is in the
   developed flow, not at the ends.
4. At `N = 50` with the option: 32 correctors, one outer iteration: `+1.22e-2`, `-9.04e-2` (B4 `0.726`, 98 s); **2 correctors and 8
   outer iterations** (no relaxation, `alpha = 1`): `+5.53e-3`, `-9.4e-4` (B4 `0.852`, 63 s).

**The location (c1), as now understood.** (i) The divergence is the corrector instability H-RC of ADR 0214: it alternates every
step (DG3 sign), B4 exceeds 1 above an onset between `D = 2.5` and `3.0` (DG2 and the records; the runs of ADR 0446 were stable at
`D = 3` for their length), more correctors only lower it (`9.23, 5.19, 3.79` for 2, 3, 4), the Rhie-Chow time term and the
grading play no part, and the consistent correctors of ADR 0214 F0 remove it at every `D` tried (DG4, A1, AP). Its rate grows
with the cells across at a given `D`. (ii) **A second property of the non-iterated step**, found by A3: at large `D` the step
does not converge the pressure-velocity coupling within the step. With the option each corrector removes only a few per cent of
a smooth pressure error (B4 `0.92` to `0.98` with 2 to 8 correctors), because a corrector does not re-solve the momentum
equation, and the pressure that the step applies lags. That costs the response its inertia (imaginary part `-14 %` at `D = 50`,
`-3 %` at `D = 25`, falling faster than `dt`). Outer iterations, which re-solve momentum with the corrected pressure, remove it
(diagnosis 4). The registered "removal" is therefore only half met by the option alone: bounded, but not matching the impedance
at `D >= 25`.

**Corrected and added checks (the limits of the registered checks unchanged; the registered tests stay as ignored records under
`*_known_miss` names):** `qf_fast_channel_default_diverges_consistent_bounded` becomes `..._known_miss` (ignored);
`dg_diagnosis_record`, `a_removal_and_stable_reference_record` and `ap_piston_consistent_record` become `dg_diagnosis_known_miss`,
`a_removal_and_stable_reference_known_miss`, `ap_piston_consistent_known_miss` (names only).

* **The cycle rule** (periodicity of a run with a slow numerical mode): with `rho_c` the B4 estimate (20 iterations) of the run's
  settings at its `N`, the run has `C = max(3, 2 + ceil(ln(1e-7)/(N ln rho_c)))` periods (a mode of relative size one decays to
  `1e-7` between the last two cycles: margin 10 on the tolerance `1e-6`); refused if `rho_c >= 1` or `C > 80`. Boundedness is
  checked over every period, and "bounded over the first 3 periods" is the acceptance's clause. Estimate: QF's change `3.9e-3`
  after one period at `rho_c^16 = 0.455`.
* **QFc** (fast; replaces QF as the fast test of the option's path): FQ with the option, `N = 16`, `C` by the rule: bounded,
  periodic `<= 1e-6`; the impedance printed (its accuracy is A3's miss; no limit). QF1 is repeated in it unchanged.
* **QFo** (fast; the outer iterations): FQ with the option and `n_outer = 8` (2 correctors, no relaxation), `N = 16`, `C` by the
  rule: bounded, periodic `<= 1e-6`, real and imaginary parts within `5e-2` of `Z_dt` (QF2's limit; diagnosis 4 measured
  `+5.5e-3`, `-9e-4` on FC at `N = 50`; FQ's ends at `20 h` are of order 1 %).
* **DG3c** (the corrected growth measurement): ADR 0214's twin-run power iteration TW on the solver itself, FC default at `N =
  100`: base state `S` after `n_b = 2` steps; `K = 60` iterations; the direction `d_0` the xorshift field of ADR 0214 (seed
  `0x2545F4914F6CDD1D`, values in `[-0.5, 0.5)`) in the cell pressure and the `x`, `y` velocity, zero in the face flux; iteration
  `k`: the state `S + eps d_k` (the velocity and its old level, the pressure, the flux and its old level perturbed alike; `eps` such
  that the perturbation has `1e-7` of the norm of `S`) stepped once with the inflow of step 3, the base step from `S` subtracted,
  `T d_k` the difference over `eps`; norm `|d|^2 = sum_c V_c (|d_u|^2/V0^2 + d_p^2/P^2)`, `P = max_c |p_c(S)|`; `|rho_TW| =
  exp(mean ln(|T d_k|/|d_k|))` and the sign `mean sign(<T d_k, d_k>)` over `k = 30 .. 59`. Limits: `|rho_TW|` within 10 % of the
  B4 estimate with 200 iterations of the same step (ADR 0214: within 4.4 % on its meshes); sign `<= -0.5`.
* **Ao** (the removal with outer iterations): FC with the option and `n_outer = 8` (2 correctors, `alpha_u = alpha_p = 1`), `N =
  25, 50, 100`, `C` by the rule. **Disclosure**: `n_outer = 8` is the value of diagnosis 4, made at `N = 50` after the registered
  runs; it is not derived, and the `N = 50` level is not an independent test of it. Checks with the registered limits: A1o
  bounded over the first 3 periods and over all `C`; A2o periodic `<= 1e-6`; A3o each level within `3e-2` of `Z_dt` (real and
  imaginary); A4o `R_c = 2 Z_100 - Z_50` within `3e-2` of the formula; A6o `|R_c - R_d|/|R_d| <= 1e-2`, with `R_d` from the
  default `N = 1000, 2000` runs of A5 made again in the same test (deterministic; their `Z` must equal A5's logged values, which
  the addendum compares); Oo the order from the three levels, `0.8` to `1.25` per part; NC1o the `N = 25` level against the
  formula must miss `3e-2`. Estimates: D2 (`R_c` `+2.6e-4`, `+3.5e-3`; per level the model exactly) plus diagnosis 4 (`+5.5e-3`,
  `-9e-4` at `N = 50`): A3o, A4o `<= 1e-2`; A6o `2.1e-3` plus the ends' difference.
* **APo** (record): the piston of AP with the settings of Ao, `N = 50, 100`, `C` by the rule: bounded and periodic, `2 Z_100 -
  Z_50` within `3e-2` of the formula each part. ALE with outer iterations is the path of `step_ale_with_walls`; if it refuses, the
  refusal is recorded.

**Mutations**: unchanged (MU1 to MU5), run against the corrected tests (QFc, QFo, Ao); a mutation of the outer-iteration count is
added: **MU6** (test) `n_outer = 8` replaced by `1` in the settings of QFo and Ao; expected: caught by QFo and A3o.

**Decision**: the registered rule applies; DG1, DG3 (growth), QF2 (periodic), A2 (`N = 25`), A3 (imaginary) and AP (imaginary)
missed on their registered runs, so the item is **partial** whatever the corrected runs give. A1 (the boundedness of the removal at
`D = 25` and `100` over 3 periods) was met on the registered run.

## Addendum 1: results (2026-11-14)

Commits on `f530-piso-stability`: `53765c27` (this registration), `ae8fa02f` (the registered tests), `c7b3a7cd` (amendment 1),
`13314def` (amendment 1's tests, the registered tests renamed `*_known_miss`), and the last commit (two clippy rewrites of
amendment 1's test code without change of behaviour, the topic documentation, this addendum). Every run in release
(`CARGO_TARGET_DIR=f530-scratch/target`); logs in `w49i-work/f530-scratch`. **No library line changed** (N1), so the defaults are
bit-identical by construction; `baseline_hashes` was therefore not required and not run.

### Results

Registered checks (each run made once, on `ae8fa02f`):

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| DG0 FC default, `D = 25.1` and `100.5` | not bounded | not bounded at step 5 and step 4 | yes |
| DG1 B4 default, `D = 25.1` | `3.5` to `6.5` | `9.232` | **no** (`dg_diagnosis_known_miss`) |
| DG2 B4 default, `D = 2.51` / `5.03` | `< 1` / `>= 1` | `0.962` / `2.036` | yes |
| DG3 growth against B4 | within 25 % | `0.553` (`4.13` against `9.23`) | **no** (defective set-up, amendment 1) |
| DG3 alternation | every dot `< 0` | all 5 negative | yes |
| DG4 B4 with the option, `D = 25.1` / `100.5` | `< 1` | `0.956` / `0.985` | yes |
| A1 option bounded over 3 periods, `D = 100.5, 50.3, 25.1` | bounded | bounded; max `|u|` `1.493, 1.499, 1.498 V0` | yes |
| A2 periodic, `N = 25, 50, 100` | `<= 1e-6` | `1.005e-6` (refused), `1.43e-7`, `4.2e-9` | **no** at `N = 25` |
| A3 against `Z_dt`, `N = 50`, `100` | `3e-2` each part | real `1.65e-2`, `9.5e-3`; imaginary `1.428e-1`, `3.083e-2` | **no** (imaginary) |
| A4, A6, O, NC1 | | not formed (`N = 25` refused) | **not run** |
| A5 default `N = 1000, 2000`, `R_d` against the formula | bounded, periodic, `3e-2` | bounded; `1.2e-12`, `1.0e-10`; `+4.68e-3`, `-1.39e-3` | yes |
| NC2 no alternation, option `N = 100` | longest run `< 5` | 1 | holds |
| QF1 FQ default | not bounded; B4 `>= 1` | not bounded at step 7; `5.010` | yes |
| QF2 FQ option | bounded; B4 `< 1`; periodic; `5e-2` | bounded; `0.952`; change `3.89e-3` (refused) | **no** (periodic) |
| AP piston, option, `2 Z_100 - Z_50` | bounded, periodic; `3e-2` each part | yes; real `+9.2e-4`, imaginary `+6.53e-2` | **no** (imaginary) |
| N1 no library line | empty | empty | yes |
| N2 tests: new file only | | only `crates/rcfd-post/tests/piso_diffusion_f530.rs` added; `sa_aerodamp.rs` untouched | yes |

Checks of amendment 1 (each run once, on `13314def`):

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| QFc FQ option, cycle rule (B4 `0.952`, 23 periods) | bounded; periodic `<= 1e-6` | bounded; `1.9e-13` (Z printed: `3.025 - 0.562 i` against the model `3.374 + 0.510 i`) | yes (after amendment) |
| QFo FQ option, 8 outer (B4 `0.731`, 6 periods) | bounded; periodic; `5e-2` of `Z_dt` | bounded; `1.8e-13`; `1.66e-2`, `2.55e-2` | yes (after amendment) |
| DG3c twin-run radius against B4 (200 iterations) | within 10 %; sign `<= -0.5` | `9.130` against `9.399` (`2.9e-2`); sign `-1.000` | yes (after amendment) |
| A1o bounded over 3 periods and over `C`, `D = 100.5, 50.3, 25.1` (`C = 10, 5, 3`; B4 `0.920, 0.852, 0.731`) | bounded | bounded; max `|u|` `1.490, 1.493, 1.496 V0` | yes (after amendment) |
| A2o periodic | `<= 1e-6` | `4.5e-12`, `1.8e-12`, `5.6e-11` | yes (after amendment) |
| A3o against `Z_dt`, `N = 25, 50, 100` | `3e-2` each part | real `+5.28e-3`, `+5.53e-3`, `+5.66e-3`; imaginary `-4.61e-3`, `-9.4e-4`, `-2.5e-4` | yes (after amendment) |
| A4o `R_c` against the formula | `3e-2` each part | `+6.06e-3`, `+3.82e-3` | yes (after amendment) |
| A6o `abs(R_c - R_d)/abs(R_d)` | `<= 1e-2` | `2.31e-3` (A5's runs made again: the same `Z` to every printed digit) | yes (after amendment) |
| Oo order, real / imaginary | `0.8` to `1.25` | `0.833` / `1.109` | yes (after amendment) |
| NC1o `N = 25` against the formula | must miss | `+9.25e-2`, `-3.54e-1` | holds |
| APo piston, option, 8 outer, `N = 50, 100` (`D = 50.3, 25.1`) | bounded, periodic; `3e-2` each part | yes; real `+4.65e-3`, imaginary `+3.21e-3` | yes (after amendment) |

Also run: `model_matches_the_registered_arithmetic` (the test's model against the registration's arithmetic; passes), `cargo fmt
-p rcfd-post --check` (clean), `cargo clippy -p rcfd-post --all-targets -- -D warnings` (clean after two rewrites: `!(rho < 1.0)`
to `rho.is_nan() || rho >= 1.0`, and `is_none_or`, above the MSRV, to `matches!`; the fast tests passed again after them),
`cargo test -p rcfd-cli --release --test docs_site` (2 passed; `docs/dynamic-mesh.md` changed).

### Mutations

Applied one at a time to a `git archive` copy of `13314def` by `f530-scratch/mutate.py` (exact replacement, one occurrence
asserted), the fast tests run in release in `f530-scratch/target-mut`, the file restored and compared after each
(`run_mutations.sh`, `mutations_summary.log`, `MU1.log` to `MU6.log`). A first pass applied none (a syntax error in the script
left every run unmutated; all passed, an unmutated control) and was repeated with the script corrected.

| Mutation | Caught by |
| --- | --- |
| MU1 the option's cell velocity with `rAU` | QFc (not bounded at step 4), QFo (step 1) |
| MU2 `rAtU = rAU` | QFc and QFo (the cycle rule's B4 `5.01`, `3.9e5`) |
| MU3 `s = i w` in the model | `model_matches_the_registered_arithmetic`, QFo (`1.44e-1`, `5.85e-1`) |
| MU4 force on `xmax` | QFo (both parts `1.0`) |
| MU5 inflow `q0 w cos(w t_n)` | QFo (imaginary `1.28`) |
| MU6 `n_outer = 1` | QFo (`1.03e-1`, `2.10`) |

Every mutation was caught, by the fast tests alone.

### Deviations and what was not run

* Four unregistered diagnoses (amendment 1) between the registered runs and the corrected ones; `n_outer = 8` came from one of
  them (not derived; disclosed in amendment 1).
* A4, A6, O and NC1 of the registration were not formed (the `N = 25` level was refused as not periodic). The values amendment 1
  computes from the logged `N = 50, 100` impedances are not registered measurements.
* The registration's "documentation" paragraph is met by a bullet in `docs/dynamic-mesh.md` (the page that holds the F-299 limit);
  `docs/cosimulation-protocol.md` links there and was not changed.
* The low-frequency case of ADR 0446 (`D = 100` to `400`) and other cell counts were not run. The cost of the option with 8 outer
  iterations is about 13 times that of the plain step (FC `N = 100`: 98 s against 14 s for 300 steps), not limited.
* No refusal (c2') was built.

### Decision

The registered rule: DG1, DG3 (growth), QF2 (periodicity), A2 at `N = 25`, A3 (imaginary) and AP (imaginary) missed on their
registered runs, and A4, A6 and O were not formed. The checks of c2b were met only after amendment 1, made after the registered
runs. **Proposed status: partial.** What holds:

* The divergence is located (c1): it is the corrector instability of ADR 0214. It alternates in sign every step, the solver's own
  step-map radius is `9.13` against B4's `9.40` at `D = 25`, and the onset by B4 lies between `D = 2.5` and `3.0`. More correctors
  lower it, the Rhie-Chow time term and the grading play no part, and its rate depends on the cells across at a given `D`.
* The opt-in consistent correctors remove it: bounded over 3 periods at `D = 25` and `100` on the registered run (A1).
* The step with the option alone does not converge the pressure within a step at such `D`: a second property of the non-iterated
  step. With outer iterations the same channel matches the impedance as the stable runs do, but only after amendment 1.
* Defaults are bit-identical (no library change), and every mutation was caught.

### Remaining questions

1. **A default for large diffusion numbers.** Options: (a) make the consistent correctors, with outer iterations, the default
   above some estimate; (b) keep the defaults and document (done here); (c) refuse by default. (a) and (c) change default results:
   not taken, open for the maintainer. Recommendation: (b) now, with question 2.
2. **An opt-in refusal naming the diffusion number** (the acceptance's other branch). The B4 estimate in Raise mode already stops
   the default at step 1, but its message does not name `nu dt/dy^2`. Options: (a) a follow-up item adding an opt-in refusal at
   step 1 whose message names the largest cell diffusion number and the estimate; (b) nothing. Recommendation: (a) (follow-up
   below).
3. **The outer-iteration count.** `n_outer = 8` is a measured, not derived, value. Options: (a) a follow-up item that sets the
   count by a criterion (the outer pressure change, or a derived contraction); (b) leave it to the case. Recommendation: (a)
   (follow-up below).
4. **F-421's registered W1 and LF levels.** Options: (a) note in F-421 that its channel runs at `D = 25` and `50` with the option
   and 8 outer iterations (APo: within `4.6e-3`, `3.2e-3` after the extrapolation); (b) a new F-421 run. Recommendation: (a)
   (note text below; taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)).

### For the lead: roadmap entries

F-530, proposed status **partial**, note:

> Numerics. PARTIAL (ADR 0451, 2026-11-14; registered before the code, amendment 1 after the registered runs). Located: the divergence of ADR 0446 is the corrector instability of ADR 0214 (the first corrector over-corrects a pressure error by about the cross-gap diffusion number, the second does not remove the Rhie-Chow part left in the cell velocity): on the fixed-mesh channel driven by an oscillating inflow (120 x 40 cells, Womersley 1) the default step alternates in sign every step, the twin-run radius of the step map is 9.13 per step at nu dt/dy^2 = 25 against the ADR 0073 estimate 9.40, the estimate crosses one between 2.5 and 3.0 and is 5.0 at 16 cells across at the same number; more correctors lower it, the Rhie-Chow time term and the grading play no part. Removed by the opt-in consistent correctors of ADR 0214: bounded over 3 periods at nu dt/dy^2 = 25, 50 and 100. Found: with one outer iteration the step does not converge the pressure at such numbers (each corrector removes a few percent of a smooth pressure error), and the impedance's imaginary part is 14 percent low at 50 and 3 percent low at 25 (also with 8 correctors); with the consistent correctors and 8 outer iterations (no relaxation) every level matches the discrete implicit-Euler impedance within 0.6 percent, the extrapolation the oscillating-channel impedance within 0.61 and 0.38 percent and the stable default runs within 0.23 percent; the moving piston of F-421 the same (0.46, 0.32 percent). Partial: the registered estimate of the rate (16 cells) and growth measurement missed, the option alone missed the impedance and one periodicity tolerance (a slowly decaying numerical mode, 0.95 to 0.99 per step), and the match was met only after the amendment, with 8 outer iterations taken from a diagnosis. No library line changed (defaults bit-identical); six mutations caught; docs/dynamic-mesh.md. Open: ADR 0451 Q1 to Q3. Follow-ups F-NEW-a (opt-in refusal naming the diffusion number), F-NEW-b (outer-iteration criterion).

F-421, the note gains one sentence at its end (the full new note):

> Structural solver, advanced tier (plan ADR 0310). PARTIAL (ADR 0446, 2026-11-14; registered before the code, amendments 1 to 3 after registered runs). rcfd_post::solid_aerodamp: drive_mode moves the wall of a cosim::Fluid in a mode shape through an InterfaceMap (q = q0 sin(w t)) and records the generalised force by the transposed map; Harmonic::fit (least squares of mean, sine and cosine), periodic_response (every cycle fitted; refused with the cycle-to-cycle change of the first harmonic above a tolerance), added mass Q_p/(w^2 q0), damping -Q_q/(w q0), impedance, work per cycle pi q0 Q_q, damping ratio -W/(2 pi w^2 q0^2 m). Synthetic history: in-phase and quadrature parts to 4.8e-16, work per cycle against pi q0 Q_q to 4.0e-16, a decaying history refused naming the change (after amendment 1: the test's parse of the message). F-401's open column at T/100: added mass 0.997698 rho_f A L_f (limit 2 percent), damping ratio 3.14582e-2 against the registered tan(w dt)/2 + 1e-4 (the step's lag; viscous part 8.8e-7). Plane channel (h = 0.02 m, L_f = 100 h), the roadmap's impedance A i w rho L_f/(1 - tanh(kappa)/kappa) verified by derivation (12 mu L_f/h^2 is the limit per unit piston area): after the Richardson extrapolation in the time step, real and imaginary parts within 8.3e-3 and 1.6e-3 at Womersley 5, 3.2e-3 and 7.3e-3 at Womersley 1, the low-frequency limit within 9.8e-3; but only after amendments: the registered Womersley-5 run missed the real part by 8.5e-2 (the Symmetry planes of the one-cell 2D reduction add dissipation, about 7 percent; corrected with zero-gradient planes), and the registered Womersley-1 and low-frequency runs diverged (the PISO step is unstable above nu dt/dy^2 of about 3.5, also on a fixed mesh; corrected with fewer cells across and more steps). Bar on the open column: damping ratio 1.756e-2 (ratio 0.5) and 2.896e-2 (ratio 4) by this method, 1.777e-2 and 2.889e-2 from the decay of F-401's coupled run (agreement 1.2e-2, 2.5e-3): the lag of the pairing. Partial: the W5 miss, the amendments, and the order companion of the Womersley-1 imaginary part (0.68, limit 0.8). Seven mutations caught. No line of rcfd-fv or rcfd-api changed. Open: ADR 0446 Q1 to Q3. Follow-ups F-530 (PISO stability at large diffusion numbers), F-531 (Symmetry viscous flux). F-530 (ADR 0451, 2026-11-14): the divergence is the corrector instability of ADR 0214; with the consistent correctors and 8 outer iterations the Womersley-1 piston channel of this item (40 cells across) runs at nu dt/dy^2 = 25 and 50 and matches the impedance within 0.46 and 0.32 percent after the extrapolation.

Follow-up items:

* F-NEW-a, "Opt-in refusal of the PISO step at the pressure-lag onset, naming the diffusion number", deps F-530, acceptance:

> Every limit not stated here is registered in the ADR before the run. An opt-in setting makes the PISO/PIMPLE step refuse at step 1, through the divergence guard and with the state left as it was, when the pressure-lag estimate of ADR 0073 for the step's settings is at least one, with a message naming the largest cell diffusion number nu dt/dx^2, the estimate and the remedies (consistent correctors with outer iterations, a smaller step); the fixed-mesh channel of ADR 0451 is refused at nu dt/dy^2 = 25 and 100 and runs at 2.5; the default results of every existing case stay bit-identical.

* F-NEW-b, "Outer-iteration criterion for the consistent-corrector step at large diffusion numbers", deps F-530, acceptance:

> Every limit not stated here is registered in the ADR before the run. The number of outer iterations of the transient step with consistent correctors is set by a registered criterion (a derived contraction of the pressure error, or a tolerance on the change of the pressure between outer iterations) instead of a fixed count, so that the fixed-mesh channel of ADR 0451 at nu dt/dy^2 = 25, 50 and 100 matches the discrete implicit-Euler impedance within 1 percent at every level and its extrapolation the oscillating-channel impedance within 3 percent, with the outer iterations used per step recorded; the default results of every existing case stay bit-identical.
