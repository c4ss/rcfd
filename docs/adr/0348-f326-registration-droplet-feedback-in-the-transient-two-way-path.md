# ADR 0348: F-326 registration: droplet feedback in the transient two-way path (the vapour species, the momentum and the heat of evaporating and breaking parcels given to the carrier of a case)

Status: **REGISTERED 2026-11-12**, written before any code of F-326 and before any run of its checks. Results go in an
addendum; nothing above the addendum is changed after a run. Not yet accepted by the maintainer.

Parents: [ADR 0230](0230-f323-registration-droplet-feedback-and-exchange-record.md) (F-323: `enable_droplet_exchange`, the
droplet terms of the record, `Coupling::droplet_sources`, its question 5 from which F-326 comes),
[ADR 0224](0224-f245-registration-droplet-evaporation-and-breakup.md) (F-245: the evaporation and TAB breakup models, data E,
its question 2 on the case tables), [ADR 0059](0059-wave30a-wiring-particles-mixing-plane.md) (F-128: the transient two-way path
`ParticleRun::pre_step`, the body acceleration and the heat source of the step). Roadmap item F-326 (depends on F-323).

Number: the lead gave 0348. Branch `f326-droplet-feedback-transient`, worktree `w49i-work/f326`, from main ff21a3b7.

House rules (the campaign rules of 2026-11-12): limits are fixed here and are not changed after a run; a missed limit becomes
an ignored `*_known_miss` test with the limit intact and the measured number recorded, diagnosed, not tuned away; a defective
set-up gets a corrected test by an amendment committed before the corrected run, the old test staying as an ignored record;
each registered run is made once; defaults stay bit-identical; `baseline_hashes.rs`, the case and mesh hash tables and the
goldens are not touched; no existing test is edited; no new third-party crate, no unsafe code; unsigned commits without
trailers; no push.

Clean-room: nothing new is taken from a source. The split of the energy source below is derived here from the energy balance
of a constant-density, constant-`c_p` carrier receiving a dilute vapour; the droplet models are those of ADR 0224 (Both, Mira
and Lehmkuhl, arXiv:2201.02818; O'Rourke and Amsden, LA-UR-87-2105), read for ADR 0224 and not re-opened. No vendor code,
text or binary was used.

## What F-326 asks

The roadmap entry: "With the droplet exchange on, the transient two-way particle path gives the carrier the vapour mass (a
species source), the momentum and the energy sources of Coupling::droplet_sources, and the tracker reads the carrier
temperature and vapour fraction of the step; gas plus liquid mass, momentum and energy of a closed case are conserved to 1e-10
relative per step; optional particles.evaporation and particles.breakup tables (hash-omitted when absent); a case without them
is bit-identical."

Clauses (a1 to a6), with the readings taken (readings, not decisions of the maintainer):

- **a1** the carrier receives the vapour mass as a source of a transported species, the momentum source and the energy source
  of `droplet_sources`. Reading: the mass and momentum parts of `droplet_sources(vol, dt)` enter the species and momentum
  equations as they are (divided by the constant `physics.rho`); the energy part is split (D4) into the part the temperature
  equation of a constant-property carrier can hold and two parts it cannot (the mechanical energy, as on main for the heat
  transfer of ADR 0059, and the enthalpy of the arriving vapour at the carrier temperature), each booked in a ledger so that
  the whole energy source is accounted for.
- **a2** the tracker reads the carrier temperature and vapour fraction of the step: the old time level of `T` and of the
  vapour species, before the flow step, as ADR 0059 D2 does for the velocity.
- **a3** gas plus liquid mass, momentum and energy of a closed case are conserved to 1e-10 relative per step: the identities
  IM, IP, IE of ADR 0230 with the carrier side made of what the carrier's equations received (D6), at the end of every step,
  relative to the largest term (ADR 0230's definition).
- **a4** optional tables `particles.evaporation` and `particles.breakup`.
- **a5** hash-omitted when absent.
- **a6** a case without them is bit-identical.

## What was read and done before this text (disclosed)

- **Read:** ADR 0230 in full with its addendum; ADR 0266 reads 5 and 6 (how an optional table stays out of the case data, the
  texts and the hashes); `crates/rcfd-particles/src/coupling.rs` in full; `tracker.rs` lines 365 to 475 (the setters, the two
  record modes), the booking helpers around lines 940 to 955 and 1140 to 1240 (`c_l` is `Thermal::cp_p`, `L_v` the spec's);
  `evaporation.rs` lines 1 to 200; `breakup.rs` lines 1 to 80; `crates/rcfd-api/src/particle_run.rs` in full (the transient
  two-way path); `particle_coupling.rs` in full (the steady path, a fresh tracker with `enable_coupling`); `particles.rs` the
  schema, `check`, `check_coupling`, `build_tracker`, `flow_data_from`; `scalars.rs` lines 1 to 130 and 370 to 820 (the
  species, `refresh`, `advance`, the transport's tolerance); `crates/rcfd-fv/src/scalar.rs` lines 1 to 350 (`sweep_ext`,
  `ScalarTransport::new` with `tol_rel = 1e-13`); `energy.rs` lines 1 to 150; the convection operator's assembly (conservative:
  every internal face's coefficient appears with opposite signs in its two rows, the deferred correction too);
  `crates/rcfd-api/src/sim.rs` lines 2030 to 2090 (the transient energy solver: `tol_rel = 1e-10`, `max_iter = 2000`), 2505 to
  2540 (the tracker is built after the scalars), 3999 to 4373 (the transient step: `pre_step` before the energy solve and the
  flow step, the scalars after the flow step); the tests `particles_two_way.rs` (PL6 to PL8, E1 to E3, the case texts),
  `dispersion_f255.rs` (a closed `box_3d`), `scalars.rs` (species syntax), `strong_coupling_case_f350.rs` T0.
- **Found by reading:** `ParticleRun::report` sizes its mass array by five states and indexes it by `State as usize`; a parcel
  `Evaporated` (index 5) would panic there. Main cannot reach it (evaporation is not wired). Fixed in D7.
- **Arithmetic** (by hand, no script, no rcfd code), the estimates of the checks below:
  1. Case CB (below). Cell volume `1e-6` m^3, gas mass per cell `5e-7` kg, total `8e-6` kg; heat capacity `8.8e-3` J/K.
     Drop `d0 = 2e-4` m: `m = 700 (pi/6) 8e-12 = 2.93e-9` kg, parcel of 5: `1.47e-8` kg; 10 parcels: `1.47e-7` kg.
     Liquid energy at injection `1.47e-7 (2200 x 350 + 3600/2) = 0.113` J.
  2. Breakup: `We = rho_g U^2 r/sigma = 0.5 x 3600 x 1e-4/0.01 = 18 > 6`; `omega0 = sqrt(8 sigma/(rho_l r^3)) = 1.07e4`
     1/s; from rest `1 - cos(omega0 t) = 12/We` gives `t = 1.15e-4` s; the drop loses about 0.9 m/s by then (`Re = 200`,
     `C_D = 0.81`, deceleration `7.8e3` m/s^2). Each parcel is expected to break once in the first 0.2 ms after its release.
     Children of radius `r0/(4/3 + We/6) = 2.3e-5` m.
  3. Evaporation of the children at the wet bulb of data E (`K = 3.42e-7` m^2/s): lifetime `(4.6e-5)^2/K = 6.2` ms; over the
     run most of their mass evaporates. If no parcel broke, the parents (`d^2/K = 0.12` s) lose about 6 percent of their mass
     in 5 ms. Either way the vapour enthalpy `dm (c_l T + L_v)` is above `9e-9 x 1.1e6 = 1e-2` J, `0.09` of the liquid
     energy: the scale of NC1 and NC2.
  4. Cooling of the gas by the latent heat: at most `1.47e-7 x 3.2e5/8.8e-3 = 5` K on average; the vapour fraction at most
     `1.47e-7/8e-6 = 0.018` on average (higher in the cells the parcels cross).
  5. The solver tolerances against the per-step limits: the energy system is nearly diagonal (`V/dt = 0.1` m^3/s against a
     diffusion coefficient `alpha A/d = 9.1e-7` and a convective flux of at most `u A = 1e-3`), so a few iterations reach
     round-off; the bound from `tol_rel = 1e-10` alone is about `sqrt(16) x 1e-10 = 4e-10` of the step's source. The species
     transport has `tol_rel = 1e-13`. The round-off of a per-cell difference of `T` (800 K) is `1e-13` K against a per-step
     change of order `1e-2` K.

## Design

**D1. The tables (a4, a5).** Two optional tables (`Schema::optional`, `hash_omit_default`), absent from every case that does
not write them, as the film, erosion, rough-wall and dispersion tables of `particles` (ADR 0266 read 6: an optional table stays
out of the case data, both texts and the settings hash; no `DEFAULT_KEYS` entry):

| Key | Default | Meaning |
| --- | --- | --- |
| `evaporation.enabled` | `false` | evaporation of the parcels (ADR 0224 E1 to E8) with its feedback to the carrier |
| `evaporation.species` | `""` | the transported species (`species.<name>`, not the balance species) that receives the vapour |
| `evaporation.latent_heat` | `-1` | `L_v` [J/kg], must be set (> 0) |
| `evaporation.boiling_temperature` | `-1` | `T_b` [K] at `boiling_pressure`, must be set (> 0) |
| `evaporation.boiling_pressure` | `101325` | [Pa] |
| `evaporation.pressure` | `101325` | carrier pressure [Pa] |
| `evaporation.vapour_molar_mass` | `-1` | `W_f` [kg/mol], must be set (> 0) |
| `evaporation.gas_molar_mass` | `-1` | `W_b` [kg/mol], must be set (> 0) |
| `evaporation.diffusivity` | `-1` | `D_m` [m^2/s], must be set (> 0) |
| `evaporation.vapour_cp` | `-1` | `c_p,vap` [J/(kg K)], must be set (> 0) |
| `evaporation.min_diameter` | `1e-7` | [m] below which a droplet counts as evaporated |
| `evaporation.max_step_fraction` | `0.1` | the largest sub-step as a fraction of `tau_T`, in `(0, 1]` |
| `breakup.enabled` | `false` | TAB breakup (ADR 0224 B1 to B6) |
| `breakup.surface_tension` | `-1` | `sigma` [N/m], must be set (> 0) |
| `breakup.liquid_viscosity` | `-1` | `mu_l` [Pa s], must be set (>= 0) |
| `breakup.c_f`, `c_k`, `c_d`, `c_b`, `k`, `c_v` | `1/3`, `8`, `5`, `1/2`, `10/3`, `1` | the TAB constants of `BreakupSpec::tab` |
| `breakup.seed` | `1` | seed of the normal-velocity directions |

The liquid density is `particles.density`, its specific heat `particles.particle_cp` (`c_l`), the mean-state gas density,
viscosity, conductivity and specific heat are `physics.rho`, `rho nu`, `physics.conductivity`, `physics.cp`.

**D2. Where the tables are accepted (static checks, located).** With `particles.enabled`:

| Id | Condition | Path |
| --- | --- | --- |
| S1 | either table enabled without `particles.two_way = true` | its `enabled` |
| S2 | either table enabled with a steady algorithm (the steady path keeps `enable_coupling`, Limits) | its `enabled` |
| S3 | evaporation without `particles.heat_transfer = true` | `particles.evaporation.enabled` |
| S4 | `evaporation.species` empty, not a `species.<name>`, or the balance species | `particles.evaporation.species` |
| S5 | a required number unset or not positive; `max_step_fraction` outside `(0, 1]`; `min_diameter` not positive | the key |
| S6 | `surface_tension` unset or not positive; `liquid_viscosity` unset or negative; a constant refused by `BreakupSpec::validate` | the key; `particles.breakup` |
| S7 | either table enabled without `particles.enabled` | its `enabled` |

S1 and S3 together make evaporation need the energy equation (ADR 0059's heat-transfer checks). A disabled table is inert.

**D3. The tracker (a2).** `build_tracker` sets evaporation (spec from the table, the seen vapour fraction from the species'
initial cells clamped at 0) and breakup when their tables are enabled. `ParticleRun::new` calls `enable_droplet_exchange()`
in place of `enable_coupling()` when either is enabled ("droplet mode"); otherwise the lines of main. Before each flow step
the carrier's old-level `T` is given to `thermal_mut` (as on main) and, with evaporation, the old-level vapour species
`Y^n`, clamped below at 0, to `set_evaporation_vapour` (a value at or above 1 stops the run with a located message). A new
read-only `Tracker::thermal()` lets the check read what the tracker holds (rcfd-particles, additive).

**D4. What the carrier receives over a flow step of length `dt` (a1; derived).** With the record of the step and
`s = droplet_sources(vol, dt)` (normalised by the flow step, so that the carrier receives over the step exactly what the
record holds, also when the last tracking interval is shorter than `dt`):

- species: the source `S_Y = s.mass/rho` [1/s] of the named species (the constant-density species equation
  `d Y/dt + div(phi Y) - div(D grad Y) = S_Y`, so `rho sum V Delta Y = sum vapour_mass`);
- momentum: the body acceleration `a = s.momentum/rho` (drag plus vapour momentum), through the path of ADR 0059;
- temperature: the source `q = Q_T/(rho c_p V dt)` [K/s] with, per cell,
  `Q_T = energy_heat + vapour_enthalpy - vapour_mass (c_l T^n + L_v)`.

  Derivation. A carrier of fixed mass and constant `c_p` holds `rho c_p V T` in a cell; the vapour arriving there is a dilute
  species whose enthalpy at the carrier temperature, on the datum of ADR 0230 D4, is `h_v(T) = c_l T + L_v`. The record gives
  the cell the total energy `E = energy_mech + energy_heat + vapour_kinetic + vapour_enthalpy`. Of it, `energy_mech +
  vapour_kinetic` is mechanical energy (the momentum source acts on the gas kinetic energy, and the dissipation of the
  exchange is not fed into the temperature, as ADR 0059 for the heat transfer on main), and `vapour_mass h_v(T^n)` is the
  enthalpy the arriving vapour holds once it is at the carrier temperature. The rest is the sensible heat of the carrier:
  `Q_T = energy_heat + vapour_enthalpy - vapour_mass h_v(T^n) = -m0 c_l Delta T_p - dm L_v + dm c_l (T_1 - T^n)`: the gas
  pays the heating of the liquid, the latent heat, and brings the vapour from the droplet temperature to its own. With
  `dm = 0` it is `energy_heat`, the source of main (normalised by `dt`). The vapour's heat capacity in the carrier and the
  mass source in the continuity equation are left out (dilute vapour, constant density; Limits).
- ledgers: `K += sum (energy_mech + vapour_kinetic)`, `H_v += sum vapour_mass h_v(T^n)`, so that
  `Q_T + K-part + H_v-part = E V dt` per cell (check SR).

Without evaporation (breakup alone) the vapour terms are zero; without heat transfer there is no temperature source.

**D5. Code.** `particles.rs` (schema, checks, `build_tracker`), `particle_run.rs` (droplet mode in `new` and `pre_step`, the
ledger `DropletLedger` in `TransientCoupling`, the vapour source of the step, the report), `scalars.rs` (a per-step particle
source of a transported species, added in `refresh` only when set), `sim.rs` (three insertions in the transient step: the
vapour fraction given before `pre_step`, the species source given before `sc.advance`; lines reported in the addendum),
`rcfd-particles/src/tracker.rs` (`thermal()` getter). `docs/case-schema.md` and `docs/particles.md` gain text (no TOML
block).

**D6. The ledger** (`TransientCoupling::droplets: Option<DropletLedger>`, droplet mode only), cumulative from the start:
the record sums (vapour mass, momentum, heat, vapour enthalpy, the total energy `E V dt`), what each carrier equation received
(`rho V S_Y dt` summed, the existing `delivered` momentum `rho V a dt` and `delivered_heat` `rho c_p V q dt`), `K`, `H_v`, and
the external terms of the record (gravity impulse and work, wall impulse and energy, breakup impulse and energy), plus the
per-step sums of `abs` per cell of the vapour mass and of `Q_T` (the scales of C1b and C3b).

**D7. Report.** `particle_report.json` gains, only in droplet mode, `counts.evaporated`, `mass_by_state.evaporated` and a
`droplets` object (vapour given, evaporated, completed, breakup events); the mass array is sized by six states, which changes
no existing report (a parcel cannot be `Evaporated` without the table).

## The closed case CB

`box_3d`, `nx, ny, nz = 4, 2, 2`, `lx, ly, lz = 0.04, 0.02, 0.02` (16 cells of `1e-6` m^3); every patch a no-slip wall at rest,
adiabatic (`temperature = "zero_gradient"`); the species with no boundary entry (zero gradient). `physics`: `rho = 0.5`,
`nu = 6e-5` (`mu = 3e-5`), `cp = 1100`, `conductivity = 0.05`, `gravity = [0, 0, -9.81]`. Initial `T = 800`, at rest.
`[energy] enabled = true, buoyancy = false`. Species `vap` (`diffusivity = 3e-5`, `initial = 0`) and `air` (`remainder =
true`, `initial = 1`). `solver`: `piso`, `dt = 1e-5`, `end_time = 5e-3` (500 steps), `pressure_rel_tol = 1e-10`, euler.
`particles`: `density = 700`, Schiller-Naumann, analytic, gravity, `restitution = 0.8` (reflecting walls), `end_time = 5e-3`,
`two_way`, `heat_transfer`, `particle_cp = 2200`; one point injection at `(0.005, 0.0075, 0.0125)`, `rate = 1e4`,
`start_time = 0`, `end_time = 1e-3` (10 parcels), velocity `(60, 0, 0)`, `diameter = 2e-4`, `parcel_count = 5`,
`temperature = 350`; `[particles.evaporation]` with data E of ADR 0224 (`latent_heat = 3.2e5`, `boiling_temperature = 371.5`,
`boiling_pressure = pressure = 101325`, `vapour_molar_mass = 0.1`, `gas_molar_mass = 0.029`, `diffusivity = 3e-5`,
`vapour_cp = 2500`, `min_diameter = 1e-7`, `max_step_fraction = 0.1`, `species = "vap"`); `[particles.breakup]`
`surface_tension = 0.01`, `liquid_viscosity = 4e-4`, TAB constants, `seed = 31`. CBp: the same with `pimple`, `n_outer = 2`,
the first 100 steps. CBb: breakup alone (no evaporation table, `heat_transfer = false`, no energy, no species), 200 steps.

Injected totals (one injector, the parcels identical at injection): `N = injected()`, `m0 = 5 x 700 (pi/6) d0^3`,
`M_inj = N m0`, `P_inj = N m0 (60, 0, 0)`, `E_inj = N m0 (60^2/2 + c_l 350)`.

The identities at the end of every step (the carrier side is what its equations received or hold):

- **IM:** `M_liq + rho sum V (Y - Y0) = M_inj`;
- **IP:** `sum m v + D_p - G_p - W_p - B_p = P_inj`, `D_p` the delivered momentum `sum rho V a dt`, `G_p`, `W_p`, `B_p` the
  ledger's gravity, wall and breakup impulses;
- **IE:** `sum ((1/2) m v^2 + m c_l T_p) + rho c_p sum V (T - T0) + H_v + K - G_w - W_w - B_w = E_inj`.

Each residual is the norm of (left minus right) over the largest norm among its terms (each sum is one term).

## Registered checks

All in the new file `crates/rcfd-api/tests/droplet_feedback_f326.rs`, release
(`cargo test --release -p rcfd-api --test droplet_feedback_f326`).

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| C1 (a3 mass, a1 species) | CB, every step: IM | 1e-10 | below 1e-13 |
| C1b (a1 species) | CB, every step: `rho sum V (Y^{n+1} - Y^n)` against the step's record vapour mass, relative to the step's `sum abs(vapour_mass)` plus `1e-13 rho sum V` | 1e-8 | 1e-13 (tol 1e-13) |
| C2 (a3 momentum, a1 momentum) | CB, every step: IP; and `D_p` against the record's `sum (impulse + vapour_impulse)` | 1e-10; 1e-12 | round-off |
| C3 (a3 energy, a1 energy) | CB, every step: IE | 1e-10 | below 1e-12 |
| C3b (a1 heat) | CB, every step: `rho c_p sum V (T^{n+1} - T^n)` against the step's `sum Q_T`, relative to the step's `sum abs(Q_T)` plus `1e-13 rho c_p sum V T` | 1e-8 | 1e-12 (bound 4e-10) |
| SR (a1, the split) | CB, at the end: `delivered_heat + K + H_v` against the record's total energy `sum E V dt`; the vapour given to the species against the record's vapour mass | 1e-12 each | round-off |
| R1 (a2) | CB, every step: the tracker's `thermal().t_fluid` against `T` read before the step; its `vapour_fraction()` against `max(Y^n, 0)` read before the step | bit for bit | same values |
| CV (coverage) | CB at the end: breakup events; vapour mass; `H_v/E_inj`; the parcels' states recorded; the run did not fail | at least 1; `> 0`; `>= 1e-3`; as stated | 10; 0.09 to 0.9 |
| C4 (pimple) | CBp, every step: IM, IP, IE | 1e-10 each | as C1 to C3 |
| C5 (breakup alone) | CBb, every step: IP; breakup events at the end; the ledger's vapour mass | 1e-10; at least 1; zero | round-off; 10 |
| Z1 (a6) | the transient heated two-way case of D below without the tables, against the same with `[particles.evaporation] enabled = false` and `[particles.breakup] enabled = false` written: `u`, `p`, `phi`, `T` after every step and the tracker fingerprint at the end | bit for bit | the lines of main run |
| Z2 (a5) | CB's text without both tables: `to_toml` and `to_json_string` contain neither `evaporation` nor `breakup`; with them both texts contain them and `content_hash` differs | as stated | ADR 0266 read 6 |
| S (D2) | S1 to S7 each give a diagnostic at its path; each bad text is otherwise CB, which has none; the same bad table with `enabled = false` gives none | as stated | D2 |
| K0 (a5, a6) | unedited: `rcfd-api` `baseline_hashes`, `case_hashes_w48u`, `particles_two_way`, `particle_run`, `particles_case`, `particles_ext_case`, `scalars`, `dispersion_f255`, `wall_interaction_f254`, `erosion_presets_f254`, `rules_w19c`; `rcfd-particles` `droplet_exchange_f323`, `evaporation_f245`, `breakup_f245`, `lagrangian_ext`; `rcfd-cli` `cli`; `rcfd-mcp` `default_f1` | pass | nothing of main changed |

Case D of Z1: CB without the tables and without species, the injection `diameter = 2e-4` at `(60, 0, 0)`, 100 steps (the
heated two-way path of main on a closed box).

Printed, no limit: the identity terms at the end, the worst residuals, the step counts by state, the vapour fraction and
temperature extrema.

### Negative controls

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| NC1 | CB at the end: IE with `H_v` left out | at least 1e-3 | 0.09 to 0.9 |
| NC2 | CB at the end: IM with the carrier's vapour left out | at least 1e-3 | 0.06 to 0.9 |
| NC3 | CB at the end: IE with the temperature source taken as `energy_heat` alone (the carrier inventory replaced by the ledger's sum of `energy_heat`) | at least 1e-3 | `dm c_l (T^n - T_1)` over `E_inj`: 0.04 to 0.4 |

### Mutations (scratch copy, not committed)

Each applied alone to the final tree by an exact text replacement that must occur once; the named test run once in release
must fail; the file restored and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MF1 | the particle source of the species not added in `refresh` | C1 |
| MF2 | `Q_T` without the vapour terms (`energy_heat` alone) | C3 |
| MF3 | the vapour fraction not given to the tracker | R1 |
| MF4 | the momentum source without the vapour momentum (`impulse` alone) | C2 |
| MF5 | `enable_coupling()` in place of `enable_droplet_exchange()` with the tables | C1 (the run fails) |
| MF6 | the check S4 (species name) removed | S |

## Decision rule

- **done**: C1 to C5, C1b, C3b, SR, R1, CV, Z1, Z2, S and K0 are met, the three negative controls hold and the six
  mutations make their checks fail.
- **partial**: Z1, Z2, K0, R1 and C1, C2, C3 are met and a miss is confined to C1b, C3b, C4, C5, SR, CV or S, or a mutation is
  not caught; or the tables need a hash-table or golden edit (K0 fails on that alone): the tables are then taken out, the
  path and its library-level tests stay, and the schema step is reported as the maintainer's.
- **todo**: anything else. Z1 and K0 are preconditions of landing.

A negative control that does not hold is a defective set-up: corrected by an amendment before any re-run, never a pass.

## Limits (not covered)

- The steady two-way path keeps `enable_coupling` and refuses the tables (S2): the fixed-point iteration of ADR 0059 has no
  vapour species update; one-way tracking (no `two_way`) refuses them too (S1; evaporation needs the heat transfer, which
  needs two-way on main).
- The carrier stays constant-density and constant-property: the vapour mass is not a source of the continuity equation, the
  vapour's heat capacity is not in the temperature equation, and the gas properties seen by the droplets are the case's
  constants. So the energy identity holds with the ledger `H_v` (the enthalpy of the vapour as it arrived), not with
  `sum rho Y h_v(T)` evaluated on the fields; the difference is of the order of `M_v c_l Delta T` (dilute vapour). A
  variable-density carrier with a vapour species is F-252's.
- The mechanical energy of the exchange is not fed into the temperature (as ADR 0059 on main); its sum is the ledger `K`.
  The momentum the carrier receives is checked as delivered (ADR 0059 PL6), not as the gas momentum of the box (the walls of a
  closed box exert forces on the gas).
- The breakup's normal momentum stays a residual without a carrier counterpart (ADR 0230 D5, its Q2).
- Film, erosion, rough walls and the random walk with the droplet tables are not exercised; the film is refused with
  `two_way` on main already.
- BDF2: the per-step identities hold for euler (the case's scheme); with BDF2 the inventory follows the scheme's
  three-level relation and IM, IE are not registered for it.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default.

- **T1. Tables with `enabled`**, as the film, erosion, rough-wall and dispersion tables (absent = off; a written table with
  `enabled = false` = off).
- **T2. Transient two-way only** (S1, S2); the steady path and one-way tracking are listed as a question.
- **T3. The energy split of D4** (temperature gets the sensible part; the mechanical part and the vapour's enthalpy at the
  carrier temperature are ledgers). The other option, all of `droplet_sources().energy` into the temperature equation, gives
  an exact inventory of `rho c_p T` but heats the gas by `c_l T` per kilogram of vapour (about 600 K per unit vapour-to-gas
  mass ratio at 300 K for data E), which is not physical for a constant-mass carrier.
- **T4. Normalisation by the flow step** `dt` in droplet mode (main normalises by the tracking interval `h`, which differs
  only on the last tracking step; main is unchanged).
- **T5. The seen vapour fraction is clamped below at 0** (round-off undershoot of the transport); a value at or above 1 stops
  the run.

## Open questions (for the addendum)

- Q1: the steady two-way path with the tables (a vapour species in the fixed point). Q2: the vapour mass as a source of a
  variable-density continuity equation (with F-252). Q3: one record method or two (ADR 0230 Q1), now that a case path reads
  the vapour terms.

## Amendment 1 (2026-11-12, written before any run of the checks)

Found by reading the schema while writing the code, before the test file existed: `vof.mass_transfer` is a plain
(non-optional) object of the schema with the key `evaporation_frequency`, so `to_toml` and `to_json_string` of every case
contain the word `evaporation` whatever `particles` holds. Z2 as written above ("contain neither `evaporation` nor
`breakup`") would fail on that key alone: a defective set-up. Z2 is replaced by **Z2a**: CB's text without both tables:
`to_toml` has no line `[particles.evaporation]` or `[particles.breakup]` and the `particles` object of `to_json_string`
has no key `evaporation` or `breakup`; with them both texts have them and `content_hash` differs. Limit and estimate as
registered. No other check changes.

NC3 needs the record's `sum energy_heat`, which D6 did not list: the ledger gains `heat_raw` (that sum). No limit changes.

## Amendment 2 (2026-11-12, after the first registered run, before any corrected run)

The first run (code d9f7f943, `w49i-work/f326-scratch/run1.log`) passed Z1, Z2a and S and stopped CB, C4 and C5 at step 1:

- **C4, C5: "IM step 1: NaN", "IP step 1: NaN".** No parcel is injected in the first step (`floor(1e4 x 1e-5) = 0`), every
  term of the identities is zero and the test divided zero by zero. ADR 0230's convention (its test's `identities`: the
  residual is 0 when the largest term is 0) was meant and not written. A defect of the test code.
- **CB: "C3b step 1: 7.99e-4".** With no source, the carrier's thermal inventory moved by `5.6e-16` J (round-off of the
  temperature solve), against the floor `1e-13 rho c_p sum V T = 7.0e-13` J that the registration put in the denominator;
  with the limit `1e-8` on top, the floor allowed `7e-21` J. The registration's own estimate (round-off of order `1e-15` J
  against a floor of `7e-13` J) shows the floor was meant as an absolute allowance for round-off, not as a denominator; as
  written it can only pass with a source. A defect of the set-up (my registration), not a miss of the code. C1b has the same
  form (it did not fail: the vapour fraction is exactly 0 until the first evaporation).

Corrections (numbers unchanged):

- **A2.1** The residual of IM, IP, IE is 0 when every term of the identity is 0 (C1, C2, C3, C4, C5, ADR 0230's convention).
- **A2.2** C1b reads `abs(rho sum V (Y^{n+1} - Y^n) - vapour_mass) <= 1e-8 sum abs(vapour_mass) + 1e-13 rho sum V`; C3b reads
  `abs(rho c_p sum V (T^{n+1} - T^n) - sum Q_T) <= 1e-8 sum abs(Q_T) + 1e-13 rho c_p sum V abs(T)`. The relative limit and the
  allowance are the registered numbers. Estimate of the allowance's use: the round-off of the solve, about `1e-15` J against
  `7e-13` J (the measured `5.6e-16` J at step 1, seen in the first run, is consistent with it).

The three tests of the first run stay in the file as ignored records (`*_setup_defect`) with their measured numbers; the
corrected tests are new functions. Nothing else changes; the corrected run is made once.

## Amendment 3 (2026-11-12, after the corrected run of amendment 2, before any further run)

The corrected run (47edd4c0, `w49i-work/f326-scratch/run2.log`) passed C5 (worst IP `2.4e-16`, 10 breakup events, no
vapour) and stopped CB and CBp at step 1 with "IE step 1: 1e0". Before the first parcel is injected (the first nine steps),
the only non-zero term of IE is the round-off of the carrier's thermal inventory (the `5.6e-16` J of amendment 2), so the
residual divided by the largest term is that term over itself, 1, whatever the size of the round-off. The same defect as
A2.1 in another form: a relative residual is meaningless while the identity holds no parcel. A defect of the set-up, not a
miss of the code.

- **A3.1** IM, IP and IE (C1, C2, C3, C4, C5) are evaluated at the end of every step with at least one parcel injected
  (`injected() > 0`), with A2.1 kept. Before the first injection the carrier's inventories are checked by C1b and C3b with
  the round-off allowance of A2.2 (that is, at every step, as registered). Limits unchanged.

The two tests stopped by it stay as ignored records (`*_a2`, renamed `*_setup_defect2`); C5 of amendment 2 met its limits and
stays as it is. The corrected tests are new functions; the run is made once.

## Addendum 1: results (2026-11-12)

Release builds on Windows. Registration c0a50c58; amendment 1 eb7b29bf (before any run); code and tests d9f7f943 (committed
before their first run); first run `w49i-work/f326-scratch/run1.log`; amendment 2 e524592a and its tests 47edd4c0, run
`run2.log`; amendment 3 f04d22cb and its tests 81d883c9, run `run3.log` (the run whose numbers are below, except C5). Each run
was made once; no limit was changed. The new file has 11 tests (6 run, 5 ignored records), 0.8 s in total.

### Results

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| C1 IM at every step (A3.1: steps with a parcel) | 1e-10 | worst 1.8e-15 | yes |
| C1b species against the step's vapour (A2.2: error over `1e-8 S + 1e-13 rho sum V`) | 1 | worst 1.6e-5 | yes |
| C2 IP at every step; delivered momentum against the record | 1e-10; 1e-12 | 5.8e-16; 2.3e-16 | yes |
| C3 IE at every step | 1e-10 | worst 6.7e-13 | yes |
| C3b temperature against the step's `Q_T` (A2.2, error over the bound) | 1 | worst 8.0e-4 (step 1, the round-off of amendment 2) | yes |
| SR split of the energy source; vapour to the species | 1e-12 each | 3.7e-15; 1.3e-15 | yes |
| R1 tracker `t_fluid` and vapour fraction against `T^n`, `max(Y^n, 0)` | bit for bit | equal at all 500 steps | yes |
| CV breakup events; vapour; `H_v/E_inj`; no failure | >= 1; > 0; >= 1e-3 | 10; `1.4455e-7` kg (0.986 of the injected liquid); 2.51; none | yes |
| C4 pimple, 100 steps: IM, IP, IE | 1e-10 each | 3.0e-16, 4.3e-16, 5.8e-13 | yes |
| C5 breakup alone, 200 steps: IP; events; vapour | 1e-10; >= 1; zero | 2.4e-16; 10; 0 (run of amendment 2, A2.1) | yes |
| Z1 tables disabled against absent, 100 steps: u, p, phi, T, fingerprint | bit for bit | equal (10 parcels, the gas moving) | yes |
| Z2a (amendment 1) texts and hash | as stated | no table in either text without, both with; hashes differ | yes |
| S located errors S1 to S7 (17 texts) and the inert controls | as stated | all located, none left with the tables disabled | yes |
| K0 suites, unedited | pass | rcfd-api baseline_hashes 3+1 ignored, case_hashes_w48u 1, particles_two_way 14+6, particle_run 9, particles_case 6+1, particles_ext_case 3, scalars 2, dispersion_f255 15+1, wall_interaction_f254 6, erosion_presets_f254 6, rules_w19c 8+1 (the last three and H1 again after the docs edit); every rcfd-particles test binary (droplet_exchange_f323 10, breakup_f245 8, lagrangian_ext 13+1, closed_forms 24+3, ...); rcfd-cli cli 8; rcfd-mcp default_f1 2; 0 failed | yes |
| NC1 IE without `H_v` | >= 1e-3 | 1.0 | held |
| NC2 IM without the carrier vapour | >= 1e-3 | 0.986 | held |
| NC3 IE with `energy_heat` alone | >= 1e-3 | 0.444 | held |

CB at the end (500 steps): 10 parcels, 8 active and 2 evaporated; IE terms [liquid, carrier heat, `H_v`, `K`, gravity, wall,
breakup, injected] = [1.60e-3, -0.1727, 0.2840, 2.46e-4, -1.6e-10, -1.74e-5, 4.4e-8, 0.1132] J; the vapour fraction in
[-9.7e-4, 0.112]; the coolest cell 682.3 K. The evaporation went further than estimated (98.6 percent of the liquid), so
`H_v/E_inj` is above 1 (the datum is the liquid at 0 K).

### Mutations

Each alone on the `git archive` copy of 81d883c9 (`w49i-work/f326-scratch/mut`, script `mutate.py`, log `mutations.log`),
its test run once in release; every file restored and equal byte for byte.

| Id | Must fail | Result |
| --- | --- | --- |
| MF1 species source not added | C1 | failed in CB at C1b, step 11 (error over the bound 1.1e7; C1b precedes C1 in the step) |
| MF2 `Q_T = energy_heat` | C3 | failed: C3 IE 8.3e-4 at step 11 |
| MF3 vapour fraction not given | R1 | failed: R1 vapour fraction at step 12 |
| MF4 momentum without the vapour | C2 | failed: C2 delivered 0.33 at step 11 |
| MF5 `enable_coupling` with the tables | C1 (the run fails) | failed: step 1, "evaporation and breakup: not available with the exchange record" |
| MF6 S4 removed | S | failed: `particles.evaporation.species` not located |

### Tools

`cargo fmt -p rcfd-api --check`, `cargo fmt -p rcfd-particles --check`: clean. `cargo clippy --release -p rcfd-api -p
rcfd-particles --all-targets -- -D warnings`: clean. The whole workspace suite was not run (the lead's gate).

### Deviations and disclosures

- Two of the three amendments followed runs: amendment 2 after the first run stopped CB, C4 and C5 at step 1 (0/0 before
  the first injection; a round-off floor used as a denominator), amendment 3 after the second stopped CB and C4 at step 1
  (the carrier's round-off inventory as the only non-zero term). Both were defects of the relative measures before any
  parcel exists, not of the code; no limit number changed. The stopped tests are kept as ignored `*_setup_defect` and
  `*_setup_defect2` records. The test bodies were moved into `cb_run`, `c4_run`, `c5_run` with flags; the first-run logic is
  kept under the old flags.
- C5's numbers are from the run of amendment 2 (it passed there; A3.1 does not change it).
- The vapour species undershoots to -9.7e-4 (deferred-correction convection near the evaporating parcels); the tracker sees
  it clamped at 0 (T5). The inventory identity is unaffected (conservative scheme).
- The shared solver file `sim.rs` has two insertions (new-file lines 4053 to 4059 and 4239 to 4242); `baseline_hashes` passes.

### Decision by the registered rule

Every check (as amended) met its limit, the three negative controls held, the six mutations failed their tests: **done by
the rule**, pending the maintainer's acceptance. No hash table, golden or existing test was edited; the tables needed no
`DEFAULT_KEYS` entry.

### Remaining questions

- **Q1. The steady two-way path with the tables.** (a) keep them refused (as built); (b) a vapour species in the fixed point of
  ADR 0059. Recommendation: (a) until a case needs it.
- **Q2. The vapour as a mass source of the carrier.** (a) dilute, constant density (as built); (b) a variable-density carrier
  (F-252). Recommendation: (b) with F-252.
- **Q3. One record method or two** (ADR 0230 Q1). (a) keep both; (b) merge and retire RF1/RF2 of ADR 0224.
  Recommendation: (a): the steady path and API callers still use `enable_coupling`.
- **Q4. Species undershoot.** (a) clamp what the tracker sees (as built); (b) a bounded scheme for the vapour species when the
  table is on. Recommendation: (a); (b) changes a solver default of the species. Open (it would change a default).

### For the lead: roadmap entries

**F-326: proposed status `done`.** Proposed note:

> From ADR 0230 question 5, recommendation taken on the maintainer's standing instruction of 2026-11-11 (a separate item, checked by conservation without a data set). DONE (ADR 0348, 2026-11-12; registered before the code, three amendments, two of them after runs that stopped at step 1 on the relative measures before the first injection, no limit changed): optional tables [particles.evaporation] (the film-model data of ADR 0224 and the transported species that receives the vapour) and [particles.breakup] (TAB constants), absent from every case that does not write them; transient two-way only (piso, pimple; evaporation also needs heat_transfer). The tracker records with enable_droplet_exchange and reads T and the vapour fraction of the old time level (bit for bit); over each flow step the carrier receives the mass of Coupling::droplet_sources as the species source, its momentum as the body acceleration, and of its energy the sensible part energy_heat + vapour_enthalpy - vapour_mass (c_l T + L_v) in the temperature equation, the mechanical part and the enthalpy of the arrived vapour kept in a ledger. A closed adiabatic box (16 cells, 10 parcels breaking up and evaporating 98.6 percent of their mass) conserves gas plus liquid mass, momentum and energy at every step to 1.8e-15, 5.8e-16 and 6.7e-13 of the largest term (limit 1e-10), with pimple too (5.8e-13); three negative controls held, six mutations caught. A case without the tables or with them disabled is bit-identical (baseline hashes, case_hashes_w48u, the particle suites). Not in it: the steady path, a variable-density carrier (the vapour is not a continuity source), the breakup's normal momentum (a residual, ADR 0230 D5). Open: ADR 0348 Q1 to Q4.

**F-323: proposed note change** (replace its last sentence "Case-tree wiring (optional particles.evaporation and particles.breakup tables, ADR 0224 question 2) goes with F-252." with):

> Case-tree wiring of the transient two-way path (optional particles.evaporation and particles.breakup tables) done by F-326 (ADR 0348, 2026-11-12); the comparison with data stays with F-252.
