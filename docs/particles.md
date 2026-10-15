# Lagrangian particle tracking (F-056, ADR 0028)

One-way coupled point-particle tracking in the new crate `rcfd-particles` (layer 2: depends on `rcfd-core` and `rcfd-mesh` only)
and the settings-tree section `particles` (`rcfd-api::particles`). The particles never act on the flow, so existing flow results
and the baseline hashes are unchanged. Status: `partial`, see "Not delivered".

## Model

For a sphere of diameter `d` and density `rho_p` in a fluid of density `rho_f` and viscosity `mu`:

    dv/dt = f(Re) (u - v)/tau_p + (1 - rho_f/rho_p) g,   dx/dt = v
    tau_p = rho_p d^2/(18 mu),   Re = rho_f d |u - v|/mu,   f = C_D Re/24

| `particles.drag` | `C_D` | status of the formula |
| --- | --- | --- |
| `stokes` | `24/Re` | derived |
| `schiller_naumann` | `24/Re (1 + 0.15 Re^0.687)` for `Re <= 1000`, `0.44` above | coefficients verified in two secondary sources; the validity limit is quoted as 800 or 1000 (switch at 1000 UNVERIFIED); original 1933 paper not accessed |
| `none` | 0 | ballistic |

Gravity is applied with the buoyancy factor `1 - rho_f/rho_p`. Not modelled: added mass, pressure-gradient force, Basset history,
lift, Brownian motion, turbulent dispersion, rotation, particle-particle interaction, feedback on the fluid.

## Integration

`analytic`: fluid velocity and drag factor frozen over a sub-step; `v` relaxes exponentially to `u + tau g_eff`, position by the exact
integral. Exact for Stokes drag in a uniform field. `euler`, `rk2` (midpoint), `rk4`: fluid velocity re-interpolated at the stage
positions. Sub-steps are limited to half a cell length per step and (non-analytic schemes) to a tenth of the relaxation time.

## Location and tracking

Faces are triangulated in a fan around the vertex average, so neighbouring cells share the same surface. A particle moves along
the segment of a sub-step, crossing faces through the triangle where the segment leaves the cell (monotone, terminating walk). Point
location without a hint starts at the nearest cell centre (bin grid); if the walk leaves the domain, a bounding-box brute force search
is the fallback (the domain may be non-convex). A boundary hit time is refined by bisection on the integrated path. Non-conformal
interfaces work after `rcfd_mesh::non_conformal::join_non_conformal` produced one conformal mesh (not tested here); periodic patches are
rejected.

## Boundary actions and injection

Per patch: `reflect` (specular, restitution `e`: normal component times `-e`, tangential kept), `stick` (deposited), `escape`.
Defaults: wall patches follow `particles.wall_interaction`, symmetry and empty patches reflect with `e = 1`, other patches escape.
More than `max_bounces` (1000) reflections inside one flow step stop the parcel (state `stuck`, reported; this is the Zeno case of a
tiny particle pressed onto a wall by the flow).

Injection `point` or `surface` (uniform by area), `rate` parcels/s between `start_time` and `end_time` (count `floor(rate (t - t0))`,
independent of the step), `parcel_count` real particles per parcel, initial velocity given or the fluid velocity, diameter constant or
Rosin-Rammler (oversize mass fraction `exp(-(d/d_bar)^n)`, formula verified in two secondary sources) sampled by inversion
`d = d_bar (-ln u)^(1/n)` with optional truncation. Each sampled parcel has the same `parcel_count` (documented simplification: no equal-mass
parcel weighting). Random numbers: serial SplitMix64 seeded from `particles.seed`, so a run is reproducible bit for bit.

## Case tree and run hooks

See "particles" in `docs/case-schema.md`. Library entry points in `rcfd_api::particles`:

* `check(case)`: located settings errors (also part of `Case::semantic_errors`).
* `build_tracker(case, &sim)`: tracker on the simulation mesh, mesh-dependent located errors (unknown patch, point outside the mesh).
* `track_frozen(case, &sim)`: tracks on the current velocity field (a `solver.frozen_flow` field or a converged steady solution).
* `track_transient(case, &mut sim)`: steps a `piso` / `pimple` simulation and tracks over each flow step on the current field.

These are library functions; the `rcfd run` command and the service do not call them yet (named gap).

## Output

ASCII VTK PolyData (`.vtp`): points and point data `id`, `diameter`, `velocity`, `density`, `parcel_count`, `cell`, `age`, `state`
(0 active, 1 deposited, 2 escaped, 3 stuck). `particles.output_file`, `particles.output_every`.

## Not delivered (F-056 stays partial; F-128 not started)

Cyclone separation case (the roadmap acceptance names settling and cyclone sanity cases; only closed-form settling-type checks exist),
droplet / combusting / multicomponent particle types, cone / group / file injections, parcel-count control, two-way coupling, turbulent
dispersion, heat and mass transfer, breakup, wall film, erosion, periodic patches, tracking through unjoined non-conformal pairs,
gradient limiting in the interpolation, CLI/service invocation, compressible and variable-density flows, a time-dependent sequence of
VTK files tied to the flow time (only every n tracking steps), a pure-walk guarantee on non-convex domains (see ADR 0028 results).
