# ADR 0206: F-292 registration: gas buoyancy and gas shear in the particle wall film

Status: **REGISTERED 2026-11-06**, written before any code of F-292 and before any run of its checks. Not yet accepted by
the maintainer. Before this text, two things were run on main 8db45fb, with the code of main as it is: the record tests of
checks D2 and D3 (R6, R7), which made the records below, and a scratch probe (not committed) that built the set-ups KB and
KS of the case-tree checks without the new keys, to see that `Simulation::new` accepts them and that the frozen Couette
field of KS reaches the cells as written (item 9 of the code read). The probe measured nothing that a registered check
measures; its first two runs stopped at the set-up of KS, which was corrected before this text (item 9). Results go in the
addendum at the end.

Parents: [ADR 0192](0192-f253-registration-particle-wall-film.md) (F-253, the film: its model M1 to M9, settings S1 to
S3, checks A to D, the records R1 and R2, its questions Q2 and Q3 and their decision record),
[ADR 0194](0194-f254-registration-particle-wall-interaction.md) (records R3 and R4), [ADR 0197](0197-f254-named-steel-preset-of-the-erosion-constants.md)
(record R5; a field added inside an optional particle table), [ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md)
(optional objects, the record R0 of `case_hashes_w48u.rs`), [ADR 0156](0156-job48v-adaptive-pressure-correctors.md) D6
(`hash_omit_default`), [ADR 0137](0137-linux-gate-platform-values.md) (platform values). Roadmap item F-292 (depends on
F-253).

Number: the lead gave 0206 to F-292. Main 8db45fb holds 0203 as its highest; 0198 is on the branch `f285-warped-kernel`,
0204 on `f291-gradient-options` and 0205 on `f295-seam-diffusivity`. No branch holds a 0206.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0192):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as
  an ignored record;
- every new code path is opt-in: a case without the new keys runs, hashes and writes its results as before, bit for bit;
- each new key uses `hash_omit_default`; neither `crates/rcfd-api/tests/baseline_hashes.rs` nor the record R0
  (`case_hashes_w48u.rs`) is edited or re-recorded, no golden is re-recorded, and no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Clean-room: both additions are derived below from the momentum balance of a thin laminar liquid layer, as ADR 0192 M1
derived the film. Status words as in ADR 0192: "derived" means derived here; "registered here" marks a choice of this text
that no source and no instruction fixes. No source was read for this work, and no vendor code, text or install was used.

## What F-292 asks

The roadmap entry: two opt-in additions to the film of F-253: gas buoyancy, the film driven by `(rho_l - rho_g) g_t` in
place of `rho_l g_t` (ADR 0192 Q2 (b)), and gas shear, a shear-driven closure checked against
`h = (2 mu_l Gamma / (rho_l tau_g))^(1/2)` for a film driven by shear alone (ADR 0192 Q3 (a)); each meets a closed form
derived in the ADR within limits fixed by its registration ADR, the parcel mass ledger of F-253 balances, and runs without
them are unchanged bit for bit (F-253's checks, the records R1 and R2 and the baseline hashes unchanged). The note: from the
maintainer's answers of 2026-11-06 to ADR 0192 Q2 and Q3; the film of F-253 stays without them.

So both additions are off unless a case or a caller switches them on, and the film of ADR 0192 is the film without them.
Splash and rebound at the film are F-284 and are not touched here.

## What the code read shows (main 8db45fb, nothing run but R6, R7 and the probe)

1. **The film.** `rcfd_particles::film`: `FilmSpec {patches, density, viscosity, gravity}` has four public fields and no
   constructor, and tests and `rcfd-api` build it by literal, so no field is added to it. `Film::new` keeps per face the
   area, centre, unit normal `n` (the unit area vector, out of the domain) and the tangential gravity
   `g_t = g - (g . n) n`. `Film::velocity(k)` is `film_velocity(rho_l, mu_l, h_k, g_t,k) = rho_l h_k^2 g_t,k / (3 mu_l)`;
   `Film::advance(dt)` recomputes every face velocity at the start of every sub-step, the outflow rates
   `r_f = sum_e L_e max(0, u_f . m_e) / A_f`, and the sub-step `min(rest, C_film / (3 max r_f))` (ADR 0192 M5, M6).
   `Film::csv` writes `face,patch,x,y,z,area,thickness,mass,ux,uy,uz`. `reynolds`, `max_reynolds`, `max_thickness` and the
   CSV use `velocity`.
2. **The tracker.** `Tracker::advance(flow, dt)` checks the size of `flow.u`, moves the parcels, makes the injections of
   the step, and then calls `film.advance(dt)`. `Flow {u, grad, rho, mu}` carries the carrier's cell velocities and its
   constant density and viscosity; the parcels' buoyancy uses `flow.rho` (`geff = g (1 - rho_f / rho_p)`,
   `advance_particle`). `Tracker::fingerprint` appends the face masses and the drained total when a film is set.
3. **The case tree.** `particles.film` is an optional object with `hash_omit_default` on the object and the fields
   `enabled`, `patches`, `viscosity`. `check_film` (called by `particles::check`, which `Case::semantic_errors` calls)
   gives the located errors of ADR 0192 S2. `build_tracker` builds `FilmSpec` with `density = particles.density` and the
   tracker's gravity. `flow_data` hands the tracker `rho = physics.rho`, `mu = physics.rho * physics.nu`, the cell
   velocities and their least-squares gradient, on the frozen path (`track_frozen`), the transient path
   (`track_transient`) and the run path (`ParticleRun::advance_step`, steady and transient).
4. **One density.** `particles::check` refuses particles with the variable-density and compressible solvers
   (`vd::enabled`, `comp::enabled`, ADR 0028), so wherever a film runs in the case tree the carrier density is the
   constant `physics.rho`.
5. **Wall shear elsewhere.** `rcfd_post::derived::wall_shear(geom, patch, u, u_wall, rho, nu)` computes, per face,
   `v = (u_P - u_wall) rho nu / dn` and removes its normal part, `v - (v . n) n`, with `P` the owner cell,
   `dn = abs(S . (x_f - x_P)) / abs(S)` (`FvGeometry::dn`) and `n = unit(S)` (`FvGeometry::normal`). The Laplacian's
   boundary coefficient of a fixed-value face is the same one-sided `abs(S) / dn` (`FvGeometry::lap_coeff`), so for a
   no-slip wall at rest this is the wall stress of the laminar momentum equation.
6. **The run path.** `ParticleRun::report` adds `counts.film`, `mass_by_state.film` and the object `film {faces, held,
   drained, absorbed, max_thickness, max_reynolds}` only when the film is on; `write_outputs` writes `particle_film.csv` as
   `Film::csv` gives it.
7. **Hashes and written text.** Once an optional object is in the data it holds every one of its fields, so a field added
   to `particles.film` without `hash_omit_default` would change the settings hash of every case that writes the table (as
   ADR 0197 read 4 found for `particles.erosion`). No case file, fixture or `docs/` TOML block writes `particles.film`
   (searched for `particles.film` in every `.toml`, `.json` and `.md` file: only ADRs, `docs/particles.md`,
   `docs/case-schema.md` and the roadmap mention it, in prose, in no TOML block), so H1 of `case_hashes_w48u.rs` reads no
   source that has the table.
8. **Boundaries.** A mesh patch without a `boundaries.<patch>` entry is an error of `Simulation::new` ("no boundary
   condition for mesh patch"). `boundaries.<patch>.velocity` defaults to `"fixed"` and `velocity_value` to `[0, 0, 0]`; a
   fixed condition can also take `velocity_expression_x/y/z` and `velocity_udf`; `moving_wall`, `shear`, `partial_slip` and
   `marangoni` are other wall conditions. Turbulence is `turbulence.model` (default `"laminar"`); moving meshes are
   `dynamic_mesh.enabled`; rotating frames are `sources.<name>.type = "rotating_frame"` or `"mrf"`.
9. **The probe (scratch, main 8db45fb).** Its first run stopped at the set-up of KS: an initial velocity expression
   without units is refused (`initial.velocity_expression_x`: "expression has dimension 'm' but 'm s^-1' is required"). Its
   second run, with `5555.555555555556 [m/s] * y / 1 [m]`, stopped at `initial.velocity`: "the prescribed flux is not
   divergence free" (1.042e-3 against a largest face flux of 1.042e-3), because the fixed walls `xmin` and `xmax` carried
   no flux where the field crosses them. With the same expression on `xmin` and `xmax` (KS below), KS without `gas_shear`
   and KB without `gas_buoyancy` pass `semantic_errors` and `Simulation::new`, and `track_frozen` runs them to their end
   times. In KS the owner cells of the ten `ymin` faces have the centre `y = 2.5e-3` (or the double below it), the
   velocity `(13.888888888888888, 0, 0)` or `(13.88888888888889, 0, 0)`, and `dn = 2.5e-3` within one unit in the last
   place; without the shear the film stays on the first face (held 9.99e-5 kg, nothing drained, largest thickness 4.0e-3 m
   at 40 s), as GSN below expects of the library.

## The model

**G1. Gas buoyancy (derived).** In ADR 0192 M1 the gas over the film is at rest and its pressure is left out. The gas at
rest has the hydrostatic pressure `p_g = p_0 + rho_g g . x`; at the free surface the film takes this pressure, so along the
wall it has the gradient `rho_g g_t` in addition to the liquid's own hydrostatic part (whose tangential gradient vanishes
where the thickness is uniform). The momentum balance along `g_t` becomes `mu_l u'' = -(rho_l - rho_g) abs(g_t)`, so

    u_bar = (rho_l - rho_g) abs(g_t) h^2 / (3 mu_l)
    Gamma = rho_l u_bar h = rho_l (rho_l - rho_g) abs(g_t) h^3 / (3 mu_l)
    h_B(Gamma) = (3 mu_l Gamma / (rho_l (rho_l - rho_g) abs(g_t)))^(1/3) = h_N(Gamma) (1 - rho_g/rho_l)^(-1/3)

The film's mass flux stays `rho_l h u_bar`: only the drive changes. With `rho_g = 0` it is the film of ADR 0192. The
factor is 1.000400 for water in air (1.2 kg/m^3) and 1.100642 for water under a dense gas of 250 kg/m^3. `rho_g` is the
carrier density of each tracking step (`Flow::rho`, the density the parcels' buoyancy uses); in the case tree that is
`physics.rho` (read 4). The dynamic part of the gas pressure gradient along the wall is not included.

**G2. Gas shear (derived).** The gas exerts the tangential stress `tau` on the free surface: `mu_l u'(h) = tau` in place of
`u'(h) = 0`. The balance is linear, so the profile is the sum of M1's and the Couette part `(tau / mu_l) y`:

    u_bar = D h^2 g_t / (3 mu_l) + h tau / (2 mu_l),   D = rho_l - rho_g with G1, rho_l without

For a film driven by shear alone (`g_t = 0`, or gravity normal to the wall), `Gamma = rho_l h^2 abs(tau) / (2 mu_l)` and

    h_S(Gamma) = (2 mu_l Gamma / (rho_l abs(tau)))^(1/2)

which is the roadmap's formula with `tau_g = abs(tau)`.

**G3. The two together, co-current (derived).** With `tau` along `g_t` (both pointing the same way down the wall),
`Gamma / rho_l = a h^3 + b h^2` with `a = D abs(g_t) / (3 mu_l)` and `b = abs(tau) / (2 mu_l)`. The cubic
`a h^3 + b h^2 - Gamma / rho_l` is negative at 0 and increasing for `h > 0`, so it has one positive root `h_C`. It is the
closed form of check GC1, found in the test by bisection and compared there with Cardano's root
`h_C = t - b/(3a)`, `t = cbrt(-r/2 + sqrt(r^2/4 + p^3/27)) + cbrt(-r/2 - sqrt(r^2/4 + p^3/27))`,
`p = -b^2/(3 a^2)`, `r = 2 b^3/(27 a^3) - Gamma/(rho_l a)`.

**G4. The shear the film takes (registered here).** For each film face `f` with owner cell `P`,

    tau_f = mu_g (u_P - (u_P . n_f) n_f) / d_f,   d_f = abs(S_f . (x_f - x_P)) / abs(S_f)

with `u_P` the carrier's cell velocity and `mu_g = Flow::mu`: the one-sided laminar wall stress of a no-slip wall at rest,
the stress of `rcfd_post::derived::wall_shear` and of the momentum equation's no-slip wall (read 5). It is exact for a
linear (Couette) profile next to the wall and first order otherwise. Because the film never acts on the carrier, the
carrier sees the bare wall, and `tau_f` is the stress at the wall, not at the moving film surface; the film surface
velocity (`D abs(g_t) h^2/(2 mu_l) + tau h / mu_l`, 1e-2 m/s in check GS5) is not subtracted from the gas velocity (Q5). The
shear is recomputed from the flow at every `Tracker::advance` and held over the step's sub-steps.

**G5. Numerics (registered here).** The donor-cell flux, the edge normals, the drain and the sub-step rule of ADR 0192 M4
to M6 are kept, with the velocity of G2 in place of M2's. Along an edge normal `m`, `u_bar . m` has the gravity part
`D h^2 g_t . m / (3 mu_l)` and the shear part `h tau . m / (2 mu_l)`, and the kinematic wave has the speed
`d(h u_bar . m)/dh`, 3 times the gravity part plus 2 times the shear part. Where neither part is negative along `m`
(co-current) this is at most `3 u_bar . m`, so the sub-step rule of M6
still bounds the wave's Courant number by `C_film` and the loss of a face in a sub-step by `C_film / 3 = 1/6`. The bound on
the loss holds for any velocity (it uses the outflow rate itself), so no face mass becomes negative in any case. Where the
two parts oppose (a counter-current film, gas blowing up the incline), the wave speed can exceed `3 u_bar . m` and can even
point against `u_bar`, and the donor-cell flux, upwinded by `u_bar`, is then not upwind for the wave. Such films are not
delivered and not checked (Q3); the library counts, over the run, the face sub-steps in which the two parts have a negative
dot product (`opposed`), and the run report gives that count.

**G6. Re_f.** `Re_f = 4 rho_l h abs(u_bar) / mu_l` (ADR 0192 M9) with the velocity of G2. The registered laminar range of
ADR 0192 is for the gravity film; no range was read for a sheared film (Q6), and the checks below run at `Re_f = 2`.

## Settings and code

**S1. Library (`rcfd-particles`, module `film`).**

- `FilmGas {buoyancy: bool, shear: bool}` (`Default`: both false, the film of ADR 0192).
- Closed forms `buoyant_thickness(rho_l, rho_g, mu_l, gamma, g_t)` (G1), `shear_film_velocity(mu_l, h, tau)` (G2) and
  `shear_thickness(rho_l, mu_l, gamma, tau)` (G2).
- `Film::set_gas(gas)`, `Film::gas()`; `Film::set_gas_state(rho_g, tau)`, which refuses a `rho_g` that is not finite or is
  negative, `rho_g >= rho_l` with the buoyancy on, and, with the shear on, a `tau` list without one vector per film face or
  with a value that is not finite; `Film::carrier_shear(u, mu_g)` (G4); read access `gas_density()`, `shear(k)`,
  `max_shear()`, `opposed()`.
- `Film::velocity(k)`: with `FilmGas::default()` the expression of ADR 0192 as it is; otherwise the sum of G2, with `D`
  from G1 when the buoyancy is on and the shear part when the shear is on.
- `Film::csv`: with the shear on, three more columns `tx,ty,tz` (the shear of the last step) after `uz`; otherwise the file
  of ADR 0192.
- `Tracker::set_film_gas(gas)`, refused when no film is set. With a film whose gas is not the default, `Tracker::advance`
  sets the gas state from the flow before it moves any parcel: `rho_g = flow.rho` and, with the shear on,
  `tau = carrier_shear(flow.u, flow.mu)`; a refusal of `set_gas_state` is returned by `advance` before anything moves.
- Unchanged: `FilmSpec`, `Particle`, `Flow`, `Options`, `InjectorSpec`, `Tracker::fingerprint` and the film's absorption.

**S2. Case tree.** Two fields of the optional object `particles.film`, each `.hash_omit_default()`:

| Key | Default | Meaning |
| --- | --- | --- |
| `particles.film.gas_buoyancy` | `false` | drive the film by `(rho_l - rho_g) g_t` with `rho_g = physics.rho` (G1) |
| `particles.film.gas_shear` | `false` | add the carrier's wall shear to the film velocity, `h tau / (2 mu_l)` (G2, G4) |

Located errors, checked without the mesh by `check_film` and only with `particles.film.enabled` (the keys are inert with the
film off):

- E1 at `particles.film.gas_buoyancy`: `gas_buoyancy` with `physics.rho` not below `particles.density` (when the density is
  set);
- E2 at `particles.film.gas_shear`: `gas_shear` with `turbulence.model` other than `"laminar"` (G4 is not the wall stress of
  a turbulence model's wall treatment);
- E3 at `particles.film.gas_shear`: `gas_shear` with `dynamic_mesh.enabled`;
- E4 at `particles.film.gas_shear`: `gas_shear` with a source of type `"rotating_frame"` or `"mrf"` (the wall velocity in
  the frame of the solution is not read here);
- E5 at `particles.film.gas_shear`: `gas_shear` with a film patch whose velocity condition is not a no-slip wall at rest:
  `velocity` other than `"fixed"`, a non-zero `velocity_value`, a non-empty `velocity_expression_x`, `_y` or `_z`, or a
  non-empty `velocity_udf` (a patch without a `boundaries` entry is left to `Simulation::new`, read 8).

Nothing else changes: the errors of ADR 0192 S2 stay as they are, including `particles.gravity` and `physics.gravity`
(Q4), and `build_tracker` calls `set_film_gas` only when one of the keys is true.

**S3. Run path.** Only when a gas option is on: the report's `film` object gains `gas {buoyancy, shear, gas_density,
max_shear, opposed}`, and with the shear on `particle_film.csv` has the columns of S1. Without them every output is that of
ADR 0192, byte for byte (R2, R6).

**S4. Written text.** The written text (`Case::to_toml`, `Case::to_json_string`) of a case that writes `[particles.film]`
lists the two new fields with their defaults, as ADR 0197 P10 lists `material = "none"`; its settings hash does not change
(R7). No H1 source writes the table (read 7).

## Registered checks

All tests run in release. Library checks in `crates/rcfd-particles/tests/film_gas.rs`; case-tree and run-path checks in
`crates/rcfd-api/tests/film_gas_f292.rs` (which holds the record tests R6 and R7 from the start). "Relative" is
`abs(a - b) / abs(b)`; for vectors, the largest component difference over the size of the reference vector.

Common data: `rho_l = 1000`, `mu_l = 1e-3`, `g = 9.81`, design flow `Gamma = 5e-4` kg/(m s) per unit width (`Re_f = 2`),
column width 5 mm, feed `S = Gamma x 5 mm = 2.5e-6` kg/s per column, steps of 0.01 s. Geometry G is that of ADR 0192 (the
box `box_mesh([10, 4, 2], [0.05, 0.02, 0.01])` rotated by 30 degrees about y, the film on `zmin`, rows `i = 0..9` down the
slope, the station row 6). **Geometry H** is the same box not rotated: `zmin` is a horizontal floor (normal `(0, 0, -1)`),
gravity `(0, 0, -9.81)` is normal to it, so `g_t` has no part in the plane of the face and the film is driven by the shear
alone; rows are along x. The shear of H is the frozen Couette field `u = (s z, 0, 0)` at the cell centres with
`s = 5555.555555555556` 1/s and `mu_g = 1.8e-5`, so `tau = mu_g s = 0.1` Pa; `h_S = 1e-4` m, `u_bar = 5e-3` m/s.

### A. Gas buoyancy (library)

`h_B = buoyant_thickness(1000, rho_g, 1e-3, 5e-4, 9.81 sin 30 deg)`: 7.415327e-5 m for `rho_g = 250`, 6.739969e-5 m for
`rho_g = 1.2`; `h_N = 6.737272e-5` m.

| Id | Check | Limit |
| --- | --- | --- |
| GB1 | closed forms: `Gamma = rho_l u_bar h_B` with `u_bar` of G1 (`film_velocity(rho_l - rho_g, ...)`), for `rho_g` = 250 and 1.2; `h_B / h_N = (1 - rho_g/rho_l)^(-1/3)`; `buoyant_thickness` with `rho_g = 0` equals `nusselt_thickness` | 1e-14; 1e-14; bit for bit |
| GB2 | residual, geometry G, buoyancy on, `set_gas_state(250, [])`: every face at `rho_l h_B A_f`, one `advance` of 0.01 s: rows 1 to 9 keep their mass; row 0 loses `Gamma 5 mm 0.01 s`; held + drained equals the initial mass; sub-steps | 1e-13; 1e-13; 1e-14; 1 |
| GB3 | approach, geometry G, `rho_g = 250`: an empty film, `S dt` added to each row-0 face at the start of every step, 2000 steps: rows 1 to 9 against `h_B` at 20 s; added = held + drained; smallest face mass; sub-steps | 1e-10; 1e-12; `>= 0`; 2000 |
| GB4 (acceptance) | the case B of ADR 0192 (four point injectors, `d = 1e-4`, from rest, 730 parcels/s, `mass_flow = 2.5e-6` each, 0 to 20 s, drag `none`, analytic, cell-centre interpolation, carrier at rest) with the carrier density 250 (`Flow {rho: 250, mu: 1.8e-5}`) and `set_film_gas(buoyancy)`: every station face against `h_B` as the average over `[18, 20]` s and at 20 s; the ledger of ADR 0192 B3 at the end of every step; deposited, escaped or stuck parcels at the end; smallest face mass; largest face `Re_f` at 20 s | 5 percent each; 1e-10; 0; `>= 0`; `<= 30` |
| GB5 | consistency: in the run of GB4, the window average of every station face against `h_B`, and absorbed against held + drained at the end of every step; the same run with the carrier of air (`rho = 1.2`): the window average against `h_B(1.2)` | 1e-6; 1e-10; 1e-6 |
| GBN (negative control) | the run of GB4 without `set_film_gas`: the window average of every station face against `h_N`, and against `h_B` | `<= 1e-6`; `>= 0.05` |

GBN shows that GB4 can fail: without the buoyancy the station sits 9.1 percent from `h_B`. With air the effect (4.0e-4) is
seen by GB5 only.

### B. Gas shear (library)

| Id | Check | Limit |
| --- | --- | --- |
| GS1 | closed forms: `Gamma = rho_l h_S abs(shear_film_velocity(mu_l, h_S, tau))` with `h_S = shear_thickness(rho_l, mu_l, Gamma, 0.1)` | 1e-14 |
| GS2 | the carrier shear: (a) geometry H, the Couette field of H: `carrier_shear` of every `zmin` face equals `(0.1, 0, 0)`; (b) the same field plus `(0, 0, 0.3)` m/s in every cell (a part normal to the wall): the same `tau`; (c) geometry G with the Couette field along the slope, `u = s z_l t` (`z_l` the local height above `zmin`, `t` the unit vector down the slope): `tau = 0.1 t` on every face | relative 1e-14 each |
| GS3 | residual, geometry H, shear on, `set_gas_state(1.2, tau)` with `tau = (0.1, 0, 0)` on every face: every face at `rho_l h_S A_f`, one `advance` of 0.01 s: rows 1 to 9 keep their mass; row 0 loses `Gamma 5 mm 0.01 s`; held + drained equals the initial mass; sub-steps | 1e-13; 1e-13; 1e-14; 1 |
| GS4 | approach, geometry H, `tau` as GS3: an empty film fed into row 0 as GB3, 4000 steps: rows 1 to 9 against `h_S` at 40 s; added = held + drained; smallest face mass; sub-steps | 1e-10; 1e-12; `>= 0`; 4000 |
| GS5 (acceptance) | geometry H through the tracker with `set_film_gas(shear)` on the frozen Couette field of H (`Flow {rho: 1.2, mu: 1.8e-5}`, gradient none, cell-centre interpolation), drag `none`, analytic, gravity `(0, 0, -9.81)`; four point injectors at `(2.5e-3, (j + 1/2) 5e-3, 2e-3)`, from rest, 730/s, `d = 1e-4`, density 1000, `mass_flow = 2.5e-6`, 0 to 40 s; 4000 steps; the window `[38, 40]` s (20 feed periods): every station face (row 6) against `h_S` as the window average and at 40 s; the ledger at the end of every step; parcels deposited, escaped or stuck; smallest face mass; largest `Re_f` at 40 s; `opposed` | 5 percent each; 1e-10; 0; `>= 0`; `<= 30`; 0 |
| GS6 | consistency: in the run of GS5, the window average against `h_S`; absorbed against held + drained at the end of every step | 1e-6; 1e-10 |
| GSN (negative control) | the run of GS5 without `set_film_gas`: the mass of every station face at 40 s (the floor has no in-plane gravity, so the film stays on row 0) | `== 0` |

### C. Both, and the tracker (library)

| Id | Check | Limit |
| --- | --- | --- |
| GC1 | co-current residual, geometry G, both on, `set_gas_state(1.2, tau)` with `tau = 0.05 t` on every face (`t` down the slope): the bisection root `h_C` of G3 against Cardano's; every face at `rho_l h_C A_f`, one `advance` of 0.01 s: row 0 loses `Gamma 5 mm 0.01 s`; rows 1 to 9 keep their mass; `opposed` | 1e-12; 1e-12; 1e-13; 0 |
| GC2 | counter-current count: GC1 with `tau = -0.05 t`: `opposed` after one `advance` | 40 times the sub-steps |
| GT1 | the tracker passes the gas state: geometry H, both on, the Couette field, one `advance`: `gas_density()` equals `flow.rho` and every `shear(k)` equals `carrier_shear(flow.u, flow.mu)[k]` | bit for bit |
| GT2 | refusals (each `Err`): `set_film_gas` on a tracker without a film; `advance` with buoyancy on and `flow.rho = 1000` (= `rho_l`); `set_gas_state` with a NaN `rho_g`, with `rho_g = -1`, with the shear on and a `tau` list one short, with the shear on and a NaN in `tau`. Controls (each `Ok`): `rho_g = 999.9` with buoyancy; `rho_g = 1000` with only the shear on | as stated |
| GD1 | the run of ADR 0192 B (carrier density 1.2) with `set_film_gas(FilmGas::default())` against the run without the call: fingerprints | bit for bit |

### D. Case tree and run path

Texts: **K** is case K of ADR 0192 (amendment 1, as in `film_gas_f292.rs`). **KB** is K with `physics.rho = 250.0` and
`particles.film.gas_buoyancy = true`. **KS** is the floor case: `box_2d`, `nx = 10`, `ny = 2`, `lx = 0.05`, `ly = 0.01`
(depth 5 mm); `physics.gravity = [0, -9.81, 0]` (normal to `ymin`); `rho = 1.2`, `nu = 1.5e-5`; `energy.enabled` and
`initial.temperature = 300` (as K); `solver` as K (simple, `frozen_flow`, 2 iterations); the frozen Couette field
`initial.velocity_expression_x = "5555.555555555556 [m/s] * y / 1 [m]"`, with every patch `velocity = "fixed"` and the
same expression on `xmin` and `xmax` (where the field enters and leaves; item 9), `ymin` and `ymax` at rest; the film on
`ymin` with `viscosity = 1e-3` and `gas_shear = true`; `particles.end_time = 40`, `particles.dt = 0.01`, `density = 1000`,
drag `none`, analytic; one point injection at `(2.5e-3, 2e-3, 2.5e-3)`, rate 730, `d = 1e-4`, `mass_flow = 2.5e-6`, to
40 s. Its `tau` is `mu_g u_P / dn = 1.8e-5 x 13.9 / 2.5e-3 = 0.1` Pa on every `ymin` face, as in geometry H.

| Id | Check | Limit |
| --- | --- | --- |
| CT1 | located errors (`parse_toml`, `semantic_errors`, `Simulation::new`): E1 with KB at `physics.rho = 1000.0` and at `1200.0`; E2 with KS and `turbulence.model = "k_epsilon"`; E3 with KS and `[dynamic_mesh] enabled = true`; E4 with KS and a source `[sources.rot] type = "rotating_frame"`; E5 with KS and on `ymin` each of `velocity = "moving_wall"`, `velocity_value = [0.1, 0.0, 0.0]`, `velocity_expression_x = "0.1"`, `velocity_udf = "w"`. Controls with no diagnostic: K, KB, KS, K with both keys true, K with both keys written false, and KS with `particles.film.enabled = false` (the key `gas_shear = true` inert) | each at its path; none |
| CT2 | tree equals library: for KB and for KS, `track_frozen`, the run path's `steady_run` and a tracker built by hand from the same inputs (with `set_film_gas`, and the flow of the simulation's cell velocities, `rho = physics.rho`, `mu = rho nu`): fingerprints, face masses, drained | bit for bit |
| CT3 (acceptance in the tree) | the row-6 face (x centre 32.5 mm): KB against `h_B(250)` at 20 s; KS against `h_S` at 40 s | 5 percent, and 1e-6, each |
| CT4 | the shear of the tree: in KS after `track_frozen`, every `film.shear(k)` against `rcfd_post::derived::wall_shear(geom, "ymin", u, u_wall, 1.2, 1.5e-5)` (`u_wall` the boundary values of the simulation's velocity field), relative to the largest `abs(tau)`; and against `(0.1, 0, 0)` | 1e-15; 1e-12 |
| CT5 | outputs: KB and KS through the run path: the report's `film.gas` equals the tracker's values (`buoyancy`, `shear`, `gas_density`, `max_shear`, `opposed`); KS's `particle_film.csv` has the header `face,patch,x,y,z,area,thickness,mass,ux,uy,uz,tx,ty,tz` and one row per film face whose last three columns read back the tracker's shear (as `f64`, equal); KB's has the header of ADR 0192 | equal; as stated |

### E. Defaults

| Id | Check | Limit |
| --- | --- | --- |
| D1 | the existing suites, unedited: `wall_film` (A, B, R1), `wall_film_f253` (C1 to C4, D4, R2), `wall_interaction` (R3), `wall_interaction_f254` (R4), `erosion_presets` and `erosion_presets_f254` (R5), the particle suites `closed_forms`, `lagrangian_ext`, `particles_case`, `particles_ext_case`, `particle_run`, `particles_two_way`, H1 of `case_hashes_w48u`, `baseline_hashes` | pass (R1 to R4 compared on Windows) |
| D2 R6 | FNV-1a 64 of the names and bytes of the particle output files of K through the run path (`particle_film.csv`, `particle_report.json`, `particles.vtp`), recorded on main 8db45fb (Windows): `2a6c2d99f94d77dc`; after the code, K and K with both keys written false | equal to R6 (on Windows; printed elsewhere, ADR 0137) |
| D3 R7 | the settings content hash of K, recorded on main 8db45fb: `95175a46b9941edc`; after the code: K; K with both keys written false; K with `gas_shear = true`; K with `gas_buoyancy = true` | equal, equal, differs, differs |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named test is run once, and it must fail; the file is restored and checked
byte for byte against a kept copy. Their numbers go in the addendum. `scripts/limit_audit.py` is run on the two new test
files and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `Film::velocity` drives by `rho_l` with the buoyancy on (`rho_g` ignored) | GB4 |
| MU2 | `shear_film_velocity` divides by `3 mu_l` in place of `2 mu_l` | GS5 |
| MU3 | `carrier_shear` keeps the normal part of `u_P` | GS2 |
| MU4 | `Tracker::advance` passes `0.0` in place of `flow.rho` to `set_gas_state` | CT3 (KB) |
| MU5 | `.hash_omit_default()` removed from `particles.film.gas_shear` | D3 |
| MU6 | the refusal E5 removed | CT1 |
| MU7 | the report writes `film.gas` with the gas options off | D2 |

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a
`git archive` copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), in the target dir
`E:/projects/cfd/w49i-work/target-f292-gate`, taken in turn through the machine's gate lock and started with at least
20 GB of physical memory free. A failure of a known flaky test is run again alone on the same build and reported with its
name.

### Classes

D1 to D3 are preconditions of landing: a miss means the change is not merged and the cause is recorded. A miss of any other
limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A negative control (GBN,
GSN) that does not hold, or a mutation that does not make its check fail, means the check is not shown to be able to fail:
it is reported as a defective set-up, corrected by an amendment before any re-run, and never counted as a pass.

## Predictions (mine; from a Python model of the scheme on one column, not in the repository)

| Id | Prediction |
| --- | --- |
| PR1 | GB3: 5.1e-15 at 20 s (3.5e-14 at 15 s). GS4: 5.4e-16 at 30 s (4.6e-12 at 25 s), so 40 s leaves a margin |
| PR2 | GB4: window average 2.2e-15, at 20 s 1.4e-11; GB5 with air: window average 6.0e-16; GBN: 2.8e-15 from `h_N` and 0.0914 from `h_B` |
| PR3 | GS5: window average 2.8e-14, at 40 s 1.4e-13 |
| PR4 | CT2 and CT4 bit for bit (CT4: the same arithmetic as `wall_shear`); CT3 as GB4 and GS5 at their end times |
| PR5 | The library tests take about a second each in release, the case-tree tests under a second each |

## Not delivered

Counter-current films (G5, Q3); the dynamic part of the gas pressure gradient along the wall; a turbulent or wall-function
shear (E2, Q2); the shear on a moving wall, on a moving mesh or in a rotating frame (E3 to E5); the film surface velocity
in the shear (Q5); a gravity-free film driven by shear alone in the case tree (Q4); a laminar range for sheared films (Q6);
everything ADR 0192 does not deliver (surface tension, inertia and waves, splash, rebound and breakup at impact (F-284),
separation and dripping, evaporation, any effect of the film on the carrier, the film with `two_way` or `heat_transfer`,
the film state in a restart); a comparison with measured data.

## Open questions and choices for the maintainer

- **Q1. Status of F-292.** If every registered check is met, the acceptance is met on the branch. Marking F-292 done is
  the maintainer's decision; the roadmap says partial until then.
- **Q2. Where the gas shear comes from.** Options: (a) the carrier's one-sided laminar wall stress at each film face (G4),
  as registered; (b) a prescribed shear stress, a vector key; (c) a wall-function stress for turbulent carriers.
  Recommendation: (a), which uses the carrier the parcels already see and needs no new input; (c) as its own item when a
  turbulent case needs it (E2 refuses turbulence until then).
- **Q3. Counter-current films.** Options: (a) count and report them, as registered (`opposed`); (b) refuse a run in which
  any face's parts oppose; (c) a new item with a flux that follows the kinematic wave (a Godunov-type flux) and its own
  checks. Recommendation: (a) now, (c) when a counter-current case is wanted.
- **Q4. Gravity with the shear on.** ADR 0192 S2 refuses a film without gravity ("driven by gravity alone"), and that stays.
  Options: (a) keep it; (b) lift it when `gas_shear` is on. Recommendation: (a): every film case has gravity, and a
  shear-alone film runs on a floor with gravity normal to it, as KS does.
- **Q5. The film surface velocity.** G4 takes the stress of the bare wall. Options: (a) as registered (one-way); (b) the
  stress of the gas velocity relative to the film surface, which needs an iteration per step. Recommendation: (a) while the
  film never acts on the carrier; the surface speed is small next to the gas speed in the intended cases (1e-2 against
  13.9 m/s at the cell centre in KS).
- **Q6. The laminar range of a sheared film.** No source was read. Recommendation: read one before relying on sheared films
  above the `Re_f = 2` of the checks; until then the report gives the largest face `Re_f`, as ADR 0192 does.

## Addendum 1: results (2026-11-06)

Measured against the limits registered above; no limit was changed. Release builds on Windows. The registration is
9d994dd (with the records R6 and R7), the code, tests and docs e698ca5, and c96fa5c splits one assertion of CT3 for clippy
(below). Each registered check ran once, the library checks on the code of e698ca5 before its commit and the case-tree
checks on e698ca5; the runs are deterministic and none was repeated to change a result. Every check met its limit, both
negative controls held, and every mutation made its check fail.

### Library (`crates/rcfd-particles/tests/film_gas.rs`, 18 tests, 9.7 s on one thread)

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| GB1 `Gamma` round trip (250 and 1.2); `h_B / h_N`; `rho_g = 0` | 1e-14; 1e-14; bit for bit | 2.2e-16; 4.4e-16 (1.100642 and 1.000400); equal | met |
| GB2 rows 1 to 9; loss of row 0; held + drained; sub-steps | 1e-13; 1e-13; 1e-14; 1 | 0; 1.2e-15; 1.8e-16; 1 | met |
| GB3 rows 1 to 9 at 20 s (worst: row 9); ledger; smallest mass; sub-steps | 1e-10; 1e-12; `>= 0`; 2000 | 1.4e-14; 1.8e-13; 0; 2000 | met (predicted 5.1e-15) |
| GB4 (acceptance) the four station faces, window average and at 20 s; ledger; lost parcels; smallest mass; largest `Re_f` | 5 percent; 1e-10; 0; `>= 0`; `<= 30` | 3.3e-15 and 1.4e-11 on each face; 6.1e-13; 0 (58 400 injected, 58 324 absorbed, 76 in flight); 0; 2.0000 | met (predicted 2.2e-15, 1.4e-11) |
| GB5 window average (250); absorbed against held + drained; window average with air | 1e-6; 1e-10; 1e-6 | 3.3e-15; 6.1e-13; 1.0e-15 (`h_B(1.2)` is 4.0e-4 above `h_N`) | met (predicted 6.0e-16 with air) |
| GBN (negative control) window average against `h_N`; against `h_B` | `<= 1e-6`; `>= 0.05` | 2.6e-15; 0.0914 | held (predicted 2.8e-15, 0.0914) |
| GS1 `Gamma` round trip | 1e-14 | 2.2e-16 (`h_S = 1e-4` m, `u_bar = 5.0e-3` m/s) | met |
| GS2 carrier shear: (a) floor; (b) with a normal part; (c) incline | 1e-14 each | 1.4e-16; 1.4e-16; 2.8e-15 | met |
| GS3 rows 1 to 9; loss of row 0; held + drained; sub-steps | 1e-13; 1e-13; 1e-14; 1 | 0; 7.3e-15; 1.4e-16; 1 | met |
| GS4 rows 1 to 9 at 40 s (worst: row 9); ledger; smallest mass; sub-steps | 1e-10; 1e-12; `>= 0`; 4000 | 3.8e-14; 8.5e-14; 0; 4000 | met (predicted 5.4e-16 at 30 s) |
| GS5 (acceptance) the four station faces, window average and at 40 s; ledger; lost parcels; smallest mass; largest `Re_f`; `opposed` | 5 percent; 1e-10; 0; `>= 0`; `<= 30`; 0 | 2.7e-14 and 1.3e-13 on each face; 1.8e-12; 0 (116 800 injected, 116 740 absorbed); 0; 2.0000; 0 | met (predicted 2.8e-14, 1.4e-13) |
| GS6 window average; absorbed against held + drained | 1e-6; 1e-10 | 2.7e-14; 1.8e-12 | met |
| GSN (negative control) station masses at 40 s | `== 0` | 0 on the four faces (116 740 parcels absorbed, all on row 0) | held |
| GC1 Cardano against bisection; loss of row 0; rows 1 to 9; `opposed` | 1e-12; 1e-12; 1e-13; 0 | 1.6e-13 (`h_C = 6.266368e-5` m); 1.2e-15; 0; 0 | met |
| GC2 `opposed` after one `advance` | 40 times the sub-steps | 40 (one sub-step) | met |
| GT1 gas density and shear passed by the tracker | bit for bit | equal | met |
| GT2 six refusals; two controls | `Err`; `Ok` | all six `Err`; both `Ok` | met |
| GD1 explicit default gas against no call | bit for bit | equal (759 243 words) | met |

### Case tree and run path (`crates/rcfd-api/tests/film_gas_f292.rs`, 9 tests, 3.2 s)

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| CT1 nine invalid inputs; six controls | each at its path; none | each at its path; none | met |
| CT2 KB and KS: `track_frozen`, `steady_run`, by hand | bit for bit | equal (KB 189 813 words, KS 379 613 words) | met |
| CT3 (acceptance in the tree) row-6 face: KB against `h_B(250)` at 20 s; KS against `h_S` at 40 s | 5 percent and 1e-6 each | 1.4e-11; 1.3e-13 | met |
| CT4 shear against `wall_shear`; against `(0.1, 0, 0)` | 1e-15; 1e-12 | 0 (bit for bit); 1.4e-16 | met (predicted bit for bit) |
| CT5 report `film.gas`; CSV headers and shear columns | equal; as stated | KB `{buoyancy: true, shear: false, gas_density: 250, max_shear: 0, opposed: 0}` and the header of ADR 0192; KS `{buoyancy: false, shear: true, gas_density: 1.2, max_shear: 0.1, opposed: 0}`, the 14-column header and the shear columns equal; 10 rows each | met |

### Defaults

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| D1 the suites listed | pass unedited | `wall_film` (R1 `e56421d672c5454f`), `wall_film_f253` (R2 `ce9df10b3a885c96`), `wall_interaction` (R3 `fe94f34ac9fc5dbd`), `wall_interaction_f254` (R4 `5514d6332741d3a4`), `erosion_presets`, `erosion_presets_f254` (R5), `closed_forms`, `lagrangian_ext`, `particles_case`, `particles_ext_case`, `particle_run`, `particles_two_way`, H1 (41 recorded sources, 0 differ), `baseline_hashes`: all pass, none edited | met |
| D2 R6: K; K with both keys false | `2a6c2d99f94d77dc` | equal; equal | met |
| D3 R7: K; both false; `gas_shear = true`; `gas_buoyancy = true` | equal, equal, differs, differs | `95175a46b9941edc`; `95175a46b9941edc`; `e2887d4c2c25776d`; `5b3cb43a25d5113a` | met |

`rules_w19c` and `w48v_adaptive_correctors`, whose census tests read every case text in the test sources (K and KS are new
there), pass as well.

### Mutations

Each was applied alone to c96fa5c by an exact text replacement, the named test was run once, and the file was restored and
compared byte for byte with the kept copy (equal in all seven).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 | GB4 | failed: the station 0.0914 from `h_B` (the Nusselt thickness) |
| MU2 | GS5 | failed: the station 0.2247 from `h_S` (`sqrt(3/2) - 1`) |
| MU3 | GS2 | failed at (b): 2.16e-2 with the normal part kept |
| MU4 | CT3 (KB) | failed: the row-6 face 0.0914 from `h_B(250)` |
| MU5 | D3 | failed: the hash of K became `f20e4382aac31594` |
| MU6 | CT1 | failed at input 5 (`moving_wall` on `ymin`): no diagnostic |
| MU7 | D2 | failed: R6 became `9bffcf7ee9383ba4` |

`scripts/limit_audit.py` on the two new files has one hit, `film_gas_f292.rs` (iv), the early `return` of the record test
D3 when `RCFD_F292_RECORD=1` is set (the record switch of ADR 0192's record tests); read: without the variable the test
asserts.

Disclosures:

- c96fa5c changes CT3 after its run: clippy refused `e <= 0.05 && e <= 1e-6` (the first bound is implied by the second), so
  the two registered limits are now asserted one after the other. Nothing else in the test changed.
- GB3 and GS4 end above their predictions (1.4e-14 against 5.1e-15, 3.8e-14 against 5.4e-16): the row-9 values sit at the
  round-off floor of the Rust arithmetic, as ADR 0192 A3 did (1.3e-14 against 5e-15); both are four orders below the limit.
- CT1: inputs 2, 3, 7 and 8 also give diagnostics of their own (k-epsilon with the energy equation and without an initial
  k; a moving mesh with simple and the energy equation; the expression `0.1` without units; the unknown user function
  `w`); the registered diagnostic is present in each.
- KS carries the Couette field through `xmin` and `xmax`, which the tracker treats as reflecting walls; no parcel reaches
  them.
- Not exercised by a test: the transient run path with a gas drive (`ParticleRun::after_step` and `track_transient` call the
  same `Tracker::advance`), the service method `particles.report` with a gas drive, the JSON Schema export of the two keys,
  the written text of S4 (from the code read), the shear on a non-orthogonal face (G4 is first order there), and a film
  over more than one patch with the shear.

### The gate (2026-11-07)

Main stayed at 8db45fb while F-292 was built, so no merge was needed; 0206 is on no other branch. The gate ran on a
`git archive` copy of 0ebe292 (this addendum's commit before this section) with three jobs and one process per test
(`scripts/gate_test.py`), in the target dir `E:/projects/cfd/w49i-work/target-f292-gate`. It took the machine's gate lock
at 23:36 when the F-285 gate released it (the F-291 and F-295 gates were waiting for the lock too; the lock takes no turns
by arrival) and started with 30 GB of physical memory free: `cargo fmt` per package 0, `cargo clippy --workspace
--all-targets -- -D warnings` 0, tests **2989 passed, 1 failed, 835 ignored** (2990 test processes in 580 binaries, wall
4058 s, peak summed working set 3906 MB, largest time against its limit 0.222, lowest free physical memory 22 GB). The lock
was released at the end.

The one failure is `rcfd-runner` `runner_w47b` s1, a known flake of the gate: its negative control ("the sampler does not see
an over-subscription of 2") saw status 3 and OS 0 under `max_runs = 3`, while its registered part held (status 2, OS 2).
Run again alone on the gate's sources it failed the same way (status 3, OS 0), as it did three times in the F-289 / F-290
gate; the runner tests do not run the particle code. All 27 tests of this ADR ran in the gate and passed, and so did the
suites of D1 (R1 to R5, H1, `baseline_hashes`). F-292 adds no ignored test.

Q1 to Q6 stay open for the maintainer. F-292 stays partial (Q1).
