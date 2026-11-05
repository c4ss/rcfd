# Dynamic mesh and rigid-body dynamics (F-063, F-229; ADR 0033)

rcfd can move the mesh during a run and integrate a rigid body driven by the flow. Status: partial. Incompressible laminar
PISO/PIMPLE with implicit Euler on a mesh whose points move every step. Decisions and checks are in
`docs/adr/0033-wave20b-dynamic-mesh.md` (read its addendum for the misses).

This page explains how the mesh moves, how the body is integrated, and where the method stops. It is for users who plan a
moving-wall or moving-body case and want to know what the solver does and does not cover. For the exact keys and defaults of the
case section see [case-schema.md](case-schema.md); for the full decision record and the measured checks read the ADR named above.

## Limits and gaps

* Euler time scheme only.
* No turbulence, energy, scalars, particles, sources, rheology, statistics or restart.
* No sliding interfaces (AMI, F-250). Layering is in F-230 and remeshing in F-247.
* No strong coupling or added-mass stabilisation. The explicit coupling needs a body that is not much lighter than the displaced
  fluid; the checks use a fluid-to-body mass ratio of 0.2.
* The streamwise velocity in the cell next to an accelerating piston lags by about 3e-4 of the piston speed (registered limit
  1e-6 missed, cause not proven).
* The cyclic-face lag of PISO that earlier needed 10 outer iterations no longer reproduces (test `g4b`, moving-vs-fixed mesh
  difference 1.84e-14, re-run 2026-10-21).
* Baseline limitation met: `Symmetry` boundaries with the averaged diagonal produce a spurious tangential force (use
  `zero_gradient` sides).

## Method

- **Mesh motion.** Prescribed: displacement `d(t)` (expressions of `t`) and a rotation `angle(t)` about an axis through a point,
  applied to the points of the moving patches. Interior: `rigid` (all points follow) or `smoothing` (Laplacian or
  inverse-distance-diffusivity displacement equation solved from the initial mesh each step, prescribed on the moving patches,
  zero on `fixed_patches`, sliding in the plane for all other patches). Every moved mesh is validated; a failure stops the run
  with `dynamic_mesh: step N, t = ...: mesh error: cell i has non-positive volume ...`.
- **Swept volume and mesh flux.** For every face the volume swept between the old and the new position, divided by `dt`, is the
  mesh flux. The cell volume change equals the sum of its swept volumes to round-off (geometric conservation law, checks G1 to G3).
- **ALE momentum and pressure.** The convective flux is the relative flux `phi - W/dt`, the time derivative uses the old and new
  volumes, and the continuity equation of the flux uses the relative flux. A wall that follows the mesh has the mesh velocity as
  boundary value. On accelerating walls the pressure condition is the fixed-flux `dp/dn = -n.a_wall` (a condition of the ALE path
  only).
- **Walls at rest on a rigidly moving mesh** (`dynamic_mesh.rest_patches`, ADR 0183 P-B). An example is the hub and casing of a
  rotating passage whose faceted faces do not lie on a surface of revolution. Each face gets the velocity `(m_f/|S_f|) n_f`: no
  tangential part (the wall is at rest) and a normal part that carries the swept volume, so no relative flux crosses the wall.
  A plain `fixed` 0 wall would leave the relative flux `-m_f`. The pressure condition is that of a following wall. A rigid
  prescribed motion may then have no moving patch at all. The checks are V5 and PB-1 of the ADR 0183 amendment P-AB.
- **Limited `linear_upwind`.** `solver.linear_upwind_limiter = "barth_jespersen"` also applies to the momentum convection of the
  moving-mesh step (ADR 0183 P-A).
- **Six degrees of freedom.** A rigid body (mass, inertia tensor, translation locks, rotation locked / free / about a principal
  axis / hinge about a fixed axis) with gravity, translational and torsional springs and dampers, a user force hook (library
  only) and a linear drag. Integrators: classical RK4 (all modes) and Newmark average acceleration (translations and single-axis
  rotations). Orientation is a unit quaternion. The flow force and moment are
  `F = rho sum [p S - nu (grad u + grad u^T).S]` on the force patches. Coupling is loose and explicit: the force comes from the
  old step, the body advances, the mesh moves to the new pose, and the flow steps.

## Case tree

The section is `dynamic_mesh`; keys, defaults and unset values are in `docs/case-schema.md`. Static errors are located at the
key; incompatible sections are reported at `dynamic_mesh.enabled`.

## Layering and compressible ALE (F-230, ADR 0090; library only)

- **Layering.** The type `rcfd_mesh::layering::LayerStack` is a planar layer stack along x whose last plane is a piston. Only the
  layer next to the piston deforms. Below `a_min h0` it merges with its neighbour; above `a_max h0` it splits into `h0` plus the
  rest (`1 + a_min <= a_max` is required). Every change returns a cell source map, and `remap` carries cell totals over exactly.
- **Compressible ALE.** The method `CompressibleSolver::step_ale(new_geom, dt)` advances the explicit density-based solver while
  the points move (same topology). The fluxes are computed in the frame of the moving face, and moving slip walls mirror relative
  to the wall. A uniform state stays uniform to round-off. The method `replace_mesh` installs the mesh and state after a layer
  change.
- **Stiffness.** The smoothing method `SmoothingMethod::InverseVolume(k)` uses the diffusivity `1/V^k`, so small cells deform less.
- **Not available:** local remeshing, unstructured layering, case-tree keys, and characteristic boundary conditions on moving faces.

## Where to go next

* [mesh-quality.md](mesh-quality.md): what the mesh validation checks after every move.
* [meshing.md](meshing.md): how the meshes that move are generated.
