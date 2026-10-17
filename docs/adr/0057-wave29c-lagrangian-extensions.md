# ADR 0057: Wave 29C, Lagrangian extensions: two-way exchange, discrete random walk dispersion, injection distributions (F-128, F-056)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave and before any run
of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up
defect gets a NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended at the end.

Clean-room: first principles and published literature only. No vendor code, text or installs, nothing from
vendor material. Waves 29A (rotor interfaces), 29B (mixture multiphase) and 29D (parallel) work in the same tree. This
wave touches `crates/rcfd-particles` (new modules, additive changes to `tracker.rs`, `inject.rs`, `rng.rs`), `rcfd-api/src/particles.rs`
and `particle_run.rs` (additive keys) and docs; it does not touch `sim.rs`, `schema.rs` or `lib.rs` of other crates, so the baseline
hashes and all existing tests are unchanged by construction (struct literals of `Particle`, `Flow`, `InjectorSpec` in existing tests
stay valid: new state lives in parallel vectors and optional extension structs).

## Audit (HEAD 67dc5d3)

* `rcfd-particles` (wave 18C/21C): one-way point particles, `Tracker::advance` calls `advance_particle` per parcel; each sub-step
  integrates drag + buoyant gravity (`step_particle`) and walks the segment across cells; reflection/stick/escape at boundaries.
  Injection: point or surface, rate in parcels/s, constant or Rosin-Rammler diameters sampled from the MASS oversize fraction with
  EQUAL parcel counts (documented simplification: the sampled number distribution is then mass-biased).
* The carrier has no hook that accepts a per-cell momentum/energy source in every algorithm branch of `Simulation::step`
  (`body_force` is assembled per branch in `sim.rs`, a file other wave-29 agents edit). The solver-loop feedback is therefore NOT
  wired in this wave; two-way exchange is delivered as a conservative library that produces per-cell sources and is checked in a
  closed particle/carrier loop (see "Not delivered").
* The k and epsilon fields of the turbulence model are not exposed to the particle hook; dispersion uses constants in the case tree
  (homogeneous turbulence) and per-cell arrays in the library.

## Decisions

D1 Exchange (two-way). A particle (parcel weight `n`, mass `m = n rho_p pi d^3/6`) moves under drag from the carrier and external
gravity with buoyancy `g_eff = (1 - rho_f/rho_p) g`. Per integration sub-step of length `h` from `(x0, v0)` to `(x1, v1)` (before any
wall reflection) the DRAG IMPULSE on the particle is `J = m (v1 - v0) - m g_eff h` [N s]. The carrier receives `-J`. Wall reflection
changes the particle momentum without the carrier; its impulse `m (v_after - v_before)` is booked in `wall_impulse`. Distribution:
the whole sub-step impulse goes to the cell that contains the START of the sub-step (sub-steps are limited to `courant` (0.5) of a
cell length, so the point is within one cell length of the true path); a cell receives the sum, the source is
`S_c = -sum J / (V_c dt)`. This is conservative to round-off by construction: `sum_p J_p = -sum_c S_c V_c dt` up to floating-point summation only.
Energy: particle total energy is `E_p = 1/2 m |v|^2 + m c_p T_p`. The carrier TOTAL ENERGY receives
`-(Delta KE - W_g + m c_p Delta T_p)` with `W_g = m g_eff . (x1 - x0)`, i.e. exactly what the particle gained from the carrier (drag work,
including the dissipated part `F.(u - v) dt` that heats the carrier, plus heat). Heat transfer (optional, needs a carrier temperature
field and constants `k_f, c_pf, c_p`): `Nu = 2 + 0.6 Re^(1/2) Pr^(1/3)` (Ranz-Marshall, secondary source), lumped sphere
`dT_p/dt = (T_f - T_p)/tau_T`, `tau_T = rho_p c_p d^2/(6 k_f Nu)`, exponential update with `T_f` and `Nu` frozen over the sub-step
(exact for frozen data). The mechanical and the heat parts are stored separately (`energy_mech`, `energy_heat`) and summed in the
total-energy source `S_E` [W/m^3]. Coupling is observation-only for the particles: the tracker never reads the sources, so the
trajectories with coupling on are bit-identical to coupling off; the CALLER feeds the sources into its carrier equations.
The carrier loop used in the checks is an explicit cell momentum update `u_c += S_c dt / rho_f`.

D2 Discrete random walk (Gosman and Ioannides 1983 type, from first principles). Homogeneous isotropic Gaussian turbulence of
kinetic energy `k` and dissipation `eps`: `sigma = sqrt(2k/3)` per component, integral time `T_L = C_L k/eps` (`C_L = 0.15` default,
case key `dispersion_cl`), eddy length `L_e = C_mu^(3/4) k^(3/2)/eps` (`C_mu = 0.09`). A particle sees `u + u'` with `u'_i = zeta_i sigma`
(`zeta_i` standard normal, SplitMix64 + Box-Muller) held for the interaction time `t_int = min(t_e, t_cross)`, `t_e = -T_L ln r`
(`r` uniform in (0, 1], exponential distribution with mean `T_L`), `t_cross = -tau ln(1 - L_e/(tau |u + u' - v|))` when the argument
of the log is positive (else infinite), `tau = tau_p/f(Re)` the effective relaxation time (Stokes: relative speed decays as
`exp(-t/tau)`, relative distance `tau |w| (1 - exp(-t/tau))`, set equal to `L_e`). The sub-step is truncated at the end of the
interaction time, then a new `u'` and `t_int` are drawn. A single serial generator (seed `dispersion_seed` offset from `particles.seed`),
particles processed in id order, so a run is reproducible bit for bit. `crossing = false` gives the pure-lifetime model.
Closed form (Tchen/Hinze-type, derived): with pure lifetime `t_e ~ Exp(T_L)` and independent Gaussian redraw, the fluid velocity
seen is a renewal process whose autocorrelation is `R(s) = sigma^2 exp(-s/T_L)` exactly (the probability that no redraw happened in
lag `s`). A Stokes particle `dv/dt = (u' - v)/tau` has the stationary variance
`<v^2> = (1/tau^2) int_0^inf int_0^inf exp(-(a+b)/tau) R(|a-b|) da db = sigma^2 T_L/(T_L + tau)` per component (the double integral equals `tau^2 T_L/(T_L+tau)`; Hinze 1975, Tchen 1947: the ratio of particle to fluid variance is `1/(1 + tau/T_L)`); it tends to `sigma^2` for `tau -> 0`.

D3 Injection. (a) Size distributions: constant, Rosin-Rammler (existing), and `uniform` in `[d_min, d_max]`. (b) Parcel weighting by
mass flow: when `mass_flow` [kg/s] is set the weight of a parcel is `n = (mass_flow/rate) / (rho_p pi d^3/6)`, so every parcel carries
the same mass `mass_flow/rate` (EQUAL-MASS parcels). Sampled parcel diameters then follow the mass distribution, and the mass fraction
of parcels below `d` is an unbiased estimate of the distribution's CDF (Rosin-Rammler mass CDF `1 - exp(-(d/d_bar)^n)`, truncated:
renormalised on `[d_min, d_max]`). The total mass injected by `N` parcels is `N mass_flow/rate`, round-off exact. Without `mass_flow` the
behaviour is unchanged (equal counts). (c) Cone: parcel direction uniform in solid angle within the hollow/solid cone about `axis`:
`cos(phi)` uniform in `[cos(theta), cos(theta_in)]`, azimuth uniform in `[0, 2 pi)`, speed `cone_speed`; any position kind (point or surface)
may emit the cone. Surface positions (existing) are uniform by area, offset `1e-9` cell lengths inward.
Extensions are stored in `InjectorExt` (set with `Tracker::set_injector_ext`) so existing `InjectorSpec` literals stay valid.

D4 Case tree (additive keys, unset sentinels `-1`, defaults inert): `particles.dispersion = none|drw`, `dispersion_k`,
`dispersion_epsilon` [homogeneous values, required when `drw`], `dispersion_cl` (0.15), `dispersion_crossing` (true),
`dispersion_seed` (1); per injection `distribution = uniform` with `d_min`, `d_max`, `mass_flow`, `cone_half_angle`, `cone_inner_half_angle`,
`cone_axis`, `cone_speed`. Located errors name the full path (for example `particles.injection.jet.cone_half_angle`). Two-way coupling has no
case key (library only).

D5 Stretch (breakup, wall film): not attempted in this wave unless time remains after the registered checks; named as NOT DELIVERED
in the result if skipped.

## Registered checks (numeric limits, fixed before any run)

All in `crates/rcfd-particles/tests/lagrangian_ext.rs` (library) unless stated; `N` is the sample count, seeds are fixed in the tests.

* L1 momentum balance. 40 parcels, Schiller-Naumann, RK4 and analytic, gravity on, non-uniform carrier velocity on a 4x4x4 box, no wall
  hit: `|sum m (v_end - v_0 - g_eff t) + sum_c S_c V_c t| <= 1e-12 sum |m (v_end - v_0 - g_eff t)|`. With reflecting walls and hits:
  `|sum m (v_end - v_0 - g_eff t) - W_impulse + sum S V t| <= 1e-12 * sum |m Delta v|` where `W_impulse` is the booked wall impulse.
* L2 energy balance (thermal on, same set-up): `|sum (Delta KE - W_g + m c_p Delta T) + sum_c S_E,c V_c t| <= 1e-12 * sum |terms|`
  (independent recomputation from the particle list: `W_g` from the displacement).
* L3 observation-only: fingerprint with coupling on equals coupling off (exact); settling terminal velocity with coupling on within
  `1e-6` relative of `d^2 (rho_p - rho_f) g/(18 mu)` (F-056 limit); Stokes relaxation of a particle in uniform flow within `1e-12` relative
  of `u (1 - exp(-t/tau))`.
* L4 single-cell coupled closed form. One cell of carrier mass `M`, one Stokes particle of mass `m`, `phi = m/M`; the relative
  velocity decays as `exp(-(1+phi) t/tau)` and `m v + M u` is conserved. With the explicit carrier update at `dt = tau/100`: at `t = tau`
  `|w/w0 - exp(-(1+phi))| <= 0.02 exp(-(1+phi))` for `phi = 1`; `|m v + M u - (m v0 + M u0)| <= 1e-13 |m v0|`; first order: errors at `dt = tau/100, tau/200,
  tau/400` have successive ratios in `[1.8, 2.2]`.
* L5 dilute limit: relative velocity difference two-way vs one-way at `t = tau`: `|v_two - v_one|/|v0| <= 0.5 phi` for
  `phi in {1e-2, 1e-4, 1e-6}` (closed form `phi (t/tau) e^(-t/tau) <= phi/e = 0.37 phi`), and the ratio of the differences at
  `phi = 1e-2` and `1e-4` lies in `[90, 110]`.
* L6 heat transfer closed form: quiescent fluid (`Nu = 2`), exponential lumped relaxation within `1e-12` relative; energy exchanged equals
  `m c_p (T_p,end - T_p,0)` to `1e-12`.
* L7 DRW constants: `sigma`, `T_L`, `L_e` formulas exact to `1e-14`; crossing time: after `t_cross` the relative displacement of a Stokes
  particle equals `L_e` within `1e-10` relative; infinite when `L_e >= tau |w|`.
* L8 DRW statistics, pure lifetime, Stokes particle, zero mean flow, `N = 40000` particles from `v = 0`, run `12 max(T_L, tau)`, seed
  `20260329`: relative error of `<v_i^2>` (average over three components) against `sigma^2 T_L/(T_L+tau)` `<= 4%` for `tau/T_L in {0.1, 1, 5}`
  (sample-variance standard error about 0.7%, so 4% is more than 5 sigma); `|<v_i>| <= 4 sigma_p/sqrt(N)` per component; mean eddy lifetime within
  `4%` of `T_L`; variance of `u'` samples within `2.5%` of `sigma^2`. Determinism: same seed gives identical bits, other seed differs.
* L9 DRW in the tracker (one big cell of side 1e4 m, no walls reached, `N = 4000`): `<v^2>` within `8%` of the closed form for
  `tau/T_L = 1`; with `dispersion = none` the fingerprint equals the previous behaviour (exact).
* L10 Rosin-Rammler: `N = 20000` parcels, equal mass, `d_bar = 100 um, n = 2.5` (and truncated `[30, 300] um`): Kolmogorov distance of the
  parcel diameters to the CDF (truncated: renormalised) `<= 1.95/sqrt(N) = 0.01379`; the mass fraction computed from the parcel weights and
  diameters `sum_{d_i<=d} n_i m_i/sum n_i m_i` at the five quantiles 0.1,...,0.9 within the same bound; uniform distribution likewise.
* L11 mass flow: `|sum_i n_i rho_p pi d_i^3/6 - N mass_flow/rate| <= 1e-12 N mass_flow/rate`.
* L12 cone: `N = 20000`; all angles to the axis in `[theta_in - 1e-12, theta + 1e-12]` (axis off the coordinate axes); `|v| = speed` within
  `1e-14` relative; Kolmogorov distance of `cos(phi)` to the uniform law on `[cos theta, cos theta_in]` and of the azimuth to uniform
  `<= 1.95/sqrt(N)`.
* L13 surface injection: all positions inside the patch rectangle, distance to the patch plane `<= 1e-8` cell lengths and on the inner side;
  fraction in the half of the patch with area fraction 0.5 within `4 sqrt(0.25/N)` of 0.5 (`N = 20000`).
* L14 located errors (API, `rcfd-api/tests/particles_ext_case.rs`): each invalid key (cone half-angle outside (0, pi], inner >= half, zero axis,
  speed <= 0, mass_flow <= 0, uniform with `d_min >= d_max` or unset, `dispersion = drw` without `dispersion_k`/`dispersion_epsilon`, `dispersion_cl <= 0`)
  gives a diagnostic whose path is the full key path; valid input gives none.
* L15 case-tree run: a frozen-flow case with `dispersion = drw` produces a spread of final positions (std of the transverse position `> 0`) and is
  reproducible bit for bit; with defaults (`dispersion = none`) the run output equals the previous behaviour (the existing particle tests pass
  unchanged). Baseline hashes unchanged. No Symmetry side walls in the flow tests.

## Not delivered (planned statement, confirmed or reduced in the result addendum)

Solver-loop feedback of the sources (the carrier equations of `Simulation::step` do not receive them), a particle-laden jet case, dispersion
from the turbulence model fields (constants only), cone injections with swirl, droplet evaporation, breakup, wall film, erosion,
particle reactions, tracker checkpointing.
