# ADR 0040: Wave 23B, interfacial mass-transfer closures (F-226) and free-surface wave generation and absorption (F-227), library only

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave existed.
Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an
ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect gets a
NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended at the end without
touching a registered limit.

## Scope and dependency fact (stated up front)

The multiphase and free-surface base does NOT exist in the tree: F-055 (volume-of-fluid) and F-127 (Euler-Euler) are `todo`, and
F-226 depends on both, F-227 on F-055. Therefore both items are delivered as LIBRARY-ONLY closures, tested standalone against
closed forms and invariants. There is no case-tree wiring, no schema change and no solver change in this wave; the
case-tree wiring and the CFD-level acceptance statements (1-D Stefan evaporation in a solver, film condensation within 10 percent
of Nusselt in a CFD run, reflected-wave fraction in a free-surface CFD run) wait on F-055/F-127. Both items stop at `partial`.
No VOF solver is built here. The natural home that exists today is a per-cell source-term evaluator that takes phase fractions,
temperatures and pressures as inputs and returns phase sources; that is the shape of the F-226 API. Baseline hashes
(`crates/rcfd-api/tests/baseline_hashes.rs`) stay bit-identical. Clean-room: published literature and first principles only;
no vendor code, text, data or names. Parallel waves 23A and 23C share the tree: the only shared-file edit is two `pub mod` lines
in `crates/rcfd-fv/src/lib.rs`.

## Decisions

### D1 F-226 library (`crates/rcfd-fv/src/masstransfer.rs`, new)

Conventions. `mdot` is the volumetric phase-change rate in kg/(m^3 s), POSITIVE for evaporation (liquid -> vapour). The
returned `PhaseSource { liquid, vapour, energy }` is `liquid = -mdot`, `vapour = +mdot` (kg/(m^3 s)), `energy = -mdot h_fg`
(W/m^3, the source in the mixture enthalpy equation: evaporation removes the latent heat). So the phase sources sum to zero
exactly (bit-exact, they are negatives of one number) and the latent term is exactly `mdot h_fg`. Saturation data
(`T_sat(p)`, `p_sat(T)`, `h_fg`, densities) are INPUTS (the caller takes them from the F-220 saturation line
`PtTable::psat/tsat` or from `rcfd_core::iapws::saturation_pressure`); no property data is shipped. All entry points validate
their inputs and return `Err(String)` whose text names the offending parameter (`alpha_l`, `t_sat`, ...).

* **Lee model (temperature driven).** With `theta = (T - T_sat)/T_sat`:
  `mdot = c_e alpha_l rho_l theta` for `T > T_sat` (evaporation, uses the liquid present);
  `mdot = c_c alpha_v rho_v theta` for `T < T_sat` (condensation, `theta < 0` so `mdot < 0`, uses the vapour present);
  `mdot = 0` at `T = T_sat` (bit-exact). `c_e`, `c_c` are relaxation frequencies in 1/s (>= 0; case-dependent, tuned in
  practice, no default is claimed). The form is that of the model usually attributed to Lee (1980); the literature entry is
  recorded as UNVERIFIED (formula reproduced from knowledge, not read here).
* **Hertz-Knudsen-Schrage (kinetic).** Interfacial flux (kg/(m^2 s), positive evaporation)
  `j = f(sigma) sqrt(M/(2 pi R_u)) ( p_sat(T_l)/sqrt(T_l) - p_v/sqrt(T_v) )`, with `f = sigma` (plain Hertz-Knudsen) or
  `f = 2 sigma/(2 - sigma)` (Schrage correction for the net bulk motion), accommodation coefficient `sigma in (0, 1]`,
  `R_u = 8.314462618 J/(mol K)`, `M` in kg/mol. Volumetric rate `mdot = j a_i` with the interfacial area density
  `a_i` (1/m, an input; a VOF solver supplies `|grad alpha|`). `j = 0` exactly when `p_v = p_sat(T_l)` and `T_v = T_l`.
  The maximum (evaporation into vacuum) of plain Hertz-Knudsen with `sigma = 1` is the kinetic-theory one-way flux
  `p sqrt(M/(2 pi R_u T))` (= `n m vbar/4` with `vbar = sqrt(8 R_u T/(pi M))`), used as an independent check.
* **Pressure driven (Zwart-Gerber-Belamri form), DERIVED here from the Rayleigh-Plesset equation.** Neglecting surface tension
  and viscosity and the second-derivative term, a bubble of radius `R_B` grows at `dR/dt = sqrt((2/3) (p_v - p)/rho_l)`
  (negative = collapse). Each bubble carries vapour mass flow `rho_v 4 pi R_B^2 dR/dt`; with the number density `N` of
  bubbles, `alpha_v = N (4/3) pi R_B^3`, so `N 4 pi R_B^2 = 3 alpha_v/R_B`. Hence
  `mdot = F_vap (3 alpha_nuc (1 - alpha_v) rho_v/R_B) sqrt((2/3)(p_v - p)/rho_l)` for `p < p_v` (nucleation sites replace
  `alpha_v` because no vapour exists to grow from), and
  `mdot = -F_cond (3 alpha_v rho_v/R_B) sqrt((2/3)(p - p_v)/rho_l)` for `p > p_v`, zero at `p = p_v`. Empirical factors
  `F_vap`, `F_cond`, `alpha_nuc`, `R_B` are inputs (the values 50, 0.01, 5e-4, 1e-6 m are the ones usually quoted with the
  original paper; recorded UNVERIFIED and offered only as `Default`, with the library tests not depending on them).
* **Raoult's law (ideal liquid mixture, ideal gas).** `y_i p = x_i p_sat,i(T)`. API: `bubble_pressure(x, psat) = sum x_i psat_i`,
  `dew_pressure(y, psat) = 1/sum(y_i/psat_i)`, `raoult_vapour_fractions(x, psat, p)` (`y_i = x_i psat_i/p`, no normalisation; their
  sum is 1 exactly at the bubble pressure, > 1 below it, < 1 above it), and `k_values`. Optional activity coefficients
  `gamma_i` (default 1) multiply `psat_i`.
* **Henry's law with van 't Hoff temperature dependence.** `p_i = H(T) x_i` (volatility form, Pa) with
  `H(T) = H_0 exp(-C (1/T - 1/T_0))`, `C = -Delta_sol H/R_u` in K (`C > 0`: solubility falls with temperature, so `H` rises).
  `C = 0` gives a temperature-independent constant. `henry_liquid_fraction(y, p, H) = y p/H`. For an ideal solution
  `H = p_sat` and Henry's law coincides with Raoult's (used as a consistency check).
* **Species transfer rate.** `m_i = k a_i rho_r (w_eq - w)` kg/(m^3 s), positive into the receiving phase `r`
  (mass-transfer coefficient `k` m/s, area density `a_i`, receiving-phase density, equilibrium and actual mass fractions);
  the donor source is `-m_i` so a species conserves exactly. Mole -> mass fraction helper for a mixture.
* **Wall condensation (Nusselt film, laminar, vertical plate, constant wall temperature, saturated stagnant vapour,
  negligible vapour shear and sub-cooling of the film).**
  `delta(x) = [4 mu_l k_l (T_sat - T_w) x/(g rho_l (rho_l - rho_v) h_fg)]^(1/4)`; local `h(x) = k_l/delta`;
  wall mass flux `m''(x) = k_l (T_sat - T_w)/(delta h_fg)` (so the wall heat flux is `m'' h_fg`);
  film mass flow per unit width `Gamma(x) = rho_l (rho_l - rho_v) g delta^3/(3 mu_l)`; mean coefficient over length `L`
  `h_L = (4/3) h(L) = 0.9428 [g rho_l (rho_l - rho_v) k_l^3 h_fg/(mu_l (T_sat - T_w) L)]^(1/4)` (the familiar 0.943).
  Optional Rohsenow correction `h_fg' = h_fg + 0.68 c_p (T_sat - T_w)`, off by default (UNVERIFIED coefficient).
  Wall-cell source: `mdot_cell = m'' A_wall/V_cell` (condensing: the vapour source is negative, the liquid source positive).
  `T_w >= T_sat` gives zero flux, `delta = infinity` is not returned (the function returns a zero flux and delta NaN is
  avoided: `delta` returns an `Err` for `T_w >= T_sat`).
* **0-D relaxation cell (`LeeCell`).** One cell of volume 1 with liquid mass `m_l`, vapour mass `m_v` (per volume), temperature `T`,
  heat capacity per volume `C = (rho c_p)` of the cell (constant), Lee rate on `alpha_l rho_l = m_l`. ODE
  `dm_v/dt = -dm_l/dt = c_e m_l (T - T_sat)/T_sat`, `C dT/dt = -h_fg dm_v/dt`. Integrated by classical RK4. With the liquid
  mass frozen (`frozen = true`; valid when the evaporated mass is small) the ODE is linear:
  `T - T_sat = (T_0 - T_sat) e^{-lambda t}`, `lambda = h_fg c_e m_l/(C T_sat)`. The depleting case conserves
  `C T + h_fg m_v` and `m_l + m_v` (both linear invariants, preserved by RK4 to round-off).

### D2 F-227 library (`crates/rcfd-fv/src/waves.rs`, new)

Coordinates: `x` along propagation, `z` vertical, `z = 0` the still-water level, bed at `z = -h`; phase `theta = k x - omega t + phi`.

* **Dispersion** `omega^2 = g k tanh(k h)`, solved for `k` given `omega, h, g` by Newton iteration on
  `f(k) = g k tanh(kh) - omega^2` from the Eckart-type start `k_0 = omega^2/g / sqrt(tanh(omega^2 h/g))`, until
  `|f|/omega^2 <= 1e-14` (cap 100 iterations, then an error). `h = INFINITY` uses `k = omega^2/g`.
  Derived: phase speed `c = omega/k`, group velocity `c_g = (c/2)(1 + 2kh/sinh 2kh)`.
* **Airy (linear) wave**, amplitude `a`:
  `eta = a cos(theta)`, `u = a omega cosh(k(z+h))/sinh(kh) cos(theta)`, `w = a omega sinh(k(z+h))/sinh(kh) sin(theta)`,
  dynamic pressure `p_d = rho g a cosh(k(z+h))/cosh(kh) cos(theta)`, potential `phi = (a g/omega) cosh(k(z+h))/cosh(kh) sin(theta)`
  (so `u = d phi/dx`, `w = d phi/dz`). The depth ratios are evaluated in the overflow-safe exponential form. Energy density
  `E = rho g a^2/2`, energy flux `E c_g`, mean mass flux (wave momentum) `E/c` (derived below, checked).
* **Second-order Stokes wave** (first-harmonic amplitude `a` fixed, dispersion relation unchanged at this order, Eulerian
  description, no mean-flow term at fixed `z`), formulas from the standard textbook solution (UNVERIFIED citation, formulas from
  knowledge; they are checked below against the free-surface boundary conditions, which is independent of their provenance):
  `eta = a cos(theta) + (k a^2/4) cosh(kh)(2 + cosh 2kh)/sinh^3(kh) cos(2 theta)`,
  `u = a omega cosh(k(z+h))/sinh(kh) cos(theta) + (3/4) a^2 omega k cosh(2k(z+h))/sinh^4(kh) cos(2 theta)`,
  `w = a omega sinh(k(z+h))/sinh(kh) sin(theta) + (3/4) a^2 omega k sinh(2k(z+h))/sinh^4(kh) sin(2 theta)`,
  `phi = (a g/omega) cosh(k(z+h))/cosh(kh) sin(theta) + (3/8) a^2 omega cosh(2k(z+h))/sinh^4(kh) sin(2 theta)`.
  Deep-water limit of the second harmonic of `eta`: `k a^2/2`; shallow limit `3 a^2/(4 k^2 h^3)`.
* **Wave-inlet boundary values**: `inlet_state(x_b, t, z)` returns `(eta, u, w)` for a chosen order (1 or 2), multiplied by a smooth
  ramp `ramp(t, T_r) = (1 - cos(pi t/T_r))/2` (0 at `t <= 0`, exactly 1 at `t >= T_r`); `liquid_fraction(z_c, dz, eta)`
  returns the exact volume fraction below the surface in a cell `[z_c - dz/2, z_c + dz/2]`, `clamp((eta - (z_c - dz/2))/dz, 0, 1)`.
  These are the values a F-055 inlet would impose; no BC object is wired.
* **Irregular waves.** Spectra in angular frequency, Bretschneider/Pierson-Moskowitz in significant-height/peak form
  `S(w) = (5/16) Hs^2 wp^4 w^-5 exp(-1.25 (wp/w)^4)` (exact `m0 = Hs^2/16`), and JONSWAP
  `S(w) = A g^2 w^-5 exp(-1.25 (wp/w)^4) gamma^r`, `r = exp(-(w - wp)^2/(2 s^2 wp^2))`, `s = 0.07` (`w <= wp`) or `0.09`,
  with `A` fixed numerically so that `m0 = Hs^2/16` (peak-enhancement factor `gamma = 3.3` default). Deterministic-amplitude
  discretisation: `N` midpoint bins of width `d_w` on `[w_lo, w_hi]`, `a_i = sqrt(2 S(w_i) d_w)`, `k_i` from the dispersion,
  phases `phi_i` from a seeded `splitmix64` stream (own implementation, no dependency), uniform on `[0, 2 pi)`,
  bit-reproducible for a seed. `eta(x,t) = sum a_i cos(k_i x - w_i t + phi_i)`; velocities by summing the Airy kinematics. The
  record repeats with period `4 pi/d_w` (midpoint frequencies are odd multiples of `d_w/2`); that is the maximum record length
  without repetition and is returned. Moments `m_n = int w^n S dw` by midpoint quadrature; `Hs = 4 sqrt(m0)`.
* **Relaxation zone / numerical beach.** With the zone coordinate `chi in [0,1]` (0 at the inner edge where the full solution
  lives, 1 at the outer edge; `zone_chi(x, x_inner, x_outer)` works for either orientation and clamps), the weight of the
  computed solution is
  `exponential`: `w(chi) = 1 - (exp(chi^p) - 1)/(e - 1)`, `p = 3.5` default (the blending usually attributed to the waves2Foam
  authors, UNVERIFIED), or `polynomial`: `w(chi) = (1 - chi)^n`, `n >= 1`. Endpoints are exact (`w(0) = 1`, `w(chi >= 1) = 0`
  bit-exact by clamping), monotone decreasing. The field is blended as `phi <- w phi + (1 - w) phi_target`. Also a sponge rate
  `sigma(chi) = sigma_max chi^n` (1/s) with `int sigma dx = sigma_max L/(n+1)`.
* **1-D reflection toy (`WaveTank`)**: linear shallow-water equations `eta_t = -h u_x`, `u_t = -g eta_x` on a staggered grid,
  leapfrog (`eta` at cell centres and integer time, `u` at faces and half time), left face `u_0 = (c/h) eta_inc(t)`
  (`c = sqrt(g h)`, incident wave with the ramp above), right end wall `u_N = 0`, optional relaxation zone at the right end
  applying `eta, u <- w eta, w u` after each step (target = still water; a pure sponge `exp(-sigma dt)` variant is also
  provided and only reported). Numerical-scheme caveat: the per-step blending strength depends on `dt` (inherent to the blending
  form). The reflection coefficient is `R = |D_omega|/|I_omega|`, with `D = eta_beach - eta_ref` at a gauge, `eta_ref` from an
  otherwise identical run in a domain long enough that no reflection returns inside the window, `I = eta_ref`, and `X_omega` the
  single-frequency complex Fourier amplitude over an integer number of periods (exact for the sampled harmonic).

## Registered checks

Library tests: `crates/rcfd-fv/tests/masstransfer.rs` (M) and `crates/rcfd-fv/tests/waves.rs` (W). Limits are relative unless
stated. "hand" means the expected value is written in the test as an independent literal expression, not by calling the function.

**M1 Lee exact values.** `T = 380, T_sat = 373.15, alpha_l = 0.6, rho_l = 958, c_e = 0.1`: `mdot` equals `0.1*0.6*958*(380-373.15)/373.15`
to 1e-14. Condensation `T = 360, alpha_v = 0.3, rho_v = 0.6, c_c = 0.2`: equals `-0.2*0.3*0.6*(373.15-360)/373.15` to 1e-14.
`T = T_sat`: `mdot == 0` exactly. Sign: `mdot > 0` iff `T > T_sat` over a 200-point scan. `alpha_l = 0` gives no evaporation (`== 0`),
`alpha_v = 0` no condensation (`== 0`). Linear in `T - T_sat` (doubling `theta` doubles `mdot` to 1e-14).
**M2 conservation (all models).** For 2000 deterministic pseudo-random states per model (Lee, HKS, ZGB): `liquid + vapour == 0`
exactly (`<= 0` ulp, i.e. `==`), `energy == -mdot h_fg` exactly, and all three change sign with the driving difference.
**M3 Hertz-Knudsen-Schrage.** `M = 0.018015, T_l = T_v = 373.15, p_sat = 101325, p_v = 95000, a_i = 100`: for `sigma = 0.8`, Schrage
and plain forms equal the hand expressions `(2*0.8/(2-0.8))*sqrt(0.018015/(2*pi*8.314462618))*(101325-95000)/sqrt(373.15)*100` and
the `sigma` version to 1e-14. `p_v = p_sat`, `T_v = T_l` gives exactly 0; `p_v > p_sat` gives `mdot < 0`. With `sigma = 1`, plain form, `p_v = 0`:
`j = p sqrt(M/(2 pi R_u T))` against the independent kinetic-theory expression `0.25 * (p M/(R_u T)) * sqrt(8 R_u T/(pi M))` to 1e-13.
Temperature jump sign: `T_l > T_v` at `p_v = p_sat(T_v)` evaporates (using `rcfd_core::iapws::saturation_pressure` for `p_sat(T_l)`).
**M4 pressure driven (ZGB).** hand values for `p = 0.9 p_v` (vaporisation) and `p = 1.2 p_v` (condensation) with `rho_l = 998, rho_v = 0.023,
p_v = 2339, R_B = 1e-6, alpha_nuc = 5e-4, F_vap = 50, F_cond = 0.01`, to 1e-14; zero at `p = p_v` (`== 0`); `mdot` for `4 |dp|` is twice that for `|dp|`
(1e-14); the bubble-dynamics reading `rho_v N 4 pi R_B^2 dR/dt` with `N = 3 alpha/(4 pi R_B^3)` matches to 1e-13;
`alpha_v = 1` gives no vaporisation, `alpha_v = 0` no condensation.
**M5 Raoult.** Binary `x = (0.3, 0.7)`, `psat = (1.0e5, 4.0e4)`: `bubble_pressure = 5.8e4` to 1e-15 (hand), `y = (0.3*1e5/5.8e4, 0.7*4e4/5.8e4)`, `sum y = 1` to 1e-15 at the
bubble pressure, `> 1` at `0.9 p_b`, `< 1` at `1.1 p_b`. Ternary at three compositions: `sum y = 1` to 1e-14 at the bubble pressure. Pure component: `y = psat/p`.
Dew/bubble round trip: `x_i = y_i p_d/psat_i` sums to 1 to 1e-14. Located errors: negative fraction (`x[1]`), fractions not summing to 1 within 1e-9 (`sum`),
`psat <= 0` (`psat[0]`), length mismatch (both lengths), `p <= 0`.
**M6 Henry.** `H(T_0) == H_0` exactly; `d ln H/d(1/T) = -C` by central difference (step 1e-6 in `1/T`) to 1e-6 relative; `C > 0` gives `H` increasing in `T` over a 100-point scan;
`C = 0` gives the same `H` at every `T` (`==`); `x_eq = y p/H` hand value to 1e-14; Henry with `H = p_sat` equals Raoult for the same `x` (`y` equal to 1e-15); mass-fraction round trip to 1e-14.
Transfer rate: zero at `w = w_eq` (`== 0`), sign follows `w_eq - w`, donor source `= -` receiver source (`==`).
**M7 Nusselt film.** Water at 1 atm values `rho_l = 958.4, rho_v = 0.5977, mu = 2.82e-4, k = 0.679, h_fg = 2.257e6, g = 9.81`, `T_sat - T_w = 10 K`: `delta(0.5)` equals the hand
expression to 1e-14; scaling `delta ~ (dT x)^(1/4)` (16 times `dT` doubles `delta`, 1e-14); `m'' h_fg = k dT/delta` (1e-14); `Gamma(x) = rho_l (rho_l - rho_v) g delta^3/(3 mu)` equals
`int_0^x m'' dx'` (exact Simpson in `u`, `x = u^4`, 1e-12); `h_L = (4/3) h(L)` (1e-14) and the coefficient `(4/3)(1/4)^(1/4)` is within 3e-4 of 0.943;
`T_w >= T_sat` gives zero flux and `delta` an `Err`. Wall-cell source `= m'' A/V` (`==`) with opposite signs for the phases.
**M8 relaxation cell.** Frozen mass, `T_0 = 400, T_sat = 373.15, c_e = 0.1, m_l = 500, C = 2e6, h_fg = 2.257e6`: RK4, 200 steps to `lambda t = 3`, `max |T - T_exact| <= 1e-9 (T_0 - T_sat)`.
Depleting: `C T + h_fg m_v` and `m_l + m_v` conserved to 1e-12; `|T - T_sat|` non-increasing; `T` approaches `T_sat` from above and never undershoots for `lambda dt <= 0.1`; liquid mass stays positive. Condensing side (`T_0 < T_sat`, vapour supplied) symmetric (`T` rises to `T_sat`).
**M9 located input errors.** `alpha_l` outside `[0,1]` (`alpha_l`), `rho <= 0`, `T_sat <= 0` (`t_sat`), negative coefficient, NaN, `sigma` outside `(0,1]` (`sigma`), `p <= 0`, `h_fg <= 0`: each `Err` containing the parameter name.

**W1 dispersion.** For `h in {0.5, 2, 10, 50} m`, `T in {2, 5, 9, 15} s`, `g = 9.81`: `|omega^2 - g k tanh(kh)|/omega^2 <= 1e-12` (target of the solver 1e-14); deep (`h = INFINITY`) `k = omega^2/g` bit-exact; for `kh >= 6`
`|k - omega^2/g|/k <= 2.1 e^{-2kh}`; shallow for `kh <= 0.2`: `1 - c/sqrt(g h)` lies in `[(kh)^2/6 - 0.06 (kh)^4, (kh)^2/6]`; `c_g = d omega/dk` by central difference to 1e-8;
`c_g/c -> 1/2` deep (`|c_g/c - 1/2| <= 2e-4` at `kh = 6`) and `-> 1` shallow (`1 - c_g/c <= (kh)^2` at `kh <= 0.2`). Bad input (`omega <= 0`, `h <= 0`, `g <= 0`): located errors.
**W2 Airy kinematics and invariants.** `a = 0.5, h = 3, T = 6`: periodic in `x` (by `lambda`) and `t` (by `T`) to 1e-13; `d eta/dt = w(z=0)` to 1e-12; `w(z = -h) == 0` (`<= 1e-15 a omega`);
continuity `du/dx + dw/dz = 0` by central differences (step 1e-5) to 1e-8 relative to `a omega k`; Laplace via `u = d phi/dx, w = d phi/dz` central differences 1e-8;
orbital amplitude ratio `u_amp(z)/u_amp(0) = cosh(k(z+h))/cosh(kh)` to 1e-14, and for `kh = 6`, `z in [-h/4, 0]`: relative difference from `e^{kz}` `<= 2 e^{-2k(z+h)}`;
time-mean kinetic energy integrated over depth (Simpson 2000 intervals, 64 time samples) `= rho g a^2/4` to 1e-8; mean `eta u(0)` over a period `= E/(rho c)` to 1e-12; bed dynamic pressure `rho g a/cosh(kh)` to 1e-14.
**W3 second-order Stokes.** (i) deep limit of the second harmonic `k a^2/2` to 1e-6 relative at `kh = 12`; shallow limit `3 a^2/(4 k^2 h^3)` to 2e-2 at `kh = 0.05`; crest height minus trough depth `= 2 eta_2 > 0`.
(ii) kinematic free-surface residual `eta_t + u eta_x - w` at `z = eta`, Taylor-expanded about `z = 0` through second order, and (iii) the dynamic residual `g eta + phi_t + eta phi_tz + (u^2 + w^2)/2` at `z = 0` with the mean over a period removed:
with `k a = 1e-3` the ratio of the maximum residual (over 720 phases) at `a` to that at `a/2` lies in `[7.5, 8.5]` (third-order scaling) for the kinematic residual (scaled by `a omega`) and for the dynamic residual (scaled by `g a`) at `kh in {0.5, 1.5, 4}`. No absolute residual bound is registered.
First-order only is shown to FAIL the same dynamic residual test with ratio near 4 (diagnostic, asserted ratio in `[3.5, 4.5]`), which proves the test discriminates.
**W4 inlet and fractions.** `ramp(0) == 0`, `ramp(T_r) == 1` bit-exact, monotone; `liquid_fraction` is 0 above the cell, 1 below, 0.5 when the surface is at the cell centre, linear between (hand), and the column sum of `liquid_fraction*dz` equals `eta + h` to 1e-14; `inlet_state` order 1 equals the Airy functions times the ramp.
**W5 irregular waves.** Pierson-Moskowitz, `Hs = 2.5, Tp = 9`, `N = 200`, `[0.4 wp, 6 wp]`: recovered `Hs = 4 sqrt(sum a_i^2/2)` within 1e-3 relative of 2.5 (truncation estimate 5e-4); closed-form moments `m1 = (5/64) Hs^2 wp 1.25^(-3/4) Gamma(3/4)` (to 2e-3, wide-range quadrature `[0.3 wp, 40 wp]`, 20000 bins) and
`m2 = (5/64) Hs^2 wp^2 1.25^(-1/2) sqrt(pi)` (to 2e-3); spectrum peak (argmax on a 1e-4 grid) at `wp` within 1e-3. JONSWAP `gamma = 3.3`, same `Hs`: recovered `Hs` within 2e-3 (normalisation), argmax of the spectrum in `[wp, 1.05 wp]`; JONSWAP with `gamma = 1` (normalised) equals the Pierson-Moskowitz spectrum to 1e-6 relative at 50 frequencies.
Reproducibility: same seed gives bit-identical phases and `eta`; different seeds differ; phases in `[0, 2 pi)`; `|mean cos phi|, |mean sin phi| <= 0.15` for the registered seed 12345 (3 sigma of the sample mean). Record variance over one repetition period (exact trapezoid, enough samples) `= sum a_i^2/2` to 1e-10 relative; each `k_i` satisfies the dispersion to 1e-12; deep-water point velocity equals the sum of Airy velocities (`==` up to 1e-13).
**W6 relaxation weights.** Exponential: `w(0) == 1`, `w(1) == 0`, `w(chi > 1) == 0`, `w(chi < 0) == 1`; strictly decreasing on a 10001-point grid; `w(0.5)` equals `1 - (exp(0.5^3.5) - 1)/(e - 1)` (hand) to 1e-14; flat start `1 - w(0.01) <= 1e-6`. Polynomial: `w = (1 - chi)^n` to 1e-14, exact endpoints, monotone. `zone_chi` orientation both ways. Sponge: `sigma` endpoints, `int sigma dx` by Simpson `= sigma_max L/(n+1)` to 1e-12.
**W7 1-D reflection (registered with the acceptance limit).** `g = 9.81, h = 1, T = 8 s` (`c = sqrt(g h)`, `lambda = c T`), `dx = lambda/40`, `dt = T/80` (CFL 0.5), ramp `2T`, gauge `9 lambda`, zone start `12 lambda`, Fourier window as derived: start `(2 x_e - x_g)/c + 3T`, length `3T`, which ends before the left re-reflection `(2 x_s + x_g)/c` (asserted in the test).
(a) rigid end wall (no zone): `R in [0.995, 1.005]`. (b) exponential beach, `L = 2 lambda`, `p = 3.5`: `R <= 0.05`. (c) the same with `L = 4 lambda`: `R_4 <= R_2`. (d) abrupt zone (`L = 0.05 lambda`): `R >= 0.5`. Sponge variant reported only (`sigma_max` chosen as `3 c/L` and printed), no limit.
Energy accounting for the incident wave: the Fourier amplitude of the reference run equals `a` times the ramp-saturated value within 2 percent (pure scheme dispersion/impedance check, registered).

## Not delivered (named gaps, written in advance)

F-226: case-tree wiring and a VOF/Euler-Euler host (F-055/F-127); the 1-D Stefan evaporation acceptance (needs a solver with a moving interface; the closed Nusselt form and the 0-D relaxation are the library checks); the CFD-level film-condensation-within-10-percent
acceptance; temperature-dependent property feeding from the case tree; multi-component (more than Raoult/Henry) equilibrium such as activity-coefficient models; mass transfer in Euler-Euler with interfacial-area transport (F-228).
F-227: case wiring of wave inlets and relaxation zones into a free-surface solver (needs F-055), higher-order (third and above) Stokes and cnoidal/stream-function waves, directional spectra, wave-maker (piston) boundaries, 2-D/3-D reflection tests, nonlinear (non-shallow-water) beach tests.

## Addendum (end of wave 23B): measured results and deviations

Registered limits above are unchanged. Everything below is what happened. Measured on the committed code (`cargo test --release -p rcfd-fv`).

### Measured against the registered checks

- M1-M9 all pass. M3 kinetic one-way flux against the independent kinetic-theory expression, within 1e-13; M5 bubble pressure 5.8e4 to 1e-15 and `sum y = 1` to 1e-15; M7 water film at 1 atm, 10 K subcooling, `x = 0.5 m`: `delta = 1.1716e-4 m`, `h_L = 7727.3 W/(m^2 K)`, film Reynolds number 242.8 (reported; far above the laminar-wavy limit near 30, so at 0.5 m this is the closed form evaluated, not a claim that a laminar film is physical there; the 0.943 coefficient check and the energy and mass-flow identities hold to 1e-12 to 1e-14); M8 frozen-mass relaxation, RK4 200 steps to `lambda t = 3` (`lambda = 0.1512 1/s`): `max |T - T_exact| = 6.38e-11 (T_0 - T_sat)` (limit 1e-9); depleting and condensing runs conserve the invariants and relax monotonically.
- W1: worst dispersion residual 1.83e-15 (limit 1e-12); 3 deep cases (`kh >= 6`) and 3 shallow cases (`kh <= 0.2`) in the grid, bounds hold; `c_g = d omega/dk` to 1e-8. W2 passes (kinetic energy `rho g a^2/4`, mean flux `E/(rho c)`, continuity, Laplace, bed). W4 passes.
- W3 (i) limits and the naive-hyperbolic cross-check of the formulas pass. W3 (ii)/(iii): see deviation 1. Corrected test w3b: second-order kinematic and dynamic residuals scale with ratio 8.006 / 8.010 (`kh = 0.5`), 8.0004 / 8.000002 (`kh = 1.5`), 8.000003 / 8.0000008 (`kh = 4`); the first-order dynamic residual scales with ratio 4.000000 and is larger than the second-order one by more than 10x at all three depths (the factor 10 was added in the corrected test and not registered).
- W5: Pierson-Moskowitz recovered `Hs` 2.4988 for 2.5 (relative 4.82e-4, limit 1e-3, truncation estimate 5e-4); `m1` relative 2.0e-5 and `m2` 7.88e-4 (limit 2e-3); JONSWAP recovered `Hs` relative 3.32e-4 (limit 2e-3); peak and `gamma = 1` equality pass; phase means cos -0.0242 and sin 0.0994 (limit 0.15); variance over one repetition period equals `sum a^2/2` to 1e-10 on a commensurate grid.
- W6 passes. W7: incident amplitude ratio 1.00000 (limit 2 percent), rigid wall `R = 1.00000` (limit [0.995, 1.005]), exponential beach `L = 2 lambda`: `R = 2.6e-4` (limit 0.05), `L = 4 lambda`: `R < 5e-6` (`R_4 <= R_2` holds). Sponge variant (`sigma_max = 3c/(2 lambda) = 0.1875 1/s`, `n = 2`, `L = 2 lambda`), reported only: `R = 0.135`.
- **W7 (d) MISSES** (known miss, ignored test `w7d_abrupt_zone_reflects_half_known_miss`): abrupt zone `L = 0.05 lambda` gives `R = 0.214`, registered limit `>= 0.5`. Probable cause (not separately tested): the premise of the check was wrong. The relaxation multiplies the field by `w < 1` at every time step in two cells, and a wave reflected at the wall inside that zone passes those cells repeatedly (80 steps per period), so even a two-cell zone dissipates much of the wave energy; a thin relaxation zone is not a rigid reflector. Not tuned.

### Deviations from the registered text

1. **W3 (ii)/(iii) scaling statement is a registered-text defect.** The registered text divided the residual by `a omega` and `g a` and then expected the amplitude-halving ratio 8 (and 4 for first order). With that division an `O(a^3)` residual scales like `a^2` (ratio 4), so the registered check cannot pass for any correct second-order solution. Kept as the ignored test `w3_residual_scaling_amplitude_scaled_known_miss` (measured ratio 4.003 kinematic, 4.005 dynamic at `kh = 0.5`). Corrected test `w3b_residual_scaling_absolute` divides by the amplitude-independent constants `omega/k` and `g/k`, with the same ratio limits [7.5, 8.5] and [3.5, 4.5]; it passes (numbers above). The formulas themselves are validated by it (the second-harmonic coefficient of `eta`, the second-order velocity and potential satisfy both free-surface conditions through second order).
2. **W7 (d) corrected set-up written after seeing the numbers.** Because the registered discrimination check failed on a wrong premise, test `w7e_abrupt_zone_vs_beach` was added with the post-hoc limits `R_abrupt >= 0.1` and `R_abrupt >= 50 R_beach` (measured 0.2144 and 828x). This is an observation-based statement, not a pre-registered one, and is labelled so.
3. The repetition period `4 pi/d_omega` returned by `IrregularWave::repetition_period` holds exactly only when `w_lo` is an integer multiple of `d_omega` (the ADR text was unconditional). The W5 variance check therefore uses a commensurate grid (`w_lo = 20 d_omega`); the Hs check uses the registered range `[0.4 wp, 6 wp]`.
4. Saturation data enter as arguments (`t_sat`, `p_sat`); `rcfd_core::iapws::saturation_pressure` supplies `p_sat` in test M3. The F-220 `PtTable` is not called inside the library (a pure source-term evaluator needs no such dependency).
5. `LeeCellParams` uses one frequency `c` for both evaporation and condensation of the cell (the ADR text described `c_e`); the condensation branch acts on the vapour mass.
6. `Wave` carries both orders in one type (`order` 1 or 2) instead of separate Airy and Stokes types; `potential_t`, `potential_tz`, `dw_dz` and `eta_t/eta_x` are exposed for the boundary-condition checks.
7. Shared-tree note: development tests were run on a `git archive` copy of HEAD plus my four files and two `pub mod` lines, because another wave's uncommitted `lorentz.rs` did not compile in the shared tree. The code commit adds only the two `pub mod` lines to `crates/rcfd-fv/src/lib.rs`, staged through a temporary index so that other waves' hunks are not swept in.

### Not delivered (in addition to the list above)

Everything on the "Not delivered" list stands. Additionally: a Lee-versus-HKS coefficient relation (linearisation of the kinetic flux to a Lee frequency); the Rohsenow coefficient 0.68 is unverified; no 2-D/3-D free-surface verification of any wave closure.

8. Post-gate wording fix (docs only): the roadmap note and reference-data row had said the Stokes formulas satisfy the free-surface conditions "to third order"; the correct statement is through second order (the residual is third order). Registered Decisions mention a toolbox name in D2; the reference-data entry cites authors and year only.
