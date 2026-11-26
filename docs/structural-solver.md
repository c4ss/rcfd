# The structural solver

rcfd has a finite-element structural solver for solid parts, the library crate `rcfd-fem`. This page says what it covers,
how to run it, and what the fan-blade demonstration shows (F-387,
[ADR 0500](adr/0500-f387-registration-fan-blade-static-and-modal-demonstration.md)). It is early-stage software. **Nothing in
it is validated against a measured blade or a measured fan.** The checks are closed forms, identities of the discretisation and
mesh convergence.

## Reading list for an agent starting work here

Read first, about 35 KB in all: this page (28 KB without the generated list at its end), `crates/rcfd-fem/src/lib.rs`
(3 KB, the module list), and the entry of your item in [the roadmap](ROADMAP.md) with its `gap`. Then the ADRs your brief
names, and the rows of the line "Structural solver" in [the ADR index](adr/index.md) to find the records of neighbouring
items. The three tier plans are reference, not first reading: [ADR 0270](adr/0270-structural-solver-basic-tier-plan.md)
(93 KB), [ADR 0290](adr/0290-structural-solver-intermediate-tier-plan.md) (115 KB) and
[ADR 0310](adr/0310-structural-solver-advanced-tier-plan.md) (164 KB); read the section of your item, not the whole.

## Scope

The basic tier, planned in [ADR 0270](adr/0270-structural-solver-basic-tier-plan.md), is in the code. Five of its items are
still partial by a missed check (F-378, F-381, F-383, F-384, F-386; see "Not there or partial" below):

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

## What the solver does today

A hand-written summary of the 307 roadmap items of the line "Structural solver" as of 2026-11-18: 152 done, 129
partial, 26 todo. It was drafted by Claude from the items' titles, acceptances, notes and gaps in `docs/roadmap.json`,
not from the ADRs; 32 of its numbers were compared with the roadmap by script. It is not regenerated: when the list
below has grown by the number of items the lead has set, a distillation item rewrites it
([lead-process.md](process/lead-process.md)). Ids in parentheses are done items, except in the last subsection and
where a base item is named as partial. Numbers are relative errors
against the reference named, from the item's note. Several base capabilities (modal analysis, Hex20, Shell3, buckling,
linear transient, the Newton driver, J2, contact, the sparse factorisations) are in the code but their own item is
partial: they appear below only through done follow-up items, and by name in "Not there or partial". The sections
above (the fan-blade demonstration is F-387) are not repeated.

### Model, meshes and solid elements

* `rcfd-fem` holds the solid model as validated data: Tet4, Hex8 and Tet10 blocks, node, element and surface sets
  (F-370, F-388). Meshes convert from an all-tetrahedron or all-hexahedron `rcfd_mesh::Mesh` and from the tetrahedral
  mesher's output, with patches as surface sets (F-371).
* Tet4 (1 point), Hex8 (2 x 2 x 2 Gauss) and Tet10 (4 points) with isotropic elasticity (F-372). Patch test on boxes
  with moved interior nodes: 4.4e-16, 6.1e-16, 1.2e-15 (F-374). Manufactured solution: Tet4 L2 order 1.9176 on the
  pair 16 -> 32 (F-458); orders on distorted meshes are a registered check (F-459, F-464).
* Hex8 has two opt-in formulations, B-bar and incompatible modes. One-layer cantilever L/h = 20: 0.99913 of the closed
  form against 0.667 with full integration; Lame cylinder at nu = 0.4999 with B-bar: 1.7e-3 against 0.225 (F-390).
* The incompatible-modes Hex8 runs at finite strain in the Newton driver as an enhanced assumed strain element (F-596,
  F-693). Its entry points accept a committed state carrying the nine local modes, the "warm start" (F-847, F-748);
  measured as a would-be default it passes 11 of 11 set-ups, today's default 7; no default was changed (F-844).
* Hex20: face loads through the model path (F-484); cantilever with the beam-theory support 0.04 percent (F-485).

### Beams and shells

* Beam2: two nodes, six unknowns per node, exact prismatic Timoshenko stiffness, nodal moments, consistent mass with
  rotary inertia. Pinned-pinned frequencies within 1.3e-5, 1.9e-4, 9.1e-4 of the exact Timoshenko values (F-391).
* Opt-in warping unknown for Beam2 (14 unknowns per element). Lateral-torsional buckling of an I beam under uniform
  moment: 5.9e-4 at 16 elements, against -19.2 percent without warping (F-598). It is on the static solve, the modes
  and the pre-stressed modes (F-626), takes monosymmetric sections (F-627), load height of point and line loads
  (F-643, F-663, F-670) and a pre-stress on its harmonic and transient paths (F-661, F-667, F-668).
* Shell4: flat facet, MITC4 shear, drilling penalty. Simply supported plate 16 x 16: 7.5e-4 of Navier at t/a = 0.01,
  1.26e-3 at 0.001 (F-392). Rotating plane-stress disc against the closed form (F-524). An opt-in ANDES membrane lets
  Shell4 share edges with ANDES Shell3: mixed patch 2.1e-14, against 2.1e-1 with the default Shell4 (F-549).
* Laminated Shell4: classical lamination theory, ply stresses, maximum-stress, maximum-strain, Tsai-Hill and Tsai-Wu
  indices; [0/90]s plate within 1.58e-3 of Navier (F-428); body loads and pre-stress (F-591, F-709).
* Shells take rigid sets, supports, gravity and centrifugal loads on the transformation path (F-522). An opt-in check
  reports constant-strain Shell3 membranes on warped meshes (F-574). Shell pressure: see above (F-540, F-546, F-634).

### Materials

* Orthotropic elasticity from nine constants with fixed, cylindrical or per-element axes, orthotropic expansion and
  temperature tables, on the linear static solve of solid elements (F-409); on the cyclic paths when the axes are
  cylindrical about the cyclic axis (F-544, F-547). Thermal strain from one nodal temperature field (F-380).
* Linear viscoelasticity: Prony series, WLF and Arrhenius shift, complex moduli; relaxation of three branches within
  4.7e-16 (F-410). On the quasi-static time-stepping driver (F-576) and in the direct harmonic solve with its stress
  (F-577, F-583); the own-frequency iteration can skip pseudo-modes (F-609).
* Power-law creep with time hardening, backward Euler and a consistent tangent, on the quasi-static time-stepping
  driver (F-411, F-651, F-636); a temperature history and an opt-in thermal predictor (F-592, F-617).
* Hyperelastic (base item F-425 partial): Beam2 and shells take the linearised material (F-645), by default since
  ADR 0746 (F-666); linear-path checks (F-644); modes, buckling and harmonic response about a nonlinear state (F-646,
  F-669).
* J2 plasticity (base item F-422 partial): thick sphere on 16 radial Hex20 layers, residual hoop stress at the bore
  4.52e-3 (F-602); yielded-set record checked (F-606).

### Loads, constraints, statics and substructures

* Nodal forces, prescribed displacements, reactions; free rigid-body motions and mechanisms are refused. Assembly
  gives the same bits for 1 and 8 threads; the static solve is direct by default, CG the alternative (F-374).
  Traction, pressure, gravity, centrifugal load and a body force field as consistent nodal loads (F-375, F-523).
* Strain and stress at integration points, element means, nodal averages, von Mises, principal stresses, strain energy
  (F-376). Closed forms: cantilever L/h = 20, Tet10, 5.3e-4; Lame cylinder, Hex8 32 x 64, bore displacement 1.8e-4,
  bore hoop stress 5.7e-3 (F-379). Error-estimate checks (F-614, F-696).
* Linear multi-point constraints, rigid sets with a reference point and elastic supports by a transformation of the
  unknowns (F-393, F-488). Beam-to-solid and shell-to-solid joints with consistent face weights: 5.4e-6 and 5.6e-4
  (F-543). A beam node shared with a solid is refused (F-518).
* Cyclic symmetry: one sector for statics and for modes by harmonic index; N = 12 ring and disc equal the full model
  within 4.0e-14 and 1.8e-13 (F-394). Static loads of a harmonic index (F-489). Shells opt-in (F-628), two-sided shell
  edge ties on non-matching boundaries (F-662), Beam2 in the full rotor opt-in (F-674).
* Reduced parts attach to an ordinary model as superelements for the static, modal and direct harmonic solves (F-553).
  Opt-in Craig-Bampton interface enrichment: frequencies within 2.9e-6 with one block, against 5.1e-3 without (F-603).

### Modes, buckling, harmonic and random response

* Eigensolver for `K x = lambda M x`: dense Jacobi and subspace iteration on the factorisation, with a Sturm count
  that verifies no eigenvalue was missed (F-382); an opt-in backward-error residual test (F-457); opt-in coordinate
  nested dissection, 98 s against 127 s per modal analysis at 104 544 unknowns (F-595).
* Buckling: knife-edge Hex20 column within 4.4e-5 and 1.7e-4 of the derived closed form (F-492); mechanisms are
  refused on both constraint paths (F-495, F-496); factors are returned on sign-asymmetric spectra (F-642, F-650).
* Harmonic response by mode superposition: modal damping, static correction, effective mass, force harmonics (F-399);
  the correction covers free bodies and the transformation path (F-491). Direct harmonic base motion there (F-619).
* Damped and complex modes on a modal subspace (F-414), with residual vectors for boundary dampers (F-532). Radiation
  impedance of a baffled plane in `rcfd-post` (F-579).
* Random vibration by modal superposition for force and base-acceleration spectra, root-mean-square von Mises stress
  (F-417); complex cross-spectra and coherence (F-567); spectral moments m0, m1, m2, m4 of a stress component (F-589).

### Rotating parts

* Coriolis coupling: gyroscopic matrix, whirl modes, Campbell table; a cyclic sector equals the full rotor's 252 whirl
  frequencies within 3e-13 (F-499). Sparse whirl solver: 20 064 reduced unknowns in 6.3 s (F-506); see also F-520.
  Stationary-frame frequencies (F-507). Stability flags above the loss of stiffness (F-510, F-511).
* Further registered checks: F-498, F-554, F-575, F-686, F-657 (spinning shell disc), F-679 (spinning Beam2 ring).
* Rotor on bearings (base item F-416 partial): speed-dependent bearing tables (F-563); unbalance-response peaks and
  Campbell crossings, peak within 4.4e-9 of the damped Jeffcott closed form (F-564).

### Nonlinear statics and dynamics

* Newton driver (base item F-397 partial): the roll-back of a failed attempt is a registered check (F-497). Opt-in
  retry with an indefinite tangent after a refused factor, reporting the inertia of every accepted state (F-780).
* Follower pressure with its load stiffness, and arc-length continuation (Crisfield): two-bar truss limit load within
  6.2e-4, and modes about a nonlinear equilibrium (F-424); a follower-only reference load and an opt-in unsymmetric
  envelope LU (F-572); a hyperelastic sphere through its pressure maximum (F-649). Hinged cylindrical roof, Shell4 16
  x 16: limit load 2222.4 N against the published 2226.3 N of Sze, Liu and Lo (2004), 1.8e-3 below (F-610).
* Flutter and buckling about a nonlinear state with the full unsymmetric contribution tangents (F-676, F-677); a
  follower tip force on Beam2 (F-665); the pencil eigensolver reports normwise backward errors (F-664).
* Linear transient on the transformation path and with prescribed motion histories (F-490, F-545). Nonlinear
  transient: opt-in residual reference with inertia terms (F-631, F-691), rounding-floor diagnosis (F-571), and the
  energy-momentum step with neo-Hookean and Mooney-Rivlin energies (F-611).

### Contact, fatigue and fracture

* Contact (base item F-427 partial): opt-in supernodal factor in the linear contact driver, hub fit of 96 046 free
  unknowns in 82.0 s against 1456.6 s (F-615). Traction recovery of the mortar contactor (F-638). Total Lagrangian
  contact: curvature-free fallback tangent (F-639), lifted contactor (F-608), line search (F-684), a Newton order
  reading (F-600). The pressure variation of the one-layer interference fit is a discretisation error (F-599).
* High-cycle fatigue: Basquin, power and table curves, Goodman, Gerber and Soderberg corrections, rainflow counting,
  Miner sum, narrow-band rate; cantilever life 0.9527 of the hand value (F-413, F-534).
* Fracture (base item F-429 partial): interaction integral for K_I, K_II, K_III; embedded elliptical crack within
  8.4e-3 of Irwin at 13 stations (F-632); closed fronts and mirror-plane stations (F-689).

### Linear solvers and performance

* Envelope `L D L^T` after reverse Cuthill-McKee, factor once, solve many; about 1e5 unknowns in minutes (F-373).
* CG preconditioners: incomplete Cholesky with one level of fill, 58 iterations against 156 of block Jacobi on 4704
  unknowns (F-460); multigrid smoother for Tet10, 23 iterations at 121 680 unknowns (F-477); smoothed aggregation with
  rotation unknowns for beams and shells (F-629) and a vertex-patch smoother for thin plates (F-641, F-659).
* Static solves on ranks of `rcfd-dist` and on a CUDA device: Jacobi CG on 304 842 unknowns in 0.60 s, 4.50 s on 16
  threads (F-433).

### Input, output and coupling with the flow solver

* `.vtu` output of finite-element cells with point and cell data (F-377); Shell4 results as quad cells with resultants
  (F-521); a beam result in another layout is refused (F-516).
* A [solid case file](solid-case-file.md) run with `method = cg` and each of the six preconditioner keys reaches 1e-10
  and agrees with the direct run within 1.2e-12 (F-526).
* One-way coupling: fluid face forces map to a solid surface set and are solved statically; `StaticSolid` is a
  co-simulation participant without mass, for mock fluids and quasi-static use only; no flow solver is coupled (F-385,
  F-463, F-469). Coupled frequency on non-matching interfaces (F-502); opt-in bilinear projection (F-504).

### Not there or partial

Not built (todo):

* No CLI subcommand and no MCP tool (F-405). No `[solid]` table in the flow case (F-404). The case file covers none of
  the intermediate and advanced tiers (F-403, F-435; single keys F-533, F-565, F-605, F-616, F-621, F-625, F-652,
  F-688, F-660, F-842, F-509).
* No fan-rotor demonstration (F-406, F-436) and no public coupled benchmark (the elastic flag, F-434).
* Shells on the linear dynamic paths (F-630). The existing smoothed-aggregation CG still refuses structures (F-640).
* Held or waiting: F-462, F-678, F-658, F-694 (default or schema changes), F-798 (6.35 roof), F-655 (heavy record).

Built, but the item is partial (a registered check missed, or the maintainer has not closed it):

* Verification: Tet4 L2 order 1.7925 against 1.8 on the pair 8 to 16 (F-378). The direct-solve residual limit 1e-14 is
  below the representability floor on all twelve cases (F-455). Error estimate (F-432, F-568, F-695): the hole
  criterion misses (18, 44 and 28 of 48).
* Hex20 (F-389): cantilever 1.8 percent and bore hoop stress 1.17 percent under the acceptance's own set-ups. Shell3
  (F-419): the one-diagonal cylinder is met only by the opt-in pressure of F-540; mixed Shell3 plus Shell4 meshes with
  the Allman membrane are refused (F-539).
* Modal analysis (F-383) waits for acceptance of an amendment. Sparse `L D L^T` (F-431): the 3e5-unknown record was
  not run, "nnz(L) would be 13.6 GB". Supernodal factor (F-529, F-594): acceptance only.
* CG (F-381): IC(0) needs 0.545 of block Jacobi's iterations against 0.5. Multigrid on Tet10 (F-402, F-470): 79 and 31
  iterations against the limit 30. Case file (F-386): on the Tet10 cantilever every CG preconditioner stops at 5.7e-10
  to 6.2e-10 against 1e-10; the fan blade and the centrifugal load in the case file are not built (F-593).
* Buckling (F-395): second factor 2.0135e-2 against 2e-2. Rotating modes (F-396): flap-lag misses 1e-4. Transient
  (F-398): generalised-alpha decay misses at omega dt = 1e3. Direct harmonic (F-412): `HarmonicResponse::stress` omits
  the loss part. Craig-Bampton (F-430, F-552, F-559): harmonic clauses miss without F-603.
* Newton driver (F-397): linear-limit clause met only after an amendment. J2 (F-422): residual hoop stress 2.6 percent
  against 2, residual order 1.27 to 1.44 against 1.8. Nonlinear transient (F-423): the free spinning bar misses,
  "nonlinear instability of the trapezoidal rule". Hyperelastic (F-425): small-strain clause 4.7e-6 against 1e-6.
* Contact (F-427, F-601, F-623): no `[[contact]]` case-file table; Hertz on Hex20 and Tet10 point contactors stalls at
  a gap of 5e-7 to 8e-6 against 1e-8; the mortar contactor misses the contact radius from nodal multipliers and the
  Hertz problem on the total Lagrangian body. Press fit (F-604): plane-strain clause 8.6e-2 against 2e-2.
* Ties (F-426): the unit-weight joints miss (2.25 and 2.36 percent) and the node-to-surface tie's local stress error
  does not fall. Shell cyclic boundary 2:3: 4.2e-2 against 2e-2 (F-648).
* Coupling: deck loads (F-384) refuse Tet, Tet10 and Hex20 surfaces. Dynamic participant (F-400): frequency change
  6.3e-3 against 1e-3. Two-way coupling (F-401) and aerodynamic damping (F-421): estimates missed.
* Rotor (F-416, F-562): whirl "still refuses a massless damped unknown". Coriolis split table (F-415): cyclic models
  refused. Sector split-mode estimates miss (F-569, F-584, F-612).
* Beams (F-514): centrifugal, nodal body force and thermal loads are refused. Warping (F-635): no warping inertia, no
  mode superposition. Beam2 in a cyclic sector: taken with the shells option alone (F-672, ADR 0744); `full_rotor` and
  the whirl paths keep their own options, beam ends on interpolated or axis boundaries are refused. Hyperelastic
  material on Beam2 and shells: taken by default on the linear paths as its linearised material (F-666, ADR 0746;
  `set_hyperelastic_structures` is without effect); a temperature load with it, a hyperelastic ply and the corotated
  shells are still refused.
* Shells: temperature loads have rounding misses and no per-ply temperatures (F-525, F-527, F-687); the
  energy-momentum step, `solve_in_time` and `buckling_factors` still refuse geometrically nonlinear shells (F-585).
* Creep on B-bar and F-bar Hex8 (F-580, F-587, F-597, F-637): growth bound 6.0e-3 against 2e-3; no finite-strain
  reference built.
* Incompatible Hex8 "rule L" under a punch (F-719 to F-797, F-846): on distorted meshes the default local update
  fails; on the 16^3 block rule L loses positive definiteness at load factor 0.966 (F-747) and "why one element goes
  soft is open" (F-763); the library arc-length trace without the warm start is lost short of the fold (F-797); with it the
  fold is traced, test-only (done: F-841, F-843, F-845).
* Broadband fatigue (F-590): the Benasciutti-Tovo rate misses 1e-2 (1.646e-2, 3.106e-2). Fracture (F-682):
  coarse-ellipse K 2.29e-2 against 2e-2; no mixed-mode elliptical or non-planar fronts. Flutter (F-653): the pair
  residual 1e-8 was missed (2.0e-8 and 7.3e-6). Radiated sound (F-418, F-551): no unbaffled or curved radiators.

## What was built

<!-- process-report:built begin (generated by scripts/process_report.py --write; do not edit) -->
The 161 items of the line "Structural solver" that are done, from `docs/roadmap.json`:

- F-370: Crate rcfd-fem: the data model of a solid model and the structured builders
- F-371: Solid mesh from the existing meshes: polyhedral mesh, tetrahedral mesher output, patches as surface sets, curved edges
- F-372: Element library: linear tetrahedron, trilinear hexahedron, quadratic tetrahedron; isotropic elasticity matrix
- F-373: Sparse direct solver for symmetric positive definite matrices (envelope L D L^T after reverse Cuthill-McKee)
- F-374: Assembly, displacement constraints, nodal forces and the linear static solve; patch test
- F-375: Surface and body loads: traction, pressure, gravity, centrifugal load of a rotating part, body force field
- F-376: Stress and strain recovery: integration-point, element and nodal values; von Mises and principal stresses
- F-377: VTK output of a solid result: finite-element cells with point and cell data
- F-379: Closed-form benchmarks: cantilever bending and the thick-walled cylinder under pressure
- F-380: Thermal strain: nodal temperature field, expansion coefficient, thermal load and stress
- F-382: Eigensolver for K x = lambda M x: dense Jacobi and subspace iteration on the direct factorisation
- F-385: One-way coupling 2: fluid face forces in process, and a static solid as a co-simulation participant
- F-387: Demonstration on a fan blade: static response to centrifugal and pressure load, and the modes at rest; user page
- F-388: Scaffold of the intermediate tier: module layout, model extension data, element-trait extension points
- F-390: Hex8 without locking: mean-dilatation (B-bar) and incompatible-mode formulations, opt-in per element block
- F-391: Six unknowns per node: rotations in the DofMap, nodal moments, and a two-node shear-flexible beam element
- F-392: Four-node shell element (flat facet, assumed shear strain): plates, housings and thin blades as surfaces
- F-393: Linear multi-point constraints by a transformation, rigid sets with a reference point, elastic supports
- F-394: Cyclic symmetry: one sector of a rotor for statics and for modal analysis by harmonic index
- F-399: Harmonic response by mode superposition: modal damping, static correction, effective mass, force harmonics
- F-409: Orthotropic elasticity with material axes, orthotropic expansion, and temperature-dependent properties from tables
- F-410: Linear viscoelasticity: Prony series in time, complex modulus in frequency, time-temperature shift
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
- F-465: Coupled step: a failing fluid in a later iteration as a registered check
- F-469: Coupled step: failing-participant checks with a start state that does not use the restores at the start of an iteration
- F-477: Elasticity multigrid on quadratic tetrahedra: a smoother that brings Tet10 within twice the Hex8 count
- F-484: Hex20 face loads through the model path
- F-485: Hex20 cantilever with the beam-theory support and the bore stress on a refined mesh
- F-488: Zero modes on the transformation: the eigenvalue measure against the shift and the explicit-shift report
- F-489: Cyclic symmetry: harmonic loads and rigid sets at a non-zero harmonic index
- F-490: Transient integrator on the transformation path and with prescribed motion histories
- F-491: Static correction of the harmonic response for free bodies and on the transformation path
- F-492: Buckling factors of the knife-edge column against the solid's closed form (ADR 0390 known miss)
- F-495: Refuse a mechanism in buckling on the transformation path (ADR 0398 known miss)
- F-496: Refuse a mechanism that is not rigid in buckling on the prescribed path
- F-497: Nonlinear driver: roll-back of a failed attempt as a registered check
- F-498: Flap-lag identity of a rotating solid beam: the acceptance restated
- F-499: Coriolis coupling of a rotating part: gyroscopic matrix, whirl modes and the Campbell table
- F-502: Coupled frequency of the dynamic solid on non-matching interfaces: face refinement and map
- F-504: Opt-in bilinear projection on quadrilateral interface elements
- F-506: Sparse whirl solver for large rotating parts
- F-507: Stationary-frame frequencies of a cyclic whirl table
- F-510: Odd-index instability of a rotating part on the model path
- F-511: Whirl stability of a cyclic sector above the loss of stiffness
- F-516: Beam output: refusal of a result with the model's count of unknowns in another layout
- F-518: Beams on the transformation: a check that observes the entry refusal before the transformation is built
- F-520: Accurate far-mode vectors of the sparse whirl state solver
- F-521: Shell results in the solid output: VTK quad cells, rotations and the resultants as cell data
- F-522: Shells on the transformation path and body loads on shells: rigid sets and supports on shell nodes, gravity and centrifugal loads
- F-523: Deck pressure loads on non-parallelogram faces
- F-524: Rotating plane-stress disc of Shell4 facets against the closed form
- F-526: Iterative solve of a solid case file to its tolerance, and the box bound
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
- F-591: Laminated Shell4 under body loads and pre-stress (gravity, centrifugal, nodal body force, buckling and spin softening of a layup)
- F-592: A temperature history on the quasi-static driver and the nonlinear transient integrator
- F-595: Coordinate nested dissection in the eigensolvers
- F-596: The incompatible-modes Hex8 at finite strain (enhanced assumed strain, internal modes condensed in the Newton loop)
- F-598: Warping degree of freedom for Beam2 (14-DOF element) and the I-section lateral-torsional buckling
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
- F-616: Case-file keys for superelements and interface enrichment
- F-617: A thermal predictor for the attempts of the nonlinear paths with a temperature field
- F-619: Direct harmonic base motion and reference response on the transformation path
- F-626: Warping unknown on the static solve, modes and pre-stressed modes
- F-627: Monosymmetric thin-walled sections for the warping element
- F-628: Shells in the cyclic constraint
- F-629: Six-unknown nodes in the smoothed-aggregation CG
- F-630: Shells on the linear dynamic paths
- F-631: Residual reference of the Newton driver in free vibration
- F-632: Domain integral on a front with varying J, and mode separation
- F-634: Shell documentation note on pressure-loaded thin curved shells on unstructured meshes
- F-636: Bore-zone witness for the finite-strain growth of the B-bar creep cylinder
- F-638: Pressure recovery of the mortar contactor
- F-639: Curvature-free fallback of the total Lagrangian contact body
- F-641: Smoothed aggregation for thin plates and shells
- F-642: Buckling eigensolver on sign-asymmetric spectra
- F-643: Load height and non-uniform moment for monosymmetric warping beams
- F-644: Hyperelastic linear paths: near-incompressible modes, order-free damped modes on two materials, multi-material sites under mutation
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
- F-669: Buckling and harmonic response about a nonlinear hyperelastic state
- F-670: Load height on the pre-stressed warping modes and harmonic path
- F-672: Beam2 elements in the cyclic constraint
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
- F-774: The corner element of rule L at the punch corner and the equilibrium past the predictor
- F-780: Driver retry with the indefinite tangent after a refused factor
- F-796: The end of the subcritical branch of rule L's 16^3 punch path at amplitude 2.2e-4
- F-841: The subcritical branch of rule L's 16^3 punch path past amplitude 1.65e-4 with the warm start of the local modes
- F-843: The fold of rule L's 16^3 punch branch by an extended system with warm-started local modes
- F-844: The local mode update of the incompatible Hex8 as a default, compared on the ADR 0658 set-ups and on rule L's 16^3 punch branch
- F-845: The library arc-length driver through rule L's 16^3 fold with warm-started local modes
- F-847: The element entry points of the incompatible Hex8 given a committed state with the nine modes of the warm start
- F-850: Four supporting checks that reader trial 1 found weaker than their record says
- F-889: Case-file keys for the cutoff, the load's residual vector and the load patterns of the reduction
- F-975: whirl_stability of the cyclic beam wheel sector converges
<!-- process-report:built end -->
