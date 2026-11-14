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
