# The structural solver

rcfd has a finite-element structural solver for solid parts, the library crate `rcfd-fem`. This page says what it covers,
how to run it, and what the fan-blade demonstration shows (F-387,
[ADR 0500](adr/0500-f387-registration-fan-blade-static-and-modal-demonstration.md)). It is early-stage software. **Nothing in
it is validated against a measured blade or a measured fan.** The checks are closed forms, identities of the discretisation and
mesh convergence.

## Scope

The basic tier, planned in [ADR 0270](adr/0270-structural-solver-basic-tier-plan.md), is in place:

* Linear elastostatics and linear modal analysis of isotropic elastic solids in Tet4, Hex8 and Tet10 elements (Hex20 from the
  next tier as well).
* Meshes from the structured box builders, from the repository's finite-volume meshes, from the tetrahedral mesher and from a
  Gmsh file; Tet4 meshes lift to Tet10 with straight mid-edge nodes (`rcfd_fem::builders::upgrade_to_tet10`).
* Displacement constraints, and loads: nodal forces, traction, pressure, gravity, the centrifugal load of a part rotating at a
  constant speed, body force fields and thermal strain.
* A direct solve (envelope or sparse `L D L^T`), preconditioned conjugate gradients, reactions, strain and stress recovery, von
  Mises and principal stresses, strain energy.
* The consistent mass matrix and the lowest natural frequencies and mode shapes.
* Output to `.vtu`, and the [solid case file](solid-case-file.md), a TOML file of its own run by a library function.

Packages of the intermediate and advanced tiers (beams, shells, rotating parts, cyclic symmetry, dynamics, nonlinear materials
and others) have been added one by one. Each is a roadmap item of the category "Structural solver"; the
[detailed roadmap](ROADMAP.md) gives the status and the limits of each one. There is no CLI command and no MCP tool for the
structural solver yet: it is used from Rust or through the solid case file.

## Shells under pressure on curved meshes

This section states only what [ADR 0550](adr/0550-f546-registration-angle-weighted-shell-pressure-on-an-unstructured-sphere-octant.md)
measured (test only, no library change).

* **Mesh requirement.** On an unstructured triangulation of a thin curved shell, the error of a pressure load, whichever split of
  each triangle's pressure is used, is set by the shell's bending response to the nodal unevenness of the load, `~ c h^2 / (R t)`:
  second order in the edge length `h` and growing as `1 / t`. It is not the stiffness: the area-gradient load gives the uniform
  expansion to `6e-14` on the same mesh. Measured on a sphere octant (geodesic grid with jittered nodes and random edge flips,
  `R = 1`, `t = 0.01`, internal pressure), the radial displacement against `p R^2 (1 - nu) / (2 E t)`: error `1.40e-1`, `3.33e-2`,
  `7.99e-3` at n = 32, 64, 128 (order 2.07 and 2.06). Two percent at every node needs n = 128 (edges about `0.012 R`) at
  `t / R = 0.01`; the default load also meets it there (`1.01e-2`). ADR 0550 asks for `h^2 / (R t)` well below one for a percent
  level displacement; it measured no other threshold, thickness or shape.
* **When to choose `Load::shell_pressure_angle_weighted`.** It is opt-in; the default consistent split of Shell3 stays the
  default. ADR 0550 measured its error against the default's: better by a factor 0.62 to 0.79 on the unstructured octant above
  (a small gain; refinement is what reaches the percent level), by 16 on the structured geodesic grid, and by 300 on the
  one-diagonal cylinder of [ADR 0461](adr/0461-f540-registration-angle-weighted-shell-pressure.md). Choose it on curved meshes
  where the default split is uneven in that way (structured or one-diagonal meshes); on unstructured meshes do not expect it to
  replace refinement.

## The fan-blade demonstration

The test `crates/rcfd-fem/tests/sb_fan_blade.rs` runs one blade of the parametric 120 mm computer fan
(`rcfd_mesh::fan_design::ComputerFan::default()`, an illustration of the parameter schema and not a real fan) as a solid clamped
at the hub. Run it with `cargo test -p rcfd-fem --release --test sb_fan_blade -- --nocapture`.

What it uses:

| Item | Value |
|---|---|
| Blade | span 36.5 mm (radius 21.5 to 58 mm), NACA 4-digit sections, chord 22 to 36 mm, stagger 40 to 62 degrees, mean thickness 0.85 mm |
| Material | nominal PBT-like values: E = 2.5 GPa, nu = 0.38, density 1300 kg/m^3 (round numbers, not data) |
| Elements | quadratic tetrahedra (Tet10): a structured split of the blade, 1056 and 4416 elements |
| Support | every node of the hub section held (a rigid hub, no root fillet) |
| Loads | centrifugal load at 2000 rpm; a uniform 50 Pa pressure on the pressure side |
| Modes | the four lowest modes at rest |

What it shows (measured 2026-11-14):

* The reactions balance the centrifugal load (2.09 N on the finer mesh) to about 1e-11 and the pressure force to about 1e-10;
  the strain energy equals half the work of the loads to about 1e-10.
* From the coarse to the fine mesh the mean tip displacement under the pressure changes by 1.4 percent (41.8 to 42.4
  micrometres), under the centrifugal load by 2.3 percent, and the first natural frequency by 1.5 percent (188.8 to 186.1 Hz).
  A third mesh with twice the layers through the thickness changes them by 0.1 to 0.2 percent more.
* A flat rectangular plate with the blade's span, mean chord and thickness has its first bending frequency (152.0 Hz) between
  the Euler-Bernoulli beam value (142.9 Hz) and the wide-plate value (154.5 Hz), as plate theory says it must.
* The tetrahedral mesher of the repository meshes this blade only at some surface resolutions and leaves slivers (smallest
  dihedral angle 0.16 degrees); on the one mesh it made, the same balances hold and the tip displacement and first frequency are
  within 0.3 percent of the structured mesh.

## Limits

* **Not validated.** No measured or published data exist for this blade (see the
  [fan project breakdown](fan-project-breakdown.md), part G). The numbers above show that the solver is consistent and
  converges on this geometry. They do not show that a real fan blade has these frequencies or deflections: the material values,
  the hub clamp and the geometry are assumptions.
* Linear and at rest: the modes are those of a blade that does not turn. Stress stiffening, spin softening and Coriolis effects of
  the rotating blade are other roadmap items and are not part of this demonstration.
* The blade surface is faceted (straight element edges): the meshed volume is 1.3 and 0.3 percent below the exact blade volume
  on the two meshes.
* The structured blade mesh is available as a library function (see below). The solid case file cannot yet describe this blade
  or the centrifugal load: the keys are proposed in [ADR 0505](adr/0505-f593-registration-library-fan-blade-mesher-in-rcfd-fem-builders.md)
  and wait for the maintainer.
* The tetrahedral mesher does not mesh the thin blade reliably (see above); the structured split is the route that works.

## The blade mesher in the library

`rcfd_fem::builders` has the structured blade mesh of the demonstration as a public function
([ADR 0505](adr/0505-f593-registration-library-fan-blade-mesher-in-rcfd-fem-builders.md)):

* `fan_blade_tet4(&AxialFan, [n_c, n_t, n_s])` and `fan_blade_tet10(...)`: the blade as linear or quadratic tetrahedra, with
  `n_c` cells along the chord (cosine spaced, at least 2), `n_t` through the thickness and `n_s` along the span.
  `computer_fan_blade_tet10(&ComputerFan, blade, [n_c, n_t, n_s])` takes blade number `blade` of a fan design (with its
  manufacturing deviation, if any); `ComputerFan::default()` gives the blade of the demonstration.
* Sets: the element set `all`; node sets `all`, `hub` and `tip`; surface sets `hub`, `tip`, `pressure` (the pressure side with
  the leading and trailing edges) and `suction`. The four surface sets do not overlap and together make up the whole boundary.
* Refused: a zero division, `n_c` below 2, a blade design that does not validate, a blade number at or above the blade count.
* With the divisions `[12, 2, 8]` and `[24, 2, 16]` the meshes are bit for bit those of the demonstration test, and so are its
  results. As there, the mid-edge nodes lie on the straight edges, not on the curved blade surface, and the function does not
  check element quality of unusual blades.
