# ADR 0056: Wave 29B, mixture model with algebraic slip (drift flux) and Schnerr-Sauer cavitation on the VOF host (F-127, partial)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave and before any run
of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up
defect gets a NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations, diagnosis findings) may be
appended at the end without touching a registered limit. Clean-room: first principles and published literature only
(verification status in `docs/reference-data.md`); no vendor code, text, names or data; nothing from vendor material.

Scope (gap rank 2 of `docs/parity-matrix.md`, rows 93 and 96). Delivered: (1) a mixture model with an algebraic-slip (drift-flux)
relative velocity on the existing VOF / variable-density host; (2) ONE cavitation mass-transfer model, Schnerr-Sauer form with a
bubble-number density. NOT delivered and not claimed: full multi-fluid Euler-Euler (per-phase momentum, interphase force
catalogue: lift, virtual mass, turbulent dispersion, wall lubrication; parity rows 94, 95), per-phase turbulence (row 51),
polydisperse bubble sizes, bubble-size evolution (population balance coupling), thermal effects on cavitation (no energy equation in
the multiphase path), vapour compressibility, non-condensable gas, the Zwart-Gerber-Belamri and Singhal models, turbulence
cavitation pressure correction. Published cavitation or bubble-column data are NOT compared (see "Reference data").

## Audit (HEAD 67dc5d3)

* `AlphaTransport::substep` solves `alpha_t + div(alpha phi + alpha(1-alpha) phi_r) = 0` with a first-order upwind low flux, a
  central-plus-compression high flux and a Zalesak limiter (bounded, conservative for a solenoidal flux). The compression term has
  exactly the algebraic form of a drift flux `alpha(1-alpha) v_12 . S`, so a relative velocity enters through the same slot.
* `VofSolver::step` builds the momentum convection from the mass flux `rho_phi = r2 phi + (r1 - r2) alpha_phi`, where `alpha_phi`
  is the time-averaged total alpha flux; it is automatically the mixture mass flux if `alpha_phi` includes the slip flux (below).
* `apply_mass_transfer` (ADR 0052 B) is the phase-change plumbing: it updates the liquid fraction after the transport, returns the
  delivered `mdot`, the continuity volume source `S_v = mdot (1/rho_v - 1/rho_l)`, and `solve_pressure_parts_src` consumes it.
  A fixed pressure patch is required (the vapour volume leaves through it).
* AMG is enabled in the tree for multiphase meshes with more than 2000 cells (ADR 0052 addenda); this wave changes nothing there.

## Decisions

**A. Mixture formulation (volume-averaged velocity).** `j = sum alpha_k u_k` is the solenoidal volume-averaged velocity that the host
already solves for (phase 1 fraction `alpha`, phase 2 `1 - alpha`). With the relative velocity `v_12 = u_1 - u_2` the phase
velocities are `u_1 = j + (1 - alpha) v_12`, `u_2 = j - alpha v_12`. Hence
`alpha_t + div(alpha j + alpha(1-alpha) v_12) = 0` (phase 1), the mixture mass flux is `sum alpha_k rho_k u_k = rho_m j + J` with the
drift mass flux `J = alpha(1-alpha)(rho_1 - rho_2) v_12` (this is exactly the host's `rho_phi` once the slip flux is part of
`alpha_phi`), and the summed phase momentum equations give
`d(rho_m j + J)/dt + div[ rho_m j j + j J + J j + T ] = -grad p + div(tau_m) + rho_m g`, with the drift (diffusion) stress
`T = alpha(1-alpha)[(1-alpha) rho_1 + alpha rho_2] v_12 v_12`. The host convects with `rho_phi = rho_m j + J`, which covers
`div(rho_m j j + J j)`; the added explicit source is `-div(j (x) J + T)`, with face flux `phi_f J_f + T_f . S_f` (linear face
interpolation of cell values; boundary faces use the cell value, zero on walls because `phi = 0` there). The term `dJ/dt` is dropped
(quasi-steady slip; stated limitation, it vanishes in the registered tests except at the fronts). The mixture viscosity stays the
host's linear mixture; no slip-induced viscous stress.

**B. Algebraic-slip closure (Manninen, Taivassalo and Kallio 1996 form; derived, not copied).** For a dispersed phase `d` of
diameter `d_p` in the continuous phase `c` the steady force balance on one particle under the body acceleration `a = g` (only
gravity; inertial/centrifugal `-(u.grad)u - du/dt` terms of the original are NOT included) is
`(pi d_p^3/6)(rho_d - rho_c) a = (1/2) rho_c C_D(Re) (pi d_p^2/4) |v|^2 sign`, `Re = rho_c |v| d_p/mu_c`,
`C_D = (24/Re) f(Re)`. With Stokes `f = 1` or Schiller-Naumann `f = 1 + 0.15 Re^0.687` (Re < 1000) this is
`|v| f(Re(|v|)) = V_St = d_p^2 |rho_d - rho_c| |a|/(18 mu_c)`. The mixture version replaces `rho_c` in the buoyancy by `rho_m`
(the particle feels the mixture-average pressure gradient, as in the cited form): `v_dc = d_p^2 (rho_d - rho_m)/(18 mu_c f) a`, i.e.
the single-particle speed times the hindrance factor `(1 - alpha_d)` (because `rho_d - rho_m = (1 - alpha_d)(rho_d - rho_c)`);
`f` is solved with Newton iteration from the (decreasing) function `s f(Re(s)) - V_St(alpha)`. `v_12 = v_dc` if phase 1 is the dispersed
phase, `-v_dc` otherwise. Consequences used as closed forms: with `j = 0` (closed column) the dispersed volume flux is
`Q(alpha) = -V alpha(1-alpha)^2` (Stokes, `V` the single-particle speed), convex for `alpha < 2/3`; the clear-liquid front (the Kynch
shock; Lax condition `lambda(lower) > s > lambda(upper)` is satisfied for `alpha_0 < 1/3`) moves at
`s = Q(alpha_0)/alpha_0 = -V (1 - alpha_0)^2`; the packed-bed shock rises at `V alpha_0 (1 - alpha_0)` and cannot reach the front in
the windows below.

**C. Slip flux in the alpha transport.** Face slip flux `q_f = s(alpha_f) (a . S_f)/|a|` (sign included), with `alpha_f` the linear face
interpolation, on internal faces only (boundary slip flux zero: a wall or an inlet/outlet carries no drift). Low-order flux: the
Godunov flux of the scalar law with flux function `F(alpha) = q alpha(1-alpha)(...)`, evaluated as the min (alpha_L <= alpha_R) or
max (alpha_L > alpha_R) of `F` over 8 interior sample points plus the end points (monotone to the sampling accuracy; the Zalesak
limiter retains boundedness); high-order flux: central `q alpha_f(1-alpha_f)`. The sub-step Courant number counts
`|phi| + |q|` on both faces of a face (conservative). When the slip flux of a face is exactly zero nothing is added, so a run without
slip (feature off, zero gravity or equal densities) is bit-identical to the plain VOF path.

**D. Cavitation, Schnerr-Sauer from first principles.** Bubble-number density `n` per unit volume of LIQUID, uniform spherical
bubbles of radius `R`; with `x = n (4/3) pi R^3` (vapour volume per liquid volume) `alpha_v = x/(1 + x)`, so
`R = (3 alpha_v/(4 pi n (1 - alpha_v)))^(1/3)`. Inertia-controlled growth (Rayleigh-Plesset without surface tension and viscosity,
`R R'' + (3/2) R'^2 = (p_v - p)/rho_l`, `p_v = p_sat`) has the first integral `R'^2 = (2 (p_v - p)/(3 rho_l))(1 - (R_0/R)^3)` for a
constant pressure, whose asymptote is `R' = sign(p_v - p) sqrt(2|p_v - p|/(3 rho_l))`. Then `d alpha_v/dt = 3 alpha_v(1 - alpha_v) R'/R`
(material), and with incompressible phases the volume source is the vapour mass source `mdot` of
`d(alpha_v rho_v)/dt + div(alpha_v rho_v u) = mdot`: `D alpha_v/Dt = mdot rho_m/(rho_v rho_l)` with
`rho_m = alpha_v rho_v + (1 - alpha_v) rho_l`, hence
`mdot = (rho_v rho_l/rho_m) alpha_v (1 - alpha_v) (3/R) sign(p_v - p) sqrt(2|p_v - p|/(3 rho_l))` (positive = evaporation). Optional
empirical factors `F_vap`, `F_cond` (default 1) multiply the two signs. In the host the cell update does not use `mdot` as an
explicit rate: with the pressure of the start of the step frozen, the radius is advanced exactly, `R' = R + F R'_inf dt`, and
`alpha_v' = x'/(1 + x')`; the delivered `mdot_eff = rho_l (alpha_l - alpha_l')/dt` then feeds the continuity source exactly as in ADR 0052
B2 (so mass is conserved by construction and the update is bounded in `[alpha_floor, 1)` for any `dt`). Nuclei: the bubble radius never
falls below the nucleus radius `R_0` (condensation stops at `alpha_floor = x_0/(1 + x_0)`, `x_0 = n (4/3) pi R_0^3`); the field is
initialised at that floor, which is also the inception seed (without seeds `alpha_v = 0` never grows, `mdot = 0`). The liquid `p`
is the host's absolute pressure `p = p_rgh + rho_m g.(x - x_ref)`. One pressure (Dirichlet) patch is required, as in ADR 0052 B3.
Cavitation and `mass_transfer` (thermal) are mutually exclusive in the tree.

**E. Wiring.** `VofSettings::mixture: Option<MixtureSettings>`, `VofSettings::cavitation: Option<Cavitation>`; both `None` leave
the path bit-identical. Library: new modules `mixture.rs` (closures) and `cavitation.rs` (Schnerr-Sauer, 0-D radius integrator).
Case tree (additive, under `multiphase`, unset sentinels `-1`, located `Diagnostic` paths, inert-key errors while the model is off):
`multiphase.mixture.{model none|drift_flux, dispersed_phase, particle_diameter, drag stokes|schiller_naumann}` and
`multiphase.cavitation.{model none|schnerr_sauer, liquid_phase, saturation_pressure, bubble_number_density, nucleus_radius,
vaporisation_factor, condensation_factor}`. Baseline hashes (`baseline_hashes.rs`) stay unchanged and are not re-recorded; if a fix
needed would change one, the wave stops and reports.

## Registered checks (before any run)

Notation: `dx` cell size, `V_St` Stokes speed. Gravity `-9.81 y`. Water-like continuous phase `rho_c = 1000`, `mu_c = 1e-3`.

**D. Drift flux (`crates/rcfd-fv/tests/mixture_drift.rs`).**
* D1 Conservation, closed column 4 x 200 cells, 0.008 x 0.4 m, walls all round, glass-like dispersed phase `rho_d = 2500`, `d_p = 50 um`
  (`V_St = 2.043e-3 m/s`, `Re = 0.10`), Stokes drag, `alpha_0 = 0.1` in `y <= 0.35` (face aligned), 400 steps of 0.25 s: the dispersed
  volume `sum alpha V` and the mixture mass `sum rho_m V` change by a relative amount `<= 1e-12`; `alpha in [-1e-9, 1 + 1e-9]` at every
  step; `max |u| <= 1e-9 m/s` (1-D: `j = 0`).
* D2 Stokes terminal velocity (closure, no solver): at `alpha_d -> 0` the computed speed equals `d_p^2 (rho_d - rho_c) g/(18 mu_c)`
  to relative `1e-14`; with hindrance it equals that times `(1 - alpha_d)` to `1e-14` for `alpha_d = 0.1, 0.3`; a lighter dispersed phase
  (`rho_d = 1.2`) gives the same magnitude formula with the opposite sign (rise).
* D3 Schiller-Naumann: for `d_p = 50 um, 100 um, 300 um` the solution satisfies the dimensional force balance
  `(pi d^3/6) drho g = (1/2) rho_c C_D(Re) (pi d^2/4) s^2` with `C_D = 24/Re (1 + 0.15 Re^0.687)` to relative `1e-12`; `s <= V_St` and
  `s >= V_St/(1 + 0.15 Re_St^0.687)` (`Re_St` built on `V_St`); for `d_p = 1 um` the relative difference to Stokes is `<= 1e-5`.
* D4 Batch settling front (the column of D1, window `t = 10, 20, ..., 100 s`): the front is the interpolated position where `alpha`
  crosses `alpha_0/2` descending from the top; exact `y_f(t) = 0.35 - V_St (1 - alpha_0)^2 t`; error `<= 1.0 dx = 2 mm` at every sample, and
  the least-squares front speed is within 3 percent of `V_St (1 - alpha_0)^2` (1.655e-3 m/s).
* D5 The same with Schiller-Naumann drag, `d_p = 100 um`, `alpha_0 = 0.02`, `dt = 0.1 s`, samples `t = 5, 10, ..., 40 s`: the exact front speed
  is `(1 - alpha_0) s(alpha_0)` with `s` the SN solution at `rho_d - rho_m(alpha_0)`; front error `<= 1.0 dx` at every sample, speed within 3 percent.
* D6 Rising dispersion (phase 2 dispersed, `rho_d = 1.2`, `d_p = 50 um`, Stokes, `alpha_0 = 0.1` in `0.05 <= y <= 0.15`, `dt = 0.25 s`, samples
  `t = 10..60 s`): the lower edge (crossing of `alpha_d = alpha_0/2`, ascending) follows `0.05 + V_St' (1 - alpha_0)^2 t` with
  `V_St' = d_p^2 (rho_c - rho_d) g/(18 mu_c)`, error `<= 1.0 dx`.

**M. Momentum consistency (`crates/rcfd-fv/tests/mixture_drift.rs`).**
* M1 Mixture on, gravity zero (slip identically zero), channel 40 x 10 with inflow 0.1 m/s, fixed-pressure outlet, a phase-1 block in the
  stream, 50 steps: `u`, `p_rgh`, `alpha`, `phi` equal (`==`, bitwise except the sign of zero) the plain VOF run.
* M2 Equal phase densities with gravity (`rho_d - rho_m = 0`) and the mixture on: the same equality, all fields.
* M3 Drift-stress source against the analytic divergence: smooth prescribed `alpha(y) = 0.3 + 0.2 sin(2 pi y/L)`, `j = 0`, closure with
  non-zero slip; the explicit source `-div(j (x) J + T)` of the solver at `n = 50` and `n = 100` cells (2 x n box) differs from the analytic
  `-dT_yy/dy` (using the closure's `v(alpha)`) by at most 2 percent of the maximum at `n = 100` (cells two away from both ends) and the
  observed order between the two resolutions is `>= 1.8`; the x-component is `0` to `1e-12` of the y-component scale.
* M4 Mass-flux consistency: for the D1 column after 40 steps, the step-averaged mixture mass flux built by the solver (`rho_phi`) equals
  `sum_k alpha_k rho_k u_k . S` with the phase velocities `u_1 = j + (1-alpha) v_12`, `u_2 = j - alpha v_12` at interior faces, relative `1e-12`.

**C. Cavitation (`crates/rcfd-fv/tests/cavitation.rs`).** `rho_l = 1000, rho_v = 0.02, p_sat = 2339 Pa, n = 1e11 /m^3, R_0 = 1e-5 m`.
* C1 Zero at saturation: `p = p_sat` gives `mdot = 0` exactly for every `alpha_v`, and the cell update leaves `alpha_v` unchanged
  bit for bit; also `mdot = 0` at `alpha_v in {0, 1}`.
* C2 Sign: `p < p_sat` gives `mdot > 0`, `p > p_sat` gives `mdot < 0` for `0 < alpha_v < 1` over `alpha_v = 1e-4 ... 0.99`,
  `|p - p_sat| = 1 ... 1e7 Pa`.
* C3 Boundedness: for `dt` from `1e-12` to `1 s`, `alpha_v` start 1e-4 ... 0.999 and `p - p_sat` from `-1e8` to `+1e8` Pa, the updated
  `alpha_v` is finite and in `[alpha_floor, 1]` and moves in the direction of the sign of `p_sat - p` (or stays), for 1000 random
  combinations plus the extremes.
* C4 Rayleigh-Plesset: (a) algebraic identity: `mdot rho_m/(rho_v rho_l)` equals `3 alpha(1 - alpha) R'/R` pointwise to `1e-13` relative;
  (b) a cell with `p = p_sat - 1e4 Pa` and the exact radius update: after `k` steps of `dt = 1e-6 s` the radius is `R_0 + R'_inf t`
  to `1e-12` relative and `alpha_v` is the closed form of that radius to `1e-12` relative, independent of `dt` (compare `dt = 1e-6, 5e-6`);
  (c) full Rayleigh-Plesset ODE (RK4 with a tight step, `p = p_sat - 1e4`, `R(0) = R_0`, `R'(0) = 0`): `R'^2` obeys the first integral to
  `1e-6` relative, and the difference `R_SS(t) - R_RP(t)` lies in `[0, 0.6 R_0]` (analytic: the constant start-up lag
  `R_0 integral_1^inf (1/sqrt(1 - r^-3) - 1) dr = 0.569 R_0`, computed here by quadrature at the time of writing) for all `t` with
  `R_RP >= 5 R_0`; (d) collapse stops at the nucleus: starting from `alpha_v = 0.5` with `p = p_sat + 1e5` for `dt = 1 s` the result is
  `alpha_floor` exactly.

**N. Cavitating venturi, 2-D (`crates/rcfd-api/tests/mixture_cavitation_tree.rs` through the case tree).** Channel `L = 0.2 m`, height 0.02 m
at the inlet and outlet sections, throat height 0.01 m at `0.09 <= x <= 0.11`; cosine contraction over `0.05 <= x <= 0.09`, cosine
recovery over `0.11 <= x <= 0.19`; 80 x 20 cells (`dx = 2.5 mm`, `dy` follows the wall), laminar, no gravity. Inlet `u = 2.5 m/s`
(fixed), outlet `p = 9000 Pa`. `rho_l = 1000, rho_v = 0.02`, `mu_l = 1e-3`, `mu_v = 1e-5`, `p_sat = 2339`, `n = 1e11`, `R_0 = 1e-5`
(`alpha_floor = 4.2e-4`). `q_t = rho u_t^2/2 = 12.5 kPa` with the continuity throat speed `u_t = 5 m/s`. Without cavitation the 1-D
Bernoulli estimate from the outlet is `p_t = 9000 - 500 (25 - 6.25) = -375 Pa`, i.e. `p_sat - p_t = 2.7 kPa`. `dt = 2.5e-4 s`, `0.30 s`.
* N1 No cavitation run (model none): `|p_t,sim - (p_in,sim - (rho/2)(u_t^2 - u_in^2))| <= 0.10 q_t = 1250 Pa`, with `p_t,sim` the throat
  mean at `x = 0.10`, `p_in,sim` the inlet-section mean at `x = 0.02`; and the minimum pressure in `0.09 <= x <= 0.11` is `< p_sat` (inception
  expected there).
* N2 Cavitation run (Schnerr-Sauer): `alpha_v in [alpha_floor - 1e-9, 1 + 1e-9]` at every step and cell, no NaN, the run completes.
* N3 Inception location: cells with `alpha_v > 100 alpha_floor` at the end exist and all lie in `0.05 <= x <= 0.19`; no cell with `x < 0.05`
  exceeds `5 alpha_floor`. The cell index of the extreme is reported.
* N4 Sign discipline located: in every step every cell whose `alpha_v` rose by more than `1e-12` had `p < p_sat` at the start of the step
  (violation count must be 0; worst cell reported).
* N5 Volume balance: the net volume flux through the boundary equals `sum S_v V` (history `volume_source`) to relative `1e-6` at every step
  after step 10 (located cell of the largest `|div phi - S_v|` reported).
* N6 Cavity relieves the suction: the minimum pressure in `0.09 <= x <= 0.11` of the cavitating run at the end is above that of the
  non-cavitating run and `>= p_sat - 0.25 q_t = -787 Pa`.

**W. Wiring (`crates/rcfd-api/tests/mixture_cavitation_tree.rs`).** One test per refusal with the path in the diagnostic: mixture model on
without `particle_diameter`; non-positive diameter; cavitation on without `saturation_pressure` / `bubble_number_density` /
`nucleus_radius`; mixture/cavitation keys set while the model is `none` (inert-key error); mixture and thermal `mass_transfer` both on; cavitation without a
pressure patch; mixture with `multiphase.enabled = false`; a tree run of the D4 column reproduces the library run bitwise (same front).
Baselines: `baseline_hashes.rs` unchanged and passing.

## Reference data (status, to be mirrored in `docs/reference-data.md`)

Read in this wave: none (WebSearch/WebFetch not required). Unverified, used from memory and labelled as such: the Manninen et al.
(1996, VTT Publications 288) algebraic-slip form; the Schiller and Naumann (1935) drag correlation; Schnerr and Sauer (2001) cavitation
model; Rayleigh (1917) and Plesset (1949) bubble dynamics; Kynch (1952) batch-settling theory; the vapour pressure of water at 20 C
(2339 Pa). Everything else is derived above. No published cavitation or bubble-column data is compared: NOT attempted.
