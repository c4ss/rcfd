# Dynamic mesh and rigid-body dynamics (F-063, F-229; ADR 0033)

rcfd can move the mesh during a run and integrate a rigid body driven by the flow. Status: partial. Incompressible laminar
PISO/PIMPLE with implicit Euler on a mesh whose points move every step, and the explicit compressible solver with a
prescribed motion or a layered piston. Decisions and checks are in `docs/adr/0033-wave20b-dynamic-mesh.md` (read its addendum
for the misses), `docs/adr/0090-wave38c-dynamic-mesh-layering.md`,
`docs/adr/0191-f259-registration-case-tree-and-cli-wiring-of-layering-and-compressible-ale.md`,
`docs/adr/0213-f257-registration-prism-layering-and-incompressible-layering.md` and
`docs/adr/0215-f247-registration-local-remeshing-on-unstructured-tetrahedral-meshes.md`.

This page explains how the mesh moves, how the body is integrated, and where the method stops. It is for users who plan a
moving-wall or moving-body case and want to know what the solver does and does not cover. For the exact keys and defaults of the
case section see [case-schema.md](case-schema.md); for the full decision record and the measured checks read the ADRs named above.

## Limits and gaps

* Euler time scheme only (PISO/PIMPLE).
* No turbulence, energy, scalars, particles, sources, rheology, statistics or restart.
* Sliding interfaces in the library only (F-250, ADR 0216): a rotor block turning against a stator block across a
  cylinder, the interface rebuilt every step, in the PISO/PIMPLE path (`rcfd_fv::sliding`,
  `PimpleSolver::step_sliding`). The tree does not use it yet (ADR 0216 Q1); no planar or sector slide.
* Local remeshing of tetrahedral meshes is in the library since F-247 (ADR 0215), for the compressible solver in a box
  with planar walls; the case tree does not use it yet (ADR 0215 Q1).
* Overset (chimera) meshes in the library only (F-134, ADR 0219): one component mesh moving rigidly over a static
  background mesh (`rcfd_mesh::overset`, `rcfd_fv::overset::OversetSolver`), PISO with implicit Euler and upwind, central
  or blended convection, no non-orthogonal correctors; outer iterations (F-335, ADR 0238) and BDF2 (F-338, ADR 0241) as
  opt-ins of the solver. The tree does not use it yet (ADR 0219 Q1); no second component, no moving background.
* Layering in the case tree only for the compressible solver, on a `box_3d` stack with one piston. The library also
  stacks prisms over any planar cross-section and layers the PISO/PIMPLE path since F-257 (ADR 0213); the tree does not
  use them yet (the wiring is open under F-259, ADR 0213 Q1).
* Compressible runs in the case tree: no characteristic boundary condition on a moving face, and a time step without the
  mesh speed. The library has both since F-258 (ADR 0207); the tree does not use them yet (the wiring is open under F-259,
  ADR 0207 Q1 and Q2). No six-DOF body (F-244).
* No strong coupling or added-mass stabilisation. The explicit coupling needs a body that is not much lighter than the displaced
  fluid; the checks use a fluid-to-body mass ratio of 0.2.
* A time-step limit of the default PISO step found by F-244 (ADR 0211 amendment 1) and located by F-299 (ADR 0214): across a
  narrow gap the step grows unstable above a diffusion number `nu dt/dx^2` of about 3 to 4 across the finest cells (16 cells
  across the gap, two correctors), on a fixed mesh as on a moving one. The first corrector over-corrects a pressure error by
  about that number, and on the collocated mesh the second corrector does not remove what is left for a pressure wave of four
  cells along the gap (the Rhie-Chow term the cell-gradient velocity update leaves); the mode alternates in sign every step.
  More correctors only raise the onset (three: unstable at `tau/10` with `-1.66` per step). Keep `nu dt/dx^2` near 3 or below
  in narrow gaps, or use the consistent correctors (SIMPLEC-type correctors, ADR 0214; the case key
  `[solver.consistent_correctors] enabled = true` since F-301, ADR 0223), which remove the instability: F-244's registered `tau/10`
  falling block passes with them. On a fixed mesh the ADR 0073 lag check in Raise mode refuses the default at step 1 (3.30 per
  step). On a moving mesh the library makes the same estimate on the moving step's own matrices with the opt-in
  `PimpleSettings::moving_mesh_lag` (ADR 0223 E0: the check, the adaptive corrector count and `lag_preflight_moving`, the pre-run
  estimate of the next moving step); the case path does not use it yet (the adaptive count and SV301 still exclude dynamic meshes,
  ADR 0223 Q2).
* The streamwise velocity in the cell next to an accelerating piston lags by about 3e-4 of the piston speed (registered limit
  1e-6 missed, cause not proven).
* The cyclic-face lag of PISO that earlier needed 10 outer iterations no longer reproduces (test `g4b`, moving-vs-fixed mesh
  difference 1.84e-14, re-run 2026-10-21).
* Baseline limitation met: `Symmetry` boundaries with the averaged diagonal produce a spurious tangential force (use
  `zero_gradient` sides).

## Method

- **Mesh motion.** Prescribed: displacement `d(t)` (expressions of `t`) and a rotation `angle(t)` about an axis through a point,
  applied to the points of the moving patches. Interior: `rigid` (all points follow) or `smoothing` (Laplacian,
  inverse-distance or inverse-volume diffusivity of the displacement equation solved from the initial mesh each step,
  prescribed on the moving patches, zero on `fixed_patches`, sliding in the plane for all other patches). Every moved mesh is
  validated; a failure stops the run with `dynamic_mesh: step N, t = ...: mesh error: cell i has non-positive volume ...`.
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
- **Flow-coupled validation (F-244, ADR 0211; library tests in `crates/rcfd-fv/tests/sixdof_flow_f244.rs`).** A block
  falling along a closed 2-D channel, the displaced fluid returning through two side gaps of a 500:1 length ratio, reaches
  the lubrication terminal velocity `U_t = (M - rho V_b) g / C`, `C = 2 mu H_b dz (3 B^2 + 1)/h`, `B = (w_b + h)/h`, to
  0.55 percent after 10 time constants (added-mass ratio 0.18; the neglected end, inertia and finite-time terms are bounded
  to 0.46 percent before the run, the end effect by a minimum-dissipation argument). The C2 piston on a spring oscillates
  at `sqrt(k/(M + rho A L0))` to 0.13 percent (mass ratio 0.2), where without the fluid force it is 9.5 percent off, and its
  amplitude decays by about 5 percent per period at 100 steps per period (the lag of the explicit coupling).

## Overset meshes (F-134, ADR 0219; library only)

- **Composite mesh.** `rcfd_mesh::overset::compose` joins a background mesh and a component mesh into one mesh with no
  face between them (the component patches get a prefix). Every step the component is moved rigidly to a pose and
  `Overset::connectivity` sorts the cells into calculated, fringe and hole cells: the component cells on its outer patches
  are fringe, the background cells where the user's hole function is true (in the component's frame) are holes, and the
  background cells next to a hole are fringe.
- **Interpolation.** A fringe cell takes its values from the cell of the other part that contains its centre and that
  cell's calculated neighbours, with weights that reproduce linear fields exactly. The fringe rows of the momentum and
  pressure systems are these interpolations, so the coupling is implicit (one system for both meshes).
- **Mass.** With `MassMode::Conservative` one offset of the background fringe pressures makes the net flux into the hole
  region zero (or, in a closed domain, keeps the reference cell exact). Then every calculated cell keeps its continuity
  and the domain keeps its mass. `MassMode::Interpolated` leaves the overlap non-conservative.
- **Checks.** A cylinder at Re 40 moving 7.5 diameters through a Cartesian background: the net outflow of the domain is
  `7.5e-15` of `U D L` and the continuity `8e-17` in every step, and the drag is within 0.62 percent of a body-fitted run of
  the same motion between `t = 2` and `t = 8` (limit 5 percent). Without the offset the domain loses `2.7e-3` of `U D L`. A
  uniform flow through a moving, turning component stays uniform to `1.4e-14`. In the Taylor-Green vortex the overset error
  stays within 1.82 times that of the background alone; its largest value, in an interpolated cell of the component,
  converges at order 1.62 between the two meshes tried (registered limit 1.7, missed), and the background at order 1.98.
- **Outer iterations (F-335, ADR 0238).** `OversetSolver::set_outer_iterations(n_outer, alpha_u, alpha_p)` repeats the
  PISO pass within a step on the kinds, donors and weights of the new pose, each pass with the convecting flux and the
  pressure of the pass before it (the under-relaxation factors act on every pass but the last), as `PimpleSolver::step`
  does; a solver that does not call it runs as before, bit for bit. On a moving component one pass leaves a first-order
  time error from the lagged convecting flux; in the Taylor-Green set-up on the finer mesh at `dt = 0.0025` three passes
  bring it from `4.49e-4` to `1.69e-4` and the overset error from 3.31 to 1.86 times that of the background alone. More
  passes do not lower it further (`1.69e-4` with five): the rest is the truncation of the implicit Euler scheme along the
  paths of the moving cells.
- **BDF2 (F-338, ADR 0241).** `OversetSolver::set_time_scheme(TimeScheme::Bdf2)` switches the three-level scheme on; a
  solver that does not call it runs as before, bit for bit. The time term is the backward difference of `V u` of the
  moving cell, and the mesh flux is `(c0 W^{n+1} - c2 W^n) / dt` of the swept volumes of the last two steps, so that a
  uniform flow stays uniform on a deforming mesh (to `3e-15`). The first step is an implicit Euler step; a cell that was a
  hole two steps back takes the implicit Euler row once (its old-old value is not a fluid value). On a mesh at rest with
  every cell calculated the step is `PimpleSolver::step` with BDF2, bit for bit; the library's own moving-mesh step has
  no BDF2. In the Taylor-Green set-up on the finer mesh at `dt = 0.0025` with three passes the time error falls from
  `1.69e-4` to `2.1e-5`. What is left does not fall with the square of the step (ratio 2.46 between 200 and 400 steps,
  second order gives 5); the library's BDF2 step shows the same on a single mesh at rest (ratio 2.30), independent of
  the start, the correctors and the passes; its source is open. The follow conditions of a moving wall are those of the
  two-level step. The bounded BDF2 and Crank-Nicolson are refused.
- **Not available:** a second component, a moving background, hole cutting from the wall geometry, more than linear
  interpolation, non-orthogonal correctors, linear-upwind convection, turbulence, energy and the case tree.

## Case tree

The section is `dynamic_mesh`; keys, defaults and unset values are in `docs/case-schema.md`. Static errors are located at the
key; incompatible sections are reported at `dynamic_mesh.enabled`.

## Layering and compressible ALE (F-230, ADR 0090; case tree F-259, ADR 0191)

- **Compressible ALE.** `CompressibleSolver::step_ale(new_geom, dt)` advances the explicit density-based solver while the points
  move (same topology). The fluxes are computed in the frame of the moving face, and a moving slip wall mirrors relative to the
  wall. A uniform state stays uniform to round-off. In the case tree: `solver.algorithm = "compressible"`,
  `compressible.mode = "transient"` and a prescribed `dynamic_mesh` motion with `deformation = "rigid"` or `"smoothing"`.
- **Characteristic conditions on moving faces and the mesh-speed step (F-258, ADR 0207; library only).** On a moving
  face, `step_ale` evaluates the total inlet, the pressure outlet, the non-reflecting outlet and the far field in the
  frame of the face (`compressible::characteristic_face_state`): the switches read the normal velocity relative to the
  face, the far field's free stream and the total inlet's total conditions are lab-frame data, and the flux is
  `F(q).n - w_n U(q)`. A uniform flow stays uniform to 1.9e-14 with all four on faces that move, and a piston-generated
  simple wave leaves through an outlet moving at Mach 0.2 with a reflection of 4.6e-4 (far field) and 3.8e-4
  (non-reflecting, `sigma = 0`) of its pressure jump, where a fixed-pressure outlet reflects all of it.
  `max_dt_moving(wn)` is the explicit step with the signal speed `|u.n - w_n| + c` relative to each face, and
  `max_dt_ale()` takes the face speeds of the last ALE step (`mesh_speeds()`). `max_dt()` stays the absolute step. The
  case tree does not use this yet: it refuses a characteristic condition on a patch that moves and steps with `max_dt()`.
- **Layering.** `rcfd_mesh::layering::LayerStack` is a planar layer stack along x whose last plane is a piston. Only the layer
  next to the piston deforms. Below `a_min h0` it merges with its neighbour; above `a_max h0` it splits into `h0` plus the rest
  (`1 + a_min <= a_max` is required). Every change returns a cell source map, `remap` carries cell totals over exactly, and
  `replace_mesh` installs the new mesh and state. In the case tree: `dynamic_mesh.deformation = "layering"` on a
  `mesh.generator = "box_3d"` mesh, the piston `xmax` moved by `displacement_expression_x`, `h0 = lx / nx`, `a_min = 0.4`,
  `a_max = 1.6`. The example below is ADR 0090's piston P as a case, with the compressible keys at their defaults (HLLC, van
  Leer, SSP-RK3, CFL 0.4): 9191 steps, 24 removals and 24 additions, and `p_bar V^gamma` back within 3.4e-5 of its start
  after the stroke.
- **Prism layers and layering on the PISO/PIMPLE path (F-257, ADR 0213; library only).**
  `rcfd_mesh::layering::ExtrudedStack` stacks prisms over the cells of a planar cross-section `Section` (for example
  `Section::rect_triangles`, rectangles cut into triangles) with the piston, the thresholds and the cell source map of
  `LayerStack`. ADR 0090's piston on triangular prisms meets ADR 0090's limits (`p_bar V^gamma` within 1.5e-5 and 3.4e-5,
  conservation over an event 6.7e-16, mass drift 7.0e-13). For the incompressible solver, `face_sources` and
  `remap_face_flux` carry the face flux through an event: a merged side face takes the sum of the two old faces, a split
  one the share of its layer height, and the faces of a new plane close the balance of the cell below them, so the
  discrete continuity holds across the event. `PimpleSolver::replace_mesh` then installs the new mesh with the remapped
  velocity, pressure and flux. On ADR 0033's piston channel (mirrored: the piston is `xmax`) with 7 removals, every step
  keeps the continuity within 3.1e-15 and the outlet flux within 1.3e-14 of the piston's swept flux, and each event
  conserves the volume to 2.2e-16. The piston can only be the last plane, and `replace_mesh` refuses BDF2, a face
  viscosity, body forces, sources and jumps.
- **The tree equals the library.** Run through the case tree, the piston and the moving-mesh checks of ADR 0191 give the same
  bits as the same set-ups driven by library calls, and `rcfd run` gives the same reports.
- **Stiffness.** The smoothing method `SmoothingMethod::InverseVolume(k)` uses the diffusivity `1/V^k`, so small cells deform less.
  In the case tree it is `dynamic_mesh.smoothing.method = "inverse_volume"` with `k = smoothing.exponent` (F-290, ADR 0203),
  on the incompressible and the compressible moving paths; `V` is the volume of the initial mesh, from which the displacement
  is solved. On ADR 0033's piston with cells 8 times smaller at the ends it keeps the smallest cell at 0.945 of its initial
  volume where the Laplacian keeps 0.75, and lowers the largest aspect ratio from 9.3 to 7.4.
- **Not available:** non-planar layers, more than one moving layered boundary, and, in the case tree,
  prism layers, layering with PISO/PIMPLE, characteristic boundary conditions on moving faces and a time step that
  includes the mesh speed (library only, above).

```toml
# ADR 0090's piston P: a closed box compressed to 0.4 of its length and re-expanded (peak piston Mach 0.05)
[mesh]
generator = "box_3d"
nx = 40
ny = 2
nz = 2
lx = 1.0
ly = 0.1
lz = 0.1

[solver]
algorithm = "compressible"
end_time = 32.0

[compressible]
gamma = 1.4

[compressible.initial.left]
density = 1.0
pressure = 1.0

[dynamic_mesh]
enabled = true
deformation = "layering"
moving_patches = ["xmax"]
displacement_expression_x = "-0.3 [m] * (1 - cos(pi * t / 16 [s]))"
```

## Local remeshing of tetrahedral meshes (F-247, ADR 0215; library only)

- **What it is.** `rcfd_mesh::remesh::TetRemesher` holds a tetrahedral mesh bounded by axis-aligned planes (for example
  the six sides of a box from `box_planes`). `move_plane_with_band(k, value, w)` moves one plane (a piston) with its
  points and drags the points within `w` behind it in proportion, so no point is overtaken and the motion alone inverts no
  cell. After each step `needs_event()` checks the cells: a mean ratio below `q_trigger`, or an edge shorter than `l_min`
  or longer than `l_max`. `remesh()` then repairs the mesh locally with edge splits, edge collapses, edge removals (3-2
  flips, larger rings and the 2-2 flip of an edge on a wall), 2-3 flips and point relocations, each checked to tile the
  same region with its boundary on the planes. `RemeshSettings::for_size(h0)` gives the registered thresholds
  (`l_min = 0.5 h0`, `l_max = 2 h0`, `q_trigger = 0.3`).
- **Conservation.** Every operator gives the totals of the cells it removes to the cells it makes in proportion to their
  intersection volumes, so an event returns, for every new cell, the fractions of the old cells it receives
  (`RemeshChange::sources`). `remesh::remap` carries the cell densities through it, and
  `CompressibleSolver::replace_mesh` installs the new mesh. Mass, momentum, energy and volume are kept to round-off; the
  remap is first order (cell averages).
- **Measured** (ADR 0215, `crates/rcfd-fv/tests/remesh_f247.rs`). ADR 0090's piston on a Delaunay tetrahedral mesh of
  1711 cells (`tetmesh::tetrahedralize`, `h0 = 0.035`), compressed to 0.4 of its length and back with remeshing instead
  of layering: `p_bar V^gamma` within 3.7e-6 at full compression and 9.4e-6 after the stroke (limit 2e-2), 1806 events,
  volume, mass, momentum and energy through every event within 1.6e-15, smallest mean ratio over the run 0.241 (threshold
  0.2), and no point below `x = 0.35` touched. Without remeshing the mean ratio falls below 0.2 when the piston is 0.14
  into its 0.6 stroke. The run takes 55497 steps (193 s); events are frequent, and many find nothing to change.
- **Limits.** Walls must be axis-aligned planes; one uniform target size; no face-flux map, so not for the PISO/PIMPLE
  path; not wired into the case tree (ADR 0215 Q1, Q3, Q4).

## Where to go next

* [mesh-quality.md](mesh-quality.md): what the mesh validation checks after every move.
* [meshing.md](meshing.md): how the meshes that move are generated.
