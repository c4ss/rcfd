# ADR 0250: F-251 registration: strong (implicit) coupling of a rigid body with the moving-mesh flow step, with added-mass stabilisation (opt-in)

Status: **REGISTERED 2026-11-12**, written before any code of F-251 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run: the code and the documents listed under "What was read" were read, the
arithmetic under "Arithmetic before this registration" was done (a scratch script on the model problem, no flow solver), and
the unchanged tree of main was compiled once (`cargo test --release -p rcfd-fv --test sixdof_flow_f244 --no-run`, to fill the
target directory of the new worktree; nothing was executed). Results go in the addendum at the end; nothing above the
addendum is changed after a run.

Parents: [ADR 0033](0033-wave20b-dynamic-mesh.md) (the moving-mesh step D2/D3, the six-DOF body D6, the loose coupling D7 and
its remark that the explicit coupling is conditionally stable), [ADR 0211](0211-f244-registration-flow-coupled-six-dof-validation.md)
(F-244: the spring-mounted piston SP at mass ratio 0.2, its model M6, the coupling lag of `1.5 dt`, the measured
`omega_m/omega = 1.001307` and turning-point ratio `0.95299` at `T/100`). Roadmap item F-251 (depends on F-244).

Number: the lead gave 0250 to F-251. Branch `f251-strong-fsi` from main 3c8b5243, worktree `w49i-work/f251`.

House rules (ADR 0130, ADR 0211, ADR 0239, ADR 0241):

- limits are fixed here, before the code and the runs, and are not changed after a run; the limits of F-244 are not changed;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away; the relaxation factor and the added-mass estimate are fixed here from the derivation, and a
  second choice needs an amendment with its reasoning;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- defaults stay bit-identical: the loose coupling of ADR 0033 D7 is unchanged and remains what every existing caller runs;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded; no existing
  test is edited; no case-schema key is added;
- each registered run is made once, and nothing is re-run to pick a result (a deterministic run that two tests share is the
  same run);
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. Everything below is derived in this text from first principles. The names of the ideas, cited from memory (no
paper, book, code or vendor document was opened for this work, so every formula used is derived here and none is taken on
the authority of the citation): the added-mass instability of staggered fluid-structure schemes for an incompressible fluid
(P. Causin, J.-F. Gerbeau, F. Nobile, "Added-mass effect in the design of partitioned algorithms for fluid-structure
problems", Comput. Methods Appl. Mech. Engrg. 194 (2005) 4506-4527; C. Förster, W. A. Wall, E. Ramm, "Artificial added mass
instabilities in sequential staggered coupling of nonlinear structures and incompressible viscous flows", Comput. Methods
Appl. Mech. Engrg. 196 (2007) 1278-1293); sub-iterations with a fixed or a dynamic (Aitken) relaxation (B. M. Irons, R. C.
Tuck, "A version of the Aitken accelerator for computer iteration", Int. J. Numer. Methods Engrg. 1 (1969) 275-277; U.
Küttler, W. A. Wall, "Fixed-point fluid-structure interaction solvers with dynamic relaxation", Comput. Mech. 43 (2008)
61-72); an added-mass (Robin-type) term in the structure equation (S. Badia, F. Nobile, C. Vergara, "Fluid-structure
partitioned procedures based on Robin transmission conditions", J. Comput. Phys. 227 (2008) 7027-7051).

## What F-251 asks

The acceptance text of `docs/roadmap.json`, binding in full:

> A spring-mounted piston driving a fluid column at an added-mass to body-mass ratio of at least 1 oscillates at omega =
> sqrt(k/(M + rho A L0)) within 2 percent, the amplitude at successive same-side turning points never grows (ratio <= 1 +
> 1e-3) over at least 10 periods, and the explicit coupling of ADR 0033 D7 grows or diverges on the same case (registered
> before the run); halving dt changes the frequency by <= 0.5 percent

Clauses: (c1) the frequency within 2 percent; (c2) no growth of same-side turning points over at least 10 periods; (c3) the
explicit coupling grows or diverges on the same case, registered before the run; (c4) halving `dt` changes the frequency by
at most 0.5 percent; (c5) the mass ratio is at least 1.

## What was read (no run)

1. **The loose coupling** (ADR 0033 D7; the loop of `crates/rcfd-fv/tests/sixdof_flow_f244.rs` `sp_run` and of
   `sixdof_coupled.rs` C2). Per step: `patch_force_moment` from the flow at `t_n`; `Body::advance(dt, Rk4, force)` with that
   force held constant over the step; the points of the new body position; `ale::move_mesh`; `ale::set_follow_conditions`
   (the wall velocity that carries the swept volume, and the fixed-flux pressure gradient `dp/dn = -n . (u_b,new -
   u_b,old)/dt`); `PimpleSolver::step_ale_with_walls`. The loop lives in test code and in the case driver of `rcfd-api`; the
   library has no function for it.
2. **The body** (`crates/rcfd-fv/src/sixdof.rs`): `Body::advance` with RK4 evaluates the spring at the stages and the fluid
   force as a constant; `Body::state` is a public `Copy` value, so a step can be repeated from a saved state.
3. **The flow step** (`crates/rcfd-fv/src/pimple/moving.rs`, read only): implicit Euler only; `self.geom` is replaced by
   the new geometry. `PimpleSolver::snapshot` and `restore` (`pimple.rs`, public, ADR 0031 D3) capture and put back the
   cell values, the flux, the time, the step counter and the time-level history; `restore` cuts `history` to its last entry
   and clears `diverged`. The geometry, the velocity conditions (`flow.u`, a `Clone` field with its boundary conditions) and
   `history` are public fields. So a flow step can be repeated from `t_n` without any change to `pimple.rs` or
   `pimple/moving.rs`: **no line of either file is touched by this item**.
4. **ADR 0211** on the added mass: the model M6 (`(M + rho A (L0 - x)) x'' = -k x`, the flow a plug), the coupling lag
   (`1.5 dt` of the added-mass force, a decay of `exp(-2 pi zeta)` per period, `zeta = 1.5 omega dt m_a/(2 (M + m_a))`),
   the measured `1.001307` (`T/100`) and `1.001111` (`T/200`) of `omega`, of which the lag explains `+3.7e-4` and `+0.9e-4`;
   the rest (`+1.0e-3`, not diagnosed there) does not fall with `dt`. ADR 0033 D7 and the F-244 note give `0.2` as the bound
   of the registered cases and say the limits do not apply beyond it; neither derives a threshold.

## The model and the derivations

Notation: piston mass `M`, spring `k` about `x = 0`, column of cross-section `A`, length `L = L0 - x`, density `rho`,
`m_a = rho A L0`, `mu = m_a/M`, `omega^2 = k/(M + m_a)`, period `T`, step `dt`.

**D1. The model problem and its added mass (exact).** The channel of ADR 0211 M6: slip sides, outlet at `p = 0`, so the
incompressible flow is a plug with the piston's velocity and no shear. The momentum balance of the column gives the pressure
on the piston `p = rho L a`, `a` the acceleration of the column, so the fluid force on the piston is `F = -rho A L a`: the
added mass is `rho A L`, exactly, and with `|x| <= X = 0.01 L0` it is `m_a` within 1 percent at every instant (the variation
averages out over a period, ADR 0211 B0).

**D2. The classical result (generic staggered scheme).** Take the body equation with the fluid force of the previous
step, `M a^{n+1} = -k x^{n+1} - m_a a^n`. For a perturbation of the acceleration at the step frequency the spring term is of
relative size `k dt^2/M` and drops out as `dt -> 0`: `a^{n+1} = -mu a^n`. The perturbation is multiplied by `-mu` every
step: the scheme is unstable when `mu > 1`, whatever the step, and reducing `dt` only makes the same growth per step happen
sooner in time. This is the result of Causin, Gerbeau and Nobile and of Förster, Wall and Ramm (cited from memory; the two
lines above are its whole derivation for this model). It holds for an incompressible fluid because the pressure answers the
wall acceleration at once and in full: there is no factor of `dt` between the body's acceleration and the fluid's force.
The constant in `mu > C` depends on the two time discretisations; D3 derives it for the loop of this repository.

**D3. The loop of ADR 0033 D7 as built (threshold `mu = 2`).** `F_n` is the force used in the step `n -> n+1`. RK4 with a
constant force integrates the body exactly up to the spring: `x_{n+1} = x_n + dt v_n + dt^2 F_n/(2M)`, `v_{n+1} = v_n + dt
F_n/M`. The wall velocity of the flow step is the displacement over the step, `w_{n+1/2} = (x_{n+1} - x_n)/dt = v_n + dt
F_n/(2M)`: it sees **half** of the velocity change the force makes over the step. The flow step gives the plug the
velocity `w_{n+1/2}`, so its pressure is `rho L (w_{n+1/2} - w_{n-1/2})/dt` and the next force is

    F_{n+1} = -m_a (w_{n+1/2} - w_{n-1/2})/dt = -m_a [ (v_n - v_{n-1})/dt + (F_n - F_{n-1})/(2M) ] = -(mu/2) (F_n + F_{n-1}).

The characteristic equation `z^2 + (mu/2) z + mu/2 = 0` has, for `mu < 8`, a complex pair with `|z|^2 = mu/2`, and for
`mu >= 8` two real roots, the larger of magnitude above 1. So the loose loop is unstable exactly when **`mu > 2`**, with a
growth of `sqrt(mu/2)` per step for `mu < 8`, independent of `dt`. With the spring the recurrence gains terms of relative
size `k dt^2/M` (0.02 at `T/100` for `mu = 4`); the scratch model (below) puts the threshold between 1.9 and 2.1.

Consequence for the acceptance: at a ratio of exactly 1 the D7 loop is **stable** (`|z| = 0.71`), so clause (c3) cannot
hold there. The case is registered at `mu = 4` (clause c5 holds, `4 >= 1`; predicted growth `sqrt(2)` per step). The
threshold itself is registered as a check (L4, L5).

For a 2-term recurrence with a complex pair `z, conj(z)` the quantity `C_n = F_{n+1}^2 - F_n F_{n+2}` obeys `C_{n+1} =
|z|^2 C_n` exactly, so `(|C_j|/|C_i|)^{1/(2(j - i))}` measures the growth per step without a fit (used in L3).

**D4. The strong coupling.** Within one time step, iterate on the force `F` (and moment) held over the step:

1. predictor `F^1` = the force of the flow at `t_n` (the force of D7);
2. iterate `k`: from the saved body state at `t_n` advance the body with `F^k`; from the saved flow state at `t_n` move the
   mesh to the body's new position and take the flow step; evaluate the force `Ft^k` of the new flow state on the body;
3. residual `r^k = Ft^k - F^k`; converged when `|r^k| <= tol max(|Ft^k|, F_scale)` (and the same for the moment);
4. otherwise `F^{k+1} = F^k + w_k r^k` and repeat, at most `K` iterations.

The converged step has the body advanced by the force of the flow at the **end** of the same step. The body, the mesh and
the flow of the last iterate are consistent with each other (the last flow step was taken on the mesh of the body advanced
with `F^k`, and `F^k` is within the tolerance of that flow's force), so no further solve follows convergence.

*Stability of the converged scheme.* `F_n = -m_a (w_{n+1/2} - w_{n-1/2})/dt` with `w_{n+1/2}` now depending on `F_n` itself:
by the algebra of D3, `F_n = -(mu/2)(F_n + F_{n-1})`, so `F_n = -[(mu/2)/(1 + mu/2)] F_{n-1}`: a perturbation alternates and
decays by `mu/(2 + mu) < 1` per step for every `mu` (0.67 at `mu = 4`, 0.91 at `mu = 20`). The scheme is stable for every
mass ratio.

*What it costs in accuracy.* The force held over `[t_n, t_{n+1}]` is `m_a` times the acceleration centred at `t_n`
(difference of two step-mean velocities): a lag of `dt/2` against `1.5 dt` of D7. A force `-m_a a(t - tau)` is, for a
harmonic motion, `-m_a a - m_a omega^2 tau v`: a damping `m_a omega^2 tau`, so `zeta = omega tau m_a/(2 (M + m_a))` and the
turning points fall by `exp(-2 pi zeta)` per period: with `tau = dt/2`, `mu = 4`, 0.924 per period at `T/100` and 0.961 at
`T/200` (first order in `dt`, the order of the implicit Euler flow step). The amplitude never grows.

**D5. Convergence of the sub-iteration; the added-mass stabilisation.** On the model the map `F^k -> Ft^k` is affine: by D3,
`Ft^k = -m_a (v_n + dt F^k/(2M) - w_{n-1/2})/dt = -(mu/2) F^k + const`. Its slope is `g = -mu/2` (times `(1 - k
dt^2/(12M))(L/L0)`, RK4's spring term and the instantaneous column length: `-1.977` instead of `-2` at `mu = 4`, `T/100`,
`x = X`). The error of the relaxed iteration is multiplied by

    q(w) = 1 - w (1 - g) = 1 - w (1 + mu/2)

per iteration. So: without relaxation (`w = 1`) the sub-iteration diverges when `mu > 2`, the threshold of D3 (the loose loop
is its first iterate); a fixed factor converges exactly when **`0 < w < 4/(2 + mu)`**; the best fixed factor is

    w* = 1/(1 + mu/2) = 2M/(2M + m_a),

for which `q = 0` on the linearised model.

`w*` *is* the added-mass (Robin-type) term in the body equation. Write the body equation of iteration `k + 1` with an
estimate `m'` of the added mass moved to the left, `(M + m') a^{k+1} = Ft^k + m' a^k + R` (`R` the other forces, `M a^k =
F^k + R`). In terms of the force handed to the body, `F^{k+1} = M a^{k+1} - R = (M Ft^k + m' F^k)/(M + m') = F^k + [M/(M +
m')] (Ft^k - F^k)`: a relaxation with `w = M/(M + m')`. The added mass that answers the body's acceleration *within one
step of this discretisation* is `m_a/2` (D3: the mesh velocity sees half the step's velocity change), so `m' = m_a/2` and
`w = 2M/(2M + m_a) = w*`. The library gets the helper `relaxation_from_added_mass(M, m_est) = 2M/(2M + m_est)`; the user
supplies the estimate (here the exact `rho A L0`). The half is a property of the pairing "RK4 body with a constant force,
wall velocity from the displacement, implicit Euler flow" and is derived only for it.

**D6. Aitken's dynamic relaxation (derived).** For an affine scalar map `G(x) = g x + c`, the residual `r(x) = G(x) - x`
changes between two iterates by `r_k - r_{k-1} = (g - 1)(x_k - x_{k-1}) = (g - 1) w_{k-1} r_{k-1}`, and the step that lands on
the fixed point is `w = 1/(1 - g)`: `w_k = -w_{k-1} r_{k-1}/(r_k - r_{k-1})`. For vectors the quotient is replaced by the
projection on the change of the residual: `w_k = -w_{k-1} r_{k-1} . (r_k - r_{k-1}) / |r_k - r_{k-1}|^2` (the form of
Irons and Tuck as used by Küttler and Wall). On an affine scalar map the third iterate is the fixed point for any `w_0 !=
0`, including a `w_0` outside the range of D5. The force (3 components) and the moment (3 components) get one factor each,
because they have different units and, for a body whose translation and rotation are not coupled by the fluid, different
added masses. `|w_k|` is capped at `max`; a zero or non-finite `w_k`, or an unchanged residual, keeps `w_{k-1}`. Every time
step starts again from `initial`.

**D7. The convergence test and the cap.** Test quantity `e^k = max(|r_F^k|/max(|Ft^k|, F_scale), |r_M^k|/max(|Mt^k|,
M_scale))` (a ratio `0/0` counts as 0); converged when `e^k <= tol`. `F_scale` is a floor chosen by the user so that the
test does not ask for a relative accuracy of a force that passes through zero. Registered: `tol = 1e-8`, `F_scale = k X`
(the largest spring force), cap `K = 20` flow solves per step. An error `dF` of the force displaces the body by `dt^2
dF/(2M)` in a step: at the tolerance, `1e-8 X k dt^2/(2M) = 1e-10 X` per step at `T/100`, far below every limit. When the
cap is reached the step is kept as it is (body, mesh and flow of the last iterate, consistent as in D4) and reported with
`converged = false`; a non-finite force or flow, a failed mesh move or flow step is an error that leaves the flow and the
body at `t_n`.

**D8. What stays explicit.** (i) The fluid force is constant over the body's step (the end-of-step value): first order in
`dt`, the lag of `dt/2` of D4. (ii) The flow step itself is the unchanged PISO/PIMPLE step on the moved mesh, implicit Euler,
with its own lagged terms (the convective flux, the non-iterated correctors). (iii) The body's own forces (spring, damper,
weight, hook) are integrated by the body's integrator, RK4 here, explicit. (iv) The mesh smoothing is repeated every
iterate, so the geometry is implicit; the Aitken factor is not carried from one time step to the next. (v) The predictor
is the force of the flow at `t_n`, so with `K = 1` the strong step *is* the loose step, bit for bit (check Q1).

## Arithmetic before this registration (disclosed)

A scratch script (`w49i-work/f251-scratch/model.py`, not committed) integrates the model problem only: the body by the
RK4 of `Body::advance` with a constant force, the fluid force by `F = -rho A (L0 - x_{n+1})(w_{n+1/2} - w_{n-1/2})/dt (1 -
delta)`. No flow solver was run. `delta` is a deficit of the discrete added mass: `delta = 0` is the exact model; `delta =
0.011` is the value at which the model's loose run at `mu = 0.2` gives `1.001181` and `0.95294` (`T/100`) and `1.000986`,
`0.97601` (`T/200`), against the `1.001307`, `0.95299`, `1.001111`, `0.97604` that ADR 0211 measured. So the undiagnosed
`+1.0e-3` of ADR 0211 behaves like an added mass about 1.1 percent below `rho A L0`. A hypothesis for part of it (not
tested): `patch_force_moment` extrapolates the wall-cell pressure to the wall with a least-squares gradient that sees the
pressure field's own (zero-gradient) wall value, which in one dimension gives `0.8` of the true gradient in the wall cell
and a wall pressure low by `0.1 dx/L0 = 0.5` percent on the 20-cell column. The predictions below are given for both
`delta`; the limits do not depend on which is right.

| Model run | `delta = 0` | `delta = 0.011` |
| --- | --- | --- |
| loose, `mu = 4`, `T/100`: first `abs(x) >= 10 X`; end (`x >= L0`) | step 27; step 30 | step 27; step 32 |
| loose, `mu = 4`, `T/200` | step 31; step 36 | step 31; step 36 |
| loose, `mu = 4`: growth per step from `C_8` to `C_20`, `T/100`; `T/200` | 1.403; 1.396 | 1.395; 1.388 |
| loose, `mu = 1.9`, `T/100`, 11.5 periods | bounded, turning ratio 0.825 | 0.825 |
| loose, `mu = 2.1`, `T/100` | grows, `10 X` at step 374 | step 461 |
| loose, `mu = 1.5`, `T/100` | `omega_m/omega` 0.99953, ratio 0.839 | 1.00285, 0.839 |
| loose, `mu = 3`, `T/100` | `10 X` at step 45 | step 45 |
| strong, `mu = 4`, `T/100`: `omega_m/omega`; largest turning ratio | 1.000042; 0.92467 | 1.004472; 0.92452 |
| strong, `mu = 4`, `T/200` | 1.000025; 0.96150 | 1.004455; 0.96142 |
| strong, `mu = 4`: `T/200` against `T/100` | 1.7e-5 | 1.7e-5 |
| strong, `mu = 4`, Aitken from `w*`: iterations, largest; mean | 4; 3.00 | 4; 3.00 |
| strong, `mu = 4`, Aitken from `w_0 = 1` | 5; 3.90 | 5; 3.89 |
| strong, `mu = 20`, `T/100`, Aitken from `w*`: `omega_m/omega`; ratio; iterations | 0.999963; 0.911; 4, 3.52 | 1.005242; 0.911; 4, 3.75 |
| strong against loose, `mu = 0.2`, 6.5 periods, `T/100`: frequency difference; `max abs(x_s - x_l)/X` | 2.0e-4; 0.173 | 2.0e-4; 0.172 |
| the same at `T/200`: `max abs(x_s - x_l)/X` | 0.096 | 0.096 |
| turning ratios at `mu = 0.2`, `T/100`: loose; strong | 0.952; 0.984 | 0.953; 0.984 |
| first step from rest, `mu = 4`, `T/100`, fixed `w`: residual ratio `q` for `w = 0.25, 0.5, 1, 1/3` | 0.2558, -0.4884, -1.977, 0.0077 | 0.261, -0.478, -1.955, 0.015 |

The last row gives the gain `(1 - q)/w = 1 + mu_eff/2`: 2.977 (`delta = 0`), 2.955 (`delta = 0.011`); D5 says 3 without the
spring and the `L/L0` factors.

## Design

**S1. Library** (new file `crates/rcfd-fv/src/fsi.rs`, plus the line `pub mod fsi;` in `lib.rs`; nothing else in the
library changes, nothing in `rcfd-api`, the CLI or the case tree):

- `Relaxation::{Fixed(w), Aitken { initial, max }}`; `Relaxer` (the factor for one 3-vector sequence, D6);
- `relaxation_from_added_mass(mass, added_mass) -> f64` (D5);
- `StrongCoupling { relaxation, tolerance, force_scale, moment_scale, max_iterations }`, validated: `0 < w < 2` (fixed and
  initial), `initial <= max`, `tolerance > 0`, scales `>= 0`, all finite, `max_iterations >= 1`;
- `BodyCoupling { rho, nu, force_patches, follow_patches, integrator }` (what the D7 loop needs);
- `strong_step(flow, body, dt, &body_coupling, &strong, &mut points) -> Result<CouplingStep, String>` (D4, D7), `points` a
  caller's function from the body to the mesh points (the smoothing), as in the D7 loops; it refuses
  `settings.corrector_control` (its schedule is reset by `restore`); the flow step's own refusals pass through;
- `CouplingStep { iterations, converged, residuals (e^k), force_residuals (r_F^k), factors, force, moment (the pair used by
  the last iterate), flow (its StepInfo), mesh_flux, min_volume_ratio }`.

A repeated iterate restores, in this order, the geometry, the velocity field with its conditions, the snapshot, the full
`history`, the lengths of `lag_estimates` and `lag_warnings`, and the body state.

**S2. Tests** (new file `crates/rcfd-fv/tests/strong_fsi_f251.rs`; the heavy ones carry `#[cfg_attr(debug_assertions,
ignore = "heavy: run in release")]`). The helpers of ADR 0211's SP (channel, smoothing, crossings, turning points) are
copied into the new file; `sixdof_flow_f244.rs` is not edited.

**Set-up SPm.** SP of ADR 0211 exactly (`box_mesh([20, 2, 2], [1, 0.2, 0.2])`, `xmin` the piston, `xmax` fixed with `p =
0`, the four sides `ZeroGradient` and slip, `rho = 1`, `nu = 0.01`, `A = 0.04`, `L0 = 1`, `X = 0.01`, Laplacian smoothing
from the initial mesh each iterate, `piso(0.01)`, central, solvers `1e-16`/`1e-13`/5000, RK4, body at rest at `x = X`, fluid
at rest) with the mass `M = rho A L0/mu` and `k = (2 pi)^2 (M + rho A L0)` (`T = 1`): `mu = 4` (`M = 0.01`, `k =
1.973921`), `mu = 20` (`M = 0.002`), `mu = 1.5`, `mu = 3`, and `mu = 0.2` (`M = 0.2`, SP itself). Frequency and turning
points as in ADR 0211 (upward crossings by linear interpolation, `omega_m = 2 pi (N - 1)/(t_N - t_1)`; maxima and minima by
the parabola through three samples, the start the first maximum). Runs of 11.5 periods (11 crossings, 10 periods between
the first and the last; 12 maxima, 11 minima) unless stated.

**Strong settings (fixed now).** Aitken with `initial = relaxation_from_added_mass(M, rho A L0)` (`1/3` at `mu = 4`,
`1/11` at `mu = 20`, `10/11` at `mu = 0.2`) and `max = 2`; `tolerance = 1e-8`; `force_scale = k X`; `moment_scale = 0`;
`max_iterations = 20`.

**Loose runs** (`loose_run` in the test file): the D7 loop of `sp_run` with every failure caught: the run ends at the
first error of the mesh move or the flow step or the first non-finite value, and records the step.

## Registered checks

All in `crates/rcfd-fv/tests/strong_fsi_f251.rs`, release. "Relative" is `abs(a - b)/abs(b)`.

### Acceptance (SPm, `mu = 4`)

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| S0 (c5) | `rho A L0/M` of the case; the fluid-free shift `sqrt((M + rho A L0)/M) - 1` | `>= 1`; `>= 0.08` | 4; 1.236 |
| S1 (c1) | strong, `T/100`: `omega_m/omega` | `abs(. - 1) <= 0.02` (acceptance) | 1.0000 to 1.0045 (model) |
| S2 (c4) | strong, `omega_m` of `T/200` against `T/100` | relative `<= 0.005` (acceptance) | 1.7e-5 |
| S3 (c2) | strong, both runs: every ratio of successive maxima and of the magnitudes of successive minima; crossings | `<= 1 + 1e-3` (acceptance); `>= 11` | 0.925 and 0.961 (D4) |
| L1 (c3) | loose, `T/100`, at most 1150 steps: the first step with `abs(x) >= 10 X`, or the step of the first error or non-finite value if earlier | `<= 60` | step 27 (growth `sqrt(2)` per step from a seed of `0.01 X`) |
| L2 (c3) | the same at `T/200` | `<= 60` | step 31 |
| L3 (c3, D3) | loose, both steps: `(abs(C_20)/abs(C_8))^{1/24}` of the forces used in steps 1, 2, ... | in `[1.30, 1.50]` | 1.40 and 1.39; D3 says `sqrt(2)`; the two steps differ by under 0.01, which is the independence of `dt` |
| SN1 (negative control of S1) | strong, `T/100`, `rho = 0` in the force (fluid force off): `omega_m/omega` | `abs(. - 1) >= 0.08` | `sqrt(5) = 2.236` |

### The coupling iteration

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| S4 | the two runs of S1/S2: every step converged; iterations per step, largest and mean; the last `e^k` of every step | all; `<= 8` and `<= 5`; `<= 1e-8` | 4 and 3.0 (model); the cap of 20 is never reached |
| S5 (set-up) | the two runs: continuity of every step; discrete GCL of every step; `max abs(x)`; cyclic patches | `<= 1e-10`; `<= 1e-12 mean(V)`; `<= X (1 + 1e-9)`; none | ADR 0211 B3, B4, B6 |
| C1 (D5) | first step from rest, `T/100`, `Fixed(w)`, `w = 0.25, 0.5, 1`, cap 8: gain `(1 - r_{k+1}/r_k)/w` from the x components of the force residuals, `k = 1..5` | every value in `[2.90, 3.00]` | 2.977 (`delta = 0`), 2.955 (`delta = 0.011`) |
| C2 (negative control: the condition `w < 4/(2 + mu) = 0.667`) | first step, `Fixed(1)`, cap 20: `converged`; `abs(r_20)/abs(r_1)` | false; `>= 1e4` | `1.977^19 = 4e5` |
| C3 (D5, the added-mass factor) | first step, `Fixed(relaxation_from_added_mass(M, rho A L0))`: converged; iterations | true; `<= 8` | 5 (`q = 0.008`) to 6 (`q = 0.015`) |
| C4 (D6) | the `T/100` run with Aitken from `initial = 1` (outside the fixed range): every step converged; iterations largest and mean; `max_n abs(x_n - x_n(S1))` | all; `<= 8` and `<= 6`; `<= 1e-5 X` | 5 and 3.9; the tolerance moves `x` by `1e-10 X` a step, `1e-7 X` over 1150 steps if every error added up |
| R1 (the restart is exact) | after 37 strong steps (`mu = 4`, `T/100`): against a second solver that takes the same 36 strong steps and then step 37 as one D7-style pass with the force that step 37's last iterate used: `u`, `p`, `phi`, body `x`, `v` | bit-identical (difference exactly 0) | a restored state is a copy |
| H1 | `relaxation_from_added_mass(1, 4)`; `(0.5, 0)`; Aitken on the affine 3-vector map `G(x) = -2 x + c` from `w_0 = 1`: `abs(r_3)/abs(r_1)`; `Fixed(0.25)` on it: `r_2/r_1` | `1/3` and `1` to `1e-15`; `<= 1e-12`; `0.25` to `1e-12` | D5, D6 |
| P0 (model, no flow; values known before registration) | the scratch model in Rust: loose `mu = 1.9` bounded over 11.5 periods (largest turning ratio); loose `mu = 2.1` reaches `10 X`; growth per step at `mu = 4`, `C_8` to `C_20`, `T/100` and `T/200`; converged scheme at `mu = 4`: largest turning ratio `T/100` | `<= 1`; before step 1150; in `[1.38, 1.42]`; in `[0.92, 0.93]` | the table above |

### Threshold, equivalence, small ratio, extension

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| L4 (D3: below the threshold) | loose, `mu = 1.5`, `T/100`, 11.5 periods: finishes; largest turning ratio; `omega_m/omega` | yes; `<= 1 + 1e-3`; `abs(. - 1) <= 0.02` | 0.839; 0.9995 to 1.003 |
| L5 (D3: above it) | loose, `mu = 3`, `T/100`: first step with `abs(x) >= 10 X` or the first error | `<= 120` | step 45 |
| Q1 (D8 v; the loose coupling is what `K = 1` gives) | `mu = 0.2`, `T/100`, 100 steps: `strong_step` with `max_iterations = 1` against the D7 loop written out in the test: `x`, `v` after every step, `u`, `p`, `phi` at the end | bit-identical | same operations in the same order |
| A1 (small ratio: agreement) | `mu = 0.2`, 6.5 periods, `T/100`: `omega_m` strong against loose; each against `omega`; iterations | relative `<= 1e-3`; `abs(. - 1) <= 0.02`; every step converged, `<= 8` | 2.0e-4; 1.0010, 1.0012; 3 |
| A2 (small ratio: the difference is the lag, and falls with `dt`) | largest turning ratio at `T/100`, loose and strong; `D = max_n abs(x_s - x_l)/X` at `T/100`; `D(T/200)/D(T/100)` | loose in `[0.945, 0.960]`, strong in `[0.975, 0.992]`; `D(T/100) <= 0.25`; ratio in `[0.45, 0.65]` | 0.953, 0.984; 0.173; 0.096/0.173 = 0.555 |
| X1 (extension, `mu = 20`) | strong, `T/100`, 11.5 periods: `omega_m/omega`; largest turning ratio; converged; iterations largest and mean | `abs(. - 1) <= 0.02`; `<= 1 + 1e-3`; all; `<= 8`, `<= 5` | 1.0000 to 1.0052; 0.911; 4, 3.5 to 3.75 |
| E1 (refusals) | `strong_step` with `Fixed(0)`, `Fixed(2)`, `Aitken { initial: 0.5, max: 0.1 }`, `tolerance = 0`, `max_iterations = 0`, a NaN scale, a `corrector_control`; a `points` function that fails at its second call | each an error; after each the body state, `u`, `p`, `phi`, the time and the geometry pointer are those before the call | D7 |
| B1 (defaults) | the existing suites `sixdof`, `sixdof_coupled`, `sixdof_flow_f244`, `ale_flows`, `ale_gcl` of `rcfd-fv`, release, unedited; `git diff main` touches only `lib.rs` (one added line), `fsi.rs`, the new test file and this ADR | all pass; exactly those files | no existing code path is changed |

Printed, no limit: the turning points, the iteration histograms, the Aitken factors of the first three steps, the step at
which each loose run ended and how, the pressure iterations.

### Mutations (scratch, not committed)

Each is applied alone to `fsi.rs` of the final tree by an exact text replacement that must occur once, the named tests are
run once and must fail, and the file is restored and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | a repeated iterate does not restore the velocity field and its conditions | R1 |
| MU2 | a repeated iterate does not restore the body state | R1 |
| MU3 | a repeated iterate does not restore the flow snapshot | R1 |
| MU4 | a repeated iterate does not restore the geometry | R1 |
| MU5 | the Aitken factor with the wrong sign (`+w r.(dr)/dr^2`) | C4 |
| MU6 | the relaxed update ignores the factor (`F^{k+1} = Ft^k`) | C1, S4 |
| MU7 | the convergence test is always met (the loose coupling under another name) | S3 or S1 (the run diverges), S4 |
| MU8 | `relaxation_from_added_mass` returns `M/(M + m)` (the factor of D2's generic scheme, not of this pairing) | H1, C3 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### After the code

`cargo test -p rcfd-fv --release` for the new test file and for the suites of B1; `cargo fmt -p rcfd-fv --check`; `cargo
clippy -p rcfd-fv --all-targets -- -D warnings`. `cargo test -p rcfd-api --release --test baseline_hashes` is not planned:
no solver file is changed (a new module and one `pub mod` line). The whole-workspace suite is not run (the lead's gate).

## Predictions (mine, before any run)

S1 `1.004` (the 1.1 percent deficit of ADR 0211 carries over); S2 `2e-5`; S3 `0.925`, `0.961`; L1 step 27 to 32, ending in
an error of the mesh move when the piston passes the outlet, or earlier in a non-finite flow; L3 `1.40`; S4 4 and 3.0; C1
`2.96`; C2 `3e5`; C3 5 or 6; C4 5 and 3.9, difference `1e-9 X`; L4 `0.84`; A1 `2e-4`; A2 `0.953`, `0.984`, `0.17`, `0.56`;
X1 `1.005`, `0.911`. The check I trust least is R1 (it needs the solver to be deterministic to the bit between two solvers
in one process) and the pure-ratio windows of C1 (they assume the PISO step answers the wall acceleration as the plug
model does, on the first step from rest).

## Decision rule

- **done**: every acceptance check (S0 to S3, L1 to L3) is met, SN1 holds, the coupling checks (S4, S5, C1 to C4, R1, H1,
  P0), Q1, A1, A2, E1 and B1 are met, and every mutation fails its tests. The acceptance stays the maintainer's.
- **partial**: the strong coupling runs and an acceptance check is missed (kept as `*_known_miss`, diagnosed); or the
  acceptance checks are met and a coupling, threshold (L4, L5), small-ratio or extension (X1) check is missed and not
  shown by an amendment to be a defective set-up; or a mutation is not caught.
- **todo**: the strong step does not run or does not converge on SPm.

A set-up check (S5, P0, H1) or a negative control (SN1, C2) that does not hold means the check is not shown to be able to
fail: reported as a defective set-up, corrected by an amendment before any re-run, never counted as a pass.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default.

- **T1. The mass ratio of the acceptance case is 4, not 1.** The text says "at least 1" and asks that the explicit coupling
  grow on the same case; D3 shows the D7 loop is stable up to 2. Options: (a) `mu = 4`; (b) `mu = 1` with clause (c3)
  recorded as false; (c) a second, deliberately cruder explicit loop that fails at 1. Taken (a): it meets the text as
  written and tests the loop that exists.
- **T2. Sub-iterations with Aitken relaxation started from the added-mass factor**, not a one-pass added-mass term. A
  one-pass term with an estimate above the true added mass has a negative lag damping (the force `-(m_a - m') a(t - tau)`
  of D4 with `m' > m_a`), so it can grow slowly; iterating to convergence removes the dependence on the estimate.
- **T3. One Aitken factor for the force and one for the moment** (D6), each restarted every time step.
- **T4. At the cap the step is kept and flagged**, not refused (D7): the caller decides.
- **T5. The function lives in a new module `rcfd_fv::fsi`**, and nothing calls it but the tests: no case key.

## Limits of this registration

One degree of freedom, translation; the moment path of the relaxation is exercised only by H1 (a model map), not by a
flow. One geometry (the plug column), whose map is affine: Aitken's behaviour on a strongly non-affine map is not tested.
RK4 only (Newmark, which iterates its own forces, is not combined with the sub-iteration here). Implicit Euler flow only
(the moving-mesh step has no other). The half in `2M/(2M + m_a)` is derived for this pairing only. No case-tree wiring, no
`rcfd-api` change. No comparison with a published flexible-structure benchmark. Turbulence, energy, several bodies and
contact are outside.

## Open questions

Listed with options and a recommendation in the addendum (the case key that selects the coupling; the wording of the
acceptance; the cap; the wall-pressure extrapolation of `patch_force_moment`).
