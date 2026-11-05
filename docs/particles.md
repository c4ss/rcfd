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
* erosion
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

## Where to go next

* [case-schema.md](case-schema.md): the particle keys, defaults and output files of the run path.
* [architecture.md](architecture.md): where the particle crate sits in the crate graph.
