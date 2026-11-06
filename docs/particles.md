# Lagrangian particle tracking (F-056, ADR 0028)

rcfd tracks point particles through a computed flow with one-way coupling. The particles never act on the flow, so existing flow
results and the baseline hashes are unchanged. Status: `partial`, see "Not delivered".

This page explains the force model, the time integration, how a particle is followed from cell to cell, what happens at
boundaries, and what is not modelled. It is for users who add a particles section to a case or call the tracker from code. For the
exact keys and defaults see the "particles" section of [case-schema.md](case-schema.md); for the full decision record and the
measured checks read [ADR 0028](adr/0028-wave18c-particles.md) and its addendum. The [glossary](glossary.md) explains ids such as
F-056 and wave 21C.

The new crate `rcfd-particles` (layer 2: depends on `rcfd-core` and `rcfd-mesh` only) implements the tracking, and the
settings-tree section `particles` (`rcfd-api::particles`) configures it.

## Not delivered (F-056 stays partial; F-128 done)

rcfd does not deliver:

* a cyclone separation case (the roadmap acceptance names settling and cyclone sanity cases; only closed-form settling-type checks
  exist)
* droplet / combusting / multicomponent particle types
* group / file injections
* feedback of the two-way sources inside `Simulation::step` (the exchange is a library record only)
* dispersion from the turbulence-model k and epsilon fields (constants only)
* turbulent dispersion of the heat transfer
* evaporation and mass transfer
* breakup
* in the wall film (F-253, see "Wall film" below): gas shear, splash and rebound at impact, film separation and
  dripping, evaporation and heat transfer of the film, any effect of the film on the flow, the film with two-way coupling
  or heat transfer
* in the wall interaction (F-254, see "Wall interaction" below): erosion correlations other than DNV RP O501, the brittle
  angle function, erosion that changes the wall shape, friction and particle rotation at the wall, roughness out of the
  plane of incidence or depending on the particle size, the shadow-effect distribution of Sommerfeld and Huber
* periodic patches
* tracking through unjoined non-conformal pairs
* gradient limiting in the interpolation
* checkpointing of the tracker state
* compressible and variable-density flows
* a time-dependent sequence of VTK files tied to the flow time (only every n tracking steps)
* a pure-walk guarantee on non-convex domains (see ADR 0028 results)

## Model

For a sphere of diameter `d` and density `rho_p` in a fluid of density `rho_f` and viscosity `mu`, drag and gravity set the motion:

    dv/dt = f(Re) (u - v)/tau_p + (1 - rho_f/rho_p) g,   dx/dt = v
    tau_p = rho_p d^2/(18 mu),   Re = rho_f d |u - v|/mu,   f = C_D Re/24

Gravity is applied with the buoyancy factor `1 - rho_f/rho_p`. The key `particles.drag` selects the drag coefficient `C_D`:

| `particles.drag` | `C_D` | status of the formula |
| --- | --- | --- |
| `stokes` | `24/Re` | derived |
| `schiller_naumann` | `24/Re (1 + 0.15 Re^0.687)` for `Re <= 1000`, `0.44` above | coefficients verified in two secondary sources; the validity limit is quoted as 800 or 1000 (switch at 1000 UNVERIFIED); original 1933 paper not accessed |
| `none` | 0 | ballistic |

Not modelled: added mass, pressure-gradient force, Basset history, lift, Brownian motion, turbulent dispersion, rotation,
particle-particle interaction, feedback on the fluid.

## Integration

The schemes differ in how they treat the fluid velocity:

* `analytic` freezes the fluid velocity and drag factor over a sub-step. The velocity `v` relaxes exponentially to
  `u + tau g_eff`, and the position follows from the exact integral. The scheme is exact for Stokes drag in a uniform field.
* `euler`, `rk2` (midpoint) and `rk4` re-interpolate the fluid velocity at the stage positions.

Sub-steps are limited to half a cell length per step and, for the non-analytic schemes, to a tenth of the relaxation time.

## Location and tracking

Faces are triangulated in a fan around the vertex average, so neighbouring cells share the same surface. A particle moves along
the segment of a sub-step and crosses faces through the triangle where the segment leaves the cell (monotone, terminating walk).

Point location without a hint starts at the nearest cell centre (bin grid). If the walk leaves the domain, a bounding-box brute
force search is the fallback (the domain may be non-convex). Bisection on the integrated path refines a boundary hit time.

Non-conformal interfaces work after `rcfd_mesh::non_conformal::join_non_conformal` has produced one conformal mesh (not tested
here). Periodic patches are rejected.

## Boundary actions and injection

Each patch applies `reflect` (specular, restitution `e`: normal component times `-e`, tangential kept), `stick` (deposited) or
`escape`. By default, wall patches follow `particles.wall_interaction`, symmetry and empty patches reflect with `e = 1`, and other
patches escape.

More than `max_bounces` (1000) reflections inside one flow step stop the parcel, which is reported with state `stuck`. In the PT7
test this happens to small particles that slide along a reflecting wall the flow presses them onto. Resting contact is not
modelled, so the reflections accumulate (Zeno); see ADR 0028 addendum A2.

An injection has these properties:

* Location: `point` or `surface` (uniform by area).
* Rate: `rate` parcels/s between `start_time` and `end_time`. The count is `floor(rate (t - t0))`, independent of the step.
* Particles per parcel: `parcel_count` real particles.
* Initial velocity: given, or the fluid velocity.
* Diameter: constant or Rosin-Rammler (oversize mass fraction `exp(-(d/d_bar)^n)`, formula verified in two secondary sources),
  sampled by inversion `d = d_bar (-ln u)^(1/n)` with optional truncation.

Each sampled parcel has the same `parcel_count` (documented simplification: no equal-mass parcel weighting). A serial SplitMix64
generator seeded from `particles.seed` supplies the random numbers, so a run is reproducible bit for bit.

## Case tree and run hooks

See "particles" in `docs/case-schema.md`. Library entry points in `rcfd_api::particles`:

* `check(case)`: located settings errors (also part of `Case::semantic_errors`).
* `build_tracker(case, &sim)`: tracker on the simulation mesh, mesh-dependent located errors (unknown patch, point outside the mesh).
* `track_frozen(case, &sim)`: tracks on the current velocity field (a `solver.frozen_flow` field or a converged steady solution).
* `track_transient(case, &mut sim)`: steps a `piso` / `pimple` simulation and tracks over each flow step on the current field.

These are the library functions. Since ADR 0037 (wave 21C) the run loop drives the tracker itself, with the arithmetic of these
hooks (bit-identical, checks W1/W2):

* `Simulation::new` builds the tracker (`rcfd_api::particle_run::ParticleRun`).
* The end of a transient `Simulation::step` advances it over the flow step.
* Steady algorithms are tracked once on the final field when the results are written.

`rcfd run` and the service (`run.headless`, `workflow`, `run.start`, and `particles.report`) therefore need no hook call.
`track_transient` detaches the built-in hook of the simulation it is given. A transient run with particles is not restartable.

Outputs of the run path (`particles.vtp`, `particles_<n>.vtp`, `particle_report.json`, optional `particle_tracks.csv`) are in
`docs/case-schema.md` ("particles", Run path).

## Output

The output format is ASCII VTK PolyData (`.vtp`): points and point data `id`, `diameter`, `velocity`, `density`, `parcel_count`,
`cell`, `age`, `state` (0 active, 1 deposited, 2 escaped, 3 stuck, 4 absorbed into the wall film). The settings are
`particles.output_file` and `particles.output_every`.

## Extensions of wave 29C (F-128, ADR 0057)

* Two-way exchange record (`rcfd_particles::Coupling`, `Tracker::enable_coupling`). Per integration sub-step (before any wall
  reflection) the carrier receives minus the DRAG IMPULSE `m (v1 - v0) - m g_eff h` in the cell where the sub-step starts, and minus the
  particle's total-energy gain `Delta KE - W_g + m c_p Delta T_p` (`W_g = m g_eff . Delta x`; with `Tracker::set_thermal`, Ranz-Marshall
  lumped-sphere heat transfer). `Coupling::sources(volumes, dt)` returns the per-cell momentum [N/m^3] and total-energy [W/m^3]
  sources. Wall reflections are booked separately in `wall_impulse`. Recording never changes the particles (bit-identical).
  `Simulation::step` does NOT apply the sources to the carrier: the caller feeds them into its equations (the checks use an explicit
  cell momentum update in a closed particle/carrier loop).
* Discrete random walk (`Dispersion`, `Tracker::set_dispersion`; case keys `particles.dispersion*`): see ADR 0057 D2 and the closed
  form `sigma_p^2 = sigma^2 T_L/(T_L + tau_p)`. Homogeneous isotropic turbulence of given k and epsilon only.
* Injection: `uniform` diameters, equal-mass parcels from `mass_flow`, cone injections (`ConeSpec`, `InjectorExt`).

## Wall film (F-253, ADR 0192)

Parcels of a liquid can feed a thin film on wall patches. With `[particles.film]` (`enabled = true`, the wall `patches`,
the liquid `viscosity`), a parcel that reaches a face of a film patch is absorbed: its mass joins the film on that face,
and the parcel ends with state 4 (`film`). The film liquid density is `particles.density`.

Each face holds a film mass `M`, so its thickness is `h = M/(rho_l A)`. The film runs along the wall with the mean
velocity of a steady laminar film of its own thickness under the part of gravity along the wall:

    u_bar = rho_l h^2 g_t / (3 mu_l),   g_t = g - (g . n) n

Down a plane inclined at `theta` this gives the Nusselt thickness for a mass flow `Gamma` per unit width:

    h_N = (3 mu_l Gamma / (rho_l^2 g sin(theta)))^(1/3)

Mass moves between neighbouring film faces by a donor-cell flux across their shared edges. The film is advanced at the
end of every tracking step, in explicit sub-steps that keep the Courant number of the kinematic wave (speed `3 u_bar`)
at 0.5 or less, so no face mass becomes negative. Film that reaches an edge where no other film face continues leaves the
film and is counted as drained.

Below the feed, the steady film has exactly the Nusselt thickness. ADR 0192 checks this on a 30-degree incline fed by
four parcel streams at `Re_f = 4 Gamma/mu_l = 2`. The check also requires the mass of all parcels injected to equal
airborne + deposited + escaped + stuck + film on the wall + drained to 1e-10. The model holds for laminar wave-free films,
`Re_f` up to about 30. It has no film inertia, so it cannot show the waves of a real film. The run report gives the
largest face `Re_f`.

Not modelled: shear and pressure gradients from the gas, surface tension and the hydrostatic gradient, splash and
rebound at impact, separation and dripping, evaporation and heat transfer of the film. The film never acts on the flow,
and it is refused together with `two_way` or `heat_transfer`. From code, `Tracker::set_film(FilmSpec)` switches it on
and `Tracker::film()` reads it (`rcfd_particles::film`).

## Wall interaction (F-254, ADR 0194)

Two optional models act where parcels hit walls. Both are off unless their table is written, and a run without them is
unchanged bit for bit.

### Erosion

With `[particles.erosion]` (`enabled = true`, the wall `patches`, `k`, `n` and the wall material `density`, or the steel
preset of ADR 0197 below in place of the three numbers), every parcel
hit on a face of an erosion patch erodes the mass

    dE = m K U^n F(alpha),   F(alpha) = sum_{i=1..8} (-1)^(i+1) A_i alpha^i

from that face. `m` is the parcel mass, `U` the impact speed, `alpha` the angle between the path and the wall (in radians,
from 0 for a grazing hit to `pi/2` for a normal one), and `A_1` to `A_8` = 9.370, 42.295, 110.864, 175.804, 170.137,
98.398, 31.211, 4.170. This is DNV RP O501 (Revision 4.2-2007), eq. 7.1 to 7.3 and Table 7-1, with the angle function DNV
gives for steel grades: it is 0 at grazing incidence, largest (1.002) near 36 degrees and 0.569 at normal incidence.

`K`, `n` and `density` have no default, because DNV says `K` and `n` have to be found by experiment. Its Table 7-2
recommends:

| Material | `K` [(m/s)^-n] | `n` | density [kg/m^3] |
| --- | --- | --- | --- |
| steel grades | 2.0e-9 | 2.6 | 7800 |
| titanium grades | 2.0e-9 | 2.6 | 4500 |
| GRP/epoxy | 0.3e-9 | 3.6 | 1800 |
| GRP/vinyl ester | 0.6e-9 | 3.6 | 1800 |

The angle function is the steel one whatever `K` and `n` are; DNV gives the brittle curve only as a plot.

**The steel preset (ADR 0197).** Instead of writing the three numbers, a case can name the steel row:

    [particles.erosion]
    enabled = true
    patches = ["ymin"]
    material = "dnv_o501_2007_steel"

This fills `k = 2.0e-9`, `n = 2.6` and `density = 7800.0`, the steel grades of the table above. **The Table 7-2 values come
from a third-party copy of DNV RP O501 Rev 4.2-2007 that has not been checked against the maintainer's edition; a preset
gives an indicative, not certified, erosion rate.** It is the only preset: the GRP rows go with the brittle curve, which is
not delivered, and titanium is not steel.

The preset is opt-in. Without `material` (or with `material = "none"`, the default) the three keys are required as
before, and the run and the settings hash of the case are unchanged. With a preset, a key written with a value other
than -1 overrides the preset value. `k` and `n` go together: `K` has the unit (m/s)^-n and the table gives the two as a
pair, so with a preset you set both or neither, and setting one alone is an error at the other. `density` can be set
alone. An unknown preset name is an error at `particles.erosion.material`.

The preset never fills in silently. The case file names it, the case data keeps the unset keys at -1, and the run records
what it used: `particle_report.json` gets `erosion.material` (the preset, its row and table, the three values used, for
each whether it came from the preset or the case, and the provenance text above), and `particle_erosion.csv` starts with
three comment lines, for example

    # erosion preset dnv_o501_2007_steel: steel grades, DNV RP O501 Rev 4.2-2007 Table 7-2
    # values used: k = 2e-9 (preset), n = 2.6 (preset), density = 2700.0 (case)
    # The Table 7-2 values come from a third-party copy of DNV RP O501 Rev 4.2-2007 that has not been checked against the maintainer's edition; a preset gives an indicative, not certified, erosion rate.

before its usual header. From code, `ErosionSpec::from_preset(patches, erosion_preset("dnv_o501_2007_steel").unwrap())`
gives the same spec (`rcfd_particles::erosion::EROSION_PRESETS`, `PRESET_PROVENANCE`).

The hit is booked before the wall action, whatever it is: reflect, rough reflect, stick or escape. Erosion is an
observation and never changes a parcel. An erosion patch cannot carry the wall film. The run writes
`particle_erosion.csv` with, per face, the hits, the parcel mass that hit it, the eroded mass `E`, the rate `E/(A T)` in
kg/(m^2 s) over the tracked time `T`, and the depth rate (the rate over the material density) in m/s. DNV's eq. 8.1 gives
the same depth rate in mm/year through a unit factor; rcfd reports SI only.

ADR 0194 checks the model with three uniform parcel streams hitting a plate at 15, 35 and 90 degrees: the reported rate of
each face equals `G K U^n F(alpha)`, with `G` the impact mass flux, to 1e-10.

### Rough walls

With `[particles.rough_wall]` (`enabled = true`, the `patches`, `angle_std`), a parcel that hits a rough patch reflects
about a virtual wall instead of the face. The virtual wall normal is the face normal tilted, in the plane of incidence, by
a roughness angle `gamma = angle_std xi` with `xi` drawn from a standard normal distribution, so the parcel meets the
virtual wall at `alpha + gamma` instead of `alpha`. This is the virtual-wall model of Sommerfeld (1992), as restated in
the literature that ADR 0194 lists. The reflection about the virtual wall uses the patch restitution `e` (normal part
times `-e`, tangential part kept).

A draw is kept only if the parcel reaches the virtual wall from the fluid side and its rebound leaves the real wall;
otherwise `gamma` is drawn again. At small impact angles this removes many negative angles, which tilts the rebounds
upwards. The literature calls this the shadow effect; the exact distribution of Sommerfeld and Huber (1999) was not read
and is not used. At normal incidence the plane of incidence is drawn at random.

Measured roughness angles in the literature range from about 1 to 15 degrees and depend on the particle size and the
wall; `angle_std` is limited to 30 degrees. A rough patch must reflect with a restitution above 0. The roughness has its
own random numbers (`seed`), so the other random streams of a run do not change. Not modelled: wall friction and
particle rotation, roughness out of the plane of incidence, a roughness that depends on the particle size.

ADR 0194 checks that the rebound angles of 20 000 parcels at each of 10, 40 and 75 degrees follow the distribution the
model prescribes (the Gaussian, truncated by the rule above and mapped through the reflection), with a Kolmogorov distance
below the 1 percent critical value. From code, `Tracker::set_erosion(ErosionSpec)` and `Tracker::set_rough_wall(RoughSpec)`
switch the models on (`rcfd_particles::erosion`, `rcfd_particles::rough`).

## Where to go next

* [case-schema.md](case-schema.md): the particle keys, defaults and output files of the run path.
* [architecture.md](architecture.md): where the particle crate sits in the crate graph.
