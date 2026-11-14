# ADR 0434: F-401 registration: two-way coupling of an elastic bar with the incompressible flow solver, the piston on an elastic bar

Status: **REGISTERED 2026-11-14, written before any code of F-401 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: the design stub [ADR 0304](0304-two-way-coupling-with-the-flow-solver.md) (SI-14) and the intermediate-tier plan
[ADR 0290](0290-structural-solver-intermediate-tier-plan.md). Builds on [ADR 0410](0410-f400-registration-dynamic-solid-participant.md)
(F-400: `DynamicSolid`, the bar, the added-mass finding of its amendment 1), [ADR 0250](0250-f251-registration-strong-fluid-body-coupling.md)
(F-251: the piston column, its added mass and the lag of the pairing, D1, D3, D4), [ADR 0252](0252-f351-registration-added-mass-deficit-wall-pressure.md)
(F-351: the prescribed wall gradient of the force, the default of `FlowWall`), [ADR 0256](0256-f139-registration-cosimulation-participant.md)
(F-139: `coupled_step`, `FlowWall`), [ADR 0261](0261-f361-registration-quasi-newton-coupling.md) (F-361: the least-squares
update), [ADR 0263](0263-f362-registration-interface-mapping.md) (F-362: `InterfaceMap::build`), [ADR 0412](0412-f502-registration-nonmatching-coupled-frequency.md)
and [ADR 0414](0414-f504-registration-bilinear-quad-projection.md) (F-502, F-504: the frequency change on 3:2 walls, the opt-in
bilinear projection). Roadmap item F-401 (deps F-400, F-251, F-351, F-361, F-362). Size M. Number 0434 given by the lead.
Branch `f401-piston-two-way` (from main `dcdda624`), worktree `w49i-work/f401`, scratch `w49i-work/f401-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; a
missed limit becomes an ignored `*_known_miss` test with the limit intact; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; existing tests are not edited; no new dependency, no unsafe code;
clean-room (every formula below is derived here or is the Newmark recurrence of ADR 0389 and the flow step of the repository).

## What F-401 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. Elastic bar
(nu = 0) closing a channel of incompressible fluid of length L_f, started from a displaced state, mass ratios rho_f L_f/(rho_s
L_s) of 0.5 and 4: the coupled iteration converges in every step to 1e-6 with Aitken's factor and with the least-squares update,
iteration counts recorded; the frequency of the first coupled mode equals the root of k L tan(k L) = rho_s L_s/(rho_f L_f) within
2 percent after the registered estimate of the time-step and viscous effects; over ten periods the work of the fluid on the
interface equals the change of the solid's energy within 1e-4 of its amplitude; the loose coupling (one pass) at ratio 4 grows
(registered control). With a 3:2 non-matching interface mapped by F-362 the frequency changes by less than 2e-3. A failure of
either participant leaves both at the start of the step."

The roadmap note to honour: "ADR 0410 (F-400, 2026-11-13) amendment 1: with an added mass that acts per wall face, the interface
force components below the convergence test are loosely coupled and grew by about 700 per step on the bar of F-400; the
registration of this item should check the growth of the non-uniform part of the interface force over the run (ADR 0410 Q3)."

Clauses: (c0) every other limit registered here; (c1) bar `nu = 0`, a channel of length `L_f`, started from a displaced state,
ratios 0.5 and 4; (c2) every step converges to `1e-6` with Aitken and with least squares, iteration counts recorded; (c3) the
frequency equals the root within 2 % after the registered time-step and viscous estimate; (c4) ten periods: fluid work equals the
change of the solid's energy within `1e-4` of its amplitude; (c5) the loose coupling at ratio 4 grows; (c6) 3:2 interface mapped
by F-362: frequency change below `2e-3`; (c7) a failure of either participant leaves both at the start of the step; (c8, note)
the growth of the non-uniform part of the interface force is checked.

## What was read (no Rust run)

ADR 0304, 0410 (whole, with its amendments and addendum), 0414 (addendum), 0412 (summary lines), 0250 (D1 to D8 and the
addendum), 0252 (the addendum), the head of 0256; `crates/rcfd-fv/src/cosim.rs` (whole: `coupled_step`, `FlowWall`,
`MeshMotion`, `CouplingSettings`); `crates/rcfd-post/src/solid_dynamic.rs` (whole); the stub `crates/rcfd-post/src/solid_fsi.rs`
(one comment line); the set-ups of `crates/rcfd-fv/tests/cosim_f139.rs` (the channel, the fluid bits) and
`crates/rcfd-fv/tests/strong_fsi_f251.rs` (the piston channel), `crates/rcfd-post/tests/si_solid_dynamic.rs` and
`si_solid_dynamic_bilinear.rs` (the bar, the 1D chain, the maps); `rcfd_mesh::gen::{box_mesh, axis_coords}`,
`rcfd_fem::transient::{TransientOptions, LoadHistory, Method}`, `rcfd_fem::Load`, `PimpleSettings::piso`. Nothing was built or
run in Rust.

**Arithmetic before this registration (disclosed).** One Python script, `w49i-work/f401-scratch/prearith.py` (numpy), on the
1D reduction of the bar (ADR 0410 D5) coupled to the plug column of D3 below, started as in D4: the roots `k L =
1.076873986312` (ratio 0.5, `omega = 1358.892 rad/s`, `dt = T/100 = 4.6238e-5 s`) and `0.480094436957` (ratio 4, `omega =
605.825 rad/s`, `dt = 1.03713e-4 s`); the discrete coupled model's crossing frequency against the root over 1001 steps
`+1.566e-4` (ratio 0.5) and `-1.055e-3` (ratio 4) (20 crossings each); the same model without the lag of the flow (the
Newmark-consistent piston of ADR 0410) `+4.73e-4` and `-3.55e-4`, so the lag moves the frequency by `-3.2e-4` and `-7.0e-4`; the
solid's energy after the ten periods `0.107` and `0.0266` of its value after the first step (the lag damps); the model's energy
balance `4e-15` and `7e-15`; the bound `sum |du| max(|H|, P)/E_1` `47.2` and `37.2`; the slope of the strong iteration
`-(m_a/4) e^T S^{-1} e` `-3.840` and `-14.923`; the loose coupling at ratio 4 passes `10 U` at step 3, the force growing by
`14.8` per step (ratio 0.5: step 9, `2.2` to `3.2`); the first discrete coupled mode's share of the initial tip displacement
`0.9636` (ratio 0.5) and `0.9988` (ratio 4). Also the geometric grading of D4 (`Expansion(15)` on 24 cells over 32 m: first
cell 0.25 m). Nothing else was computed. The tests compute their own predictions (D8); no printed number is pasted into a test.

## Design

### D1. The coupled frequency equation for these boundary conditions (derived)

The bar `0 <= x <= L_s`, section `A`, density `rho_s`, modulus `E`, `nu = 0` (so the axial field is one-dimensional, ADR 0410
D5), clamped at `x = 0`. Its end face `x = L_s` closes a straight channel of the same section `A` filled with incompressible fluid
of density `rho_f` up to the open end `x = L_s + L_f`, where the pressure is zero; the side walls are impermeable slip walls.

The fluid: incompressibility in a channel of constant section with impermeable walls makes the axial velocity uniform over the
column (the flux through every section is the flux through the moving wall), so the flow is a plug with the wall's velocity and
no shear. The momentum balance of the column, `rho_f a = -dp/dx`, with `p = 0` at the open end, gives on the wall `p = rho_f
L_f a`, `a` the wall's acceleration (to first order in the displacement: the column is `L_f - u(L_s)`). The force on the bar's
end face is `-p A = -rho_f A L_f a`: the column is an added mass `m_a = rho_f A L_f`, exactly (ADR 0250 D1). If the far end were
closed the fluid could not move and the end would be clamped; it is open here.

The bar: `rho_s u_tt = E u_xx`, `u(0, t) = 0`, and at the end the traction balances the fluid force, `E A u_x(L_s, t) = -m_a
u_tt(L_s, t)`. With `u = sin(k x) cos(omega t)`, `omega = k c`, `c^2 = E/rho_s`: `E A k cos(k L_s) = m_a k^2 c^2 sin(k L_s)`,
i.e. `tan(k L_s) = rho_s A/(m_a k)`, i.e.

`k L_s tan(k L_s) = rho_s A L_s/m_a = rho_s L_s/(rho_f L_f)`,

the equation of the acceptance with `L = L_s`. Its first root lies in `(0, pi/2)`; `omega = (k L_s) c/L_s`, `T = 2 pi/omega`.

### D2. The pairing and its discrete model (the time-step estimate, derived)

The solid is `DynamicSolid` (ADR 0410) with the average-acceleration Newmark method, one integrator step per coupling step; the
fluid is `FlowWall` (ADR 0256) with its default prescribed wall gradient (ADR 0252), the moving-mesh PISO step, implicit Euler.
In coupling step `n` (`t_n -> t_{n+1}`) the solid holds the force `H_n`; at convergence `H_n` is the flow's force at `t_{n+1}`.

The flow step moves the wall from `u_n` to `u_{n+1}` (tip values), so the wall velocity it imposes is `w_{n+1/2} = (u_{n+1} -
u_n)/dt` and, for the plug, the pressure of the new state is `rho_f L_f (w_{n+1/2} - w_{n-1/2})/dt` (ADR 0250 D3; reproduced by
the corrected force to `1e-5` in frequency, ADR 0252 A2, K5). The force at `t_{n+1}` is therefore

`F_{n+1} = -m_a (u_{n+1} - 2 u_n + u_{n-1})/dt^2`,

the second difference centred at `t_n`: a lag of `dt` behind the Newmark acceleration `a_{n+1}` that the solid balances. The
converged pair is, on the 1D chain of ADR 0410 D5 (20 linear elements, consistent mass, tip unknown `e`), the Newmark step with
`M a_{n+1} + K u_{n+1} = F_{n+1} e`, `u_{n+1} = u* + (dt^2/4) a_{n+1}`, `u* = u_n + dt v_n + (dt^2/4) a_n`, which is linear in
`a_{n+1}`: `(S + (m_a/4) e e^T) a_{n+1} = -K u* - (m_a/dt^2)(u*_tip - 2 u_{n,tip} + u_{n-1,tip}) e`, `S = M + (dt^2/4) K`. This is
**the discrete model**: it contains the spatial error of the chain, Newmark's period error and the lag of the pairing. The test
integrates it from the initial state of D4 and measures its frequency with the same crossings as the run (D7), giving
`Omega_model`. The registered estimate of the time-step effect is `delta_dt = Omega_model/omega - 1` (arithmetic: `+1.57e-4` at
ratio 0.5, `-1.06e-3` at ratio 4).

What the lag does (for the reader; the model contains it exactly): a force `-m_a a(t - tau)` is `-m_a a cos(omega tau) - m_a
omega sin(omega tau) v`: a damping and a reduction of the added mass by `(omega tau)^2/2`. With `tau = dt` and `omega dt = 2
pi/100` the damping ratio is `zeta = (omega dt/2) s_a`, `s_a` the added mass's share of the mode's kinetic energy (0.92 at ratio
4): the amplitude falls to about 0.16 over ten periods at ratio 4, 0.33 at ratio 0.5 (arithmetic: energy `0.027` and `0.107`).
The fluid takes that energy; check EN measures it on the interface.

### D3. The viscous effect (derived: zero for this flow)

The plug has a uniform velocity, so its strain rate is zero everywhere: the viscous term `nu laplacian(u)` vanishes, the slip side
walls carry no shear, and the viscous part of the wall force, `rho nu (grad u + grad u^T) . n`, is zero on the end wall. The
discrete flow represents the uniform field exactly on the moving mesh (the Laplacian of a uniform field is zero; the side walls are
`Symmetry`, impermeable and free of shear). So the registered viscous estimate is **`delta_visc = 0`**. It is checked (VS): the
same short run with the viscosity times 100 gives the same tip motion within `1e-6 U`. A non-uniform wall motion (the 3:2 map,
D6) drives a local non-plug flow whose viscous force is of order `nu/(omega delta^2)` of its inertial force, `delta` the decay
length `W/pi`: `1e-4/(600 * 0.1) = 2e-6` of a part that is itself below `1e-3` of the force (D6): neglected.

The acceptance's frequency check is therefore `abs(Omega_m/omega - 1 - delta_dt - delta_visc) <= 0.02`.

### D4. The set-ups

**Bar** (ADR 0410 D4 with a 4 x 4 end face, as ADR 0414's `n = 4`): `hex_box([0,0,0], [L_s, W, W], [20, 4, 4])`, `L_s = 4 m`,
`W = 1 m` (`A = 1 m^2`), Hex8, `E = 2e11 Pa`, `nu = 0`, `rho_s = 7850 kg/m^3`, all components fixed on `xmin`, interface the
surface set `xmax` (16 faces, 25 nodes). `DynamicSolid::new("solid", ..., TransientOptions::new(dt), 1)`.

**Channel**: `box_mesh([24, m, m], [32, 1, 1], [Expansion(15), Uniform, Uniform])`, shifted by `L_s` in `x` (points through
`moved_mesh`), so `L_f = 32 m`, the wall patch `xmin` on the bar's end face; `m = 4` (matching) or `m = 6` (3:2). First cell
0.25 m. Velocity: `Symmetry` on `ymin`, `ymax`, `zmin`, `zmax` (impermeable slip, ADR 0256 amendment 1), the follow condition on
`xmin` (set by `FlowWall`), zero gradient on `xmax`; pressure `FixedValue 0` on `xmax`. `PimpleSettings::piso(nu)` with
`nu = 1e-4 m^2/s`, central convection, momentum and pressure solvers `tol_abs = 1e-16`, `tol_rel = 1e-13`, 5000 iterations (as
`cosim_f139`). `FlowWall::new(flow, "xmin", &mesh0, rho_f, nu, motion)` with the default `WallPressure::PrescribedGradient`.

**Mass ratios** by the fluid density, `L_f` fixed: `rho_f = r rho_s L_s/L_f`, `r = 0.5`: `490.625 kg/m^3`; `r = 4`: `3925
kg/m^3`. Why the long channel: the fluid density is then at most `0.5 rho_s`, which keeps the slopes of the non-uniform force
patterns far below 1 (D6); with `L_f = L_s` the fluid would be four times denser than the bar at ratio 4.

**Time step** `dt = T/100`, `T = 2 pi/omega` of the root (D1); 1001 coupled steps (the energy window `t_1` to `t_1001` is ten
periods).

**Initial state: displaced, at rest, consistent.** The bar is held by a uniform traction `P/A` in `+x` on `xmax`
(`Load::traction`), `P = E A U/L_s`, `U = 1e-3 m`, released at `t = 0`: its load table is `(0, 1), (dt, 0), (t_end, 0)`
(`LoadHistory::model_loads`). The bar starts at the static solution of that load, `u0 = U x/L_s` in `x` at every node (exact for
`nu = 0`: a linear axial field under the consistent load of a uniform traction, ADR 0410 D5), `v0 = 0`, so `DynamicSolid::start`
gives `a0 = M^{-1}(f(0) - K u0) = 0` up to rounding (S0). The fluid starts at rest, `p = 0`, on the mesh of the displaced wall
(the motion of D5 applied to the map of `u0`): at rest and unaccelerated it has no force, and that is what the solid's `a0 = 0`
assumes. So solid and fluid start in one consistent state. Taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation); alternatives: (a) the displaced mode shape at rest without a holding load: `a0` from the solid alone misses the
added mass at the tip and is wrong there by `m_a (M^{-1})_{tip,tip}` times the tip acceleration, about 240 times it on this chain
at ratio 4, a first-step displacement error of about `0.24 U` (arithmetic of the reasoning, not run); (b) the initial velocity of
ADR 0410 with the fluid moving as a plug: not a displaced state. The release excites the higher coupled modes by the static
shape's share outside the first mode (arithmetic: 3.6 % of the tip displacement at ratio 0.5, 0.12 % at ratio 4); the model of D2
starts from the same state, so the frequency comparison contains this.

**Maps**: matching (`m = 4`): `MapSettings::projection(1e-9)` (the wall points are the face's nodes: the rows are the identity,
M0); 3:2 (`m = 6`): the opt-in bilinear projection of F-504, `MapSettings::bilinear_projection(1e-9)`, a variant of F-362's
projection; both through `InterfaceMap::build` with the solid's `node_coordinates` and `elements` and the fluid's
`point_coordinates` and `face_points`. Why the bilinear projection on a 4 x 4 face: ADR 0412/0414 measured, on this bar with the
piston mock at ratio 4, the matching-to-3:2 frequency change `-6.28e-3` (default projection, 2 x 2), `-1.79e-3` (default, 4 x 4),
`-2.17e-3` (bilinear, 2 x 2), `-8.18e-4` (bilinear, 4 x 4), `-2.52e-4` (radial basis, 4 x 4); the 4 x 4 face with the bilinear
projection is the first choice with a margin of more than 2 below `2e-3` that keeps the weights symmetric. Taken on the
standing instruction (alternatives: radial basis on 4 x 4, larger margin, not a projection; default projection on 8 x 8, a
larger solid). This choice is made from repository numbers before the run.

**Coupling settings**: `CouplingSettings::aitken(0.5)` and `CouplingSettings::least_squares(0.5)` with `tolerance = 1e-6`,
`force_scale = |f_P|` (the Euclidean norm of the interface nodal forces of the release load, the scale of the force at the start,
as `F_scale` of ADR 0250 D7), `AtCap::Stop`, caps of check I. Loose control: `max_iterations = 1`, `AtCap::Keep`.

### D5. The mesh motion of the channel (the library helper, `rcfd_post::solid_fsi`)

`ChannelMotion::new(mesh, patch, axis, length, tolerance)`: for every point of `mesh` the wall point of `patch` (wall points in
the order of `FlowWall::point_ids`) with the same transverse coordinates within `tolerance`, and the weight `max(0, 1 - |x_a -
x_wall|/length)` (`x_a` the point's coordinate along `axis`, `x_wall` the wall point's). `positions(d)` returns `p + w d_a e_a`
for every point, `d_a` the axial component of its wall point's displacement: the channel stretches linearly from the wall to
`length` from it and is fixed beyond. `into_motion()` gives the `cosim::MeshMotion` for `FlowWall`. Refuses: an unknown or empty
patch, `axis > 2`, a `length` or `tolerance` that is not positive and finite, a mesh point with no wall point on its line, and in
`positions` a wrong count or a non-finite value. Only the axial component is applied: the transverse displacement of the
interface is not followed by the fluid mesh (it would move the points of the slip walls off their planes); the runs record its
size (TR). Here `length = L_f`, so the outlet points have the weight 0 exactly. With an axial motion the mesh flux is that of a
uniformly stretched column; `move_mesh` keeps the volume balance (ADR 0250 S5).

### D6. The iteration, and the non-uniform part of the interface force (the roadmap note)

Uniform part. With the matching map the force patterns that the iteration sees are the consistent pattern `w` of the face (ADR
0410 D5), and the map `H -> Ft` is affine with slope `s = -(m_a/4) e^T S^{-1} e` (D2: the force depends on `u_{n+1}` through
`(dt^2/4) a_{n+1}`); arithmetic `-3.84` and `-14.92`. Aitken's secant lands on the fixed point at the third iterate, the
least-squares update likewise (ADR 0410 D6); the flow is affine up to the mesh motion's effect on the column (`U/L_f = 3e-5`) and
the solver tolerance, so 3, at most 4, iterations per step (F-251 measured 3 to 4 at `1e-8`). The loose coupling multiplies the
force by about `s` per step (arithmetic `14.8` at ratio 4): it grows (c5).

Non-uniform part. A wall motion with zero mean over the section drives a local flow that decays from the wall over `W/(j pi)`
(the cosine modes of the section, ADR 0256 `omega_j`): its added mass per area is `rho_f/kappa_j`, `kappa_j = j pi/W`, not
`rho_f L_f`. The solid answers such a pattern within one Newmark step locally too: an effective mass per area of about
`rho_s/kappa_j` times `1 + (kappa_j c dt)^2/4` (the stiffness term of `S`; `kappa_1 c dt = 1.64` at ratio 4). With the factor
`1/4` of D2 the slope of a non-uniform pattern is about `-(1/4)(rho_f/rho_s)/(1 + (kappa_j c dt)^2/4)`: `-0.07` at ratio 4,
`-0.01` at ratio 0.5 for `j = 1`, smaller for higher `j` (an estimate within a factor 2; not derived further). These are far
below 1, so the components below the convergence test that F-400's amendment 1 found loosely coupled decay here by that factor
per step instead of growing. In F-400 the mock put the whole `m_a` on each face (slopes of hundreds); a real column puts it on
the mean only. This is the hypothesis that check G tests.

The measure: on the fluid's faces, `F^nu_f = F_f - (A_f/A) sum_g F_g` (all three components, `A_f` the reference areas), and
`eta_n = |F^nu|/max_m |sum_g F_g|` (Euclidean norm over the faces at the end of step `n`, over the largest total force of the run).
Matching: the wall moves uniformly up to rounding, so `eta` is rounding (estimate `<= 1e-12`). 3:2: the transposed map loads the
solid face with non-consistent weights (ADR 0412), the face deforms non-uniformly by about `0.3/(kappa_1 L_s) = 2 %` of the tip
displacement (estimate), and the fluid answers that part with `(1/kappa_1)/L_f = 1 %` of the plug's added mass: `eta` of order
`2e-4` (estimate). Growth is measured as the largest `eta` over the last period against the largest over the second period.

3:2 and Aitken: the uniform part converges by the secant, the non-uniform remainder with Aitken's single factor at a rate set by
the two slopes; no finite termination, no estimate tighter than the default cap 20 is derived. The least-squares update needs one
column per active pattern: estimate at most 6 iterations.

The frequency change between the maps: the extra static compliance of the 3:2 weights (ADR 0412 D3); measured with the piston
mock on this bar and face `-8.18e-4` at ratio 4 (ADR 0414 C-B-4). The real column adds the non-uniform added mass above, which
acts on a part of `2 %` of the motion with `1 %` of the mass: a further change of order `1e-6`. Estimate `-8.2e-4` at ratio 4;
at ratio 0.5 the change scales as in ADR 0410 (`2.38e-3/6.28e-3`): about `-3.1e-4`.

### D7. Measurements (as in ADR 0410 D4)

Signal: the mean over the interface nodes of the `x` displacement at the end of each step (`t_0 = 0` included). Zero crossings by
linear interpolation between steps; `T_m = 2 (t_last - t_first)/(N_c - 1)`, `Omega_m = 2 pi/T_m` (about 20 crossings in ten
periods).

Energy: `E_n = 1/2 v^T M v + 1/2 u^T K u` (`Integrator::kinetic_energy`, `strain_energy`). Fluid work over the window, on the
fluid's faces: `W_fl = sum_{n=1}^{1000} sum_f (d_f^{n+1} - d_f^n) . (F_f^n + F_f^{n+1})/2`, `F^n` the fluid's face forces at
`t_n` (the face forces of the step that ends at `t_n`), `d^n` the face displacements (the map of the node displacements, mean of
each face's points). Derivation of the balance: Newmark's average-acceleration step with the held force satisfies exactly
`E_{n+1} - E_n = (u_{n+1} - u_n)^T (H_{n-1} + H_n)/2` (with `v_{n+1} - v_n = (dt/2)(a_n + a_{n+1})`, `u_{n+1} - u_n = (dt/2)(v_n +
v_{n+1})` and `M a_j + K u_j = H_{j-1}` at each end; no external load after `t_1`); at convergence `H_n = B^T F^{n+1} + r_n` with
`|r_n| <= 1e-6 max(|B^T F^{n+1}|, |f_P|)`, and `B^T` conserves the work (ADR 0256). So `E_1001 - E_1 - W_fl = sum (u_{n+1} -
u_n)^T (r_{n-1} + r_n)/2`, bounded by `1e-6` times `sum |du| max(|H|, |f_P|)`, which is at most `47 E_1` by the arithmetic (norms
over the nodes give the same order): `|E_1001 - E_1 - W_fl| <= 4.7e-5 E_1`, typically far less (the converged residuals lie well
below the tolerance and change sign). The amplitude of the energy is `E_1`.

### D8. What the tests compute themselves

The root (bisection, 200 halvings), `omega`, `dt`; the discrete model of D2 on the 1D chain (Thomas algorithm; the tip mass term
on the diagonal) from the initial state of D4 over the same steps, its crossing frequency `Omega_model`, its energy ratio
`E_1001/E_1`, its tip displacement at every step; the same model with the force `-m_a a_{n+1}` (no lag, `Omega_nolag`, for the
control NC1); the static displacement `u0`. No number of the arithmetic is pasted into a test.

## Registered checks

Test file `crates/rcfd-post/tests/si_fsi_piston.rs` (the stub's name). Every value is printed; a test fails at the end if any of
its checks missed. Heavy runs (the ten-period runs, R*) are `#[ignore]` records run once with `--include-ignored`; the fast tests
cover the same code path. Runs: `RA05`, `RL05` (ratio 0.5, matching, Aitken, least squares), `RA4`, `RL4` (ratio 4, matching),
`RA4n`, `RL4n` (ratio 4, 3:2), `RA05n` (ratio 0.5, 3:2, Aitken), 1001 steps each.

### A. The acceptance

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| A0 | c0 | this file committed before any code | holds | holds |
| I1 | c2 | matching runs `RA05`, `RL05`, `RA4`, `RL4`: every one of the 1001 steps converged to `1e-6`; iterations per step at most the cap; the histogram, the largest and the mean printed | cap 10 | 3 to 4 |
| I2 | c2 | 3:2 runs: `RA4n`, `RA05n` (Aitken) | cap 20 | none derived |
| I2b | c2 | 3:2 run `RL4n` (least squares) | cap 10 | `<= 6` |
| F1 | c3 | `abs(Omega_m/omega - 1 - delta_dt - delta_visc)` (D2, D3), and `abs(Omega_m/omega - 1)`, each run of I1 | `<= 2e-2` each | `<= 2.5e-4`; `1.6e-4` (0.5), `1.1e-3` (4) |
| EN | c4 | `abs(E_1001 - E_1 - W_fl)/E_1`, each of the seven runs | `<= 1e-4` | `<= 4.7e-5` (bound), typically `<= 1e-7` |
| LC | c5 | ratio 4, matching, loose (`max_iterations = 1`, `Keep`), at most 10 steps, stopped when the signal reaches `10 U` or a step fails: the signal reaches `10 U` within 10 steps; and the geometric mean of `abs(H_{n+1}/H_n)` (total axial held force) over the steps from 2 to the stop | step `<= 10`; `>= 5` | step 3; `14.8` |
| M2 | c6 | `abs(Omega_3:2/Omega_matching - 1)`: `RA4n` against `RA4`, `RL4n` against `RL4`, `RA05n` against `RA05` | `< 2e-3` each | `8.2e-4`, `8.2e-4`, `3.1e-4` |
| FS1 | c7 | the solid fails: ratio 4, matching, Aitken, the release table ending at `3 dt`: steps 1 to 3 converge; step 4 fails with participant `solid`, iteration 1, the message naming the load table; the fluid's bits (velocity, pressure, flux, time, mesh points, boundary values, wall gradient, history length) and the solid's (time, `TransientState::to_bytes`, held forces) equal those before step 4 | all hold | holds |
| FS2 | c7 | the fluid fails: a mesh motion that fails at its call in iteration 2 of step 3 (the helper wrapped by a counter): step 3 fails with participant `fluid`, iteration 2; both sides' bits equal those before step 3; then, with the counter past, step 3 repeated converges and equals bit for bit the step 3 of an unfailed run | all hold | holds |
| G1 | c8 | matching runs: `max_n eta_n` (D6) | `<= 1e-6` | `<= 1e-12` |
| G2 | c8 | every run: `max eta` over the last period (steps 902 to 1001) over `max eta` over the second period (steps 101 to 200) | `<= 10` | `<= 1` |
| G3 | c8 | 3:2 runs: `max_n eta_n` | `<= 1e-2` | `2e-4` |

### C. Companions

| Id | Quantity | Limit | Estimate |
|---|---|---|---|
| FP | `abs(Omega_m/Omega_model - 1)`, each run of I1 | `<= 2.5e-4` | `<= 1e-5` (ADR 0252 A2: the corrected flow is the model to `1e-5`) |
| ED | `abs((E_1001/E_1)/(model's E_1001/E_1) - 1)`, each run of I1 | `<= 2e-2` | `<= 1e-3` |
| S0 | initial state: `max abs(a0)` over `omega^2 U`; the fluid's initial face forces are zero; the wall points at `U` in `x` within `1e-15 U` (matching and 3:2) | `<= 1e-9`; all zero; holds | `1e-13`; zero; holds |
| M0 | the matching map's rows are the identity: `max abs(H - I)` | `<= 1e-14` | 0 |
| MC | 3:2 runs, last step: resultant and work of the face forces against the nodal forces, and resultant of the displacement map (`sum` of each row minus 1), each relative to its scale | `<= 1e-12` | `1e-16` |
| TR | 3:2 runs: `max |q_yz|/max |q_x|` over the interface nodes and steps (record of what the mesh motion does not follow, D5) | record | `<= 1e-2` |
| QF1 | fast: ratio 4, matching, Aitken and least squares, 25 steps: every step converged within cap 10; `max_n abs(q_n - q_model,n)/U` (signal against the model's tip); `abs(E_25 - E_1 - W_fl)/E_1`; `max eta` | cap; `<= 1e-3`; `<= 1e-6`; `<= 1e-6` | 3; `<= 1e-5`; `1e-9`; `1e-12` |
| QF2 | fast: ratio 4, 3:2, Aitken and least squares, 10 steps: every step converged within the caps of I2, I2b; MC on the last step; `max eta <= 1e-2` | as stated | as stated |
| VS | fast: QF1's Aitken run (25 steps) with `nu = 1e-2` instead of `1e-4`: `max_n abs(q_n(1e-2) - q_n(1e-4))/U` | `<= 1e-6` | rounding |
| H1 | helper: on the matching channel with a non-uniform `d` (a different axial value and a transverse component at every wall point): each wall point at `p + d_x e_x` bit for bit (`p + d_x` computed as the helper does); every outlet point unchanged bit for bit; a point at `x_wall + L_f/2` moved by `d_x/2` within `1e-15` m; no point moves in `y` or `z` | all hold | holds |
| H2 | helper refusals: unknown patch, `axis = 3`, `length = 0` and `NaN`, `tolerance = 0`, a patch whose lines do not reach every point (`ymin` with axis 0), a wrong count and a non-finite displacement in `positions`; each a message naming the cause | all hold | holds |

### N. Negative controls

* LC is the registered control (c5).
* NC1: ratio 4, `RA4`: `abs(Omega_m/Omega_nolag - 1)`, the model without the lag (the Newmark-consistent piston of ADR 0410):
  must be at least `5e-4`, so that FP (`2.5e-4`) can see the lag of the pairing. Estimate `7.0e-4`.
* NC2: H1 with the transverse component only (`d_x = 0`): no point moves (the helper does not apply transverse motion).

### S. Existing tests (unedited, release)

No line of `rcfd-fv`, `rcfd-fem` or `rcfd-api` changes; the library change is the new content of `rcfd-post/src/solid_fsi.rs`
(declared, empty). S1: `cargo test -p rcfd-post --release --test si_solid_dynamic --test si_solid_dynamic_bilinear --test
si_solid_dynamic_nonmatching` (the neighbours that use `DynamicSolid` and the maps). The baseline hashes are not run (no shared
solver file changes; stated in the addendum).

### M. Planned mutations (each alone, on a `git archive` copy of the test commit in the scratch folder, after the registered run)

* MU1: the helper's weight is 1 for every point (the whole mesh translates): expected caught by H1 (outlet points).
* MU2: the helper matches lines by `y` only (ignores `z`): expected caught by H1 (non-uniform `d`).
* MU3: the helper applies `-d_x`: expected caught by H1, QF1.
* MU4: the helper uses the `y` component of the displacement as the axial one: expected caught by H1, QF1.
* MU5 (in the copy's `rcfd-fv`, to test the power of FS1, FS2): `FlowWall::restore` does not restore the geometry: expected caught
  by FS1, FS2 and QF1.
* MU6 (in the copy's `rcfd-fv`): `coupled_step` returns the error without restoring the participants: expected caught by FS1,
  FS2.

## Decision rule

* **done**: every check of A met, every companion of C met, LC and NC1 hold, S1 passes, every planned mutation caught.
* **partial**: any check of A missed (recorded as `*_known_miss`, limit intact) or not run; a check met only after an amendment
  made after its run; a companion missed with a diagnosed cause; a mutation not caught.
* **todo**: no coupled run of the flow with the solid possible.

## Limits (what is not covered)

* One geometry: a straight channel of the bar's section, a plug flow; a wall that bends (the 3:2 map's non-uniform part) is
  measured only through G3, TR and M2, with no limit on its shape. Transverse interface motion is not followed by the fluid mesh
  (D5).
* Newmark average acceleration, one sub-step per coupling step, `T/100`. No convergence study in `dt` or in the cells.
* No compressibility, no turbulence, no case key, CLI, MCP tool or service call; `rcfd_api::solid_coupled` is not written.
* The time-step estimate is the 1D model of D2, which assumes the plug; its agreement with the flow is a companion (FP), not an
  assumption of the acceptance check.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The initial state (D4): the static release with the fluid at rest (consistent) rather than a displaced state with an
   inconsistent initial acceleration or a velocity start.
2. The map of the 3:2 interface: F-504's bilinear projection on a 4 x 4 face (D4), chosen from ADR 0412/0414's numbers.
3. The channel length `L_f = 32 m` (8 `L_s`) so that the fluid is at most half as dense as the bar (D4, D6).
4. The helper moves the mesh axially only (D5).

## Amendment 1 (2026-11-14, written after the registered run of the fast tests of commit ab78f8cc and before the corrected run)

**What the registered run showed** (log `f401-scratch/run1_fast.log`, made once, `cargo test -p rcfd-post --release --test
si_fsi_piston`, 0.9 s): every fast check met except two. (a) H1, the point at half the channel length: `1.222e-15 m` against the
limit `1e-15 m`. (b) VS: `8.28e-5 U` against `1e-6 U`. The heavy records were not run yet; this amendment does not touch them.

**(a) H1 is defective by construction.** The checked quantity is `x_moved - x_ref - d_x/2` at `x_ref = L_s + L_f/2 = 20 m`; half
the spacing of the doubles near 20 is `1.78e-15 m`, so the limit `1e-15 m` lies below the resolution of the coordinate and the
check measures the rounding of `20 + 0.001`, not the weight (the other parts of H1, on the same points and on the graded
channel, met with 0). Correction (the set-up of the check only, the limit unchanged as a number): **H1c**, on the same uniform
channel, the weight of every point at half the length (`ChannelMotion::weights`) equals `1/2` within `1e-15` (dimensionless), and
each such point's moved `x` is bit for bit `x_ref + w d_x` with that weight. Estimate: weight `0.5` exactly (`1 - 16/32`). The
registered test `h1_nc2_the_channel_motion` stays as an ignored record (attribute only); `h1c_nc2_the_channel_motion` repeats
its other checks and NC2 unchanged with H1c in place of the half-length check. H1 counts as met only after this amendment.

**(b) VS is a miss, not a defect of the set-up; it stays a miss.** My D3 said the discrete flow carries the plug with no viscous
effect. ADR 0252 had found the opposite for this flow step (H2: "the non-iterated step has a viscous splitting error", H4: "the
viscous part of the force exists", up to `7.5e-6` of the force at its viscosity): in the PISO predictor the wall cell's velocity
differs from the wall's, so the viscous term acts through the predictor and the pressure, in proportion to `nu`. I had read that
addendum and did not carry it into D3: the estimate was wrong, the limit stands. The check becomes the ignored
`vs_viscosity_times_100_known_miss` (limit intact), which also prints, as an unregistered diagnosis, the same difference for
`nu = 1e-3` (if the effect is proportional to `nu`, about `7.5e-6 U`; at the registered `nu = 1e-4` it is then about `8e-7 U`
over 25 steps). The function `qf1_vs_fast_matching_run_against_the_model` keeps QF1 (met) and loses VS: it is moved, with its
limit, into the known-miss test. The viscous estimate `delta_visc = 0` of F1 is not changed; the companion FP measures whether the
frequency notices.

## Addendum 1: results (2026-11-14)

Runs, each once, release profile, `CARGO_BUILD_JOBS=4`, logs in `w49i-work/f401-scratch`: the fast tests of commit `ab78f8cc`
(`run1_fast.log`, 0.9 s: the registered run of every fast check); H1c of amendment 1 on commit `4d96e7b0` (`run2_h1c.log`);
the three heavy records on `4d96e7b0` (`cargo test -p rcfd-post --release --test si_fsi_piston -- --ignored r05_ r4_`,
`run3_heavy.log`, 49.6 s for the seven ten-period runs); the known miss VS with its diagnosis (`run4_vs.log`); then the file
as it stands, to check that it passes (`run5_file.log`: 8 passed, 5 ignored: the three heavy records, the H1 record, the VS
known miss). The heavy records take about 50 s together; they stay ignored as registered, the fast tests cover their path.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| A0 | registration first | `ad1eb41a` precedes every code commit | yes |
| I1 (matching, 1001 steps each) | converged to `1e-6`, cap 10 | every step converged in 3 iterations (mean 3.000) in `RA05`, `RL05`, `RA4`, `RL4` | yes |
| I2 (`RA4n`, `RA05n`, Aitken) | cap 20 | all converged; largest 12, mean 7.81 (ratio 4); largest 10, mean 6.43 (ratio 0.5) | yes |
| I2b (`RL4n`, least squares) | cap 10 | all converged; largest 7, mean 5.32 (4 iterations: 2 steps, 5: 681, 6: 317, 7: 1); the estimate `<= 6` exceeded once, the cap met | yes |
| F1 after the estimate (`delta_dt` `+1.5655e-4`, `-1.0553e-3`; `delta_visc = 0`) | `<= 2e-2` | `2.61e-7` (ratio 0.5, both methods), `4.08e-7` (ratio 4, both) | yes |
| F1 raw `abs(Omega_m/omega - 1)` | `<= 2e-2` | `1.563e-4` (0.5), `1.0557e-3` (4) | yes |
| EN (ten periods) | `<= 1e-4` | matching `7.1e-10`, `8.6e-10` (0.5, Aitken, LS), `2.14e-8`, `2.21e-8` (4); 3:2 `4.89e-6` (`RA4n`), `7.06e-7` (`RL4n`), `3.65e-6` (`RA05n`) | yes |
| LC (loose, ratio 4) | `10 U` by step 10; growth `>= 5` | step 3 (signal `-10.5 U`); `13.83` per step | yes |
| M2 | `< 2e-3` | `8.164e-4` (`RA4n`/`RA4`), `8.164e-4` (`RL4n`/`RL4`), `3.111e-4` (`RA05n`/`RA05`); the 3:2 frequency is the lower | yes |
| FS1 (solid fails) | located; both restored | `solid`, iteration 1, "key 'load table': table 0 covers [0e0, 3.111e-4], the run needs [3.111e-4, 4.149e-4]"; fluid and solid bit-equal | yes |
| FS2 (fluid fails) | located; both restored; the repeated step equals the unfailed run | `fluid`, iteration 2, the injected message; both bit-equal; the repeated step 3 bit-equal to the unfailed run's (step record, fluid and solid) | yes |
| G1 (matching) | `<= 1e-6` | `2.56e-10` (0.5), `8.30e-10` (4) | yes |
| G2 (last period over the second) | `<= 10` | matching `0.392` (0.5), `0.233` (4); 3:2 `0.438` (4, both methods), `0.580` (0.5) | yes |
| G3 (3:2) | `<= 1e-2` | `8.77e-4` (4), `5.89e-4` (0.5) | yes |
| FP | `<= 2.5e-4` | `2.61e-7` (0.5), `4.08e-7` (4) | yes |
| ED | `<= 2e-2` | `2.15e-5` (0.5; `E_1001/E_1 = 0.10722`), `8.52e-5` (4; `0.026602`) | yes |
| S0 | `<= 1e-9`; zero; `<= 1e-15`; bits | `6.0e-12`; zero; `0` (matching), `2.2e-16` (3:2); bit for bit | yes |
| M0 | `<= 1e-14` | `0` | yes |
| MC (3:2, last step; row sums) | `<= 1e-12` | resultant `<= 5.8e-16`, work `<= 1.8e-16`; row sums `2.2e-16` | yes |
| TR (record) | record | `max abs(q_yz)/max abs(q_x)` `9.25e-3` (4), `8.26e-3` (0.5) | record |
| QF1 | cap 10; `1e-3`; `1e-6`; `1e-6` | 3 iterations every step; `1.74e-5 U`; `2.05e-8`, `2.09e-8`; `8.3e-10` | yes |
| QF2 | caps 20, 10; MC; `1e-2` | Aitken largest 10 (mean 8.1), least squares largest 7 (5.9); `<= 3.9e-16`; `8.77e-4` | yes |
| **VS** | `<= 1e-6 U` | **`8.28e-5 U`** (`vs_viscosity_times_100_known_miss`); diagnosis: `7.53e-6 U` for `nu = 1e-3`: proportional to the change of `nu` (`8.28e-5/7.53e-6 = 11.0` for a change 11 times larger) | **no** |
| H1 as registered | `<= 1e-15 m`, bits | wall, outlet, weight (`0`), transverse: met; the half-length point `1.22e-15 m` (below the coordinate's resolution, amendment 1) | **no** (record `h1_nc2_the_channel_motion`) |
| H1c (amendment 1) | `<= 1e-15`; bits | weight `0` off `1/2` at 9 points; moved `x` bit for bit | yes, after the amendment |
| H2 | refusals | all eight refused with a message naming the cause | yes |
| NC1 | `>= 5e-4` | `7.01e-4` | holds |
| NC2 | no point moves | holds | holds |

What the numbers say:

* **The flow is the discrete model of the pairing.** The measured frequency equals the model of D2 (Newmark bar with the force
  `-m_a (u_{n+1} - 2 u_n + u_{n-1})/dt^2`) to `2.6e-7` and `4.1e-7`, and its energy decay to `2e-5` and `9e-5` of itself; the
  model without the lag is `7.0e-4` away (NC1). So the whole departure from the root (`1.6e-4`, `1.1e-3`) is the time
  discretisation and the lag of one step of the flow pairing, and the plug's added mass with the corrected wall pressure is
  exact to the precision of these runs (ADR 0252's conclusion, here with a structural solver).
* **The added-mass instability of F-400's per-face mock does not occur with the real column.** The non-uniform part of the force
  stays at `1e-9` of the force on the matching wall and decays over the run on the 3:2 wall (G2 below 0.6), as D6 predicted:
  the local flow's added mass is small against the bar's local mass. The roadmap note's check is G1 to G3.
* **The 3:2 wall costs iterations, not accuracy.** Aitken needs 6 to 12 iterations per step on it, the least-squares update 4 to
  7, against 3 on the matching wall; the frequency change `8.16e-4` at ratio 4 is ADR 0414's `8.18e-4` of the piston mock on
  the same face and map to `2e-6`, so the real column adds nothing measurable to it.
* **The fluid takes the energy**: the solid keeps `0.107` (ratio 0.5) and `0.027` (ratio 4) of its energy over ten periods; the
  fluid's work on the interface accounts for the loss to `5e-6` of the amplitude at worst (3:2, Aitken).

### Diagnosis of VS

D3 claimed that the discrete flow carries the plug with no viscous effect. The difference of the tip motion between `nu = 1e-2`
and `1e-4` over 25 steps is `8.28e-5 U`, and between `1e-3` and `1e-4` it is `7.53e-6 U`: proportional to `nu`, as a viscous
term must be. Its cause is the one ADR 0252 found (H2, H4) for this step: the PISO predictor's wall cell does not move with the
wall before the pressure correction, so the viscous term acts through the predicted velocity and the pressure, and the
non-iterated step keeps a part of it. At the registered `nu = 1e-4` the same proportion gives about `8e-7 U` over 25 steps, which
is why FP (`4e-7` in frequency) does not see it. The registered viscous estimate `delta_visc = 0` is therefore wrong in kind but
below the resolution of every frequency check made here.

### Mutations (copy of commit `4d96e7b0` from `git archive` in `f401-scratch/copy`, own target directory, `mutate.py`, logs `MU1.log` to `MU6.log`)

| Mutation | Caught by |
|---|---|
| MU1 the weight 1 everywhere | H1c (outlet, weight, half length), as expected |
| MU2 lines matched by `y` only | H1c (wall points, weight), as expected |
| MU3 `-d_x` | H1c, QF1 (signal `3.0 U` off the model, energy), S0 (wall position), as expected |
| MU4 the `y` component as the axial one | H1c, NC2, QF1, QF2, LC, FS2, as expected |
| MU5 `FlowWall::restore` leaves the geometry (in the copy's `rcfd-fv`) | FS1, FS2, QF1, QF2 (no step converges), as expected |
| MU6 `coupled_step` returns the error without restoring (in the copy's `rcfd-fv`) | FS2 (the solid had advanced in iteration 2), as expected; FS1 does not see it: in iteration 1 the solid refuses before changing anything and the fluid has not been advanced (equivalent there) |

Every mutation was caught.

### Existing tests, formatting, lints

* S1: `cargo test -p rcfd-post --release --test si_solid_dynamic --test si_solid_dynamic_bilinear --test
  si_solid_dynamic_nonmatching`: 13 passed (5 ignored), 3, 3.
* `cargo fmt -p rcfd-post --check`: clean. `cargo clippy -p rcfd-post --all-targets --release -- -D warnings`: clean.
* The baseline hashes were not run: no line of `rcfd-fv`, `rcfd-fem`, `rcfd-api` or any other shared solver file changed (the
  library change is the new content of `rcfd-post/src/solid_fsi.rs`, a module declared empty by ADR 0291 and called by nothing
  in the library).

### Deviations

* Amendment 1 after the first run: H1's half-length part replaced by H1c (defective limit); VS moved into a known-miss test.
* S0's "the wall points at `U` in `x` within `1e-15 U`" was implemented as two parts, decided while writing the test and before
  its run: the map of the initial interface displacement equals `U e_x` within `1e-15 U` at every wall point, and each fluid wall
  point lies bit for bit at its reference plus that mapped displacement (the coordinate `4 + 0.001` cannot hold `U` to
  `1e-15 U`, the reason of amendment 1 (a)).
* The fast tests run in 0.9 s and the heavy records in 50 s, far below what I had expected; the heavy records stay ignored as
  registered.
* `docs/cosimulation-protocol.md`: the limits section says that a structural solver has been connected, with the numbers above.

### What was not run

* The whole workspace suite (the lead's gate); `cosim_f139`, `cosim_f361`, `cosim_f362` (no line of `rcfd-fv` changed).
* A study in `dt`, in the cells or in the channel length; a bending wall; the radial-basis map in this coupled run.

### Decision by the registered rule

**Partial.** Every clause of the acceptance is met (A: I1, I2, I2b, F1, EN, LC, M2, FS1, FS2, G1 to G3), the controls hold (LC,
NC1, NC2) and every mutation was caught; but the companion VS missed (diagnosed: the viscous splitting of the PISO step,
proportional to `nu`), and the helper's check H1 was met only after amendment 1 (a defective limit below the resolution of the
coordinate). By the rule either makes the item partial.

### Remaining questions

1. **The viscous estimate.** Options: (a) register, in a later item, `delta_visc` from the measured proportional splitting (a
   short run at two viscosities, as the VS diagnosis) and check that the frequency follows it; (b) remove the splitting with
   outer PIMPLE iterations in `FlowWall` (ADR 0252 K4c, K5c saw no frequency change from them); (c) leave it: its effect on the
   frequency is below `1e-6` at the viscosity of this case. Recommendation: (c), with the sentence of the note; (a) when a
   viscous (no-slip) wall is coupled.
2. **Transverse interface motion** (TR: `9e-3` of the axial motion on the 3:2 wall) is not followed by `ChannelMotion`. Options:
   (a) use the general smoothing with slip classes as the motion for walls that bend; (b) a transverse motion tangential to the
   side walls in the helper. Recommendation: (a), when a bending wall is coupled (the advanced tier); nothing now.
3. **Aitken on non-matching interfaces** takes about twice the iterations of the least-squares update (6 to 12 against 4 to 7).
   Options: (a) recommend the least-squares update for non-matching walls in `docs/cosimulation-protocol.md`; (b) nothing.
   Recommendation: (a); written as a sentence in that file's limits section by this item.

### For the lead: roadmap entries

F-401, proposed status **partial**, note:

> Structural solver, intermediate tier (plan ADR 0290). PARTIAL (ADR 0434, 2026-11-12; registered before the code, amendment 1 after the first run). The elastic bar of F-400 (DynamicSolid, Hex8 20 x 4 x 4, nu = 0, clamped, L_s = 4 m) closes a channel of incompressible fluid (FlowWall, moving-mesh PISO with the prescribed wall gradient of F-351, slip side walls, open end, L_f = 32 m), coupled by coupled_step at mass ratios rho_f L_f/(rho_s L_s) of 0.5 and 4 from the static release of a tip traction (solid and fluid start consistent, at rest); rcfd_post::solid_fsi::ChannelMotion moves the channel mesh with the axial interface displacement. Over 1001 steps at T/100: Aitken and the least-squares update converge to 1e-6 in 3 iterations every step on the matching wall and in 4 to 12 on a 3:2 wall (bilinear projection of F-504 on a 4 x 4 face); the first coupled frequency is the root of k L tan(k L) = rho_s L_s/(rho_f L_f) to 1.6e-4 (0.5) and 1.1e-3 (4), and the discrete model of the pairing (the Newmark bar with the plug force of the flow step, one step of lag: the registered time-step estimate) to 4e-7 (limit 2 percent after the estimate); over ten periods the fluid's work on the interface equals the change of the solid's energy to 5e-6 of its amplitude (limit 1e-4); the loose coupling at ratio 4 grows by 13.8 per step; the 3:2 wall changes the frequency by 8.2e-4 (4) and 3.1e-4 (0.5) (limit 2e-3; ADR 0414's piston mock gave 8.18e-4); a failing solid (load table) or fluid (mesh motion) stops the step naming it, both sides bit-equal to the start of the step. The non-uniform part of the interface force (ADR 0410 Q3) stays at 1e-9 of the force on the matching wall and decays over the run on the 3:2 wall (at most 8.8e-4): with a real column its slopes are far below 1, so the growth seen with F-400's per-face mock does not occur. Partial: the registered viscous estimate 0 is wrong in kind (the PISO step's viscous splitting moves the tip by 8.3e-5 U at nu x 100, proportional to nu; a known miss, below 1e-6 in frequency at the case's viscosity), and the helper check H1 was met only after amendment 1 (a limit below the coordinate's resolution). Six mutations caught. No line of rcfd-fv or rcfd-api changed. Open: ADR 0434 Q1 to Q3.

F-400, sentence to append to its note:

> The added-mass growth of the per-face mock (amendment 1) does not occur with the flow solver: in F-401 (ADR 0434, 2026-11-12) the non-uniform part of the interface force stays at rounding on the matching wall and decays on the 3:2 wall, its slopes being those of the local flow, far below 1.

No follow-up item is proposed (the questions are recorded for the advanced tier).
