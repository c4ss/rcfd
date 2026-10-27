# ADR 0009: Transient variable density and viscous heating in the pressure-based kernel

Status: accepted. Design, checks and numeric limits were written before any transient code existed. They are not
changed after a run; a miss becomes an `#[ignore]`d `*_known_miss` test and a number in the roadmap entry.
Corrected before any transient code ran (second commit): T1r heat source made self-consistent, inner stopping rule
normalised by the absolute pressure, relaxation factors registered, T4 end time lengthened to 20 s (filling-time
estimate), `max_courant` rejected, section 5 (adiabatic-wall energy BC) added.
Roadmap items F-050 (energy equation, ideal-gas density) and F-061 (pressure-based compressible extension). Builds on
ADR 0005 (steady kernel) and ADR 0007 (real gas).

## Decision

### 1. Same solver, transient terms switched on by a time state

`rcfd-fv::varden::VarDensitySolver` gets an optional time state (`TransientSettings { scheme, max_inner, tol }`,
`advance(dt)`). With no time state, `iterate()` executes the same floating-point operations as before: every new
term is added inside `if transient` branches, never by reordering existing sums. Steady constant-density and steady
ideal-gas results stay bit-identical (checks Q1 and Q2 below).

A time step is a SIMPLE-inner iteration (the existing `iterate()`: momentum predictor, mass/pressure equation,
correction, energy, density) repeated at fixed time level until the iterate stops changing. PISO is not used: the
existing inner iteration already carries the deferred-correction and transonic terms and re-using it keeps one code
path for steady and transient. No temporal order is lost by this (converged inner iterations solve the fully coupled
implicit step); the price is the number of inner iterations (about 10 to 30 per step).

### 2. Discretisation in time

Euler and variable-step BDF2 (Ferziger and Peric, Computational Methods for Fluid Dynamics, 3rd ed., sec. 6.3.2;
Euler on the first step). With `r = dt/dt_old`, the derivative of a quantity `phi` at the new level is
`ddt(phi) = c0 phi + c1 phi^n + c2 phi^(n-1)` with Euler `(1/dt, -1/dt, 0)` and BDF2
`((1+2r)/((1+r)dt), -(1+r)/dt, r^2/((1+r)dt))`; `c0 + c1 + c2 = 0`.

* Continuity: `sum_f m_f + V ddt(rho) = 0`. In the pressure equation `rho_c(p') = rho* + psi (p' - p*)` for the
  density of the new level, so the matrix diagonal gains `V c0 psi` and the right side
  `-V (c0 rho* + c1 rho^n + c2 rho^(n-1)) + V c0 psi p*`. In low-Mach mode the density follows the temperature
  iterate and the thermodynamic pressure; `ddt(rho)` is a known source.
* Momentum: the steady kernel removes the continuity error with `Sp(div m) u`. The transient form removes the full
  error `div m + V ddt(rho)`, so the convective-conservative form `ddt(rho u) + div(m u) - u (ddt rho + div m)`
  becomes `V [c1 rho^n (u^n - u) + c2 rho^(n-1) (u^(n-1) - u)] + (m . grad) u`: diagonal `-V (c1 rho^n + c2 rho^(n-1))`
  (positive, equal to `c0 rho V` when the density is steady), source `-V (c1 rho^n u^n + c2 rho^(n-1) u^(n-1))`.
  The matrix stays an M-matrix whatever the iterate's continuity error.
* Energy: the same form for `h0` (compressible mode) or `cp T` (low-Mach mode), plus the work terms
  `V ddt(p)` (compressible, total enthalpy: `rho Dh0/Dt = dp/dt + div(k grad T) + ...`) and, in a closed low-Mach
  domain, `V dP_th/dt` (Le Quere et al. 2005 eq. 5), plus a uniform volumetric heat source
  `settings.heat_source` (W/m^3).
* Transient Rhie-Chow (Majumdar 1988; Moukalled, Mangani, Darwish, The Finite Volume Method in CFD, 2016, sec. 15.9):
  the face flux gets `rAU_f [(-c1)(m^n_f - rho^n_f u^n_f . S) - c2 (m^(n-1)_f - rho^(n-1)_f u^(n-1)_f . S)]` so the
  old-level flux, not the interpolated old velocity, enters (time-step independent result, no pressure
  checkerboard at small dt). Same device as `pimple.rs` for the constant-density kernel.
* Closed domain, compressible mode: the pressure level is set by the discrete mass balance of the step,
  `M^(n+1) = (-sum_boundary m_f - c1 M^n - c2 M^(n-1))/c0` (exact consequence of summing the cell equations), which is
  `M^n` to round-off when no mass crosses the boundary. The pinned cell of the steady kernel is not used in the
  compressible closed case (the `psi c0` diagonal makes the system non-singular); the low-Mach closed case keeps it.

### 3. Viscous heating (compressible and low-Mach, switchable `viscous_heating`)

Newtonian stress `tau = mu (grad u + grad u^T - (2/3)(div u) I)` from the cell gradients (Green-Gauss with the boundary
values). Compressible mode (total enthalpy, `h0 = h + |u|^2/2`): the explicit source `div(tau . u)` (viscous work, so
that it includes both the dissipation `Phi = tau : grad u` and the work of the viscous force) assembled from face values
`(tau . u)_f . S` (linear interpolation; boundary faces: cell `tau` with the boundary velocity, i.e. the work of a moving
wall appears). The existing energy equation keeps the conduction `div(k grad T)` exactly; the viscous work was absent.
Low-Mach mode (temperature): the dissipation `Phi` as a cell source divided by `cp`. Default off; off gives bit-identical
steady results. Not included: the work of the body force on the flow (`rho g . u`, absent from the steady kernel too),
radiation, variable `cp`.

### 4. Case wiring

`solver.algorithm = "pimple"` with `fluid.density = ideal_gas | real_gas` runs transient: `solver.dt`,
`solver.end_time`, `solver.time_scheme` (euler | bdf2) are the existing keys; new `fluid.inner_iterations` (maximum
SIMPLE-inner iterations per step) and `fluid.inner_tolerance`; `energy.viscous_heating` (bool); `energy.heat_source`
(W/m^3, uniform). `piso` and `coupled`/`compressible` remain validation errors; `solver.max_courant > 0` is a located
error for the variable-density solver (the Courant helper takes a volumetric flux; fixed `dt` only). Located validation errors for the combinations that do not exist (viscous heating without
`fluid.density`, `dt` with steady SIMPLE is still ignored as before).

## Pre-registered checks and limits

Relaxation factors (registered, not tuned): T1, T1r, T2, T5, T6 use the `VdSettings` defaults `alpha_u = 0.9`,
`alpha_p = 1.0` (SIMPLEC), `alpha_rho = 1` and `alpha_h = 1.0`; T3 all 1.0; T4 as stated below.

Air `R = 287`, `gamma = 1.4`, `cp = gamma R/(gamma-1)`, `p0 = 101325 Pa`, `T0 = 300 K` unless stated. Inner stopping
rule of every test: stop when `max(|dp|/max P, |dT|/max T, |du|/max a) < inner_tol` or `max_inner` is reached, with `P`
the absolute pressure (`p` in compressible mode, `P_th + p` in low-Mach mode, where `p` is the dynamic pressure);
`inner_tol` and `max_inner` are given per check and are part of the registration.

**Q1 No regression, constant density.** `cargo test -p rcfd-api --test baseline_hashes` unchanged and passing.
**Q2 No regression, ideal gas.** `varden_ideal_hash` (REGISTERED hash `0xbbedca02b6f39a6d`) unchanged and passing, and
all existing `varden_*` tests pass unchanged.

**T1 Closed box, uniform heating (sealed cavity).** `box_2d` 6 x 6, 0.1 m square, no-slip adiabatic walls, `T0 = 300 K`,
`p = p0`, `u = 0`, `heat_source q = rho cv * 100 K/s` (`cv = cp - R`, `rho = p0/(R T0)`), 20 steps of `dt = 0.05 s`
to `t = 1 s`. Analytic: no motion, uniform `rho = rho0`, `rho cv dT/dt = q` so `T = T0 + 100 t`, `p = rho0 R T`
(ideal-gas law at constant density). Source: first law for a closed fixed volume; ideal-gas law. Linear in time, hence
exact for Euler and BDF2 up to the inner-iteration tolerance. Run for Euler and BDF2, compressible mode, and for
low-Mach mode (`P_th` is then the pressure). `inner_tol = 1e-12`, `max_inner = 60`. Limits at every step end:
`|p/p_analytic - 1| <= 1e-6` (compressible: cell pressure; low-Mach: `P_th`), `|T/T_analytic - 1| <= 1e-6`,
`|M/M_0 - 1| <= 1e-11`, `max|u| <= 1e-8 m/s`, spatial uniformity of `T` and `p` to `1e-9` relative.
**T1r Same with the real gas** (CO2, Peng-Robinson, `p0 = 8 MPa`, `T0 = 330 K`, `heat_source = 7.0e6 W/m^3`, about a 40 K rise
in 1 s with `rho` about 200 kg/m^3 and `cv` about 0.8 kJ/kg/K; 20 steps of `dt = 0.05 s`): at constant density `rho (u_end - u_0) = q t` with the internal energy
`u = h(T,p) - p/rho` (first law); limits: relative energy balance error `<= 1e-6`, `|p/p_eos(rho0, T) - 1| <= 1e-6`,
`|M/M_0 - 1| <= 1e-11`.

**T2 Temporal order of accuracy (manufactured heating).** The box of T1 (compressible mode, BDF2 and Euler) with
`q(t) = q0 sin(2 pi t/t_end)`, `q0 = rho cv * 100 K/s`. Exact solution of `rho cv dT/dt = q(t)`:
`T(t) = T0 + (q0 t_end/(2 pi rho cv)) (1 - cos(2 pi t/t_end))` (the source of a step is evaluated at the new time
level). `t_end = 1 s`; steps `n = 10, 20, 40, 80`; error `e_n` = the maximum over the step ends of
`|T - T_exact|` (the whole history, not only the end point, where the solution returns to `T0`). `inner_tol = 1e-12`, `max_inner = 60`. Observed order
`log2(e_40/e_80)`: Euler in `[0.90, 1.10]`, BDF2 in `[1.85, 2.15]`. (The BDF2 first step is an Euler step; its local
error is `O(dt^2)`, which keeps the global order two.)

**T3 Standing acoustic wave in a closed tube (spatial coupling, temporal order, frequency).** Tube `L = 1 m`, 200 x 1
cells (`box_2d`, unit depth 0.01 m), slip (symmetry) side walls, rigid end walls, adiabatic, `mu = 1.8e-5`, `k` from
`Pr = 0.71`. Initial state `p = p0 (1 + eps cos(pi x/L))`, `eps = 1e-4`, isentropic `T = T0 (p/p0)^((gamma-1)/gamma)`,
`u = 0` (a standing wave at its pressure maximum). Linear theory (Kinsler, Frey, Coppens, Sanders, Fundamentals of
Acoustics, 4th ed., sec. 9.2 / 10 (closed-closed tube); Landau and Lifshitz, Fluid Mechanics, sec. 64):
`p'(x,t) = eps p0 cos(pi x/L) cos(omega t)`, `omega = pi a/L`, `a = sqrt(gamma R T0) = 347.2 m/s`, period `Tp = 5.76 ms`.
The Stokes-Kirchhoff damping of the bulk is `1.4e-4 /s` (`Gamma = omega^2 [4mu/3 + (gamma-1)k/cp]/(2 rho a^2)`), 3e-6
over four periods, so no decay is expected from the physics and the amplitude is a measure of the scheme.
`inner_tol = 1e-10`, `max_inner = 40`, linear-solver tolerances tightened to `1e-10` relative, `central`
energy/momentum convection, `alpha_u = alpha_p = alpha_h = 1`.
* T3a frequency: the `p'` time series of the cell at the closed end (`x = dx/2`), zero crossings by linear
  interpolation, `omega_num` from the mean spacing of the first 8 crossings (7 half periods); 100 steps per period,
  4 periods: BDF2 `|omega_num/omega - 1| <= 1.0 %`, Euler `<= 1.0 %`.
* T3b amplitude: the `p'` amplitude at `t = 4 Tp` over the initial amplitude (cell-end value at the extrema): BDF2 in
  `[0.98, 1.001]`; Euler reported only (first-order damping of the implicit Euler step is expected, about 0.45).
* T3c temporal order: `L2` error of `p'(x, t = Tp)` normalised by `eps p0`, against the linear solution above, with
  `n = 50, 100, 200` steps per period on the same 200-cell mesh: observed order `log2(e_100/e_200)` Euler in
  `[0.85, 1.15]`, BDF2 in `[1.80, 2.20]`.

**T4 Differentially heated cavity, time-transient approach to the validated steady result.** The T1 case of
ADR 0005 (c): low-Mach, `T0 = 600 K`, `epsilon = 0.6`, `Ra = 1e6`, `Pr = 0.71`, constant properties, 96 x 96 graded 8,
`linear_upwind`, SIMPLEC, started from rest at uniform `T0`, walls `T_h`/`T_c` imposed at `t = 0`. BDF2, `dt = 0.02 s`
(the buoyant time scale `sqrt(L/(g epsilon))` is 0.107 s, the convective turnover time of the steady flow about 0.06 s),
run to `t = 20 s` (the Patterson and Imberger 1980, J. Fluid Mech. 100, 65, boundary-layer filling time is of order
`L^2/(kappa Ra^(1/4))` = 2 s here, so 20 s is ten of them), `inner_tol = 1e-6`, `max_inner = 20`, `alpha_u = 0.8`,
`alpha_h = 0.9` (the steady values). Limits at
`t = 20 s`: hot and cold wall `Nu` within 1.0 % of 8.85978, `P_th/P0` within 0.3 % of 0.856338 (the steady limits of
ADR 0005), hot/cold heat flow equal to `2e-3` relative (transient, a stored-energy rate remains), and stationarity:
`|Nu_hot(20 s) - Nu_hot(15 s)|/Nu_hot <= 1e-3`. Heavy (`#[ignore]`).

**T5 Steady limit of a transient run equals the steady kernel.** The ADR 0005 (a1) subsonic duct on 100 x 6 cells,
compressible mode, stagnation inlet, `linear_upwind`, SIMPLEC. Reference: the steady kernel, 3000 iterations
(its own floor `res_u ~ 1e-5`, as recorded). Transient: BDF2, `dt = 1 ms` (acoustic transit time 6 ms, flow-through time
0.04 s), from uniform initial state `u = 30 m/s`, `p = 0.985 p0`, 400 steps, `inner_tol = 1e-8`, `max_inner = 30`.
Limits: `max |Mach_transient - Mach_steady| <= 2e-3`, `max |p_transient - p_steady|/p0 <= 5e-4`, `|mdot_transient
- mdot_steady|/mdot_steady <= 1e-3` at inlet and outlet, in/out mass flow equality of the transient run `<= 1e-6`.

**T6 Couette flow with viscous heating (analytic temperature profile).** Plane Couette flow between a stationary wall
(`y = 0`) and a wall moving at `U = 100 m/s` (`y = H = 0.01 m`), constant `mu = 1.8e-5`, `k = 0.026`, `x`-invariant.
Velocity `u = U y/H` solves the momentum equation (`mu u'' = 0`, `p` uniform); the energy equation reduces to
`k T'' + mu (du/dy)^2 = 0` (the viscous work `d/dy(tau_yx u) = mu u'^2` plus conduction; Bird, Stewart, Lightfoot,
Transport Phenomena, 2nd ed., sec. 10.4 and 10.5, "Couette flow with viscous heating"; Schlichting and Gersten,
Boundary-Layer Theory, 9th ed., sec. 13.3). Derivation: `T'' = -mu U^2/(k H^2)` constant, two integrations.
Case A, both walls isothermal `T_w = 300 K`: `T - T_w = (mu U^2/(2k)) (y/H)(1 - y/H)`, maximum `mu U^2/(8k) = 0.865 K`.
Case B, bottom wall isothermal `T_w`, top wall adiabatic (`T'(H) = 0`): `T - T_w = (mu U^2/(2k)) (y/H)(2 - y/H)`,
maximum at the top `mu U^2/(2k) = 3.46 K`. Heat leaving through the walls: A each wall `mu U^2/(2H)` per unit area, B the
bottom wall `mu U^2/H`, top wall zero; the total is the power `mu U^2/H` of the moving wall per unit area. Numerics:
`box_2d` 4 x 40 (uniform), `u = U y/H` imposed on the `xmin`/`xmax` faces (per-face profile), `T` zero gradient there,
no-slip walls with `u = 0`/`U`, start `u = U y/H`, `T = T_w`, BDF2, `dt = 0.5 s`, 40 steps (the slowest mode of case B has
time constant `4 rho cp H^2/(pi^2 k) = 1.8 s`, `e^-11` at 20 s), `inner_tol = 1e-10`, `max_inner = 40`; compressible
mode (total-enthalpy, viscous work) and low-Mach mode (dissipation). Limits at the end: maximum
`|T - T_analytic|/(mu U^2/(2k)) <= 1 %` at the cell centres, wall heat flow (`heat_flow_out`) within 1 % of the analytic
values above (B top wall: absolute value `<= 1e-3` of the bottom one), `viscous_heating = false` leaves
`max |T - T_w| <= 1e-6 K` (switch check).

**T7 Case file.** A closed-box heating case and a Couette case (`velocity_expression_x = U y/H` on the `xmin`/`xmax`
patches, evaluated once at start: the expression is time independent) through the case schema (`solver.algorithm = pimple`,
`fluid.density = ideal_gas`) reproduce the T1 (`|p/p_analytic - 1| <= 1e-6`) and T6 A (`1 %`) limits. Validation: `solver.algorithm = piso`
with `fluid.density`, `energy.viscous_heating` or `energy.heat_source` without `fluid.density`, and
`fluid.inner_iterations < 1` give located errors; every baseline case file still validates (`baseline_hashes`).

### 5. Energy boundary condition at an adiabatic wall (pre-existing steady-kernel limitation, found before any run)

For an adiabatic (zero-gradient `T`) patch the steady kernel prescribes `dh0/dn = cp dT/dn = 0` and, in the same
equation, subtracts the kinetic-energy diffusion `Gamma (K_b - K_c)/dn |S|` on every boundary face (it turns the
`h0` diffusion into the conduction `k grad T`). On a no-slip wall that moves tangentially this leaves a spurious heat
flux `~ (k/cp) U^2/H`, about 1.4 times the viscous heating of case T6 B. The correct condition for `k dT/dn = 0` is
`dh0/dn = cp dT/dn + (K_b - K_c)/dn`. It is applied only when the solver is transient or viscous heating is on, because
slip faces of the warped steady nozzles have `K_b != K_c` and applying it there would change Q2. The steady kernel keeps
this limitation (stationary no-slip adiabatic walls also see `Gamma K_c/dn`, small at low Mach number).

### Not claimed

No shock-tube, choked-transient or contact-discontinuity benchmark: a pressure-based, non-conservative-energy,
first/second-order scheme without shock capturing is not validated for them, and none is attempted. No claim of
acoustic accuracy beyond the standing wave above (linear, 200 cells per wavelength, fixed amplitude `1e-4`). Transient
turbulence, species, radiation and time-varying boundary expressions with the variable-density solver are not included.

## Results (measured after the code existed; criteria above unchanged)

Release builds, `cargo test --release -p rcfd-fv --test varden_transient --test varden_transient_duct --test
varden_transient_cavity -- --include-ignored` and `-p rcfd-api --test transient_variable_density`. Steady results are
bit-identical: `varden_ideal_hash` (0xbbedca02b6f39a6d), the existing `varden_*` tests and `baseline_hashes` pass
unchanged (Q1, Q2 MET).

| Check | Registered limit | Measured | Result |
|-------|------------------|----------|--------|
| T1 closed box, compressible Euler / BDF2 | p, T rel. error <= 1e-6; mass <= 1e-11; \|u\| <= 1e-8; uniformity <= 1e-9 | p 2.9e-10 / 1.7e-10; mass ~2e-15; \|u\| <= 4e-10; non-uniformity ~4e-15 | PASS |
| T1 closed box, low-Mach Euler / BDF2 | same | p 3.3e-12 / 4.8e-12; same mass, \|u\|, uniformity | PASS |
| T1r CO2 Peng-Robinson, Euler / BDF2 | energy balance <= 1e-6; p vs EOS <= 1e-6; mass <= 1e-11 | 1.2e-10 / 2.1e-10; 1e-15; 1.7e-15 (T_end 371.85 K) | PASS |
| T2 order, Euler | [0.90, 1.10] | 1.019 (errors 5.445, 2.631, 1.283, 0.633 K) | PASS |
| T2 order, BDF2 | [1.85, 2.15] | 1.989 (4.081, 1.132, 0.2915, 0.0734 K) | PASS |
| T3a frequency, BDF2 / Euler | \|err\| <= 1 % | -0.132 % / -0.133 % | PASS |
| T3b BDF2 amplitude after 4 periods | [0.98, 1.001] | 0.99157 (Euler 0.4529, reported only) | PASS |
| T3c order, Euler | [0.85, 1.15] | 0.922 (errors 0.2298, 0.1270, 0.0670) | PASS |
| T3c order, BDF2 | [1.80, 2.20] | 1.34 (errors 1.114e-2, 2.99e-3, 1.18e-3) | MISS (`t3c_order_bdf2_known_miss`) |
| T4 cavity Nu hot / cold | within 1 % of 8.85978 | 8.86759 / 8.86806 (+0.09 %, +0.09 %) | PASS |
| T4 P_th/P0 | within 0.3 % of 0.856338 | 0.856036 (-0.035 %) | PASS |
| T4 hot/cold heat-flow imbalance | <= 2e-3 | 5.3e-5 | PASS |
| T4 stationarity Nu_hot(20 s) vs (15 s) | <= 1e-3 | 4.8e-5 | PASS |
| T5 max \|dMach\| (400 steps) | <= 2e-3 | 6.04e-4 | PASS |
| T5 max \|dp\|/p0 | <= 5e-4 | 1.88e-4 | PASS |
| T5 mass flow vs steady kernel, inlet / outlet | <= 1e-3 | 1.64e-3 / 1.62e-3 | MISS (`t5_transient_steady_limit_mass_flow_known_miss`) |
| T5 in/out equality of the transient run | <= 1e-6 | 1.56e-5 | MISS (same test) |
| T6 Couette A (both walls isothermal), compressible / low-Mach | profile <= 1 % of mu U^2/(2k); wall heat flow within 1 % | 1.56e-4 / 1.56e-4; 1.00000 of mu U^2/(2H) on both walls | PASS |
| T6 Couette B (top adiabatic), compressible / low-Mach | profile <= 1 %; bottom flow within 1 %; top <= 1e-3 of bottom | 1.56e-4 / 1.56e-4; 1.00000 of mu U^2/H; top 0 | PASS |
| T6 switch off (`viscous_heating = false`) | max \|T - T_w\| <= 1e-6 K | 9.9e-11 K (compressible), 0 (low-Mach) | PASS |
| T7 case file, closed box (both modes) | \|p/p_analytic - 1\| <= 1e-6 | 1.3e-10 (compressible), 4.8e-12 (low-Mach) | PASS |
| T7 case file, Couette A (both modes) | 1 % | 1.56e-4 (both) | PASS |
| T7 located validation errors | piso, `inner_iterations < 1`, `max_courant`, `viscous_heating` / `heat_source` without `fluid.density` | all located | PASS |

### Misses and what is known about them

* **T3c BDF2 (1.34 against [1.80, 2.20]).** The ADR expected a spatial floor of 1e-5 at 200 cells; that estimate was
  wrong. Post-hoc (after the miss, not a gate): the 800-cell run gives the same errors, so the floor is not spatial;
  with `ly = 0.1`, or with a zero-gradient `u` on the y faces, the BDF2 order is 2.07 to 2.15 (errors 1.05e-2, 2.38e-3,
  5.7e-4). Cause: the pressure-equation diagonal `a_P` is the mean of the component diagonals, and on the one-cell-high
  strip of the registered case the Dirichlet normal component at the symmetry planes adds a viscous term `~ mu/ly^2` to
  the y diagonal, which inflates `a_P` and slightly damps the acoustic mode (about 1e-3 amplitude loss per period at
  `ly = 0.01`). It is a pre-existing property of the kernel on thin strips; the kernel was not changed for it.
  (`t3c_order_bdf2_posthoc_tall_strip` records the `ly = 0.1` number.)
* **T5 mass-flow limits.** At 400 steps (0.4 s, ten flow-through times) the transient run is still relaxing: the
  mass flow in and out is 0.1159 kg/s against 0.1161 steady and the in/out difference is 1.6e-5. Post-hoc 1200 steps
  (not a gate): in/out 5.3e-10, mass flow 6.4e-4 from the steady value (the floor of the steady kernel's own
  `res_u ~ 1e-5` convergence), Mach 4.6e-4, pressure 7.3e-5; they would have passed. The registered 400 steps were kept.

### Deviations and disclosures

* **Tightened linear solves in the tests** (all kernel tests except T4): momentum and pressure relative tolerance
  1e-10, energy 1e-12, iteration caps 2000 / 20000 / 2000, so that the inner tolerance and not the linear solver limits
  the accuracy. T4 uses the default linear solver settings.
* **Inner-iteration floor.** In compressible mode the inner change `max(|dP|/P, ...)` stalls near 1e-11 (round-off of the
  absolute pressure), below the registered `inner_tol = 1e-12` of T1/T1r; the tests assert `change < 1e-9` instead of
  `converged`. In T4, 200 of the 1000 steps stopped at the 20-iteration cap (early transient, change up to 1.2e-5);
  after 10 s a step takes one inner iteration. In T6 the first steps use the 40-iteration cap (change 3e-10 to 5e-8)
  and the test asserts `change < 1e-4`; the registered `inner_tol = 1e-10` is therefore not reached in the first steps.
* **Kernel faults found by the checks and fixed** (not tuning of a criterion): (1) T1r exposed that the mass shift of a
  closed compressible domain assumed the ideal-gas `rho = psi p`; it is now a Newton step with the actual EOS. (2) T6
  exposed that a prescribed-velocity boundary face, whose density used the lagged cell pressure, made the inner
  iteration diverge for a flow Courant number above one (the lagged outflow of a cell is multiplied by `u dt/dx`, here
  5e4). Prescribed-velocity faces now get the same implicit upwind density linearisation as interior faces (`phid`,
  applied only in transient compressible runs; the steady kernel and every other check are untouched). After this fix
  both T6 modes pass at the registered `dt = 0.5 s`. (3) Section 5 (adiabatic-wall energy boundary condition) was
  needed for T6 B; the steady kernel keeps its pre-existing limitation unless viscous heating is on.
* **T7 Couette setup.** The case schema had only a uniform initial velocity. A uniform start (50 m/s) or a start from
  rest with the imposed end profile `u = U y/H` makes the transient solver diverge in the first steps (compressible at
  every `dt` tried between 0.5 and 0.005 (only a few steps at 1e-4 ran); low-Mach with viscous heating at `dt` from 0.5 to 0.02; the impulsive wall layer
  heats the first cells by tens of kelvin per 10 ms). A new key `initial.velocity_expression_x/_y/_z` (expressions of
  x, y, z at the cell centres) provides the consistent start. This is a schema addition, not a change of a limit; the
  divergence from an inconsistent start is a robustness limit of the transient solver and was not fixed.
* **Compressible transient from rest is not robust** for impulsive velocity mismatches at large `dt` (see above); no
  claim beyond the checks listed is made.
* **T6 B top-wall flux is not independent evidence.** `heat_flow_out` reads the temperature boundary coefficients, so a
  zero-gradient (adiabatic) patch returns exactly 0 and the `|q_top| <= 1e-3 q_bottom` check passes by construction. The
  check of the section 5 `h0` wall condition is the temperature profile (1.6e-4 of the scale) and the bottom-wall flow.
* **Re-measurement.** After the last kernel change (prescribed-velocity `phid`) the ignored `varden_transient` and
  `varden_transient_duct` tests were run again: T3c BDF2 errors 1.114e-2, 2.99e-3, 1.18e-3 (order 1.34) and T5 6.04e-4,
  1.88e-4, 1.64e-3 / 1.62e-3, 1.56e-5 are unchanged.
* **History.** The `schema.rs` keys and the `sim.rs` transient branch were committed inside the MRF commit d27eec0 of
  another agent (it picked up the working-tree files); `vd.rs` followed in ebc22ef, so d27eec0 alone does not build.
* **Viscous heating in steady runs** is available (same terms) but unvalidated; only the transient Couette checks
  exercise the term.

## Reference data

The linear-acoustics frequency, Stokes-Kirchhoff damping and Couette-with-dissipation profile are textbook results
derived above, not table values (Bird, Stewart and Lightfoot, Schlichting and Gersten and Patterson and Imberger were
not re-read in this session; the derivations are in this ADR); the Le Quere values are those of ADR 0005 (see `docs/reference-data.md`).

## Consequences

* The transient solver is the steady one plus time terms; per time step it costs `max_inner` steady iterations.
* A second-order scheme with the transient Rhie-Chow correction and exact discrete mass balance, but acoustics are
  resolved only when `dt` is a small fraction of the transit time; large `dt` gives an implicit, damped, quasi-steady
  response (used for T4/T5).
