# ADR 0024: Wave 17B conjugate heat transfer: solid cell zones, coupled interfaces, thermal wall conditions (F-051)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new code was
compiled or run. The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the
assertion moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away.
A test set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes that were not
fixed here, results) may be appended at the end without touching registered limits.

Roadmap: F-051 (primary). F-215 (contact resistance, layered thin wall) is touched only through the checks E1 and E2 below;
F-215 is NOT claimed done on the strength of them. F-216 (per-region model sets), F-212 (solidification) and F-213 (Joule
heating) are not attempted in this wave. F-050 (energy equation), F-111 (properties), F-114 (non-conformal join) are used, not
changed in scope.
Clean-room: published literature only; nothing from vendor material, vendor installs or vendor manuals. No vendor
names in the repository. Closed forms below are derived here and need no reference; two textbook facts used only as a sanity
cross-check are marked "(from memory, unverified)".

## Audit of what exists before this wave

* `energy.rs`: `EnergySolver` solves `div(phi T) - div(alpha grad T) = S` with a single `alpha = k/(rho cp)` (or `alpha_face`
  per face), implicit Euler / BDF2 / CN in time through `scalar::sweep_ext`. Boundary conditions through `Bc`: `FixedValue`,
  `ZeroGradient`, `FixedGradient` (the heat-flux condition is `FixedGradient(q/k)` with the single `physics.conductivity`).
* `boundaries.<patch>.temperature` = `zero_gradient | fixed | heat_flux` (+ expressions and profile tables, F-104). There is NO
  convection (heat-transfer coefficient with free-stream temperature), NO thin-wall resistance and NO radiation-to-ambient
  condition. These F-200 leftovers are in scope here.
* `heat_flow` reports use one conductivity; `zone_sources` (ADR 0019) add volumetric heat in a cell zone divided by the fluid
  `physics.rho * physics.cp`.
* Mesh: `Mesh::cell_zones`, imported zones, `mesh.generator = channel_2d_interface` (non-conformal join with ordinary internal
  faces; the join keeps cell zones by name, drops face zones). `sources::zone_cells` resolves `all | box | cell_zone | register`.
* No solid region exists: every cell is fluid with `physics.conductivity`, `physics.rho`, `physics.cp`.

## Decisions

### D1 Formulation: conductive form normalised by the reference capacity

Per cell `c` the energy equation is `(rho cp)_c dT/dt + div(rho cp phi T) = div(k grad T) + S`. Dividing the WHOLE equation by
one constant, the fluid `(rho cp)_ref = physics.rho * physics.cp`, gives the form the existing machinery already solves:

```
w_c dT/dt + div(phi T) = div(gamma grad T) + S / (rho cp)_ref ,   gamma_f = k_f / (rho cp)_ref ,  w_c = (rho cp)_c / (rho cp)_ref
```

(`phi` is the volumetric flux of the fluid, zero in solids.) Dividing every cell by its own `rho cp` (the diffusivity form) is
NOT used: it breaks heat-flux continuity across a capacity jump. Consequences: steady problems need only `gamma_f` (per face)
and the masked flux; transient problems additionally need the capacity weight `w_c` multiplying the cell volume in the time
derivative. `rcfd-fv` gets an optional `ddt_weight: Option<Vec<f64>>` (EnergySolver field, `SweepExtras` field); `None` takes the
existing code path unchanged, so every existing case is bit-identical (baseline hashes, checked on HEAD before the work and
after every slice). `zone_sources` need no change (their W/m^3 are already divided by the fluid `rho cp`).

### D2 Face conductivity and contact resistance

Internal face with owner `o`, neighbour `n`, interpolation weight `w` of the owner (`weights[f]`, the existing geometry): the
distance-weighted harmonic mean

```
1 / k_f = (1 - w) / k_o + w / k_n          (d_o/(d_o+d_n) = 1 - w)
```

so the series resistance of the two half cells is reproduced exactly (a plain harmonic mean is not exact for unequal cells).
A contact (or fouling-layer) resistance `R''` [m^2 K/W] on a face adds in series: with `Delta = (S.d)/|S|` the normal distance
between the cell centres, `gamma_eff = 1 / (1/gamma_f + R''/Delta)` (then `gamma_eff |S|^2/(S.d) (T_n - T_o)` is the heat flux
`(T_n - T_o) / (Delta/k_f + R'')` times the area). Boundary faces take the conductivity of the owner cell. The non-orthogonal
correction of `laplacian` is kept as is (it uses the face `gamma`). Non-conformal interfaces need nothing extra: the join
produces ordinary internal faces, flux continuity across them holds by construction (one flux per face, added to the owner row
and subtracted from the neighbour row).

### D3 Thermal wall conditions (`ConvectiveWall`, `rcfd-fv`)

Heat flows out of the domain through the face with `q_out = (T_P - T_inf_chain)` over a series chain from the cell centre:
`dn/k` (half cell, owner conductivity), the wall resistance `R_w = sum t_i/k_i` (thin layers, steady resistance only, no
thermal capacity), then the external surface. At the external surface `q_ext(T_s) = h (T_s - T_inf) + eps sigma (T_s^4 - T_a^4)`
(`sigma = 2 pi^5 k_B^4 / (15 h_P^3 c^2)` evaluated from the exact SI constants in the code, not typed in). With `T_s = T_P - q (dn/k + R_w)`
the scalar equation `g(q) = q - q_ext(T_P - q (dn/k + R_w)) = 0` is monotone and solved by Newton (convergence to 1e-15
relative, bounded iterations). The coefficients handed to the finite-volume operators are the exact linearisation about that
solution: `q ~ q* + U (T_P - T_P*)`, `U = q_ext'(T_s*) / (1 + q_ext'(T_s*) (dn/k + R_w))`, gradient `= -q/k`, face value `T_P - q dn/k`
(`FaceCoeffs` form `c = -U/k`, `d = -(q* - U T_P*)/k`, `a = 1 + dn c`, `b = dn d`). The fixed point of the outer iteration is the
exact solution of the discrete nonlinear boundary equation, for any `eps`. Pure convection is `eps = 0`; pure radiation
is `h = 0`; a pure thin-wall resistance to a fixed external temperature is `h` very large only if asked for explicitly
(not provided; use convection with the real `h`).

Settings (additive; every new key has an explicit UNSET sentinel and a located error when it is needed and unset; none shadows
another value): `boundaries.<patch>.temperature = "convection"` with `heat_transfer_coefficient` [W/m^2/K, default `-1` =
unset, `0` allowed], `free_stream_temperature` [K, default `-1` = unset, required when `h > 0`], `wall_thickness` [m, list,
default empty = no thin wall], `wall_conductivity` [W/m/K, list of the same length], `emissivity` [default 0 = none],
`radiation_temperature` [K, default `-1` = unset, required when `emissivity > 0`]. Constants only (no expression forms for
these keys: a named gap). `fixed` and `heat_flux` keep working; `heat_flux` on a patch next to a solid uses the conductivity of
the owner cell of each face.

### D4 Solid regions in the settings tree

```
[solids.<name>]          zone (unset | all | box | cell_zone | register, default "unset" = error), box_min, box_max, cell_zone, register
                         conductivity [W/m/K], density [kg/m^3], specific_heat [J/kg/K]   (default -1 = unset = error each)
                         initial_temperature [K]   (default -1 = unset: use initial.temperature)
[thermal_interfaces.<n>] solid_a, solid_b (a solid name or "fluid"; default "" = error), resistance [m^2 K/W] (default -1 = unset = error, 0 allowed)
solver.frozen_flow = false   (additive; see D5)
```

Volumetric heat generation in a solid is a `zone_sources` entry on the temperature (no new keys). Solid zones must not overlap;
"fluid" is the set of cells in no solid. Constant isotropic properties only; temperature-dependent and orthotropic conductivity
are not provided (F-220). A contact resistance acts on the internal faces between exactly the two named materials.
Heat generation, `heat_flow` reports and `heat_flux`/`convection` boundary conditions use the per-face (owner cell) conductivity.

### D5 Flow with solid zones: prescribed (frozen) velocity only

Zeroing the flux through the solid cells belongs to the pressure-velocity core (another agent's files this wave) and is NOT
done here; a penalty (porous sink) was examined and rejected: the Rhie-Chow face interpolation of the inverse diagonal across a
fluid/solid face does not vanish, so the solid pressure equalises and a bypass leak of the order of the local through-flow
remains. Instead `solids` require `solver.frozen_flow = true`: momentum and pressure are not solved; the face flux is
`phi_f = u(x_f) . S_f` from `initial.velocity` and `initial.velocity_expression_x/y/z` evaluated at the FACE centres (boundary
faces of `wall` patches get zero flux; all other boundary faces take the expression), then the energy equation is advanced (steady
iteration or time steps, any of piso/pimple/simple/simplec as the nominal algorithm). It is a located error when the flux does
not vanish (relative to the largest face flux) on any face touching a solid cell (`initial.velocity_expression_x`), when the
largest cell imbalance exceeds `1e-6` of the largest face flux (`initial.velocity`), and for solids combined with: `properties.*`
laws, radiation, turbulence, variable density / compressible, `components`, `coupled`, buoyancy (`energy.buoyancy`),
`zone_sources` of kind `fixed_value`, rheology and user functions (each located). `frozen_flow = true` without solids is allowed
(energy transport on a prescribed field) and documented. NAMED GAP behind `partial`: a SOLVED flow coupled to the solid
(buoyancy-driven or pressure-driven) is not delivered.

### D6 Reports

`heat_flow` on a patch whose faces lie on solid or fluid cells sums `-k_face dT/dn |S|` with the owner conductivity per face.
The balance helper `rcfd_fv::cht` returns, per cell group (fluid, each solid) the sum of the discrete face heat flows crossing its
boundary plus its source, which is zero for a converged steady solution.

## Registered checks

Common set-up: library tests (`rcfd-fv/tests/cht.rs`, `rcfd-fv/tests/thermal_bc.rs`) use `rcfd_mesh::gen` meshes; case tests
(`rcfd-api/tests/cht_case.rs`) go through the settings tree. Linear solves of the steady and balance checks use
`tol_rel = 1e-13`, `tol_abs = 1e-300`, `max_iter = 5000`, relaxation 1 (the energy equation is linear for the frozen flux and
the no-radiation boundary conditions, so a few sweeps reach round-off; radiation checks iterate the boundary linearisation to a
change below 1e-13 in at most 400 sweeps). "dT" is the temperature difference across the problem. Temperature errors are
absolute and divided by dT unless stated.

### A. Thermal wall conditions (library, 1-D slab on `box_2d`, one cell across, adiabatic sides, `phi = 0`)

* A1 slab `L = 0.1`, `k = 2`, 20 cells; left: convection `h = 50`, `T_inf = 400`; right: fixed 300. Exact (series): `q = 100 /
  (1/50 + 0.1/2) = 1428.5714...` W/m^2; the profile is linear and the finite-volume solution is exact for it. Limit: max cell
  temperature error / dT <= 1e-10 and `|heat_flow(left) - q A|/(q A) <= 1e-10`.
* A2 thin wall: left chain h = 25, T_inf = 350, layers `(t, k) = (0.002, 0.5), (0.001, 45)`, slab `L = 0.05`, `k = 1.5`, right fixed
  290. `q = 60 / (1/25 + 0.002/0.5 + 0.001/45 + 0.05/1.5)`. Limit as A1 (1e-10).
* A3 radiation: slab `L = 0.05`, `k = 15`, right fixed 800 K; left convection `h = 10`, `T_inf = 300` plus `eps = 0.8` radiating
  to `T_a = 300`. The surface temperature `T_s` solves `k (800 - T_s)/L = h (T_s - 300) + eps sigma (T_s^4 - 300^4)` (bisection to 1e-14 in
  the test, independent of the code under test). A3b: `h = 0` (pure radiation, `T_inf` unset allowed). Limit: temperature error /
  dT <= 1e-9, heat-flow relative error <= 1e-9.
* A4 energy balance of A1-A3: sum of all patch heat flows (conductive, no source) <= 1e-12 of the left flow.
* A5 fin (convection on a lateral surface, 2-D resolved): half thickness `b = 0.005` (symmetry at `ymin`, convection `h = 20`,
  `T_inf = 300` at `ymax`), length `L = 0.1`, `k = 100`, base `xmin` fixed 400, tip `xmax` adiabatic. The 1-D fin of thickness `2b`:
  `m = sqrt(h P / (k A))` with `P = 2` and `A = 2b` per unit depth, so `m = sqrt(h/(k b)) = sqrt(40)`; heat rate through the half
  base `Q_half = k b m theta_b tanh(m L)`, `theta_b = 100` (insulated-tip fin; the standard result is re-derived here from
  `theta'' = m^2 theta`, `theta'(L) = 0`). The transverse Biot number `h b/k = 1e-3`; the 1-D model neglects it, an error
  of order Bi/3 = 3e-4 relative on `m^2`. Limit: mesh 40 x 10: `|Q_2D - Q_1D|/Q_1D <= 1e-2`; refinement 80 x 20 changes
  `Q_2D` by <= 2e-3 relative.
* A6 transient lumped slab: `L = 0.01` (both faces convect, symmetric: half slab with symmetry at `xmin`), `k = 50`,
  `rho cp = 4e6`, `h = 100`, `T_inf = 300`, `T_0 = 400`. `Bi = h (L/2)/k = 0.01`. Lumped: `T - T_inf = 100 exp(-t/tau)`,
  `tau = rho cp (L/2)/h = 200 s`. The first Heisler eigenvalue obeys `zeta tan zeta = Bi`, `zeta^2 = Bi (1 - Bi/3 + ...)`, so the
  lumped rate is off by Bi/3 = 0.33 percent, giving a relative error `e^{Bi/3} - 1 <= 0.34 percent` of the excess temperature at
  `t = tau`; implicit Euler with `dt = tau/2000` adds <= 0.03 percent. Limit: mean slab temperature excess at `t = tau` and
  `2 tau` within 1 percent of the lumped value (20 cells).

### B. Conforming composite walls (library)

* B1 three layers in series on `box_2d` 60 x 1, `L = 1`: layer thicknesses 0.2, 0.3, 0.5 (interfaces on faces: 12 and 30 cells),
  `k = (1, 10, 0.1)`, `rho cp = (3, 1, 7)`, left fixed 400 K, right convection `h = 5`, `T_inf = 300`. `q = 100/(0.2 + 0.03 + 5 + 0.2)`.
  The distance-weighted harmonic mean makes the solution exact for the piecewise-linear profile. Limit: max cell temperature
  error / dT <= 1e-10. Also on a graded mesh (symmetric grading 3:1 in x, interfaces at the nearest face to 0.2 and 0.5 are
  moved to the faces of the mesh and the analytic solution uses the actual interface positions): same limit 1e-10.
* B2 interface heat-flux continuity: the discrete conductive heat flow through each of the two interfaces equals the boundary
  heat flow at the left wall, relative difference <= 1e-10.
* B3 energy balance across zones: B1 geometry with a volumetric source `Q = 5e4` W/m^3 in the middle layer; per zone (the three
  layers) the sum of the discrete face heat flows out through the zone boundary equals the zone source, relative <= 1e-9 of
  `|Q V|`; globally boundary outflow equals total source <= 1e-9. Repeated on a 2-D mesh warped by `x -> x + 0.05 sin(2 pi y)` (non-orthogonal faces)
  with convection on three sides: same limit 1e-9.
* B4 transient contact of two semi-infinite solids (exercises the capacity weight): left `k = 2`, `rho cp = 1`, `T_1 = 400`;
  right `k = 0.5`, `rho cp = 2`, `T_2 = 300`; effusivities `e = sqrt(k rho cp)`: `e_1 = sqrt 2`, `e_2 = 1`. The interface
  temperature is constant, `T_i = (e_1 T_1 + e_2 T_2)/(e_1 + e_2)` (derivation: each side obeys the erfc solution of the heat equation,
  `T = T_i + (T_0 - T_i) erf(|x|/(2 sqrt(alpha t)))`, and the flux condition `e_1 (T_1 - T_i) = e_2 (T_i - T_2)` fixes `T_i`),
  right `T = T_2 + (T_i - T_2) erfc(x/(2 sqrt(alpha_2 t)))`, left `T = T_1 - (T_1 - T_i) erfc(|x|/(2 sqrt(alpha_1 t)))`.
  Domain `[-8, 8]`, `t_end = 0.2` (then `erfc(8/(2 sqrt(2 * 0.2))) = erfc(6.3) < 1e-17`, the far ends stay at their initial values), cells N =
  320, 640, 1280 (uniform, interface at a face), BDF2 with 40, 80, 160 steps (Euler first step). Limits: max |T - T_exact| / dT on the
  finest grid <= 1e-3; observed order between successive grids (from the max error) >= 1.5. These numbers are engineering bounds fixed
  before the run, not derived.
* B5 adiabatic enthalpy conservation: B1 geometry, all walls adiabatic, non-uniform initial temperature, 50 steps of Euler and of
  BDF2: `sum rho cp_c V_c T_c` constant to relative 1e-12.

### C. Non-conformal interface (library, join of two channel blocks)

* C1 composite wall through `join_non_conformal`: blocks `[0, 0.5]` with 10 x 20 cells and `[0.5, 1]` with 13 x 27 (non-matching
  across), inlet fixed 400 K, outlet fixed 300 K, walls adiabatic, `k_left = 1`, `k_right = 5` (right block = a solid zone chosen by
  `box`). Exact 1-D: `q = 100/(0.5/1 + 0.5/5)`. Limits: flux through the interface (sum over overlap faces) equals the inlet and outlet
  flows, relative <= 1e-9 (conservation by construction); max cell temperature error / dT <= 1e-2 (the explicit non-orthogonal
  correction with a least-squares gradient is not exact across a conductivity kink; the bound is an engineering limit fixed before the
  run); and the error on the 20 x 40 / 26 x 54 refinement is smaller by a factor >= 1.5.
* C2 the same through the settings tree with `mesh.generator = "channel_2d_interface"` and a `box` solid: same limits.

### D. Fluid-solid duct with a closed-form Nusselt number (fully developed, prescribed Poiseuille flux)

Half channel, fluid `0 <= y <= H`, symmetry at `y = 0`, conducting solid `H <= y <= H + t`, uniform heat flux `q` into the domain at
`y = H + t`, axial coordinate `x`. Poiseuille velocity `u = (3/2) U (1 - eta^2)`, `eta = y/H`, mean `U`. Fully developed
thermal field `T = A x + theta(y)` solves the FULL 2-D equation (so Peclet number does not matter, axial conduction is exactly zero
since `d^2T/dx^2 = 0`): fluid `rho cp u A = k_f theta''`; solid `theta_s'' = 0`. Energy balance of the half channel: `q = rho cp U H A`,
so `A = q/(rho cp U H)` and `theta'' = beta (1 - y^2/H^2)`, `beta = 3 q/(2 k_f H)`. With `theta'(0) = 0`:
`theta'(y) = beta (y - y^3/(3H^2))`, `theta'(H) = 2 beta H/3 = q/k_f` (heat-flux continuity at the interface, an independent
consistency check of `A`). `theta(y) = theta_0 + beta H^2 (eta^2/2 - eta^4/12)`; solid `theta_s = theta(H) + (q/k_s)(y - H)`.
Bulk temperature `theta_b = theta_0 + beta H^2 * (3/2) int_0^1 (1 - eta^2)(eta^2/2 - eta^4/12) d eta = theta_0 + (13/140) beta H^2`
(the integral is `1/6 - 1/60 - 1/10 + 1/84 = 13/210`, times 3/2). Wall minus bulk: `theta(H) - theta_b = beta H^2 (5/12 - 13/140) =
beta H^2 (34/105) = (17/35) q H/k_f`. Therefore `h = q/(theta(H) - theta_b) = 35 k_f/(17 H)` and, with hydraulic diameter `D_h = 4H`,
`Nu = h D_h/k_f = 140/17 = 8.2353` (this reproduces the textbook parallel-plate value, from memory, unverified; it is derived here).
Outer surface minus bulk: `Delta T_tot = q (17 H/(35 k_f) + t/k_s)`.
Numerical set-up: `H = 1`, `t = 0.5`, `k_f = 1`, `k_s = 10`, `U = 1`, `rho cp` fluid 1 (solid 4), `q = 1`, `L_x = 2`, 8 cells in x, `n_f` = 16, 32, 64 cells across the
fluid and `n_f/2` across the solid (interface on a face), upwind convection (exact for a linear profile in x with `u(y)`), inlet and outlet Dirichlet
profiles equal to the exact `T` (per-face), flux `phi_f = int u dy * depth` over each face EXACT in the library test (so the discrete
mass flow is `U H` exactly) and by midpoint sampling at the cell centres in the case test.
* D1 (acceptance): relative error of `Delta T_tot` (outer-surface face mean minus flux-weighted bulk mean at mid-length) at
  `n_f = 16` <= 1 percent. Hence the acceptance text "1D slab+fluid analytic CHT test within 1 percent" is met by this test; the
  slab-only checks A/B are essentially composite conduction and are stated as such.
* D2 observed order from the errors at 16, 32, 64: both successive orders >= 1.8.
* D3 discrete heat balance (boundary heat flows including the convective flux at inlet and outlet, plus the flux `q Lx` in): relative
  <= 1e-9 of `q L_x`; flux through the fluid-solid interface equals `q` within the solid at `L_x` scale, relative <= 1e-3 at `n_f = 32`
  (the discrete interface flux is only second-order close to `q` since inlet/outlet are Dirichlet).
* D4 the case test (settings tree, `solver.frozen_flow`, `n_f = 32`, midpoint-sampled flux): relative error of `Delta T_tot` <= 1 percent.

### E. Contact resistance and layered thin wall (F-215 evidence only)

* E1 two solids `k = 1, 10`, `L = 0.05` each, contact resistance `R'' = 0.02` m^2 K/W between them, ends fixed 400 K and 300 K: cell
  temperatures vs the exact piecewise-linear solution <= 1e-10 dT; the extrapolated temperature jump equals `q R''` within 1e-10 relative.
* E2 layered thin wall (A2) versus the same layers resolved as three explicit solid zones plus a convection surface: the temperature
  in the main slab agrees <= 1e-10 dT (the resolved wall layers have one cell each, their steady resistance equals `t/k`).

### F. Settings tree, validation, reports, regression

* F1 composite wall B1 through the settings tree (`box` solids, `frozen_flow` with zero velocity): cell temperatures <= 1e-9 dT.
* F2 transient contact B4 through the tree at N = 640: max error / dT <= 1e-3 (same bound as B4).
* F3 `heat_flow` reports on a solid patch and a fluid patch match `q A` (B1) to <= 1e-9 relative.
* F4 located validation errors, each asserted by path: `solids.s.zone` (unset), `solids.s.conductivity`/`density`/`specific_heat` (unset or
  not positive), overlapping solids (`solids.b.zone`), empty zone, `thermal_interfaces.i.solid_a` (unknown), `thermal_interfaces.i.resistance`
  (unset / negative), `solids` without `energy.enabled`, `solids` without `solver.frozen_flow`, a solid with `radiation`/`properties` laws,
  `boundaries.p.heat_transfer_coefficient` unset, `free_stream_temperature` unset with `h > 0`, `radiation_temperature` unset with `emissivity > 0`,
  `wall_thickness`/`wall_conductivity` length mismatch or non-positive entries, thin-wall keys on a non-convection patch, non-zero flux on a face touching a
  solid (`initial.velocity_expression_x`).
* F5 `crates/rcfd-api/tests/baseline_hashes.rs` passes unchanged and unmodified on HEAD (checked before the work) and after each
  slice; existing library tests of `rcfd-fv` pass.

## Not delivered (expected; reconciled with the results at the end)

Solved (pressure/buoyancy-driven) flow coupled to solids; temperature-dependent and orthotropic solid conductivity (F-220);
shell/thin-wall thermal capacity (steady resistance only); expression forms of the new boundary keys; radiation between surfaces
(only radiation to an ambient temperature); per-region model sets (F-216), solidification (F-212), Joule heating (F-213); curved or
partially overlapping non-conformal interfaces (F-114 limits).

## Addendum: results, deviations and what was not delivered

Registered limits were not changed. Library checks are in `crates/rcfd-fv/tests/{cht,thermal_bc}.rs` (commit 7491a22), case checks in
`crates/rcfd-api/tests/cht_case.rs`. "Measured" values are printed by the tests (`--nocapture`).

### Results (measured against the registered limit)

| Check | Limit | Measured |
| --- | --- | --- |
| A1 convection slab, temperature / heat flow | 1e-10 dT | 1.6e-14 / 3.2e-14 (case level 2.5e-15) |
| A2 thin wall | 1e-10 | 1.4e-14 |
| A3 convection + radiation (surface 740.950376 K); A3b pure radiation (752.690728 K) | 1e-9 | 4.6e-14; 2.2e-14 (case level A3b 8.4e-15) |
| A4 balance | 1e-12 | met |
| A5 fin (2-D vs 1-D formula; refinement change) | 1e-2; 2e-3 | 3.34e-4; 3.1e-5 |
| A6 lumped slab at tau / 2 tau | 1e-2 | 3.58e-3; 7.18e-3 |
| B1 three-layer wall uniform / graded | 1e-10 | 3.4e-13; 4.5e-13 |
| B2 interface flux continuity | 1e-10 relative | 1.2e-12; 9.2e-12 |
| B3 zone balances (1-D, global; warped mesh) | 1e-9 | 1.3e-13, 1.4e-13; 6.8e-15 |
| B4 transient contact errors (320, 640, 1280), orders | 1e-3 finest; order >= 1.5 | 6.57e-4, 1.64e-4, 4.10e-5; 2.000, 2.002 |
| B5 enthalpy drift Euler / BDF2 | 1e-12 | 3.5e-13; 4.9e-13 |
| C1 non-conformal (library): error, ratio, conservation | 1e-2; 1.5; 1e-9 | 3.2e-5 (finest), 1.98, 8.6e-14 |
| C2 same through the tree | same | 3.2e-5, 1.98, 2.3e-13 |
| D1 duct n_f = 16 (central scheme, see deviation 3) | 1e-2 | 1.5e-4 |
| D2 orders (16, 32, 64) | 1.8 | 1.9989, 1.9997 |
| D3 balance / interface flux (n_f = 32) | 1e-9 / 1e-3 | 1.2e-11 / 5.3e-5 |
| D4 case-level duct, n_f = 32, midpoint flux | 1e-2 | 1.2e-5 |
| E1 contact resistance temperature / jump | 1e-10 | 1.4e-14 / 2.3e-14 |
| E2 thin wall equals resolved layers | 1e-10 | 1.9e-14 |
| F1 composite wall through the tree | 1e-9 dT | 2.8e-13 |
| F2 transient contact through the tree (N = 640) | 1e-3 | 1.64e-4 |
| F3 heat_flow reports (solid and fluid patch) | 1e-9 | 6.7e-12; 3.5e-13 |
| F4 located validation errors (19 cases) | each asserted by path | all met |
| F5 baseline hashes | unchanged | `baseline_hashes` passes unchanged (no hash re-recorded) |

### Deviations from the registered text (disclosed, none loosens a limit)

1. **Steady stop criterion.** The registered library stop "change below 1e-13 in at most 400 sweeps" is unattainable: every sweep restarts
   the linear solve with `tol_rel = 1e-13`, so the change stagnates at 1e-12 to 1e-11 (noise of order `tol_rel T`). The library harness
   stops at 1e-11 (sanity assertion 1e-10). The accuracy limits above are unchanged.
2. **A3 set-up defect.** The first A3/A3b tests asserted the heat flow with the sign of an inflow. The tests `a3_radiation_with_convection`
   and `a3b_pure_radiation` are kept as ignored records; the corrected `*_v2` tests pass.
3. **D set-up defect (diagnosed, not tuned).** The ADR sentence "upwind convection (exact for a linear profile in x)" is wrong at the
   Dirichlet inlet face: there the boundary face value is carried, while interior faces carry the upstream CELL value, which is an O(dx)
   discrepancy independent of `n_f`. With upwind the errors were 8.4e-3, 8.36e-3, 8.33e-3 (orders 0.02, 0.005; D2 failed), D3 interface
   flux 2.95e-3 (> 1e-3); D1 passed only by coincidence. The corrected tests `*_central` use central convection (second order, exact
   for a linear profile) and meet all limits; the upwind tests are kept as ignored records. The case-level D4 uses `solver.convection =
   "central"`. Energy conservation was fine with either scheme (7e-12 with upwind).
4. **Key name.** The registered `emissivity` key of the convection condition collides with the existing P1-radiation `boundaries.*.emissivity`
   (default 1.0). It is `wall_emissivity` (default 0 = none) so that no existing value changes meaning.
5. **Which patches carry no prescribed flux.** The ADR text says "wall patches"; `box_2d` labels every patch a wall, so the rule is by the
   velocity condition instead: `symmetry`, any wall type, or `fixed` with zero velocity and no expression or user function.
6. **Buoyancy conflict** is raised only when `energy.buoyancy` (default true) can act, i.e. `physics.thermal_expansion` and `physics.gravity` are
   non-zero, not on the bare default.
7. **Additional unregistered checks** (supplementary evidence, same limits as the library counterparts): `convection_wall_through_the_tree`
   (A1 and A3b through the settings tree) and the ignored upwind records.

### Not delivered (final)

* A SOLVED flow coupled to solid zones (pressure-driven or buoyancy-driven). `solids` require `solver.frozen_flow = true`. The cell flux
  through solids cannot be removed without changing the pressure-velocity core (wave 17A's files); a penalty approach was examined and
  rejected (Rhie-Chow face interpolation across fluid/solid faces leaves a bypass leak). This is the principal gap behind `partial`.
* Temperature-dependent and orthotropic solid conductivity (F-220); thermal capacity of thin wall layers (steady resistance only);
  expression forms of the new boundary keys; surface-to-surface radiation (only radiation to an ambient temperature); solids with
  turbulence, radiation, `properties` laws, compressible flow (each a located error).
* F-216 (per-region model sets), F-212 (solidification) and F-213 (Joule heating as a dedicated feature; a `zone_sources` source in a solid
  works) were not attempted. F-215 (interface physics) is NOT marked done: only the contact-resistance and thin-wall evidence (E1, E2) exists.
* Curved or partially overlapping non-conformal interfaces (F-114 limits); an unequal-area non-conformal solid-solid check beyond C1/C2.
