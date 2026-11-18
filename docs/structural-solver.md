# The structural solver

rcfd has a finite-element structural solver for solid parts, the library crate `rcfd-fem`. This page says what it covers,
how to run it, and what the fan-blade demonstration shows (F-387,
[ADR 0500](adr/0500-f387-registration-fan-blade-static-and-modal-demonstration.md)). It is early-stage software. **Nothing in
it is validated against a measured blade or a measured fan.** The checks are closed forms, identities of the discretisation and
mesh convergence.

## Reading list for an agent starting work here

Read first, about 30 KB in all: this page (22 KB without the generated list at its end), `crates/rcfd-fem/src/lib.rs`
(3 KB, the module list), and the entry of your item in [the roadmap](ROADMAP.md) with its `gap`. Then the ADRs your brief
names, and the rows of the line "Structural solver" in [the ADR index](adr/index.md) to find the records of neighbouring
items. The three tier plans are reference, not first reading: [ADR 0270](adr/0270-structural-solver-basic-tier-plan.md)
(93 KB), [ADR 0290](adr/0290-structural-solver-intermediate-tier-plan.md) (115 KB) and
[ADR 0310](adr/0310-structural-solver-advanced-tier-plan.md) (164 KB); read the section of your item, not the whole.

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
* **`Load::shell_pressure_curvature` (opt-in, Shell3 only).** [ADR 0567](adr/0567-f633-registration-shell3-pressure-load-exact-for-the-uniform-membrane-state.md)
  scales the area-gradient load by `p / (2 kappa)`, `kappa` the mean curvature of an implicit quadric fitted to each node's
  neighbourhood, with a boundary term for free edges and symmetry cuts. It is exact for the uniform membrane state on any
  triangulation inscribed in a sphere and on cylinders of rectangle cells: on the unstructured octant above `1.2e-13` at n = 64
  and `7.8e-14` at n = 32 (`t = 0.01` and `0.001` alike), the one-diagonal cylinder O1 `2.0e-10`. On a surface that is not a
  quadric the curvature estimate is only consistent (on a torus patch second order inside, about 14 percent at the boundary
  nodes at 32 x 32 cells), and nodes of zero mean curvature (flat parts, saddles) are refused: load flat parts with the default
  pressure on their own element set.
* **`Load::shell_pressure_curvature_with(set, p, CurvatureBoundary::Jet)` (opt-in).** [ADR 0579](adr/0579-f656-registration-curvature-shell-pressure-on-a-non-quadric-shell.md)
  replaces the curvature estimate at the nodes next to the boundary (distance 0 and 1, whose stencils are one-sided) by a
  quartic height jet fitted over the same stencil, unless the quadric fits that stencil to a relative residual of `1e-9` (then
  the surface is a quadric there and the ADR 0567 estimate is kept: spheres and cylinders give the same bits). On the torus
  patch the boundary error falls from 14 percent to `9.3e-3` at 32 x 32 (third order); on a pressurised torus segment with
  symmetry cuts the normal and meridional displacements are within `4.1e-4` and `5.3e-4` of the membrane solution at the
  finer of two meshes (the default estimator `3.7e-3` and `2.1e-3`, the default pressure `7.4e-2`).

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

## What was built

<!-- process-report:built begin (generated by scripts/process_report.py --write; do not edit) -->
The 133 items of the line "Structural solver" that are done, from `docs/roadmap.json`:

- F-370: Crate rcfd-fem: the data model of a solid model and the structured builders
- F-371: Solid mesh from the existing meshes: polyhedral mesh, tetrahedral mesher output, patches as surface sets, curved edges
- F-372: Element library: linear tetrahedron, trilinear hexahedron, quadratic tetrahedron; isotropic elasticity matrix
- F-373: Sparse direct solver for symmetric positive definite matrices (envelope L D L^T after reverse Cuthill-McKee)
- F-375: Surface and body loads: traction, pressure, gravity, centrifugal load of a rotating part, body force field
- F-377: VTK output of a solid result: finite-element cells with point and cell data
- F-379: Closed-form benchmarks: cantilever bending and the thick-walled cylinder under pressure
- F-380: Thermal strain: nodal temperature field, expansion coefficient, thermal load and stress
- F-382: Eigensolver for K x = lambda M x: dense Jacobi and subspace iteration on the direct factorisation
- F-388: Scaffold of the intermediate tier: module layout, model extension data, element-trait extension points
- F-390: Hex8 without locking: mean-dilatation (B-bar) and incompatible-mode formulations, opt-in per element block
- F-391: Six unknowns per node: rotations in the DofMap, nodal moments, and a two-node shear-flexible beam element
- F-392: Four-node shell element (flat facet, assumed shear strain): plates, housings and thin blades as surfaces
- F-393: Linear multi-point constraints by a transformation, rigid sets with a reference point, elastic supports
- F-394: Cyclic symmetry: one sector of a rotor for statics and for modal analysis by harmonic index
- F-399: Harmonic response by mode superposition: modal damping, static correction, effective mass, force harmonics
- F-409: Orthotropic elasticity with material axes, orthotropic expansion, and temperature-dependent properties from tables
- F-411: Creep: power law with time hardening, implicit integration with a consistent tangent
- F-413: High-cycle fatigue: stress-life curves, mean-stress corrections, harmonic and counted histories, linear damage
- F-414: Damped and complex modes: a dense eigensolver for general real matrices and the quadratic eigenproblem on a modal subspace
- F-417: Random vibration: response to force and base-acceleration spectra by modal superposition, root-mean-square stress
- F-424: Follower pressure, arc-length continuation, and modes about a nonlinear equilibrium
- F-428: Laminated shell: layup stiffness, ply stresses and failure indices
- F-433: Elasticity solves on the ranks of rcfd-dist and on the CUDA device
- F-457: Scale-aware convergence test of the subspace eigensolver
- F-458: Manufactured solution: the Tet4 traction-case L2 order on the pair 16 -> 32 as a registered check
- F-459: Manufactured solution on distorted meshes: orders of Tet4, Hex8, Tet10 with moved interior nodes, and the Hex8 stiffness rule
- F-460: Incomplete Cholesky at half the block-Jacobi iterations on the elastic cantilever: ordering and one level of fill
- F-463: Static solid participant: the failing solve in a later iteration as a registered check
- F-464: Element tests: the moved-mesh check of the Hex8 patch test measures the stored mesh
- F-469: Coupled step: failing-participant checks with a start state that does not use the restores at the start of an iteration
- F-477: Elasticity multigrid on quadratic tetrahedra: a smoother that brings Tet10 within twice the Hex8 count
- F-484: Hex20 face loads through the model path
- F-485: Hex20 cantilever with the beam-theory support and the bore stress on a refined mesh
- F-488: Zero modes on the transformation: the eigenvalue measure against the shift and the explicit-shift report
- F-490: Transient integrator on the transformation path and with prescribed motion histories
- F-491: Static correction of the harmonic response for free bodies and on the transformation path
- F-492: Buckling factors of the knife-edge column against the solid's closed form (ADR 0390 known miss)
- F-496: Refuse a mechanism that is not rigid in buckling on the prescribed path
- F-498: Flap-lag identity of a rotating solid beam: the acceptance restated
- F-499: Coriolis coupling of a rotating part: gyroscopic matrix, whirl modes and the Campbell table
- F-502: Coupled frequency of the dynamic solid on non-matching interfaces: face refinement and map
- F-504: Opt-in bilinear projection on quadrilateral interface elements
- F-507: Stationary-frame frequencies of a cyclic whirl table
- F-510: Odd-index instability of a rotating part on the model path
- F-511: Whirl stability of a cyclic sector above the loss of stiffness
- F-516: Beam output: refusal of a result with the model's count of unknowns in another layout
- F-518: Beams on the transformation: a check that observes the entry refusal before the transformation is built
- F-520: Accurate far-mode vectors of the sparse whirl state solver
- F-522: Shells on the transformation path and body loads on shells: rigid sets and supports on shell nodes, gravity and centrifugal loads
- F-523: Deck pressure loads on non-parallelogram faces
- F-524: Rotating plane-stress disc of Shell4 facets against the closed form
- F-532: Damped modes: residual vectors for boundary dampers near the impedance
- F-534: Fatigue: rainflow residue error and Gerber compressive-mean message
- F-540: Consistent pressure on curved facet meshes
- F-543: Joint couplings with consistent face weights
- F-544: Cyclic symmetry with cylindrical material axes
- F-545: Transient isotropic equivalence with a probed limit
- F-546: Angle-weighted shell pressure on unstructured curved meshes
- F-547: Pre-stressed modes with a cyclic constraint and per-point materials
- F-549: Drilling membrane for Shell4 matching the ANDES edge
- F-553: Reduced parts as superelements in an ordinary model
- F-554: Zero-speed split frequencies against the Rayleigh quotients of the F-396 basis
- F-563: Speed-dependent bearing coefficients in whirl, Campbell and response
- F-564: Damped critical speeds as unbalance-response peaks and checked Campbell crossings
- F-567: Complex cross-spectral matrices and frequency-dependent coherence of random loads
- F-571: Rounding floor of the total Lagrangian internal force under large rotation and translation
- F-572: Arc-length with a follower reference load and an unsymmetric factor of the load stiffness
- F-574: Warning for the constant-strain Shell3 membrane on warped meshes
- F-575: Second-order Ritz error of the corrected sector split modes
- F-576: Quasi-static viscoelastic time stepping on the Newton driver, with a temperature in the nonlinear transient integrator
- F-577: The viscoelastic material in the direct harmonic solve and the damped modes
- F-579: Mutual radiation impedance matrix of a baffled plane and the light-fluid added mass and damping of structural modes
- F-583: The harmonic stress of a viscoelastic material in the direct harmonic solve
- F-589: First and fourth spectral moments of a random stress component for broadband fatigue
- F-595: Coordinate nested dissection in the eigensolvers
- F-599: Interference fit through-thickness variation of the contact pressure (re-register I3 of ADR 0507)
- F-600: A contact Newton order measure for short solves
- F-602: Thick sphere with J2 on a 16-layer octant: residual hoop stress after unloading
- F-603: Craig-Bampton interface enrichment for frequency accuracy near half the lowest omitted fixed-interface frequency
- F-606: Active-set record of the Newton driver on a load and unload solve
- F-608: Total Lagrangian contact with a lifted contactor: convexified tangent on a refused pivot
- F-609: Pseudo-modes in the viscoelastic own-frequency iteration
- F-610: Hinged cylindrical roof limit load against a published reference
- F-611: Energy-momentum step with F-425's hyperelastic materials
- F-614: A non-equilibrium quadratic displacement test for the error-estimate samples of an opted Hex8
- F-615: The supernodal factorisation in the linear contact driver
- F-617: A thermal predictor for the attempts of the nonlinear paths with a temperature field
- F-619: Direct harmonic base motion and reference response on the transformation path
- F-626: Warping unknown on the static solve, modes and pre-stressed modes
- F-628: Shells in the cyclic constraint
- F-629: Six-unknown nodes in the smoothed-aggregation CG
- F-631: Residual reference of the Newton driver in free vibration
- F-632: Domain integral on a front with varying J, and mode separation
- F-634: Shell documentation note on pressure-loaded thin curved shells on unstructured meshes
- F-636: Bore-zone witness for the finite-strain growth of the B-bar creep cylinder
- F-638: Pressure recovery of the mortar contactor
- F-639: Curvature-free fallback of the total Lagrangian contact body
- F-641: Smoothed aggregation for thin plates and shells
- F-642: Buckling eigensolver on sign-asymmetric spectra
- F-643: Load height and non-uniform moment for monosymmetric warping beams
- F-645: Hyperelastic materials on Beam2, Shell4 and Shell3 on the linear paths
- F-646: Linear paths about a pre-stressed hyperelastic state
- F-649: Arc-length continuation with a follower load direction
- F-650: Buckling eigensolver when the subspace holds fewer positive Ritz values than asked
- F-651: Arrhenius creep factor: non-positive temperature control and the nonlinear transient integrator
- F-657: Whirl of a spinning annular disc of shells (rotary block of the shell Coriolis matrix)
- F-659: Vertex-patch smoother on a mesh sequence of thin plates (robustness of the ADR 0572 smoother in h/t)
- F-661: Pre-stress on the warping harmonic path
- F-662: Two-sided shell edge ties for non-matching cyclic boundaries
- F-663: Distributed loads at a height on warping beams (a consistent load-height term (1/2) int q (r . e) phi^2 dx on the Hermite twist)
- F-664: Backward-error residual of the pencil eigensolver
- F-665: Beam2 tangential follower tip force and a linear pre-stress flutter path
- F-667: Pre-stress on the warping transient path
- F-668: Negative-pivot count of the pre-stressed warping harmonic solve
- F-670: Load height on the pre-stressed warping modes and harmonic path
- F-674: Full_rotor and the expansion of a cyclic sector with Beam2
- F-676: Flutter about a nonlinear state with the full contribution tangent
- F-677: State dependence of contribution tangents about a nonlinear state, a blind 3-D check
- F-679: An independent whirl reference for spinning beams
- F-684: Line search on the other total Lagrangian contact cases
- F-686: The free-sector pre-stress on tetrahedral, layered-shell and orthotropic sectors
- F-689: Interaction integral on closed curved fronts and mirror-plane stations
- F-691: A negative control of the energy-momentum inertia-term reference off rest
- F-693: The stabilised incompatible-modes Hex8 beyond the F-685 set-up
- F-696: Run the heavy effectivity record of ADR 0616
- F-709: The torque-free centrifugal load of a layup with a first mass moment
- F-748: The creep and viscoelastic readings of the committed modes of the warm start
- F-780: Driver retry with the indefinite tangent after a refused factor
- F-841: The subcritical branch of rule L's 16^3 punch path past amplitude 1.65e-4 with the warm start of the local modes
- F-843: The fold of rule L's 16^3 punch branch by an extended system with warm-started local modes
- F-844: The local mode update of the incompatible Hex8 as a default, compared on the ADR 0658 set-ups and on rule L's 16^3 punch branch
- F-845: The library arc-length driver through rule L's 16^3 fold with warm-started local modes
<!-- process-report:built end -->
