# ADR 0022: Boundary-condition catalogue completion (F-200), constant-density segregated kernels

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new code was compiled
or run. The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test
set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (file names, details that were not fixed
here) may be appended at the end without touching registered limits.

Roadmap: F-200 (with the pieces F-062 moved to it: outlet pressure averaging, mass-flow update strategies, flow-regime
options). Clean-room: published literature only; nothing from vendor material, vendor installs or manuals; no vendor
names in the repository. Closed forms below are derived here and need no reference. Textbook facts used: Bernoulli, the
Hagen-Poiseuille / plane Poiseuille / annular Poiseuille laminar solutions, and the radial momentum balance of an axisymmetric
swirling flow.

## Scope audit (what existed before this ADR)

Existing, unchanged: `velocity` = fixed (also per-face expressions, tables via `profile`, user functions), zero_gradient,
symmetry, freestream, moving_wall / shear / partial_slip / marangoni (F-204); `pressure` = zero_gradient | fixed | freestream
(kinematic value); `temperature` = zero_gradient | fixed | heat_flux; `turbulence` role and intensity/length/viscosity-ratio
inlet specification (k-epsilon, SST, SA); periodic and empty patches (mesh-level); the compressible kernel's `compressible_bc`
(ADR 0006: total_inlet, pressure_outlet, farfield, nonreflecting); the variable-density `inlet = stagnation`; lumped
`network_outlet` components (ADR 0012).

F-200 lists: mass-flow inlets/outlets; total-pressure inlet with direction and swirl; outflow with flow-rate weights; vents
with loss coefficients; outlet-to-inlet recirculation pair; backflow direction and pressure options; target-mass-flow and
radial-equilibrium pressure outlets; reverse-flow prevention; opening/entrainment-type boundaries; outlet pressure averaging
and mass-flow update options. None of these exists. Per-boundary thermal conditions (convection coefficient, etc.) are F-051,
not F-200, and are not touched. Turbulence inlet specification already exists; this ADR only checks that the new inlet kinds
feed it correctly.

Scope decision, stated up front: all new kinds are implemented for the constant-density segregated kernels (`simple`,
`simplec`, `piso`, `pimple`; transient use is covered by check T1 only). They are located validation errors with the
compressible kernel, the variable-density kernel and the block-coupled solver. A solid circular pipe mesh generator does not exist; the Hagen-Poiseuille check is run on a plane
channel and on an annular duct (closed form below), not on a solid pipe.

## Decisions

### D0 Schema (additive)

One new key selects the kind: `boundaries.<patch>.bc_kind` = `none` (default, the unset sentinel: the patch is read exactly as
before) | `mass_flow` | `outflow` | `pressure_outlet` | `opening` | `vent` | `total_pressure_inlet` |
`radial_equilibrium_outlet` | `target_mass_flow_outlet`. With a kind other than `none` the kind owns the velocity and pressure
conditions of the patch: `velocity` must stay `fixed`, `pressure` `zero_gradient`, `velocity_value` zero and no
`velocity_expression_*`, `velocity_udf` or `profile` given (located errors otherwise); the `temperature`, `turbulence` and
scalar keys keep their meaning. New pressure keys are in Pa and are converted with `physics.rho` (the older `pressure_value`
stays kinematic and is not used by the new kinds). New keys, all with inactive defaults:

| key | default | used by |
|---|---|---|
| `mass_flow_value` [kg/s, positive INTO the domain, negative out] | 0 | mass_flow |
| `mass_flow_shape_expression` (velocity-dimensioned expression of x, y, z; only the shape counts; a table call is allowed) | "" (uniform) | mass_flow |
| `mass_flow_source_patch`, `mass_flow_fraction` (0, 1] | "", 1 | mass_flow |
| `flow_direction` (existing key) | [0,0,0] = into the domain along `-n` (`+n` for a prescribed outflow) | mass_flow, total_pressure_inlet |
| `swirl_rate` [rad/s], `swirl_axis`, `swirl_origin` | 0, [0,0,1], origin | mass_flow, total_pressure_inlet (axis and origin also radial_equilibrium_outlet) |
| `bc_pressure` [Pa] | 0 | pressure_outlet, opening, vent, total_pressure_inlet, radial_equilibrium_outlet, target_mass_flow_outlet (initial value) |
| `backflow_direction` normal \| zero_gradient | normal | pressure_outlet |
| `backflow_pressure` static \| total | static | pressure_outlet |
| `prevent_reverse_flow` | false | pressure_outlet, outflow |
| `pressure_average` none \| area | none | pressure_outlet |
| `loss_coefficient` K >= 0 | 0 | vent |
| `flow_rate_weight` > 0 | 1 | outflow |
| `radial_reference` hub \| area_average | hub | radial_equilibrium_outlet |
| `target_mass_flow` [kg/s, positive OUT of the domain] | 0 | target_mass_flow_outlet |
| `pressure_update` bernoulli \| secant, `pressure_relax` [0.01, 1] | bernoulli, 0.5 | target_mass_flow_outlet |

A key that is not at its default on a kind that does not use it is a located error naming the key and the kinds that use it.
Every pressure-type kind takes the kinematic pressure `p = bc_pressure / rho`.

Sign conventions, three of them and deliberately different in the place where the physics names them: the input
`mass_flow_value` is positive INTO the domain, `target_mass_flow` is positive OUT of the domain, and the `mass_flow` report
(and `rho sum phi` below) is positive OUT of the domain. Notation: `n` outward unit normal of a boundary face, `A` its area, `rho = physics.rho`, `phi_f` the (kinematic) face flux
`u.S` of the last iteration or step (outward positive), `u_P` the owner-cell velocity, `u_f` the face velocity. Refresh: the
conditions that depend on the flow are rebuilt before every iteration (steady) or step (transient) from the lagged fields
(`phi_f`, `u`, `p`) with `set_bc`, the mechanism the lumped network outlet and the Marangoni stress already use. At build the
refresh runs once with `phi = 0`.

### D1 `mass_flow` (inlet or outlet, direction, profile, swirl, recirculation)

Signed `m = mass_flow_value` (into the domain). Direction `e`: `flow_direction` normalised, or `-n_f` per face when it is zero
(`+n_f` for m < 0). A face needs `-e.n_f >= 0.05` (`+e.n_f` for m < 0), otherwise a located error on `flow_direction`.
Shape `s_f >= 0`: 1, or the expression at the face centre (a negative value, or a zero total, is a located error on
`mass_flow_shape_expression`). The face velocity is `u_f = c s_f e + t_f` with the swirl `t_f = P_tan(Omega x (x_f - x_0))`
(`Omega = swirl_rate * axis/|axis|`, `P_tan` removes the normal part), and
`c = |m| / (rho sum_f s_f |e.n_f| A_f)`. Then `sum_f rho u_f.S_f = -m` exactly (up to round-off, the swirl being tangential)
and the velocity is a fixed per-face value, so the pressure equation treats the patch as a prescribed flux
(`pressure_bc.fixed_u`). The pressure condition is zero gradient (implied).
Recirculation pair: `mass_flow_source_patch = B` with `mass_flow_fraction = f` replaces `mass_flow_value`: `m = f max(m_B, 0)`
with `m_B = rho sum phi` over B (outward, lagged). Setting both is an error; B must exist, differ from the patch and not itself be
sourced from this patch (cycle check over the chain). A mass-flow-rate sourced inlet is a Picard iteration of gain `f < 1`.
With the turbulence model the inlet speed per face is `|u_f|` of the first build (`phi = 0` gives zero for a sourced patch, so a
sourced patch with a turbulence inlet role needs `turbulence_spec = values`; located error).

### D2 `outflow` with flow-rate weights and reverse-flow prevention

Total inflow `N = max(-rho sum phi, 0)` summed over all patches that are not `outflow`. Patch i takes the share
`T_i = N w_i / sum_j w_j`. Face velocity `u_f = q_f n_f + P_tan(u_P)` (zero gradient tangential part), `q_f = scale s_f`,
`s_f = max(u_P.n_f, 0)` when `prevent_reverse_flow` else `u_P.n_f`, `scale = T_i / (rho sum s_f A_f)`; if
`|sum s_f A_f| < 1e-6 sum |s_f| A_f` the shape falls back to 1. Then `rho sum phi = T_i` exactly; with all other patches
prescribed-flux the whole boundary is compatible and the pressure level comes from `solver.p_ref_*`.

### D3 `pressure_outlet`, `opening`, `vent` (backflow options, averaging, loss)

Per face backflow mask (hysteresis-free rule): backflow iff `phi_f < -eps`, outflow iff `phi_f > eps`, and for
`|phi_f| <= eps` (`eps = 1e-12 * max(sum|phi|, 1e-30)` of the patch) backflow iff `p_P < p_set` (so a closed face re-opens
when the interior pressure exceeds the exterior one and the iteration cannot chatter on `phi = 0`).
* Velocity: outflow faces zero gradient. Backflow faces: `normal` gives `u_f = n (n.u_P)` (the new `Directed` condition with
  direction n), `zero_gradient` keeps zero gradient, `prevent_reverse_flow` sets `u_f = 0` (flux zero, the face is closed).
* Pressure: `p_f = p_set` on outflow faces; on backflow faces `p_set` (`static`) or `p_set - |u_f|^2/2` (`total`). With
  `pressure_average = area`, `p_set` is replaced by `p_set + (p_P - mean_A(p_P))` per face, using the SAME lagged cell values for
  the mean and the per-face term, so the area mean of the applied static pressure equals `p_set` identically.
* `opening` is `pressure_outlet` with `backflow_pressure = total`, `backflow_direction = normal` (entrainment through the opening
  at the external total pressure `bc_pressure`); it has no further keys and is checked to be bitwise identical to that
  configuration.
* `vent`: velocity zero gradient; `p_f = p_ext + K u_n |u_n| / 2` (kinematic, `u_n = n.u_f` of the lagged face velocity, a pure
  loss law; no dynamic head is added in either direction), relaxed with weight 0.5 against the previous value. `K = 0` is a fixed
  static pressure.

### D4 `total_pressure_inlet` (incompressible), direction and swirl

Total pressure `p0 = bc_pressure/rho` (kinematic). Velocity: the new `Directed` condition `u_f = e (n.u_P)/(n.e) + t_f`, i.e. the
normal flux component is zero gradient and the direction `e` (`flow_direction`, default `-n_f`) and the swirl `t_f` of D1 are
imposed. A face needs `-e.n_f >= 0.05` and `|e_k| >= 0.05 |e|` for the dominant normal component k, otherwise a located error on
`flow_direction` (the second keeps the pressure solver from taking the face for a prescribed flux). Pressure: inflow faces
(`phi_f < eps`) `p_f = p0 - |u_f|^2/2`, outflow faces `p_f = p0`, relaxed with weight 0.5 against the previous value.
Turbulence inlet values need a known speed, which a total-pressure inlet does not have a priori: a turbulence model with this
kind requires `turbulence_spec = values` (located error).

### D5 `radial_equilibrium_outlet`

Axis `swirl_axis`, `swirl_origin`. Per face: radius `r_f` (distance from the axis line), tangential velocity
`w_f = u_f . (axis x r_hat)` from the lagged face velocity, `g_f = w_f^2 / r_f`. Faces sorted by `r`; the kinematic pressure
follows `dp/dr = g` by the trapezoid rule `p_{i+1} = p_i + (g_i + g_{i+1})(r_{i+1} - r_i)/2`. Reference: `hub` fixes the
pressure of the smallest-radius face to `bc_pressure/rho`; `area_average` shifts the whole profile so the area mean of the face
pressure equals `bc_pressure/rho`. Velocity zero gradient. Relaxed with weight 0.5 against the previous profile. A face on the
axis (`r_f < 1e-12 * max r`) is a located error (g singular).

### D6 `target_mass_flow_outlet` and the update options

Uniform static pressure `p_t` (kinematic, initial `bc_pressure/rho`), velocity zero gradient. With outward mass flow
`m_n = rho sum phi` and error `e_n = m_n - target_mass_flow`, `A` the patch area and
`U_ref = max(|m_n|, |target|)/(rho A)`:
* `bernoulli`: `p_{n+1} = p_n + r e_n U_ref/(rho A)` (a dynamic-head sensitivity estimate, `r = pressure_relax`).
* `secant`: slope `s = (p_n - p_{n-1})/(m_n - m_{n-1})` of the last two iterates when `|m_n - m_{n-1}| > 1e-9 max(|m_n|, |target|)`
  and `s > 0`, clipped to `[0.01, 1000] U_ref/(rho A)`, otherwise the `bernoulli` sensitivity; `p_{n+1} = p_n + r s e_n`.
The update runs once per iteration or step. Located errors: `target_mass_flow < 0`.

### D6b Other consumers of the velocity kind (role, init, user functions)

The existing readers resolve a patch from `velocity` and `velocity_value`, which for every new kind are the inert pair
`fixed`/zero (a no-slip wall to them). They now check `bc_kind` first: `turb::role` with `turbulence = auto` gives `Inlet` for
an inflowing `mass_flow` (`mass_flow_value > 0`, or a sourced one) and for `total_pressure_inlet`, and `ZeroGradient` for every
other kind (so none of them enters the wall set, the wall-distance set or the y+ report); an explicit `turbulence` value still
wins. The inlet speed per face is read from the refreshed boundary velocity (`|u_f|`), not from `velocity_value`.
`reference_speed` takes the same speed. `initial.method = potential` supports `mass_flow` (flux from the boundary velocity) and
rejects the other kinds with a located error on `bc_kind`. `velocity_udf` is already excluded by D0.

### D7 Library additions (`rcfd-fv/src/bc.rs`)

`Directed { dir: Option<Vec3>, tangential: Profile }` implements `u_f = e (n.u_P)/(n.e) + t` (with `dir = None`, `e = n`, the
"normal backflow" condition `u_f = n (n.u_P)`); `Switched { mask: Vec<bool>, on: Box<dyn Bc>, off: Box<dyn Bc> }` picks a branch per
face by `local_face`. Pure functions (unit tested without a mesh): outflow scaling, radial-equilibrium integration, area-mean
shift, secant/Bernoulli update.

## Registered checks

All limits are on the quantity stated; "rel" is relative to the stated scale. Files: `crates/rcfd-fv/tests/bc_catalogue.rs`
(library, no mesh or box mesh) and `crates/rcfd-api/tests/bc_catalogue.rs` (settings-tree cases). Common solver settings of the
case checks: `simplec`, `linear_upwind`, tolerances `1e-12` on `res_u`/`res_p` unless a case says otherwise, at most 6000
iterations; "converged" means the run stopped on the residual targets (a run that stops on the cap is reported NOT MET).

### L library units (`rcfd-fv`)
* L1 `Directed` with `dir = None` on an oblique normal: `u_f.n = u_P.n` and `u_f - n (n.u_P) = 0` to 1e-14; with a direction e: `u_f.n = u_P.n` and `u_f x e = 0` to 1e-14; with tangential t added: `u_f.n` unchanged to 1e-14.
* L2 `Switched` returns the `on` branch coefficients exactly (bitwise) for true faces and the `off` ones for false faces.
* L3 outflow scaling: for any shape (including negative entries) `sum q A = T` to 1e-13 rel; with clipping all `q >= 0`.
* L4 area-mean shift: mean over faces of the applied static pressure equals `p_set` to 1e-12 rel to `max|p_P|`, on random areas and values.
* L5 radial integration: solid-body `w = Omega r` on 40 non-uniform radii in [0.5, 1]: `p(r) - p(r_hub) = Omega^2 (r^2 - r_hub^2)/2` to 1e-12 rel to the range; free vortex `w = Gamma/r` on 200 uniform radii: `p = -Gamma^2/(2 r^2) + const` to 1e-4 rel to the range (trapezoid error estimate 1.6e-5); `area_average` reference: area mean equals the requested value to 1e-12 rel.
* L6 update rules: Bernoulli step sign and size equal the formula to 1e-14; secant equals the formula for a linear `m(p)` after two iterates (exact root in one further step with `r = 1`, to 1e-12 rel).

### A mass-flow kinds (D1)
* A1 exactness: first iteration, `|rho sum phi - m|/|m| <= 1e-12` for (a) uniform profile on the channel inlet, (b) parabolic expression `4 y (H-y)/H^2` shape, (c) oblique direction (30 degrees to the normal) on a periodic-in-y box, (d) swirl on an annulus inlet, (e) `rho = 1.2`, (f) negative `mass_flow_value` (a prescribed outflow), (g) a graded mesh.
* A2 plane Poiseuille: channel `lx = 10`, `ly = H = 1`, `depth = 0.1`, `nx = 100`, `ny = 20`, `nu = 0.1`, `rho = 1`, mass-flow inlet `m = 0.1` kg/s (mean velocity U = 1), outlet fixed pressure 0. Closed form `-dp/dx = 12 mu U/H^2 = 1.2` Pa/m. Check: least-squares slope of the column-mean cell pressure over columns with 6 <= x <= 9: error `<= 1 %` (the acceptance limit). Profile at the column nearest x = 9: `max|u - 6U(y/H)(1 - y/H)| <= 1.5 %` of `1.5 U`.
* A2b the same with the parabolic shape expression: the same two limits, plus the slope over `1 <= x <= 9` `<= 1 %`.
* A3 annular Hagen-Poiseuille: `annulus_3d`, `r_in = 0.5`, `r_out = 1`, `lz = 4`, `nx = 48` around, `ny = 24` radial, `nz = 24`, `nu = 0.1`, `rho = 1`, mean axial velocity `W = 1` through the polygon area, walls fixed, outer and inner no-slip, mass-flow inlet at `zmin`, fixed pressure 0 at `zmax`. Closed form with the radii of the circles of equal area to the 48-gons (`R = r sqrt(n sin(2 pi/n)/(2 pi))`, 0.499286 and 0.998572) and `Q = W A_polygon = 2.349471`: `-dp/dz = 8 mu Q / (pi [R2^4 - R1^4 - (R2^2 - R1^2)^2/ln(R2/R1)]) = 4.77614` Pa/m. Check: slope of the area-mean cell pressure over `1.5 <= z <= 3.5` within `1 %`.
  Resolution rationale (chosen before any run): the cell-centred half-cell wall gradient makes the discrete Poiseuille solution
  differ from the parabola by about `G h^2/(8 nu)`, a flow-rate error of about `1.5 (h/gap)^2`: 0.375 % for A2 (`ny = 20`) and
  0.26 % for A3 (24 cells over the gap 0.5); 12 cells would give 1.04 % and could not meet the 1 % acceptance limit.
* A4 swirl: at the first iteration the boundary velocity of an annulus inlet face equals `P_tan(Omega x r) + axial` with `Omega = 2` to 1e-12 rel, and `rho sum phi` unchanged (A1d).
* A5 direction: the oblique inlet of A1c: the face velocity direction has `|u x e| / |u| <= 1e-12`.
* A6 recirculation pair: `box_2d` 40 x 40, `xmin` mass-flow inlet `m0 = 0.1`, `ymax` mass-flow inlet sourced from `xmax` with `f = 0.3`, `xmax` fixed pressure outlet, `ymin` wall: converged `m_out = m0/(1 - f)` to `1e-8` rel and `m_ymax = f m_out` to `1e-8` rel.
* A7 turbulence inlet values: `I = 0.05`, `U = 1` (speed from `m/(rho A)`), `l = 0.07`, `Cmu = 0.09`, `nu = 0.1`: `k = 1.5 (I U)^2 = 3.75e-3`, `eps = Cmu^{3/4} k^{3/2}/l = 5.390521e-4`, `omega = sqrt(k)/(Cmu^{1/4} l) = 1.597191` to 1e-12 rel from the values the model receives; viscosity-ratio form `eps = Cmu k^2/(nu * ratio)` with ratio 10 equals `0.09 * (3.75e-3)^2 / (0.1 * 10) = 1.265625e-6` (limit 1e-12 rel).
* A8 located validation errors (each asserts the path): V1 every kind with `compressible`, with variable density, and with `coupled` -> `boundaries.<p>.bc_kind`; V2 `bc_kind != none` with `velocity != fixed` -> `.velocity`, with `pressure != zero_gradient` -> `.pressure`, with `velocity_value != 0` -> `.velocity_value`, with a `velocity_expression_x` -> `.velocity_expression_x`; V3 `flow_direction` with a face of `-e.n < 0.05` -> `.flow_direction`; V4 `mass_flow_source_patch` unknown / itself / a cycle / together with `mass_flow_value` / fraction outside (0, 1] -> `.mass_flow_source_patch` or `.mass_flow_fraction`; V5 `flow_rate_weight <= 0` -> `.flow_rate_weight`; V6 `loss_coefficient < 0` -> `.loss_coefficient`; V7 shape expression with a compile error, with a negative value, with zero total -> `.mass_flow_shape_expression`; V8 `target_mass_flow < 0` -> `.target_mass_flow`; V9 turbulence model with `total_pressure_inlet` (`turbulence_spec != values`) -> `.turbulence_spec`; V10 `swirl_axis` of zero length with `swirl_rate != 0` -> `.swirl_axis`; V11 an unused key on a kind (one case per table row) -> that key; V12 a radial outlet face on the axis -> `.swirl_axis`/`.swirl_origin`; V13 an unknown patch is already reported by the existing check (unchanged).

### B outflow weights (D2)
* B1 `box_2d` 30 x 30: `xmin` mass-flow inlet `m_in = 0.2`, `xmax` outflow weight 1, `ymax` outflow weight 3, walls elsewhere; converged: outflow shares `0.25` and `0.75` of `m_in` to `1e-10` rel.
* B2 three outflow patches (weights 1, 2, 3 on `xmax`, `ymax`, `ymin` split from a `xmin` inlet): shares 1/6, 2/6, 3/6 to `1e-10`.
* B3 mass conservation: `|sum over all patches of rho sum phi| / m_in <= 1e-12` in B1 and B2.
* B4 reverse-flow prevention: L3 plus: with `prevent_reverse_flow = true` every outflow face has `u.n >= 0` at the end of B1 (exact).

### C pressure outlet, opening, vent (D3)
* C1 averaging: L4, and a converged channel (A2 geometry, mass-flow inlet, `pressure_average = area`, `bc_pressure = 0`): the area-average report of `p` at the outlet is `0` to `1e-9` (rho U^2 = 1 scale), and the A2 slope limit holds.
* C2 total-pressure backflow: `box_2d` 20 x 4 with `ymin`/`ymax` symmetry (inviscid uniform flow is an exact solution), `nu = 1e-3`, `solver.component_diagonal = corrected` (the averaged diagonal stalls with symmetry walls, ADR 0010 / 0016; this holds for every check below that uses symmetry walls: C3, C5, D1), `xmin` fixed static pressure 0 (zero-gradient velocity), `xmax` `pressure_outlet` with `backflow_pressure = total`, `backflow_direction = normal`, `bc_pressure = 0.5` Pa (rho = 1): the flow enters through `xmax`; Bernoulli with uniform pressure gives `u = sqrt(2 * 0.5) = 1`: converged `|u_x| = 1` to `1e-5` rel (cell mean) and mass flow in = out to `1e-9`.
* C3 vent: the A1a channel without friction (symmetry walls, `nu = 1e-3`): mass-flow inlet `U = 1`, `vent` at the outlet with `bc_pressure = 0`: cell pressure at the outlet column equals `K U^2/2` to `1e-6` rel (absolute `1e-8` for K = 0) for `K = 0, 2.5, 10`; with the flow reversed (`m < 0` at the former inlet, the vent now an inflow) `p = -K U^2/2` to the same limits.
* C4 `opening` equals the explicit `pressure_outlet` (total, normal) configuration bitwise (maximum field difference 0) on the C2 case.
* C5 `prevent_reverse_flow`: the C2 case with `prevent_reverse_flow = true`: the patch flux is exactly zero (`|rho sum phi| <= 1e-12`, versus 1.0 kg/s scale of C2) and the converged `|u|_max <= 1e-6`.

### D total-pressure inlet (D4)
* D1 uniform duct as C2 mirrored: `xmin` `total_pressure_inlet` `bc_pressure = 0.5`, `xmax` static pressure 0: `u_x = 1` to `1e-5` rel.
* D2 direction: `box_2d(periodic_y)` 20 x 8, `component_diagonal = corrected`, `flow_direction = (cos 30, sin 30, 0)`, `bc_pressure = 0.5`: every cell has `|u| = 1` to `1e-5` rel and `atan2(v, u) = 30 deg` to `1e-6` rad (uniform flow with a transverse component is an exact solution of the periodic strip).
* D3 swirl: the A4 face velocity check for `total_pressure_inlet` (`swirl_rate = 2`): tangential part `= P_tan(Omega x r)` to 1e-12 rel.

### E radial equilibrium (D5)
* E1 L5.
* E2 swirling annular pipe: `annulus_3d` `r_in = 0.5`, `r_out = 1`, `lz = 4`, `nx = 48`, `ny = 12`, `nz = 24`, `nu = 0.1`, `rho = 1`. Exact solution: rigid rotation with `Omega = 2` plus uniform axial `W = 1`: `u = (-Omega y, Omega x, W)`, `p = p_a + rho Omega^2 (r^2 - <r^2>)/2` (all viscous terms vanish; `dp/dr = rho w^2/r` is the radial momentum balance). Walls: `moving_wall` with `velocity_value = (0,0,1)`, `wall_rotation_rate = 2`; inlet `mass_flow` uniform with `swirl_rate = 2`; outlet `radial_equilibrium_outlet`, `radial_reference = area_average`, `bc_pressure = 1.25` Pa (`rho Omega^2 <r^2>/2` with `<r^2> = (r_in^2 + r_out^2)/2 = 0.625`); `component_diagonal = corrected`. Pressure range over the outlet `rho Omega^2 (r_out^2 - r_in^2)/2 = 1.5` Pa. Checks: (a) outlet face pressure vs `1.25 + 2 (r_f^2 - 0.625)` max error `<= 3 %` of 1.5 Pa; (b) outlet tangential velocity `max|u_theta - Omega r| <= 3 %` of `Omega r_out = 2`; (c) mid-length (`1.5 <= z <= 2.5`) cell pressure vs the same formula `<= 3 %` of 1.5 Pa; (d) the outlet face pressures satisfy the discrete balance `p_{i+1} - p_i = (g_i + g_{i+1})(r_{i+1} - r_i)/2` for the final fields to 1e-8 rel to the range (relaxation weight 0.5 notwithstanding at convergence).

### F target mass flow (D6)
* F1 channel as A2 with the inlet fixed static pressure `pressure_value = 12` (kinematic = 12 Pa at rho = 1, zero-gradient velocity), outlet `target_mass_flow_outlet` with `target_mass_flow = 0.1` and initial `bc_pressure = 0`: after at most 6000 iterations, `bernoulli`: `|m_out - 0.1|/0.1 <= 1e-3`; `secant`: `<= 1e-4`.
* F2 developed slope at that flow rate as in A2: within `1 %` of `-1.2` Pa/m for both strategies.

### T transient (pimple)
* T1 the A2 channel with the mass-flow inlet, B1 outflow weights and a fixed-pressure outlet, `pimple`, `dt = 0.01`, 30 steps from rest: at every step `|rho sum phi - m|/m <= 1e-12` for the mass-flow patch, the B1 shares `0.25/0.75` to `1e-8` rel at the last step, and `|sum over all patches of rho sum phi|/m_in <= 1e-8` at every step after the first.

### R3 roles (D6b)
`turb::role` with `turbulence = auto` returns Inlet for `mass_flow` (`mass_flow_value > 0`) and `total_pressure_inlet`, Wall for none of the new kinds, ZeroGradient for all others; a k-epsilon case with a `mass_flow` inlet and `outflow`/`pressure_outlet` patches builds with exactly the wall patches being the `fixed`-zero ones (wall set asserted equal), and `initial.method = potential` with a non-`mass_flow` kind is a located error on `bc_kind` (V14).

### M conservation for every kind
For every converged case above (A2, A3, A6, B1, B2, C1, C2, C3, D1, D2, E2, F1; T1 at 1e-8) `|sum over all patches of rho sum phi| / throughput <= 1e-9`, throughput being the largest absolute patch mass flow.

### R regression
* R1 `crates/rcfd-api/tests/baseline_hashes.rs` unchanged and passing (bit-identical existing numerics; not re-recorded).
* R2 every new key is at its inactive default in an unmodified case, `bc_kind = none` reads the patch exactly as before.

## Known limitations stated now (status stays `partial` whatever the numbers)
No compressible, variable-density or coupled-solver support; no solid-pipe mesh; temperature of a recirculated stream is not
carried (F-051); the recirculation pair transfers mass only; `pressure_average` only on `pressure_outlet`; no time-varying
mass flow; the lagged dynamic-head conditions (total backflow, total-pressure inlet, vent, radial equilibrium) use a fixed
relaxation weight 0.5 and are only checked on the cases above.

## Addendum: results, deviations and misses (written after the implementation; no registered limit is changed)

Commits: library `e386536`, settings tree `1a36562`. Tests: `crates/rcfd-fv/tests/bc_catalogue.rs` (6) and
`crates/rcfd-api/tests/bc_catalogue.rs` (21, one ignored known miss).

### Measured against the registered limits (release build)

* L1-L6 all pass: L1 <= 2.2e-16, L3 3e-16, L4 9e-17, L5 solid body 1.5e-16 / free vortex 1.58e-5 (limit 1e-4) / area mean 0, L6 secant root exact to 2e-15.
* A1 worst relative flux error 1.39e-16 (limit 1e-12), all seven variants. A2 and A2b: slope -1.194030 Pa/m, error 0.498 % (limit 1 %), profile error 2.475e-3 of 1.5 U (limit 1.5e-2), patch imbalance 5.2e-13. A3: dp/dz -4.757139 against -4.776136, error 0.398 % (limit 1 %), 367 iterations. A4 5.2e-17 (mass_flow) and 1.1e-17 (total_pressure_inlet, D3). A5 4.8e-17. A6 m_out error 4.3e-13, m_ymax/m_out error 5.3e-15 (limit 1e-8). A7 exact to the printed digits. A8 every path asserted (V1 compressible, coupled and variable density; V2; V3; V4 five cases; V5; V6; V7 three cases; V8; V9; V10; V11 all 19 rows; V12; V14).
* B1 share error 4.4e-16, B2 3.3e-16 (limit 1e-10), B3 imbalance <= 1.4e-16 (limit 1e-12), B4 min u.n 2.6e-2 (limit >= 0).
* C1 outlet area-average p -1.4e-18 (limit 1e-9), slope error 0.498 %. C2 mean |u_x| = 1 to 2e-9 (limit 1e-5). C3 forward and reversed, K = 0, 2.5, 10: pressure error <= 7.3e-11 (limits 1e-8 / 1.2e-6 / 5e-6). C4 maximum field difference 0. C5 flux 0 and |u|max 0 (see the deviation below).
* D1 deviation 1.4e-9 (limit 1e-5), D2 magnitude 2.2e-10 (limit 1e-5) and angle 1.7e-10 rad (limit 1e-6).
* E2: outlet pressure error 0.408 % of 1.5 Pa (limit 3 %), outlet u_theta error 0.032 % of 2 (limit 3 %), mid-length pressure error 2.669 % (limit 3 %, the tightest margin), discrete balance 2.3e-15 (limit 1e-8).
* F1 bernoulli |m - 0.1|/0.1 = 1.4e-15 after 3329 iterations (limit 1e-3); F2 slope error 0.498 % (limit 1 %).
* T1 mass-flow patch error <= 5.6e-16 (limit 1e-12), B1 shares error 3.3e-16 (limit 1e-8), global imbalance <= 2.4e-14 (limit 1e-8).
* M conservation: the largest imbalance of any listed case is 1.2e-12 (limit 1e-9). R1: the baseline hash tests pass unchanged. R2 and R3 pass.

### Misses

* **F1 secant (registered limit 1e-4 in 6000 iterations): MISS**, kept as the ignored test `f1_f2_target_mass_flow_outlet_secant_known_miss` with the registered limits. The run diverges to NaN (the pressure leaves the physical range at iteration 23). Root cause, diagnosed from the per-iteration trace (m_out and p_out printed for 60 iterations): the secant slope `-(p - p_prev)/(m - m_prev)` is taken from successive iterates in which the flow is still spinning up, so the change in m is not a response to the change in p; at iteration 22 the pair (p up by 1.7, m down by 2.2e-4) gave a spurious slope of 7.6e3, which the registered clip to 1000 times the Bernoulli slope (2.4e4) turned into a pressure step of +530. The registered rule has no safeguard against a secant estimate that is contaminated by the transient. No parameter was changed to hide this. A guarded secant would be a new strategy needing its own registered check; it is not part of this delivery. The `bernoulli` option meets F1 and F2.

### Deviations from the ADR text (set-up and measurement, none touches a limit)

1. **Residual targets replaced by a settled-field criterion for the frictionless uniform-flow cases (C2, C3, D1, D2) and E2.** The ADR said converged means the residual targets 1e-12. In these cases the exact solution is uniform (or a rigid rotation) and the normalised residuals measure round-off: a control with only existing conditions (inlet velocity 1, static outlet 0, frictionless duct) reaches u = 1.000000 and p = 0 and still shows normalised residuals of 0.3 to 0.7 for 1500 iterations; E2 stalls at res_u = 4.4e-12 with the fields stationary. The tests therefore iterate until the largest change of ux, uy and p between successive iterations is at most 1e-10 for 20 iterations in a row (`settle`), and fail otherwise. The accuracy limits are unchanged.
2. **Pseudo-time stepping (`solver.pseudo_mode = "physical"`) in C2, C3, D1 and D2.** Plain SIMPLEC from rest diverges within ten iterations on a frictionless duct driven by two pressure ends (an existing-condition duct with a static pressure drop behaves the same: u = -366 after the first iteration), so the ADR cases are started with a pseudo time step of 0.2 (C2, D1, D2, C4) or 0.02 (C3). With 0.2 the C3 case with K = 2.5 diverged (first attempt, kept as an observation: the quadratic loss law lagged at relaxation 0.5 is not stable from the wild first iterations at that step); 0.02 converged for all K and both directions. The reversed K = 0 vent needs 23174 iterations to settle (a slow mode of a fixed static pressure at an inflow face with zero-gradient velocity), the other C3 cases 300 to 900.
3. **C5 is trivial.** With the face closed and 0.5 Pa at the closed patch against 0 at the other end nothing moves: flux 0 and |u|max 0 hold identically, and the case is already at its solution at the start. It checks that `prevent_reverse_flow` closes the face; it does not test a case in which the closure resists a driving pressure through a developing flow.
4. **Mesh and set-up choices left open by the ADR.** C2, C4, C5 and D1 use `box_2d` 20 x 4 with `lx = 5`, `ly = 1`. T1 uses `channel_2d` 50 x 10 (`lx = 10`) for the channel part and the 30 x 30 box for the B1 part, both with `pimple`, `dt = 0.01`, 30 steps, `end_time = 100` so the run does not stop early. A7 uses a 20 x 10 channel with `lx = 4`. C1 reads the outlet area-average pressure from `boundary_field("p")` (the applied face values weighted by area) instead of the report mechanism.
5. **V6 is raised by the schema range check** (`loss_coefficient` has a minimum of 0), at the registered path, not by `bc_cat::check`. The message is the generic range message.
6. **Turbulence set-up (found by A7, not anticipated by D6b).** The existing static turbulence checks read `velocity_value` and rejected a `mass_flow` inlet ("inlet velocity is zero") and could not derive the initial k. `turb::reference_speed` and the inlet check now use `bc_cat::nominal_speed` (`|m|/(rho A)` with the mesh, positive placeholder without it). A case whose only inlet is a total-pressure inlet, a sourced mass flow or a negative mass flow needs `turbulence.k_initial` and the initial epsilon / omega.
7. **Tests added beyond the list:** R2 (defaults inert) and D6b (potential initialisation with a mass-flow inlet gives the plug flow to 6.8e-13).

### Not delivered, or limited

Solid circular-pipe mesh (the annular duct was used for the Hagen-Poiseuille acceptance); compressible, variable-density and coupled solvers; temperature of a recirculated stream and per-boundary thermal conditions (F-051); time-varying mass flow; pressure averaging on kinds other than `pressure_outlet`; a guarded secant update; verification of the textbook closed forms against primary sources (see `docs/reference-data.md`, wave 16B).
