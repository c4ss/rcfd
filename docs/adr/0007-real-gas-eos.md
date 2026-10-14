# ADR 0007: Real-gas equation of state in the pressure-based variable-density kernel

Status: accepted. Design and every check and limit below were written before any real-gas code was compiled or run.
Roadmap item F-061 (the real-gas half), ADR 0005 section "Consequences" (the EOS is a small function pair).

## Decision

`fluid.density = "real_gas"` selects the same `rcfd-fv::varden` solver as `ideal_gas`, with a thermodynamic model
`RealGas` (new module `rcfd-fv::realgas`) instead of `rho = p/(R T)`:

* Equation of state: the cubic EOS already in `rcfd-core::fluids` (`CubicEos`: Peng-Robinson 1976, Redlich-Kwong 1949),
  `fluid.eos = peng_robinson | redlich_kwong`. Critical temperature, pressure, acentric factor and molar mass come from a
  small built-in table (`rcfd-core::eos_data`: N2, O2, Ar, CO2, CH4, H2, H2O) selected by `fluid.species`, each
  overridable by `fluid.critical_temperature`, `fluid.critical_pressure`, `fluid.acentric_factor`, `fluid.molar_mass`.
  van der Waals is not implemented (not in the library; only PR and RK are).
* Density: `rho(p, T) = p M/(Z R_u T)`, the vapour-like (largest) root of the cubic. `psi = d rho/d p|_T` analytic from
  `dp/dv|_T` of the EOS; it replaces `1/(R T)` in the transonic pressure equation and the closed-domain mass shift.
  (`psi` only linearises the density in the pressure correction; the converged mass flux is `rho_f Q_f` for any `psi`.)
* Thermal model: ideal-gas part with constant `cp0` (J/kg/K, `fluid.specific_heat`, default `gamma R/(gamma-1)` with
  `R = R_u/M` and `fluid.gamma`) plus the EOS departure functions at the same T and p:
  `h = cp0 T + h_dep(T,p)`, `s = cp0 ln(T/300) - R ln(p/1e5) + s_dep(T,p)`, where for PR
  `h_dep = RT(Z-1) + (T a' - a)/(2 sqrt2 b) ln[(Z+(1+sqrt2)B)/(Z+(1-sqrt2)B)]`,
  `s_dep = R ln(Z-B) + a'/(2 sqrt2 b) ln[...]` (RK: `(T a' - a)/b ln(1+B/Z)`, `a'/b ln(1+B/Z)`), consistent with
  `h_dep - T s_dep = RT ln(phi)` (already in `fluids.rs`). `cp = dh/dT|_p`, `cv = cp + T (dp/dT|_v)^2/(rho^2 dp/drho|_T)`
  expressed per mass, sound speed `c^2 = (cp/cv) dp/drho|_T`. T from the total-enthalpy equation by Newton in `T`
  at the cell pressure.
* Energy equation: total-enthalpy form unchanged (`h0 = h(T,p) + |u|^2/2`). The diffusivity `k/cp` uses `cp0` (a
  declared approximation: it equals `k grad T / grad h` only at constant p and for a calorically perfect gas; viscous
  and conductive effects are small in the registered cases).
* Stagnation inlet: static state from `s(T,p) = s(T0,p0)` and `h(T,p) + |u|^2/2 = h(T0,p0)` (replacing the ideal
  isentropic relation) by a bracketed 1-D root find in `p`.
* Supported: `fluid.mode = compressible` only (low_mach with a real gas is a validation error); all other unsupported
  combinations of ADR 0005 apply. Not included: phase change/two-phase (liquid root, saturation dome inside the
  flow; a state without a vapour root is a run-time error), real-gas transport properties (mu, k are the same
  constant/Sutherland user inputs), mixtures.
* The ideal-gas path (`fluid.density = ideal_gas`) is a separate branch of the same helper functions and must stay
  bit-identical.

## Pre-registered checks and limits (not changed after a run; a miss becomes an `#[ignore]`d `*_known_miss` test)

Constants (published critical data; recorded as unverified in `docs/reference-data.md` unless noted):
CO2 `Tc = 304.1282 K, pc = 7.3773 MPa, omega = 0.22394, M = 44.0098 g/mol`; N2 `126.192 K, 3.3958 MPa, 0.0372,
28.0134 g/mol`. Reference data: NIST Chemistry WebBook fluid properties (Span-Wagner CO2, Span et al. N2),
isothermal tables, retrieved 2026-10-14 through a page fetcher with an LLM summariser (so values are marked
"retrieved, not independently re-read" in `docs/reference-data.md`).

**(R1) Density vs NIST, vapour root, PR.** Point list (T K, p MPa, rho_NIST kg/m^3):
CO2 350 K: 0.1 1.5167; 1.1 17.192; 2.1 33.884; 3.1 51.745; 5.1 91.780; 7.1 139.43; 8.1 167.05; 10.1 232.44.
CO2 310 K: 0.1 1.7150; 1.1 19.762; 2.1 39.745; 3.1 62.272; 4.1 88.310; 5.1 119.57; 6.1 159.56; 7.1 217.94;
8.1 352.54 (near critical, Z about 0.39); 9.1 625.60 (dense, Z about 0.27).
N2 300 K: 0.1 1.1233; 5.1 57.468; 10.1 112.81; 15.1 165.25; 20.1 213.45.
N2 140 K: 0.1 2.4239; 3.1 101.00; 6.1 349.60 (Z about 0.42); 9.1 480.77 (Z about 0.45).
Tolerance by the NIST compressibility factor `Z_N = p M/(rho_N R_u T)`: `|Z_N - 1| <= 0.05`: 1 %;
`0.05 < |Z_N - 1| <= 0.25`: 5 %; `0.25 < |Z_N-1| ` and `Z_N >= 0.5`: 20 %; `Z_N < 0.5`: reported only, no tolerance
(a two-parameter cubic EOS is known to be unreliable in the dense/near-critical region; Peng and Robinson 1976 quote
liquid-density errors of this size). The pass criterion counts every point with a tolerance.

**(R2) Departure functions vs NIST differences** (`ideal cp0` set to the NIST 0.1 MPa `cp`, so only differences
are tested): enthalpy difference `h(T, 0.1 MPa) - h(T, p)` at the points listed here (NIST values): CO2 350 K h(0.1)=551.23, h(1.1)=544.50,
h(3.1)=530.03, h(5.1)=513.97 kJ/kg; CO2 310 K h(0.1)=516.00, h(2.1)=497.48, h(5.1)=460.91; N2 300 K h(0.1)=311.20,
h(5.1)=300.76, h(10.1)=291.77, h(20.1)=279.02. Tolerance 10 % of the difference. Entropy difference
`s(T,0.1) - s(T,p)` (CO2 350 K entropies 2.8795, 2.4128, 2.1870, 2.0587 J/g/K at 0.1, 1.1, 3.1, 5.1 MPa; N2 300 K
6.8457, 5.6457, 5.4130, 5.1611 at 0.1, 5.1, 10.1, 20.1 MPa): 2 % of the difference. Speed of sound (cp0 = NIST
0.1 MPa cp; cv0 = cp0 - R): CO2 350 K 0.1 MPa (289.04) 0.5 %, 1.1 MPa (284.40) 2 %, 5.1 MPa (266.00) 5 %;
N2 300 K 0.1 MPa (353.16) 0.5 %, 10.1 MPa (379.90) 5 %. Real `cp` change `cp(T,p) - cp(T,0.1 MPa)`: CO2 350 K, 5.1 MPa
(1.1963 - 0.89903 J/g/K) within 20 %.

**(R3) Ideal-gas limit.** At `p = 1 Pa, T = 300 K` (CO2 and N2, PR and RK): `|rho/rho_ideal - 1| <= 1e-6`,
`|psi R T - 1| <= 1e-6`, `|h_dep| <= 1e-8 cp0 T`, `|s_dep| <= 1e-8 cp0`, `|c/c_ideal - 1| <= 1e-6`. Solver level:
the CO2 subsonic duct of R5 with `p0 = 2 kPa, T0 = 400 K` run with `real_gas` and with `ideal_gas` (same `R = R_u/M`,
`cp0`): total mass flow relative difference <= 1e-3, column-mean Mach difference <= 2e-3, column-mean `p/p0` and
`T/T0` relative difference <= 1e-3.

**(R4) No regression.** `cargo test -p rcfd-api --test baseline_hashes` unchanged and passing; a new fixed-iteration
bitwise hash of the ideal-gas subsonic nozzle (u, p, T, phi after 30 iterations), recorded on the clean tree before
any edit, must be reproduced exactly after the change; the existing `varden_*` and `variable_density` tests pass
unchanged.

**(R5) Real-gas subsonic duct.** ADR 0005 case (a1) geometry (planar duct, `h_in = h_e = 2 h_t`, `h_t = 0.05`,
`L = 40 h_t`, slip adiabatic walls, 200 x 10 cells, linear upwind, SIMPLEC) with CO2 (PR), `p0 = 5 MPa`, `T0 = 400 K`,
`cp0 = 940 J/kg/K`, `mu = 1.8e-5`, throat Mach number (real sound speed) 0.3. The independent reference integrates the
isentropic path with the same EOS **in the test** by an ODE in `p` (`dh = dp/rho`, `dT/dp|_s = T (dv/dT)_p/cp`, with
`cp = dh/dT|_p` by differences of the departure enthalpy) from `(p0, T0)`, not through `s_dep`; the ODE result is
also compared with the `s_dep` root solve to 1e-4 as a self-check. The mass flux function is `G(p) = rho u`
with `u = sqrt(2 (h0 - h))`; `G_max` locates the critical (sonic) state; throat state from `M = 0.3`;
exit pressure from `G(p_e) A_e = G_t A_t` on the subsonic branch. Limits: total mass flow (per unit depth) within
1.0 % of `G_t A_t`; inlet and outlet mass flow equal to 1e-6; column-mean (columns `i = 10, 30, ..., 190`) Mach
within 1.5 %, `p/p0` within 0.5 %, `T/T0` within 0.3 % of the quasi-1-D reference at the local area.

**(R6) Real-gas choked nozzle (F-061 acceptance).** ADR 0005 case (a2) geometry (`h_in = 2 h_t`, `h_e = 1.5 h_t`,
240 x 12 cells), same gas and stagnation state; outlet pressure = real-gas isentropic supersonic value at area ratio
1.5. Mass flow within 1.0 % of `G_max A_t`; column means (`i = 12, 36, ..., 228`, throat column excluded) of Mach within
3 % and `p/p0` within 2 %. The ideal-gas formula with `gamma = cp0/(cp0 - R)` is reported for comparison (expected to
differ by the real-gas effect, not a criterion). Start-up/relaxation choices needed will be disclosed as solver tuning
(the ideal-gas choked case needed 0.7/0.5/0.7 and a back-pressure continuation).

**(R7) Validation messages.** Unit tests in `rcfd-api`: unknown `fluid.eos`, unknown `fluid.species` without constants,
missing critical constants, `low_mach` with `real_gas`, `fluid.gas_constant` set with `real_gas`, `fluid.eos`/`species`
with `constant`/`ideal_gas`, each gives a located message naming the key; `rcfd validate` and the JSON schema list the new
keys.

## Results (appended after the runs; the criteria above are unchanged)

* R1: 20 of 23 toleranced points pass; N2 300 K at 5.1, 10.1, 15.1 MPa miss the 1 percent class (+1.03, +1.68, +2.24 percent).
  Test `r1_peng_robinson_density_vs_nist_known_miss` (ignored); the passing subset is asserted separately.
* R2: entropy differences (max 0.61 percent), sound speeds (max 1.55 percent) and the cp change (-7.5 percent) pass;
  enthalpy differences pass for CO2 (max +9.3 percent) and miss for N2 at 300 K (+18.8, +15.7, +12.3 percent).
* R3: density, psi and sound speed at 1 Pa pass (5e-8 to 1e-7 against 1e-6); the enthalpy and entropy departures miss 1e-8
  for CO2 (3.1e-8/2.1e-8 PR, 2.6e-8/1.7e-8 RK): the departures are proportional to p (second virial), the limit was
  mis-estimated. A supplementary, unregistered check asserts that scaling. Solver level: mass flow 2.1e-5, Mach 2.3e-7,
  p 5.6e-8, T 5.8e-7.
* R4: the recorded ideal-gas hash is reproduced bit for bit, `baseline_hashes` and the `varden_*` tests pass unchanged.
* R5: Mach 7.7e-4, p 4.1e-5, T 8.4e-6, mass flow -2.7e-4, in/out 3.2e-13. R6: mass flow -2.8e-4, Mach 9.2e-4,
  p 1.3e-3; G* real is 5.73 percent above the ideal-gas formula. The choked run used the ideal-gas start-up
  (relaxation 0.7/0.5/0.7, back-pressure continuation); nothing else was tuned.
* R7: `rcfd-api/tests/real_gas.rs`.
