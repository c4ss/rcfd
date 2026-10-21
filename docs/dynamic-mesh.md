# Dynamic mesh and rigid-body dynamics (F-063, F-229; ADR 0033)

Status: partial. Incompressible laminar PISO/PIMPLE with implicit Euler on a mesh whose points move every step. Decisions and checks are in
`docs/adr/0033-wave20b-dynamic-mesh.md` (read its addendum for the misses).

## Method

- **Swept volume and mesh flux.** For every face the volume swept between the old and the new position, divided by `dt`, is the mesh flux. The cell volume
  change equals the sum of its swept volumes to round-off (geometric conservation law, checks G1 to G3).
- **ALE momentum and pressure.** The convective flux is the relative flux `phi - W/dt`; the time derivative uses the old and new volumes; the continuity
  equation of the flux uses the relative flux. A wall that follows the mesh has the mesh velocity as boundary value; on accelerating walls the pressure
  condition is the fixed-flux `dp/dn = -n.a_wall` (a condition of the ALE path only).
- **Mesh motion.** Prescribed: displacement `d(t)` (expressions of `t`) and a rotation `angle(t)` about an axis through a point, applied to the points of the
  moving patches. Interior: `rigid` (all points follow) or `smoothing` (Laplacian or inverse-distance-diffusivity displacement equation solved from the
  initial mesh each step, prescribed on the moving patches, zero on `fixed_patches`, sliding in the plane for all other patches). Every moved mesh is validated;
  a failure stops the run with `dynamic_mesh: step N, t = ...: mesh error: cell i has non-positive volume ...`.
- **Six degrees of freedom.** A rigid body (mass, inertia tensor, translation locks, rotation locked / free / about a principal axis / hinge about a fixed axis)
  with gravity, translational and torsional springs and dampers, a user force hook (library only) and a linear drag. Integrators: classical RK4 (all modes) and
  Newmark average acceleration (translations and single-axis rotations). Orientation is a unit quaternion. The flow force and moment are
  `F = rho sum [p S - nu (grad u + grad u^T).S]` on the force patches. Coupling is loose and explicit: force from the old step, body advanced, mesh moved to the
  new pose, flow stepped.

## Case tree

Section `dynamic_mesh`; keys, defaults and unset values are in `docs/case-schema.md`. Static errors are located at the key; incompatible sections are reported
at `dynamic_mesh.enabled`.

## Limits and gaps

Euler time scheme only; no turbulence, energy, scalars, particles, sources, rheology, statistics or restart. No sliding interfaces (AMI, F-250); layering is in F-230 and remeshing in F-247,
no strong coupling or added-mass stabilisation (the explicit coupling needs a body that is not much lighter than the displaced fluid; the checks use a
fluid-to-body mass ratio of 0.2). The streamwise velocity in the cell next to an accelerating piston lags by about 3e-4 of the piston speed (registered limit
1e-6 missed, cause not proven). The cyclic-face lag of PISO that earlier needed 10 outer iterations no longer reproduces (test `g4b`, moving-vs-fixed
mesh difference 1.84e-14, re-run 2026-10-21). Baseline limitation met: `Symmetry` boundaries with the averaged diagonal produce a spurious tangential force (use `zero_gradient`
sides).

## Layering and compressible ALE (F-230, ADR 0090; library only)

- **Layering.** `rcfd_mesh::layering::LayerStack` is a planar layer stack along x whose last plane is a piston. Only the layer next to the piston deforms.
  Below `a_min h0` it merges with its neighbour; above `a_max h0` it splits into `h0` plus the rest (`1 + a_min <= a_max` is required). Every change
  returns a cell source map, and `remap` carries cell totals over exactly.
- **Compressible ALE.** `CompressibleSolver::step_ale(new_geom, dt)` advances the explicit density-based solver while the points move (same topology).
  The fluxes are computed in the frame of the moving face, and moving slip walls mirror relative to the wall. A uniform state stays uniform to round-off.
  `replace_mesh` installs the mesh and state after a layer change.
- **Stiffness.** `SmoothingMethod::InverseVolume(k)` uses the diffusivity `1/V^k`, so small cells deform less.
- **Not available:** local remeshing, unstructured layering, case-tree keys, and characteristic boundary conditions on moving faces.
