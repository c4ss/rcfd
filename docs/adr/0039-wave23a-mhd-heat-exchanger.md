# ADR 0039: Wave 23A, Lorentz-force MHD coupling (F-214) and heat-exchanger macro models (F-217)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave existed.
Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an
ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect gets a
NEW corrected test and the old one stays as a record. Addenda (exact numbers, file names, deviations) may be appended at the end
without touching a registered limit.

Scope. F-214 (size S) and F-217 (size M) are `todo`. Each gets a vertical slice and stops at `partial` unless every acceptance point
is met; the gaps are named at the end. Clean-room: published literature and first principles only; no vendor code, text, data or
names; nothing from vendor material. Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`) stay bit-identical and
are not re-recorded (3 passed, 1 ignored on HEAD 7cf948f before any change of this wave, run with `CARGO_TARGET_DIR=<target dir>`).
Schema additions are additive with explicit unset sentinels. Parallel waves 23B and 23C work in the same tree: shared files
(`schema.rs`, `sim.rs`, `lib.rs`, `case.rs`) get small targeted edits and are committed through a temporary index with only this
wave's hunks.

## Audit of what exists

* Momentum path. `sources.<n>` (ADR 0019, 0036) is evaluated by `Sources::evaluate(geom, u)` once per SIMPLE/SIMPLEC iteration and
  returns an explicit `BodyForce` (kinematic acceleration per cell) and an implicit isotropic sink [1/s]; `sim.rs` adds the force to
  `base_body` / buoyancy / user-function force and hands the sink to `SimpleSolver::sink`. `sources::check` rejects every algorithm
  except `simple` and `simplec`; the coupled solver refuses `sources` (sim.rs); PISO/PIMPLE have no sink and no source hook (the
  21A finding is confirmed). So every momentum-type physics of this wave is steady SIMPLE/SIMPLEC only; transient MHD is a named gap.
* Pressure jumps. Fan and porous jump (ADR 0012) are a `FaceJump` (explicit rise plus resistance per internal face) on the
  downstream plane of a zone, built in `rcfd-api/src/lumped.rs`; `network_outlet` is a lagged fixed patch pressure.
  `bc_kind = "vent"` (ADR 0022 D3, check C3) is a vent boundary with a loss coefficient and already exists.
* Electric potential (ADR 0035 D2): `rcfd_fv::electric::solve` (potential with the shared sweep, `source: None`, insulating cells
  pinned), `ElectricState` in `rcfd-api` (gated on `solver.frozen_flow` and `energy.enabled`). `joule_heat` is `sigma |grad phi|^2`.
* Lumped heat exchanger (ADR 0012): `heat_exchanger` with a fixed duty or a constant effectiveness `eps C_min (T_sec - T_in)`.
  No NTU, no arrangement, no loss closure, no dual cell, no fan boundary.

Reuse decision (answers the instruction not to build a third mechanism). The Lorentz force is a new `type = "lorentz"` of
`sources.<n>` and goes through exactly the existing explicit-force plus implicit-sink path. The exchanger and fan pressure drops use
the existing `FaceJump` plane and the existing lagged patch-pressure mechanism (`network_outlet`), extended with a different law.
No new solver hook, no new algorithm support.

## Decisions

### D1 F-214 physics (library `rcfd-fv/src/lorentz.rs`, new)

Low magnetic Reynolds number approximation: the applied field `B` (given, not solved) is not changed by the flow; the induced field
is neglected. Ohm's law `J = sigma (E + u x B)`, `E = E_ext - grad phi`, steady charge conservation `div J = 0`. The body force on the
fluid is `J x B`, entering the momentum equation as the kinematic acceleration `J x B / rho`.

Uniform `B` and uniform `sigma` inside the zone (`sigma = 0` outside: an insulator, no current, no force). With
`(u x B) x B = (u.B) B - B^2 u` the force is `a = sigma (E x B)/rho + sigma (u.B) B/rho - s u`, `s = sigma B^2/rho`. Handed to the
solver as `BodyForce = J_c x B / rho + s u*` (explicit, `u*` the current iterate) and `sink = s` (implicit, all three components).
At convergence `a = J x B / rho` exactly (the `s u` terms cancel); the split removes the Hartmann stiffness (an explicit treatment is
unstable when `Ha^2` is much larger than 1). The parallel component cancels as well (`J x B` has no component along `B`).

Charge conservation, `potential` setting:

* `none` (default): `grad phi = 0` in every cell, `J_c = sigma (E_ext + u_c x B)`. Physically this is the fully developed / locally
  short-circuited or imposed-field approximation (valid when `div (u x B) = 0` and the walls impose no net return current, e.g. the
  Hartmann channel with `B` normal to the plates; it is NOT valid for a field with a component along the streamwise direction of a
  closed duct).
* `solve`: `-div(sigma_f grad phi) = S`, `S_c = -(1/V_c) sum_f sigma_f (u x B)_f . S_f`, solved with the 21A sweep
  (`rcfd_fv::electric`, new `solve_with_source`; the old `solve` calls it with `None`, bit-identical). `(u x B)_f` is the linear
  interpolation of the cell velocities on internal faces and the boundary velocity on boundary faces. Boundary conditions use the
  existing `boundaries.<p>.electric` keys: `insulated` (zero normal current: gradient `d phi/dn = (u_b x B).n`), `potential`
  (`electric_potential`), `current` (`electric_current` total into the domain: gradient `j/sigma + (u_b x B).n`). If no patch has a
  fixed potential, cell 0 is pinned to 0 (potential is defined up to a constant; the source is integrable by construction). The face
  current is `sigma_f (E_ext - grad phi + u x B)_f . S_f`; the cell current is reconstructed from it,
  `J_c = (1/V) sum_f (J_f . S_f)(x_f - x_c)` (exact for a uniform `J`), for the directions in which the mesh has faces; the
  direction normal to the `empty` faces of a 2-D mesh takes `sigma (E_ext + u x B)` of the cell (no potential gradient exists there).
  The reconstruction makes `J = 0` exactly when every face current vanishes (the null check A5).
* Joule heating with a magnetic field (`J^2/sigma`) is not coupled to the energy equation in this wave (named gap); the 21A
  `sigma |grad phi|^2` is wrong when `u x B` is present, so combining `electric.enabled` with a `lorentz` source is a located error.

`wall_pressure_trapezoid` (SIMPLE wall-pressure construction for body forces): set to `true` for a Lorentz source, like the
acceleration and actuator-disk sources, because the force can have a wall-normal component. The Hartmann check has a force
parallel to the walls, where the construction has no effect.

Settings (additive; `sources.<n>.type = "lorentz"`; `zone` keys as for the other sources):
`magnetic_field` [T] (3-vector, required, non-zero), `conductivity` [S/m] (`-1` = unset, required, > 0), `applied_electric_field`
[V/m] (3-vector, empty = 0), `potential` = `none | solve`. Output fields `lorentz_force_x/y/z` [N/m^3] and `current_density_x/y/z`
[A/m^2] (cell values of the last evaluation). Located errors: missing / zero / wrong length field, conductivity unset or <= 0, a
non-finite electric field, `potential = "solve"` with a `current` patch missing its value, `electric.enabled` together with a
Lorentz source, unsupported algorithm (existing source check), empty zone (existing). The `boundaries.<p>.electric*` keys, today
"only used with `electric.enabled`", are also accepted when a Lorentz source with `potential = "solve"` exists.

### D2 F-217 library (`rcfd-fv/src/hx.rs`, new)

* `Arrangement`: `Counterflow`, `Parallel`, `CrossBothUnmixed`, `CrossCmaxMixed` (the stream of larger capacity rate mixed, the other
  unmixed), `CrossCminMixed`, `ShellAndTube` (one shell pass, 2 or more tube passes). `effectiveness(arr, ntu, cr)` with
  `ntu = UA/C_min`, `cr = C_min/C_max` in `[0, 1]`:
  * counterflow `(1 - e^{-N(1-C)})/(1 - C e^{-N(1-C)})`, `N/(1+N)` for `C = 1` (own branch; the general form is 0/0 there; the branch
    is also used for `|1 - C| < 1e-9`);
  * parallel `(1 - e^{-N(1+C)})/(1 + C)`;
  * crossflow, `C_max` mixed (`C_min` unmixed) `(1/C)(1 - exp(-C(1 - e^{-N})))`; `C_min` mixed `1 - exp(-(1/C)(1 - e^{-NC}))`;
  * crossflow, both unmixed (series solution; the closed approximation used) `1 - exp[(N^{0.22}/C)(exp(-C N^{0.78}) - 1)]`;
  * shell and tube, one shell pass `2 / {1 + C + sqrt(1+C^2) (1 + e^{-N sqrt(1+C^2)})/(1 - e^{-N sqrt(1+C^2)})}`;
  * `C = 0`: `1 - e^{-N}` for every arrangement (handled explicitly, no division by zero).
* `Exchanger::solve(arr, c_primary, c_secondary (0 = infinite), ua, t_p_in, t_s_in)` returns heat to the primary stream
  `Q = eps C_min (T_s,in - T_p,in)` (positive when the secondary is hotter), outlet temperatures `T_p,out = T_p,in + Q/C_p`,
  `T_s,out = T_s,in - Q/C_s`, `eps`, `NTU`, `Cr`. `ntu_for_effectiveness` inverts by bisection (counterflow: closed form).
* Pressure-loss closure `core_dp(r1, k, rho, v) = r1 v + k rho v |v| / 2` (viscous-type linear plus loss-coefficient part), and
  `vent_dp(k, rho, v) = k rho v |v| / 2`.
* Independent references used only in tests (not in the library): finite-difference marching of the two-stream balance for counterflow
  and parallel flow, a row recursion for the crossflow variants (cell-exact exchange).

Derivations (kept here, nothing copied from a text). Counterflow: `dq = U dA (T_h - T_c)`, `C_h dT_h = -dq`, `C_c dT_c = -dq`
(with `T_c` increasing against the hot flow direction); `d(T_h - T_c)/dA = -(U/C_h - U/C_c)(T_h - T_c)`, integrating over the
area with the boundary conditions `T_h(0)=T_h,in`, `T_c(A)=T_c,in` gives the stated form with `N = UA/C_min`. Crossflow, `C_max`
mixed: the `C_max` stream temperature `T_B(x)` depends on its own flow direction only; the unmixed `C_min` element at `x` leaves at
`T_B + (T_A,in - T_B) e^{-N}`; the energy balance of the mixed stream `d theta/d xi = -C (1 - e^{-N}) theta`, `theta =
(T_B - T_A,in)/(T_B,in - T_A,in)`, gives `theta(1) = exp(-C(1 - e^{-N}))` and `eps = (1 - theta(1))/C`. The `C_min` mixed case is the same
with exchanger NTU of the unmixed stream `N C` and capacity ratio `1/C`.

### D3 F-217 case tree (`components.<n>`, additive)

* `heat_exchanger` gets `mode = "ntu"` (existing modes unchanged) with `arrangement` (default `counterflow`), `ua` [W/K] (`-1` unset,
  required > 0 in ntu mode), `secondary_temperature` [K] and `secondary_capacity_rate` [W/K] (0 = infinite; the existing keys),
  `loss_coefficient` K and `loss_linear` r1 [Pa s/m] (both default 0 = none; allowed in every mode). The capacity rate of the primary
  stream is `rho cp Q` of the zone flow `Q` (clamped at 0 for a reversed flow, which gives zero heat); `NTU = UA/C_min` is therefore
  evaluated at the actual flow every iteration.
* Dual cell: `dual_cell = true` with `secondary` object (`zone`, `box_min`, `box_max`, `cell_zone`, `direction`, `loss_coefficient`):
  the secondary stream is a second flow-through zone of the same mesh (same fluid properties, same constant `rho cp`). Its inlet
  temperature and capacity rate are measured on its own entry faces and flow exactly as the primary ones. The heat `Q` is added to
  the primary zone volume and removed from the secondary zone volume with the same number, so the exchange conserves energy to round
  off. `arrangement` is chosen by the user (the geometry of the macro cell is not inspected).
* Pressure-loss closure: for the primary (and secondary) zone, `loss_coefficient` and `loss_linear` act on the downstream interior
  plane as the existing `FaceJump` resistance, `resist = K |phi|/(2 a^2) + r1/(rho a)` per face (kinematic, `a = |S_f|`). They need an
  interior plane like fan and porous jump (located error otherwise).
* Fan boundary: `type = "fan_boundary"` with `patch` (fixed pressure), `curve_flow`, `curve_pressure`, `reference_pressure`, `relax`:
  patch static pressure `p_ref + dp(Q_in)` with `Q_in` the volume flow INTO the domain through the patch, the piecewise-linear curve
  of ADR 0012 (end values held). Same lagged, relaxed update as `network_outlet`. The fan curve is treated as a static pressure rise at
  the boundary (the dynamic head is not added; stated).
* Vent and porous jump are NOT re-implemented: `bc_kind = "vent"` (ADR 0022) is the vent boundary and `components.<n>.type =
  "porous_jump"` (ADR 0012, checks Z1 and Z2) is the porous jump. Their prior evidence (C3, Z1, Z2) is cited, not re-registered; the new
  checks below test the new code that uses them (`fan_boundary`, loss closure, library). The network boundary type is the existing
  `network_outlet`; a multi-branch network is library-only and not delivered.
* Monitors: `<n>_flow`, `<n>_duty`, `<n>_t_in` as before, plus `<n>_eps`, `<n>_ntu`, `<n>_dp` (loss) and for the dual cell
  `<n>_sec_flow`, `<n>_sec_t_in`. Algorithms: steady `simple`/`simplec` constant density with `energy.enabled` (as ADR 0012).
* Located errors: unknown arrangement, `ua` unset or <= 0 in ntu mode, secondary capacity rate < 0, `dual_cell` without a secondary
  zone/direction, dual-cell zones that overlap, negative loss coefficients, fan_boundary patch not a fixed-pressure patch, curve errors.

Duct tests of F-217 use `box_2d` with `periodic_y = true` (a true one-dimensional plug flow, patches `xmin`/`xmax`) so that no wall
condition enters; no `symmetry` side walls (known defect, ADR 0036 A4).

## Registered checks and numeric limits

`M` = Lorentz library (`crates/rcfd-fv/tests/lorentz.rs`), `A` = MHD through the case tree (`crates/rcfd-api/tests/mhd.rs`),
`H` = exchanger library (`crates/rcfd-fv/tests/heat_exchanger.rs`), `X` = exchanger through the case tree
(`crates/rcfd-api/tests/heat_exchanger.rs`). The Hartmann closed form is derived below.

Closed form (Hartmann channel, plates at `y = +-a`, no slip, `B = B0 e_y`, flow along `x`, current along `z`, unit density for the
algebra; `sigma`, `mu` constant). `u x B = u B0 e_z`; with `E_z` the electric field along `z`: `J_z = sigma (E_z + u B0)`, force
`J x B = J_z e_z x B0 e_y = -J_z B0 e_x`. Steady fully developed momentum: `0 = G + mu u'' - sigma B0 (E_z + u B0)`, `G = -dp/dx`
(body force per volume). `Ha = B0 a sqrt(sigma/mu)`.
(a) Short circuit `E_z = 0`: `mu u'' - sigma B0^2 u = -G`, `u = (G/(sigma B0^2)) (1 - cosh(Ha y/a)/cosh Ha)`, mean
`U = (G/(sigma B0^2))(1 - tanh Ha/Ha)`; Poiseuille at the same `G`: `U_P = G a^2/(3 mu)`; ratio `U/U_P = 3 (Ha - tanh Ha)/Ha^3`
(tends to 1 as `Ha -> 0`; checked). (b) Insulating walls: zero net current `int J_z dy = 0` gives `E_z = -U B0`; then
`mu u'' - sigma B0^2 (u - U) = -G`; with `u(+-a)=0` and mean `U`: `u/U = (1 - cosh(Ha y/a)/cosh Ha)/(1 - tanh Ha/Ha)`,
`G = sigma B0^2 U (tanh Ha/Ha)/(1 - tanh Ha/Ha)`. Both reduce to the Poiseuille parabola for `Ha -> 0`.
Profile error metric: `max_c |u_c - u_exact(y_c)| / max_y u_exact` over the cell centres (a pointwise relative error would diverge at
the wall). Flow-rate error: `|U_cfd/U_exact - 1|` with `U_cfd` the cell-volume mean.

Common A set-up: `box_2d`, `lx = 1`, `nx = 4`, `periodic_x = true`, `ly = 2a = 1`, uniform `ny`, depth default, `ymin`/`ymax` true no-slip
walls (`velocity = "fixed"`, zero), `rho = 1`, `nu = 1`, `sigma = 1`, `simplec`, `convection = "upwind"`, `h_operator = "from_solution"`,
`physics.body_force = [G, 0, 0]` (fixed), `B = [0, B0, 0]` with `B0 = 2 Ha` (so `Ha = B0 a sqrt(sigma/mu)`), `G = 400`. Stationarity:
the maximum change of `ux` over the last 200 iterations below `1e-9` of `max ux`; at most 30000 iterations (not reaching it is a miss).
Resolution rule fixed in advance: `Ha dy/a <= 0.25`, i.e. `ny = 40, 80, 160` for `Ha = 2, 10, 20` (the first-order viscous-layer
discretisation error estimate for a cell Peclet number in the layer of 0.25 is below 1 percent).

* A1 Hartmann, short circuit, `potential = "none"`, `Ha = 2, 10, 20`: profile error <= **2 %** (roadmap acceptance) and flow-rate
  error <= **1 %**. Reported: both errors and `U/U_P` against `3(Ha - tanh Ha)/Ha^3`.
* A2 Hartmann, insulating walls through the imposed field (`applied_electric_field = [0, 0, -U B0]` with `U` the exact mean of (b),
  `G` taken from (b) for `U = 1`; `B0 = 2 Ha`), `Ha = 10`: profile error <= **2 %**, flow-rate error <= **1 %**.
* A3 Lorentz integral and force balance at `Ha = 10` (A1 run): (i) `sum_c F_x V_c` of the reported `lorentz_force_x` equals
  `-sigma B0^2 sum_c u_x V_c` to **1e-9** relative; (ii) global momentum balance `G V = sum_walls mu u_c A/(dy/2) + sum_c sigma B0^2 u_c V_c`
  to **1e-6** relative; (iii) the Lorentz force never accelerates the flow: `sum_c F_x u_x V_c < 0` and equals `-sigma B0^2 sum u_x^2 V` to 1e-9.
* A4 Located diagnostics: each located error of D1 names its path (`sources.<n>.magnetic_field`, `.conductivity`, `.potential`, ...);
  `piso`, `pimple` and `coupled` with a Lorentz source are refused; `electric.enabled` together with a Lorentz source is refused.
* A5 Null check of the solved potential: same channel, `B = [0, 0, B0]` (normal to the plane of the flow, so `u x B = -u B0 e_y` has
  a divergent source), `potential = "solve"`, walls insulated, no fixed potential patch (cell 0 pinned), `Ha = 10`, `ny = 80`: the exact
  state is `J = 0`, `d phi/dy = -u B0`, plain Poiseuille. Limits: `max |J| / (sigma B0 max u) <= 1e-6`; `max|u - u_plain| / max u_plain <= 1e-6`
  against the run of the same case without the source (identical numerics); and with `potential = "none"` the same case reproduces the
  Hartmann flow-rate ratio `3(Ha - tanh Ha)/Ha^3` within **1 %** (the braking of an uncompensated `u x B`).
* M1 Lorentz algebra (no mesh solve): for several non-collinear `u`, `B`, `E_ext`, `sigma`: `J x B / rho` equals the hand evaluation
  to 1e-13; the split identity `BodyForce - sink*u = J x B / rho` to 1e-13; `F . B = 0` to 1e-12 relative; for `E = 0` the work
  identity `u . (J x B) = -|J|^2/sigma` to 1e-12; the Lorentz integral over the zone equals `V_zone J x B`.
* M2 Electrodes with the potential solve (library, `box_2d` 2-D, `B = B0 e_z`, imposed parabola `u(y) = u0 (1 - (y/a)^2) e_x`, electrodes
  `phi(ymin) = 0`, `phi(ymax) = DV`, `ny = 40`): the transverse current density `j` is uniform: `max|j_f - j|/|j| <= 1e-9` on the faces
  and `j = sigma(-DV/(2a) - B0 Ubar)` with `Ubar = 2 u0/3` to **5e-3** relative for `DV = 0` (short circuit) and for `DV = -2 a B0 Ubar`
  the open-circuit current vanishes (`|j|/(sigma B0 Ubar) <= 5e-3`); the cell reconstruction gives `sum_c F_x V_c = j B0 V` to 1e-9 relative
  to `|j B0 V|` in the first case.

H library (exchanger):
* H1 counterflow and parallel against the independent finite-difference marching (2000 steps, Richardson extrapolated): relative error
  <= **1e-5** for `NTU = 0.2, 1, 3, 8` and `Cr = 0, 0.25, 0.75, 1`; the `Cr = 1` branch is continuous with the general form (`Cr = 1 - 1e-7`) to 1e-6.
* H2 limits: `Cr = 0` gives `1 - e^{-N}` for all arrangements (1e-14); `eps <= N` small-`N` slope (`eps/N -> 1`) within 1e-3 at `N = 1e-3`
  for all arrangements; counterflow `eps -> 1` for `Cr < 1` and `N/(1+N)` for `Cr = 1`; parallel `eps -> 1/(1+Cr)`; ordering
  `eps_parallel <= eps_cross_both <= eps_counter`, `eps_shell <= eps_counter`, every `eps` in `[0,1]`, monotone increasing in `N` and
  decreasing in `Cr` on a grid of 200 points (strict violations counted, zero allowed).
* H3 crossflow variants against the cell-exact recursion (800 x 800 cells for both unmixed, 4000 rows for the mixed ones): `C_max` mixed and
  `C_min` mixed relative error <= **1e-3**; both unmixed (approximate formula) <= **2 %** (stated approximate; the series solution is not
  implemented). Shell and tube: the closed form is derived from memory of the standard result and is checked only for `Cr = 0`, limits and
  the ordering of H2; it is marked UNVERIFIED in the reference list and no numeric agreement with a published table is claimed.
* H4 energy balance exact: `|Q_p - Q_s|/|Q| <= 1e-12` (outlet temperatures from `Q`); round trip `ntu_for_effectiveness(eps(N)) = N` to 1e-9;
  `core_dp` / `vent_dp` against hand values to 1e-14; sign and reverse-flow behaviour (negative flow gives zero heat).

X case tree (exchanger), common set-up: `box_2d`, `periodic_y = true`, `lx = 2`, `ly = 0.2`, `nx = 100`, `ny = 4`, depth default, `rho = 1.2`,
`nu = 1.5e-5/1.2 * 1000` (laminar, immaterial), `cp = 1005`, `conductivity = 1e-6`, energy on (no buoyancy), inlet `xmin` velocity
`[2,0,0]`, `T = 300`, outlet `xmax` zero gradient, fixed pressure; exchanger zone `x in [0.8, 1.0]`; `simplec`, upwind. Stationarity as
in ADR 0012 (change over 300 iterations below `1e-8`). Measured stream temperatures are cell columns at `x = 0.01` and `x = 1.99`.
* X1 Counterflow ntu mode: `UA = 40 W/K`, `T_sec = 350 K`, `C_sec = 20 W/K` (`C_p = rho cp u A_frontal` is computed from the measured
  flow). Limits: stream enthalpy rise `rho cp Q (T_out - T_in)` equals the reported duty to **1e-6** relative; reported duty equals
  `eps_counter(NTU, Cr) C_min (T_sec - T_in)` evaluated with the reported `T_in` and flow to **1e-9** (consistency); and equals the
  independent marched counterflow solution (H1 reference) with the measured inlet temperature and flow within **1e-4** relative and,
  as the roadmap acceptance, within **2 %**. Repeated for `C_sec = 0` (infinite) and parallel-flow arrangement.
* X2 Loss closure: `loss_coefficient K = 5` and separately `loss_linear r1 = 2 Pa s/m` on the exchanger plane (`u = 2 m/s`): the pressure
  difference of the cell columns just before and after the zone equals `K rho u^2/2` and `r1 u` plus a no-loss reference run's difference
  (the plane sits in a uniform stream, reference `~ 0`) with relative error <= **1e-3** (Z1 measured 1e-6 for the same mechanism).
* X3 Dual cell: lower half (`y < 0.1`) hot stream 350 K, upper half 300 K (inlet `temperature_expression`), zones `x in [0.8,1.0]` each half,
  `UA = 60 W/K`, counterflow arrangement, both streams `u = 2 m/s`. Limits: stream enthalpy changes of the two halves are `+Q` and
  `-Q` of the reported duty to **1e-6** relative each; their sum is below **1e-6 Q**; reported `Q` equals the marched counterflow solution
  with the measured inlets and flows within **1e-4** and **2 %**.
* X4 Fan boundary: inlet patch `fan_boundary` with the curve of ADR 0012 Z2 (`(0, 200), (0.03, 140), (0.06, 0)` Pa at `Q` m^3/s,
  `A_frontal = 0.02 m^2`, i.e. `ly = 0.2`, `depth = 0.1`), downstream the Z1 porous jump (`alpha = 1e-8`, `C2 = 1000`, `t = 0.02`), outlet
  static pressure 0: the operating point is the intersection of the fan curve with the analytic jump (bisection 1e-12). Limits (Z2
  values): `|Q - Q_op|/Q_op <= 5e-3`; patch pressure equals the curve value at `Q` to **2e-3** relative (Z3 missed 1e-4 with 8.5e-4 on the
  same lagged mechanism, so 1e-4 is not registered; the cause there was not isolated, and this limit is chosen from that measurement).
  Relaxation `relax = 0.02` as in Z3 (stiff lagged boundaries), stated before the run.
* X5 Located diagnostics (list in D3). X6 `baseline_hashes` unchanged; existing `tests/lumped.rs` unchanged and passing.

## Named gaps registered in advance (reasons for `partial`)

F-214: transient / PISO / PIMPLE and coupled-solver MHD (no momentum source hook); Joule heating `J^2/sigma` into the energy equation;
spatially varying (case-given field or register) `B`; induced field / magnetic Reynolds number effects; the published Hartmann
experiments and a 3-D duct with side-wall layers (only the 1-D analytic profile and the electrode current are checked); non-uniform or
temperature-dependent `sigma` in the Lorentz source.
F-217: distributed (segmented) exchanger with a resolved secondary flow path; both-unmixed crossflow series solution and the shell-and-tube
closed form against a published table; two-phase and condensing exchangers; fouling; separate fluid properties on the secondary side of
a dual cell; multi-branch network boundary (the single-branch `network_outlet` exists); transient operation; the fan curve as a total
pressure with dynamic head.

## Addendum: results and deviations (end of wave 23A)

Written after the runs. Limits are those registered above; none was widened, replaced or deleted. Release builds, `cargo test --release`.

### F-214 results (limit in brackets)

* M1 (`rcfd-fv/tests/lorentz.rs`): hand values and zone split 0 to 1.3e-16 [1e-13]; work identity 3e-16 [1e-12]. Pass.
* M2: electrode current of the short circuit `j = -3.993844` against `-4.000000` (relative 1.54e-3 [5e-3]), spread over the section 7.2e-15 [1e-9],
  Lorentz force integral 4e-15 [1e-9]; open circuit `|j|/(sigma B Ubar)` 1.54e-3 [5e-3] (the residual is the half-cell wall resolution
  of the 2-D profile). Pass. The closed-form consistency test (Hartmann profile substituted into the momentum equation) passes.
* A1 (`rcfd-api/tests/mhd.rs`, short circuit, `potential = none`): profile error as a fraction of `u_max` [2 %] and flow-rate error [1 %]:
  Ha = 2 (ny 40) 1.590e-3 / 1.218e-3; Ha = 10 (ny 80) 6.532e-3 / 8.580e-4; Ha = 20 (ny 160) 6.532e-3 / 4.064e-4 (mean velocity over the
  Poiseuille value 0.38896 against 0.38849 at Ha = 2, 0.02702 against 0.02700 at Ha = 10, 0.00713 against 0.00713 at Ha = 20). Pass
  (roadmap acceptance "within 2 percent": met for this 1-D set-up at three Hartmann numbers).
* A2 insulated walls, Ha = 10: profile 6.532e-3 [2 %], flow 8.580e-4 [1 %], `G = 44.4444`. Pass.
* A3 Ha = 10: Lorentz integral 8.9e-16 [1e-9], work identity 3.3e-16, global momentum balance 1.8e-15 [1e-6], `sum F u V = -85.04 < 0`
  (the field removes kinetic energy). Pass.
* A4 located diagnostics pass. The check rejects `solver.algorithm = "coupled"` at parse time (path `sources.<n>.type`), so the test
  asserts the path from the parse diagnostics (the first version of the test unwrapped a parse that is registered to fail; corrected, not a solver issue).
* A5 solved potential (`potential = solve`) against the same channel: `max|J|/(sigma B u_max)` 8.15e-12 [1e-6], `max|u - u_plain|/u_max` 1.13e-10 [1e-6]; the
  `potential = none` flow rate matches the closed form at 8.58e-4 [1e-2]. Pass. (The solved potential is exercised only in the case where it must
  vanish by symmetry; a case with a non-trivial potential field, for example a duct with side walls, is not checked: named gap.)
* Runtime of `tests/mhd.rs` is about 340 s in release (A5's plain-Poiseuille reference run is the slow part).

### F-217 results

* H1 (`rcfd-fv/tests/heat_exchanger.rs`) counterflow and parallel against the independent marched integration: worst relative error 4.99e-13 [1e-5]. Pass.
* H4 energy balance 1.4e-15 [1e-12], NTU round trip, `core_dp`/`vent_dp` hand values: pass.
* H3 crossflow `C_max` mixed and `C_min` mixed against the cell-exact recursions: worst 4.4e-9 [1e-3]. Pass. (Reference: the Richardson extrapolation of the 4000-row and 8000-row
  recursions, `2 f(8000) - f(4000)`, not the single 4000-row value registered; the extrapolation is at least as accurate as the registered reference, see deviation 9.)
* **H3 both-unmixed approximation: MISS.** Against the registered 800 x 800 cell-exact recursion the relative error is 8.10e-3, 1.64e-2, 3.33e-2
  at N = 0.5, Cr = 0.25, 0.5, 1; 1.38e-3, 4.98e-3, 1.61e-2 at N = 1; 8.94e-3, 1.06e-2, 4.28e-3 at N = 3 [limit 2e-2]. The Cr = 1, N = 0.5 case is outside the limit.
  The 800 x 800 value agrees with the Richardson extrapolation of the 150 and 300 cell recursions to at most 1.4e-5 relative, so the reference is not the cause of the miss.
  Recorded as `h3_cross_unmixed_known_miss` (ignored, limit unchanged, 800 x 800 reference).
* **H2 both-unmixed: MISS** (small-N slope `eps/N` = 0.99836 at N = 1e-3 [within 1e-3]; 177 violations of `parallel <= cross` at small N and small Cr
  on the 200 x 19 grid, e.g. N = 0.05, Cr = 0.05, where the fit 0.04866 lies below the parallel-flow 0.04871). Recorded as `h2_cross_unmixed_slope_known_miss` and
  `h2_limits_and_ordering_known_miss` (ignored, limits unchanged).
  Root cause (diagnosed, not tuned): the both-unmixed relation is the empirical curve fit `1 - exp[N^0.22/Cr (exp(-Cr N^0.78) - 1)]`, not the exact
  series solution; its small-N behaviour `eps = N - (Cr/2) N^1.34 + ...` differs from the exact `N - N^2 (1+Cr)/2`, and its error is largest at small N and `Cr = 1`.
  The series solution is not implemented. The corrected, non-ignored test `h2_limits_and_ordering_exact_arrangements` applies the same registered limits to every
  exactly derived arrangement and replaces the both-unmixed fit by the `C_max`-mixed crossflow in the ordering `parallel <= cross <= counter`: 0 violations. The both-unmixed
  option therefore ships with a known 3.3 percent maximum deviation and must not be used where better than that is needed.
* X1 (`rcfd-api/tests/heat_exchanger.rs`; `box_2d`, `periodic_y`, `lx = 2`, `ly = 0.2`, `nx = 100`, `ny = 4`, depth 0.02 so `Q = 8.0e-3 m^3/s`, `NTU = 4.146`):
  counterflow `C_sec = 20`: duty 451.450431 W, stream enthalpy rise relative 2.57e-12 [1e-6], against the library eps-NTU 0 [1e-9], against the marched solution 9.8e-15
  [1e-4 and 2e-2]; counterflow `C_sec = 0`: 474.764283 W, 2.46e-12, 0, 1.0e-14; parallel `C_sec = 20`: 324.721140 W, 3.43e-12, 0, 1.7e-14. Pass.
  Honest reading: the macro cell is the algebraic eps-NTU relation, so agreement with it and with the marched solution is expected by construction; the check
  verifies that the flow, the inlet temperature and the energy source are wired correctly (stream enthalpy equals the duty), not the physics of a resolved exchanger.
* X2 loss closure at `u = 2 m/s`: `K = 5` gives 12.000000 Pa against 12 (relative 2.7e-14), `r1 = 2` gives 4.000000 against 4 (6.2e-15) [1e-3]; the no-loss reference drop is 6e-15 Pa. Pass.
* X3 dual cell (lower hot half 350 K, upper 300 K, `UA = 60`, `Q = 4.0e-3` per half): duty -223.250588 W; stream enthalpy changes -223.250585 and +223.250585
  (relative 1.34e-8 [1e-6]); sum over duty 9.05e-13 [1e-6]; against the marched solution 8.5e-13 [1e-4 and 2e-2]. Pass.
* X4 fan boundary with the Z2 curve and the Z1 jump: operating flow 3.051516e-2 m^3/s against the analytic intersection 3.049596e-2 (relative 6.30e-4 [5e-3]);
  patch pressure 137.5959 Pa against the curve value at the measured flow (relative 1.7e-14 [2e-3]); jump drop 82.86 Pa. Pass. The 2e-3 limit was registered because
  Z3 missed 1e-4; with the first-evaluation rule below the lag is invisible at convergence (the patch pressure equals the curve to round-off).
* X5 located diagnostics, X6 (`baseline_hashes` 3 passed 1 ignored as before; `tests/lumped.rs` 6 passed 2 ignored as before): pass.

### Deviations from the registered set-up (disclosed)

1. X4 area: `box_2d` has the depth `lx/nx = 0.02` and no `depth` key, so the registered `A = 0.02 m^2` is obtained with `ly = 1.0` (4 cells of 0.25 m) instead of `ly = 0.2`, `depth = 0.1`. The
   same curve and jump are used.
2. X4 distributed bed: a lossless duct between a pressure inlet and a pressure outlet diverged within two SIMPLEC iterations (also without the fan boundary: fixed
   inlet pressure plus the porous jump alone), for every variant tried (`ly` 0.2 and 1.0, `ny` 2 and 4, initial speeds 0.5 to 2). A linear Darcy bed `sources.bed`
   (`K = 1e-6 m^2`, `zone = all`, `dp = mu/K v L`, `L = 2 m`) over the whole duct restores stability and is part of the analytic operating point
   (`fan(Q) = jump(Q/A) + mu/K (Q/A) L`). The fan boundary and the jump are unchanged. This is a limitation of the segregated algorithm for lossless pressure-pressure ducts, not
   of the fan boundary; not isolated further. `docs/case-schema.md` states it.
3. `fan_boundary` first evaluation: the registered text said "same lagged, relaxed update as `network_outlet`", which relaxes from the reference pressure. The first
   evaluation now takes the curve value at the initial flow (no relaxation from the reference pressure); later evaluations relax with `relax = 0.02` as registered.
4. Stationarity helper: the pressure field of a lossless periodic duct is zero to round-off, so the relative change of `p` is noise (0.3 to 0.4 on first run). The helper
   scales the pressure change by `max(max|p|, 1)` (kinematic units, about the dynamic head); `ux` and `T` keep the registered relative test.
5. X1 to X3 use `conductivity = 1e-6` and depth 0.02 as listed in the registered common set-up; the X4 relaxation 0.02 is as registered. The exchanger inlet temperature used in the
   comparisons is the one the component reports (`hx_t_in`), the same convention as the supplementary check of Z4.
6. The H2 slope and ordering assertions were split as described above; no registered number changed.
7. Process slip: files written by a Windows Python script briefly carried CRLF line ends (`lib.rs`, `electric.rs`, `lorentz.rs`); they were normalised to LF before any commit.
   `docs/case-schema.md` is stored with CRLF in the working tree and was edited preserving that.
8. Z3 (ADR 0012) remains an open miss from before; not touched.
9. H3 references: the mixed variants use the Richardson extrapolation of 4000 and 8000 rows (registered: 4000 rows); the both-unmixed known-miss test uses the registered
   800 x 800 recursion (the first version of the test used a Richardson extrapolation of 150 and 300 cells; replaced after the review, results above and agreeing to 1.4e-5).
10. Rewritten registered tests are kept as ignored known-miss tests, as the process requires: `x4_registered_lossless_setup_known_miss` (registered X4 set-up without the bed, ly = 1.0,
    initial speed 2 m/s, 8000 iterations; measured: flow becomes NaN, within about 8 iterations in the diagnostic runs, patch pressure stays at 200 Pa, limits 5e-3 and 2e-3 unchanged and not met) and
    `x1_registered_stationarity_known_miss` (the originally registered stationarity helper with pressure-scale floor 1e-12, counterflow `C_sec = 20`: pressure change 0.423 against 1e-8; the same
    helper gave 0.299 for X2 and 0.132 for X3 in the first runs). The corrected set-up and helper (`run`, floor 1) pass in the non-ignored tests.

### Status

F-214 `partial`: the Hartmann acceptance (profile within 2 percent, flow-rate reduction within 1 percent) is met at three Hartmann numbers in a 1-D set-up and the integral
and located-error checks pass, but transient / PISO / PIMPLE / coupled MHD, Joule heating, case-given varying `B`, the induced field, a case with a non-trivial solved potential
(side-wall layers) and any published experiment are not delivered.
F-217 `partial`: the counterflow acceptance (within 2 percent of eps-NTU) is met and fan and vent boundaries are checked against their prescribed curves (fan boundary X4; vent by the prior
C3 and the closed-form `vent_dp`), but the both-unmixed crossflow relation misses its registered limits (known misses above), the shell-and-tube relation is unverified against a published
table, and a resolved/segmented exchanger, a multi-branch network, two-phase exchangers and a vent boundary exercised in a new CFD test are not delivered.
