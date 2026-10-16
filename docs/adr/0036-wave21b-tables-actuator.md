# ADR 0036: Wave 21B, tabulated and piecewise properties (F-220), actuator-disk momentum source (F-218), per-region turbulence assignment (F-216)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave existed.
Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an
ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect gets a
NEW corrected test and the old one stays as a record. Addenda (exact numbers, file names, mesh arguments not fixed here) may be
appended at the end without touching a registered limit.

Scope. F-220, F-218 and F-216 are `todo`. Each gets a vertical slice and stops at `partial` with named gaps (end of this file).
F-217 (heat-exchanger macro model) is NOT attempted. Clean-room: published literature and first principles only; no vendor code,
text, data or names. Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`) stay bit-identical and are not re-recorded.
Schema additions are additive with explicit unset sentinels (empty lists, `piece_order = 0`, no `regions` entry). Parallel waves:
21A (thermal extras, ADR 0035) and 21C (particles/chemistry wiring, ADR 0037); shared files get small targeted edits only.

## Decisions

### D1 F-220 library (`rcfd-core/src/tabular.rs`, new)

* `PiecewiseLinear { x, y, extrapolation }`: `x` strictly increasing (>= 2 points), all finite. `eval` interpolates linearly.
  Outside `[x_0, x_n]`: `Clamp` (default, end value), `Linear` (extend the end segment) or `Error` (NaN from `eval`, `Err` from `try_eval`).
  `monotonicity()` returns `Increasing | Decreasing | NonDecreasing | NonIncreasing | Constant | None` and the index of the first violation;
  `require_monotonic(strict)` returns a located error.
* `PiecewisePoly { breaks (n+1, strictly increasing), coeffs (n pieces, each >= 1 coefficient, ascending powers of the ABSOLUTE
  variable T, Horner) }`; piece `i` is used on `[breaks_i, breaks_{i+1})`, the last piece includes its upper end. Same extrapolation modes
  (Linear uses the end piece's derivative). `max_jump()` returns the largest value jump at an interior break and its index;
  `require_continuous(tol)` is a located error.
* `Vogel { a, b, c }`: `p(T) = a exp(b / (T - c))` (Vogel-Fulcher-Tammann; `c = 0` is the Andrade/Arrhenius form `a exp(b/T)`); valid
  for `a > 0`, finite, and `T > c` at evaluation (validate cannot know T; the case layer checks the temperature range of the
  initial and boundary data against `c`).
* `Tait { rho_ref, p_ref, b, c }`: compressible liquid, `1 - rho_ref/rho = c ln((b + p)/(b + p_ref))`, i.e.
  `rho(p) = rho_ref / (1 - c ln((b+p)/(b+p_ref)))`; inverse `p(rho) = (b + p_ref) exp((1 - rho_ref/rho)/c) - b`. Requires `rho_ref, b, c > 0`,
  `b + p_ref > 0` and `1 - c ln(..) > 0`. (Form as in the NIST TDE Tait density equation, `d = d_sat / [1 - c ln((b+p)/(b+p_sat))]`: verified.)
* `PtTable` (pressure-temperature table with a saturation line): axes `t` (n_t) and `p` (n_p) strictly increasing; two value grids
  `liquid`, `vapour` (n_t x n_p, row = temperature, NaN = no data at that node, the unset sentinel); saturation line = `PiecewiseLinear`
  `psat(T)` that must be strictly increasing (checked) so `tsat(p)` is its exact inverse (inversion of the piecewise-linear
  interpolant). `phase(T, p)`: `Liquid` when `T <= t_sat_max` and `p > psat(T)`, else `Vapour` (so `p == psat` is `Vapour`, and above the
  last saturation temperature the vapour grid is used, supercritical fluid being the caller's choice of data). `eval(T, p)` looks up
  the grid of the phase, bilinear on the cell containing (T, p); if any of the four corner nodes is NaN or the point is outside the axes
  the result is a located error naming the phase, the cell indices and the saturation pressure at T (a cell that straddles the
  saturation line is therefore reported, never blended). Tables are inputs; no property data is shipped.
* `PropertyLaw` gains the variants `PiecewiseLinear`, `PiecewisePolynomial`, `Vogel` (the `Tait` law and `PtTable` are functions of p,
  not of T, and are library-only). `eval` and `validate` cover them; existing variants and their results are untouched.

### D2 F-220 case tree (`properties.<viscosity|conductivity|specific_heat>`)

New `law` values and keys of the shared `property_law` schema: `law = "piecewise_linear"` with `temperatures` and `values` (equal length >= 2,
`temperatures` strictly increasing), `extrapolation = "clamp"` (default) | `"linear"` | `"error"` (an `error` law that leaves its range
stops the run with a message naming the property and temperature); `law = "piecewise_polynomial"` with `breakpoints` (n+1), `piece_coefficients`
(flat, n x `piece_order`), `piece_order` (coefficients per piece; 0 = unset, an error for this law) and `extrapolation`; `law = "vogel"` with
`vogel_a`, `vogel_b`, `vogel_c`. All located by key path (`properties.conductivity.temperatures`, with the index of the offending entry in
the message). Everything else about `properties` (steady SIMPLE/SIMPLEC, energy on, constant density) is unchanged and
the defaults (`law = "constant"`) are untouched.

### D3 F-218 actuator disk

New `sources.<name>.type = "actuator_disk"` (steady, like the other sources). The disk VOLUME is the existing zone of the source
(`zone` = box / cell_zone / register; a circular disk is an expression register on the radial coordinate). Keys: `thrust` [N, signed, the
total force exerted ON the fluid], `direction` (the direction of that force; required, non-zero), `profile = "uniform"` (default) | `"linear_r"`
(force density proportional to the distance from the disk axis), `disk_centre`, `disk_axis` (required for `linear_r` and for swirl; defaults to
`direction`), `torque` [N m] (0 = no swirl; tangential force on the fluid about `disk_axis`, positive right-handed), `density` (the density used
to convert force to the kinematic acceleration of the solver, 0 = use `physics.rho`). The cell weights are `w_c = V_c g(r_c)`, `g = 1` or `r`, and the
body acceleration in zone cell `c` is
`a_c = (1/rho) [ T w_c / sum(w) * n + Q w_c / sum(w r) * e_theta,c ]`, `e_theta = axis x r_hat`. Hence the total force
is `T n` and the total torque `Q` about the axis exactly (to round-off) on any mesh, with the swirl force carrying no net axial part. Face
accelerations follow the existing `BodyForce` convention (`a_f . S_f` like the `acceleration` source). Momentum-theory helpers
in `rcfd-fv/src/actuator.rs` (pure functions): `hover_induced_velocity(T, rho, A) = sqrt(T/(2 rho A))`; `axial_disk_velocity(u0, T, rho, A)
= u0/2 + sqrt(u0^2/4 + T/(2 rho A))` (root of `T = 2 rho A u_d (u_d - u0)`), `wake_velocity = 2 u_d - u0`, `power = T u_d`,
`propulsive_efficiency = u0/u_d`, `betz_cp(a) = 4 a (1-a)^2`, `betz_ct(a) = 4a(1-a)`. Not provided: blade-element lookup, tip loss, trim,
unsteady disks (named gaps).

Derivation (Rankine-Froude, incompressible, uniform, no swirl): velocity at the disk `u_d = (u0 + u_w)/2`, mass flow `m = rho A u_d`, thrust
`T = m (u_w - u0)`, power `P = T u_d = m (u_w^2 - u0^2)/2`. Hover (`u0 = 0`): `u_d = v_h = sqrt(T/(2 rho A))`, `u_w = 2 v_h`, `P = T^{3/2}/sqrt(2 rho A)`.
With axial induction `a` (`u_d = u0(1-a)`): `C_P = 4a(1-a)^2`, maximum at `a = 1/3` equal to `16/27 = 0.5926` (Betz limit; the value 0.593 verified, the
hover formulas derived here and not checked against a rotorcraft text).

### D4 F-216 slice: per-region turbulence assignment

New section `regions.<name>` (collection): the shared zone keys (`zone`, `box_min`, `box_max`, `cell_zone`, `register`), `turbulence = "model"` (default,
the global model) | `"laminar"` (the momentum equation uses the molecular viscosity `physics.nu` on every face that touches a cell of the region; boundary faces by their
owner cell). Implemented as a mask applied to the face viscosity vector the turbulence model hands to the momentum equation, steady and
transient incompressible paths (`mask_faces` in `rcfd-api/src/regions.rs`). This is the only part of F-216 that the existing zone machinery
supports without touching the conjugate-heat-transfer code (held by wave 21A). The turbulence transport equations are still solved in the
region (the model's `nu_t` there is simply not used by momentum): a named gap, as are per-region energy, radiation, species models and
region-scoped coupling. Combination with RSM, plugin models, rheology or temperature-dependent viscosity is a located error.

## Registered checks

Library tests: `crates/rcfd-core/tests/tabular.rs` (T), `crates/rcfd-fv/tests/actuator.rs` (A1, A2); tree tests: `crates/rcfd-api/tests/tabulated_properties.rs` (P),
`crates/rcfd-api/tests/actuator_disk.rs` (A3-A5), `crates/rcfd-api/tests/regions.rs` (R). All limits are relative unless stated.

**T1 piecewise-linear exactness.** Nodes `x = {0, 0.3, 1, 2.5, 4}`, `y = 2x - 3`: at the nodes bit-exact (`==`), at 1000 deterministic
points in `[0,4]` `|f - (2x-3)| <= 1e-13 (1+|2x-3|)`. A kinked table `y = {1, 3, 2, 2, 5}` on `x = {0,1,2,4,5}` at the segment midpoints equals the hand
averages `{2, 2.5, 2, 3.5}` to 1e-15.
**T2 piecewise polynomial.** The cubic `T^3 - 2T + 1` split at `T = 1.2` on `[0, 3]`, each piece storing the same absolute coefficients:
equals the Horner value to 1e-13 at 500 points; `max_jump <= 1e-14`. A variant whose second piece is shifted by 0.5: `max_jump` equals
0.5 to 1e-14 at interior break index 0 and `require_continuous(1e-9)` fails with a message containing `break 0`.
**T3 extrapolation.** `Clamp` returns the end value exactly (`==`); `Linear` returns `y_end + slope (x - x_end)` to 1e-13 (both ends; for the polynomial the end-piece tangent); `Error`: `try_eval` is `Err`, `eval` is NaN.
**T4 monotonic check.** Strictly increasing -> `Increasing`; a flat step -> `NonDecreasing` and `require_monotonic(true)` fails naming the index; a dip at index 3 ->
`None` with first violation 3; a constant table -> `Constant`; decreasing -> `Decreasing`.
**T5 malformed tables are located.** (i) `x` non-increasing at index 3 -> message contains `x[3]`; (ii) NaN `y` at index 2 -> `y[2]`; (iii) length mismatch -> both lengths; (iv) one point -> `at least 2`; (v) polynomial piece count mismatch, an empty
coefficient list in piece 1 (`piece 1`), non-increasing breaks (`breaks[`); (vi) PtTable: grid size mismatch, non-increasing axis (`t[` / `p[`), a non-increasing saturation line (`saturation`).
**T6 Vogel.** `a = 1e-3, b = 800, c = 150`: at T = 250, 300, 400 equals `a exp(b/(T-c))` to 1e-14; `c = 0` gives `a exp(b/T)`; `validate` rejects `a <= 0` and NaN.
**T7 Tait.** `rho_ref = 998, p_ref = 1e5, b = 3e8, c = 0.09`: `rho(p_ref) = rho_ref` to 1e-15; the defining relation `1 - rho_ref/rho = c ln((b+p)/(b+p_ref))` holds to 1e-13 (absolute) at 20 pressures between 1e5 and 5e8 Pa;
`pressure(density(p)) = p` to 1e-12 relative; `d rho/dp` at `p_ref` equals `rho_ref c/(b + p_ref)` against a central difference with step 1e3 Pa to 1e-6 relative; `rho` is increasing with `p`.
**T8 PtTable exactness and saturation.** Grid `T = 300..400 step 10`, `p = 1e5..5e6` (11 nodes), bilinear function `f = 3 + 0.5 T - 2e-6 p + 1e-8 T p` on all nodes (single phase, no NaN): nodes reproduce to 1e-14; 500 deterministic interior points to 1e-12 (bilinear is exact for this form).
Saturation line from IF97 (`iapws::saturation_pressure`) at 1 K nodes over 280-370 K: (i) `psat(tsat(p)) = p` to 1e-12 relative at 200 pressures; `tsat(psat(T)) = T` to 1e-10 K; (ii) interpolation error against the IF97 value at the 90 mid-points `T + 0.5` is <= 1e-3 relative (derived bound `h^2/8 * p''/p`, `p''/p <= 4.7e-3 /K^2` for `ln p ~ A - 5400/T`, i.e. <= 6e-4); (iii) phase selection: `p = psat(T)(1 + 1e-9)` -> `Liquid`, `psat(T)(1 - 1e-9)` -> `Vapour`, at 25 temperatures; (iv) a liquid grid with NaN at nodes below the saturation line: a query in a cell with a NaN corner returns an error containing `saturation` and the cell indices; a query in a fully valid cell succeeds.
**T9 IF97-built tables.** Liquid density table at `T = 290..360 K step 5`, `p = 1..10 MPa step 1` and vapour density table at `T = 380..500 K step 5`, `p = 0.01..0.1 MPa step 0.01`, values from `iapws::water_state`: nodes to 1e-12 relative; at the centre of every cell the relative error against `water_state` is <= 1e-4 for liquid and <= 1e-4 for vapour (interpolation error `h^2/8 rho_TT`, estimated 2e-5).

**P1 piecewise-linear law equals the polynomial it samples.** The steady conduction case of the F-053 test (`tests/properties.rs`, `k = k0(1 + beta (T-300))`, `k0 = 1`, `beta = 0.01`, walls 300 K and 400 K) with a `piecewise_linear` law at nodes 300, 400 (values 1, 2): the temperature field differs from the polynomial-law run by <= 1e-9 K, and the heat flow report by <= 1e-9 relative.
**P2 kinked conductivity, Kirchhoff transform.** `k = 1, 2, 4` at `T = 300, 350, 400` (piecewise-linear), same case: the Kirchhoff integral `K(T) = theta + theta^2/100` for `theta = T-300 <= 50`, `K = 75 + 2s + s^2/50` for `s = T-350`; `K(400) = 225`; `K` is linear in x, so `T(x)` is its inverse. Cell-centre temperatures: `max |T - T_exact| <= 0.5 K` (0.5 percent of the 100 K range); the `heat_flow` report within 1 percent of `225 A / L` for the mesh's `A, L`.
**P3 piecewise polynomial.** Two pieces on `[300, 350], [350, 400]` each storing the linear function of P1 (`piece_order = 2`): same as P1 to 1e-9 K.
**P4 Vogel viscosity, plane Couette flow.** The Couette case of `tests/properties.rs` (periodic box 4 x 40, walls 300 K / 400 K, `U = 1`, `rho = 1`) with `mu = 0.01 exp(300 / (T - 100))`, `T` linear in y: constant shear stress `tau`, `u(y) = tau int_0^y dy'/mu(T(y'))`, `tau` from `u(H) = U` (Simpson with 2000 intervals in the test): `max |u - u_exact| <= 1e-2 U`.
**P5 located tree errors.** `temperatures` not increasing -> path `properties.conductivity.temperatures` with the index; length mismatch -> `...values`; `piece_order` 0 or `piece_coefficients` length not a multiple -> `...piece_coefficients`; `law = "vogel"` with `vogel_a <= 0` -> `...vogel_a`.
**P6 defaults unchanged.** The existing `tests/properties.rs`, `tests/tables.rs` and `baseline_hashes.rs` pass unmodified.

**A1 closed forms.** `T = 100 N, rho = 1.2, A = 2 m^2`: for `u0 in {0, 0.5, 2, 5, 20}` the relation `T = 2 rho A u_d (u_d - u0)` holds to 1e-13; `u0 = 0` gives `u_w = 2 u_d` and `u_d = sqrt(T/(2 rho A))` to 1e-14; the power `T u_d` equals `m (u_w^2 - u0^2)/2` with `m = rho A u_d` to 1e-13 (energy theorem), and for `u0 = 0` equals `T^{3/2}/sqrt(2 rho A)` to 1e-13; `propulsive_efficiency = 2 u0/(u0+u_w)` to 1e-14; `betz_cp(1/3) = 16/27` to 1e-14, and a scan of `a` on `[0, 0.5]` in 10^6 steps has its maximum within 1e-5 of 1/3; `betz_ct(0.5) = 1`.
**A2 discrete invariants.** `box_mesh` 24x24x24 on `[0,1]^3` with `Expansion(2)` grading in x, disk = cells with centre in `|x - 0.5| <= 0.1` and `(y-0.5)^2 + (z-0.5)^2 <= 0.3^2` (axis x): for `uniform` and `linear_r`: `|sum_c rho V_c a_c - T n| <= 1e-12 |T|` componentwise; with torque `Q = 0.7`: `|sum_c rho V_c (r_c x a_c).axis - Q| <= 1e-12 Q`, the axial sum still `T` to 1e-12, and the torque is right-handed (positive); accelerations outside the zone are exactly 0; for `linear_r` the axial acceleration ratio of two cells in one x layer equals `r1/r2` to 1e-13. The selected volume vs `pi R^2 t` is REPORTED, no limit.
**A3 diagnostics.** Zero `direction`, `profile = "linear_r"` without a radial extent, unknown profile, non-finite thrust, `torque != 0` with zero `disk_axis`, a non-steady algorithm, empty zone: each a located error (`sources.<n>.direction`, ...).
**A4 pressure jump across a disk in a duct.** `channel_2d` `nx = 60, ny = 10, lx = 2, ly = 0.2, depth = 0.05`, `nu = 1e-3`, `rho = 1.2`, uniform inlet `u0 = 1`, outlet fixed pressure, side walls slip; disk zone = box `x in [0.9, 1.1]`, `thrust = 0.6 N` along +x. Momentum balance of the duct: `Delta p = T / A = 0.6/(0.2 * 0.05) = 60 Pa` (kinematic 50). Measured as the difference of the averages of the pressure columns at `x = 0.5` and `x = 1.5`: `|dp_kin - 50| <= 0.5` (1 percent). `max |u_x - u0| <= 1e-3 u0`, `max |u_y| <= 1e-3`.
**A5 hover momentum theory in a box (report only).** A 3-D box 4x4x6 m, disk radius 0.5 m, `T = 1 N`; the induced velocity averaged over the disk is REPORTED against `sqrt(T/(2 rho A))`; no pass/fail limit is registered because the domain is finite; not a validation claim. Attempted only if time remains.

**R1 laminar region equals a laminar model.** `channel_2d` 30 x 12 (lx 3, ly 0.5), `nu = 0.02`, inlet `u = 1`, steady `simple`, 400 iterations: run A with `turbulence.model = k_epsilon` plus `regions.all.turbulence = "laminar"` (zone `all`), run B with `turbulence.model = laminar`: `max |u_A - u_B| <= 1e-6` and `max |p_A - p_B| <= 1e-6 (max p - min p)`.
**R2 mask semantics.** `mask_faces`: a face with both cells outside the region is unchanged (`==`); an internal face with at least one cell inside, and a boundary face whose owner is inside, equals the molecular viscosity exactly (`==`).
**R3 diagnostics.** Unknown `turbulence` value, empty zone, no turbulence model configured, RSM/plugin/rheology/temperature-dependent viscosity combinations: located errors (`regions.<n>.turbulence`, `regions.<n>.zone`).
**R4 defaults unchanged.** Without `regions` the code path is untouched (an `Option::None`): `baseline_hashes.rs` bit-identical, existing turbulence tests pass.

## Not delivered (named gaps, written in advance)

F-220: orthotropic conductivity; NASA nine-coefficient heat-capacity polynomials as a case law (exist in `rcfd-reactor`); pressure-dependent properties (Tait density, P-T tables) are NOT connected to the solvers (library only; the incompressible solver has constant density); table-file (F-105 CSV) property laws; the acceptance statement on public water and steam values rests on the existing IF97 module, not on new table data.
F-218: blade-element lookup, tip loss, trim, unsteady/rotating disks; the model-rotor data comparison (acceptance 10 percent); a validated CFD hover induced-velocity result (A5 is report only).
F-216: per-region energy/radiation/species models, region-scoped turbulence equations, coupling of regions through interfaces, the conjugate heat transfer benchmark.
F-217: not attempted.
