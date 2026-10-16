# ADR 0035: Wave 21A thermal extras: interface physics (F-215), electric potential and Joule heating (F-213), enthalpy-porosity melting (F-212), Lorentz force (F-214, deferred)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new code was compiled or run.
The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to
an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect
gets a NEW corrected test and the old one stays as a record. Addenda (file names, results, deviations) may be appended at the end
without touching registered limits.

Roadmap: F-215 (primary, slice 1), F-213 (slice 2), F-212 (slice 3), F-214 (slice 4, only if time; see "Deferred"). Each is stopped
at `partial` with named gaps; none is marked done. F-051 (CHT) and F-201 (zone sources) are used, not changed in scope.
Clean-room: published literature only; nothing from vendor material, vendor installs or manuals; no vendor names in the
repository. All closed forms below are derived in this document.

Baseline: `crates/rcfd-api/tests/baseline_hashes.rs` passes on HEAD (51ab6e5) before any change of this wave (3 passed, 1 ignored,
run with `CARGO_TARGET_DIR=<target dir>`). It is never edited; it is run again after each slice. Every new code path
is gated on an `Option` that is `None` unless a new key is set, so existing cases take their old code path unchanged.

## Audit of what exists

* Interfaces (ADR 0024 D2): `thermal_interfaces.<n>` with `solid_a`, `solid_b`, `resistance` [m^2 K/W]; `rcfd_fv::cht::Contact`
  adds `R''/Delta` to the face resistance. A single lumped resistance per interface exists; ADR 0024 checks E1 (jump `q R''`) and E2
  (thin wall at a convection patch equals resolved layers) already give evidence for contact resistance and a layered thin wall on a
  boundary patch. They are NOT re-registered as new evidence below.
* Energy: `EnergySolver` (`alpha_face`, `ddt_weight`, `source` K/s, `sink`, `fixed`), `scalar::sweep_ext`. `ZoneSources::apply_temperature`
  assigns `solver.source = base` and overwrites `sink`/`fixed`; a source that must coexist with zone sources has to go in through `base`.
* Frozen-flow CHT paths in `Simulation::step` (steady and transient) advance only the energy equation. The solved transient path
  (PIMPLE) has NO implicit momentum sink (`sink` exists only in `SimpleSolver`).
* No electric potential, no phase change, no Lorentz force exist.

## Decisions

### D1 F-215: layered interface and open/closed connection

`thermal_interfaces.<n>` gains (additive):

* `layer_thickness` [m] and `layer_conductivity` [W/m/K]: lists of equal length, default empty = none. Total interface resistance
  `R_tot = max(resistance, 0) + sum t_i / k_i` (steady layer resistance, no layer capacity). `resistance` may stay at `-1` (unset)
  only when layers are given; without layers the existing "unset is an error" rule is unchanged.
* `connection` = `closed` (default, current behaviour) | `open`. Open means zero face conductance (`Contact.resistance = +inf` so that
  `1/(1/k + inf) = 0`): an adiabatic gap. Layers or a finite `resistance` together with `open` are a located error (`has no effect`).
  A steady problem (`solver.algorithm` simple/simplec with frozen flow) in which the open interfaces leave a connected group of cells
  with no fixed-temperature or active convection/radiation boundary face is a located error at `thermal_interfaces.<n>.connection`
  (singular system). Transient problems need no such boundary.

Named gaps: thermal capacity of the layers (shell), in-plane (tangential) conduction in the shell, a connection that opens or closes
depending on the solution (conditional on temperature or time), interfaces between separate meshes (mapped coupled interfaces beyond
the F-114 join). These are the reasons F-215 stays `partial`.

### D2 F-213: electric potential, Joule heating

Quasi-static conduction `div(sigma grad phi) = 0` (no charge storage, no magnetic induction), `sigma` per cell:
`sigma = sigma_0 / (1 + a (T - T_ref))` (linear resistivity, `a` = `electric.temperature_coefficient`, default 0 so constant), face value
by the same distance-weighted harmonic mean as ADR 0024 D2 (zero if either cell is an insulator, `sigma = 0`). Solved with the shared
sweep (zero flux, steady). Insulator cells (`sigma = 0`) decouple: their rows get a unit diagonal, `phi = 0`, no current and no heat.

Joule heat from face dissipation, so that the discrete power is exact: internal face with conductance `G_f = sigma_f |S|^2/(S.d)` carries
`G_f (phi_o - phi_n)^2` shared half and half between its two cells; a boundary face dissipates
`F_b (phi_P - phi_b)` in its owner, with `F_b` the current leaving the domain through that face (for a Dirichlet face `F_b = G_b (phi_P - phi_b)`, for a
prescribed-current face `F_b` is the prescribed value and `phi_b` the face value of the solution). Then
`sum_c Q_c V_c = - sum_b F_b phi_b` (power in = current times voltage, from the cell balances times `phi_c`) holds to round-off on orthogonal meshes. On non-orthogonal meshes the
explicit correction of the Laplacian is not part of the dissipation, so the identity is only claimed for orthogonal meshes (named gap).
The source handed to the energy equation is `Q_c / (rho cp)_ref` [K/s].

Settings (all additive, unset sentinels explicit):

```
[electric]  enabled=false, conductivity [S/m] (-1 = unset: error when enabled), temperature_coefficient [1/K] (0),
            reference_temperature [K] (-1 = unset; required when temperature_coefficient != 0), joule_heating = true
[solids.<n>] electrical_conductivity [S/m]  (-1 = use electric.conductivity, 0 = insulator)
[boundaries.<p>] electric = "insulated" (default) | "potential" | "current"
                 electric_potential [V]  (default -1e30 = unset; any real is a legal potential, so the sentinel is out of range)
                 electric_current [A]    (default -1e30 = unset; total current INTO the domain, uniform density over the patch)
```
At least one `potential` patch is required (located error at `electric.enabled`). Keys not used by the chosen kind are located errors.
Coupling: lagged (Picard), once per steady iteration or time step: potential from `T` of the previous iterate/level, then the energy solve
with the Joule source. First order in time for transient coupling (stated). `sigma(T) <= 0` or `1 + a(T-T_ref) <= 0` stops the run with a
located failure. Available with `solver.frozen_flow = true` (steady simple/simplec and transient piso/pimple) only; solved-flow paths are a
located error (named gap). Fields `electric_potential` [V], `joule_heat` [W/m^3], `electric_conductivity` [S/m]; a report kind
`electric_current` (current into the domain through a patch).

### D3 F-212: enthalpy-porosity melting and solidification

Enthalpy `H(T) = c T + L f(T)`, with `f(T) = clamp((T - T_s)/(T_l - T_s), 0, 1)` (linear liquid fraction between solidus and liquidus,
`T_l > T_s`), properties of both phases equal (`rho`, `cp`, `k` of `physics`; phase-dependent properties are a named gap). Per cell, implicit
Euler: `V (h(T) - h^n)/dt + (A_op T - b_op) = V S`, `h = H/cp` in K (`h(T) = T + (L/cp) f(T)`), `A_op` the diffusion operator of the shared FV
code with the energy boundary conditions. Solved by Newton on the monotone piecewise-linear `h`: the apparent capacity `C = 1 + (L/cp)/(T_l-T_s)`
inside the mush and 1 outside; the Newton step goes through the same FV matrix; backtracking on the 2-norm of the true nonlinear residual
(step halving, at most 8 halvings), at most 100 Newton iterations per step, convergence when the max-norm residual is below `1e-12` of the
sum of absolute terms. At convergence the discrete enthalpy balance of every cell holds exactly, so the global balance is an invariant,
not a tuned quantity. Fixed grid, no front tracking. Implicit Euler only (a located error for `bdf2`); the Stefan checks use cell centred values.
A published linearised latent-source scheme (Voller and Swaminathan) would also work; Newton on the enthalpy was chosen because its fixed point is
the exact discrete balance and needs no relaxation factor.

Momentum damping (Carman-Kozeny type, Voller and Prakash): `A(f) = C (1 - f)^2 / (f^3 + b)` [kg/m^3/s], handed to the momentum solver as the
implicit kinematic sink `A/rho` [1/s]. `C` (mushy constant) has NO default (error when damping is requested and unset): published values span
orders of magnitude and are case dependent; `b` defaults to `1e-3`. Delivered as a library function and verified in the steady SIMPLE
solver (which has `sink`); NOT wired into the settings tree because the transient PIMPLE solver has no sink, and it is owned by the
pressure-velocity core (same boundary as ADR 0024 D5). A solid fraction `f = 0` does not give exactly zero velocity in a segregated solver
(Rhie-Chow leak, ADR 0024 D5), so no "zero velocity in solid" check is registered.

Settings: `[phase_change] enabled=false, solidus [K], liquidus [K], latent_heat [J/kg] (-1 = unset each, errors when enabled),
momentum_constant [kg/m^3/s] (-1 = unset, unused in the tree), newton_max (100)`. Output field `liquid_fraction`. Tree availability:
`solver.frozen_flow = true` with transient piso/pimple, zero prescribed flux (the convection of latent heat is not implemented, so a non-zero
flux is a located error), no `solids`, no `electric`, `zone_sources` of kind `source` without linear part only (they enter through `S`).

Named gaps: melt flow coupled to the front (natural convection in the melt: needs the PIMPLE sink and buoyancy coupling), the published
gallium-melting cavity comparison, pull velocity, phase-dependent properties, a convected latent term, steady-state solidification, BDF2.

### D4 F-214 deferred

Not attempted in this wave unless F-212 finishes early; if attempted, it needs an explicit choice of the cross-flow electric field
(insulating or shorted Hartmann walls) and would register the profile shape and flow-rate relation for that choice. Default expectation: not
delivered, `todo` stays.

## Closed forms

**Series resistance with layers.** `R_tot = R_c + sum t_i/k_i`, heat flux `q = (T_1 - T_2)/(L_1/k_1 + R_tot + L_2/k_2)`, temperature jump
across the interface `q R_tot`.

**Ohm bar.** `div(sigma grad phi) = 0` in 1-D with end potentials `V, 0`: `I'' = sigma V/L` per unit area. For two layers
`J = V/(L_1/sigma_1 + L_2/sigma_2)`.

**Joule parabola.** `k T'' + sigma E^2 = 0`, `T(0) = T(L) = T_0`, `E = V/L`: `T - T_0 = (sigma E^2/(2k)) x (L - x)`, maximum `sigma E^2 L^2/(8k)`.

**Temperature-dependent resistivity.** `rho_e = rho_0 (1 + a theta)`, `theta = T - T_0 = T - T_ref`, constant `k`, constant current density `J`
(1-D), `theta(0) = theta(L) = 0`. `k theta'' + rho_0 J^2 (1 + a theta) = 0` gives `theta'' + m^2 theta = -m^2/a`, `m^2 = a rho_0 J^2/k`.
Particular solution `theta_p = -1/a`; symmetric homogeneous part: `theta = (1/a) [cos(m (x - L/2))/cos(m L/2) - 1]` (satisfies both ends; valid for
`m L < pi`). Voltage `V = int rho_e J dx = rho_0 J int cos(m(x-L/2))/cos(mL/2) dx = 2 rho_0 J tan(m L/2)/m` (since `1 + a theta =
cos(m(x-L/2))/cos(mL/2)`). For `a -> 0`, `theta -> m^2 x(L-x)/(2a) = rho_0 J^2 x(L-x)/(2k)`, the plain parabola.

**Lumped Joule heating with `sigma(T)` at fixed voltage.** `c dtheta/dt = E^2/(rho_0 (1 + a theta))` (adiabatic, uniform, `rho cp = c`):
`theta + a theta^2/2 = (E^2/(rho_0 c)) t`.

**Two-phase Neumann (Stefan) solution, equal properties.** Liquid `0 <= x < s(t)` wall at `T_w`, solid `x > s(t)` initially `T_i <= T_m`,
`alpha = k/(rho c)`, `s = 2 lambda sqrt(alpha t)`: `T_l = T_w - (T_w - T_m) erf(eta)/erf(lambda)`, `T_s = T_i + (T_m - T_i) erfc(eta)/erfc(lambda)`,
`eta = x/(2 sqrt(alpha t))`. Interface condition `q_l - q_s = rho L ds/dt` with `q_l = k (T_w-T_m) e^{-lambda^2}/(sqrt(pi alpha t) erf lambda)`,
`q_s = k (T_m-T_i) e^{-lambda^2}/(sqrt(pi alpha t) erfc lambda)` gives
`St_l e^{-lambda^2}/(sqrt(pi) erf lambda) - St_s e^{-lambda^2}/(sqrt(pi) erfc lambda) = lambda`, `St_l = c (T_w-T_m)/L`, `St_s = c (T_m - T_i)/L`.
One phase: `T_i = T_m`, `lambda e^{lambda^2} erf lambda = St_l/sqrt(pi)`. Solved by bisection in the test (independent of the code). The equation
is derived here; a retrieval tool returned the same two-phase form from general knowledge (not a primary source); marked unverified against
Carslaw and Jaeger / Alexiades and Solomon in `docs/reference-data.md`.

**Brinkman channel.** `nu u'' - kappa u + G = 0`, `u = 0` at `y = 0, 2H`, constant `kappa`: `u = (G/kappa)(1 - cosh(m(y - H))/cosh(m H))`, `m = sqrt(kappa/nu)`.

## Registered checks

Common: library tests use `rcfd_mesh::gen` meshes. Linear solves `tol_rel = 1e-13, tol_abs = 1e-300`. "dT" is the temperature difference across the
problem. Case tests go through the settings tree.

### G. F-215 (files `crates/rcfd-fv/tests/interface_w21a.rs`, `crates/rcfd-api/tests/thermal_extras_case.rs`)

* G1 layered interface through the tree: two solids `k = 1, 10`, `L = 0.05` each (box_2d 40x1, `L_x = 0.1`), interface `resistance = 0.01` plus layers
  `(t,k) = (0.002, 0.5), (0.001, 45)`, ends fixed 400 K / 300 K. `R_tot = 0.01 + 0.004 + 1/45000`. Cell temperature vs the exact piecewise-linear
  solution <= 1e-10 dT; extrapolated jump (as ADR 0024 E1) equals `q R_tot` to 1e-10 relative.
* G2 layers only (resistance unset `-1`) with the same layers: same limits (1e-10).
* G3 layered interface equals explicitly resolved layers (library, warped 1-D mesh with the layers as cells of `k = 0.5` and `45`, and the
  `Contact` with `R = sum t/k`): slab cell temperatures agree <= 1e-10 dT.
* G4 open connection, steady, through the tree: left solid `k = 1`, right solid `k = 10`, `L = 0.05` each, ends fixed 400 K and 300 K: every left cell
  is 400 K and every right cell 300 K to 1e-10 dT; the discrete flow through the interface <= 1e-12 of the flow `q A` of the same case with a closed
  interface and zero resistance.
* G5 open connection, transient (library, Euler, 50 steps, adiabatic ends, non-uniform initial temperature): per-side enthalpy `sum rho cp V T` constant
  to 1e-12 relative, and the interface flow is exactly zero (face conductance exactly 0).
* G6 default unchanged: existing `cht`, `thermal_bc`, `cht_case` tests pass without edits.
* G7 located errors, each asserted by path: `thermal_interfaces.i.layer_conductivity` (length mismatch / non-positive), `layer_thickness` non-positive,
  `resistance` unset without layers, `connection` unknown value, `connection = open` together with layers or finite resistance, open interface leaving a
  steady group with no fixed boundary.

### H. F-213 (files `crates/rcfd-fv/tests/electric.rs`, `crates/rcfd-api/tests/electric_case.rs`)

* H1 Ohm bar: box_2d 40x1, `L = 1`, `sigma = 100`, `phi = 10 / 0`. `phi` vs linear <= 1e-10 V relative to V; terminal current `I = sigma V A/L` <= 1e-10 relative;
  in/out currents sum to zero <= 1e-10 relative.
* H2 two-layer bar (`sigma = 100, 10`, halves, `V = 10`): `phi` at the interface and everywhere <= 1e-10 relative to V; current vs `V/(0.5/100 + 0.5/10)` <= 1e-10.
* H3 power identity: 16x16 box_2d and a graded orthogonal mesh, checkerboard `sigma in {1, 10}`, `phi = 1/0` on xmin/xmax, other sides insulated:
  `sum Q_c V_c = I V` <= 1e-10 relative.
* H4 Joule parabola (frozen flow, steady, `sigma = 100`, `V = 2`, `L = 1`, `k = 2`, ends 300 K, `rho cp = 1`): `E = 2`, `Q = 400`, peak 25 K. Peak-normalised max
  error at N = 20 <= 1e-2 (the roadmap acceptance); observed order from N = 20, 40, 80 >= 1.8; boundary heat outflow vs `sum Q V` and vs `I V` <= 1e-9.
* H5 `sigma(T)` bar: `rho_0 = 1`, `a = 0.1`, `k = 1`, `L = 1`, `T_ref = 300`, ends 300 K, prescribed current density `J = 4` into the domain at xmin, `phi = 0` at xmax
  (so `m = sqrt(a rho_0 J^2/k) = 1.2649`, `m L = 1.265 < pi`; peak `theta` about 2.4 K). Coupling iterated to a temperature change below 1e-11. Peak-normalised max
  temperature error at N = 40 <= 5e-3; observed order (20, 40, 80) >= 1.8; voltage `phi(xmin)` vs `2 rho_0 J tan(mL/2)/m` relative <= 5e-3 at N = 40.
* H6 transient: (i) constant `sigma = 1`, `V = 1`, `L = 1`, adiabatic, `T_0 = 300`, `rho cp = 1`, 20 implicit-Euler steps of 0.1: `T - 300` vs the exact 2.0 within 1e-10 (the source is uniform and exactly integrated by Euler). (ii) `a = 0.5`, `rho_0 = 1`, `E = 1`, `c = 1`, `t = 2`: exact `theta = 2(sqrt3 - 1) = 1.4641016`; lagged coupling error at 40 steps <= 2e-2 relative, observed order from 40, 80, 160 steps >= 0.9.
* H7 insulator: box_2d 20x20, lower half `sigma = 0`, upper `sigma = 100`, `phi = 10/0`: conducting `phi` linear <= 1e-10 relative; Joule heat in insulator cells exactly 0; face conductance across insulator faces exactly 0.
* H8 H4 and H5 through the settings tree with the same limits; `electric_current` report matches `I` of H1 through the tree <= 1e-9 relative.
* H9 located errors asserted by path: `electric.conductivity` unset, `electric.reference_temperature` unset with a coefficient, no potential patch (`electric`), `boundaries.p.electric` unknown, `boundaries.p.electric_potential` / `electric_current` unset for their kinds or set with `insulated`, `solids.s.electrical_conductivity` (negative other than -1), `electric` without `solver.frozen_flow`, without `energy.enabled`, `sigma(T)` non-positive at run time (failure message).
* H10 baseline hashes unchanged.

### P. F-212 (file `crates/rcfd-fv/tests/solidification.rs`)

Common: `c = 1, rho = 1, k = 1` (alpha = 1), `T_m = 0` in test units (the library has no temperature positivity requirement), mush half-width `epsilon = 0.002`
(`T_s = -epsilon`, `T_l = +epsilon`), `L = 2`, wall at x = 0 fixed, domain `[0, 6]` with the far end fixed at the initial solid value (truncation error
`erfc(3)` ~ 2e-5 of the driving temperature), `box_2d` N x 1. Start from the analytic profile at `t_0 = 0.1` (cell-centre values; disclosed: the front has no thickness
at t = 0), implicit Euler with `dt = 0.5 h` to `t_end = 1.0`. Interface position from the liquid content `s_num = sum f V / A`.
Grids N = 60, 120, 240, 480.

* P1 one-phase melting: `T_w = +1` (`St_l = 0.5`), solid at `T_s`. |s_num - s|/s at N = 240 <= 3 percent (roadmap acceptance); observed order between successive grids (60-120-240-480) >= 0.9 (first order is the expected behaviour of a fixed-grid enthalpy method; the floor leaves margin and is a limit fixed before the run, not derived).
* P2 two-phase melting: `T_w = +1`, `T_i = -0.5` (`St_s = 0.25`): same limits.
* P3 sensible limit (`L = 0`): erf solution `T = T_w + (T_i - T_w) erf(x/(2 sqrt t))` from `t_0 = 0.1`, `T_w = 1`, `T_i = 0`: max |T - T_exact|/dT at N = 240 <= 5e-3 (engineering bound fixed before the run).
* P4 enthalpy balance per step: `|sum V (H^{n+1} - H^n) - dt * (heat entering through the boundary faces)| / max(|sum V dH|, |dt Q_in|)` <= 1e-10, all steps of P1, P2, P8, all grids.
* P5 `0 <= f <= 1` and per cell `f^{n+1} - f^n >= -1e-10` in the melting runs P1, P2; `f^{n+1} - f^n <= 1e-10` in the freezing run P8.
* P6 Newton: no step reaches the 100-iteration cap in any run; the maximum is reported.
* P7 the transcendental equation residual of the bisection root <= 1e-12 (check of the test's own reference).
* P8 two-phase solidification by symmetry: wall `T_w = -1`, liquid initial `+0.5`; position from `sum (1 - f) V / A`; same limits as P2 (equal properties make it the mirror image).

### K. F-212 momentum damping (library, `crates/rcfd-fv/tests/solidification.rs`)

* K1 `A(f)` closed values: `f = 0.5`, `C = 1e5`, `b = 1e-3` gives `C * 0.25/(0.125 + 0.001) = 198412.698...`; `f = 1` gives exactly 0; `f = 0` gives `C/b`; relative <= 1e-14. The sink is `A/rho`.
* K2 Brinkman channel with SIMPLE, periodic in x, body acceleration `G = 1`, `nu = 0.1`, `rho = 1`, uniform `f = 0.5`, `b = 1e-3`, `C` such that `kappa = 3.6` (so `m = 6`, `m H = 3` with `H = 0.5`, walls at y = 0 and 1): centre-normalised max error at ny = 20 <= 1e-2; observed order from ny = 20, 40, 80 >= 1.8.

### T. F-212 through the settings tree (`crates/rcfd-api/tests/phase_change_case.rs`)

* T1 P1 through the tree (frozen flow, zero velocity, transient, `phase_change` section): position within the same 3 percent at N = 240 and equal to the library run to 1e-10 relative.
* T2 `liquid_fraction` field is available, in [0, 1], and equals the clamp of `T` between solidus and liquidus to 1e-14.
* T3 located errors by path: `phase_change.solidus` / `liquidus` / `latent_heat` unset, `liquidus <= solidus`, `solver.time_scheme = bdf2`, steady algorithm, non-zero prescribed flux, combined with `solids`, combined with `electric`, `fixed_value` zone sources, `solver.frozen_flow = false` (solved flow; named gap).

### R. Regression

* R1 `baseline_hashes` passes unmodified after each slice. R2 the whole workspace builds, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --no-fail-fast` pass on a `git archive` of the final commit.

## Not delivered (expected; reconciled with results at the end)

Thermal capacity of interface layers and in-plane shell conduction; solution-dependent open/closed connections; mapped interfaces between meshes; Joule heating
with solved flow, on non-orthogonal meshes (exact power identity), with electric insulation by thin layers, magnetic induction; coupled melt flow, the gallium cavity
comparison, pull velocity, phase-dependent properties, damping in the settings tree; F-214 (Lorentz force, Hartmann flow).

## Addendum: results and deviations (written after the runs; registered limits above were not changed)

### F-215 (commit 36c6cdb)

G1 error/dT 2.2e-14, jump 1.0e-14; G2 2.0e-14 / 5.4e-14; G3 within 1e-10; G4 error/dT 2.3e-14, end-wall flows 9.6e-14 and 0 against the 1e-12 limit; G7 all located errors found. **G5 MISSED at the registered dt = 0.01**: per-side enthalpy drift 2.0e-12 and 1.8e-11 against 1e-12; the cell Fourier number is about 8e3 and the drift is solver round-off (independent of the linear tolerance, 2.5e-13 / 3.8e-14 at dt = 1e-3). Kept as the ignored `g5_open_connection_transient_conserves_each_side_known_miss`; the corrected set-up `g5_v2_...` (dt = 1e-3, chosen after seeing the miss, the ADR did not fix dt) passes.
Deviation: the tree G4 measures the end-wall heat flows as the proxy for "no flow through the open interface"; the zero interface face conductance is asserted exactly in the library check G5.

### F-213 (commit accd924)

H1 phi 1.4e-15, current 1.1e-14, in/out 8.7e-15; H2 4.6e-15 / 2.3e-15; H3 3.6e-16 uniform, 1.8e-14 graded; H4 errors N=20/40/80 2.5e-3, 6.25e-4, 1.56e-4 (orders 2.000, 2.000), balances <= 6.5e-14; H5 errors 2.157e-3, 5.30e-4, 1.314e-4 (orders 2.025, 2.012), voltage 5.8e-4, 1.4e-4, 3.5e-5; H6(i) 1.999999999998 (exact 2), H6(ii) 5.47e-3, 2.72e-3, 1.36e-3 (orders 1.008, 1.004); H7 1.6e-15 with exact zeros; H8 tree equals the library (same numbers, orders 2.000) and current rel <= 2.8e-14; H9 12 located-error cases and the runtime message naming `electric.temperature_coefficient`. All met.
Deviations: (a) the ADR text names a report kind `electric_current`; the implementation exposes the boundary field `electric_current_density` read by the existing `area_integral` report. (b) The H8 current check compares with the analytic current `sigma V A / L` of the H4 bar, not "the I of H1". (c) The 2D mesh depth scales with the cell size, so the patch area depends on N; the tree tests prescribe the total current `J A` from the built simulation. (d) The first attempt of the tree H5 test passed a current density as the total current and diverged; this set-up defect was corrected before any pass was recorded. (e) The identity is exact on orthogonal meshes only, as registered.

### F-212 (library commit accd924, tree wiring in the following commit)

Set-up: the tree needs absolute temperatures, so the tree runs use Tm = 300 K (shift-invariant; the library run compared with it uses the same shift). The T2 reference clamp is computed from the literal solidus and liquidus (an earlier version through `T - Tm + eps` had 2.4e-12 round-off from the shift: test defect, corrected).
P1 first set-up started the solid at Tm (mid-mush, f = 0.5 everywhere), i.e. not "solid at Ts": defect; kept as the ignored record `p1_one_phase_melting_setup_defect_record`; corrected test with the solid at Ts. Results (position rel. error N = 60/120/240/480): P1 2.05e-3, 3.00e-3, 2.88e-3, 1.46e-3; P2 and P8 (identical numbers) 1.86e-2, 3.50e-3, 6.3e-5, then 5.1e-3 at N=480 with a failed Newton solve. P3 max error/dT 1.56e-3 (limit 5e-3) met. K1 values match to round-off. P7 root residual met.
**Misses (ignored `*_known_miss` tests, criteria unchanged):**
* P1 successive-order criterion >= 0.9: orders -0.55, 0.06, 0.98; the position error is 0.1 to 0.3 percent, at the level of the mush and time-step offsets, so it does not fall monotonically. The 3 percent limit and the invariants P4 to P6 hold at all four levels.
* P2 and P8 at N = 480: Newton hits the 100-iteration cap on some steps (P6), so the per-step balance (P4, 1.06) and monotonicity (P5, 0.21) fail there. Diagnosed with the residual history: the iteration falls into a period-20 limit cycle of the semismooth Newton on the piecewise-linear `h(T)` (mush half-width 0.002 against a cell of 0.0125, so a cell crosses the whole mush in one step) and the backtracking cannot break it. At N = 60, 120, 240 all of P2/P8 hold (evidence tests `*_n60_to_240`, which do not replace the registered four-level check). The remedy would be a bracketing or chord-slope safeguard of the nonlinear solver: not implemented (named gap).
* K2 ny = 20 centre-normalised error 1.0040e-2 against 1e-2 (orders 1.84, 1.92 meet >= 1.8). Not tuned. The convergence loop of the test stops on `max_du` and continuity because the normalised momentum residual of the identically-zero v-equation is noise.
T1 (tree vs Neumann position rel 1.30e-3; tree vs library difference 0.0) , T2 (difference 0.0) and T3 (13 located-error cases) are met. Deviation: the non-zero prescribed flux check is in `PhaseState::from_case`; a prescribed flux needs `initial.velocity` with matching inlet/outlet patches.

### Reconciliation of "Not delivered"

F-214 was not started. The damping is library-only (K1, K2); melt flow is not coupled. No gallium comparison. Documents: `docs/case-schema.md`, `docs/reference-data.md` (Neumann solution UNVERIFIED against a primary text).
