# ADR 0208: F-255 registration: particle dispersion from turbulence-model fields and particle surface reactions

Status: **REGISTERED 2026-11-07**, written before any code of F-255 and before any run of its checks. Not yet accepted by
the maintainer. Before this text, two things were run on main d953cd6 with the code of main as it is: the record tests of
checks D2 and D3 (R8, R9), which made the records below, and a scratch probe (not committed) that built the case-tree
set-ups (item 8 of the code read). The probe measured one number close to a registered check, disclosed in item 8; the
registered runs use other seeds. Results go in the addendum at the end.

Parents: [ADR 0028](0028-wave18c-particles.md) (F-056, the tracker), [ADR 0057](0057-wave29c-lagrangian-extensions.md)
(F-128: the discrete random walk D2, its closed form and the checks L7 to L9, L14, L15),
[ADR 0059](0059-wave30a-wiring-particles-mixing-plane.md) (two-way coupling in the solver loops),
[ADR 0192](0192-f253-registration-particle-wall-film.md) (the record R1 and the house rules of the particle items),
[ADR 0194](0194-f254-registration-particle-wall-interaction.md), [ADR 0206](0206-f292-registration-gas-buoyancy-and-gas-shear-in-the-particle-wall-film.md)
(records R1 to R7 and the form of this registration), [ADR 0199](0199-f286-registration-transient-k-omega-sst-with-pimple.md)
(transient SST on pimple), [ADR 0156](0156-job48v-adaptive-pressure-correctors.md) D6 (`hash_omit_default`),
[ADR 0137](0137-linux-gate-platform-values.md) (platform values). Roadmap item F-255 (depends on F-128), split from F-245
by the user on 2026-10-22.

Number: the lead gave 0208 to F-255. Main d953cd6 holds 0206 as its highest; 0204 is on `f291-gradient-options` and 0205
on `f295-seam-diffusivity`. No branch holds a 0207 or a 0208.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0192, ADR 0206):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as
  an ignored record;
- every new code path is opt-in: a case or a library caller without the new keys or calls runs, hashes and writes its
  results as before, bit for bit;
- each new case key uses `hash_omit_default`; neither `crates/rcfd-api/tests/baseline_hashes.rs` nor the record R0
  (`case_hashes_w48u.rs`) is edited or re-recorded, no golden is re-recorded, and no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Clean-room: the dispersion closed forms are derived below from the definition of the displacement; the conversion
`epsilon = beta* k omega` is the one the repository already uses (`chemistry::mixing_vectors`, the `chemistry.model` row of
`docs/case-schema.md`) with the SST `beta*` that `docs/reference-data.md` lists as verified against the NASA TMR page. The surface-reaction rate law is
a choice of this text (D4), not taken from a source. Status words as in ADR 0192: "derived" means derived here;
"registered here" marks a choice of this text that no source and no instruction fixes. No source was read for this work,
and no vendor code, text or install was used.

## What F-255 asks

The roadmap entry: particle dispersion driven by turbulence-model fields reproduces the analytic dispersion growth of a
passive cloud in homogeneous turbulence (Taylor result, closed form derived in the ADR) within 5 percent, and a particle
surface reaction conserves the mass of each species to 1e-10 relative. The note: split from F-245 on 2026-10-22; these
were in F-128's not-delivered list; acceptance text proposed by the lead, refine when scheduled; size L.

Readings of this text (readings, not decisions; Q2 and Q7 ask the maintainer):

- **"Driven by turbulence-model fields"**: the discrete random walk of ADR 0057 D2 takes `k` and `epsilon` per cell from
  the case's turbulence model (k-epsilon, or k-omega SST through `epsilon = beta* k omega`), and in a transient run takes
  the fields of the current step, in place of the homogeneous constants `dispersion_k` and `dispersion_epsilon`.
- **"A passive cloud"**: a cloud of tracer parcels, Stokes parcels with a relaxation time a thousandth of the Lagrangian
  integral time. A point particle of the tracker always has drag, so a fluid particle is its limit `tau -> 0`; the closed
  form below is derived for any `tau`, and the check compares with both the exact form for the cloud's `tau` and Taylor's
  `tau = 0` form.
- **"Homogeneous turbulence"**: the turbulence model's fields uniform over the mesh. Without production the k-epsilon and
  SST equations have no stationary homogeneous solution (the turbulence decays: item 8), so the homogeneous check tracks
  on the model's state before its first sweep, which is uniform at `turbulence.k_initial`, `epsilon_initial` or
  `omega_initial` (Q5).
- **"Within 5 percent"**: the mean-square displacement per component of the cloud at four times that span the ballistic
  and the diffusive ranges, each within 5 percent of the closed form.
- **"A particle surface reaction conserves the mass of each species"**: for each species, the solid in the parcels and
  every gas species of the reaction, the mass the reaction takes from or gives to the parcels or the carrier equals its
  stoichiometric share of the reaction extent, measured independently on the parcel side and on the carrier side, to
  1e-10 relative; and the two phases together lose no mass. No surface reaction exists in the repository (item 7), so a
  minimal one is registered here (D4, Q7).

## What the code read shows (main d953cd6)

1. **The random walk.** `rcfd_particles::dispersion` (ADR 0057 D2): `Dispersion {c_l, crossing}` (default 0.15, true),
   `sigma(k) = sqrt(2k/3)`, `t_lagrangian(c_l, k, eps) = c_l k / eps`, `eddy_length(k, eps) = C_MU^(3/4) k^(3/2) / eps`
   with the constant `C_MU = 0.09`, `crossing_time(tau, l_e, w)`. `Eddy::draw` draws three standard normals times `sigma`
   and an exponential lifetime of mean `T_L` (one serial generator), limited by the crossing time when `crossing`; with
   `k <= 0` or `eps <= 0` it holds a zero fluctuation for an infinite time (Q6).
2. **The tracker.** `Tracker::set_dispersion(model, k, eps, seed)` stores one `k` and one `eps` per cell (refused when a
   length is not the cell count or `c_l` is not positive); `advance_particle` draws a new eddy whenever the parcel's
   interaction time has run out, with the `k` and `eps` of the cell where the sub-step starts, truncates the sub-step at
   the end of the interaction time and carries the eddy across flow steps. There is no call to replace the arrays.
   `fingerprint` holds the parcels, then the film, erosion and rough-wall parts when they are set.
3. **The case tree.** `particles.dispersion = "none" | "drw"` with `dispersion_k`, `dispersion_epsilon` (unset `-1`,
   located errors when unset with `drw`), `dispersion_cl`, `dispersion_crossing`, `dispersion_seed`; `build_tracker`
   passes `vec![dispersion_k; n]` and `vec![dispersion_epsilon; n]` and the seed `dispersion_seed + 0x5EED_D15B`. The
   tracker is built in `Simulation::new` for the transient engines (the run path's `ParticleRun`), by
   `particle_run::steady_run` after a steady run, by `particles::track_frozen` and `track_transient` (library hooks), and
   by `particle_coupling::SteadyCoupling` for every re-track of the steady two-way coupling.
4. **The transient hooks.** `ParticleRun::after_step(sim)` advances the tracker over the step just made on the flow at
   the end of the step (`flow_data(sim)`); with two-way coupling, `ParticleRun::pre_step` (called in `Simulation::step`
   before the flow step, on the velocity of the old time level) does the tracking and `after_step` returns early.
   `track_transient` calls `sim.step()` and then advances on the new field.
5. **Turbulence fields.** `Simulation::field("k" | "epsilon" | "omega")` gives the model's cell values when the model
   transports them; the turbulence state is the private field `turb` of `Simulation` (`TurbState`, `pub(crate)`,
   `scalar(name)`). `chemistry::mixing_vectors` already turns the model state into `k` and `epsilon` per cell, with
   `epsilon = 0.09 k omega` for `k_omega_sst`, and `chemistry::check` admits only `k_epsilon` and `k_omega_sst` for the
   closures that need them. `rcfd_turb::sst::BETA_STAR = 0.09` is a public constant that the SST model does not let a
   case change (`SST_COEFFICIENTS`).
6. **Models and algorithms.** k-epsilon runs with the steady algorithms only; k-omega SST with the steady algorithms and
   with pimple on a static mesh (ADR 0199); RANS turbulence is refused with the energy equation and with a prescribed flow
   (`solver.frozen_flow`, ADR 0024 D5); the Reynolds stress model transports the stresses and `epsilon`, and has no
   scalar named `k`.
7. **Reactions.** The repository has gas-phase chemistry between `species.<name>` mass fractions (`chemistry`,
   `rcfd-reactor`) and nothing for a particle. A parcel carries `d`, `rho` and `parcel` (real particles); its mass is
   `parcel rho pi d^3 / 6` everywhere it is used (the exchange record, erosion, film absorption, the run report).
8. **The probe (scratch, main d953cd6).** Its first run used the box of case T below with `energy.enabled` and
   `solver.frozen_flow`: refused ("RANS turbulence together with the energy equation is not supported", "a turbulence model
   cannot be combined with solid zones or a prescribed flow", and "k-epsilon is available with the steady algorithms").
   Without the energy equation and the frozen flow (case T as registered, steady simple): `Simulation::new` accepts it, the
   k-epsilon and SST fields are uniform at the initial values in all 125 cells (k 1.5, epsilon 0.225, omega
   1.6666666666666667), and after two steady sweeps they are not (k 0.1357 to 0.1508, epsilon 3.1e-3 to 2.0e-2 for
   k-epsilon; k 0.1361 to 0.1373, omega 0.1512 to 0.1524 for SST). Case P (SST on pimple, dt 0.05): every step changes
   the fields (k 1.5 -> 1.48883 -> 1.47783 -> 1.46697, spread over the cells 3e-12 to 1e-6, omega spread up to 1e-4).
   **Disclosure:** the probe also ran `track_frozen` on T with the constant keys `dispersion_k = 1.5`,
   `dispersion_epsilon = 0.225`, the default `dispersion_seed = 1` and 20 000 parcels (0.7 s): the mean-square displacement
   per component at 10 s was 17.959, against 17.998 from the closed form D3 below (-0.22 percent). That run uses the
   arithmetic CT3 will use at 10 s; CT3 and the other statistical checks use the seeds 20261107 and above, so their samples
   are not the probe's. Nothing else that a registered check measures was measured.

## The model

**D1. The random walk (ADR 0057 D2, unchanged).** A parcel sees `u + u'`, `u'_i = zeta_i sigma`, `sigma^2 = 2k/3`, held
for `min(t_e, t_cross)` with `t_e` exponential of mean `T_L = C_L k/epsilon` and `t_cross` the crossing time of the eddy
length `L_e = C_mu^(3/4) k^(3/2)/epsilon`, `C_mu = 0.09`, with `k` and `epsilon` of the cell where the eddy is drawn.
Nothing of D1 changes; F-255 changes where `k` and `epsilon` come from.

**D2. Fields of the turbulence model (registered here).** With `particles.dispersion_fields = "turbulence_model"` the
tracker takes, per cell, `k` and `epsilon` of the case's turbulence model:

- `turbulence.model = "k_epsilon"` (any variant): the transported `k` and `epsilon`;
- `turbulence.model = "k_omega_sst"`: the transported `k` and `epsilon = beta* k omega`, `beta* = rcfd_turb::sst::BETA_STAR`
  (0.09), as `chemistry::mixing_vectors` does;
- any other model (laminar, Spalart-Allmaras, mixing length, the LES models, the Reynolds stress model, a plugin): refused
  (no `k`, or anisotropic stresses; Q3).

The values are passed as the model holds them. When the fields are read:

- steady and frozen tracking (`track_frozen`, the run path's `steady_run` after a steady run, each re-track of the steady
  two-way coupling): once, when the tracker is built, from the current state;
- transient tracking without two-way coupling (`ParticleRun::after_step`, `track_transient`): before every tracking step,
  the fields at the end of the flow step, the time level of the velocity the step tracks on;
- transient two-way coupling (`ParticleRun::pre_step`): before every tracking step, the fields of the old time level, the
  level of the velocity `pre_step` tracks on.

The eddy a parcel holds is not redrawn when the fields are replaced; the next draw uses the new fields. The constant
`C_mu` of the eddy length stays 0.09 whatever the model's `c_mu` (Q4).

**D3. The closed form (derived).** In homogeneous fields with no mean flow, the fluid velocity a parcel sees is a renewal
process: a Gaussian value of variance `sigma^2` per component, held for an exponential time of mean `T = T_L`, then
replaced by an independent value. Without crossing times (`crossing = false`, or a parcel too small for the crossing time
to be finite, see below) the probability that no replacement falls in a lag `s` is `exp(-s/T)`, so the autocorrelation of
each component is `R(s) = sigma^2 exp(-abs(s)/T)` (ADR 0057 D2), and the process starts stationary at `t = 0` because the
exponential lifetime has no memory. A Stokes parcel of relaxation time `tau` released at rest obeys
`dv/dt = (u - v)/tau`, so

    v(t) = (1/tau) int_0^t exp(-(t - a)/tau) u(a) da,    X(t) = int_0^t v = int_0^t u(a) (1 - exp(-(t - a)/tau)) da

and with `p = t - a`, `q = t - b`, `g(p) = 1 - exp(-p/tau)`:

    <X^2>(t) = int_0^t int_0^t g(p) g(q) R(p - q) dp dq = sigma^2 (I0 - 2 I1 + I2)

with `a = 1/tau`, `b = 1/T` and

    I0 = int int exp(-b abs(p - q))                  = 2 T^2 (t/T - 1 + exp(-t/T))
    I1 = int int exp(-a p) exp(-b abs(p - q))        = T (2 (1 - exp(-a t))/a - (1 - exp(-(a + b) t))/(a + b) - t G(b, a))
    I2 = int int exp(-a (p + q)) exp(-b abs(p - q))  = (1 - exp(-(a + b) t) - (a + b) t G(a + b, 2a)) / (a (a + b))

where `G(c, e) = (exp(-c t) - exp(-e t)) / ((e - c) t) = exp(-min(c, e) t) phi(abs(e - c) t)`, `phi(y) = (1 - exp(-y))/y`
(`phi(0) = 1`), which is finite and free of cancellation when `c = e` (`tau = T`). `I1` uses
`int_0^t exp(-b abs(p - q)) dq = T (2 - exp(-b p) - exp(-b (t - p)))`; `I2` is twice the integral over `q < p` and was
reduced with `(F(c1) - F(c2))/(c2 - c1) = (1 - exp(-c1 t)(1 + c1 t phi((c2 - c1) t)))/(c1 c2)`,
`F(c) = (1 - exp(-c t))/c`.

- `tau -> 0` (a fluid particle): `I1, I2 -> 0` and `<X^2> = 2 sigma^2 T^2 (t/T - 1 + exp(-t/T))`, **Taylor's result** for
  an exponential autocorrelation (Taylor 1921, not read; the formula follows from `<X^2> = 2 int_0^t (t - s) R(s) ds`):
  `sigma^2 t^2` for `t << T`, `2 sigma^2 T (t - T)` for `t >> T`.
- `t -> infinity`: `<X^2> = 2 sigma^2 T t - sigma^2 (2 T^2 + 4 T tau - tau T (2T + tau)/(T + tau)) + ...`, so the
  long-time diffusivity `sigma^2 T` does not depend on `tau` (the Tchen/Hinze result recalled in ADR 0057, there for the
  velocity variance).
- An independent derivation used by CF1: `u` is a Markov jump process, so the second moments of `(X, v, u)` obey the
  linear system `d<vu>/dt = a (sigma^2 - <vu>) - b <vu>`, `d<v^2>/dt = 2a (<vu> - <v^2>)`, `d<Xu>/dt = <vu> - b <Xu>`,
  `d<Xv>/dt = <v^2> + a (<Xu> - <Xv>)`, `d<X^2>/dt = 2 <Xv>`, from zero (`<u^2> = sigma^2` throughout). A Python model of
  the system (mine, not in the repository) agrees with the closed form to 5.4e-11 relative or better on the grid of CF1.

Crossing times. For the tracer of the checks (`tau = 1e-3 T`) the argument `L_e / (tau w)` of the crossing time exceeds 1,
so the crossing time is infinite, unless the relative speed `w` exceeds `L_e / tau = 1341` m/s, 1300 times `sigma`: the
default `crossing = true` leaves the tracer's eddies at their lifetimes, and D3 holds exactly. The inertial cloud of CT5
(`tau = T`) runs with `crossing = false`.

**D4. The surface reaction (registered here; library only).** One heterogeneous reaction between a solid in the parcels
and gas species of the carrier:

    nu_s S(solid) + sum_{nu_i < 0} abs(nu_i) G_i  ->  sum_{nu_i > 0} nu_i G_i

given by the caller: the solid's molar mass `M_s` and coefficient `nu_s > 0`; for each gas species a name, a molar mass
`M_i` and a signed coefficient `nu_i` (negative consumed, positive produced, not zero); the index `o` of the gas species
whose concentration sets the rate (`nu_o < 0`); the rate coefficient `k_s` [m/s]; the reacting mass fraction `f_r` of a
parcel, in `(0, 1)`. The molar masses must balance: `abs(sum_i nu_i M_i - nu_s M_s) <= 1e-12 nu_s M_s`, else refused.

- R1. Rate per unit external surface of a particle: `omega = k_s rho_g Y_o / M_o` [mol/(m^2 s)], first order in the molar
  concentration of `G_o` in the cell where the sub-step starts (`Y_o` a mass fraction per cell given by the caller,
  `rho_g = Flow::rho`); kinetic control, no film diffusion, no temperature dependence.
- R2. Composition: at a parcel's first sub-step with the reaction on, its mass per real particle `m0 = rho_p pi d^3/6`
  splits into the reacting mass `m_r = f_r m0` and the inert mass `m_i = (1 - f_r) m0`. The density `rho_p` stays; the
  diameter follows the mass, `d = (6 (m_i + m_r) / (pi rho_p))^(1/3)` (a shrinking particle at constant density).
- R3. Kinetics (derived): `dm/dt = -nu_s M_s omega pi d^2` and `m = rho_p pi d^3/6` give `dd/dt = -2 nu_s M_s omega / rho_p`,
  so over a sub-step `h` with `omega` frozen the diameter falls linearly, exactly. Per real particle:
  `d_lin = d - 2 nu_s M_s omega h / rho_p`, `dm = min(m_r, (pi rho_p / 6)(d^3 - max(d_lin, 0)^3))`; when `dm > 0`,
  `m_r <- m_r - dm` (0 exactly when `dm = m_r`) and `d` from R2; when `dm = 0` nothing changes, not even the bits of `d`.
  A parcel whose `m_r` is 0 keeps the inert diameter `d_i = d0 (1 - f_r)^(1/3)` and reacts no more.
- R4. Booking (an observation for the carrier, as the exchange record of ADR 0057 D1): per parcel of weight `n` the
  extent `dXi = n dm / (nu_s M_s)` [mol]; the cell where the sub-step starts receives the mass `nu_i M_i dXi` [kg] of every
  gas species `i` (signed); the totals `solid_reacted += n dm` and `extent += dXi`. The carrier's fields are not changed:
  the caller may feed the sources to its species equations.
- R5. Where: after the motion of each sub-step, over the sub-step's length (`h`, or the time to the wall hit), before the
  wall action; erosion and film absorption at that hit use the mass after the reaction.

Closed form (derived from R3): a parcel at rest in a uniform `Y_o` has `d(t) = d0 - 2 nu_s M_s omega t / rho_p` until
`d_i`, reached at `t_b = rho_p (d0 - d_i) / (2 nu_s M_s omega)`.

## Settings and code

**S1. Library (`rcfd-particles`).**

- `dispersion`: `taylor_msd(sigma2, t_l, t)` (D3 with `tau = 0`) and `drw_msd(sigma2, t_l, tau, t)` (D3), the mean-square
  displacement per component.
- `Tracker::set_dispersion_fields(k, eps)`: replaces the per-cell arrays of the random walk (copied from slices); refused
  when dispersion is not set, when a length is not the cell count, or when a value is not finite. Read access
  `Tracker::dispersion_fields() -> Option<(&[f64], &[f64])>`. The generator, the model and the eddies are untouched.
- module `reaction`: `GasSpecies {name, molar_mass, coefficient}`, `ReactionSpec {solid_molar_mass, solid_coefficient,
  gas, oxidiser, rate, reacting_fraction}`, `SurfaceReaction` with read access `spec()`, `sources(i) -> &[f64]` (kg per
  cell of gas species `i`), `solid_reacted()` (kg), `extent()` (mol), `oxidiser_fraction() -> &[f64]`.
- `Tracker::set_reaction(spec, y_o)`: refused when a reaction is already set, when heat transfer or the exchange record is
  on, and for a `ReactionSpec` that fails one of: `rate`, `solid_molar_mass`, `solid_coefficient` finite and positive;
  `reacting_fraction` in `(0, 1)`; at least one gas species; every gas molar mass finite and positive, every coefficient
  finite and not zero; `oxidiser` an index with a negative coefficient; the balance of D4; `y_o` one finite value in
  `[0, 1]` per cell. `Tracker::set_reaction_oxidiser(y_o)` replaces `Y_o` under the same rule (refused without a reaction).
  `Tracker::reaction()`, `Tracker::reacting_mass(i)` (the reacting mass per real particle of parcel `i`, NaN before its
  first sub-step with the reaction on). `set_thermal` is refused when a reaction is set, and `advance` refuses a tracker
  with a reaction and the exchange record (`enable_coupling` has no result to refuse with).
- `fingerprint`: with a reaction set, after everything it holds today, the source arrays, `solid_reacted`, `extent` and the
  per-parcel reacting masses. Without one, unchanged.
- Unchanged: `Particle`, `Flow`, `Options`, `InjectorSpec`, `Dispersion`, `Eddy`, every existing result.

**S2. Case tree.** One field of `particles`, `.hash_omit_default()`:

| Key | Default | Meaning |
| --- | --- | --- |
| `particles.dispersion_fields` | `"constant"` | `"constant"`: `dispersion_k` and `dispersion_epsilon` (ADR 0057); `"turbulence_model"`: `k` and `epsilon` per cell of the turbulence model (D2) |

Located errors (`particles::check`, without the mesh, only with `particles.enabled`, as every particle key):

- DE1 at `particles.dispersion_fields`: `"turbulence_model"` with `particles.dispersion` other than `"drw"`;
- DE2 at `particles.dispersion_fields`: `"turbulence_model"` with `turbulence.model` other than `"k_epsilon"` or
  `"k_omega_sst"`;
- DE3 at `particles.dispersion_k`, at `particles.dispersion_epsilon`: the key set (not `-1`) with `"turbulence_model"`
  (two sources of `k` would be ambiguous).

With `"turbulence_model"` the errors of an unset `dispersion_k` or `dispersion_epsilon` are not given; `dispersion_cl`
keeps its error. `build_tracker` with `"turbulence_model"` and no turbulence state returns a located error at
`particles.dispersion_fields` (not reachable after DE2).

**S3. The refresh (D2).** `rcfd-api`: a crate function that turns the turbulence state into `(k, epsilon)` (D2); a
`pub(crate)` accessor of the turbulence state on `Simulation`; `ParticleRun::refresh_dispersion(case, turb)` (nothing
unless `"turbulence_model"`), called by `after_step` before it tracks and by `Simulation::step` before `pre_step`;
`track_transient` refreshes before each tracking step. Read access `ParticleRun::tracker()`. Nothing changes with
`"constant"`.

**S4. Run path and written text.** No new output: the report and the particle files are those of ADR 0057 to ADR 0206.
The written text of a case with `[particles]` lists `dispersion_fields = "constant"` (every field of `particles` is
written); its settings hash does not change (R8).

## Registered checks

All tests run in release. Library checks in `crates/rcfd-particles/tests/dispersion_f255.rs` (dispersion) and
`crates/rcfd-particles/tests/reaction_f255.rs` (reaction); case-tree checks in `crates/rcfd-api/tests/dispersion_f255.rs`,
which holds the record tests R8 and R9 from the start. "Relative" is `abs(a - b) / abs(b)`.

Common data: `k = 1.5`, `epsilon = 0.225` (`sigma^2 = 1`, `T_L = C_L k/epsilon = 1` s with `C_L = 0.15`), carrier
`rho = 1.2`, `nu = 1.5e-5` (`mu = 1.8e-5`), parcels of density 1000 with Stokes drag, the analytic scheme and no gravity.
Tracer `d = 1.8e-5` m (`tau = rho_p d^2/(18 mu) = 1e-3` s); inertial `d = 5.692099788303082e-4` m (`tau = 1` s). The
statistic of a cloud of `N` parcels released at `x0` is `V = sum_i abs(x_i - x0)^2 / (3N)` against
`C = (1/N) sum_i f(age_i)` with `f` the closed form (each parcel at its own age).

**Case T** (`crates/rcfd-api/tests/dispersion_f255.rs`): `box_3d`, `nx = ny = nz = 5`, `lx = ly = lz = 400` (cells of 80
m), every patch `velocity = "fixed"` at rest; `physics.nu = 1.5e-5`, `rho = 1.2`; `turbulence.model = "k_epsilon"`,
`k_initial = 1.5`, `epsilon_initial = 0.225`; `solver.algorithm = "simple"`, `max_iterations = 2`, `dt = 0.01`;
`particles`: `enabled`, `density = 1000`, `drag = "stokes"`, `integration = "analytic"`, `gravity = false`, `dt = 0.05`,
`dispersion = "drw"`, `dispersion_fields = "turbulence_model"`, `dispersion_seed = 20261107`; one point injection `c` at
`(200, 200, 200)` (the centre of the middle cell), `rate = 1e8`, `end_time = 2e-4` (`N = 20 000`, released over 2e-4 s),
`diameter = 1.8e-5`. `particles.end_time` is the time of the measurement. Variants: **TS** = T with
`model = "k_omega_sst"`, `k_initial = 1.5`, `omega_initial = 1.6666666666666667` (`epsilon = 0.225` to round-off) and
`dispersion_seed = 20261108`; **TI** = T with `diameter = 5.692099788303082e-4`, `dispersion_crossing = false`,
`dispersion_seed = 20261109`; **T6** = T with `k_initial = 6.0`, `epsilon_initial = 3.6` (`sigma^2 = 4`, `T_L = 0.25`) and
`dispersion_seed = 20261110`; **TC** = T without `dispersion_fields` and with `dispersion_k = 1.5`,
`dispersion_epsilon = 0.225` (the constants of ADR 0057); **TC6** = TC with the model's `k_initial = 6.0`,
`epsilon_initial = 3.6`; **TW** = T with `rate = 1e7` (`N = 2000`), `interpolation = "cell_centre"`, `end_time = 2.0`;
**P** = TS with `algorithm = "pimple"`, `dt = 0.05`, `end_time = 0.5` in `[solver]` (no `max_iterations`), `rate = 1e7`,
`particles.end_time = 0.5`; **P2** = P with `particles.two_way = true`.

### A. The closed form (library)

| Id | Check | Limit |
| --- | --- | --- |
| CF1 | `drw_msd(1, 1, tau, t)` against the moment system of D3 integrated by RK4 from zero with `n = max(4000, ceil(400 t / min(tau, 1)))` steps, for every `tau` in {1e-3, 0.5, 1, 1 + 1e-7, 2, 10} and `t` in {0.25, 1, 4, 10} | 1e-9 relative each |
| CF2 | `drw_msd(1, 1, 1e-9, t)` against `taylor_msd(1, 1, t)` at the four times; `taylor_msd(1, 1, 50)` against `2 (50 - 1)` | 1e-7; 1e-14 |
| CF3 | long-time diffusivity: `(drw_msd(1, 1, tau, 101) - drw_msd(1, 1, tau, 100)) / 2` against `sigma^2 T = 1` for `tau` in {1e-3, 0.5, 2} | 1e-9 |

### B. Fields in the library

| Id | Check | Limit |
| --- | --- | --- |
| LF1 | refusals (each `Err`): `set_dispersion_fields` without `set_dispersion`; with `k` one value short; with `eps` one value short; with a NaN in `k`. Control: after `set_dispersion_fields(k2, e2)`, `dispersion_fields()` returns `k2` and `e2` | as stated; bit for bit |
| LF2 | no side effect: the scenario of ADR 0192 R1 (`wall_film.rs`, every extension; 150 steps of 0.01 s) with `set_dispersion_fields` called with the arrays it already holds before every `advance`: fingerprint against the run without the calls; with `2 k` from step 76 on: fingerprints equal after step 75 and differ after step 150 | bit for bit; as stated |
| LT (library Taylor) | the tracker on the mesh of T (`box_mesh([5, 5, 5], [400, 400, 400])`), `set_dispersion(Dispersion::default(), [1.5; 125], [0.225; 125], 20261111)`, the injection of T, `Flow {u: 0, grad: 0, rho: 1.2, mu: 1.8e-5}`: `V` against `C` with `f = drw_msd(1, T_L, 1e-3, .)` at 0.25, 1, 4 and 10 s (steps of 0.05 s); every parcel active | 5 percent each; all |

### C. Case tree

| Id | Check | Limit |
| --- | --- | --- |
| CT1 | located errors (`parse_toml`, `semantic_errors`): DE1 with T and `dispersion = "none"`; DE2 with T and `model = "laminar"`, and with T and `model = "spalart_allmaras"` (without the initial k and epsilon); DE3 with T and `dispersion_k = 1.5`, and with T and `dispersion_epsilon = 0.225`; an unknown value `dispersion_fields = "model"` at `particles.dispersion_fields`. Controls with no diagnostic: T, TS, TC, TC with `dispersion_fields = "constant"`, T with `particles.enabled = false` | each at its path; none |
| CT2 | the fields: (a) T: after `build_tracker`, `dispersion_fields()` equals `field("k")` and `field("epsilon")`; (b) T after two `sim.step()`: the same, and the fields are not uniform (largest over smallest `k` at least 1.05); (c) TS, before and after two steps: `epsilon` equals `BETA_STAR * k * omega` of `field("k")`, `field("omega")` cell by cell | bit for bit; 1.05; bit for bit |
| CT3 (acceptance) | T through `track_frozen` on the simulation before any sweep, `particles.end_time` = 0.25, 1, 4 and 10 s (one run each): `V` against `C` with `f = drw_msd(1, T_L, 1e-3, .)`, and against `C` with `f = taylor_msd(1, T_L, .)` (Taylor); every parcel active; `N` | 5 percent each; all; 20 000 |
| CT4 | TS as CT3: `V` against `C` (`drw_msd`, `T_L` and `sigma^2` from the tracker's `k` and `epsilon` of the cell of `x0`) and against Taylor | 5 percent each |
| CT5 | TI (inertial, `tau = T_L`, no crossing) as CT3: `V` against `C` with `f = drw_msd(1, T_L, 1, .)` | 5 percent each |
| CTN1 (negative control) | in CT5 at 1 s: `V` against Taylor's tracer form | `>= 50` percent off |
| CTN2 (negative control) | T6 at 1 s: `V` against `C` of its own fields (`sigma^2 = 4`, `T_L = 0.25`) and against `C` of the fields of T | 5 percent; `>= 50` percent off |
| CTN3 (negative control) | TC6 at 1 s (the constants 1.5 and 0.225, the model at 6 and 3.6): `V` against `C` of the constants | 5 percent |
| CT6 | tree equals library, non-uniform fields: TW through the run path (`runctl::drive`, two steady sweeps, then `particle_run::steady_run` on the final simulation), `track_frozen` on the same simulation, and a tracker built by hand (`Tracker::new` with the options of the case, `add_injector` with the spec and seed `build_tracker` derives, `set_dispersion(Dispersion {0.15, true}, field("k"), field("epsilon"), 20261107 + 0x5EED_D15B)`, `Flow {u: velocity, grad: None, rho: 1.2, mu: 1.2 * 1.5e-5}`, steps of 0.05 s to 2 s): fingerprints | bit for bit |
| CT7 | transient refresh: (a) P through `Simulation::step` (its `ParticleRun`), 10 steps: after every step the tracker's arrays equal `field("k")` and `BETA_STAR * k * omega` of that step, and `k` differs from the previous step's in some cell; (b) `track_transient` on a new simulation of P: its arrays equal the final fields, and its fingerprint equals that of (a) after 10 steps | bit for bit; as stated; bit for bit |
| CT8 | two-way refresh: P2 through `Simulation::step`, 10 steps: after every step the tracker's arrays equal the fields read before that step (the old time level) | bit for bit |

### D. The surface reaction (library)

Common data: the reaction `S + G_O -> G_P` with `M_s = 0.012`, `nu_s = 1`, gas `O` (`M = 0.032`, `nu = -1`, the
oxidiser) and `P` (`M = 0.044`, `nu = 1`), `f_r = 0.875`, parcels of density 1500. These molar masses are round test
inputs, not data for a real substance.

| Id | Check | Limit |
| --- | --- | --- |
| RX1 | closed form: `box_mesh([1, 1, 1], [1, 1, 1])`, carrier at rest (`rho = 1.2`, `mu = 1.8e-5`), no gravity, Stokes drag, analytic, one parcel released at rest at the centre (`rate = 1000`, `t_end = 1e-3`, so one parcel at 1e-3 s; `d0 = 1e-4`, `parcel_count = 1e6`), `Y_o = 0.23`, `k_s = 0.01` (`omega = 0.08625`, `dd/dt = -1.38e-6` m/s, `t_b = 36.23` s), steps of 0.1 s: after steps 100, 200 and 300, `d` against `d0 - 1.38e-6 age` (`age` the parcel's); after step 400, `reacting_mass == 0`, `d` against `d0 / 2` (`d_i`), `solid_reacted` against `n f_r m0` (`m0 = 1500 pi d0^3 / 6`), and the summed sources of `O` and `P` against `-0.032` and `0.044` times `extent` | 1e-12; exact, 1e-14, 1e-12, 1e-12 |
| RX2 (acceptance) | per-species conservation: the scenario of ADR 0192 R1 (`wall_film.rs`: mesh, options with Schiller-Naumann and RK4, wall actions, the injectors with the cone and the mass flow, the flow, its gradient and the dispersion) without the heat transfer and the exchange record, with constant diameters (`a`: `d = 1e-4`, density 1500; `b`: `d = 6e-5`, density 1400), `Y_o = 0.05 + 0.2 x_c / 1.2` per cell (`x_c` the cell centre), `k_s = 1.0`; 300 steps of 0.01 s. At the end of every step, with `dm_p = sum_i n_i (pi rho_i / 6)(d0_i^3 - d_i^3)` over all parcels (from the parcel list, `d0` by density) and `Xi_p = dm_p / (nu_s M_s)`: `solid_reacted` against `dm_p`; for `O` and `P` the summed sources against `nu M Xi_p`; the sum of the two against `dm_p` | 1e-10 relative each |
| RX3 | the RX2 run at the end: every `reacting_mass` `>= 0`; every `d` in `[d_i (1 - 1e-14), d0]`; at least 10 percent of the parcels burnt out (`reacting_mass == 0`) and 10 percent partly reacted; at least one parcel escaped and one deposited | as stated |
| RX4 | inert at zero oxidiser: RX2 with `Y_o = 0` in every cell, against RX2's set-up without `set_reaction`: the fingerprint of the run without the reaction against the same leading words of the run with it | bit for bit |
| RX5 | refresh: RX1 with `set_reaction_oxidiser(0.46)` after step 100: after steps 150 and 200, `d` against `d_100 - 2.76e-6 (age - age_100)` (`d_100`, `age_100` measured after step 100) | 1e-12 |
| RX6 | refusals (each `Err`): `set_reaction` with `rate = 0`, `reacting_fraction = 1`, `reacting_fraction = 0`, no gas species, a gas coefficient 0, a gas molar mass `-0.032`, the oxidiser index 2, the oxidiser `P` (positive coefficient), `M_P = 0.045` (unbalanced), `y_o` one value short, a value 1.5 in `y_o`; a second `set_reaction`; `set_reaction` after `set_thermal`; `set_thermal` after `set_reaction`; `advance` after `set_reaction` and `enable_coupling`; `set_reaction_oxidiser` without a reaction. Control (`Ok`): RX1's spec | as stated |

### E. Defaults

| Id | Check | Limit |
| --- | --- | --- |
| D1 | the existing suites, unedited: `rcfd-particles` `closed_forms`, `lagrangian_ext` (L1 to L13), `wall_film` (R1), `film_gas`, `wall_interaction` (R3), `erosion_presets`; `rcfd-api` `particles_case`, `particles_ext_case` (L14, L15), `particle_run`, `particles_two_way`, `wall_film_f253` (R2), `wall_interaction_f254` (R4), `erosion_presets_f254` (R5), `film_gas_f292` (R6, R7), H1 of `case_hashes_w48u`, `baseline_hashes` | pass (R1 to R6 compared on Windows) |
| D2 R8 | the settings content hash of TC, recorded on main d953cd6: `fcb39f5493969989`; after the code: TC, and TC with `dispersion_fields = "constant"` | equal |
| D3 R9 | FNV-1a 64 of the names and bytes of the particle output files of TCR (TC with `rate = 1e7`, `particles.end_time = 2.0`) through the run path (`runctl::drive` and `runctl::write_results`; the files `particle_report.json` and `particles.vtp`), recorded on main d953cd6 (Windows): `98977560c22d623c`; after the code: TCR, and TCR with `dispersion_fields = "constant"` | equal (on Windows; printed elsewhere, ADR 0137) |

The records were made by `RCFD_F255_RECORD=1 cargo test --release -p rcfd-api --test dispersion_f255` on main d953cd6
with the file as committed with this text (Windows, release), each test run once.

### Mutations (scratch, not committed)

Each is applied alone to the final tree by an exact text replacement, the named test is run once, and it must fail; the
file is restored and checked byte for byte against a kept copy. Their numbers go in the addendum. `scripts/limit_audit.py`
is run on the new test files and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `drw_msd` without the `I2` term | CF1 |
| MU2 | the SST conversion gives `epsilon = k omega` (no `beta*`) | CT4 |
| MU3 | `build_tracker` passes the constants with `"turbulence_model"` | CT3 |
| MU4 | `after_step` without the refresh | CT7 |
| MU5 | `Simulation::step` without the refresh before `pre_step` | CT8 |
| MU6 | `.hash_omit_default()` removed from `particles.dispersion_fields` | D2 |
| MU7 | the refusal DE3 removed | CT1 |
| MU8 | the reaction books every gas species with the oxidiser's molar mass | RX2 |
| MU9 | `d_lin` without the factor 2 | RX1 |
| MU10 | `track_transient` without the refresh | CT7 |

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a
`git archive` copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), in the target dir
`E:/projects/cfd/w49i-work/target-f255-gate`, taken in turn through the machine's gate lock and started with at least
20 GB of physical memory free. A failure of a known flaky test is run again alone on the same build and reported with its
name.

### Classes

D1 to D3 are preconditions of landing: a miss means the change is not merged and the cause is recorded. A miss of any other
limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A negative control
(CTN1 to CTN3) that does not hold, or a mutation that does not make its check fail, means the check is not shown to be able
to fail: it is reported as a defective set-up, corrected by an amendment before any re-run, and never counted as a pass.

## Predictions (mine)

| Id | Prediction |
| --- | --- |
| PR1 | CF1 at most 1e-10 (the Python model: 5.4e-11 at `tau = 10`, `t = 0.25`); CF2 about `2 tau / t`, 8e-9 at 0.25; CF3 below 1e-12 |
| PR2 | CT3, CT4, LT: the standard error of `V` is about `sqrt((kappa - 1)/(3N))` with the kurtosis `kappa` of `X` between 3 (ballistic, diffusive) and about 4, so 0.6 to 1.2 percent: errors of 0 to 3 percent; Taylor's form differs from `drw_msd` by 0.77, 0.17, 0.03 and 0.01 percent at the four times |
| PR3 | CT5 the same; CTN1 86 percent off (0.105 against 0.736 at 1 s); CTN2 about 105 percent off; CTN3 as CT3 |
| PR4 | CT2, CT6 to CT8 bit for bit; RX1 and RX5 at round-off (1e-15); RX2 below 1e-13 |
| PR5 | the library tests take seconds in release; CT3 to CTN3 about 0.7 s per 10 s of tracking |

## Not delivered

The case-tree keys of the surface reaction and the coupling of its sources to the carrier's species equations (Q8); a
temperature-dependent rate, heat of reaction, film diffusion, porosity or any published char or particle kinetics (Q7);
parcels without an inert residue (Q9); a reaction with heat transfer or the exchange record (S1); dispersion from the
Reynolds stress model, Spalart-Allmaras, the mixing length, the LES models or a plugin (Q3); an anisotropic random walk;
the model's `c_mu` in the eddy length (Q4); a check of dispersion in decaying turbulence against a closed form (Q5); the
finite hold of a zero eddy (Q6); a comparison with measured dispersion data.

## Open questions and choices for the maintainer

- **Q1. Status of F-255.** If every registered check is met, the acceptance is met on the branch with the reaction in the
  library only. Marking F-255 done is the maintainer's decision; the roadmap says partial until then.
- **Q2. The key.** Options: (a) `particles.dispersion_fields = "constant" | "turbulence_model"`, as registered; (b) a new
  value of `particles.dispersion` (for example `drw_model`); (c) read the model's fields whenever a two-equation model is
  on (would change existing results; not opt-in). Recommendation: (a), opt-in and orthogonal to the model choice.
- **Q3. The models.** Options: (a) k-epsilon and SST only, as registered (the set `chemistry` admits); (b) the Reynolds
  stress model with `k = tr(R)/2` and its `epsilon` in the isotropic walk; (c) an anisotropic walk with the stresses (a new
  item). Recommendation: (a) now, (c) as its own item when an anisotropic case needs it.
- **Q4. `C_mu` of the eddy length.** It stays 0.09 for every model (ADR 0057 D2), also for RNG (`c_mu` 0.0845) and a case
  that overrides `c_mu`. Options: (a) keep; (b) the model's `c_mu`. Recommendation: (a): `C_L` and `C_mu` of the walk are
  unverified constants (`docs/reference-data.md`, wave 29C) and (b) would tie them to a closure they were not taken from.
- **Q5. The homogeneous check.** It tracks on the model's state before the first sweep, the only homogeneous state a
  two-equation model holds without production. Options: (a) as registered, with the transient checks CT7 and CT8 for the
  refresh; (b) add a closed form for the walk in decaying homogeneous turbulence (SST on pimple decays homogeneously, item
  8), which needs a derivation for a non-stationary renewal process. Recommendation: (a); (b) only if a decaying-turbulence
  validation is wanted.
- **Q6. A zero eddy.** With `k <= 0` or `epsilon <= 0` in the cell of a draw, the walk holds a zero fluctuation for an
  infinite time, so the parcel never disperses again (ADR 0057 code). Model fields are positive in practice. Options: (a)
  keep; (b) hold it for one sub-step. Recommendation: (b) as a small item of its own (it changes library results where a
  cell has `k <= 0`).
- **Q7. The surface-reaction law.** Registered here: first order in the cell's oxidiser concentration, constant `k_s`, a
  shrinking particle at constant density with an inert residue (D4). Options: (a) keep as the base law; (b) an Arrhenius
  `k_s(T_p)` with the parcel temperature and the heat of reaction (needs heat transfer with the reaction); (c) a published
  kinetics and film-diffusion char model with constants from a source the maintainer chooses. Recommendation: (a) until a
  target case with data exists; (b) and (c) need data and sources not read here.
- **Q8. The reaction in the case tree.** Options: (a) keys under `particles.reaction` naming `species.<name>` entries, with
  the sources fed to the species equations (steady only today); (b) library only, as registered. Recommendation: (a) as
  its own item when a case needs it, with the molar masses taken from the chemistry mechanism's elements.
- **Q9. Parcels that burn out completely** (`f_r = 1`): refused, because a parcel of zero size has no state today.
  Options: (a) a new parcel state for a vanished parcel; (b) keep `f_r < 1`. Recommendation: (a) with Q8.
- **Q10. Molar masses.** Inputs of the caller, refused unless they balance to 1e-12. Options: (a) keep; (b) element
  compositions and a table of atomic weights (needs a source). Recommendation: (a) in the library; (b) with Q8.

## Amendment 1 (2026-11-07, written after the first run of RX2 and RX3 and before any corrected run)

The library checks of A, B and D ran once on the code before its commit (`cargo test --release -p rcfd-particles --test
dispersion_f255 --test reaction_f255`). RX2 and RX3 are one test (`rx2_rx3_species_conservation`). RX2 met its limits at
every one of the 300 steps (worst 1.0e-14), and RX3 met every condition but the last: of the 220 parcels, 160 burnt out and
60 partly reacted, every reacting mass was `>= 0` and every diameter in range, 60 escaped, but **none was deposited** (9
active, 151 stuck). The parcels of injector `a`, shrunk to the inert diameter, fall at about 0.1 m/s and are carried by the
flow onto the reflecting wall `xmax`, where the tracker stops them after 1000 reflections in one step (ADR 0028 A2), before
any reaches the sticking floor `zmin`. The condition "one deposited" was there to make the scenario cover the stick action
in the ledger; the scenario does not, so the set-up is defective for that condition. Nothing in the reaction or its ledger
is in question (the stuck parcels are in RX2's ledger).

Correction, before any further run of D:

- The test as run stays, unchanged but for its name and an `#[ignore]` with this reason, as the record
  `rx2_rx3_species_conservation_setup_defect`.
- New test `rx2b_rx3b_species_conservation_with_deposition`: the scenario of RX2 with a third injector `c`, point
  `(0.6, 0.5, 0.05)` (half a cell above the floor), 200 parcels/s from 0 to 0.4 s, released at rest
  (`VelMode::Given([0, 0, 0])`), `d = 1e-4`, density 1300, `parcel_count = 10`, seed 13. RX2b: the checks and limits of RX2
  at the end of every step (1e-10 relative each, `d0` by density: 1500 and 1300 give 1e-4, 1400 gives 6e-5). RX3b: the
  conditions of RX3 (every reacting mass `>= 0`; every `d` in `[d_i (1 - 1e-14), d0]`; at least 10 percent burnt out and
  10 percent partly reacted; at least one escaped and one deposited) at the end.
- Prediction: the parcels of `c` reach the floor within about 0.15 s, partly reacted, so RX3b holds; RX2b as RX2.

## Amendment 2 (2026-11-07, written after the first run of C, D1 to D3 and before any corrected run)

The code with the key as registered (S2: the field `dispersion_fields` of `particles`) and the checks of C, D2 and D3 ran
once in commit 8f02f50 (`cargo test --release -p rcfd-api --test dispersion_f255`): all 14 tests passed (the numbers are in
the addendum). The precondition D1 then **missed at H1** of `case_hashes_w48u.rs`: of its 41 recorded sources, the 30 valid
ones differ in the text columns `toml` and `json` (their settings and mesh columns are equal). Cause: every field of the
object `particles` is in the data of every case, so the new field is written in the text of every case (S4 said so),
and H1 compares the written text against its record R0 after removing only the keys of its own list `DEFAULT_KEYS`.
Adding the key to that list would edit `case_hashes_w48u.rs`, which the house rules of this work forbid. The other suites
of D1 passed (`baseline_hashes` included). S2 and S4 are defective as registered; nothing measured is in question.

Correction, before any corrected run:

- **S2 (amended).** The key is the optional table `particles.dispersion_from_model` with the one field `enabled`
  (boolean, default `false`), `.optional().hash_omit_default()` on the table and `.hash_omit_default()` on the field, as
  `particles.film`. The table is absent from the data unless a case writes it, so no written text changes. `enabled =
  true` is what `dispersion_fields = "turbulence_model"` was; an absent table or `enabled = false` is `"constant"`. The
  located errors: DE1 and DE2 at `particles.dispersion_from_model.enabled`, DE3 at `particles.dispersion_k` and
  `particles.dispersion_epsilon`, with the same conditions. D2 of the library code (S1, S3) is unchanged.
- **S4 (amended).** The written text of a case changes only when it writes the table.
- **Case texts.** In T and its variants, `[particles.dispersion_from_model]` with `enabled = true` takes the place of
  `dispersion_fields = "turbulence_model"`; nothing else changes (seeds, sizes, times, limits).
- **CT1 (amended inputs).** Input 6 (an unknown value) becomes `enabled = "yes"` (not a boolean), at
  `particles.dispersion_from_model.enabled`; the other inputs as registered with the new spelling. The control "TC with the
  default written" becomes TC with `[particles.dispersion_from_model]` and `enabled = false`.
- **D2 and D3 (amended texts).** After the code: TC, and TC with the table written with `enabled = false`, against R8;
  TCR, and TCR with the table written with `enabled = false`, against R9.
- **D1.** Unchanged, H1 included.
- **MU6 (amended).** `.hash_omit_default()` removed from the table `particles.dispersion_from_model` and from its field.
- The checks of C, D2 and D3 run once more with the corrected key; A, B and D are unaffected (library only) and are not
  re-run but in the gate. The first run's tests are in commit 8f02f50; their numbers stay in the addendum.
- Prediction: every check of C as in the first run, bit for bit (the key changes no arithmetic): the same numbers.
