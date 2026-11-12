# ADR 0230: F-323 registration: the mass, heat and momentum of evaporating and breaking parcels in the exchange record

Status: **REGISTERED 2026-11-12**, written before any code of F-323 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; nothing above the addendum is changed after a run.

Parents: [ADR 0057](0057-wave29c-lagrangian-extensions.md) (F-128: the exchange record `Coupling`, decision D1),
[ADR 0224](0224-f245-registration-droplet-evaporation-and-breakup.md) (F-245: evaporation E1 to E8, TAB breakup B1 to B6; its
questions Q2 and Q3). Roadmap item F-323 (depends on F-245), made from ADR 0224 Q3.

Number: the lead gave 0230. Branch `f323-droplet-feedback` from main 0d250c75, worktree `E:/projects/cfd/w49i-work/f323`.

House rules (ADR 0130, ADR 0192, ADR 0224): limits are fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the number recorded; a defective set-up is corrected by an amendment written before
the corrected run and the old test stays as an ignored record; each registered run is made once; every new code path is opt-in
and a caller that does not use it gets the tracker of main bit for bit; no case-schema key, no hash table, no golden and no
existing test is edited; no new third-party crate; no commit trailers.

Clean-room: nothing new is taken from a source. The accounting below is derived here from the conservation of mass, momentum and
energy applied to the sub-step of ADR 0224 E6 and B4, whose models were read for ADR 0224 (Both, Mira and Lehmkuhl,
arXiv:2201.02818; O'Rourke and Amsden, LA-UR-87-2105). The statement that the TAB normal velocity "does not conserve momentum in
detail but ... on the average" is the report's, as quoted in ADR 0224. No vendor code, text or binary was used.

## What F-323 asks

The roadmap entry: "Vapour mass, heat and momentum from evaporating and breaking-up parcels are returned to the gas, conserved to
1e-10 relative; the evaporation and breakup models may be used with the particle exchange record."

Readings of this text (readings, not decisions of the maintainer):

- **"Returned to the gas"**: booked per cell in the exchange record of ADR 0057, which is the library's interface to a carrier
  (the caller feeds the record to its equations and gives the tracker the new carrier temperature and vapour fraction with the
  existing `thermal_mut` and `set_evaporation_vapour`). The check CL1 closes that loop with a carrier of the test's own. No flow
  solver of rcfd is wired (the note of F-323 puts the case-tree wiring with F-252).
- **"Conserved to 1e-10 relative"**: for each of mass, momentum and total energy, the sum of what the parcels hold and what the
  record says the carrier received equals what was injected plus the external terms the record also holds (gravity, walls, and
  the breakup residual of D5), at the end of every step, with a residual of at most 1e-10 of the largest term of the identity.
- **"May be used with the particle exchange record"**: `advance` accepts evaporation and breakup together with the record when
  the caller asked for the droplet terms (D1).

## What was read and computed before this text (disclosed)

- **Read:** ADR 0224 in full with its addendum; ADR 0057 (audit, D1, the checks L1 to L6); `crates/rcfd-particles/src/`
  `evaporation.rs`, `breakup.rs`, `coupling.rs`, `tracker.rs`, `lib.rs`, the step of `drag.rs`; the tests `evaporation_f245.rs`
  (set-ups, SP) and `lagrangian_ext.rs` (L1, L2); the uses of `Coupling` in `rcfd-api` (`particle_coupling.rs`,
  `particle_run.rs`: they read `impulse` and `energy_heat` only and build the record with `enable_coupling`; none builds a
  `Coupling` literal); the section of `docs/particles.md` on droplets.
- **Arithmetic, no rcfd code** (scratch Python, `E:/projects/cfd/w49i-work/f323-scratch/predict.py`, not committed): the closed
  forms of ADR 0224 E2 to E6 for data E, used for PR2 and PR5 below (the Jacobian at 300 K, the heat of one sub-step against
  `Q h`, the equilibrium of CL1 by bisection, and a 3000-step model of the CL1 loop with one droplet class).
- No rcfd code or test was built or run for F-323 before this text.

## What the code read shows (main 0d250c75)

1. `Tracker::advance` returns an error when evaporation or breakup is set and the record is on (ADR 0224 E8, B5); the tests RF1
   and RF2 of ADR 0224 assert that error after `enable_coupling`.
2. The record (`Coupling`): per cell `impulse` (`-(m (v1 - v0) - m g_eff h)`), `energy_mech` (`-(Delta KE - W_g)`),
   `energy_heat` (`-m c_p Delta T_p`), and `wall_impulse`. `book` is called once per sub-step, after the heat step, with the
   parcel mass `mass`; in the present order `mass` would be the mass after the evaporation of the sub-step.
3. `evaporate` (E6) changes `T_p`, then `d`, and books `n dm` of vapour in the start cell; at a completion the whole liquid mass
   of the sub-step's start. The motion of the sub-step (`step_particle`) used the diameter of the start.
4. `break_up` (B4) rescales the count by the mass rule (`n d^3` kept to round-off), adds the normal velocity and has no carrier
   counterpart. Wall reflections change the momentum (booked in `wall_impulse`) and, with a restitution below 1, the kinetic
   energy (booked nowhere).
5. The record does not hold the gravity impulse or work; the checks of ADR 0057 rebuild them from the constant parcel mass and the
   age, which is not possible when the mass changes.

## Design decisions

**D1. Opt-in (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; Q1).** A new
`Tracker::enable_droplet_exchange()` starts the record of ADR 0057 with the droplet terms; with it `advance` accepts evaporation
and breakup. `Tracker::enable_coupling()` is unchanged: with it `advance` still refuses evaporation and breakup, with the message
of main, because a caller that reads only the fields of ADR 0057 (the two-way paths of `rcfd-api` do) would drop the vapour terms
and lose conservation silently. The last of the two calls wins; both start an empty record. The surface reaction stays refused
with either (ADR 0208). The record remains an observation: the parcels are the same bit for bit with or without it.

**D2. The splitting of a sub-step (derived).** Over a sub-step of length `h` (or `h_hit`) that starts in cell `c` with the
parcel mass `m0 = n rho_l pi d0^3/6`, velocity `v0`, temperature `T0`: (i) the motion from `(x0, v0)` to `(x1, v1)` with the
mass `m0` (the tracker integrates the motion with the diameter of the start); (ii) the temperature step `T0 -> T1` at the mass
`m0`; (iii) the mass `dm = n (pi rho_l/6)(d0^3 - d1^3)` (the number ADR 0224 books; the whole `m0` at a completion; negative for
condensation) leaves as vapour with the velocity `v1` and the temperature `T1`. The parcel then holds `m1 = m0 - dm`, `v1`, `T1`.

**D3. What the carrier receives in cell `c` (derived from D2).**

- drag, as ADR 0057 with the mass of the start: `impulse -= m0 (v1 - v0) - m0 g_eff h`,
  `energy_mech -= (1/2) m0 (v1^2 - v0^2) - m0 g_eff . (x1 - x0)`;
- heat: `energy_heat -= Q_h`, `Q_h = m0 c_l (T1 - T0) + dm L_v`: the heat the gas supplied is the sensible heat the liquid
  gained plus the latent heat of the mass that evaporated. It is defined by the energy balance of the sub-step, so the balance
  closes to round-off for any `h`, and it is a first-order approximation of the model's `integral Q dt` (E4; check DX3). Without
  evaporation `dm = 0` and it is the `m c_p Delta T_p` of ADR 0057;
- vapour: `vapour_mass += dm`, `vapour_impulse += dm v1`, `vapour_kinetic += (1/2) dm v1^2`,
  `vapour_enthalpy += dm (c_l T1 + L_v)`.

Then, per sub-step, exactly (up to round-off): `m1 v1 = m0 v0 + m0 g_eff h - (carrier impulse) - dm v1`,
`m1 = m0 - dm`, and with the parcel energy `E = (1/2) m v^2 + m c_l T`:
`E1 = E0 + m0 g_eff . (x1 - x0) - (carrier mech) + Q_h - dm ((1/2) v1^2 + c_l T1 + L_v)`.

**D4. The enthalpy datum (registered here; Q3).** The liquid enthalpy is `c_l T` (zero for the liquid at 0 K) and the vapour
leaves with `c_l T1 + L_v`, which is the vapour enthalpy the model of ADR 0224 E4 implies (`m c_l dT/dt = Q - mdot L_v` is the
balance of `m c_l T` with an outflow `mdot (c_l T + L_v)`). A caller with another datum `T_ref` for the liquid subtracts
`c_l T_ref` times `vapour_mass`.

**D5. Breakup (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; Q2).** The normal velocity
of B4 changes the parcel's momentum by `m dv` and its kinetic energy by `(1/2) m (abs(v + dv)^2 - v^2)`. In the source the
product drops leave in all directions around the relative velocity (zero net momentum), and their kinetic energy comes from the
oscillation energy of the parent (its eqs. 18 to 22), which the tracker does not hold; one parcel takes one direction, so its
momentum is conserved "on the average". Neither is charged to the gas: they are booked as `breakup_impulse` and
`breakup_energy`, the momentum and kinetic energy given to the parcels by breakup without a carrier counterpart (as
`wall_impulse` for walls), so the identity closes and the residual can be inspected. The rescaling of the count conserves the
mass to round-off and changes nothing else.

**D6. External terms booked so that the identities can be closed.** `gravity_impulse` (`sum m0 g_eff h`) and `gravity_work`
(`sum m0 g_eff . (x1 - x0)`), `wall_energy` (`sum (1/2) m (v_after^2 - v_hit^2)` of reflections, beside the existing
`wall_impulse`). They are booked in both modes; the four fields of ADR 0057 are computed as on main.

**D7. Sources.** `Coupling::droplet_sources(vol, dt)` gives per cell the vapour mass source [kg/(m^3 s)], the momentum source
(drag plus vapour) [N/m^3] and the total-energy source (mechanical, heat, vapour kinetic energy and vapour enthalpy) [W/m^3].
`Coupling::sources` is unchanged.

**D8. Code.** `crates/rcfd-particles/src/coupling.rs` (the new fields, `reset`, `droplet_sources`), `tracker.rs`
(`enable_droplet_exchange`, the refusal, the booking in the two branches of the sub-step, the wall and breakup terms), `lib.rs`
(export). `Evaporation`, `Breakup`, `fingerprint`, `Particle`, `Flow`, `Options`, `Thermal` unchanged. `rcfd-api` is not edited.

## Registered checks

All in `crates/rcfd-particles/tests/droplet_exchange_f323.rs`, run in release
(`cargo test --release -p rcfd-particles --test droplet_exchange_f323`). "Relative" is `abs(a - b)/abs(b)`. Sums over cells are
written `S(field)`. Each reference is computed by the test's own code (its own Clausius-Clapeyron, Spalding numbers, bisection),
not by library functions. The identities, at the end of a step, over all parcels of every state (a parcel that left keeps the
diameter, velocity and temperature of its last sub-step; an evaporated one has `d = 0`):

- **IM (mass):** `M_liq + S(vapour_mass) = M_inj`.
- **IP (momentum):** `sum m v + S(impulse) + S(vapour_impulse) - gravity_impulse - wall_impulse - breakup_impulse = P_inj`.
- **IE (energy):** `sum ((1/2) m v^2 + m c_l T_p) + S(energy_mech) + S(energy_heat) + S(vapour_kinetic) + S(vapour_enthalpy)
  - gravity_work - wall_energy - breakup_energy = E_inj`.

`M_inj`, `P_inj`, `E_inj` are the sums over the injected parcels of `m0`, `m0 v_inj`, `(1/2) m0 v_inj^2 + m0 c_l T_inj` with the
injector's values. The residual of an identity is the norm of (left side minus right side) divided by the largest norm among its
terms (each sum above is one term).

Data E, the single-droplet set-up and the wet bulb `T_wb` are those of ADR 0224 A (box of one cell of 0.01 m, Stokes drag,
analytic scheme, cell-centre interpolation, no gravity, gas at rest at 800 K, `Y_s = 0`, one parcel of `n = 1000` droplets of
`d0 = 5e-5` m released at rest at the end of the first step), with `enable_droplet_exchange()`. `M0 = n rho_l pi d0^3/6`.

### A. Single droplets

| Id | Check | Limit |
| --- | --- | --- |
| DX1 (mass and heat at the wet bulb) | injected at `T_wb`, steps of `1e-4` s until `Evaporated` (at most 200). After every step with a loss: (a) `vapour_mass` of the cell against the evaporation source of the cell; (b) `S(vapour_mass)` against `n (pi rho_l/6)(d0^3 - d^3)`; (c) `-S(energy_heat)` against `L_v S(vapour_mass)`; (d) `S(vapour_enthalpy)` against `(c_l T_wb + L_v) S(vapour_mass)`; (e) IE; (f) `impulse`, `vapour_impulse`, `energy_mech`, `vapour_kinetic`, `gravity_impulse`, `gravity_work`, `wall_impulse`, `wall_energy`, `breakup_impulse`, `breakup_energy`. At completion: `S(vapour_mass)` against `M0` | (a) bit for bit; (b) 1e-10; (c) 1e-10; (d) 1e-10; (e) 1e-10; (f) zero, bit for bit; 1e-10 |
| DX2 (heat-up) | injected at 300 K, the same run. After every step: IM, IE. At completion: `-S(energy_heat) - L_v M0` (the heat that warmed the liquid) | 1e-10, 1e-10; in `(0, M0 c_l (T_wb - 300)]` |
| DX3 (the heat against the model's `Q`, first order) | injected at 300 K, released at `h` (`rate = 1/h`, `t_end = h`), then one step of `h`: `e(h) = -S(energy_heat)/(n pi d0 lambda_m Nu0 g(beta) (800 - 300) h) - 1` with the test's `g(beta)` at 300 K, `Nu0 = 2`. For `h = 1e-6` s: `abs(e)`; `e(2e-6)/e(1e-6)` | at most 1e-3; in `[1.9, 2.1]` |
| DX4 (momentum and kinetic energy of the vapour, many cells) | mesh `box_mesh([8, 1, 1], [0.08, 0.01, 0.01])`, drag law `None`, no gravity, gas at rest at 800 K, `Y_s = 0`; the parcel injected at `(0.005, 0.005, 0.005)` with the velocity `V = (20, 0, 0)` at the test's wet bulb for `Re0 = 16.667` (ADR 0224 EV4), released at `1e-5` s, 300 steps of `1e-5` s. After every step: the velocity; `S(vapour_impulse)` against `(M0 - M) V`; `S(vapour_kinetic)` against `200 (M0 - M)`; `impulse` and `energy_mech` of every cell; `vapour_mass` against the evaporation source in every cell; IM, IP, IE. At the end: cells with `vapour_mass > 0`; the parcel active | `V` bit for bit; 1e-10; 1e-10; zero bit for bit; bit for bit; 1e-10 each; at least 5; as stated |
| SR1 (sources) | at the end of DX4, `droplet_sources(vol, t)` with `t = 3e-3` s: in every cell `mass V_c t` against `vapour_mass`, `momentum V_c t` against `impulse + vapour_impulse`, `energy V_c t` against the sum of the four energy fields (cells where the reference is not zero); zero where it is zero | 1e-12 relative; as stated |
| DX5 (walls and gravity) | the single-cell box, drag law `None`, gravity `(0, 0, -9.81)`, default wall `Reflect(0.8)`; injected at the centre with `(3, 0, -20)` m/s at 300 K, released at `1e-5` s, 100 steps of `1e-5` s. After every step: IM, IP, IE. At the end: `wall_impulse_z`; `wall_energy`; `abs(gravity_impulse_z)` | 1e-10 each; `> 0`; `< 0`; `> 0` |
| BX1 (breakup alone) | the BU4 set-up of ADR 0224 B at `We = 1000` (`mu_l = 0`, `C_v = 1`, drag law `None`, no gravity, no heat transfer, one parcel at rest in a gas at `U`), 60 steps of `1e-5` s. One breakup; `breakup_impulse` against `m v` of the parcel after the run; `abs(breakup_impulse)` against `M0 C_b r0 yd_b`, `yd_b = W omega0 sqrt(2/W - 1/W^2)`; `breakup_energy` against `(1/2) m v^2`; `impulse`, `energy_mech`, the vapour fields | one; 1e-12; 1e-10; 1e-12; zero bit for bit |

### B. The spray and the closed loop

| Id | Check | Limit |
| --- | --- | --- |
| DX6 (acceptance: mass, momentum, heat of a spray with evaporation and breakup) | scenario SP of ADR 0224 C (160 parcels, 32 cells, Schiller-Naumann drag, gravity, walls, evaporation, TAB breakup with `C_v = 1`), with `enable_droplet_exchange()`, 300 steps of `1e-4` s. After every step: IM, IP, IE. At the end: breakup events; `abs(breakup_impulse)`; parcels `Evaporated`; parcels `Escaped`; the counts per state, the terms of the three identities and `abs(breakup_impulse)/abs(S(impulse))` recorded | 1e-10 each; at least 80; `> 0`; at least 1; at least 1 |
| DX7 (observation only) | the fingerprint of DX6's tracker at the end against that of the same run without any record | bit for bit |
| CL1 (closed loop: the vapour and the heat reach a carrier and stop the evaporation) | the single-cell box (volume `1e-6` m^3), Stokes drag, all at rest; one parcel of `n = 20000` droplets of `5e-5` m at 300 K released at `1e-4` s; the test's carrier: `M_air = 5e-7` kg, `M_v = 0`, `T_g = 500` K, enthalpy `H_g = M_air c_p,m T_g + M_v (c_l T_g + L_v)`; 2000 steps of `1e-4` s; after each step `M_v += S(vapour_mass)`, `H_g += S(energy_heat) + S(vapour_enthalpy)`, the record reset, `T_g = (H_g - M_v L_v)/(M_air c_p,m + M_v c_l)` given to `thermal_mut`, `Y_s = M_v/(M_air + M_v)` given to `set_evaporation_vapour` (`Flow::rho`, `mu` held). After every step: `M_liq + M_v` against `M0`; `H_g + m c_l T_p` against its initial value. At the end, with `T_eq` the test's bisection root of `(M_air c_p,m + M0 c_l) T + M_air Y_i(T)/(1 - Y_i(T)) L_v = H(0)`: `abs(T_g - T_eq)`; `abs(T_p - T_eq)`; `abs(Y_s - Y_i(T_eq))`; the parcel active with `d < d0` | 1e-10; 1e-10; 1e-6 K; 1e-6 K; 1e-8; as stated |

### C. Refusals, inert paths, defaults

| Id | Check | Limit |
| --- | --- | --- |
| RF3 | `advance` is `Err` with: evaporation and `enable_coupling()`; breakup and `enable_coupling()`; evaporation with `enable_droplet_exchange()` followed by `enable_coupling()`. `set_reaction` is `Err` after `enable_droplet_exchange()`. `advance` is `Ok` with `enable_droplet_exchange()` and: evaporation; breakup; both; neither | as stated |
| IN1 (inert) | scenario SP without evaporation and breakup (heat transfer on, the injection temperatures kept), 300 steps, once with `enable_coupling()` and once with `enable_droplet_exchange()`: `impulse`, `energy_mech`, `energy_heat`, `wall_impulse`, `gravity_impulse`, `gravity_work`, `wall_energy` and the fingerprints equal; in both the vapour and breakup fields | bit for bit; zero bit for bit |
| DF1 (defaults) | the existing suites, unedited: every test binary of `rcfd-particles` (among them `lagrangian_ext` L1 to L6, `evaporation_f245`, `breakup_f245` with RF1 and RF2); `rcfd-api` `particles_case`, `particles_ext_case`, `particle_run`, `particles_two_way`, H1 of `case_hashes_w48u`, `baseline_hashes` | pass |

### Negative controls

| Id | Check | Limit |
| --- | --- | --- |
| NC1 | DX1 at completion: `-S(energy_heat)` against `(c_l T_wb + L_v) M0` (the vapour enthalpy in place of the latent heat) | at least 50 percent off |
| NC2 | DX6 at the end: the residual of IP with the term `S(vapour_impulse)` left out | at least 1e-3 |
| NC3 | CL1: `abs(T_eq - H(0)/(M_air c_p,m + M0 c_l))` (the equilibrium without the latent heat) | at least 10 K |

### Mutations (scratch, not committed)

Each is applied alone to the final tree by an exact text replacement, the named test is run once in release and must fail; the
file is restored and checked byte for byte against a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MX1 | the vapour momentum booked with `v0` in place of `v1` | DX6 |
| MX2 | `Q_h` without the latent term `dm L_v` | DX1 |
| MX3 | the drag impulse booked with the mass after the evaporation | DX6 |
| MX4 | the vapour enthalpy with `T0` in place of `T1` | DX2 |
| MX5 | the breakup impulse not booked | BX1 |
| MX6 | the wall energy not booked | DX5 |
| MX7 | `vapour_mass` booked as `0.999 dm` | DX1 |

### Predictions (mine; the Python model, not rcfd code)

| Id | Prediction |
| --- | --- |
| PR1 | DX1, DX2, DX4, DX5, BX1: every relative number below 1e-12; the bit-for-bit items hold |
| PR2 | DX3: at 300 K `J = -616.5 /s`, `Q = 7.0669e-3` W and `mdot L_v = 4.174e-4` W per droplet; `e(1e-6) = -2.903e-4`, `e(2e-6) = -5.804e-4`, ratio 2.000 |
| PR3 | DX6: the three residuals below 1e-11; 80 breakup events; the vapour carries of the order of 0.1 of the momentum scale (NC2 near 0.1 to 0.3) |
| PR4 | NC1: 71 percent (`L_v/(c_l T_wb + L_v) = 0.29`) |
| PR5 | CL1: `M0 = 9.162978573e-7` kg, `H(0) = 0.8797565858` J, `T_eq = 310.5231755868` K, `Y_i(T_eq) = 0.3415612235`, `M_v = 2.5937e-7` kg (72 percent of the liquid left); the model loop is within 2.1e-6 K after 900 steps, 7.4e-9 K after 1200 and at 3e-12 K from 1800 on; NC3: 32.3 K |
| PR6 | the new tests take a few seconds in release |

## Decision rule

- **done**: every check of A, B and C meets its limit, the three negative controls hold and the seven mutations make their
  checks fail.
- **partial**: DX1, DX2, DX4, DX5, DX6, DX7, BX1, RF3, IN1 and DF1 are met (the conservation and the use with the record), and a
  miss is confined to DX3, SR1 or CL1.
- **todo**: anything else. DF1, IN1 and DX7 are preconditions of landing: a miss means the change is not to be merged.

A negative control that does not hold, or a mutation that does not make its check fail, is a defective set-up: corrected by an
amendment before any re-run and never counted as a pass.

## Limits (not covered)

No rcfd flow solver receives the record: the transient and steady two-way paths of `rcfd-api` read only `impulse` and
`energy_heat` and keep using `enable_coupling`, and no case key is added (with F-252, as the roadmap note says). The carrier of
CL1 is the test's own, one cell, with constant `Flow::rho` and `mu`. The breakup's normal momentum and kinetic energy are not
given to the gas (D5). The latent heat is constant, so the vapour enthalpy is consistent with the liquid's only for the datum of
D4 (Q3). The whole exchange of a sub-step goes to its start cell (ADR 0057). The film, erosion, rough walls and the random walk
are not exercised together with the droplet terms; a parcel absorbed by the film keeps its energy out of the record. Parcels
`Stuck` after too many reflections stop being advanced, as on main. Condensation (`dm < 0`) is covered only as far as CL1 passes
through it.

## Open questions

- **Q1. Opt-in or lifting the refusal.** (a) `enable_droplet_exchange()` beside an unchanged `enable_coupling()` (taken, D1);
  (b) `enable_coupling()` itself accepts the droplet models and the two assertions of RF1 and RF2 of ADR 0224 are retired to
  ignored records. Recommendation: (a) until the `rcfd-api` two-way paths consume the vapour terms (F-252), then (b).
- **Q2. The breakup's normal momentum.** (a) booked as a residual without a carrier counterpart (taken, D5; the source's "on
  the average"); (b) charged to the gas of the cell (exact detail conservation of gas plus liquid, at the price of a random
  lateral force on the gas that the model does not contain); (c) two parcels with opposite normal velocities per breakup
  (conserves in detail, doubles the parcels at each breakup and changes the results of ADR 0224). Recommendation: (a).
- **Q3. The enthalpy datum and the constant latent heat.** With `c_l` different from `c_p,vap` a constant `L_v` is consistent at
  one temperature only. (a) keep D4 and let the caller shift the datum; (b) a temperature-dependent `L_v` (Kirchhoff) inside the
  evaporation model, which changes ADR 0224's results. Recommendation: (a) now; (b) is a change of the F-245 model and belongs
  with the property work of F-252. Not taken here because it would change a registered model.
- **Q4. Velocity and temperature of the leaving vapour.** Registered: the end of the sub-step (`v1`, `T1`). Any choice within the
  sub-step is first-order equivalent and conserves equally. Recommendation: keep.
