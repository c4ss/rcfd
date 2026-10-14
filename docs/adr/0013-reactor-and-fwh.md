# ADR 0013: 0D reactor toolkit (F-112) and Ffowcs Williams-Hawkings acoustics (F-136)

Status: accepted. The checks, numeric limits and decision rules below were written before any code of the two
modules was compiled or run. They are not changed, widened, replaced or deleted after a run; a miss is reported as
measured and the test moves to an `#[ignore]`d `*_known_miss` test with the registered limit intact.
Roadmap: F-112 (reactors), F-136 (FW-H). F-054 (CHEMKIN reader) is still `todo`; this ADR only uses the reader that
F-112 needs (`rcfd-reactor::chemkin`), it does not advance F-054 (no CFD coupling).

## Scope

New crate `crates/rcfd-reactor` (std only plus serde_json for case files) and a new file
`crates/rcfd-post/src/fwh.rs`. CLI: `rcfd reactor <case.json>` and `rcfd acoustics <surface.json> --observer x,y,z`.

### Reactor module

* Thermo: NASA 7-coefficient polynomials (two ranges), ideal gas, R = 8.31446261815324 J/mol/K, standard state 1 atm.
* Mechanism: reader of the published CHEMKIN-II text format (ELEMENTS/SPECIES/THERMO/REACTIONS; Arrhenius, reversible
  by equilibrium constant, explicit REV, third body with efficiencies, Lindemann and Troe falloff, DUPLICATE; SRI, PLOG,
  FORD, Landau-Teller are diagnosed as unsupported with a line number). No mechanism is shipped in the repository.
  GRI-Mech 3.0 (licence "citation requested", unverified) is used only from the git-ignored `validation-data/data`
  fetch (F-125), tests skip when it is absent. Toy mechanisms in the tests are written here.
* ODE: stiff, adaptive, in-repo: extrapolation of the linearly implicit Euler method (Deuflhard), harmonic sequence
  1..6, Jacobian by forward differences, dense LU, error estimate from the last two extrapolation columns, step-size
  control; Hermite dense output. Default tolerances rtol 1e-8, atol 1e-14 (species) / 1e-8 (T), set before the runs.
* Reactors: constant pressure, constant volume (adiabatic or isothermal), perfectly stirred reactor (prescribed
  residence time, steady state by pseudo-time integration, continuation to blowout), plug flow (constant area,
  constant pressure, steady), equilibrium (Gordon-McBride element-potential Newton) at TP, HP, UV.
* Ignition delay definitions: time of maximum dT/dt (default) or time of T0 + dT (dT = 400 K default).
* Not in scope here: 1D flame speed (listed in the F-112 title), so F-112 cannot become `done` from this ADR.

### FW-H module

Formulation 1A (Brentner and Farassat 1998, AIAA J. 36(8)) for a permeable data surface in a uniform medium at rest, with
the surface allowed to move subsonically on a prescribed path. Surface data from sampled time series (uniform time
step) per element: position, normal, area, surface velocity, p', rho', u. Permeable terms:
U_i = (1 - rho/rho0) v_i + rho u_i / rho0, L_i = p' n_i + rho u_i (u_n - v_n); retarded time by Newton on
tau + |x - y(tau)|/c = t; 4-point Lagrange interpolation, 5-point central time derivatives. Volume quadrupole is not
included (documented). Verified against exact analytic fields; the Farassat/Brentner formulas are from memory of the
published papers and are validated only through the checks below.

## Checks and numeric limits

### F-112

* R1 Prothero-Robinson stiff problem y' = -1e6 (y - sin t) + cos t, y(0) = 0, exact y = sin t, t in [0, 1], rtol 1e-8,
  atol 1e-12: max abs error <= 1e-6. Plain decay y' = -y at t = 2: relative error <= 1e-7.
* R2 Robertson problem (k1 = 0.04, k2 = 3e7, k3 = 1e4; y(0) = (1,0,0)) at t = 40 against the published reference
  y = (0.7158270687, 9.185534764e-6, 0.2841637457) (Hairer and Wanner, vol. II; Mazzia and Magherini test set): relative
  error <= 1e-5 for y1 and y3, <= 1e-4 for y2, solver rtol 1e-10 atol 1e-14 (y2 atol 1e-18); |y1+y2+y3-1| <= 1e-12.
  The reference digits are from memory (the Bari PDF was not readable here); they are flagged unverified in
  docs/reference-data.md. Agreement of an independent integrator with remembered digits is itself the evidence.
* R3 Mechanism reading on GRI-Mech 3.0 (skip if absent): 53 species, 325 reactions, 5 elements; every reaction balances
  every element exactly (integer test); NASA ranges give continuous h and s at T_mid within 1e-3 relative.
* R4 Toy dissociation A2 <=> 2A (constant cp/R = 3.5 and 2.5, h0 and s0 chosen in the test), written as CHEMKIN text.
  Equilibrium TP vs the closed-form quadratic from Kp = exp(-dG/RT) (P0 = 101325 Pa), T = 3000 K and P = 1 atm and 10 atm:
  mole fractions relative error <= 1e-9. Isothermal constant-volume ODE to t >> 1/k reaches the same equilibrium
  (composition computed from Kc at the final volume): relative <= 1e-6. HP equilibrium conserves the mixture enthalpy
  to relative <= 1e-9.
* R5 Constant-pressure adiabatic toy run: mass-specific enthalpy conserved to |dh| <= 1e-8 (|h| + cp T0)
  (rtol 1e-10). Constant-volume adiabatic: internal energy likewise. Element mass fractions conserved to relative 1e-12
  in toy and (GRI H2/air, skip if absent) runs. Species mass fractions sum to 1 within 1e-12.
* R6 Isothermal plug flow of irreversible A -> B (first order, k = 50 1/s, u = 2 m/s): conversion at x = 0.1 m is
  1 - exp(-k x / u), relative error <= 1e-8. Isothermal PSR of the same reaction: conversion k tau / (1 + k tau),
  tau = 0.01 s, relative error <= 1e-8.
* R7 GRI-Mech 3.0 (skip if absent), all tolerance-free consequences registered as limits:
  * H2/air phi = 1, 1 atm: constant-volume ignition delay (max dT/dt) at T0 = 1000, 1100, 1200 K is finite and strictly
    decreasing with T0; the same for constant pressure.
  * Equilibrium HP of CH4/air phi = 1 (CH4:1, O2:2, N2:7.52, 300 K, 1 atm): T within 1 percent of 2225 K (the commonly
    quoted value for this mixture; the figure is from memory, unverified; flagged in reference-data.md). Element
    conservation of the equilibrium composition <= 1e-10 relative.
  * The long-time (1 s) limit of the constant-volume H2/air run from 1200 K lies within 1 percent (in T) of the UV
    equilibrium of the same mixture (kinetic/thermo consistency).
  * The constant-pressure long-time (1 s) limit is within 1 percent in T of the HP equilibrium.
  * PSR blowout trends (H2/air, 1 atm, adiabatic, residence time swept downwards from 1e-2 s): a burning branch exists at
    1e-2 s; blowout residence time is found within 5 percent bracket; it is shorter for 600 K inlet than for 300 K
    inlet (preheat) and longer for phi = 0.5 than for phi = 1.0 (lean). Literature trend only; no number is claimed.
* Published GRI-based ignition delay values (acceptance text: within 5 percent) are NOT registered: no number could be
  verified from an accessible source in this session (searches returned only qualitative statements). F-112 therefore
  stays `partial` at best, whatever the checks above show.

### F-136

Fields use rho0 = 1.2 kg/m3, c0 = 340 m/s, perturbation amplitudes with u/c <= 1e-5 (so the quadratic terms of the
permeable formulation are below 1e-5 relative), sources band-limited sinusoids, 64 samples per source period, quadrature
on spheres with Gauss-Legendre in cos(theta) (16 points) times 32 uniform azimuth points. Error measure
e = RMS(p_fwh - p_exact) / RMS(p_exact) over one source period at steady state.

* A1 Stationary monopole q(t) = q0 sin(w t) (p' = rho0 q_dot(t - r/c)/(4 pi r), u_r from the exact potential), permeable
  sphere R_s = 1 m, wavelength 4 R_s (kR_s = pi/2, non-compact), observers at r = 3, 10, 50 m: e <= 1e-3 each.
* A2 1/r decay: amplitude(r) * r for r = 50, 100, 200, 400 m: max/min - 1 <= 1e-3.
* A3 Retarded-time phase: phase of the fundamental (DFT over whole periods) of the FW-H signal at r = 50 m minus the
  analytic -w r/c: |dphi| <= 1e-3 rad.
* A4 Stationary dipole (point force F(t) e_x, exact p', exact potential-flow u, formulas in the test): directivity
  |p'(theta)| / |p'(0)| = |cos theta| at r = 100 m, theta = 0, 30, 60, 90, 120, 150, 180 deg: abs error <= 1e-3, and the
  RMS time-series error e <= 1e-3 at theta = 45 deg, r = 100 m.
* A5 Rotating monopole (circular orbit radius 1 m, tip Mach 0.3, source frequency 4x rotation frequency; the permeable sphere
  of radius 0.3 m translates with the source, exact Lienard-Wiechert potential phi = -q(tau)/(4 pi r (1 - Mr)), p' = -rho0 phi_t,
  u = grad phi by 8th-order central differences): observers at r = 30 m in the orbit plane (angle 0 and 90 deg to the
  orbit-plane x axis) and 30 degrees out of plane: e <= 1e-3.
* A6 Rotating dipole realised as a pair of opposite monopoles +q/-q on the same orbit separated by 0.1 m along the
  orbit, same exact superposition and same sphere enclosing both: e <= 1e-3.
  A rotating point force with exact non-compact u is not tested (its exact velocity needs the mean potential);
  this is disclosed in the roadmap note.
* A7 File round trip: the surface JSON writer/reader reproduces the series bit-exactly and the CLI reproduces the A1
  r = 10 m result to 1e-12 relative; malformed files report a located error (path in the JSON).
* A8 Guard checks: sonic or supersonic surface motion rejected; observer time outside the valid window rejected with
  the window in the message; non-uniform time step rejected.

### Both

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (heavy GRI tests
`#[ignore]`d, run with `--ignored` in release). Roadmap: F-112 and F-136 get `done` only if their acceptance text and
the full title scope are met; otherwise `partial` with an honest note.

## Tuning disclosure

* ODE method replaced twice during development (an implementation choice, not a registered criterion). The
  extrapolated linearly implicit Euler method of the Scope section needed 4e4 to 7e4 steps for 3e-4 s of GRI H2/air
  ignition (tried 2 to 6 columns, a central-difference Jacobian and looser tolerances, no fix) and was discarded. It was
  replaced by the 3-stage Radau IIA method of order 5 (Hairer and Wanner, Solving ODEs II, IV.8: simplified Newton on
  the 3n stage system, Radau5 embedded error estimator, forward-difference Jacobian reused while the Newton contraction
  rate is below 1e-3, collocation start values). That version first showed thousands of rejected steps on
  Prothero-Robinson and on the toy dissociation: the cause was a defect in the dense LU solver (row swaps were applied
  interleaved with the forward substitution although the stored multipliers had been swapped with the rows), invisible
  on diagonally dominant systems and wrong when pivoting happens. Fixed in `ode::lu_solve`; no tolerance or limit was
  touched for it. Final step counts: Prothero-Robinson 10 steps, Robertson to t = 40 hundreds of steps, GRI H2/air
  ignition 430 to 520 steps.
* The Prothero-Robinson test was first written with time as a second state (autonomous form) and is now written with the
  explicit-time right-hand side (the integrator accepts f(t, y)); same problem, same limit (1e-6), same tolerances.
* Default tolerances: unchanged from the Scope section (rtol 1e-8, atol 1e-14 species / 1e-8 T).
* R3 (NASA polynomial continuity at Tmid, limit 1e-3 relative) is MISSED by GRI-Mech 3.0 thermo data as published:
  measured 5.1e-3 in h and 4.8e-4 in s (the file's polynomials are discontinuous at the 1000 K junction by that
  amount; this is a property of the data file, not of the reader). The limit is untouched; the check lives in the
  `#[ignore]`d `r3_gri_nasa_continuity_known_miss`.
* A7 CLI part: the registered text said the CLI reproduces the A1 r = 10 m result; the implemented test
  (`crates/rcfd-cli/tests/acoustics.rs`) compares the CLI with the library on a coarse 72-element monopole-like surface
  file instead (library accuracy against the exact field is A1, the CLI is only a file/IO wrapper). Limit 1e-12
  relative as registered. The reduced scope is a deviation, disclosed here.
* A5/A6 reference fields: the exact velocity u = grad(phi) is computed by 8th-order central differences of the exact
  Lienard-Wiechert potential, as the registered text says; no tuning.
* No other limit, case parameter or quadrature setting was changed after seeing a result.

## Results

Measured (release builds, commit hashes in the final report):

Reactor (`cargo test --release -p rcfd-reactor`, R7 with `-- --ignored`):

* R1 Prothero-Robinson lambda = 1e6: max node error 1.2e-9 in 10 steps (limit 1e-6); decay rel error 1.4e-11 (1e-7).
* R2 Robertson t = 40: relative errors 2.7e-11 / 6.1e-11 / 1.6e-10 for y1 / y2 / y3 (limits 1e-5 / 1e-4 / 1e-5);
  sum - 1 = 2.2e-16 (1e-12); 782 steps. (Reference digits from memory, see reference-data.md.)
* R3 GRI reads 53 species, 325 reactions, 5 elements, zero element imbalance: pass. NASA continuity: known miss (above).
* R4 TP equilibrium relative error <= 1.3e-15 (limit 1e-9); HP enthalpy 1.8e-15 (1e-9); CV isothermal relaxation 1.4e-16
  (1e-6).
* R5 adiabatic energy drift 1.5e-13 (CP) and 2.8e-12 (CV) (limit 1e-8); element drift 7.1e-16, sum Y drift 8.9e-16
  (limit 1e-12).
* R6 plug flow 7.1e-15, PSR 5.2e-15 (limit 1e-8).
* R7 GRI-Mech 3.0, H2/air phi = 1, 1 atm, ignition delay at max dT/dt:
  constant volume 3.054e-4 s (1000 K), 8.616e-5 s (1100 K), 4.422e-5 s (1200 K);
  constant pressure 3.120e-4 s, 8.861e-5 s, 4.533e-5 s. Strictly decreasing; element drift <= 2.4e-15; sum Y error
  <= 1.6e-15. These numbers are NOT compared with published values (see above).
  CH4/air phi = 1 HP T_ad = 2225.5 K (registered 2225 K +- 1 percent), element error 2.2e-14.
  1 s limits: CV 2931.78 K vs UV equilibrium 2931.78 K (rel 2.1e-11); CP 2751.87 K vs HP 2751.87 K (rel 9.0e-12).
  PSR blowout (residence time bracket): phi 1, 300 K inlet: [1.494e-5, 1.534e-5] s (bracket 2.6 percent);
  phi 1, 600 K: [1.034e-5, 1.061e-5] s (2.6 percent); phi 0.5, 300 K: [4.286e-5, 4.400e-5] s (2.6 percent). Trends hold
  (preheat shortens, lean lengthens).
* CLI `rcfd reactor` run on the 1100 K H2/air constant-volume case: delay 8.616e-5 s (same as the test), 610 steps,
  1 s wall; malformed case file gives located errors (`reactor.temperature: must be a positive number (got -5)`),
  exit code 3.

FW-H (`cargo test --release -p rcfd-post --test fwh`, also passes in debug in 26 s):

* A1 e = 4.08e-6 / 4.39e-6 / 4.49e-6 at r = 3 / 10 / 50 m (limit 1e-3). A2 spread 2.7e-8 (1e-3). A3 phase error 8.8e-9
  rad (1e-3). A4 directivity abs error <= 2.0e-9 (1e-3), series e = 4.9e-6 (1e-3). A5 e = 4.6e-6 in plane, 4.6e-6 at
  30 degrees out of plane (1e-3). A6 e = 1.0e-5 (1e-3). A7 round trip bit exact, located errors; CLI vs library
  relative difference 0 (limit 1e-12). A8 guards pass.
* All FW-H checks passed on the first run; no limit is close to its bound, which also means the limits were loose
  relative to what the method achieves.
