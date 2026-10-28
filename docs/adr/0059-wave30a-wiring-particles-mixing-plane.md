# ADR 0059: Wave 30A, wiring of the two-way particle coupling (F-128) and of the mixing plane (F-126) into the solver loops

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave was changed and
before any run of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and
the assertion moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A
test set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended
at the end.

Clean-room: first principles and published literature only. No vendor code, text or installs, nothing from
vendor material. Waves 30B/30C (validation) work in the same tree and mostly add tests and docs; this wave touches
`rcfd-api/src/sim.rs`, `schema.rs`, `particles.rs`, `particle_run.rs`, new modules `particle_coupling.rs` and `mixing_run.rs`,
`rcfd-mesh/src/gen.rs` (one new generator, nothing existing is edited), small additive hunks in `rcfd-particles` and docs.
`rcfd-fv/src/lib.rs` is not touched (its working copy shows a stale module order that is not ours). Every new behaviour is behind a
key whose default is inert and every inert path is the old code path (no `Some(zero)`), so `baseline_hashes` and all existing tests
are unchanged by construction; this is re-verified, never re-recorded.

## Audit (HEAD 879715c)

* F-128 (ADR 0057): `Tracker::enable_coupling` records, per cell, the momentum given to the carrier (`Coupling::impulse`, already
  with the sign "to the carrier", N s) and the mechanical and heat parts of the total energy given to it. `Coupling::sources(vol, dt)`
  divides by `V dt`. The tracker never reads the sources. `Simulation::step` does not apply them. The tracker is driven by
  `ParticleRun::after_step` at the END of a transient `step` (flow first, particles on the new field) and, for steady algorithms, once
  on the final field when the results are written (`steady_run`). Heat transfer (`Thermal`) and the injection temperature exist in the
  library only (no case keys). `particles::check` requires constant-property incompressible solvers.
* Momentum-source hook (ADR 0045): the steady branch of `Simulation::step` assembles `s.body_force` from `base_body`, Boussinesq,
  the user-function force and `sources` (a flat list of `Option<BodyForce>`); the transient branch passes an `OuterUpdate` through
  `PimpleSolver::step_with`, and the hook REPLACES `source_force` at every outer iteration. `BodyForce::from_cells` gives the face
  consistent acceleration used in the four places (cell RHS, face flux, fixed-flux wall pressure, cell velocity correction).
* F-126 (ADR 0055): `MixingPlane::{average, pass}` and `FrozenRotorInterface` are libraries. The solver's frozen rotor is the
  absolute-velocity MRF zone (ADR 0008); `annulus_3d_interface` JOINS the blocks into internal faces, so no mesh generator makes the
  two unjoined boundary patches a mixing plane needs. `merge_meshes` stitches coincident faces (it would join them as well).
  `catalogue.refresh(geom, u, p, phi)` is the existing per-iteration boundary update (kind `bc_kind`), called before `s.iterate()`.
  A fixed-velocity boundary face has the flux `u_b . S` exactly (`corrected_flux` with `ub`); a fixed-pressure outlet with zero
  gradient velocity has the corrected flux, which differs from `u_P . S` at truncation level.
  `radial_equilibrium_outlet` gives a pressure profile `dp/dr = w^2/r` from the outlet tangential velocity (trapezoid over the faces).
* R4 of ADR 0055 recorded that the normalised residuals stall near 4e-2 on exact (uniform) solutions, so `is_converged` is not used as a
  criterion on exact-solution tests below; the exchanged state and the fields are.

## Decisions: F-128 two-way coupling

D1 Sign, units. `Coupling::impulse_c` [N s] is the momentum GIVEN to the carrier in cell `c`. The carrier equation is the kinematic
momentum equation of the constant-density solver, so the added acceleration is `a_c = impulse_c / (rho_f V_c T)` [m/s^2] with `T` the
normalisation time below, and it enters through `BodyForce::from_cells(geom, a)` (face-consistent, ADR 0045). Positive `a` along a
velocity direction accelerates the carrier. A particle slower than the fluid gains momentum, the carrier gets a negative impulse
(decelerates), total momentum `sum m v + sum rho_f V u` is conserved except for wall reflections (booked in `wall_impulse`) and gravity.

D2 Case keys (all additive, defaults inert):

| Key | Default | Meaning |
|---|---|---|
| `particles.two_way` | false | feed the particle exchange back into the carrier equations |
| `particles.two_way_relax` | 0.5 | steady: `S <- S + beta (S_new - S)`, `0 < beta <= 1` |
| `particles.two_way_every` | 1 | steady: re-track every this many SIMPLE iterations (>= 1) |
| `particles.two_way_start` | 0 | steady: first iteration (counted from 0) at which the coupling is updated (>= 0) |
| `particles.two_way_tolerance` | 1e-6 | steady: the run is converged only if the coupling change is below this (> 0) |
| `particles.heat_transfer` | false | Ranz-Marshall lumped heat transfer of the parcels, needs `energy.enabled` and the carrier `physics.cp`, `physics.conductivity` |
| `particles.particle_cp` | -1 (unset) | particle specific heat [J/(kg K)] (> 0 when `heat_transfer`) |
| `particles.injection.<n>.temperature` | -1 (unset) | parcel temperature at injection [K] (> 0 when `heat_transfer`) |

D3 Transient (piso, pimple): "particles first, same interval". Inside the transient branch, after the step size `dt` is known and
before the flow step, the tracker is advanced by `h = min(dt, end - t_tracker)` on the velocity field `u^n` (the field at the start of the
step) with the exchange record reset; the record of this advance is converted with `T = h` (the tracked interval, equal to the flow
`dt` while the tracker runs) and handed to the flow step of the SAME interval through the hook (`OuterUpdate::source_force`). The source
is therefore constant over the outer iterations of the step; the particles see `u^n`, the carrier sees the impulse they exchanged in
the step (first order, explicit, "loose" coupling). Consequences, stated: (a) the sum of the delivered `rho V a dt` equals the recorded
impulse of every step exactly (no lagged last record); (b) the one-way `after_step` hook is not run in two-way mode (it would advance the
tracker a second time); (c) stability of the explicit exchange needs `dt` below about `2 tau_p/(1+phi_cell)`, with `phi_cell` the local
particle-to-carrier mass ratio (the particle update itself is exact for frozen data and unconditionally stable); (d) no under-relaxation
and no sub-iteration of the exchange within a step (a documented limit); (e) with `sources`/`components` present the particle
acceleration is ADDED to `up.source_force` inside the same hook (the hook replaces the field every outer iteration); (f) adaptive step
control that rejects steps (`time_control` error control) and `dynamic_mesh` are located errors, CFL and planned step control are fine
(`dt` is known before the step). If no parcel exchanged momentum in the step (no parcels, or none active) the hook receives NO particle
force (`None`, not a zero field), which keeps the zero-parcel run bit-identical to the existing path.

D4 Steady (simple, simplec): a fixed-point coupling iteration (particle-source-in-cell type, Crowe, Sharma and Stock 1977; stated
from first principles below). The steady carrier sees the force density of a statistically steady particle stream. A set of
`N_k` parcels per injector is a sample of the stream; parcel `i` injected at rate `R` [parcels/s] stands for a stream, so the
momentum per unit time given to the carrier in cell `c` is the accumulated impulse of the whole sample divided by the injection window
`W = N/R` (`N` parcels, all injectors the same `R` and window; checked, located error otherwise). Every `two_way_every` iterations,
from `two_way_start`, at the top of `Simulation::step` (before the engine match, because `build_tracker` needs `&Simulation`):
build a FRESH tracker (identical parcels every time: the sources are then a deterministic function of the velocity field, which is
what makes a fixed point exist; a reused tracker would advance its generator), `enable_coupling()`, push the carrier temperature when
`heat_transfer`, track from 0 to `particles.end_time` with `particles.dt` (or `solver.dt`) on the current velocity, take
`a_new = impulse/(rho_f V W)`, relax `a <- a + beta (a_new - a)` starting from `a = 0` (so the first update is `beta a_new`), and hold
`a` constant until the next update. Precondition, checked: no parcel is still `Active` when the tracking ends (otherwise the source is
biased low): that is a failure with a message located at `particles.end_time`. The coupling change is
`c = sum_cells |a_new - a_old| V / max(sum_cells |a_new| V, 1e-300)` (1 at the first update); it is a monitor `res_particles`. The run
counts as converged only when additionally `updates >= 2` and `c < two_way_tolerance` (so the key is a convergence criterion, not a
stop that hides a drifting coupling). The converged source is a fixed point of "track on the field, solve on the source".
If `N = 0` there is no source (`None`).

D5 Where the force enters. Steady: one more `Option<BodyForce>` in the list summed into `s.body_force`, and the branch condition gains
`|| tw_force.is_some()`; it is `None` unless a source exists. `base_body`, Boussinesq, user-function force and `sources` are unchanged. Transient: D3.

D6 Energy. The temperature equation of the constant-property solver is `rho c_p` divided; its `source` is in K/s. The heat given by the
parcels (`energy_heat`, `-m c_p Delta T_p`) is the only part fed in: `q_c = energy_heat_c/(rho_f c_pf V_c T)` [K/s]. The mechanical part of the
total-energy source (`energy_mech`) is NOT fed in: it is the kinetic energy and drag work exchange that, in the incompressible
constant-property model, is carried by the momentum equation, and the dissipated part `F.(u - v)` is neglected exactly as viscous heating
is neglected in this energy equation (stated approximation; feeding `Coupling::sources().1` would double count). Steady: merged AFTER the
zone, component and chemistry sources of the iteration, the way `chem_heat` is merged (added to whatever source the iteration built, creating one
if there is none); transient: the same just before `EnergySolver::advance`. The carrier temperature seen by the parcels is pushed into
`Thermal::t_fluid` (cell values) before each tracking (steady: each update; transient: each step). `heat_transfer` without
`two_way` heats the parcels from the carrier only (no feedback).

D7 Located errors (`particles::check` and `Simulation::new`): at `particles.two_way` when `particles.enabled` is false, for the
`coupled`, VOF, compressible and variable-density engines, `solver.frozen_flow`, `dynamic_mesh.enabled`, fractional-step time advance
and error-controlled adaptive steps; at `particles.two_way_relax` (outside (0, 1]), `particles.two_way_every` (< 1),
`particles.two_way_start` (< 0), `particles.two_way_tolerance` (<= 0); steady and unequal injection `rate`, `start_time` or effective
`end_time` at `particles.injection.<n>.rate` / `.end_time`; at `particles.heat_transfer` without `energy.enabled`; `particles.particle_cp`
and `particles.injection.<n>.temperature` unset when `heat_transfer`; `physics.cp`/`physics.conductivity` not positive.

D8 Outputs. `ParticleRun` records `delivered` (sum of `rho V a h` handed to the carrier per step) and `exchanged` (sum of the records) and
the steady state records the last coupled tracker, the used and raw accelerations, the coupling change and the update count, available
through accessors (`Simulation::particle_run`, `Simulation::two_way`). The steady results writer uses the last coupled tracker when
two-way is on instead of a second one-way tracking.

## Decisions: F-126 mixing plane in the steady loop

D9 Mesh. A new generator `annulus_3d_stage`: two unjoined annulus blocks about z (as `annulus_3d_interface`, heights `lz/2`, upper block
`nx_right x ny_right x nz_right` cells, 0 = same as the lower), ONE mesh with patches `inner`, `outer` (both blocks), `zmin` (inlet of
the lower block), `plane_out` (lower `zmax`), `plane_in` (upper `zmin`), `zmax` (outlet of the upper block). The two blocks share no
face and no point (the plane is a pair of boundary patches). Added in `rcfd-mesh/src/gen.rs` without editing any existing function.

D10 Exchange (per iteration, before `s.iterate()`, from the second iteration on; cheap enough to run every `every` iterations):

* Forward, upstream outlet -> downstream inlet. Donor faces: the `upstream_patch` faces, area vectors as the mesh gives them (outward =
  flow direction), absolute velocity = the boundary-face velocity with its NORMAL component replaced so that `u . S` equals the solver
  face flux `phi_f` (so that the donor volume flux `Q_b` is the solver's flux and not the truncation-level different `u_P . S`; the
  tangential velocity is untouched). `MixingPlane::average` then `pass` onto the `downstream_patch` faces (area vectors flipped to the
  flow direction). The passed absolute velocity vector (`u_n`, `u_theta = Gamma_b/r`, `u_r`) is relaxed per face:
  `u_bc <- u_bc + beta (u_pass - u_bc)` and installed as a per-face `FixedValue` on the downstream patch (the case's own
  `velocity_value` is the starting state, so a coarse initial guess is the user's).
* Backward, downstream inlet -> upstream outlet. Donor = the downstream inlet faces (flow-oriented areas, boundary velocity, face
  pressure with the velocity-boundary logic), receiver = the upstream outlet faces; only the passed pressure is used: the band
  mixed-out pressure `p_b = (F_b - Q_b u_n)/A'_b`, which for the uniform-per-band velocity of the forward pass is the band
  area-mean of the face pressures. It is relaxed per face and installed as a per-face fixed pressure on the upstream patch. The
  forward pass does NOT also impose the mixed-out pressure (the downstream inlet would be over-constrained: velocity and pressure).
* Bands: `mixing_plane.bands` equal-width annuli between `r_min` and `r_max` (unset `-1` = the extremes of the vertex radii of the upstream
  patch), about `axis`, `origin`. The band edges must line up with the radial face layers on both sides (ADR 0055 addendum 4: with
  centre-based assignment a uniform flow passes scaled by `A_b/A'_b` otherwise); the checks use equal radial layers.
* Convergence measure of the exchange: `c_x = max(max|du|/max|u_new|, max|dp|/max(|u_new|^2))` over the receiving faces of the last
  exchange (`du`, `dp` the change of the relaxed state); the run is converged only if `c_x < mixing_plane.tolerance` as well. Exposed as the monitor `res_mixing`.
* Flux cancellation or a face outside all bands after the first iteration is a FAILURE with the library's located message prefixed with
  `mixing_plane:` (no silent fallback).

D11 Case keys (new top-level section `mixing_plane`, additive, default inert):

| Key | Default | Meaning |
|---|---|---|
| `enabled` | false | switch |
| `upstream_patch` | "" | boundary patch of the upstream (rotor) zone; its pressure is set by the plane, velocity must be `zero_gradient` |
| `downstream_patch` | "" | boundary patch of the downstream (stator) zone; its velocity is set by the plane, `velocity = "fixed"` |
| `axis`, `origin` | [0,0,1], [0,0,0] | rotation axis and a point on it |
| `bands` | -1 (unset) | number of radial bands (>= 1) |
| `r_min`, `r_max` | -1 (unset) | band range; unset = vertex radii extremes of the upstream patch |
| `relax` | 0.5 | exchange under-relaxation `0 < relax <= 1` |
| `every` | 1 | exchange every this many iterations (>= 1) |
| `tolerance` | 1e-6 | exchange convergence criterion |

Located errors (`Case::semantic_errors` and `Simulation::new`): unknown patch (`mixing_plane.upstream_patch` / `.downstream_patch`),
same patch twice, an empty patch name when enabled, a patch that has a `bc_kind` (path `boundaries.<patch>.bc_kind`), upstream velocity
not `zero_gradient`/upstream pressure not `fixed` (`boundaries.<patch>.velocity` / `.pressure`), downstream velocity not `fixed`,
`bands` unset or < 1, `r_min >= r_max` when given, zero axis, `relax` outside (0, 1], `every` < 1, `tolerance` <= 0, any algorithm
other than `simple`/`simplec` (path `mixing_plane.enabled`), `solver.frozen_flow`, patch faces whose areas do not point to the stated
sides (an inlet that is not an inlet), runtime failure of the plane (path `mixing_plane`).

D12 Not attempted (stated now): transient and coupled solvers, compressible mixed-out state, rothalpy, area-fraction splitting of faces
across band edges, curved interfaces, sector periodicity of a stage, the downstream-velocity constraint, flow-regime options,
non-constant density, passing turbulence quantities or scalars through the plane (only the mean flow), a published turbomachine
(Rotor 37, Hannover compressor: NOT ATTEMPTED).

## Registered checks (numeric limits fixed before any run)

Common. Release builds; `RUST_TEST_THREADS=3`. "Flow" tests have no symmetry side walls (no-slip walls moving with the exact
flow). Test files: `crates/rcfd-api/tests/particles_two_way.rs` (F-128), `crates/rcfd-api/tests/mixing_plane_run.rs` (F-126).

### F-128

Set-ups. PLUG: `box_2d`, `lx = 2`, `ly = 0.5`, `depth = 0.1`, `nx = 80`, `ny = 1`, `rho = 1`, `nu = 1e-3`; `xmin` and the walls `ymin`, `ymax`
fixed `(1, 0, 0)` (walls moving with the stream: no shear), `xmax` zero-gradient velocity with fixed `p = 0`; `initial.velocity = [1,0,0]`;
`simple`, `linear_upwind`. The unloaded carrier is the exact plug flow `u = (1, 0, 0)`. Particles: Stokes drag, `integration = analytic`,
no gravity, `density = 1000`, `diameter = 3e-3` (so `tau_p = rho_p d^2/(18 mu) = 0.5 s` exactly), surface injection on `xmin`,
`velocity = [0,0,0]`, `rate = 2000`, injection window `[0, 0.5]` (`N = 1000` parcels), `mass_flow = 0.02 kg/s`; steady tracking
`end_time = 6`, `dt = 0.02`. Closed form: a parcel released at rest in a uniform stream `U = 1` has `v = U (1 - e^{-t/tau})`,
`x = U (t - tau (1 - e^{-t/tau}))`; it leaves at `x = L = 2` at `t_L` (root of `x(t_L) = L`) with `v_L = U (1 - e^{-t_L/tau})`. In the steady
stream the force on the carrier is `F_s = mdot_p v_L` along `-x`; the carrier continuity forces `u = U` (it cannot change), so the force
is balanced by pressure alone: `(p_in - p_out) A = mdot_p v_L` with `A` the inlet area (`p` kinematic, `rho = 1`).
JET: as PLUG but `ly = 0.5`, `ny = 21` (odd, the centreline is a cell-centre row), `nx = 80`, `nu = 0.01`, `depth = 0.1`; one point injection at
`(0.1, 0.25, 0.05)` (inside cell column 4: no source cell touches the inlet or a wall), `velocity = [3,0,0]` (three times the stream),
`distribution = uniform`, `d_min = 4e-3`, `d_max = 8e-3` (equal-mass parcels, `tau_p` 0.089 to 0.356 s), `rate = 400`, window `[0, 1]`, `mass_flow = 0.01 kg/s`,
steady tracking `end_time = 6`, `dt = 0.02`; coupling `two_way_start = 20`, `two_way_every = 10`, `two_way_relax = 0.5`, `two_way_tolerance = 1e-6`;
`simplec`, `linear_upwind`, `tolerance_u = tolerance_p = 1e-9`, at most 4000 iterations.

* PL1 (PLUG, steady, wiring + closed form). `|F_s - mdot_p v_L| <= 1e-6 mdot_p v_L` with `F_s = rho sum_c V_c a_c` of the used acceleration
  after convergence (no wall hit; exits at `xmax`).
* PL2 (PLUG, steady). `|(p_in - p_out) A - mdot_p v_L| <= 1e-3 mdot_p v_L` with the area-weighted boundary face pressures (`pressure_face_values`).
* PL3 (PLUG, steady). The carrier stays the plug: `max |u_x - 1| <= 1e-6`, `max |u_y| <= 1e-6`, `max |u_z| <= 1e-6` (this tests that the
  face-consistent force and the pressure balance exactly).
* PL4 (PLUG, steady, coupling iteration). With `two_way_every = 1`, `two_way_start = 0`, `two_way_relax = 0.5` the coupling change falls below 1e-8
  within 40 updates (the iteration is `a_k = (1 - (1-beta)^k) a`, change `(1-beta)^(k-1)`), and `updates` is reported; precondition 0 active parcels at the end of tracking holds.
* PL5 (PLUG, steady). Particle side independence: the exit velocity of the parcels of the last coupled tracker equals the one-way tracking
  (`track_frozen` on the final field) to `1e-6` relative (the carrier is unchanged; observation-only).
* PL6 (PLUG, transient `piso`, `dt = 0.01`, 400 steps to `t = 4`, injection window `[0, 4]`, `rate = 500`, `mass_flow = 0.02`, particles `end_time = 4`).
  Per-step delivered impulse equals the recorded exchange: `|sum_steps delivered - (-sum_steps exchanged)|` ... precisely `|delivered + exchanged| <= 1e-12 sum|exchanged|`
  summed over the vector components; and `|delivered + sum_parcels m v| <= 1e-12 sum |m v|` (parcels start at rest, no gravity, no wall hit, so the particle momentum
  gained equals minus what the carrier received).
* PL7 (PLUG, transient `piso`, same run). At `t = 4` (the stream fills the duct since `t_L < 4`): `|(p_in - p_out) A - mdot_p v_L| <= 1e-2 mdot_p v_L`;
  `max |u_x - 1|, max |u_y| <= 1e-6` at every 40th step. PL7b: the same with `pimple`, `n_outer = 3`.
* PL8 (unloaded, bit identity). A run with `two_way = true` and ZERO parcels (`rate = 1`, window `[0, 0.5]`, so `floor(0.5) = 0` parcels) gives the exact same bits
  (`ux, uy, p`, face flux) as (a) `two_way = false` with the same particles section and (b) `particles.enabled = false`, for `simple` (JET geometry, 300 iterations),
  `piso` (PLUG, 50 steps) and with `energy.enabled` and `heat_transfer` (steady). The baseline hashes are unchanged.
* J1 (JET, steady, coupling iteration). Within 4000 iterations and 800 coupling updates the coupling change is `<= 1e-6` and the last `res_u`, `res_p` are `<= 1e-6`;
  precondition 0 active parcels holds; counts and residuals reported. (The residual-normalisation stall of R4 does not apply to this non-uniform flow; if it stalls the test
  reports it and misses.)
* J2 (JET, steady). Delivered source equals minus the particle momentum change: raw record vs the independent particle sum
  `sum_i m_i (v_end,i - v_0,i)` (injection velocity `(3,0,0)`, no wall hit): `|sum_c rho V_c a_raw,c W + sum_i m_i Delta v_i| <= 1e-12 sum |m_i Delta v_i|`;
  used vs raw: `sum |a_used - a_raw| V <= 1e-6 sum |a_raw| V`.
* J3 (JET, steady, carrier momentum balance). In the converged solution the x-momentum balance of the whole duct
  `sum_out (phi u_x) - sum_in (phi u_x) + sum_faces (p_f S_x) - sum_faces nu |S| (u_x,f - u_x,P)/d_n  =  F_s,x` (kinematic units, `F_s,x = sum_c V_c a_x,c`, boundary faces only,
  `d_n` the cell-centre-to-face normal distance, orthogonal mesh) has a residual `<= 2e-2 |F_s,x|`; the y balance residual `<= 1e-6 |F_s,x|`; `|F_s,y| <= 1e-9 |F_s,x|`.
  Stated: because the inlet velocity, the outlet pressure and the moving walls are prescribed, the streamwise force is mostly balanced by the pressure term; the carrier
  momentum-flux increase alone is NOT equal to the impulse, the full balance is. Both sides and every term are printed.
* J4 (JET, dilute limit). Mass loading `f` times the base `mass_flow` (`f = 0.01, 0.1`) with the same coupling: the deviation `D(f) = max |u_x - 1|` satisfies
  `D(0.1)/D(0.01) in [9, 11]` (linear response of the carrier to the force) and `D(f)` decreases monotonically with `f`.
* J5 (JET, independence of the relaxation). `two_way_relax = 0.5` and `0.25` converge to the same fixed point: `max |u - u'| <= 1e-5` (stream speed 1).
* E1 (ENERGY, steady, closed form). PLUG geometry, `energy.enabled`, `physics.cp = 1000`, `conductivity = 0.1`, inlet `T = 300` fixed, `xmax` zero-gradient, walls adiabatic;
  particles hot `T_p0 = 400` K, `particle_cp = 800`, `density = 1000`, `diameter = 1e-3`, injected AT the stream velocity `(1,0,0)` (zero slip: no drag, `Nu = 2`),
  `mass_flow = 0.01`, `rate = 2000`, window `[0, 0.5]`. `tau_T = rho_p c_p d^2/(6 k Nu) = 2/3 s`, `C_f = rho c_pf U A = 50 W/K`, `C_p = mdot_p c_p = 8 W/K`, `r = C_p/C_f`;
  `T_p - T_f = (T_p0 - T_in) exp(-(1+r) x/(U tau_T))`, `C_f (T_f - T_in) = C_p (T_p0 - T_p)`, so `T_f(L) = (T_in + r T_p0 - r Delta_L)/(1+r)`. The carrier outlet temperature rise
  (mean of the last cell column) is within 2e-2 of the closed-form rise `T_f(L) - T_in` (relative).
* E2 (ENERGY, steady). Heat delivered to the carrier `rho c_pf sum_c V_c q_c` equals the particle heat loss `sum_i m_i c_p (T_p0 - T_end,i)/W` (independent from the particle list) to `1e-6` relative; the
  carrier enthalpy flux rise `rho c_pf (sum phi T)_out - (sum phi T)_in` equals the delivered heat to `1e-3` relative.
* E3 (ENERGY). The carrier velocity stays the plug: `max |u_x - 1| <= 1e-6` (no drag, nothing for the pressure to do).
* LE1 Located errors (each invalid input names the full key path): `two_way` without `particles.enabled`; with `coupled`; `frozen_flow`; `dynamic_mesh`; `two_way_relax = 0`, `1.5`;
  `two_way_every = 0`; `two_way_start = -1`; `two_way_tolerance = 0`; steady with injections of different `rate`; `heat_transfer` without `energy.enabled`; `particle_cp` unset;
  injection `temperature` unset; a steady run whose parcels are still active at the end of tracking (`particles.end_time`); valid input gives no diagnostic.
* BL Baseline hashes (`baseline_hashes`), all existing particle and source suites unchanged and passing.

### F-126

Set-up MP: `annulus_3d_stage`, `r_in = 0.5`, `r_out = 1`, `lz = 0.8`, `nx = 48` around (both blocks), `ny = 8` radial (both blocks, uniform), `nz = 4` (lower), `nz_right = 6` (upper),
`nu = 0.1`, `rho = 1`. Exact solution (FREE VORTEX plus uniform axial flow): `u = W z^ + (Gamma/r) theta^`, `W = 1`, `Gamma = 0.4`, i.e.
`u_x = -Gamma y/r^2`, `u_y = Gamma x/r^2`, `u_z = W`, kinematic `p = p_0 - Gamma^2/(2 r^2)`. It is an exact incompressible Navier-Stokes solution: the flow is irrotational (`curl u = 0`)
so `nu laplacian u = nu grad(div u) - nu curl curl u = 0`, the convective acceleration `u.grad u = -Gamma^2 r^-3 r^` is balanced by `grad p`. In the absolute-velocity frozen
rotor (ADR 0008) the rotating-zone terms `-(Omega x r).grad u + Omega x u` cancel identically for ANY axisymmetric field (`(Omega x r).grad u = Omega d_theta u = Omega z^ x u`), so the same field is the exact
solution of both zones for any `Omega` (here `Omega = 5` in the lower block, an MRF box over `z < 0.4`). Boundary data: `inner`, `outer` of both blocks fixed to the exact velocity (velocity expressions),
`zmin` the exact velocity, upper `zmax`: `radial_equilibrium_outlet` (`swirl_axis = z`, hub reference `bc_pressure` equal to the exact `p` at the hub radius of the outlet faces),
`plane_out`: `velocity = zero_gradient`, `pressure = fixed`; `plane_in`: `velocity = fixed`. `mixing_plane`: `bands = 8` (one band per radial layer on both sides: the layers tile the bands),
`relax = 0.5`, `tolerance = 1e-9`. `simplec`, `linear_upwind`, `non_orthogonal_correctors = 1`, `max_iterations = 3000`, `tolerance_u = tolerance_p = 1e-10`. Face fluxes are the solver's `sim.flux()`.
`V_ref = max |u_exact| = sqrt(W^2 + (Gamma/r_in)^2) = 1.2806`; the pressure scale `P = Gamma^2/2 (1/r_in^2 - 1/r_out^2) = 0.24`.

* MP1 Exchange convergence: after at most 3000 iterations the exchange change (monitor `res_mixing`) is `<= 1e-8`; the run is not terminated by a failure; the number of iterations and the final value are reported.
* MP2 Mass across the plane, converged: per band `|Q_up,b - Q_dn,b| <= 1e-7 |Q_b|` with `Q_up,b = sum phi` over the `plane_out` faces of the band and `Q_dn,b = -sum phi` over the `plane_in` faces
  (solver fluxes), the total `|Q_up - Q_dn| <= 1e-9 |Q|`; duct inlet against outlet mass flow `<= 1e-9`.
* MP3 Angular momentum across the plane, converged: per band `|G_up,b - G_dn,b| <= 1e-7 |G_b|` with `G = sum phi r u_theta` evaluated from the solver flux and the boundary face velocity (`plane_out`: zero-gradient cell value,
  `plane_in`: the fixed value), `r u_theta = (x - o) x u . axis`; the duct totals at `zmin` and `zmax` agree within `1e-2` relative (the exact solution has zero net wall torque; the residual is discretisation).
* MP4 Solution error against the exact field in all cells of both blocks: `max |u - u_exact| <= 2e-2 V_ref` (R5 of ADR 0055 measured 7.6e-3 V_i on a 48-around annulus with 16 radial cells; here 8 radial cells and a radial-equilibrium
  outlet); the pressure, after removing the mean offset (the level is set by the outlet reference), `max |p - p_exact - mean| <= 0.05 P`. The `plane_in` face velocity against `Gamma/r_g` (swirl) and `W` (axial): `<= 2e-2 V_ref`.
* MP5 Unset keys: a case with `mixing_plane.enabled = false` (and one without the section) on the connected `annulus_3d_interface` mesh is bit-identical in `ux, uy, uz, p` and the flux; the baseline hashes are unchanged; the new mesh generator
  is only reachable by name.
* MP6 Relaxation independence: `relax = 0.3` and `0.7` converge to the same state: `max |u - u'| <= 1e-6 V_ref`.
* MP7 Circumferential mixing (distortion): same set-up, inlet axial velocity `W (1 + 0.3 cos theta)` on `zmin` (not an exact solution): mass and angular momentum conservation as MP2 and MP3 per band `<= 1e-7`; the downstream-inlet
  velocity depends on radius only (max circumferential spread of each component within a band `<= 1e-12`); the mechanical energy flux of the plane is not created: `E_up - E_dn >= -1e-6 |E_up|` (mixing loss non-negative)
  with `E = sum phi (p + |u|^2/2)` from solver fluxes.
* MP8 Different sector counts (the limitation case of ADR 0055 addendum 4): upper block `nx_right = 36` around, otherwise MP; mass and angular momentum conservation as MP2 and MP3 per band `<= 1e-7` (they hold for any receiver tiling),
  `max |u - u_exact| <= 3e-2 V_ref` (the polygon rings of 48 and 36 sectors differ in area by 0.22 percent, so the passed axial velocity is scaled by that ratio).
* MP9 Located errors (each with a test): unknown upstream and downstream patch; same patch twice; patch with a `bc_kind`; upstream velocity not `zero_gradient`; upstream pressure not fixed; downstream velocity not fixed; `bands` unset; `r_min >= r_max`; zero axis;
  `relax = 0` and `1.5`; `every = 0`; `tolerance = 0`; `piso`; `coupled`; `frozen_flow`; a band with cancelling flux at run time is a failure whose message starts with `mixing_plane:`; valid input gives no diagnostic.
* BL Baseline hashes and all existing tests unchanged.

## Not delivered (planned statement, confirmed or reduced in the result addendum)

F-128: breakup, wall film, erosion, evaporation, dispersion from turbulence-model fields, the mechanical (kinetic/dissipation) energy exchange in the temperature equation, sub-iteration of the transient exchange, a published
comparison (the acceptance of the roadmap entry cites none that I have read; a published particle-laden jet is NOT ATTEMPTED). F-126: see D12; in particular no published turbomachine.

## Order of work

1. This ADR committed alone. 2. F-128 code, targeted tests, runs. 3. F-126 code (generator, run module, keys), targeted tests, runs. 4. Docs (case-schema, reference-data, roadmap), addendum with measured values and deviations.
5. Final gate on a `git archive` copy of the final commit.

## Addendum 1 (registered after the first PLUG run, before any run of the corrected set-up)

First PLUG run (as registered, steady, nx = 80): PL1 met (9.7e-8), PL5 met (4.4e-16), PL8 and LE1 met; PL2 missed (1.64e-3 against 1e-3), PL3 missed (1.5e-2 against 1e-6), PL4 missed (the change first fell below 1e-8 at update 144, against 40).
Diagnosis so far (a, b are measured; c is the hypothesis the corrected set-up tests):
(a) PL1 is exact by construction (the record is conservative); the measured p_c0 - p_last = 0.33884 against sum a_f dx = 0.33816 shows the face balance is not the cell balance, and the pressure the solver prints at the fixed-velocity inlet face (`pressure_face_values`) equals the
cell value, without the body-force gradient term of the fixed-flux condition; (b) the deviation of u_x is largest in the first cell (-1.5e-2) and decays over about 5 cells; with refinement it falls only slowly (nx = 40, 80, 160, 320: 2.1e-2, 1.5e-2, 1.1e-2, 7.6e-3, about
h^(1/2)), as expected from a source that is singular at the injection point (parcel number density ~ 1/v for parcels at rest, source ~ x^(-1/2)); (c) with a source that is smooth and starts away from the inlet boundary the plug is preserved much better and the global balance closes.
PL4: the coupling change cannot fall faster than the carrier field it depends on settles (about 140 SIMPLE iterations for the cell pressure gradient and velocity to adjust to a changing source), so the registered 40 updates assumed a frozen carrier; this is a mis-specified limit, and is
recorded as a known miss, not replaced.

PL2, PL3 and PL4 stay as registered in ignored `*_known_miss` tests (PL3 and PL4) and PL2 (ignored, limit intact). New corrected set-up PLUG-B (a set-up defect: the rest-injection singularity at the boundary cell), registered here BEFORE it is run:
`channel_2d` as PLUG, point injection at (0.4, 0.25, 0.05), velocity given (0.5, 0, 0), `tau = 5 s` (`d = 9.487e-3` for `rho_p = 1000`, `nu = 1e-3`), `rate = 2000`, window [0, 0.5], `mass_flow = 0.02`, tracking `end_time = 12`, `dt = 0.05`.
Closed form: `v = U - (U - v0) e^{-t/tau}`, `x = x0 + U t - (U - v0) tau (1 - e^{-t/tau})`; `x(t_L) = L`; `F = mdot_p (v_L - v0)` (the parcels enter with momentum `mdot_p v0`).
* PL1b `|F_s - mdot_p (v_L - v0)| <= 1e-6 F`; PL2b `|(p_in - p_out) A / rho - F| <= 1e-3 F` with the inlet pressure the cell value extrapolated with the body-force gradient of the fixed-flux condition (a definition, as used for PL2 in the corrected measurement);
PL3b `max |u_x - 1| <= 2e-3` and `max |u_y| <= 2e-3` (estimate: the cell-pressure-gradient inconsistency at the start of the source is `a_max/2` with `a_max = mdot_p (U - v0)/(tau v0 rho A) = 0.08`, giving `dx a_max / (2 U^2) = 5e-4`; the limit has a factor 4).
The measured values and the diagnosis are recorded in the result addendum. Definition notes for the measured PL1: `F_s := -sum rho V a` (the force the carrier exerts on the parcels; the record gives the momentum TO the carrier, which is minus that).

## Addendum 2 (result addendum, written after all runs)

Everything below was measured in release builds with `RUST_TEST_THREADS=3`. No published data was read or used for either feature (no web page was fetched).
A published particle-laden jet and a published turbomachine are NOT ATTEMPTED. Limits are those registered above and in addendum 1; none was widened, replaced or deleted.

### F-128, measured against the registered limits

| Check | Measured | Limit | Result |
|---|---|---|---|
| PL1 force on parcels = parcel momentum flux (PLUG) | 9.71e-8 | 1e-6 | met |
| PL2 pressure drop (PLUG) | 1.64e-3 | 1e-3 | MISS (`pl2_steady_plug_pressure_drop_known_miss`, ignored) |
| PL3 plug uniformity (PLUG) | 1.526e-2 | 1e-6 | MISS (`pl3_plug_stays_uniform_known_miss`, ignored) |
| PL4 coupling change < 1e-8 within 40 updates | first at update 144 | 40 | MISS (`pl4_coupling_converges_within_40_updates_known_miss`, ignored) |
| PL5 | 4.44e-16 | 1e-6 | met |
| PL6a transient identity (piso, pimple) | 0 and 1.99e-16 | 1e-12 | met |
| PL6b transient delivered vs exchanged (piso, pimple) | 4.31e-14 and 1.75e-14 | 1e-12 | met |
| PL7 pressure drop, transient (piso, pimple) | -2.26e-3 and -2.27e-3 relative | 1e-2 | met |
| PL7, PL7b plug uniformity, transient | 1.14e-2 | 1e-6 | MISS (ignored known_miss tests, same cause as PL3) |
| PL8 unset keys: steady, transient, heat/energy | bit-identical | bit-identical | met |
| PL1b (PLUG-B) | 1.836e-5 | 1e-6 | MISS (ignored known_miss test) |
| PL2b (PLUG-B) | 7.60e-4 | 1e-3 | met |
| PL3b (PLUG-B) | 5.30e-4 | 2e-3 | met |
| J1 jet convergence: coupling change, res_u, res_p | 4.67e-9 (27 updates, 288 iterations), 9.9e-10, 1.7e-10 | 1e-6 | met, converged = true |
| J2 raw record vs particle sum / used vs raw | 5.65e-16 / 2.34e-9 | 1e-12 / 1e-6 | met |
| J3 x-momentum balance residual (convective 1.058e-4, pressure 2.640e-2, viscous 8.088e-3, source 1.8415e-2) | 3.9e-11 | 2e-2 | met |
| J3 y residual / `|F_y|/|F_x|` | 2.06e-13 / 3.4e-13 | 1e-6 / 1e-9 | met |
| J4 `D(0.1)/D(0.01)` (D = 6.554e-3, 6.275e-2, 0.4571 at f = 0.01, 0.1, 1), monotone | 9.57 | [9, 11] | met |
| J5 stationarity | 1.82e-9 | 1e-5 | met |
| E1 outlet temperature rise 13.3582 K against the closed form 13.3682 K | 7.46e-4 | 2e-2 | met |
| E2 delivered heat 668.230569 W vs particle loss 668.230568 W / enthalpy flux rise | 4.7e-10 / 4.80e-4 | 1e-6 / 1e-3 | met |
| E3 | 4.35e-14 | 1e-6 | met |
| LE1 located errors (including the runtime "particles.end_time ... still in flight") | all present | all present | met |

Acceptance (roadmap F-128: "a two-way coupled particle-laden jet conserves total momentum"): reached on the first-principles jet (J3 residual 3.9e-11 against 2e-2, J2 5.65e-16). Not a published jet.

Diagnoses and deviations (F-128):

* PL2/PL3/PL4/PL7 uniformity (addendum 1 diagnosis confirmed). The deviation falls only about as `h^(1/2)` with refinement (nx = 40, 80, 160, 320: 2.1e-2, 1.5e-2, 1.1e-2, 7.6e-3), as expected from the `x^(-1/2)` source of parcels injected at rest in the boundary cell, plus the collocated Green-Gauss cell pressure gradient being inconsistent with the face balance for a source that varies on the cell scale. PL4: the coupling change cannot settle faster than the carrier (a mis-specified limit, kept).
* PL1b (a registered PLUG-B limit) missed, 1.836e-5 against 1e-6. Cause: the closed form assumes an exactly uniform carrier, but the discrete carrier deviates from uniform by up to 5.3e-4 (PL3b), which changes the drag by that relative amount in the first cells. The identity "force on the carrier = momentum of the parcels, from the record" holds to 2.1e-16, but that check was added AFTER the PL1b miss (post-hoc, not a registered criterion; reported as such).
* PLUG-B was run with `max_iterations = 1500`; it did NOT meet the residual criterion (`converged` false, residual stall) while the coupling change fell below 1e-8 at update 102; PL2b and PL3b are measured on that state. The PLUG-B test therefore does not assert `is_converged`.
* Change metric (a definition fixed during the work, not a limit): the denominators of the relative change have floors of 1e-3 of the momentum (heat) content the parcels carry. Without the floor a heat-only case has a momentum record that is pure round-off (1e-15) and its relative change stalls at about 3e-3 and the run can never be converged; with it, 7.5e-9.
* `particles.heat_transfer` requires `particles.two_way` (a located error otherwise); the one-way thermal path is NOT delivered. Only `energy_heat` is fed into the temperature equation; the mechanical exchange (drag dissipation) is neglected.
* Sign and definitions: the record is the momentum TO the carrier; in the tests `F_s := -sum rho V a` is the force the carrier exerts on the parcels. PL2 uses the inlet pressure extrapolated with the body-force gradient of the fixed-flux condition, `p_in = p_c0 - x_c0 a_c0`, because `pressure_face_values` returns the cell value at fixed-velocity faces.
* PL8 and LE1 test bounds: tolerances below 1e-14 are rejected by the schema, so the tightest tolerance used is 1e-14; `physics.cp` and `physics.conductivity` have valid defaults, so they are not in the expected error list of LE1.
* `write_run_outputs` for a steady two-way run still tracks the final field once with the one-way `steady_run` (not the tracker of the last coupling update).
* The steady coupling needs every parcel to leave or deposit within `particles.end_time`; otherwise it is a located failure at `particles.end_time`.

### F-126, measured against the registered limits (set-up MP exactly as registered)

| Check | Measured | Limit | Result |
|---|---|---|---|
| MP1 exchange change `res_mixing` after 189 iterations, converged = true | 4.23e-14 | 1e-8 | met |
| MP2 mass per band / total / duct inlet vs outlet | 2.47e-14 / 4.54e-15 / 6.75e-14 | 1e-7 / 1e-9 / 1e-9 | met |
| MP3 angular momentum per band; duct totals in and out (-0.939789, 0.937900) | 2.40e-14; 2.01e-3 | 1e-7; 1e-2 | met |
| MP4 velocity error, all cells | 6.00e-3 V_ref | 2e-2 V_ref | met |
| MP4 `plane_in` face velocity error | 2.94e-3 V_ref | 2e-2 V_ref | met |
| MP4 pressure error after removing the mean | 6.85e-2 P | 5e-2 P | MISS (`mp4_pressure_known_miss`, ignored) |
| MP5 `enabled = false` (with and without other keys, with and without the section) on `annulus_3d_interface` | bit-identical in ux, uy, uz, p, flux; no `res_mixing` monitor | bit-identical | met |
| MP6 relax 0.3 vs 0.7 (329 iterations for 0.3) | 2.63e-13 V_ref | 1e-6 V_ref | met |
| MP7 mass / angular momentum per band (cos-theta distortion) | 5.05e-13 / 5.33e-13 | 1e-7 | met |
| MP7 polar spread of `plane_in` velocity within a band | 6.66e-16 | 1e-12 | met |
| MP7 energy flux up 1.325413, down 1.168667: loss / E | +1.18e-1 | >= -1e-6 | met |
| MP8 (36 cells around above) mass / angular momentum per band; velocity error | 1.47e-14 / 1.49e-14; 6.12e-3 V_ref | 1e-7; 3e-2 V_ref | met |
| MP9 located errors (each mutation), runtime cancelling band message prefix `mixing_plane:`, valid input clean | all present | all present | met |
| BL baseline hashes and existing tests | see below | unchanged | see the final gate |

Acceptance (roadmap F-126, a public compressor case reproducing pressure ratio and efficiency): NOT MET, NOT ATTEMPTED. Reached here: the steady swirling annular duct with a known exact solution through the plane, mass and angular momentum conserved across the plane.

Diagnoses and deviations (F-126):

* MP4 pressure: the maximum is in the innermost radial cell of both blocks (the block means differ by 4e-3 P), and a plane-free control (one connected annulus, `annulus_3d`, same exact flow, informational, no criterion, ignored test `mp4_control_single_block`) gives 1.13e-1, 6.62e-2, 4.24e-2 P for 4, 8, 16 radial layers: the same error without a plane, converging at roughly first order. The cause is attributed to the near-hub-wall pressure discretisation (the zero-gradient wall pressure against the exact centripetal gradient); that attribution is an order-of-magnitude estimate (`dp/dr dr/4 = 0.0167` against the measured 0.0164), not an isolated proof. The limit stays as registered.
* MP2 and MP3 are identities of the passing construction (`u_n = Q/A'`, `u_theta = Gamma/r`) at the band level, evaluated on the solver flux; they test the wiring and the installed boundary state, and are not independent physics checks. MP4, MP6 and MP8 are the independent checks.
* Generator: the sector polygon of the two blocks coincides in radius extremes; for MP8 the two rings differ in area, the passed axial velocity is scaled (accounted for in the registered MP8 limit).
* The `nx_right` and `nz_right` keys are reused by `annulus_3d_stage` (0 = same as `nx`, `nz`) instead of new keys; `ny_right` likewise.
* The exchange runs at the top of every `every`-th iteration from the FIRST (the donor is then the initial field), not from the second as D10 planned; the initial downstream state is the installed boundary value read from the fields, and the upstream pressure is the installed pressure.
* MP9: patch names exist only in the mesh, so "unknown upstream/downstream patch" is a located error of `Simulation::new` (path `mixing_plane.upstream_patch` / `.downstream_patch`), not of `Case::semantic_errors`; the boundary checks of the patches that have a `[boundaries]` section are in `semantic_errors`; a mesh patch without a `[boundaries]` section is a located error at `boundaries.<name>` from `Simulation::new`.
* The exchange fails the run through the existing failure channel (`failure()` and `stop_reason()`), message prefix `mixing_plane:`.

### Not delivered (confirmed)

F-128: breakup, wall film, erosion, evaporation, turbulence-model dispersion fields, the mechanical energy exchange, sub-iteration of the transient exchange, the one-way thermal path (`heat_transfer` without `two_way`), a published comparison. F-126: see D12 (transient and coupled solvers, compressible mixed-out state, rothalpy, curved or non-aligned interfaces, band edges that do not follow cell layers, sector periodicity of a stage, the downstream-velocity constraint, flow-regime options, turbulence or scalar transfer), a published turbomachine.
