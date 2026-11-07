# ADR 0211: F-244 registration: flow-coupled six-DOF validation (falling block, spring-mounted piston)

Status: **REGISTERED 2026-11-07**, written before any code of F-244 and before any run of its checks. Not yet accepted by
the maintainer. Results go in the addendum at the end.

Parents: [ADR 0033](0033-wave20b-dynamic-mesh.md) (F-063 and F-229: the ALE step D2, the moving walls D3, the six-DOF body
D6, the loose coupling D7, the checks C1 and C2, the smoothing check S3), [ADR 0101](0101-wave41a-oscillating-cylinder-two-timescale-decay-vortex-bracket-milne.md)
(F-063 done: continuity and GCL asserted on a smoothed moving mesh). Roadmap item F-244 (depends on F-229 and F-063, both
done).

Number: the lead gave 0211 to F-244. Main c744b5f holds 0208 as its highest; 0209 is on `f249-parametric-studies` and 0210
on `f294-transient-keps-sa`. No branch holds a 0211.

What was run before this text (disclosed): no rcfd code and no rcfd test. Three scratch Python scripts (numpy, not
committed) did the arithmetic of the design: the closed forms of M1 to M3 and M6 for the chosen numbers, the integrals
`F0`, `F1`, `F2` and `J(delta)` of M4 with its minimum over `delta`, a Gauss-Legendre quadrature of the 2-D trial field of M4
against the closed form (equal to 1e-14), and the 1-D finite-volume system of M5 for `n = 10` to `32` (the `2/n^2` law).

House rules (ADR 0207's, unchanged):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the
  cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as
  an ignored record;
- each registered run is made once, and nothing is re-run to pick a result;
- no case-schema key is added, no library code is changed (S1), so `crates/rcfd-api/tests/baseline_hashes.rs`,
  `case_hashes_w48u.rs` and the goldens are not touched and every default stays bit for bit;
- no new third-party crate, no commit trailers.

Clean-room: the closed forms below are derived here from the Stokes and Navier-Stokes equations (plane Couette-Poiseuille
flow, the minimum-dissipation theorem for Stokes flow, the energy balance of a closed domain, unsteady Bernoulli for a plug
flow). No source was read for this work, and no vendor code, text or install was used. Where a textbook theorem is used it
is stated and proved in the few lines it needs.

## What F-244 asks

The roadmap entry (acceptance, quoted in full in `docs/roadmap.json`): laminar incompressible PISO/PIMPLE on a moving mesh,
the existing loose coupling (ADR 0033 D7), added-mass-to-body-mass ratio at most 0.2 from the ADR closed form, no cyclic faces
carrying a moving-wall velocity.

- (a) A rigid block falling under gravity minus buoyancy along a closed 2-D channel, the displaced fluid returning through the
  two side gaps, reaches the terminal velocity of the lubrication closed form derived in the ADR within 2 percent, after an
  approach of at least 5 time constants that fits inside the F-063 smoothing compression limit. The ADR bounds the neglected
  end-effect, finite-Reynolds and finite-length terms to at most 0.5 percent before the run.
- (b) Free decay of a spring-mounted piston driving a fluid column (the C2 geometry plus a spring, amplitude at most 1 percent of
  `L0`) oscillates at `omega = sqrt(k/(M + rho A L0))` within 2 percent, the fluid lowering the frequency by at least 4 times
  the tolerance, so that the check fails with the fluid forces off.
- Both: halving `dt` changes the measured quantity by at most 0.5 percent; per-step continuity at most 1e-10 and the
  uniform-flow GCL to round-off are asserted; in (b) the amplitude at successive same-side turning points never grows (ratio at
  most `1 + 1e-3`).
- Out of scope: strong coupling and added-mass stabilisation (F-251), published falling-body experiments and wall-correction
  tables, vortex-induced vibration (user decision 2026-10-22: "in a flow" is free decay in still fluid), free 3-D rotation with
  Newmark, contact, multi-body, turbulence or energy on moving meshes.

**Reading of the acceptance (registered here).**

1. "Terminal velocity ... within 2 percent after an approach of at least 5 time constants": the body velocity at
   `t_end = 10 tau` (`tau` the time constant of M3) against the closed form `U_t` of M1.
2. "Fits inside the F-063 smoothing compression limit": the only compression F-063 validated is ADR 0033 S3 (ii) / S3b, a
   Laplacian-smoothed uniform compression to `V_new/V_old = 0.5` with slip sides ("no claim of validity beyond the registered
   displacements", ADR 0033 D5). Registered: every cell keeps `V(t)/V(0) >= 0.5` against the initial mesh, at every step.
3. "Gravity minus buoyancy": the fluid is solved without gravity, so `p` is the pressure minus the hydrostatic part, and the
   body carries its weight `M g` and the buoyancy `rho V_b g` of the hydrostatic part (Archimedes, exact for a fully immersed
   body in a constant-density fluid). The buoyancy is a body hook; the weight is `BodySpec::gravity`.
4. "The uniform-flow GCL to round-off": two parts, both asserted. (i) The discrete GCL of every step of the coupled runs,
   `max_c |sum_f sigma_f W_f - (V^{n+1} - V^n)| <= 1e-12 mean(V)` (ADR 0033 G1 (i)). (ii) A uniform flow `u0` on the same
   moving mesh, every patch a fixed velocity `u0` (ADR 0033 G2's set-up), stays uniform: `max |u - u0| <= 1e-12 |u0|` and
   `max |p| <= 1e-12 |u0|^2` after every step.
5. "Per-step continuity" is `StepInfo::continuity` (largest cell imbalance of the corrected flux over the largest face area),
   the quantity ADR 0033 and ADR 0101 assert; limit `1e-10` in every step of every coupled run.
6. "Halving `dt`": the registered runs use `dt` and `dt/2` with everything else equal; the measured quantity is `U(t_end)` in
   (a) and the measured frequency in (b).
7. "No cyclic faces carrying a moving-wall velocity": neither mesh has a cyclic patch (asserted).

## What the code read shows (main c744b5f, read only)

1. **Six-DOF body** (`crates/rcfd-fv/src/sixdof.rs`): `BodySpec` (mass, inertia, `translation_locked`, `rotation`,
   `gravity` applied as `gravity * mass`, per-axis `stiffness`, `damping`, `rest_position`, `drag`), `BodyState`, `Body::advance`
   with `Integrator::Rk4` or `Newmark` and the fluid force held constant over the step, `Body::set_hook` (a user force added to
   every evaluation). `patch_force_moment(geom, u, p, rho, nu, patches, x_ref)` gives
   `F = rho sum_f [p_f S_f - nu ((grad u)_f + (grad u)_f^T) . S_f]`, `S_f` pointing out of the fluid, the wall-normal
   derivative replaced by `(u_b - u_c)/dn`, `p_f` the cell value extrapolated with the least-squares gradient.
2. **Moving mesh** (`crates/rcfd-fv/src/ale.rs`, `pimple/moving.rs`): `move_mesh(geom, points, dt)` (swept volumes, the moved
   mesh validated, `min_volume_ratio`), `set_follow_conditions` (the follow velocity `u_b = w_t + (m_f/|S|) n` and the
   fixed-flux pressure gradient `dp/dn = -n . a_wall`), `PimpleSolver::step_ale_with_walls`, implicit Euler only.
3. **Smoothing** (`crates/rcfd-fv/src/smoothing.rs`): `smooth_displacement(geom, classes, prescribed, method, control)` with
   `Prescribed`, `Fixed`, `Slip` classes, solved from the initial mesh. Every step of it is linear in the prescribed
   displacement (a linear diffusion solve, a linear least-squares point reconstruction, linear projections onto slip planes).
4. **The existing flow-coupled checks** (`crates/rcfd-fv/tests/sixdof_coupled.rs`): C1b (plate on a Couette layer, mesh fixed)
   and C2 (piston of mass 0.2 pushed by a constant force into a column on `box_mesh([20, 2, 2], [1, 0.2, 0.2])`, `xmin`
   prescribed, `xmax` fixed with `p = 0`, the other four patches `ZeroGradient` and slip, `nu = 0.01`, Laplacian smoothing each
   step, RK4, `dt = 0.005`; `v^2` within 3e-5 of the closed form). Neither has a spring, and neither has a drag from fluid
   displaced past the body.
5. **Meshes**: no generator makes a channel with a rectangular hole. `Mesh::from_cell_faces(points, cells, boundary)` and
   `BoundarySpec` are public; the masked structured builder of `gen.rs` is private. The tests build the mesh of (a) with
   `from_cell_faces` (test code).
6. **Pressure**: `FvMatrix::solve` is IC0-preconditioned CG; `PimpleSettings::amg_pressure` selects AMG-preconditioned CG.
   A domain without a pressure Dirichlet condition pins `p_ref` (cell 0, value 0).
7. **Continuity**: `StepInfo::continuity = max_c |sum_f phi_f| / max_f |S_f|` after the last corrector.

## The model (derived)

Notation for (a): gap width `h`, block width `w_b`, block height `H_b`, channel width `W = w_b + 2h`, depth `dz` (one cell, empty
front and back), `B = (w_b + h)/h`, fluid density `rho`, kinematic viscosity `nu`, `mu = rho nu`, block velocity `(0, -U)`.

**M1. Lubrication drag (a).** In each gap, fully developed flow between the fixed channel wall `x = 0` and the block side
`x = h` moving at `-U`, pressure gradient `G = -dp/dy`:

    v(x) = (G/(2 mu)) x (h - x) - U x/h,    q = int_0^h v dx = G h^3/(12 mu) - U h/2.

The channel is closed, so the fluid displaced below any section through the block crosses that section through the two gaps:
`q_left + q_right = w_b U` exactly, at every instant (incompressibility). With the symmetric split `q = w_b U/2`:

    G = 6 mu U (w_b + h)/h^3,   v(x) = U [3 B xi (1 - xi) - xi],  xi = x/h.

The block feels the pressure difference `G H_b` over its width and the shear `-mu v'(h) = G h/2 + mu U/h` on each side, so the
drag per unit depth is `D = G H_b w_b + 2 H_b (G h/2 + mu U/h)`:

    D_lub = C U,   C = 2 mu H_b dz (3 B^2 + 1)/h      (with the depth dz).

The same `C` follows from the dissipation: `int_0^h v'^2 dx = (U^2/h)(3B^2 + 1)` per gap, so the two gaps dissipate
`C U^2 = D_lub U`. Terminal velocity, with `V_b = w_b H_b dz`:

    U_t = (M - rho V_b) g / C.

**M2. Added mass (a).** The kinetic energy of the lubrication field in the two gaps is `(1/2) m_a U^2` with
`int_0^1 [3 B xi (1 - xi) - xi]^2 dxi = 0.3 B^2 - 0.5 B + 1/3`:

    m_a = 2 rho H_b h dz (0.3 B^2 - 0.5 B + 1/3).

This is the quasi-steady added mass (the inertia force of a field `U(t) u_hat(x)` is `-rho U' int |u_hat|^2`). The added mass that
governs the stability of the explicit coupling is the instantaneous (potential) one, which by Kelvin's minimum-energy theorem is
at most the kinetic energy of any divergence-free field with the right normal velocities. The lubrication field of the gaps
continued by the trial field of M4 into the end regions is such a field, and its end layers add `0.57` percent (M4), so
`m_pot <= 1.0057 m_a`. The displaced-fluid mass `rho V_b` is 0.375 of `m_a` here and is not used.

**M3. Approach (a).** With the quasi-steady drag and added mass, `(M + m_a) U' = (M - rho V_b) g - C U`, so from rest
`U(t) = U_t (1 - e^{-t/tau})`, `tau = (M + m_a)/C`, and the travel is `s(t) = U_t (t - tau (1 - e^{-t/tau}))`. The viscous
profile across a gap develops with the slowest diffusion mode `e^{-t/t_m}`, `t_m = h^2/(pi^2 nu)`.

**M4. The neglected terms (a), bounded before the run.**

*E1, end effects (Stokes flow), two-sided and rigorous.* Let `u` be the Stokes flow of the instantaneous geometry. Every boundary
piece is a straight wall moving rigidly (the channel walls at rest, the block faces at `(0, -U)`), so for any divergence-free field
with these boundary values `int 2 e:e = int |grad u|^2` (the difference `int d_j(u_i d_i u_j)` vanishes on every straight rigid
wall: there `u.n` and `u.t` are constant along the wall and `d_n u_n = -d_t u_t = 0`). Then:

- *Lower bound.* `D_S U = mu int |grad u|^2 >= mu int_gaps (d_x v)^2 dx dy`. On every section of a gap, `v(0) = 0`, `v(h) = -U`
  and `int v dx = q_i(y)` with `q_left + q_right = w_b U`; the minimum of `int (v')^2` under these constraints is the parabola of
  M1 (Euler-Lagrange: `v''` constant), and it is a convex quadratic in `q_i`, so the sum over the two gaps is smallest at the equal
  split. Hence `D_S >= D_lub` at every instant.
- *Upper bound* (Helmholtz minimum dissipation: the Stokes flow dissipates least among divergence-free fields with its boundary
  values). Trial stream function (`u = psi_y`, `v = -psi_x`), left half `0 <= x <= W/2` mirrored (`psi(W - x) = -psi(x)`, which is
  admissible because `psi = 0` on `x = W/2`). In the gaps the lubrication field, `psi = f(x) = a x^2 + b x^3` on `[0, h]` with
  `a = -U (3B - 1)/(2h)`, `b = U B/h^2` (so `f(0) = f'(0) = 0`, `f(h) = -U w_b/2`, `f'(h) = U`), and `f(x) = U (x - W/2)` under
  the block (`f` is C1). Below the block, `s = y_b - y` the depth below its lower face, `psi = f(x) g(s/delta)` with
  `g(sigma) = 1 - 3 sigma^2 + 2 sigma^3` on `[0, 1]` and `psi = 0` deeper; above the block the mirror image. This field is in H2,
  divergence-free, no-slip on every wall and moves with the block on its faces. Its dissipation in one end layer of one half is
  `mu [ (13/35) delta F2 + (12/5) F1/delta + 12 F0/delta^3 ]` with `F0 = int f^2`, `F1 = int f'^2`, `F2 = int f''^2` over
  `[0, W/2]` (from `int g^2 = 13/35`, `int g'^2 = 6/5`, `int g''^2 = 12`, `int g g'' = -6/5`, `int f f'' = -F1`). In units
  `h = U = 1`: `F2 = 1 + 3B^2`, `F1 = (3B - 1)^2/3 - 3B(3B - 1)/2 + 9B^2/5 + (B - 1)/2`,
  `F0 = a^2/5 + a b/3 + b^2/7 + ((B - 1)/2)^3/3`. Four end layers (two halves, two ends) over `D_lub U`:

      0 <= D_S/D_lub - 1 <= eps_1 = 2 J(delta) / ((H_b/h)(3B^2 + 1)),   J = (13/35) delta F2 + (12/5) F1/delta + 12 F0/delta^3.

  For `B = 5`: `F0 = 4.3714`, `F1 = 7.3333`, `F2 = 76`; `delta = 1.64 h` gives `J = 68.919` (a quadrature of the 2-D trial field
  gives the same to 1e-14, checked before registering), so with `H_b/h = 500`, **`eps_1 = 0.3627 %`**. The layer `1.64 h` lies
  inside the region below the block at every step (`>= 9.5 h`, A5).

*E2, finite Reynolds number (estimate, not a proof).* For the Navier-Stokes flow `u` of the closed domain the energy balance is
`D U = Phi + dK/dt` (`K` the kinetic energy of the fluid; walls at rest do no work). `Phi >= Phi_S` by the minimum-dissipation
theorem, and with `w = u - u_S`, `Phi = Phi_S + Phi(w)` exactly (the cross term vanishes because `u_S` is a Stokes flow and `w`
is divergence-free with zero boundary values), so the first-order inertial correction to the drag is zero and
`Phi(w) = -rho int a . w`, `a` the Lagrangian acceleration. `a` vanishes in the fully developed part of the gaps (parallel steady
flow in the block frame) and is largest at the four gap ends. A crude bound: `|a| <= V_m^2/h` with `V_m = 4.267 U` the peak
speed of the gap profile relative to the block, on four squares of side `h`, and the Poincare inequality with the channel
width, `||w|| <= (W/pi) ||grad w||`, give `Phi(w) <= rho^2 ||a||^2 (W/pi)^2 / mu`, so

    eps_2 = Phi(w)/(C U^2) <= 4 (V_m/U)^4 (W/(pi h))^2 Re_U^2 / (2 (H_b/h)(3B^2 + 1)) = 0.0636 Re_U^2,   Re_U = U h/nu.

At `Re_U = 0.0995`, **`eps_2 <= 0.064 %`**. (A first-order Bernoulli estimate, inertial pressure at most `rho V_m^2` across
the block, gives `0.012 %`.)

*E3, finite length and finite time.* (i) The block is rigid, so the gap length is constant; the gap profile has developed to
`e^{-t_end/t_m} = e^{-46}`. (ii) The transient left at `t_end = 10 tau`: `e^{-10}` for the closed-form `tau`; allowing the true
time constant up to `1.25 tau` (the end-layer added mass is `+0.6 %`, the discretisation of the drag `-0.8 %`, the viscous memory
decays with `t_m = 0.22 tau`), at most `e^{-8} =` **`0.034 %`**. (iii) `dK/dt`: the gap kinetic energy is constant at the
terminal state; the end regions change height by `0.42 h` of `10 h` during the whole run, and their kinetic energy (at most
`0.6 %` of `m_a U^2/2` for the trial field) changes by a few percent of that, so `|dK/dt|/(D U) < 1e-5`.

*Total*: `eps_1 + eps_2 + eps_3 <= 0.363 + 0.064 + 0.034 + 0.001 = 0.462 %` `<= 0.5 %` (registered as the A0 check below). The
signs: E1 and E2 raise the drag (lower the terminal velocity); E3 (ii) lowers the velocity.

**M5. Discretisation (a), an expectation, not a limit.** For the finite-volume momentum equation across a gap of `n` uniform
cells with the half-cell wall distance, the discrete solution of the Poiseuille part is the parabola plus the constant
`G dx^2/(8 mu)` (the wall equation leaves a defect `G dx/(4 mu)` that a uniform shift removes; the interior equations are exact for
a parabola), and the midpoint sum adds `G h dx^2/(24 mu)`, so the discrete flux at a given `G` is `q (1 + 2/n^2)`; the Couette
part is exact. With the flux fixed by continuity the discrete drag is low by about `2/n^2`: 0.77 % for `n = 16` (a 1-D solve of the
discrete system gives `-0.765 %` for `B = 5`). The expected `U(t_end)/U_t` is therefore about `1 + 0.008 - eps_1,true`, inside the
2 % limit.

**M6. The spring-mounted piston (b).** The C2 channel: `L0 = 1`, `A = 0.04`, `rho = 1`, `nu = 0.01`, piston mass `M = 0.2`, the
outlet `xmax` at `p = 0`, slip (`ZeroGradient`) sides, so the flow is a plug, `u = v_p` everywhere, with no shear. The momentum
balance of the column of length `L = L0 - x` (outflow `rho A v^2` balanced by `d(rho A L v)/dt = rho A (L a - v^2)`) gives the
piston pressure `p_front = rho L a`, as in C2, so with a spring `k` about `x = 0`:

    (M + rho A (L0 - x)) x'' = -k x.

For small amplitude `omega = sqrt(k/(M + rho A L0))`. The mass varies with `x` by `eps = rho A X/(M + rho A L0) = 1.7e-3` of
itself at amplitude `X`; the first-order frequency change averages out over a period (the motion is symmetric about 0), so the
nonlinear period differs from `2 pi/omega` at order `eps^2 = 3e-6` (checked against an RK4 integration of the ODE, B0).
Registered: `M = 0.2`, `rho A L0/M = 0.2`, `k = (2 pi)^2 (M + rho A L0) = 9.474820...` (`T = 1`), `X = 0.01 = 0.01 L0`. With
the fluid force off the frequency is `sqrt(k/M) = 1.0954 omega`, 9.54 % above `omega` (the fluid lowers the frequency by 8.71 %
of the fluid-free one), more than 4 times the 2 % tolerance.

*Coupling lag (expectation).* The flow step `n -> n+1` moves the piston with `(x_{n+1} - x_n)/dt`; the fixed-flux pressure
gradient makes its pressure `rho L` times the acceleration centred at `t_n`, and that force is held over `[t_{n+1}, t_{n+2}]`, a
lag of `1.5 dt`. A lag `tau_l` of the added-mass force gives the damping `m_a omega^2 tau_l` and the added mass
`m_a (1 - (omega tau_l)^2/2)`: a decay of about `exp(-2 pi zeta)` per period with `zeta = 1.5 omega dt m_a/(2 (M + m_a))`
(4.8 % per period at `dt = T/100`, 2.4 % at `T/200`) and a frequency raised by about `3.7e-4` at `T/100`, `0.9e-4` at
`T/200`. The amplitude decays, so B5 expects ratios near 0.95 and 0.98.

## Settings and code

**S1. Test code only.** No library file is changed, no case-schema key is added, nothing in `rcfd-api`, `rcfd-cli` or the case
tree. New file `crates/rcfd-fv/tests/sixdof_flow_f244.rs`. The heavy tests carry
`#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]` (they run in the release gate).

**Set-up FB (falling block, (a)).** Units SI-like: `h = 0.01`, `w_b = 0.04` (`B = 5`), `W = 0.06`, `H_b = 5.0`
(`H_b/h = 500`), region below and above the block `y_b0 = y_a0 = 0.1` (`10 h`), channel height `5.2`, `dz = 0.01`; `rho = 1`,
`nu = 0.01`; `M = 0.03`, `g = 27` (gravity `(0, -27, 0)` on the body, buoyancy hook `(0, rho V_b g, 0)`, `V_b = 0.002`). Closed
forms (M1 to M3): `C = 7.6`, `m_a = 5.3333e-3`, **`m_a/M = 0.1778`** (`m_pot <= 0.1788 M`), `tau = 4.64912e-3`,
`U_t = 0.0994737`, `Re_U = 0.0995` (mean speed relative to the block `3 U`, peak `4.27 U`), `t_m = 0.218 tau`; travel to
`10 tau` `4.16e-3 = 0.42 h`, the region below the block compressed to `0.958`. A block whose sides did not move (Poiseuille only
in the gaps, `C' = 6 mu w_b (w_b + h) H_b dz/h^3 = 6.0`) would fall at `U_t' = 0.126 = 1.267 U_t` (negative control AN1).

Mesh (structured, masked, built in the test with `Mesh::from_cell_faces`, one cell in z): x: 16 uniform cells per gap; the
block span `[h, h + w_b]` two-sided: cells from `h/16` at both ends growing by 1.2 per cell while below `h/4`, the middle filled
with equal cells of at most `h/4`. y: below the block one-sided, from `h/16` at the block face growing by 1.2 per cell while below
`h/2`, then equal cells of at most `h/2` to the wall; the block span two-sided from `h/16` growing by 1.15 while below `2 h`, the
middle equal cells of at most `2 h`; above the block the mirror of below. The cells of the block are removed. Patches: `block`
(the four faces of the hole, wall), `left` (`x = 0`), `right` (`x = W`), `bottom`, `top` (walls), `frontAndBack` (empty). About
12 300 cells; the exact counts go in the addendum.

Flow: all walls fixed velocity zero except `block`, which follows the mesh (`set_follow_conditions`, with its pressure gradient);
no pressure Dirichlet condition (`p_ref` cell 0). `PimpleSettings::piso(nu)` with central convection, momentum solver
`tol_abs 1e-16`, `tol_rel 1e-13`, `max_iter 5000`, pressure the same with `max_iter 20000` and `amg_pressure = true`. Fluid and
body at rest at `t = 0`.

Mesh motion: smoothing classes `block` prescribed, `left` and `right` slip (the gap moves with the block, the side walls slide in
their planes, their swept volume is zero), `bottom` and `top` fixed, `frontAndBack` skipped. Laplacian smoothing
(`smooth_displacement`, tolerance as the momentum solver) of the unit displacement `(0, 1, 0)` of the `block` points, solved once
from the initial mesh; the points of a step are `x_0 + s d_unit`, `s` the body's displacement (exact by the linearity of read 3;
A8 checks it against a direct smoothing at the final `s`).

Coupling (ADR 0033 D7, the loop of C2): `patch_force_moment` on `block` from the state at `t_n`, `Body::advance` with RK4
(`translation_locked = [true, false, true]`, rotation locked), the points from the new body position, `move_mesh`,
`set_follow_conditions`, `step_ale_with_walls`. Runs: `dt = tau/10` (100 steps) and `dt = tau/20` (200 steps), both to
`t_end = 10 tau`.

**Set-up SP (spring-mounted piston, (b)).** C2 exactly (`box_mesh([20, 2, 2], [1, 0.2, 0.2])`, classes and conditions of read 4,
Laplacian smoothing from the initial mesh each step, `piso(0.01)`, central, tight solvers of C2) plus the spring of M6:
`stiffness = [k, 0, 0]`, `rest_position = 0`, `translation_locked = [false, true, true]`, RK4. The body starts at rest at `x = X`
with the fluid at rest on the mesh smoothed to that position. Runs: `dt = T/100` (650 steps) and `T/200` (1300 steps), both to
`6.5 T`. Frequency: from the upward zero crossings of `x(t_n)` (linear interpolation, ADR 0033 R2), `omega_m = 2 pi (N - 1)/(t_N -
t_1)` over all `N` crossings of the run (`N = 6`, five periods; crossings near `0.75 T + k T`). Turning points: the local maxima
and minima of `x(t_n)` refined by the parabola through the three samples, the start `x(0) = X` counted as the first maximum.

Uniform-flow replays (reading 4 (ii)): FB's mesh moved by the closed-form travel `s(t_n)` of M3, `dt = tau/10`, 100 steps; SP's by
`x_p(t_n) = X cos(2 pi t_n)`, `dt = T/100`, 100 steps. Every patch a fixed velocity `u0`, initial `u = u0`, `p = 0`, no pressure
Dirichlet condition. `u0 = (0.3, -0.2, 0)` for FB (z is empty), `(0.3, -0.2, 0.1)` for SP.

## Registered checks

All in `crates/rcfd-fv/tests/sixdof_flow_f244.rs`, release. "Relative" is `abs(a - b)/abs(b)`.

### (a) Falling block

| Id | Check | Limit |
| --- | --- | --- |
| A0 (bound) | E1 by quadrature of the 2-D trial field (Gauss-Legendre, 40 points per piece) against `J(1.64 h)` of M4; the total `eps_1 + eps_2 + eps_3` with the numbers of FB | relative `1e-10`; total `<= 0.5 %` |
| A1 (acceptance) | `dt = tau/10`: `U(10 tau)/U_t` | `abs(. - 1) <= 0.02` |
| A2 (acceptance) | `U(10 tau)` of `tau/20` against that of `tau/10` | relative `<= 0.005` |
| A3 (acceptance) | continuity of every step, both runs | `<= 1e-10` |
| A4 (acceptance) | the discrete GCL of every step, both runs; the total fluid volume against the initial | `<= 1e-12 mean(V)`; `<= 1e-12 V0` |
| A4u (acceptance) | the uniform-flow replay: after every step `max abs(u - u0)` and `max abs(p)` | `<= 1e-12 abs(u0)`; `<= 1e-12 abs(u0)^2` |
| A5 (acceptance) | `min_c V_c(t)/V_c(0)` over every step of both runs (the F-063 compression limit) | `>= 0.5` |
| A6 (acceptance) | `m_a/M` of M2 with the end-layer bound, from the FB numbers | `<= 0.2` |
| A7 (set-up) | `abs(U(10 tau) - U(9 tau))/U_t`, `dt = tau/10` | `<= 1e-3` |
| A8 (set-up) | `max` over the points of `abs(x_0 + s d_unit - x_direct)` at the final `s` of the `tau/10` run, `x_direct` from `smooth_displacement` with the actual displacement | `<= 1e-9 abs(s)` |
| A9 (set-up) | the travel `abs(s(10 tau))` of the `tau/10` run; no cyclic patch | `>= 0.25 h`; none |
| AN1 (negative control) | `U(10 tau)` against `U_t'` (sides at rest) | relative `>= 0.08` |

Printed, no limit: `U(t)/U_t` against `1 - e^{-t/tau}` at `t = tau, 2 tau, 5 tau`; the pressure and momentum iterations; the
cell counts; the travel and the compression.

### (b) Spring-mounted piston

| Id | Check | Limit |
| --- | --- | --- |
| B0 (model) | the period of the ODE of M6 (RK4, `dt = T/2000`, five periods from `x = X`, crossings as in SP) against `2 pi/omega`; `rho A L0/M`; `sqrt((M + rho A L0)/M) - 1` | relative `<= 1e-5`; `<= 0.2`; `>= 0.08` |
| B1 (acceptance) | `dt = T/100`: `omega_m/omega` | `abs(. - 1) <= 0.02` |
| B2 (acceptance) | `omega_m` of `T/200` against that of `T/100` | relative `<= 0.005` |
| B3 (acceptance) | continuity of every step, both runs | `<= 1e-10` |
| B4 (acceptance) | the discrete GCL of every step, both runs | `<= 1e-12 mean(V)` |
| B4u (acceptance) | the uniform-flow replay, after every step | `max abs(u - u0) <= 1e-12 abs(u0)`; `max abs(p) <= 1e-12 abs(u0)^2` |
| B5 (acceptance) | each ratio of successive maxima, and of the magnitudes of successive minima, both runs | `<= 1 + 1e-3` |
| B6 (set-up) | upward crossings per run; `max abs(x)` over the run; no cyclic patch | `>= 6`; `<= 0.01 L0 (1 + 1e-9)`; none |
| BN1 (negative control) | the `T/100` run with the fluid force set to zero before `Body::advance` (mesh and flow as in B1): `omega_m/omega` | `abs(. - 1) >= 0.08` |

Printed, no limit: the turning-point ratios, the amplitude after the run, the iterations.

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named tests are run once, and they must fail; the file is restored and checked
byte for byte against a kept copy. Their numbers go in the addendum. `scripts/limit_audit.py` is run on the new test file and
every hit is read.

| Id | Mutation (library) | Must fail |
| --- | --- | --- |
| MU1 | `patch_force_moment` without the pressure term (`df` starts at zero) | A1, B1 |
| MU2 | `patch_force_moment` without the viscous term | A1 |
| MU3 | `AleStep::fix_ddt` does nothing (old level with the new volume) | A4u, B4u |
| MU4 | `AleStep::relative_flux` returns `phi` (no mesh flux in the convection) | A4u, B4u |

Expected: MU1 leaves the side shear, `32/152` of `D_lub`, so the block falls at about `4.75 U_t`, and removes the added mass of
(b) (`+9.5 %`); MU2 leaves the pressure part, `120/152`, `U` about `1.27 U_t`; MU3 and MU4 break the GCL of the momentum equation
in every cell whose volume changes.

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a `git archive`
copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), through the machine's gate lock
(`gate-lock.sh acquire f244`) and started with at least 20 GB of physical memory free. A failure of a known flaky test is run
again alone on the same build and reported with its name.

### Classes

The defaults are untouched by construction (S1); the existing suites that touch this path (`rcfd-fv` `sixdof`,
`sixdof_coupled`, `ale_flows`, `ale_gcl`, `mesh_smoothing`, `rcfd-api` `dynamic_mesh`, `baseline_hashes`, H1 of
`case_hashes_w48u`) must pass in the gate unedited. A miss of an acceptance or bound check is reported with its number and kept
as an ignored `*_known_miss` test with the limit intact. A set-up check (A7 to A9, B0, B6), a negative control (AN1, BN1) that does
not hold, or a mutation that does not make its tests fail, means the check is not shown to be able to fail: it is reported as a
defective set-up, corrected by an amendment before any re-run, and never counted as a pass.

**Decision rule.** Every registered check met (A0 to A9 with A4u, B0 to B6 with B4u), the negative controls held and every
mutation failed its tests: F-244 is set done (`[x]`), acceptance still the maintainer's. A miss of any acceptance check: partial with the miss named.
Neither case runs: todo.

## Not delivered

A case-tree or CLI version of either case (Q2); strong coupling and added-mass stabilisation (F-251); a mass ratio above 0.2;
cross-flow vortex-induced vibration; published falling-body experiments or wall-correction tables; free rotation of the falling
block (its rotation is locked; a centred block in a symmetric channel has no torque at rest, but the stability of that symmetry is
not tested); a mesh-convergence study of (a) (the 2 % limit absorbs the expected 0.8 % of M5); Newmark in the coupled loop (RK4
only, as C2).

## Open questions and choices for the maintainer

- **Q1. The refined acceptance text.** The roadmap note says the refined F-244 text of 2026-10-21 is "not yet user-approved"; the
  user decision of 2026-10-22 approved the split and the reading "free decay in still fluid". Options: (a) take the refined text as
  approved by that decision; (b) review it first. Recommendation (a); this ADR implements it as written.
- **Q2. A case-tree version.** Not in the acceptance. The tree has `six_dof.stiffness` and `six_dof.gravity`, so (b) could run as
  a case (as ADR 0033 T4 ran C2), but (a) needs a block-in-channel mesh the tree cannot generate and a buoyancy force the tree does
  not have. Options: (a) none; (b) a tree version of (b) only, registered as a follow-up under F-244's note. Recommendation (a):
  the tree path of six-DOF coupling is already checked by T4.
- **Q3. Status.** If every check is met the acceptance as written is met by library tests. Options: (a) done; (b) partial until a
  tree version exists. Recommendation (a).

## Amendment 1 (2026-11-07): the time step of the falling block, written before the corrected run

**What happened.** The test file was written as registered (S1) and every registered test was run once, in release, on
7a36513 plus the new file: A0, A4u, B0, B1 (with B2 to B6), B4u and BN1 ran to the end (their numbers go in the addendum; none
is repeated). The registered A1 run (`dt = tau/10`) did not finish: at step 31 `move_mesh` stopped with "mesh error: cell 0 has
non-positive volume", so no quantity of A1 to A9 or AN1 was measured. Before that the set-up smoke (two steps, counts only) had
shown 12 320 cells (grid 56 x 343, block 24 x 287 cells), as estimated in FB.

**Diagnosis** (scratch tests, not committed, release; each printed the fluid force on the block every few steps). From step 10
on, the force of the coupled run alternates in sign and grows by a factor of about -3.4 per step, its x component as well,
although the body's x translation is locked, so the growth is in the flow and not in the coupling. Then:

| Variant (everything else as FB) | `dt` | Result |
| --- | --- | --- |
| prescribed travel `s(t)` of M3, no body | `tau/10` | grows from step 15, non-finite by step 30 |
| the same without the Rhie-Chow transient term; IC0-CG pressure; upwind convection | `tau/10` | the same in each |
| the same with 3 correctors | `tau/10` | grows from step 30, slower |
| PIMPLE, 5 outer iterations, no relaxation | `tau/10` | grows from step 4 |
| prescribed travel | `tau/20` | grows from step 30 |
| prescribed travel | `tau/40` | 400 steps, no growth (x force below 2e-9 throughout) |
| **fixed mesh**, block at rest, its faces given the velocity `(0, -U(t))`, `step` (not ALE) | `tau/10`, `tau/20` | grows as with the moving mesh |
| fixed mesh, cells along the block capped at `h` (aspect 16) or `h/2` (aspect 8) | `tau/10` | grows as with `2 h` |
| fixed mesh, registered mesh | `tau/40` | 400 steps, no growth |
| a straight 2-D channel (16 x 40 cells of aspect 32, one wall moving with `U(t)`, open ends at `p = 0`) | `D` = 12, 6, 3 | no growth (the parallel flow is exact, nothing seeds a mode) |

On the registered mesh the growing mode sits in the gap cells where the y grading reaches its cap (`y = 0.24` to `0.28` and the
mirror positions), symmetric about mid-height and between the two gaps. Conclusion: the baseline PISO step of rcfd is unstable on
this geometry above a time step between `tau/40` and `tau/20`, that is, a cell diffusion number across the gap
`D = nu dt/dx^2` between 3.0 and 6.0. It is not caused by the moving mesh, the six-DOF coupling, the Rhie-Chow transient term,
the pressure solver, the convection scheme or the cell aspect ratio. The mechanism (the PISO splitting at large diffusion
numbers near the gap ends is the likely candidate) is not proven, and nothing in the solver is changed (S1). This is a
baseline-solver limitation; it joins the list of ADR 0033's addendum (Q4).

**Disclosed.** The `tau/40` prescribed-travel diagnostic moves the block at the closed-form trajectory of M3, so its last force
is the discrete drag at `U_t`: `0.7518` against `F_net = 0.7560`, so the discrete drag at `U_t` is `0.9944 D_lub`. This
foreshadows the corrected A1: with a drag linear in `U`, the terminal velocity would be about `1.0056 U_t`. The limits are not
changed by it.

**Corrected set-up (registered now, before its run).** FB with `dt = tau/40` (400 steps) and `dt = tau/80` (800 steps) to
`t_end = 10 tau`; everything else as registered. The new test `a1b_falling_block_reaches_the_lubrication_terminal_velocity`
checks A1 to A3, A4, A5, A7 to A9 and AN1 with their registered limits on these two runs: A1 on `tau/40`, A2 `tau/80` against
`tau/40`, A3 to A5 on both runs, A7 `abs(U(10 tau) - U(9 tau))/U_t` and A8, A9 on the `tau/40` run. The registered test keeps its
set-up and becomes the ignored `a1_falling_block_reaches_the_lubrication_terminal_velocity_known_miss` (the record of this
amendment). A4u keeps its registered `tau/10` replay (a uniform flow has nothing to seed the mode, and it ran once, above).
Expected Courant number at the gap ends: 0.08 at `tau/40`. The decision rule counts A1 to A9 from the corrected test.

Added to the open questions above: **Q4.** The time-step limit of the baseline PISO found here. Options: (a) a new roadmap item to diagnose
and remove it (a fixed-mesh reproduction exists: the fixed-mesh variant of the table, block at rest, `tau/10`); (b) record it
only. Recommendation (a): an implicit scheme that needs `D <= 3` across the finest cells limits every narrow-gap case.

## Addendum 1: what was built and the results (2026-11-07)

Measured against the limits registered above and in amendment 1; no limit was changed. Release builds on Windows. The
registration is 7a36513, amendment 1 is 9978c5f, the test file is 4c12b68, and this addendum's commit also carries a
layout fix of three printed lines of that file (no value or assertion changed). Each registered check ran once; the runs are
deterministic and none was repeated to change a result.

**Built as registered.** S1 as written: test code only. `crates/rcfd-fv/tests/sixdof_flow_f244.rs` holds nine tests: `a0`
(A0, A6), `fb_setup_smoke`, `a1_..._known_miss` (the registered `tau/10` set-up, ignored, amendment 1), `a1b` (A1 to A5, A7 to
A9, AN1 at `tau/40` and `tau/80`), `a4u`, `b0`, `b1` (B1 to B6), `bn1`, `b4u`. The heavy ones are ignored in debug builds only.
No library file, case-schema key, golden, `baseline_hashes.rs` or `case_hashes_w48u.rs` was touched. Documents:
`docs/dynamic-mesh.md` (the validation and the time-step limit), and the F-244 entries of `docs/ROADMAP.md` and
`docs/roadmap.json` (done, by the decision rule).

FB mesh as built: 12 320 cells, 26 060 points, grid 56 x 343 with 24 x 287 cells removed (16 cells per gap, 24 across the
block span, 28 rows below and above the block, 287 along it; the middle cells along the block are `1.9975 h` tall, aspect
32). Patches: `block` 622 faces, `left` and `right` 343, `bottom` and `top` 56, `frontAndBack` 24 640.

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| A0 trial quadrature against `J(1.64 h)` | relative `1e-10` | `6.2e-16` (`J = 68.919062`) | met |
| A0 bound of the neglected terms | `<= 0.5 %` | `eps_1 0.3627 % + eps_2 0.0630 % + eps_3 0.0345 % = 0.4602 %` | met |
| A6 added-mass ratio | `<= 0.2` | `m_a/M = 0.1778`, with the end layers `0.1788` | met |
| A1 (acceptance) `U(10 tau)/U_t`, `tau/40` | `abs(. - 1) <= 0.02` | `1.00545` | met |
| A2 (acceptance) `tau/80` against `tau/40` | `<= 0.005` | `3.4e-6` (`1.00545` both) | met |
| A3 (acceptance) continuity, every step | `<= 1e-10` | `1.89e-11` (`tau/40`), `3.88e-11` (`tau/80`) | met |
| A4 (acceptance) GCL every step; total volume | `<= 1e-12 mean(V)`; `<= 1e-12 V0` | `6.46e-16`, `6.49e-16`; `5.96e-14`, `5.81e-14` | met |
| A4u (acceptance) uniform flow on the moving FB mesh | `<= 1e-12 abs(u0)`; `<= 1e-12 abs(u0)^2` | `4.2e-15`; `0` | met |
| A5 (acceptance) `min V(t)/V(0)` | `>= 0.5` | `0.9164` (both runs) | met |
| A7 (set-up) `abs(U(10 tau) - U(9 tau))/U_t` | `<= 1e-3` | `7.1e-5` | held |
| A8 (set-up) scaled unit field against direct smoothing | `<= 1e-9 abs(s)` | `9.1e-13` | held |
| A9 (set-up) travel; cyclic patches | `>= 0.25 h`; none | `0.419 h`; none | held |
| AN1 (negative control) against `U_t'` (sides at rest) | `>= 0.08` | `0.206` | held |
| B0 model period; mass ratio; fluid-free shift | `1e-5`; `<= 0.2`; `>= 0.08` | `1.2e-7`; `0.2`; `0.0954` | met |
| B1 (acceptance) `omega_m/omega`, `T/100` | `abs(. - 1) <= 0.02` | `1.001307` | met |
| B2 (acceptance) `T/200` against `T/100` | `<= 0.005` | `1.96e-4` (`1.001111`) | met |
| B3 (acceptance) continuity, every step | `<= 1e-10` | `3.88e-15`, `3.47e-15` | met |
| B4 (acceptance) GCL every step | `<= 1e-12 mean(V)` | `6.09e-16`, `5.99e-16` | met |
| B4u (acceptance) uniform flow on the moving SP mesh | `<= 1e-12 abs(u0)`; `<= 1e-12 abs(u0)^2` | `1.3e-15`; `0` | met |
| B5 (acceptance) largest ratio of successive same-side turning points | `<= 1 + 1e-3` | `0.95299` (`T/100`), `0.97604` (`T/200`) | met |
| B6 (set-up) crossings; `max abs(x)`; cyclic patches | `>= 6`; `<= 0.01 L0 (1 + 1e-9)`; none | 6 and 6; `1.0e-2`; none | held |
| BN1 (negative control) fluid force off | `abs(. - 1) >= 0.08` | `1.095445` (7 crossings), off by `0.0954` | held |
| A1 as registered (`tau/10`) | as A1 | did not finish (amendment 1) | ignored record |

Printed, no limit. The approach at `tau/40`: `U/U_t` `0.642`, `0.873`, `0.9991` at `tau`, `2 tau`, `5 tau` against
`0.632`, `0.865`, `0.9933` of the closed form, so the coupled approach is a little faster than the quasi-steady model of M3
(the instantaneous added mass is the smaller potential one, M2). Pressure iterations (AMG-CG, two correctors): 9 384 over the
400 steps and 17 681 over the 800. The turning points of B5 decay by 4.7 percent per period at `T/100` and 2.4 percent at
`T/200`, as the coupling lag of M6 predicts (4.8 and 2.4). B1's `+0.13 %` is above the lag's expected `+0.04 %`; the difference
is not diagnosed and is within the limit.

Against the expectations of M4 and M5: the discrete drag at `U_t` was `0.9944 D_lub` (amendment 1). The 1-D discrete gap gives
`0.99235`, so the discrete end regions add about `0.21 %` to the drag, inside the registered `0 <= eps_1 <= 0.363 %` of the
continuum (consistent with, though not a check of, E1).

### Mutations

Each was applied alone to the code of 4c12b68 (with the print layout fix) by an exact text replacement (a script that checks
that each replaced text occurs once), the named tests were run once, and the file was restored and compared byte for byte with
the kept copy (equal in all four).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 no pressure in `patch_force_moment` | A1, B1 | A1 failed: `U(10 tau)/U_t = 4.369` (and A7 `0.115`); B1 failed: `1.095445`, the fluid-free frequency, with turning-point ratios exactly 1 |
| MU2 no viscous term in `patch_force_moment` | A1 | failed: `1.2728` (AN1 `0.0049`: the block falls at the speed of a block whose sides do not move, as M1 says it should) |
| MU3 `AleStep::fix_ddt` does nothing | A4u, B4u | both failed: `9.8e6 abs(u0)` (FB, the flow blows up), `2.4e-3 abs(u0)` (SP) |
| MU4 `AleStep::relative_flux` returns `phi` | A4u, B4u | both failed: `1.7e7 abs(u0)`, `2.4e-3 abs(u0)` |

`scripts/limit_audit.py` reports two hits in the new file, both class (i), B0's assertions on the mass ratio and the
fluid-free frequency shift: they check the registered constants (as the acceptance asks that the parameters be chosen so), so
they cannot fail unless the constants change; read and kept.

Disclosures:

- Order of the runs: the smoke, then the registered run of every test once (A1 crashed, amendment 1), then the diagnostics of
  amendment 1, then the corrected `a1b` once, then the mutations.
- Between the registered runs and `a1b`: the diagnostic code was removed; the A1 checks moved into a helper called by the
  ignored registered test and by `a1b` (the same checks and limits); `std::iter::repeat_n` in the FB mesh builder became
  `Vec::resize` with the same value (clippy's MSRV lint; the mesh is identical, and A4u, which uses it, had already run).
- After `a1b`: three printed lines of the A1 helper had their string continuations broken by a scripted edit (spaces in the
  output); they were fixed by hand. No value or assertion changed.
- Not exercised: Newmark in the coupled loop, a free rotation of the falling block, a mesh study of (a) (Not delivered).

Status by the decision rule: every registered check met (A1 to A9 from the corrected test), both negative controls held, every
mutation failed its tests. F-244 is set done in the roadmap; the acceptance is still the maintainer's. Q1 to Q4 stay open.

### The gate (2026-11-07)

Main stayed at c744b5f while F-244 was built, so no merge was needed; 0211 is on no other branch. The gate ran on a
`git archive` copy of 95af0e1 (this addendum's commit before this section) with three jobs and one process per test
(`scripts/gate_test.py`), in the target dir `E:/projects/cfd/w49i-work/target-f244-gate`. It took the machine's gate lock at
09:49 (UTC) when the F-249 gate released it and started with 25 GB of physical memory free: `cargo fmt` per package 0,
`cargo clippy --workspace --all-targets -- -D warnings` 0, tests **3055 passed, 2 failed, 866 ignored** (3057 test processes in
590 binaries, wall 4720 s, peak summed working set 3435 MB, largest time against its limit 0.218, lowest free physical memory
18 GB). The lock was released at the end (11:21).

The two failures are `rcfd-runner` `runner_w47b` s1 and s9, both known flakes of the gate. Run again alone on the gate's binary,
s9 passed; s1 failed the same way as in the F-292 and F-258 gates (its negative control saw status 3 and OS 0 under
`max_runs = 3`). The runner tests do not run the flow solver. All eight non-ignored tests of this ADR ran in the gate and passed
(`a1b` 112 s), with the ignored known-miss record of amendment 1; the suites named under Classes (`sixdof`, `sixdof_coupled`,
`ale_flows`, `ale_gcl`, `mesh_smoothing`, `rcfd-api` `dynamic_mesh`, `baseline_hashes`, H1 of `case_hashes_w48u`) passed
unedited. F-244 adds one ignored test (the known-miss record). This gate section and the wording fix in amendment 1 ("the
open questions above") are documents only and were not gated again.
