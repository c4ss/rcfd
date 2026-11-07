# ADR 0214: F-299 registration: the PISO/PIMPLE instability at a large cell diffusion number across a thin gap

Status: **REGISTERED 2026-11-07**, written before any code of F-299 and before any rcfd run of its checks. Not yet accepted by
the maintainer. Results go in the addendum at the end.

Parents: [ADR 0211](0211-f244-registration-flow-coupled-six-dof-validation.md) (F-244, amendment 1: the failure and its
fixed-mesh reproduction, the ignored known miss `a1_falling_block_reaches_the_lubrication_terminal_velocity_known_miss`, Q4),
[ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) (R5, the pressure-lag model, and B4, its run-time estimate),
[ADR 0154](0154-job48t-cylinder-ogrid-pressure-spokes.md) (the same family on the refined cylinder O-grid: two correctors fail in
thin wall cells above a wall diffusion number, spokes of a few cells), [ADR 0156](0156-job48v-adaptive-pressure-correctors.md)
(adaptive corrector count; not used on a moving mesh). Roadmap item F-299 (depends on F-244, done).

Number: the lead gave 0214 to F-299. Main c4b2cae holds 0211 as its highest; 0210 is on `f294-transient-keps-sa`, 0212 on
`f296-pair-diffusivity`, 0213 on `f257-prism-layering`. No branch holds a 0214.

What was run before this text (disclosed): no rcfd code and no rcfd test. Scratch Python scripts (numpy, not committed, in the
work folder `E:/projects/cfd/w49i-work/f299-scratch`) did the analysis of M1 to M3 below: `gap1d.py` (a one-column model across
the gap), `gap2d.py` and `gap2d_v2.py` (a linear replica of the PISO step of `PimpleSolver::step_with` on a masked tensor grid),
`fbmodel.py` and `fbtable.py` (the replica on the lower end of the F-244 mesh, built with the coordinates of
`falling_block_mesh`), `mode2d.py` (dense eigenvectors of a small gap). Their numbers are the model values of the checks. The
observations of ADR 0211 amendment 1 were known before (they are postdictions here, and are marked so).

House rules (ADR 0207's and ADR 0211's):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the
  cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as
  an ignored record;
- every new code path is opt-in (`PimpleSettings::consistent_correctors`, default `false`): a run without it runs as before,
  bit for bit (N2); no case-schema key is added, so `crates/rcfd-api/tests/baseline_hashes.rs`, `case_hashes_w48u.rs` and
  the goldens are not touched; no existing test file is edited (the F-244 set-up is copied into the new test file);
- each registered run is made once, and nothing is re-run to pick a result (N2 is a registered re-run for identity only);
- a change that the default path would need is not made: it is an open question with a recommendation;
- no new third-party crate, no commit trailers.

Clean-room: the analysis below is derived from the discrete equations of the solver as read in `crates/rcfd-fv/src/pimple.rs`
and from ADR 0073 R5. The fix applies, per PISO corrector, the SIMPLEC split that rcfd's steady solver already uses
(`crates/rcfd-fv/src/simple.rs`, `Coupling::Simplec`: `rAtU = V/(a_P - H1)`, the cell term `-(rAU - rAtU) grad p` and the face
compensation `(rAtU - rAU)_f snGrad(p) |S|`, ADR 0050, ADR 0102); SIMPLEC itself is the textbook method of Van Doormaal and
Raithby (1984). No source outside the repository was read for this work, and no vendor code, text or install was used.

## What F-299 asks

The roadmap entry (acceptance, quoted in full in `docs/roadmap.json`): identify the mechanism of the failure found in ADR 0211
amendment 1 (fixed mesh, block at rest, time step above between `tau/40` and `tau/20`, cell diffusion number 3 to 6 across the
gap; not removed by dropping the Rhie-Chow time term, IC0 pressure solver, upwind convection, 3 correctors, 5 PIMPLE outer
iterations or cell aspect ratios 16 and 8), and either remove it with the ignored `tau/10` known-miss test of F-244 then passing,
or record it as a located limit with a refusal; defaults bit-identical.

**Reading of the acceptance (registered here).** "Identified" means a hypothesis whose quantitative predictions on the solver
itself are met by checks that can fail (D below). "Removed" with defaults bit-identical can only be an opt-in: the known-miss
test runs the default solver, and making it pass needs the default changed, which this ADR does not do (Q1). So the deliverable
is: the mechanism located (D), the existing located refusal of the default on a fixed mesh shown (L1), an opt-in that removes
the instability with the registered `tau/10` set-up of F-244 run under it (F, S, A1c), and the default-path decision left to
the maintainer.

## What the code read shows (main c4b2cae, read only)

1. **The PISO step** (`PimpleSolver::step_with`, `n_outer = 1`, `n_correctors = 2` for `PimpleSettings::piso`): the momentum
   predictor `A u* = b - V grad p^n` with the Green-Gauss cell gradient; `a_P` = the matrix diagonal averaged over the active
   components, `rAU = V/a_P`, `gamma_p = interp(rAU)`. Each corrector: `HbyA = H(u)/a_P` with `H = b - sum_offdiag A_ij u_j`
   (`b` holds `V u^n/dt`), `phiHbyA = interp(HbyA).S + gamma_p/dt * defect` (the transient Rhie-Chow term), the pressure equation
   `div(gamma_p snGrad p |S|) = div(phiHbyA)` (compact face gradient), the flux `phi = phiHbyA - gamma_p snGrad(p) |S|`
   (prescribed on fixed-velocity faces), and the cell velocity `u = HbyA - rAU grad p` (Green-Gauss again).
2. **ALE** (`pimple/moving.rs`): `step_ale_with_walls` sets the moving-mesh data and calls the same step; on the moving walls the
   pressure has the fixed-flux gradient and `add_wall_flux` adds `gamma_p |S| dp/dn` to `phiHbyA` so that the Neumann term of the
   pressure equation cancels.
3. **The pressure-lag estimate** (ADR 0073 B4, `lag_map`, `lag_amplification_n`): the pressure error alone through one step
   (velocity error zero at the start), power iteration; not defined with ALE (`lag_due` and the adaptive count are off there).
   Its documentation says that a second corrector "starts from a projected velocity and removes the lag".
4. **Steady SIMPLEC** (`simple.rs`): `h1` = the sum of the off-diagonal magnitudes, component-averaged; `rAtU = V/(a_P -
   h1).max(1e-3 a_P)`; the cell velocity `HbyA - (rAU - rAtU) grad p_old`; the face compensation on internal faces, and on
   boundary faces whose flux the pressure sets with `simplec_boundary_compensation` (ADR 0102, off by default there).
5. **Public state** (`PimpleState`, `snapshot`, `restore`): the cell velocity and pressure, the face flux and the time-level
   histories are public, so a test can step the solver from a perturbed copy of a state (TW below) without library code.

## The mechanism (derived; hypothesis H-RC)

Per unit volume, `A = I/dt + L` (implicit Euler, `L` the viscous operator with the half-cell wall coefficient), `a = diag(A)`,
`N = a - A` (the links), `G` the Green-Gauss gradient, `I` the face interpolation, `S_n` the compact face gradient. Across a gap
of `n` cells of width `dx = h/n` the cross-gap diffusion number is `D = nu dt/dx^2`; for FB (`h = 0.01`, `n = 16`, `nu = 0.01`,
`tau = 4.64912e-3`) `D = 119.02/spt` at `dt = tau/spt`: 23.8, 11.9, 5.95, 3.97, 2.98 at `spt` 5, 10, 20, 30, 40.

**M1. The first corrector over-corrects by about `D` (ADR 0073 R5).** A pressure error `e` drives the predictor velocity error
`-A^{-1} G e`. The corrector removes it with the pressure it obtains from `rAU = 1/a`, `a = (1 + 2D)/dt` in the gap interior,
while the gap column answers a pressure gradient uniform across it with about `dt` (the whole column moves). The corrector's
pressure is therefore too large by a factor of order `D`, with the opposite sign: R5's `kappa = -D` for one corrector.

**M2. The second corrector does not remove the rest on a collocated mesh.** R5's model assumes that the corrected velocity is
projected (divergence-free in the face sense), so that the second corrector returns zero lag. Here the cell velocity is corrected
with the cell gradient, `u = HbyA - rAU G p`, while the flux is corrected with the face gradient, so
`div I(u) = div(phi) + div(rAU_f (S_n - I G) p)`: the Rhie-Chow term is left in the cell velocity. For a pressure wave
`e^{i k y}` along a row spacing `dy`, `I G / S_n = cos^2(k dy/2)`: the left-over fraction `sin^2(k dy/2)` is zero for long waves
and one half for a wave of four cells (`k dy = pi/2`), where the drive of the predictor through `G` (`sin(k dy)`) is also
largest. That remainder enters the next corrector's `HbyA` through the cross-gap links (`N/a = 2D/(1 + 2D)`) and is amplified
again as in M1. Hypothesis **H-RC**: the growing mode of ADR 0211 amendment 1 is this one: pressure uniform across the gap, a
period of four rows along it, the sign alternating every step, growing in the long rows of the gap, with the per-step factors of
the replica (M3).

**M3. The replica (scratch, numbers registered as model values).** A linear Stokes replica of item 1 (no convection: the cell
Peclet number `U dx/nu` is 0.006 at `U_t`), with the transient Rhie-Chow term and the face flux in the state, exact solves, the
pressure level removed by its mean (gradients are the same as with the reference cell), the dominant eigenvalue of the step map
by power iteration (300 iterations) and its sign by the Rayleigh quotient:

| `dt` | correctors | FB lower end | uniform gap 16 x 48 | consistent, FB lower end | consistent, gap 16 x 48 |
| --- | --- | --- | --- | --- | --- |
| `tau/5` | 2 | -4.992 | -5.013 | +0.977 | +0.953 |
| `tau/10` | 1 | -20.75 | -15.87 | +0.978 | +0.954 |
| `tau/10` | 2 | **-3.165** | -3.174 | +0.957 | +0.916 |
| `tau/10` | 2, no Rhie-Chow time term | -3.153 | -3.168 | | |
| `tau/10` | 3 | -1.660 | -1.671 | | |
| `tau/10` | 4 | -1.035 | -1.046 | | |
| `tau/20` | 2 | -1.737 | -1.736 | +0.972 | +0.854 |
| `tau/30` | 2 | -1.144 | -1.146 | | |
| `tau/40` | 1 | -7.315 | -5.180 | | |
| `tau/40` | 2 | +0.980 | +0.883 | | |

"FB lower end": the 28 rows below the block, the 25 graded rows along it and 15 rows of `2h`, the full width of 56 columns with
the block's 24 removed, closed by a wall (2848 cells). "Uniform gap": 16 x 48 cells of `h/16 x 2h`, walls all round. The leading
mode on the FB lower end at `tau/10` with two correctors: cross-gap fraction `5.2e-8`, along-gap autocorrelations `+0.046` (lag
one row) and `-0.875` (lag two rows), amplitude rising from the graded rows into the long ones (dense eigenvectors of a 16 x 12
gap: rows `+1, -1, -1, +1, ...`, the same at every column). The one-column model (`gap1d.py`, long waves only) gives a stable
lag factor with two correctors (`-0.55` at `tau/10`, `-0.30` at `tau/20`): the long waves are not the mode. The rate depends
little on the row height above `h` (`tau/10`, two correctors: `-3.05`, `-3.18`, `-3.21` for rows of `h`, `2h`, `4h`) and less
above `h/2` (`-2.61` at `h/2`, `-1.78` at `h/4`), which agrees with "aspect ratios 16 and 8 grow as 32" of amendment 1. The
onset with two correctors lies between `tau/40` and `tau/30` (D 2.98 and 3.97).

Postdictions (known before, consistent with H-RC): the alternating sign and "about -3.4 per step" (amendment 1, coupled run;
replica `-3.17`); growth at `tau/20`, none at `tau/40`; three correctors grow more slowly (`-1.66`); the Rhie-Chow time term
changes nothing (`-3.153`); one corrector per outer iteration (PIMPLE with 5 outer iterations and no relaxation) grows much
faster; the moving mesh and the coupling play no part (the replica has neither).

Alternatives the checks separate: the mode carried by the velocity history or the body coupling (the replica's numbers would
then not match the twin-run radius of the fixed-mesh solver); a long-wave lag (the shape check D7 would see `c_2 > 0`); a
solver-tolerance effect (the twin runs use tight solvers; the replica uses exact ones; a mismatch would fail D1 to D4).

## The opt-in: consistent correctors (F)

**F0. `PimpleSettings::consistent_correctors: bool`, default `false`** (`piso` and `pimple` set `false`). When `true`, every
pressure corrector of every outer iteration (fixed and moving mesh) uses, with `p_prev` the pressure that drove it (the
predictor's for the first corrector, the previous corrector's after that), `h1` the component average of the off-diagonal
magnitudes of the momentum matrices of the outer iteration and `rAtU = V/(a_P - h1).max(1e-3 a_P)`:

- the cell velocity `u = HbyA - (rAU - rAtU) G p_prev - rAtU G p_new`;
- `phiHbyA` as before (with `gamma_p = interp(rAU)` in the transient Rhie-Chow term) plus the compensation
  `interp(rAtU - rAU) snGrad(p_prev) |S|` on internal faces and `(rAtU - rAU)_owner * gradient_b(p_prev) |S|` on boundary faces
  whose flux the pressure sets (not fixed-velocity, not empty);
- the pressure equation and the flux correction with `interp(rAtU)`, and `add_wall_flux` of a moving wall with the same
  `interp(rAtU)`.

At a converged state (`p_new = p_prev`) every face flux and cell velocity equals that of the default, so the steady solutions
coincide (S1). The pressure-lag map (ADR 0073 B4, `lag_map`) uses the same algebra when the option is on, so the B4 check, the
pre-run estimate and the adaptive count see the corrector actually used (F3).

R5 with the option: the corrector's pressure error for a mode of symbol `lambda` is `lambda dt/(c0 + lambda dt)`, in (0, 1), for
any `D` (the over-correction of M1 is gone because `a - h1 = 1/dt` in the gap interior). Replica: +0.96 to +0.98 on the FB
lower end at `tau/5` to `tau/20` (table above).

**Refusals.** With the option on, the step stops at once through the divergence guard (stage = the conflicting setting,
quantity `consistent correctors`, the state left as it was, a message naming ADR 0214) for the combinations whose algebra is not
derived or not checked here: the fractional step (`fractional`), `non_orth_correctors > 0`, `component_diagonal`, a periodic
(coupled) pressure face, and, per outer iteration after the hook, a body force (`body_force` or `source_force`) or a face jump
(`face_jump`). The default path never reaches this check.

## Set-ups

**FB mesh** (ADR 0211, copied with its constants into the new test file `crates/rcfd-fv/tests/piso_thin_gap_f299.rs`; the copy
is asserted to give ADR 0211's 12 320 cells, grid 56 x 343, block 24 x 287). `fb_settings()` of ADR 0211: `piso(nu)`, central
convection, momentum `tol_abs 1e-16`, `tol_rel 1e-13`, `max_iter 5000`, pressure the same with `max_iter 20000`, AMG pressure.

**FX (the fixed-mesh reproduction of amendment 1).** The FB mesh, not moved. Every wall a fixed velocity zero except `block`,
which before the step from `t_n` to `t_{n+1}` gets the fixed velocity `(0, -U(t_{n+1}), 0)`, `U(t) = U_t (1 - e^{-t/tau})` (M3
of ADR 0211); no pressure Dirichlet condition (`p_ref` cell 0); fluid at rest; `PimpleSolver::step`, `dt = tau/spt`. "Bounded":
no trip of the divergence guard, every velocity finite and `max |u| <= 10 U_t` after the step.

**TW (twin-run power iteration of the step map, on the solver itself).** From the state `S` after `n_b = max(1, spt/2)` steps of
FX: `K = 60` iterations. The direction `d_0`: in the cell pressure and the two in-plane velocity components a deterministic
pseudo-random field (the xorshift of `lag_amplification_n`, seed `0x2545F4914F6CDD1D`, values in `[-0.5, 0.5)`), zero in the face
flux. Iteration `k`: the state `S + eps d_k` (cell velocity and its old level, cell pressure, face flux and its old level all
perturbed alike; `eps` such that the perturbation has `1e-7` of the norm of `S`) is stepped once (FX, the block velocity of step
`n_b + 1`), the base step from `S` is subtracted, the difference over `eps` is `T d_k`. Norm:
`|d|^2 = sum_c V_c (|d_u|^2/U_t^2 + d_p^2/P^2)`, `P = max_c |p_c(S)|` (the flux part is carried, not measured). Estimate:
`|rho| = exp(mean of ln(|T d_k|/|d_k|))` and the sign `s = mean of sign(<T d_k, d_k>)` over `k = 30 .. 59`; "alternating" if
`s <= -0.5`. The mode: the pressure part of `d_60`.

**Shape descriptors** of a cell field `q` (the TW mode): gap cells are the columns of the two gaps and the rows along the block;
`m_{g,j}` the mean of `q` over gap `g` in row `j`; the gap fraction `E = sum_gap V q^2 / sum V q^2`; the cross-gap fraction
`X = sum_gap (q - m)^2 / sum_gap q^2`; the along-gap autocorrelations `c_l = sum_g sum_j m_{g,j} m_{g,j+l} / sum_g sum_j m_{g,j}^2`
for `l = 1, 2`.

**SC (steady channel).** `box_2d_graded(20, 12, 0.3, 0.1, [Uniform, Symmetric(3.0)])`; `xmin` fixed velocity `(U0, 0, 0)`,
`U0 = 0.1`; `xmax` velocity zero gradient, pressure fixed 0; `ymin`, `ymax` velocity zero; `nu = 0.01`; `fb_settings` with
`nu = 0.01`; `dt = 0.002` (wall diffusion number about 1); from rest.

**FB coupled** (ADR 0211 set-up FB and its loop, copied: Laplacian smoothing of the unit displacement, RK4, follow conditions,
`step_ale_with_walls`), with `consistent_correctors = true` added to `fb_settings`.

## Registered checks

All in `crates/rcfd-fv/tests/piso_thin_gap_f299.rs`, release (the heavy ones carry
`#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]`). "Within 10 %" is `abs(|rho| - m)/m <= 0.10` against the
model value `m` (FB lower end column). "Relative" is `abs(a - b)/abs(b)`.

### D, the diagnosis (default solver; run on the library of c4b2cae before any library change)

| Id | Check | Limit |
| --- | --- | --- |
| D0 (set-up, postdiction) | FX at `tau/10`, up to 100 steps | not bounded at some step |
| D0b (set-up, postdiction) | FX at `tau/40`, 400 steps | bounded at every step |
| D1 | TW at `tau/10` | within 10 % of 3.165; alternating |
| D2 | TW at `tau/20` | within 10 % of 1.737; alternating |
| D3 | TW at `tau/5` | within 10 % of 4.992; alternating |
| D4 | TW at `tau/10` with 3 correctors | within 10 % of 1.660; alternating |
| D5 | TW at `tau/40` | `|rho| < 1` |
| D6 | TW at `tau/40` with 1 corrector | within 10 % of 7.315 |
| D7 | the shape of the D1 and D2 modes | `E >= 0.9`, `X <= 0.05`, `abs(c_1) <= 0.3`, `c_2 <= -0.7` |
| DN1 (negative control of TW) | TW at `tau/40` with 1 corrector (R5's one-corrector instability, D 2.98) | `|rho| >= 1`; alternating |
| DN2 (negative control of D7) | the shape test on the pressure of the base state `S` of D1 | fails (`c_2 > -0.7`) |

Printed, no limit: TW at `tau/10` with 4 correctors (model 1.035), at `tau/10` without the transient Rhie-Chow term (model
3.153), at `tau/30` (model 1.144); D0's first step with `max |u| > 10 U_t` and its stop; the location (row centre `y`) of the
largest entry of each mode; the descriptors of every mode.

### L, the located limit of the default (fixed mesh)

| Id | Check | Limit |
| --- | --- | --- |
| L1 | FX at `tau/10` with `pressure_lag_check = LagCheck { every: 0, start: 1, iterations: 20, action: Raise }` | the run stops at step 1 with quantity `lag amplification`, value `>= 1.3`, and the cell of the mode in a gap (centre `x < h` or `x > h + w_b`, `y` within the block span) |

### F and S, the opt-in

| Id | Check | Limit |
| --- | --- | --- |
| F1 | TW with the option at `tau/5`, `tau/10`, `tau/20` | `|rho| < 1` each |
| F2 | FX with the option at `tau/10`, 100 steps: bounded and continuity at every step; the `y` force on `block` (`patch_force_moment`) at `10 tau` against that of FX at `tau/40` with the default (the D0b set-up, computed again in the F2 test because the D tests run before the option exists; its printed hash must equal D0b's) | bounded; `<= 1e-10`; relative `<= 1e-3` |
| F3 | the B4 estimate (`Warn`, step 1, 20 iterations) on FX at `tau/10` with the option | `rho < 1` |
| S0 (set-up) | SC default, 3000 steps: `max |u^{n+1} - u^n|` of the last step | `<= 1e-12 U0` |
| S1 | from S0's state, 50 steps with the option: after every step `max |u - u_S0|` and `max |p - p_S0|` | `<= 1e-10 U0`; `<= 1e-10 (max p_S0 - min p_S0)` |
| A1c (acceptance) | FB coupled with the option at `tau/10` (100 steps) and `tau/20` (200 steps): ADR 0211's A1 (on `tau/10`), A2 (`tau/20` against `tau/10`), A3, A4, A5 (both), A7, A8, A9 (on `tau/10`), AN1 | ADR 0211's limits, unchanged |
| A4u-c | ADR 0211's A4u replay (`tau/10`, 100 steps) with the option | A4u's limits |
| R1 | each refusal of F0 on a 2-D box (periodic in x for the periodic one), option on: `step` once | stopped at step 1, stage as named, quantity `consistent correctors`; `u`, `p`, `phi` bit-identical to before; the message contains `ADR 0214` |

Printed, no limit: F1's signs and modes; F2's iterations; F3's `rho` against the default's (L1); A1c's approach as ADR 0211
prints it.

### N, the defaults

| Id | Check | Limit |
| --- | --- | --- |
| N1 | `PimpleSettings::piso(0.01)` and `pimple(0.01, 3)` | `consistent_correctors == false` |
| N2 | every D and L test prints an FNV-1a hash of the bits of its final states (and of every TW estimate); the D and L tests run on the library of c4b2cae (the diagnosis commit) and again on the final tree | the printed lines equal |
| N3 | the gate: the existing suites unedited (`rcfd-fv` `sixdof_flow_f244`, `sixdof_coupled`, `ale_flows`, `ale_gcl`, `w34b_lag`, the `pimple` unit tests, `rcfd-api` `w48t_cylinder_spokes`, `w48v_adaptive_correctors`, `baseline_hashes`, H1 of `case_hashes_w48u`) | pass |

### Mutations (scratch, not committed)

Each is applied alone to the final tree by an exact text replacement (a script that checks that the replaced text occurs once),
the named tests are run once and must fail, and the file is restored and compared byte for byte with a kept copy.
`scripts/limit_audit.py` is run on the new test file and every hit is read.

| Id | Mutation (library, under the option) | Must fail |
| --- | --- | --- |
| MU1 | `rAtU = rAU` (the option inert in the correctors) | F1 (`tau/10`), A1c |
| MU2 | the cell term `-(rAU - rAtU) G p_prev` dropped | S1 |
| MU3 | the internal-face compensation dropped | S1 |
| MU4 | the boundary-face compensation dropped | S1 |
| MU5 | `add_wall_flux` with `interp(rAU)` under the option | A1c (A3) |
| MU6 | `lag_map` without the option's algebra | F3 |

Expected: MU1 makes the option the default algebra (D1 and the known miss); MU2 to MU4 move the fixed point by about
`(rAtU - rAU) grad p`, of order `1e-2 U0` on SC; MU5 leaves `(rAtU - rAU)_b |S| dp/dn` in the cells under and over the
accelerating block, a continuity (A3's measure) of order `1e-5` in the first steps; MU6 gives the default's estimate (L1, about
3).

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a `git archive`
copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), through the machine's gate lock
(`gate-lock.sh acquire f299`), started with at least 19 GB of physical memory free (the machine's free memory is about 18 to 25 GB
while other gates run; ADR 0211 used 20 GB). A failure of a known flaky test is run again alone on the same build and reported
with its name.

### Classes

A miss of D1 to D7 means H-RC is not shown: reported with its number, kept as an ignored `*_known_miss` test with its limit,
and the mechanism stays open (amendment). A set-up check (D0, D0b, S0) or a negative control (DN1, DN2) that does not hold, or a
mutation that does not make its tests fail, means a check is not shown to be able to fail: reported as a defective set-up,
corrected by an amendment before any re-run, and never counted as a pass. A miss of F1 to F3, S1, A1c, A4u-c or R1 is a miss of
the opt-in: ignored known miss, limit intact.

**Decision rule.** D1 to D7 met (with D0, D0b, DN1, DN2 held) and L1 met: the mechanism is located. If in addition F1 to F3, S1
(S0 held), A1c, A4u-c, R1, N1 to N3 are met and every mutation failed its tests: the instability is removed under the opt-in,
and F-299 is set **partial** (`[~]`), the remaining part named: the default path (the known miss of F-244 still fails by
construction, Q1) and a refusal on the moving-mesh default path (Q3). A miss of the opt-in with the mechanism located: partial,
the miss named, the located limit L1 recorded. A miss of D: todo stays, with the miss named. Done (`[x]`) is the maintainer's
call (Q2).

## Not delivered

Any change of the default PISO/PIMPLE (Q1); a case-schema key (Q4); the option on periodic faces, with body forces, face jumps,
non-orthogonal correctors, the component diagonal or the fractional step (refused, Q5); the lag estimate on a moving mesh (Q3);
the cylinder O-grid spokes of ADR 0154 with the option (Q6); a temporal-order study of the option (ADR 0069/0073 harnesses);
BDF2 with the option on a moving mesh (the ALE step is implicit Euler only).

## Open questions and choices for the maintainer

- **Q1. The default.** The known miss of F-244 passes only if the default corrector changes. Options: (a) make
  `consistent_correctors` the default of `piso` and `pimple` (needs the bit-identity lift; every transient result and the
  baseline hashes change); (b) keep the default and expose the option (Q4); (c) keep both as they are. Recommendation (b) now,
  and (a) only after a registered benchmark set (the ADR 0069/0073 temporal-order harnesses, the cylinder K1/K2 of ADR 0154, the
  Studio examples) with the option.
- **Q2. Status.** Options: (a) partial, as the decision rule says; (b) done, taking the opt-in as the removal. Recommendation
  (a) until Q1 is decided.
- **Q3. A refusal on the moving-mesh default path.** The located refusal L1 needs the lag estimate, which is not defined with
  ALE, and a refusal there would change what the default does. Options: (a) define the estimate on the moving-mesh step (its
  matrices are fixed within a step) and wire SV301 and the adaptive count to it, as a follow-up item; (b) nothing.
  Recommendation (a).
- **Q4. A case key** (for example `solver.consistent_correctors`). Needs a schema golden edit. Recommendation: yes, as a
  follow-up item with the benchmarks of Q1.
- **Q5. The refused combinations.** Recommendation: extend one at a time, each with a steady-identity check like S1 (periodic
  faces need the coupled-face compensation of ADR 0102; body forces need the balanced-force wall term of the steady SIMPLEC).
- **Q6. The cylinder spokes.** ADR 0154's spokes (wavelength 4.5 to 7 cells along the wall, two correctors, wall diffusion
  number above 6.5) look like the same mechanism along the wall direction. Recommendation: a follow-up that runs K1 and K2 with
  the option once Q4 gives it a key.
- **Q7. The LagCheck text.** Its documentation and its Raise message say that a second corrector removes the lag; M2 shows that
  this does not hold for short waves on thin cells. Changing the message changes a user-visible string. Recommendation: correct
  the text in the follow-up of Q4.

## Amendment 1 (2026-11-07): the base state of D6 and DN1, written before the corrected run

**What happened.** The D and L tests were run once, in release, on the diagnosis commit 4ae3ec7 (the library of c4b2cae, test
code only; the test binary was linked at 11:58:36 UTC from the library built at 11:57:44, before any library edit). Every test
ran to its end except `d6_dn1_twin_run_with_one_corrector_at_tau_40`: TW's base run with one corrector at `tau/40` was not
bounded at step 4 (of the `n_b = 20` base steps), so the base state `S` of D6 and DN1 does not exist and neither was measured.
The one-corrector step is unstable from the start-up (R5: about `-7` per step on the start-up error), which the registered rule
`n_b = max(1, spt/2)` did not allow for. The numbers of the other D and L tests go in the addendum; none is repeated.

**Corrected set-up (registered now, before its run).** D6b and DN1b: the base state is that of D5 (FX at `tau/40` with the
registered two correctors, `n_b = 20` steps, bounded in D5's run); then `n_correctors = 1` is set on the solver and the 60 TW
iterations are made with one corrector (the base step from `S` too). Limits unchanged: D6b within 10 % of 7.315; DN1b
`|rho| >= 1` and alternating. The registered test stays as the ignored record
`d6_dn1_twin_run_with_one_corrector_at_tau_40_defective_setup`; the new test is `d6b_dn1b_one_corrector_from_the_d5_base_state`.
It runs on the same diagnosis tree (library of c4b2cae).

## Amendment 2 (2026-11-07): the set-up check of the steady identity, written before the corrected run

**What happened.** The F, S, A1c, A4u-c, R1 and N1 tests were run once, in release, on 2bd0faa (the opt-in and its tests);
their numbers go in the addendum and none is repeated. The set-up check S0 did not hold: after the 3000 default steps on SC the
last step changed the velocity by `1.95e-12 U0` against the limit `1e-12 U0`. The run had reached the floor of its solvers
(about `2e-13` m/s per step; the slowest physical mode has decayed by `e^-59` by then), which the registered limit did not allow
for. S1's numbers on that run (`1.97e-12 U0`, `1.77e-11` of the pressure range) are therefore not counted.

**Corrected set-up (registered now, before its run).** S0b and S1b: the same SC run of 3000 default steps gives the state `S0`;
from `S0` two branches of 50 steps each: (A) the default, (B) with the option. S0b (set-up): branch A's `max |u - u_S0|` over its
50 steps `<= 2e-11 U0` (the default's own drift at the floor is at most a fifth of S1b's limit). S1b: after every step of branch
B, `max |u - u_S0| <= 1e-10 U0` and `max |p - p_S0| <= 1e-10 (max p_S0 - min p_S0)` (S1's limits, unchanged); branch B against
branch A after the 50 steps is printed. MU2 to MU4 must fail S1b. The registered test stays as the ignored record
`s0_s1_steady_state_of_the_consistent_correctors_is_the_default_one_defective_setup`; the new test is
`s0b_s1b_steady_state_of_the_consistent_correctors_is_the_default_one`.
