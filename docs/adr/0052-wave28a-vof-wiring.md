# ADR 0052: Wave 28A, AMG on the variable-density pressure matrix, and wiring of mass transfer (F-226) and waves (F-227) into the VOF host

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave and before any run
of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up
defect gets a NEW corrected test and the old one stays as a record. A newly passing known-miss gets a NEW passing test with the same
limit. Addenda (exact numbers, deviations, diagnosis findings) may be appended at the end without touching a registered limit.
Clean-room: first principles and published literature only (verification status in `docs/reference-data.md`); no vendor code, text,
names or data; nothing from vendor material.

Scope. Wave 27A delivered the VOF host (`rcfd-fv/src/vof.rs`, `rcfd-api/src/vof.rs`, ADR 0049). Three library-only items can now be
wired: F-226 (interfacial mass transfer, `masstransfer.rs`, ADR 0040), F-227 (wave generation and absorption, `waves.rs`, ADR 0040)
and F-228 (population balance, only if time remains). One defect is carried over: AMG-preconditioned CG on the pressure equation
broke down in the 120 x 40 dam break at step 120 (ADR 0049 addendum, "not investigated"). Other waves (28B, 28C) work in the same
tree; this wave edits `vof.rs` (rcfd-fv and rcfd-api), `pressure.rs` (one additive function), the schema and docs, and adds new files.

## Audit (HEAD 9c75732)

* `solve_pressure_parts` assembles `laplacian(gamma_eff) p = -div(phiHbyA)` with `gamma = interpolate(V / a_p)`; with the mixture
  density inside `a_p = rho/dt + ...` the coefficient spans about 1000:1 between water and air cells. With no Dirichlet pressure
  patch the level is fixed by doubling the diagonal of the reference cell (`p_ref`). `solve_amg` rebuilds the smoothed-aggregation
  hierarchy from the current coefficients on every call (no setup reuse), then runs CG with one W-cycle (Gauss-Seidel smoothing,
  dense LU on the coarsest level).
* The AMG strength measure is `|a_ij| >= theta sqrt(a_ii a_jj)`, `theta = 0.08`, halved on each coarser level.
* `AlphaTransport::substep` is the conservative form `alpha' = alpha - dt/V sum F` with a Zalesak limiter that assumes a nearly
  solenoidal flux; a volume source means `div phi != 0`.
* The energy equation is NOT available in the multiphase path (`energy.enabled` is refused by `vof::check`). The VOF host has no
  temperature field. Consequence for F-226: temperature is a prescribed field; no latent-heat feedback; a Stefan problem (moving
  front driven by conduction) is NOT reachable and is not claimed.
* `VofStepInfo` does not expose the liquid volume flux through the boundary; the tank mass-balance check needs it.

## Decisions

**A. AMG defect (diagnosis first, then at most one fix).**
A1 Reproducer in `rcfd-linalg` (`tests/amg_contrast.rs`): the 5-point FV Poisson matrix on 120 x 40 cells with a piecewise-constant
face coefficient (harmonic mean of cell coefficients; 1 below an interface row, 1000 above, also an interface that cuts through a
row and a slanted interface), all-Neumann with the reference-cell diagonal doubling, smooth right-hand side. Records AMG-CG iterations,
setup complexity, whether any hierarchy entry is non-finite, whether the preconditioner is symmetric positive (`x^T M x > 0` and
`x^T M y = y^T M x` on random vectors), against plain CG.
A2 Replay: the actual pressure matrix and right-hand side of the dam break at the failing solve (step 120, first corrector), written
to scratch by a temporary, uncommitted dump in `solve_pressure_parts`, then solved with AMG-CG, plain CG and a dense reference.
Bisect the failure over: matrix finite? symmetric? positive definite? (min diagonal, row sums), AMG levels, preconditioner
positivity, CG breakdown (`pq <= 0`, `rz <= 0`), and whether the NaN enters through the matrix (upstream: `a_p`, `r_au`) rather than
the solver. Only a PROVEN defect is fixed. If the cause lies in the coefficients (a NaN or negative `r_au` from the momentum step)
the fix is in the VOF step, not in AMG.
A3 Constraint: every existing AMG user (single-phase transient pressure, `Amg::new` default settings) must stay bit-identical
(baseline hashes). A fix inside `rcfd-linalg` must therefore be inert for matrices that do not trigger the defect; if a proven fix
would change any baseline hash this wave STOPS and reports.
A4 Acceptance of a fix (registered before the fix): with `amg_pressure = true` the 120 x 40 dam break of ADR 0049 D (same
parameters) completes to `t* = 3.6` without NaN; every pressure solve converges below the iteration cap (5000) with at most 60
iterations per corrector (plain CG needs several hundred on this mesh; the claim to be checked is that AMG is faster, so the mean
iterations per corrector must be at most half those of plain CG, and wall time at most that of plain CG); D1, D2, D3 hold with their
registered limits; the front position of the AMG run at `t* = 3` differs from the plain-CG run by at most one cell (`dx = 5.7 mm`).
Only if A4 passes does the case tree enable AMG in the multiphase path (same rule as the single-phase path, `n_cells > 2000`, and an
explicit key to turn it off); otherwise `amg_pressure = false` stays and the defect is reported.

**B. F-226 wiring (`VofSettings::mass_transfer`).**
B1 Model. `MassTransfer { liquid_phase (1|2), model: Lee{c_evap,c_cond} | HertzKnudsen{accommodation, molar_mass, schrage, p_vapour,
p_sat_ref}, t_sat, h_fg, temperature: prescribed per-cell field }`. The HK saturation pressure is Clausius-Clapeyron with constant
`h_fg`: `p_sat(T) = p_sat_ref exp(h_fg M / R (1/T_sat - 1/T))` (so `p_sat(T_sat) = p_sat_ref`); the area density is `|grad alpha|`
(Green-Gauss); the rate is `mdot = j |grad alpha|`. `mdot > 0` is evaporation.
B2 Per step, in this order: `mdot` from `alpha`, `T` at the start of the step (no new time-lag between the alpha and continuity
sources); transport `alpha` with the old flux; apply the source with an exponential integrator for Lee,
`alpha_l <- alpha_l exp(-c_evap theta dt)` (evaporation, `theta = (T - T_sat)/T_sat > 0`) and `1 - alpha_l <- (1 - alpha_l)
exp(-c_cond rho_v theta_abs dt / rho_l)` (condensation), which is the exact solution of the frozen-temperature source ODE and
bounded for every `dt`; for HK a bounded explicit increment clipped to `[0, 1]`. The delivered rate used everywhere afterwards is
`mdot_eff = rho_l (alpha_l_before - alpha_l_after)/dt` (the clipped amount), so the liquid sink `-mdot_eff/rho_l` and the vapour
source `+mdot_eff/rho_v` are the same number in the same cell by construction. The continuity equation gets the volume source
`S_v = mdot_eff (1/rho_v - 1/rho_l)` per unit volume: `div phi = S_v`. The pressure matrix is unchanged; only the right-hand side
changes, through a new function `solve_pressure_src` in `pressure.rs` (additive; the existing `solve_pressure_parts` calls it with
no source, so every existing path is bit-identical).
B3 Closed domain: a net volume source needs an outflow; with all-Neumann pressure the reference-cell anchor would absorb the
imbalance, so a case with mass transfer needs a pressure (Dirichlet) patch; the tree refuses mass transfer without one.
B4 Limitations stated, not hidden: no energy equation, so no latent-heat cooling (the energy source `-mdot_eff h_fg` is reported as a
diagnostic power, never applied); the momentum convection mass flux `rho_phi` is built from the alpha flux and does not carry the
phase-change contribution (second-order for the quiescent cases registered here); no Stefan front.

**C. F-227 wiring (`VofSettings::waves`).**
C1 Wave inlet. `WaveInlet { patch, wave: Wave, level, t_ramp }`; 2-D, propagation along +x, vertical coordinate `y`, `z = y - level`.
At the start of each step the velocity and alpha conditions of the inlet patch are replaced by per-face fixed values for time `t + dt`:
`eta = ramp * wave.eta(x_b, t)`; face alpha `= liquid_fraction(y_f, dy_f, level + eta)` (`dy_f` the vertical extent of the face);
face velocity `= alpha_f * wave.velocity(x_b, min(z_f, eta), t) * ramp`, zero in air (so the inflow equals the water-column flux
and no air flux is injected).
C2 Relaxation zones. `RelaxZone { x_inner, x_outer, blend: Blend (23B), target: StillWater{level} | Wave }`. After the pressure
correction of each step: `alpha <- w alpha + (1-w) alpha_target`, `u <- w u + (1-w) u_target` with `w = blend.weight(zone_chi(x_c))`
of the cell centre. The face flux `phi` is left solenoidal (a convex blend keeps `alpha` in `[0,1]`); to keep the Rhie-Chow
defect of the next step unaffected by the blend, the defect is formed from the velocity before the blend (`u_rc`). The liquid
volume added or removed by the blend is accumulated (`relax_volume`), so mass is bookkept, not hidden.
C3 `VofStepInfo` gains `alpha_outflow` (net liquid volume leaving through the boundary during the step, from the time-averaged alpha
flux) and `relax_volume`; no existing field changes.
C4 Ambient numerical-error estimate (first principles, for the tolerance below): a first-order implicit-Euler step damps a wave of
angular frequency `omega` at rate `omega^2 dt / 2`; first-order upwind momentum convection adds the numerical viscosity
`nu_num = |u| dx / 2` with the orbital speed `|u| = omega a`, damping the amplitude at `2 nu_num k^2`; the alpha equation is
compressed but adds interface smearing of about one cell. For the tank of D below (`omega = 6.28`, `dt = 2.5e-3`, `a = 0.03`,
`dx = lambda/32`, `k = 4.6`) the two rates are about 0.039 /s and 0.04 /s: about 8 percent per second, and with a group velocity of
about 0.9 m/s about 9 percent per wavelength travelled.

**D. Process.** Library checks in new test files (`rcfd-fv/tests/vof_masstransfer.rs`, `vof_waves.rs`, `rcfd-linalg/tests/amg_contrast.rs`),
tree checks in `rcfd-api/tests/vof_wiring_tree.rs`. Heavy tests (the wave tank) are run once per change; the first run is the
registered run.

## Registered checks (before any run)

**M. Mass transfer (`tests/vof_masstransfer.rs`).** Mesh: 2-D box 8 x 24 cells (width 0.1 m, height 0.3 m), water
(phase 1, 1000) below `y = 0.15` with the interface exactly on a cell face except in M5, vapour (phase 2, rho 0.6, a steam-like
density; ratio 1667), bottom and sides no-slip, the top a fixed `p_rgh = 0` outlet with `zero_gradient` velocity, no gravity.
Lee model `c_evap = c_cond = 100 /s`, `t_sat = 373.15`, `h_fg = 2.26e6`, `dt = 1e-3`.
* M1 Closed form, source operator alone (no flow): a uniform field `alpha_l = 0.6` with `T = T_sat (1 + 0.02)` (theta = 0.02): after
  `n` applications `alpha_l(t) = 0.6 exp(-c theta t)` to relative error `<= 1e-12` for `t = 0.1, 0.5, 1.0 s` with `dt = 1e-3`, and
  independent of `dt` (`dt = 1e-2` agrees to `<= 1e-12`). Condensation (theta = -0.02, alpha_l = 0.4): `1 - alpha_l(t) = 0.6
  exp(-c rho_v |theta| t / rho_l)` to `<= 1e-12`.
* M2 Phase mass conservation of the transferred mass, per cell and in total: `|rho_l S_l + rho_v S_v| <= 1e-13 rho_l/dt` per cell
  (the same number by construction) and the integrated liquid mass `rho_l sum alpha V` changes only by `- sum mdot_eff V dt`
  (no flux through the boundary because `alpha = 0` at the top): relative `<= 1e-12` over 200 coupled steps of the superheated-slab
  case below.
* M3 Zero transfer at saturation: `T = T_sat` uniform, 200 steps with the mass-transfer wiring active: every field (`alpha`, `u`,
  `p_rgh`, `phi`) is bit-identical to the same run without mass transfer.
* M4 Superheated liquid, `theta = 0.02`, `T` uniform (prescribed): 200 coupled steps; `alpha in [-1e-9, 1+1e-9]`; the volume flux
  through the pressure patch equals `sum S_v V` to `<= 1e-6` relative (continuity with the linear-solver tolerance 1e-8 plus a margin); the
  total delivered rate equals the located sum `sum mdot_eff V`; the located error reported is the cell index of the largest
  `|div phi - S_v|`. (Lee acts on every liquid cell, not only on the interface; an interface-only source is NOT claimed.)
* M5 Hertz-Knudsen-Schrage closed form: a sharp planar interface cut at a cell face (monotone 1-D alpha profile 1,...,1,0,...,0),
  `sigma = 1`, `M = 0.018`, `schrage = false`, `p_v = 0.5 p_sat(T)`: total evaporation rate `sum mdot V` equals `j A` with `A` the
  interface area (width times depth) and `j` the Hertz-Knudsen flux, relative `<= 1e-9` (the sum of the central-gradient magnitudes
  of a monotone step is exactly the jump). At `p_v = p_sat(T)` the rate is `<= 1e-12 * j(0.5)`.
* M6 Wiring/located errors in the tree: a bad key gives a diagnostic with the path (tree test, one test per refusal).
* NOT reachable and not claimed: Stefan front; relaxation of `T` to `T_sat` by latent heat (needs the energy equation);
  the 0-D library cell remains the only check of that closed form.

**W. Waves (`tests/vof_waves.rs`).** Tank: depth `h = 0.30 m`, `T = 1.0 s`, `H = 0.06 m` (`a = 0.03`), Airy waves (order 1),
`g = 9.81`, `lambda` from the dispersion relation of `waves::wavenumber` (about 1.37 m, `kh` about 1.4, steepness `H/lambda` about
0.044), water 1000 / 1e-3, air 1 / 1.8e-5, `sigma = 0`. 2-D box `4.5 lambda` long, `0.45 m` high, `dx = lambda/32`, `dy = 0.01`
(about 6480 cells), no-slip bottom, the top is the fixed `p_rgh = 0` pressure outlet (`zero_gradient` velocity), the bottom
no-slip. Inlet at `x = 0` (the wave inlet), outlet end `x = 4.5 lambda`: no-slip wall. Beach: relaxation zone `[3 lambda, 4.5 lambda]`
to still water, exponential blend (p = 3.5). `dt = 2.5e-3`, 10 periods (4000 steps), upwind momentum, `t_ramp = 2 T`.
Tolerances are stated as first-principles estimates (C4) BEFORE the run:
* W1 Height and period. Wave height `H_m = 2 * (harmonic amplitude of eta at omega)` over the window `t in [6T, 10T]` (integer
  periods) at gauge G1 `x = 0.5 lambda` and gauge G2 `x = 1.5 lambda`, `eta` taken as `level + eta = sum_column alpha dy` (the
  integral of alpha over the column, exact for the VOF fraction). Limits: G1 within 12 percent of 0.06 (estimated numerical
  damping 4 to 5 percent over 0.5 lambda, plus 2 percent from the inlet discretisation), G2 within 25 percent (estimated 9 percent
  per wavelength, 14 percent over 1.5 lambda, with a margin for dispersion error). Period from the mean spacing of the zero
  up-crossings of `eta` over the window at both gauges: within 2 percent of 1.0 s (the inlet imposes the frequency; the
  measurement is limited by the 2.5e-3 s sampling).
* W2 Reflection coefficient of the beach by the two-gauge method (Goda and Suzuki 1976, separation `lambda/4`, gauges at `2.0 lambda`
  and `2.25 lambda`, window `t in [7T, 10T]`, solved for incident and reflected complex amplitudes by the 2 x 2 system with the
  linear wavenumber): `K_r <= 0.15` (a relaxation zone of 1.5 lambda with exponential weights is expected to reflect 3 to 10 percent;
  the estimator itself is biased by about 1 percent by the decay between the gauges). Control: the same tank with the beach removed
  (reflecting end wall): `K_r >= 0.5` (a no-slip wall reflects nearly totally, damped by 9 percent per wavelength over the 2 x 1
  wavelengths of path), proving the estimator discriminates.
* W3 Mass conservation: the bookkeeping identity `V_l(t_end) - V_l(0) + sum_steps dt alpha_outflow - sum relax_volume = 0` to
  `<= 1e-10 V_l(0)`; and the mean-level drift `|V_l(t_end) - V_l(0)| / (footprint) <= 0.1 H`. Boundedness of alpha `1e-9`.
* W4 Still water with the relaxation zone on (no inlet motion, `H = 0`, same tank, 200 steps, zone `[3 lambda, 4.5 lambda]`):
  `max |U| <= 1e-10` at every step and `max |alpha - alpha_0| <= 1e-12`: no spurious current from the zone. With a stationary
  hydrostatic state the target equals the state, so the blend is the identity up to round-off.
* W5 Unit checks of the wiring: the inlet face values reproduce `Wave::eta` and the column flux `sum alpha u dy` equals
  `int_{-h}^{eta} u dz` of the Airy wave to `<= 3 percent` (cell-midpoint integral with a 6-cell wave height) at three phases.
* Time-box: the first tank run is the registered run; if it exceeds 25 minutes the mesh is not refined further, the time is reported.
  F-227 stays `partial` unless every acceptance point of the roadmap item is met (irregular waves in the tree, 3-D, the second-order
  Stokes inlet in the tank and a long-run absorption study are not).

**R. Regression.** `crates/rcfd-api/tests/baseline_hashes.rs` bit-identical and not re-recorded; all existing tests unchanged; the
default `VofSettings` (no mass transfer, no waves) gives bit-identical results to HEAD (checked by the existing `vof_*` tests).

## Registered expected failure modes (before any run)

1. The AMG failure may not be in AMG at all (a NaN entering through the coefficients). The bisect decides; no fix is guessed.
2. W1 G2 and W2 may miss: the dissipation estimate of C4 is crude and the first-order Euler step is the largest term; a smaller
   `dt` or a higher-order scheme would be a new, separately registered run (not a tune of the registered one).
3. HK with `|grad alpha|` as area density is only a closure; M5 checks the arithmetic of the wiring, not the physics.
4. Lee evaporation in pure-liquid cells is the model as written (it acts on every liquid cell, not only on the interface); M4 does
   not claim an interface-only source.
5. The relaxation blend does not conserve liquid volume; it is bookkept (W3), not conserved.

## Named gaps

Energy equation in the multiphase path (latent heat feedback, Stefan problem); species equilibrium transfer and wall condensation
(Nusselt film) wiring; irregular-wave and Stokes-2 inlets in the tank (the inlet supports order 2 in the library path but the
registered run uses Airy); a generation relaxation zone (only the inlet boundary generates); 3-D tanks; F-228 population balance
(not attempted unless stated in the addendum).
