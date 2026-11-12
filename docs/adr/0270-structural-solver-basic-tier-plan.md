# ADR 0270: Structural solver, basic tier: scope, architecture and the layout of ADRs 0271 to 0288

Status: **PROPOSED 2026-11-12** (planning only; no solver code written, nothing run). Written at main 2fa39daf on the branch
`structural-basic-plan`. Not yet accepted by the maintainer. The decisions marked M1 to M7 in section 5 are open and are the
maintainer's; everything else is a proposal that the implementation agents' registrations may refine.

The maintainer wants a structural solver, planned in three tiers: basic, intermediate, advanced. This document plans the basic
tier: linear elastostatics and linear modal analysis of isotropic solids on tetrahedral and hexahedral meshes, with the loads a
fan blade sees and one-way coupling from the flow solver. It fixes the tier boundaries (a first cut for the planners of the next
two tiers), where the code lives and the interfaces later tiers build on, and it lays the work out as eighteen ADRs, 0271 to 0288,
each one work package for a single implementation agent. Each of those has a stub file here with status PLANNED; the
registration with its limits is written by the implementation agent before any code (`AGENT_RULES.md` rule 1).

It is for the maintainer (section 5 first), for the lead who enters the roadmap items (section 7), and for the planners of the
intermediate and advanced tiers (sections 1 and 2).

Rules and parents:

* The house rules of `w49i-work/AGENT_RULES.md` apply to every work package: registration before code with limits fixed in
  advance, known-miss records, defaults bit-identical, clean-room, no new third-party crate, no unsafe code, no golden or
  hash-table edit without the maintainer.
* Parents: [fan project breakdown](../fan-project-breakdown.md) item E5 (blade and housing structural response, modal analysis,
  FSI: "missing, deferred by decision") and item E4 (one-way load export, F-237); [fan milestone](../milestone-fan.md) ("Deferred
  by decision (2026-10-18)": the maintainer's request for this plan is taken as lifting that deferral, to be confirmed, M7);
  `docs/product-suites.md` S-01 and S-02.
* What the solver connects to: the load export of F-237 ([ADR 0244](0244-f237-registration-heat-rate-load-export.md), ADR 0051;
  `rcfd-post::structural`, `rcfd-io::nastran`), the co-simulation participant interface of F-139
  ([ADR 0256](0256-f139-registration-cosimulation-participant.md), `docs/cosimulation-protocol.md`, `rcfd-fv::cosim`), the
  strong coupling of F-251 ([ADR 0250](0250-f251-registration-strong-fluid-body-coupling.md)) and the quasi-Newton update of
  F-361 ([ADR 0261](0261-f361-registration-quasi-newton-coupling.md)). ADR 0262 (interface mapping, F-362) does not exist yet;
  F-362 is todo.

## What was read (disclosed)

* **Read:** `AGENT_RULES.md`; `docs/fan-project-breakdown.md`, `docs/milestone-fan.md` (head and limits), `docs/roadmap-overview.md`,
  `docs/architecture.md`, `docs/cosimulation-protocol.md`; ADR 0244 (to D2), ADR 0130 (head, for the style), ADR 0257 (the reading
  on optional tables and goldens); the public signatures of `rcfd-post/src/structural.rs`, `rcfd-io/src/nastran.rs`,
  `rcfd-fv/src/cosim.rs`, `rcfd-linalg` (all modules), `rcfd-mesh/src/mesh.rs`, `gen.rs`, `tetmesh.rs`, `rcfd-io/src/gmsh.rs`
  and `vtk.rs`; the `Cargo.toml` of every crate (the dependency graph); `crates/rcfd-cli/tests/workspace_graph.rs`; the roadmap
  entries F-139, F-237, F-242, F-251, F-361, F-362 and the categories of `scripts/roadmap_progress.py`.
* **Not read:** the bodies of ADR 0250 and 0261 (their roadmap notes were read instead); function bodies of the files above
  beyond the lines needed for the findings below. Nothing was built or run.
* **Findings that shape the plan:**
  1. `rcfd_mesh::Mesh` is a face-addressed polyhedral mesh: it has no element connectivity (nodes of a cell in an element
     order). `tetmesh::TetMesh` has `cells: Vec<[usize; 4]>`. The Gmsh reader keeps only the corner nodes of higher-order
     elements. A finite-element mesh type is therefore new, with converters from both (ADR 0271, 0272).
  2. `rcfd-linalg` has `CsrMatrix` (with `from_triplets`, `permute_symmetric`), `cg`, `Jacobi`, `Ilu0` and `Chebyshev` on CSR,
     `Ic0` on the finite-volume LDU matrix only, `rcm_order`, and a scalar smoothed-aggregation AMG with a piecewise-constant
     tentative prolongator (no rigid-body modes). There is no sparse direct solver and no eigensolver (the only dense LU is
     inside `amg.rs` and `rcfd-reactor`). A structural stiffness matrix needs both a robust solve and an eigensolver:
     ADRs 0274, 0282, 0283.
  3. `rcfd-io::vtk` writes the polyhedral mesh with cell data only: no point data, no finite-element cell types (ADR 0278).
  4. `workspace_graph.rs` panics on a crate name it does not know. A new crate needs one line of that existing test (M1).
  5. The mapping of F-237 is by parameter space (plane, tube) onto a shell surface; `cosim::InterfaceMap` takes given weights.
     Neither builds weights from two arbitrary surfaces (F-362, todo). The basic tier does not add that.

## 1. Scope of the basic tier and the tier boundaries

### 1.1 Basic tier (this plan)

* **Physics:** linear elasticity, small strain, isotropic material (`E`, `nu`, density, expansion coefficient), statics and
  undamped free vibration.
* **Elements:** 3-D solids: linear tetrahedron (Tet4), trilinear hexahedron with full integration (Hex8), quadratic tetrahedron
  (Tet10). Tet10 is in the basic tier, not later: the tetrahedral mesher is the only route to a blade mesh, and the linear
  tetrahedron locks in bending, so without Tet10 the tier could not meet its own cantilever check or say anything about a blade.
* **Constraints:** prescribed displacement per node and component (zero or not), which covers clamps, rollers and symmetry planes.
* **Loads:** nodal force, surface traction, pressure, gravity, centrifugal load of a part rotating at constant speed, body force
  field, thermal strain from a nodal temperature field.
* **Solve:** sparse direct solver (the default, to a few times 1e4 unknowns) and preconditioned CG beyond it.
* **Results:** displacement, reactions, strain and stress at integration points, element and nodal values, von Mises and
  principal stresses, strain energy; `.vtu` output with point and cell data.
* **Modal analysis:** consistent mass, the lowest modes of `K x = lambda M x`, free or constrained, at rest.
* **Coupling, one way:** loads from the deck that F-237 writes, and fluid face forces in process through the weights of
  `cosim::InterfaceMap`; a static solid behind the `cosim::Structure` trait, checked with a mock fluid only.
* **Entry point:** a solid case file of its own and a library function that runs it. No flow case key, no CLI subcommand, no MCP
  tool (section 5, M3).
* **Verification:** patch test, manufactured solution with observed orders, closed forms (uniaxial, hanging and rotating bar,
  thin ring, cantilever, Lame cylinder, thermal states, bar and beam frequencies). No NAFEMS or other published target value is
  used: none is in the repository (house rule 4: reference values come from the repository or a closed form), and the closed
  forms cover the same ground. A planner who can cite the NAFEMS values from a public source may add them later as extra records.

Departures from the brief's first cut, and why:

1. **Thermal strain is added** (ADR 0281, size S): F-237 already exports nodal temperatures, and a one-way coupling that reads
   `PLOAD4` but must drop `TEMP` would be half a connection.
2. **A static co-simulation participant is added** (ADR 0286) although strong FSI is intermediate: it costs little on top of the
   in-process load route, and it fixes the participant interface before the intermediate tier builds a dynamic one on it. It is
   checked with a mock fluid only. If the maintainer prefers, that clause moves to the intermediate tier (M5).
3. **Hex20, wedges and pyramids are left out**; so is a locking-free Hex8. The three elements above cover every mesh the
   repository can make; the others go with the element work of the intermediate tier.
4. **The entry point is a separate solid case file**, not a table in the flow case (reason in section 2.6).

### 1.2 Intermediate tier (proposal for its planner)

Everything that needs a new unknown type, a nonlinear solve or time:

* structural elements: shells (thin blades and housings are shells; a solid-element blade is expensive) and beams, which need
  six unknowns per node and so exercise `DofMap`; Hex20, wedge, pyramid; Hex8 without shear and volumetric locking (B-bar or
  enhanced strain); near-incompressible materials (mixed formulation);
* constraints beyond single-point: multi-point constraints, tied (non-matching) meshes, rigid links, cyclic symmetry (one blade
  sector of a rotor), elastic supports;
* rotating-part effects on stiffness: stress stiffening and spin softening, pre-stressed modal analysis, the Campbell diagram
  of a blade, linear buckling;
* geometric nonlinearity (large displacement and rotation, Newton iteration, follower pressure), and plasticity without contact
  (J2 with isotropic hardening); orthotropic linear elasticity; temperature-dependent properties;
* dynamics: transient (Newmark, generalised-alpha), Rayleigh and modal damping, harmonic response by mode superposition, response
  to the unsteady blade loads of fan breakdown item E3;
* two-way coupling with the flow solver: a dynamic structural participant behind `cosim::Structure` coupled to `cosim::FlowWall`,
  geometric interface mapping (F-362), the transport of the protocol; a `[solid]` table in the flow case, the CLI and MCP tools;
* scale and quality within one machine: aggregation AMG with rigid-body modes as near-null space, a fill-reducing ordering for
  the direct solver, superconvergent stress recovery and an error estimate, mesh adaptation driven by it.

### 1.3 Advanced tier (proposal)

Contact (bonded, frictionless, frictional), composites and laminates with their failure criteria, hyperelastic, viscoelastic
and creep materials, fatigue (stress-life, cycle counting) and fracture parameters, flexible-rotor dynamics (gyroscopic terms,
joined to the rigid-rotor model of `rcfd-post::rotordyn`), substructuring and component mode synthesis, explicit dynamics,
distributed-memory and GPU solves (on `rcfd-dist` and `rcfd-gpu`), coupled thermo-mechanical and other coupled-field problems,
topology optimisation.

The boundary between intermediate and advanced that I am least sure of is plasticity (it is cheap once the nonlinear solve
exists, but its verification is not) and cyclic symmetry (needed early for a fan rotor, so placed in intermediate).

## 2. Architecture decision

### 2.1 Where the code lives

A new crate **`rcfd-solid`**, depending on `rcfd-core`, `rcfd-mesh` and `rcfd-linalg` only. It sits in layer 2 of
`docs/architecture.md` (beside `rcfd-gpu`, `rcfd-dist`), below `rcfd-io`, `rcfd-fv` and `rcfd-post`:

```text
rcfd-core
rcfd-mesh, rcfd-linalg            (linalg gains: skyline.rs, eigen.rs, two preconditioners in precond.rs)
rcfd-solid  -> core, mesh, linalg (new; no io, no fv)
rcfd-io     -> core, mesh         (vtk.rs gains a finite-element writer from plain arrays; no dependency on solid)
rcfd-fv     -> ...                (unchanged; cosim traits stay here)
rcfd-post   -> ..., solid         (new dependency: solid_output.rs, solid_loads.rs, solid_coupling.rs)
rcfd-api    -> ..., solid         (new dependency: solid_case.rs)
```

The graph stays acyclic. Reasons for this place:

* The solver must not depend on the finite-volume crate: nothing in elasticity needs it, and a low layer lets `rcfd-fv`,
  `rcfd-post` and `rcfd-api` all use the solid later (a flow case with a `[solid]` table, intermediate tier) without a cycle.
* It must not depend on `rcfd-io` either: the crate returns plain arrays, and the file writers live with the other writers. The
  price is that the glue (write a result, read a deck, be a co-simulation participant) lives in `rcfd-post`, which already holds
  the structural load export and already depends on `rcfd-io` and `rcfd-fv`.
* Generic numerics (direct solver, eigensolver, preconditioners) go into `rcfd-linalg`, not into the new crate: the finite-volume
  side can use them, and they are tested on matrices with closed-form answers there.

Alternatives considered: (a) modules inside `rcfd-fv` (no new crate, no edit of the graph test; rejected: it puts a finite
element solver into the finite-volume crate and every later tier with it); (b) `rcfd-solid` above `rcfd-io` and `rcfd-fv` in
layer 4 (it could then write files and implement `cosim::Structure` itself; rejected: `rcfd-post` could no longer depend on it
and the coupling glue would have to move to `rcfd-api`); (c) a separate coupling crate for the glue (cleaner, deferred: decision
M5, natural when the intermediate tier adds the dynamic participant).

### 2.2 Conventions (fixed here for all tiers)

* SI units. Global Cartesian axes. Displacements are measured from the reference configuration (as in the co-simulation protocol).
* Unknowns are numbered through a `DofMap` (node, component) -> equation; the basic tier has three translations per node, but no
  code outside `DofMap` assumes three.
* Symmetric tensors are stored as six numbers in the order xx, yy, zz, xy, yz, zx; strains carry engineering shear
  (`gamma = 2 eps`), stresses do not.
* A pressure is positive when it presses on the solid. A surface set's normal points out of the solid. A fluid force is the force
  of the fluid on the structure (protocol convention).
* Determinism as in `rcfd-linalg`: results are bit-identical for any thread count; assembly order is fixed by element index.
* Every refusal is an error with the element, node, set or key it concerns; nothing is repaired silently.

### 2.3 Modules of `rcfd-solid`

Declared once by ADR 0271 (as stubs where the content comes later), so that no later ADR edits `lib.rs`:

| Module | Content | Filled by |
|---|---|---|
| `mesh` | `SolidMesh`, `ElementKind`, sets, validation | 0271 |
| `model` | `Material`, `Constraint`, `Load`, `SolidModel`, `SolidResult`, `DofMap` | 0271 |
| `builders` | box of hexahedra, of tetrahedra, quadratic upgrade | 0271 |
| `convert` | from `rcfd_mesh::Mesh` and `TetMesh`, patches to sets, curved edges | 0272 |
| `element`, `quadrature`, `material` | shape functions, rules, `B`, `K_e`, `D` | 0273 |
| `assemble`, `constraint`, `solve` | global matrices, elimination, static solve | 0275 (solver choice 0282) |
| `loads` | nodal, surface and body loads | 0275, 0276 |
| `recover` | strain, stress, invariants, energy | 0277 |
| `thermal` | thermal strain load | 0281 |
| `mass`, `modal` | mass matrix, modal analysis | 0284 |

### 2.4 Data model

* **`SolidMesh`**: `nodes: Vec<Vec3>`; element blocks, each of one `ElementKind` (`Tet4`, `Hex8`, `Tet10`; the enum is
  non-exhaustive so that shells and beams can be added) with a flat connectivity array in the element's own node order; named
  node sets, element sets and surface sets (a surface set is a list of (element, local face)). It is a type of its own because
  the finite-volume `Mesh` has no element connectivity; converters keep patch and zone names.
* **`Material`**: an enum with one variant today, `IsotropicElastic { e, nu, density, expansion }`, assigned per element set. The
  constitutive call is `stress(strain, state) -> (stress, tangent)` with an empty state, so a later material with history fits
  the same call.
* **`Constraint`**: `Displacement { set, component, value }`. Later tiers add variants (multi-point, cyclic); the reduction of the
  system goes through one function, `constraint::reduce`, which a transformation-matrix method can replace.
* **`Load`**: plain data, readable from a file: `NodalForce`, `Traction`, `Pressure`, `Gravity`, `Centrifugal { axis, origin,
  omega }`, `BodyForceNodal`, `Temperature { nodal, reference }`. A function-valued body force exists only as a library helper
  for tests.
* **`SolidModel`**: mesh, materials, constraints, loads. **`SolidResult`**: displacement per node, reactions, stress and strain
  at integration points, solver statistics; derived fields come from `recover`.
* **`Element` trait**: `stiffness`, `mass`, `strain_at`, `face_load`, `body_load` for one element given its node coordinates and
  material. The intermediate tier adds `internal_force` and `tangent` for nonlinear elements beside these, not instead.

### 2.5 Interfaces that later tiers and the co-simulation participant use

1. **Operators:** `assemble::stiffness(&SolidModel) -> CsrMatrix`, `mass::consistent(&SolidModel) -> CsrMatrix`, and
   `constraint::reduce`. A transient or nonlinear solver of the intermediate tier is built from these three.
2. **Interface surface:** for a surface set, its nodes, its faces, the nodal load of given face forces or pressures, and the
   displacement of its nodes from a result. This is exactly what `cosim::Structure` needs (`node_coordinates`, `advance` with one
   force vector per node, returning one displacement per node), and what the deck route needs.
3. **Participant:** `rcfd-post::solid_coupling::StaticSolid` implements `cosim::Structure`. The intermediate tier's dynamic
   participant implements the same trait with the same interface-surface functions; nothing in `rcfd-fv::cosim` changes.
4. **Linear algebra:** `rcfd_linalg::skyline` (factor once, solve many, signs of `D`) and `rcfd_linalg::eigen`. The transient and
   buckling work of the intermediate tier reuses both.
5. **Files:** `rcfd_io::vtk` finite-element writer from plain arrays; the solid case file of `rcfd-api::solid_case`.

### 2.6 The case-file entry point: in the basic tier, as a file of its own

A minimal entry point belongs in the basic tier, last (ADR 0287): without it the solver can only be driven from Rust tests, and
the fan demonstration and any use by the assistant would have nothing to call. It is a **separate TOML file for a solid model**,
read by a new module of `rcfd-api`, not a table in the flow case:

* A flow case with a `[solid]` table means a coupled run, which is intermediate. A solid-only run through the flow case's
  `Simulation` path would need changes to `sim.rs`, a shared solver file.
* ADR 0257 established by reading and by its check K0 that an optional table can be added to the flow schema with no hash table
  or golden edited. That route therefore stays open for the intermediate tier and costs nothing now.
* A CLI subcommand changes the CLI help golden and an MCP tool changes the tool-list golden. Both are left out and listed as the
  maintainer's step (M3). Until then the file is run through the library function.

## 3. The ADR list

Eighteen work packages, ADR 0271 to 0288 (0289 is left free). Each is sized for one implementation agent working alone under
`AGENT_RULES.md`: S is about a session, M a few hours; nothing is L (the two that were, the mesh model and the loads, are
already split). Each has a stub file in this directory. The acceptance texts are drafts in the roadmap's form; the
implementation agent's registration fixes every limit before its code.

| ADR | Package | Title | Size | Depends on |
|---|---|---|---|---|
| [0271](0271-rcfd-solid-crate-and-solid-data-model.md) | SB-1 | Crate rcfd-solid: the data model of a solid model and the structured builders | M | F-001, F-010, F-012 |
| [0272](0272-solid-mesh-from-existing-meshes.md) | SB-2 | Solid mesh from the existing meshes: polyhedral mesh, tetrahedral mesher output, patches as surface sets, curved edges | M | 0271, F-010, F-012, F-132 |
| [0273](0273-element-library-tet4-hex8-tet10.md) | SB-3 | Element library: linear tetrahedron, trilinear hexahedron, quadratic tetrahedron; isotropic elasticity matrix | M | 0271 |
| [0274](0274-sparse-direct-solver-for-spd-matrices.md) | SB-4 | Sparse direct solver for symmetric positive definite matrices (envelope L D L^T after reverse Cuthill-McKee) | M | F-025, F-026 |
| [0275](0275-assembly-displacement-constraints-and-static-solve.md) | SB-5 | Assembly, displacement constraints, nodal forces and the linear static solve; patch test | M | 0271, 0273, 0274, F-025 |
| [0276](0276-surface-and-body-loads-with-centrifugal-load.md) | SB-6 | Surface and body loads: traction, pressure, gravity, centrifugal load of a rotating part, body force field | M | 0275 |
| [0277](0277-stress-and-strain-recovery-von-mises-principal.md) | SB-7 | Stress and strain recovery: integration-point, element and nodal values; von Mises and principal stresses | S | 0275 |
| [0278](0278-vtk-output-of-solid-results.md) | SB-8 | VTK output of a solid result: finite-element cells with point and cell data | S | 0277, F-070 |
| [0279](0279-manufactured-solution-and-convergence-orders.md) | SB-9 | Verification by a manufactured solution: observed order of displacement and stress for the three elements | M | 0276, 0277 |
| [0280](0280-closed-form-benchmarks-cantilever-and-thick-cylinder.md) | SB-10 | Closed-form benchmarks: cantilever bending and the thick-walled cylinder under pressure | M | 0272, 0276, 0277 |
| [0281](0281-thermal-strain-load.md) | SB-11 | Thermal strain: nodal temperature field, expansion coefficient, thermal load and stress | S | 0276, 0277 |
| [0282](0282-preconditioned-cg-for-elasticity.md) | SB-12 | Iterative route for larger models: CG with block Jacobi and incomplete Cholesky on the stiffness matrix | M | 0275, F-026 |
| [0283](0283-generalised-symmetric-eigensolver.md) | SB-13 | Eigensolver for K x = lambda M x: dense Jacobi and subspace iteration on the direct factorisation | M | 0274 |
| [0284](0284-mass-matrix-and-modal-analysis.md) | SB-14 | Mass matrix and linear modal analysis of a solid model | M | 0275, 0283 |
| [0285](0285-one-way-coupling-from-the-load-deck.md) | SB-15 | One-way coupling 1: loads of the F-237 deck applied to the solid surface | M | 0272, 0278, 0281, F-237 |
| [0286](0286-in-process-loads-and-static-structural-participant.md) | SB-16 | One-way coupling 2: fluid face forces in process, and a static solid as a co-simulation participant | M | 0275, 0278, F-139 |
| [0287](0287-solid-case-file-and-library-run-entry.md) | SB-17 | A solid case file of its own and the library entry that runs it | M | 0272, 0278, 0282, 0284, F-013, F-080 |
| [0288](0288-fan-blade-static-and-modal-demonstration.md) | SB-18 | Demonstration on a fan blade: static response to centrifugal and pressure load, and the modes at rest; user page | M | 0272, 0276, 0277, 0284, 0287, F-132 |

### ADR 0271 (SB-1): Crate rcfd-solid: the data model of a solid model and the structured builders

**Goal.** Create the crate `rcfd-solid` (depends on `rcfd-core`, `rcfd-mesh`, `rcfd-linalg` only) with the data model that the
whole tier shares, as validated data without any solver: `SolidMesh` (nodes, element blocks of kind Tet4, Hex8, Tet10, node,
element and surface sets), `DofMap`, `Material` (isotropic linear elastic), `Constraint`, `Load`, `SolidModel`, `SolidResult`.
Add the builders that need nothing else: a box of hexahedra, the same box in tetrahedra (six per hexahedron, conforming), and
the upgrade of linear to quadratic tetrahedra by mid-edge nodes. `lib.rs` declares every module of the tier (section 2.3 of ADR
0270) so that each later ADR fills its own file and none edits `lib.rs`.

**Dependencies.** none in this list; existing roadmap items: F-001, F-010, F-012.

**Acceptance (draft).** A new crate rcfd-solid (dependencies rcfd-core, rcfd-mesh, rcfd-linalg only) holds the solid data model;
the workspace graph test passes with the one registered line for the new crate, the architecture page lists it, and nothing
existing changes (baseline hashes unchanged). On a box of n^3 hexahedra and on its six-tetrahedra split, for n = 1, 2, 5 with
unequal edge lengths: every element has a positive Jacobian at every node, the element volumes sum to the box volume within
1e-12 relative, the area of each of the six face sets equals the box face within 1e-12, and every interior face is shared by
exactly two elements; the quadratic upgrade has (nodes + edges) nodes and each mid-edge node is the mean of its two ends within
1e-15; an inverted element, a node repeated in an element, an empty set, E <= 0 and a Poisson ratio outside (-1, 0.5) are each
refused with an error naming the element or the key.

**Checked against.** Geometry only: volume and face areas of a box, the edge count of a structured grid (derivable). No
published benchmark.

**Files.** `Cargo.toml` (members, workspace.dependencies), `Cargo.lock` (the new workspace member only),
`crates/rcfd-solid/Cargo.toml`, `crates/rcfd-solid/src/{lib.rs, mesh.rs, model.rs, builders.rs}` and one-line stubs of the other
modules, `crates/rcfd-solid/tests/sb_model.rs`, `docs/architecture.md`, and ONE line of the existing test
`crates/rcfd-cli/tests/workspace_graph.rs` (the layer of the new crate; needs the maintainer, decision M1).

**Size.** M. **What could make it miss.** The edit of the existing graph test is not approved (the crate then panics that test).
A data model too narrow for the next tier: three unknowns per node hard-coded instead of going through `DofMap`, or loads as
closures that cannot be read from a file.

### ADR 0272 (SB-2): Solid mesh from the existing meshes: polyhedral mesh, tetrahedral mesher output, patches as surface sets, curved edges

**Goal.** Build a `SolidMesh` from what the repository already produces: an `rcfd_mesh::Mesh` whose cells are all tetrahedra or
all hexahedra (the node order of each element is recovered from the face lists), and a `rcfd_mesh::tetmesh::TetMesh`. Boundary
patches become surface sets with outward normals; cell zones become element sets. A caller-supplied projection moves the
mid-edge nodes of boundary edges onto a curved surface.

**Dependencies.** ADR 0271; existing roadmap items: F-010, F-012, F-132.

**Acceptance (draft).** A SolidMesh built from box_mesh (hexahedra), from the annulus generator (hexahedra with curved faces)
and from the tetrahedral mesher's output of a box surface has the node and element counts of its source, element volumes summing
to the source volume within 1e-12 relative (tetrahedra, planar hexahedra) or 1e-3 (annulus, registered from the chord error),
one surface set per patch whose area equals the patch area within 1e-12 and whose face normals point along the source's outward
area vectors; a prism, pyramid or general polyhedral cell is refused naming the cell. With mid-edge nodes projected onto the
cylinder walls of a quarter annulus the projected nodes lie on their radius within 1e-14 and the volume error of the quadratic
tetrahedra falls at an observed order of at least 3 under refinement.

**Checked against.** Closed forms: volume of a box and of an annulus sector, `pi (b^2 - a^2) h / 4`. No published benchmark.

**Files.** `crates/rcfd-solid/src/convert.rs`, `crates/rcfd-solid/tests/sb_convert.rs`.

**Size.** M. **What could make it miss.** Recovering the hexahedron node order from face lists on warped cells; the orientation
convention of `TetMesh`; quadratic elements inverted by a projection on a coarse curved mesh (must be refused, not repaired).
The Gmsh route is exercised only in ADR 0287, because `rcfd-solid` does not depend on `rcfd-io`.

### ADR 0273 (SB-3): Element library: linear tetrahedron, trilinear hexahedron, quadratic tetrahedron; isotropic elasticity matrix

**Goal.** Shape functions and their derivatives, quadrature rules, the strain-displacement matrix and the element stiffness `K_e
= sum_q w_q B^T D B det J` for Tet4 (one point), Hex8 (2 x 2 x 2 Gauss, full integration) and Tet10 (a rule exact to degree 2
for stiffness; a degree-4 rule is added here for the mass matrix of ADR 0284), and the isotropic matrix `D` from `E` and `nu`.
Behind an `Element` trait with the entry points later tiers need (section 2.4 of ADR 0270).

**Dependencies.** ADR 0271; existing roadmap items: none.

**Acceptance (draft).** For Tet4, Hex8 (2 x 2 x 2 Gauss) and Tet10, on the reference element and on randomly distorted ones
(registered seed): the shape functions sum to 1 and their derivatives to 0 within 1e-14 at 100 random points; the analytic
derivatives agree with central differences within 1e-7; each quadrature rule integrates every monomial up to its stated degree
within 1e-13; the element stiffness is symmetric within 1e-13 of its norm, gives no force for the six rigid-body motions within
1e-11 relative and has rank n - 6; each of the six unit strain states gives the strain energy V/2 eps:D:eps within 1e-12
relative; D gives stress E eps in uniaxial stress and G gamma in shear within 1e-14.

**Checked against.** First principles: Hooke's law; exact integrals of monomials over the reference tetrahedron, `a! b! c! / (a
+ b + c + 3)!`, and over the cube. Method texts (cited from memory, to be confirmed by the implementer): Hughes, The Finite
Element Method (1987); Zienkiewicz and Taylor, The Finite Element Method, vol. 1. Quadrature points and weights are derived or
checked by the monomial test, never taken on trust.

**Files.** `crates/rcfd-solid/src/{element.rs, quadrature.rs, material.rs}`, `crates/rcfd-solid/tests/sb_elements.rs`.

**Size.** M. **What could make it miss.** A tetrahedral rule recalled with wrong weights (the monomial check catches it); the
rank check on a badly shaped element; node-order convention of Tet10 differing from the mid-edge upgrade of ADR 0271.

### ADR 0274 (SB-4): Sparse direct solver for symmetric positive definite matrices (envelope L D L^T after reverse Cuthill-McKee)

**Goal.** `rcfd-linalg` has CG, Jacobi, an incomplete Cholesky tied to the finite-volume LDU matrix, and a scalar aggregation
AMG without rigid-body modes: nothing that solves an elasticity matrix robustly. Add a direct factorisation of a symmetric
positive definite `CsrMatrix`, `L D L^T` on the envelope after the existing `rcm_order`, kept for repeated right-hand sides (the
modal solver of ADR 0283 needs that), and reporting the signs of `D`.

**Dependencies.** none in this list; existing roadmap items: F-025, F-026.

**Acceptance (draft).** rcfd-linalg gains a direct factorisation of a symmetric positive definite CsrMatrix, reusable for many
right-hand sides: on the 7-point Laplacian of 12^3 unknowns, on a randomly permuted copy (registered seed) and on a matrix with
three unknowns per node built from it (Kronecker product with a 3 x 3 SPD block) the relative residual is <= 1e-12 and the
solution agrees with cg (tolerance 1e-13) within 1e-9; the permuted matrix factorises with an envelope no larger than 1.2 times
that of the unpermuted one; a zero or negative pivot, an unsymmetric matrix and a non-finite entry are refused with the row; the
result is bit-identical for 1 and 8 threads; a heavy record factorises 30^3 unknowns and reports time and memory. No existing
function of the crate changes.

**Checked against.** Exact solve: the reference is the residual and the existing `cg`. Method: George and Liu, Computer Solution
of Large Sparse Positive Definite Systems (1981), the envelope method (cited from memory).

**Files.** `crates/rcfd-linalg/src/skyline.rs` (new), `crates/rcfd-linalg/src/lib.rs` (one `pub mod` line),
`crates/rcfd-linalg/tests/sb_direct.rs`.

**Size.** M. **What could make it miss.** Memory: the envelope of a 3-D problem grows like n^(5/3), so the solver is practical
to a few times 1e4 unknowns (estimate: 2.8e4 unknowns of a 20^3 hexahedron box need about 0.3 GB). That bounds the mesh sizes of
ADRs 0279, 0280 and 0288; ADR 0282 is the route beyond it.

### ADR 0275 (SB-5): Assembly, displacement constraints, nodal forces and the linear static solve; patch test

**Goal.** Assemble the global stiffness of a `SolidModel` into a `CsrMatrix` in a deterministic order, impose displacement
constraints per node and component (zero or not, so rollers and symmetry planes work) by symmetric elimination, apply nodal
forces, solve with the direct solver of ADR 0274 (CG with Jacobi as the alternative), and recover reactions. This is the first
ADR that computes a displacement field.

**Dependencies.** ADR 0271, ADR 0273, ADR 0274; existing roadmap items: F-025.

**Acceptance (draft).** The stiffness of a SolidModel is assembled into a CsrMatrix deterministically (bit-identical for 1 and 8
threads) and is symmetric within 1e-13 of its norm; displacement constraints per node and component, zero or not, are imposed by
symmetric elimination and the reactions recovered. Patch test (Tet4, Hex8, Tet10; a box whose interior nodes are displaced by a
registered seed; boundary displacements of a linear field with all six strains non-zero): interior displacements within 1e-10 of
the field, relative to its maximum. Uniaxial tension by nodal forces with statically determinate constraints: elongation F L/(E
A) and lateral contraction nu times the axial strain within 1e-10. The reactions balance the applied forces and their moment
within 1e-10; a model that leaves a rigid-body motion free is refused with a message saying so, a constraint on a missing node
with its id.

**Checked against.** Closed forms: a linear displacement field is an exact solution for any conforming element (the patch test,
Irons; Taylor, Simo, Zienkiewicz and Chan, Int. J. Numer. Meth. Engng 22 (1986) 39, from memory); uniaxial Hooke's law.

**Files.** `crates/rcfd-solid/src/{assemble.rs, constraint.rs, solve.rs, loads.rs}` (in `loads.rs` the nodal force only),
`crates/rcfd-solid/tests/sb_static.rs`.

**Size.** M. **What could make it miss.** Detecting an under-constrained model from a small pivot alone is fragile (a threshold
registered from the stiffness scale is needed); the patch-test limit of 1e-10 on Tet10 with a direct solve of a moderately
conditioned matrix.

### ADR 0276 (SB-6): Surface and body loads: traction, pressure, gravity, centrifugal load of a rotating part, body force field

**Goal.** Consistent nodal loads `f = int N^T t dS` and `f = int N^T b dV` for the three elements: a traction vector and a
pressure on a surface set, gravity, the centrifugal body force `rho omega^2 r_perp` of a part rotating about a given axis at
constant speed (the dominant blade load of a fan), and a body force given at the nodes or by a function (used by the
manufactured solution of ADR 0279). Pressure is positive when it presses on the solid.

**Dependencies.** ADR 0275; existing roadmap items: none.

**Acceptance (draft).** Consistent nodal loads for surface traction, pressure (positive pressing on the solid), gravity, the
centrifugal load rho omega^2 r_perp about a given axis, and a body force field, for Tet4, Hex8 and Tet10. Resultants on a box: a
uniform pressure on the closed surface gives zero force and moment within 1e-12 p A L; the traction resultant is t A, gravity
rho V g and the centrifugal resultant rho omega^2 V r_c (r_c the centroid's distance from the axis), each within 1e-12. Closed
forms with nu = 0, displacements only: hanging bar, u(x) = rho g (L x - x^2/2)/E: Tet10 nodal error <= 1e-9 relative, Hex8 and
Tet4 tip displacement within 1 percent at 8 elements along; rotating radial bar clamped on the axis: tip displacement rho
omega^2 L^3/(3 E) within 1 percent (Tet10 at 8, Hex8 at 16 elements along); thin rotating ring (thickness/radius 0.02): radial
displacement rho omega^2 r^3/E within 2 percent.

**Checked against.** Closed forms, all derivable in a few lines: 1-D equilibrium `d sigma/dx + b = 0` with `b = rho g` and `b =
rho omega^2 r`; the thin-ring hoop stress `rho omega^2 r^2` (membrane equilibrium).

**Files.** `crates/rcfd-solid/src/loads.rs`, `crates/rcfd-solid/tests/sb_loads.rs`.

**Size.** M. **What could make it miss.** The consistent load of a uniform pressure on a quadratic triangle is zero at the
corners and one third at each mid-edge node: correct, but a resultant-only check would not catch a wrong distribution, hence the
displacement checks. The ring needs curved geometry (ADR 0272 is not a dependency: the test builds its ring with the hexahedron
builder and a coordinate map). Spin softening and stress stiffening are NOT included (intermediate tier).

### ADR 0277 (SB-7): Stress and strain recovery: integration-point, element and nodal values; von Mises and principal stresses

**Goal.** From a displacement field: strain and stress at the integration points, element means, nodal values by extrapolation
and volume-weighted averaging; von Mises stress, principal stresses with their directions, and the strain energy. The six
components are stored in the order xx, yy, zz, xy, yz, zx with engineering shear strains (fixed by ADR 0270 section 2.2).

**Dependencies.** ADR 0275; existing roadmap items: none.

**Acceptance (draft).** Strain and stress at integration points, element means and nodal averages; von Mises stress, principal
stresses with directions, strain energy. For 1000 random symmetric tensors (registered seed) the principal values from the
routine agree with an independent method (Jacobi rotations against the closed trigonometric form) within 1e-12 of the tensor
norm, are ordered, have orthonormal directions within 1e-12 and are unchanged by a random rotation of the tensor within 1e-12;
von Mises equals |s| in uniaxial stress, sqrt(3) tau in pure shear and 0 for a hydrostatic state within 1e-14 of the norm; a
tensor with repeated roots gives finite values. On the patch test of the static solve every recovered stress equals D eps within
1e-10; the strain energy equals half the work of the loads within 1e-10 for a uniaxial and a bending load.

**Checked against.** Closed forms: invariants of a symmetric 3 x 3 tensor; Clapeyron's theorem (strain energy = half the work of
the loads in linear elasticity).

**Files.** `crates/rcfd-solid/src/recover.rs`, `crates/rcfd-solid/tests/sb_recover.rs`.

**Size.** S. **What could make it miss.** Loss of accuracy of the trigonometric form near repeated roots; nodal averaging across
a material interface (basic tier: one material per averaging region, otherwise refused). Superconvergent patch recovery and
error estimation are intermediate.

### ADR 0278 (SB-8): VTK output of a solid result: finite-element cells with point and cell data

**Goal.** The existing `.vtu` writer writes the polyhedral finite-volume mesh with cell data only. Add, beside it and without
changing it, a writer of finite-element cells from plain arrays (so `rcfd-io` needs no new dependency), and in `rcfd-post` the
function that writes a `SolidResult`: displacement, nodal von Mises and principal stresses, element stress components,
reactions. This ADR also adds the `rcfd-solid` dependency of `rcfd-post` and declares its three new modules, so that ADRs 0285
and 0286 touch only their own file.

**Dependencies.** ADR 0277; existing roadmap items: F-070.

**Acceptance (draft).** rcfd-io gains a writer of an unstructured grid of finite elements from plain arrays (VTK cell types 10,
12 and 24, point data and cell data); the existing writers are unchanged and their tests pass unedited. rcfd-post writes a
SolidResult through it: displacement at the nodes, nodal von Mises and principal stresses, the six stress components per
element, reactions. A test parses the file back: counts, offsets and cell types are consistent, every value has the bits that
were written, and a box under uniform strain read back has displacement eps x at every node within 1e-15; a field of the wrong
length is refused by name.

**Checked against.** The public VTK file-format description (Kitware) for the cell type numbers and the node order of each type;
the repository's existing writer is the model for the XML layout. The node order of the quadratic tetrahedron is to be confirmed
from that public document by the implementer (recalled here, UNVERIFIED).

**Files.** `crates/rcfd-io/src/vtk.rs` (appended functions only), `crates/rcfd-io/tests/sb_vtk_elements.rs`,
`crates/rcfd-post/Cargo.toml`, `crates/rcfd-post/src/lib.rs` (three `pub mod` lines), `crates/rcfd-post/src/ solid_output.rs`,
stubs `solid_loads.rs` and `solid_coupling.rs`, `crates/rcfd-post/tests/sb_solid_output.rs`.

**Size.** S. **What could make it miss.** No reader other than the test's own parser checks the file: that a viewer shows it
correctly stays the maintainer's visual check. Mid-edge node order of Tet10 differing between the element library and VTK.

### ADR 0279 (SB-9): Verification by a manufactured solution: observed order of displacement and stress for the three elements

**Goal.** A smooth displacement field on the unit cube, its body force from the analytic divergence of the stress, and the
observed order of convergence of the displacement error (L2) and of the stress error (energy norm) under uniform refinement, for
each element. Tests only: no library file is edited; a defect found in a library file is reported, not patched here.

**Dependencies.** ADR 0276, ADR 0277; existing roadmap items: none.

**Acceptance (draft).** Manufactured solution: a smooth trigonometric displacement field on the unit cube with nu = 0.3, the
body force from the analytic divergence of the stress (checked against finite differences within 1e-6), displacements prescribed
on all faces, and a second case with tractions on three faces. On at least three meshes, each halving h, the observed order of
the L2 displacement error over the finest pair is >= 1.8 for Tet4 and Hex8 and >= 2.7 for Tet10, the observed order of the
energy-norm error is >= 0.9 and >= 1.8, and both errors fall monotonically. Negative control: a body force with the sign of the
lambda term reversed gives an order below 0.5.

**Checked against.** The method of manufactured solutions (Roache, Verification and Validation in Computational Science and
Engineering, 1998; from memory); a-priori estimates of the finite element method: order p + 1 in L2 and p in the energy norm for
complete polynomials of degree p (Strang and Fix, An Analysis of the Finite Element Method, 1973).

**Files.** `crates/rcfd-solid/tests/sb_mms.rs` only.

**Size.** M. **What could make it miss.** Meshes small enough for the direct solver (about 1.5e4 unknowns per run) may still be
pre-asymptotic for Tet10; the error norms need a quadrature of higher degree than the stiffness, or the measured order is
polluted.

### ADR 0280 (SB-10): Closed-form benchmarks: cantilever bending and the thick-walled cylinder under pressure

**Goal.** Two engineering checks with closed forms: the tip deflection and bending stress of a slender cantilever (which shows
the locking of the linear tetrahedron and that the quadratic one is free of it), and Lame's thick-walled cylinder under internal
pressure in plane strain on a quarter model with symmetry constraints (curved geometry, pressure load, rollers). Tests only.

**Dependencies.** ADR 0272, ADR 0276, ADR 0277; existing roadmap items: none.

**Acceptance (draft).** Cantilever of slenderness L/h = 20 (square section, nu = 0.3, clamped root, tip shear force): the tip
deflection is within 1 percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A L^2)), kappa = 5/6, with Tet10 on a registered mesh, and
within 5 percent with Hex8 at four elements through the thickness; the bending stress at the surface one section height from the
clamp is within 3 percent of M c/I (Tet10); Tet4 on the same nodes is the registered locking control (deflection below 90
percent of the closed form). Thick-walled cylinder, b/a = 2, plane strain, internal pressure, quarter model: the radial
displacement at the bore and at the outer wall is within 0.5 percent and the hoop stress at the bore within 2 percent of Lame's
solution (Hex8 and Tet10 with curved edges on registered meshes), and the observed order of the radial displacement error with
Hex8 is >= 1.8.

**Checked against.** Closed forms. Cantilever: Euler-Bernoulli deflection with Timoshenko's shear term (the shear term is 0.2
percent at this slenderness). Cylinder (Lame; Timoshenko and Goodier, Theory of Elasticity): `sigma_r = A - B/r^2`, `sigma_theta
= A + B/r^2`, `A = p a^2/(b^2 - a^2)`, `B = A b^2`, and in plane strain `u_r = (1 + nu)/E ((1 - 2 nu) A r + B/r)`; derivable in
a few lines, to be re-derived in the registration. No NAFEMS value is used (their target values are not in the repository).

**Files.** `crates/rcfd-solid/tests/sb_closed_forms.rs` only.

**Size.** M. **What could make it miss.** The Hex8 limit of 5 percent is a draft: full integration locks in shear and the number
depends on the element aspect ratio; the implementer registers it from an estimate. The clamp is not the beam-theory support
when nu > 0 (local effect, estimated below 0.5 percent at L/h = 20). Stress at the bore converges at first order for Hex8.

### ADR 0281 (SB-11): Thermal strain: nodal temperature field, expansion coefficient, thermal load and stress

**Goal.** The load export of F-237 carries nodal temperatures (`TEMP`). Add the isotropic thermal strain `eps_th = alpha (T -
T_ref) I`: the equivalent nodal load `int B^T D eps_th dV` and the stress `sigma = D (eps - eps_th)` in the recovery. Opt-in: a
model without a temperature field gives the bits it gave before.

**Dependencies.** ADR 0276, ADR 0277; existing roadmap items: none.

**Acceptance (draft).** With a nodal temperature field, an expansion coefficient and a reference temperature: a uniformly
heated, statically determinately held box expands by alpha dT x at every node within 1e-10 with stresses below 1e-10 E alpha dT;
a box held on all faces has the hydrostatic stress -E alpha dT/(1 - 2 nu) within 1e-10; a bar held at both ends with free sides
has the axial stress -E alpha dT within 1e-10 (nu = 0.3); a linear temperature through the thickness of a free beam gives the
curvature alpha dT/h within 1 percent and stresses below 1 percent of E alpha dT (Tet10); a model without temperature is
bit-identical to the same model before this change.

**Checked against.** Closed forms from Hooke's law with thermal strain (Timoshenko and Goodier, Theory of Elasticity, the
chapter on thermal stress; each derivable in a few lines).

**Files.** `crates/rcfd-solid/src/thermal.rs`, small hooks in `loads.rs` and `recover.rs`,
`crates/rcfd-solid/tests/sb_thermal.rs`.

**Size.** S. **What could make it miss.** It edits `loads.rs` and `recover.rs`, so it cannot run beside ADR 0276 or 0277.
Temperature-dependent `E` and `alpha` are not included (intermediate).

### ADR 0282 (SB-12): Iterative route for larger models: CG with block Jacobi and incomplete Cholesky on the stiffness matrix

**Goal.** The direct solver stops at a few times 1e4 unknowns. Add two preconditioners for the CSR stiffness matrix: a 3 x 3
block Jacobi and an incomplete Cholesky with zero fill and a diagonal shift when a pivot fails, and make the solver selectable
in the static solve. The default stays the direct solver, bit for bit.

**Dependencies.** ADR 0275; existing roadmap items: F-026.

**Acceptance (draft).** CG on the stiffness matrix with (a) 3 x 3 block Jacobi and (b) incomplete Cholesky with zero fill on CSR
and a diagonal shift on breakdown, selectable beside the direct solver: on a cantilever of about 5e3 unknowns (fast) and of at
least 1e5 unknowns (heavy record) the relative residual reaches 1e-10; on the fast case the displacement agrees with the direct
solution within 1e-7 relative, incomplete Cholesky needs at most half the iterations of block Jacobi, and the iteration count
and the solution are bit-identical for 1 and 8 threads; a breakdown is reported with the shift used, never as a non-finite
result. The default solver and its results are unchanged bit for bit.

**Checked against.** The reference is the direct solution. Methods: Meijerink and van der Vorst, Math. Comp. 31 (1977) 148
(incomplete Cholesky); Manteuffel, Math. Comp. 34 (1980) 473 (the shifted factorisation); both cited from memory.

**Files.** `crates/rcfd-linalg/src/precond.rs` (appended types, no re-export in `lib.rs`), `crates/rcfd-solid/src/solve.rs`,
`crates/rcfd-solid/tests/sb_pcg.rs`.

**Size.** M. **What could make it miss.** The factor of two over block Jacobi is a draft, not a derivation; on slender bodies CG
converges slowly whatever the one-level preconditioner. The real answer (aggregation AMG with the rigid-body modes as near-null
space) is left to the intermediate tier.

### ADR 0283 (SB-13): Eigensolver for K x = lambda M x: dense Jacobi and subspace iteration on the direct factorisation

**Goal.** `rcfd-linalg` has no eigensolver. Add a dense symmetric one (cyclic Jacobi rotations) and, on top of the factorisation
of ADR 0274, subspace iteration for the lowest m eigenpairs of the generalised symmetric problem with a shift (so that a free
body, whose K is singular, can be solved), and a count of the eigenvalues below a value from the signs of `D` so that a missed
mode is detected.

**Dependencies.** ADR 0274; existing roadmap items: none.

**Acceptance (draft).** rcfd-linalg gains a dense symmetric eigensolver and the lowest m eigenpairs of K x = lambda M x (K
symmetric, M symmetric positive definite) by subspace iteration on the factorisation of K + sigma M, with a count of the
eigenvalues below a value from the signs of D. Checks: the tridiagonal matrix (2, -1) of n = 200 with M = I: the lowest 10
eigenvalues equal 4 sin^2(j pi/(2 (n + 1))) within 1e-10 relative; the 5-point Laplacian on 30 x 30: repeated eigenvalues come
with their multiplicity and the vectors are M-orthonormal within 1e-10; the same with a non-uniform diagonal M against the dense
solver within 1e-9; residual |K x - lambda M x| <= 1e-8 |K x| for every returned pair; a singular K (free chain) with sigma > 0
returns its zero eigenvalue below 1e-9 of the first non-zero one; the count below the m-th eigenvalue equals m - 1; no
convergence within the cap is an error stating the residuals.

**Checked against.** Closed forms: eigenvalues of the second-difference matrix and of its Kronecker sums (derivable). Methods:
Bathe and Wilson, Int. J. Numer. Meth. Engng 6 (1973) 213 (subspace iteration) and Bathe, Finite Element Procedures, chapter on
eigenproblems; Golub and Van Loan, Matrix Computations (Jacobi); Sylvester's law of inertia for the count. All cited from
memory.

**Files.** `crates/rcfd-linalg/src/eigen.rs` (new), `crates/rcfd-linalg/src/lib.rs` (one `pub mod` line),
`crates/rcfd-linalg/tests/sb_eigen.rs`.

**Size.** M. **What could make it miss.** The count needs `L D L^T` of an indefinite shifted matrix without pivoting, which can
lose accuracy near an eigenvalue (the shift must be moved, by a registered rule); slow convergence for clustered modes. It edits
`rcfd-linalg/src/lib.rs`, so it starts after ADR 0274 is merged.

### ADR 0284 (SB-14): Mass matrix and linear modal analysis of a solid model

**Goal.** Consistent mass matrix of the three elements from the density, assembled like the stiffness; the lowest natural
frequencies and mass-normalised mode shapes of a constrained or free model with the eigensolver of ADR 0283. This is the modal
analysis of fan breakdown item E5 for a blade at rest (no rotation effects).

**Dependencies.** ADR 0275, ADR 0283; existing roadmap items: none.

**Acceptance (draft).** Consistent mass matrix for Tet4, Hex8 and Tet10: for a box, the total mass, the centre of mass and the
inertia tensor obtained from rigid motions r^T M r equal the closed forms within 1e-12. Modal analysis: a free box has six
frequencies below 1e-6 of the seventh; a fixed-free bar with nu = 0 has f_n = (2n - 1) sqrt(E/rho)/(4 L), n = 1, 2, 3, within
0.5 percent on a registered mesh, and the error of f_1 falls at an observed order >= 1.8 with Hex8; a cantilever of slenderness
20 and square section has its first bending frequency within 1 percent of (1.8751)^2 sqrt(E I/(rho A L^4))/(2 pi) and its second
(4.6941) within 2.5 percent with Tet10, the Euler-Bernoulli value less the shear and rotary-inertia correction estimated in the
registration (0.15 and 0.9 percent); on a symmetric Hex8 mesh the two bending planes give the same frequency within 1e-6
relative; mode shapes are M-orthonormal within 1e-9.

**Checked against.** Closed forms: the wave equation of a bar (`f_n = (2n - 1) c/(4L)`, exact for a 3-D bar with nu = 0);
Euler-Bernoulli cantilever, roots of `cos x cosh x + 1 = 0` (1.8751, 4.6941, 7.8548: to be recomputed in the registration, one
line of arithmetic); the correction for shear and rotary inertia from Timoshenko beam theory (Timoshenko, Phil. Mag. 41 (1921)
744; from memory). Rigid-body inertia of a box.

**Files.** `crates/rcfd-solid/src/{mass.rs, modal.rs}`, `crates/rcfd-solid/tests/sb_modal.rs`.

**Size.** M. **What could make it miss.** Size: 3-D solid elements on a slender beam need many unknowns for the second mode
within the direct solver's reach. Torsional and bending modes interleave, so modes must be identified by shape, not by index.
Rotating-blade effects (spin softening, stress stiffening, Campbell diagram) are intermediate.

### ADR 0285 (SB-15): One-way coupling 1: loads of the F-237 deck applied to the solid surface

**Goal.** Close the file route of fan breakdown item E4: build the structural shell surface that F-237 exports onto from a
surface set of the solid mesh (same nodes, normals toward the fluid), read the deck back (`PLOAD4`, `TEMP`), and apply its
pressures as surface loads and its temperatures as the thermal field of ADR 0281. The mapping from the fluid wall stays F-237's
(plane and tube parametrisations); a general geometric mapping is F-362.

**Dependencies.** ADR 0272, ADR 0278, ADR 0281; existing roadmap items: F-237.

**Acceptance (draft).** A surface set of a solid mesh is turned into the shell surface that the F-237 export writes onto, and a
deck read back (PLOAD4, TEMP) is applied to the solid: the resultant force and moment of the nodal loads equal deck_force_moment
within 1e-12 relative; a deck element or node that matches no solid face or node within a stated tolerance is refused with its
id. Tube: the wall pressure of the fluid mesh of the F-237 tube check, uniform, exported, written, parsed and applied to a thick
solid tube (b/a = 2, plane strain) gives the radial displacement at the bore within 1 percent and the hoop stress within 2
percent of Lame's solution; with a pressure varying along the axis and around the circumference the solid's reactions equal the
fluid's integrated force within 0.1 percent (the F-237 limit); TEMP of a uniform temperature gives the free expansion alpha dT r
within 1e-6.

**Checked against.** Lame's cylinder (as ADR 0280) and the fluid-side integrals of F-237 (`fluid_force_moment`,
`deck_force_moment`, ADR 0051 and ADR 0244).

**Files.** `crates/rcfd-post/src/solid_loads.rs`, `crates/rcfd-post/tests/sb_deck_loads.rs`.

**Size.** M. **What could make it miss.** The sign convention of `PLOAD4` in the deck is recorded as unverified against any
solver (ADR 0051 D8): this ADR fixes it only within the repository. The heat-flux card `RCFDQ` has no use until a thermal solver
of the solid exists (not in this tier). Shell faces of a Tet10 surface (six nodes) do not exist in the deck's element set.

### ADR 0286 (SB-16): One-way coupling 2: fluid face forces in process, and a static solid as a co-simulation participant

**Goal.** The in-process route: fluid face forces, as `cosim::Fluid::forces` returns them, mapped to the nodes of a solid
surface set by a given `cosim::InterfaceMap` and solved statically. On the same code a `StaticSolid` implements
`cosim::Structure` (advance = static solve; checkpoint and restore), which fixes the interface the intermediate tier's dynamic
participant will use. It is checked against a mock fluid only; coupling to the flow solver is intermediate.

**Dependencies.** ADR 0275, ADR 0278; existing roadmap items: F-139.

**Acceptance (draft).** Fluid face forces on a wall are mapped to the nodes of a solid surface set by a given InterfaceMap and
solved statically: resultant force, moment and work equal the fluid side's within 1e-12; for a matching planar interface under
uniform pressure the solid's displacement equals the uniaxial closed form within 1e-10, and for a 3:2 non-matching one within a
limit registered from the estimate of the mapping error. A StaticSolid implements cosim::Structure (advance is a static solve;
restore returns the checkpoint bit for bit) and, with a test-only mock fluid whose force falls linearly with the wall
displacement (stiffness k_f), coupled_step converges to u = F0/(k_s + k_f) within 1e-8 with the Aitken and with the
least-squares update; a failing solve stops the step with the participant's name and both sides restored. No flow solver is
coupled.

**Checked against.** Closed form: two springs in parallel, `u = F0/(k_s + k_f)` with `k_s = E A/L` of a bar (nu = 0). The
protocol of `docs/cosimulation-protocol.md` (ADR 0256) and the update of ADR 0261.

**Files.** `crates/rcfd-post/src/solid_coupling.rs`, `crates/rcfd-post/tests/sb_solid_coupling.rs`.

**Size.** M. **What could make it miss.** A static structure against a real incompressible flow has no structural mass, so the
added-mass ratio is unbounded: this participant is for the mock and for quasi-static use, and the ADR must say so. Whether the
participant belongs in `rcfd-post` or in a later coupling crate is decision M5.

### ADR 0287 (SB-17): A solid case file of its own and the library entry that runs it

**Goal.** A minimal entry point that needs no golden edit: a TOML file of its own for a solid model (mesh from a generator or a
Gmsh file, material, constraints and loads on named sets, static or modal analysis, solver, output), parsed in a new module of
`rcfd-api`, and a library function that runs it and writes the `.vtu` and a JSON report. No key is added to the flow case
schema, no CLI subcommand and no MCP tool (each of those touches a golden and is left as the maintainer's step, decision M3).

**Dependencies.** ADR 0272, ADR 0278, ADR 0282, ADR 0284; existing roadmap items: F-013, F-080.

**Acceptance (draft).** A solid case file (TOML of its own: mesh from a generator or a Gmsh file, material, constraints and
loads on named sets, static or modal analysis, solver choice, output) is read and run by a library function of rcfd-api that
writes the .vtu and a JSON report (displacement and stress extremes, reactions, frequencies, solver statistics): an unknown key,
a missing set, a wrong unit or type each give an error with the key and the line; a file run gives the bits of the same model
built by library calls (limit 1e-12, expected identical); the cantilever and the bar-frequency checks pass when run from files,
and a Gmsh tetrahedral mesh of a box read through rcfd-io gives the uniaxial closed form within 1e-10; no flow case key, CLI
help text, MCP tool list, hash table or golden changes (the existing schema, CLI and tool-list tests pass unedited).

**Checked against.** The closed forms of ADRs 0275, 0280 and 0284, re-run through the file. The way ADR 0257 established that a
schema addition needs no golden edit (its check K0) is the model for this ADR's own check that nothing existing changes.

**Files.** `crates/rcfd-api/Cargo.toml`, `crates/rcfd-api/src/lib.rs` (one `pub mod` line), `crates/rcfd-api/src/
solid_case.rs`, `crates/rcfd-api/tests/sb_solid_case.rs`, `docs/structural-solver.md` (the file format section).

**Size.** M. **What could make it miss.** Using the settings tree of `rcfd-core` (schema, located errors) against plain serde is
a design choice that changes the size; if the tree is used, the solid schema must stay a separate schema object so that the
schema golden does not change.

### ADR 0288 (SB-18): Demonstration on a fan blade: static response to centrifugal and pressure load, and the modes at rest; user page

**Goal.** Bring the tier to the target of fan breakdown item E5: one blade of the parametric fan geometry (`rcfd_mesh::fan`) as
a solid clamped at the hub, under the centrifugal load at a stated speed and a given pressure distribution, with its lowest
modes at rest. There is no measured or published reference for this blade, so the checks are consistency and mesh convergence,
plus the flat-plate limit against closed forms; the result is a demonstration, not a validation. Also the user page
`docs/structural-solver.md`.

**Dependencies.** ADR 0272, ADR 0276, ADR 0277, ADR 0284, ADR 0287; existing roadmap items: F-132.

**Acceptance (draft).** One blade of the parametric fan geometry, meshed in quadratic tetrahedra and clamped at the hub: under
the centrifugal load at a registered speed the reactions equal rho omega^2 V r_c within 1e-8 and under a uniform pressure the
integrated pressure force within 1e-8; the strain energy equals half the work of the loads within 1e-8; between two meshes (the
second with at least twice the elements) the tip displacement and the first natural frequency change by less than 5 percent,
both numbers recorded; as the flat-plate limit, an untwisted rectangular cantilever plate of the blade's span, mean chord and
thickness has its first bending frequency between the Euler-Bernoulli beam value and that value divided by sqrt(1 - nu^2), with
a 1 percent margin on each side. The page docs/structural-solver.md states the scope, the limits and that nothing is validated
against a measured blade.

**Checked against.** Closed forms only for the limits: resultant of the centrifugal body force, Clapeyron's theorem, and the
beam and wide-plate (cylindrical bending, `E/(1 - nu^2)`) bounds of the first bending frequency. No reference value exists for
the blade itself (docs/fan-project-breakdown.md, part G).

**Files.** `crates/rcfd-solid/tests/sb_fan_blade.rs` (or `crates/rcfd-api/tests/` if the Gmsh or tetrahedral-mesher route needs
`rcfd-io`), `docs/structural-solver.md`, a short section in `docs/fan-project-breakdown.md` (E5 status text).

**Size.** M. **What could make it miss.** The tetrahedral mesher on a thin twisted blade (quality known misses of F-132; slivers
are tolerated less by a structural element than by a finite-volume cell); a thin blade in solid elements needs several elements
through the thickness or quadratic ones, which may exceed the direct solver. If the blade cannot be meshed, the flat-plate limit
alone is delivered and the item is partial.

## 4. Critical path and parallel work

Dependencies (an arrow means "must be merged before"):

```text
0271 -> 0272 ----------------------------> 0280, 0285, 0287, 0288
0271 -> 0273 -> 0275 -> 0276 -> 0279, 0280, 0281, 0288
0274 ---------> 0275 -> 0277 -> 0278 -> 0285, 0286, 0287
0274 -> 0283 -> 0284 (with 0275) -> 0287 -> 0288
                0275 -> 0282 -> 0287
                0276, 0277 -> 0281 -> 0285
```

**Critical path** (seven packages in sequence): 0271 -> 0273 -> 0275 -> 0277 -> 0278 -> 0287 -> 0288. The first displacement
field exists after 0275; the first stress picture after 0278; the first closed-form engineering checks after 0280.

**Waves.** Within a wave no two packages edit the same file; each wave starts when the packages it depends on are merged.

| Wave | Packages in parallel | Files, by package |
|---|---|---|
| 1 | 0271, 0274 | 0271: workspace manifest, new crate, graph test line, architecture page. 0274: `rcfd-linalg` `skyline.rs`, `lib.rs` |
| 2 | 0272, 0273, 0283 | `convert.rs`; `element.rs`, `quadrature.rs`, `material.rs`; `rcfd-linalg` `eigen.rs`, `lib.rs` |
| 3 | 0275 | `assemble.rs`, `constraint.rs`, `solve.rs`, `loads.rs` |
| 4 | 0276, 0277, 0282, 0284 | `loads.rs`; `recover.rs`; `rcfd-linalg` `precond.rs` and `solve.rs`; `mass.rs`, `modal.rs` |
| 5 | 0278, 0279, 0280, 0281 | `rcfd-io` `vtk.rs` and `rcfd-post`; test file only; test file only; `thermal.rs`, `loads.rs`, `recover.rs` |
| 6 | 0285, 0286, 0287 | `rcfd-post` `solid_loads.rs`; `rcfd-post` `solid_coupling.rs`; `rcfd-api` |
| 7 | 0288 | test file and documentation |

Notes on the table:

* `rcfd-linalg/src/lib.rs` gets one line from 0274 and one from 0283: they are in different waves for that reason. 0282 adds to
  `precond.rs` without a re-export, so it does not touch `lib.rs`.
* 0279 and 0280 are tests only. If one of them finds a defect in a library file, it records the miss and reports it; the fix is
  a separate change by the lead's choice, so that the two can run beside 0281, which edits `loads.rs` and `recover.rs`.
* 0286 depends on 0278 only for the files: 0278 adds the `rcfd-solid` dependency of `rcfd-post` and declares its three modules.
* Each package adds its own test file (`sb_*.rs`); none edits another's.
* The machine is shared: waves 2, 4 and 5 are three or four builds at once. The heavy records (0274, 0282) should not run at the
  same time as each other.

## 5. For the maintainer: decisions

Each with options and a recommendation. None is taken here.

* **M1. A new crate, and one line of an existing test.** `crates/rcfd-cli/tests/workspace_graph.rs` panics on an unknown crate
  name, so `rcfd-solid` needs the line `"rcfd-solid" => 2` in its layer table (and `Cargo.toml` members, `docs/architecture.md`).
  House rule 2 says existing tests are not edited. Options: (a) approve the one line, as was done for `rcfd-workspace` (ADR 0173);
  (b) no new crate, the solver as modules of `rcfd-fv`. Recommendation: (a). ADR 0271 cannot be gated without it.
* **M2. The crate's name and layer.** `rcfd-solid` in layer 2 (core, mesh, linalg). Options: (a) as proposed; (b) another name
  (`rcfd-struct`, `rcfd-fem`); (c) layer 4 with `rcfd-io` and `rcfd-fv` as dependencies. Recommendation: (a); the name `solid`
  will be slightly wrong once shells and beams arrive, `rcfd-fem` avoids that.
* **M3. The entry point.** Options: (a) a solid case file of its own, run by a library function, with no CLI subcommand and no MCP
  tool in this tier (no golden changes); (b) as (a) plus a subcommand `rcfd solid` and an MCP tool, which changes the CLI help
  golden and the MCP tool-list golden; (c) no entry point in the basic tier. Recommendation: (a) now, (b) as a small follow-up
  item once the maintainer approves the two golden edits.
* **M4. Elements.** Tet4, Hex8 (full integration), Tet10 in the basic tier; Hex20, wedge, pyramid and a locking-free Hex8 in the
  intermediate tier. Options: (a) as proposed; (b) add Hex20 now (one more S package). Recommendation: (a).
* **M5. The static co-simulation participant and the home of the coupling glue.** Options: (a) as proposed: `StaticSolid` in the
  basic tier, glue in `rcfd-post`; (b) the participant moves to the intermediate tier and ADR 0286 keeps only the in-process load
  route; (c) a coupling crate of its own now. Recommendation: (a), with (c) reconsidered by the intermediate planner.
* **M6. Roadmap placement.** A category "Structural solver" in family "Solver" for `scripts/roadmap_progress.py`; phase 9 like
  the other coupling items, or a new phase. Recommendation: the new category, phase 9 (a new phase changes the progress graphic's
  phase axis).
* **M7. The deferral.** `docs/milestone-fan.md` and `docs/roadmap-overview.md` say the structural solver is deferred by decision
  and "waiting on a decision". If this plan is accepted those two texts need the lead's edit. I took the request for the plan as
  the decision to start; that is my reading, not a recorded decision.

## 6. What I am unsure of

* **Numbers.** Every limit in section 3 is a draft from standard theory or a rough estimate, not a measurement: nothing was run.
  The ones I trust least: the 5 percent for Hex8 on the cantilever (0280), "half the iterations" for incomplete Cholesky (0282),
  the 1e-3 volume tolerance of the annulus conversion (0272), the non-matching mapping limit (0286, left to the registration),
  and the 5 percent mesh-change limit on the blade (0288).
* **Citations.** Books and papers are cited from memory (marked in each stub). The formulas used are all short derivations and
  the implementers are told to re-derive them; the cantilever roots 1.8751, 4.6941, 7.8548 are recalled and take one line to
  recompute.
* **The direct solver's reach.** The estimate "a few times 1e4 unknowns" is arithmetic on the envelope size, not a measurement.
  If it is lower, 0279, 0280, 0284 and 0288 need smaller meshes or 0282 earlier (it can move to wave 4 as it is).
* **Hexahedron node order from the finite-volume mesh** (0272): I did not read how `from_cell_faces` orders faces; the recovery
  may be harder on warped cells than the size M assumes.
* **The blade** (0288): whether the tetrahedral mesher gives a usable mesh of a thin twisted blade is unknown (its quality
  clauses are known misses under F-132). The stub says what is delivered if it does not.
* **Whether `workspace_graph.rs` counts dev-dependencies.** The plan avoids the question: no crate gets a dev-dependency on a
  higher layer.
* **Sizes.** 0271, 0275 and 0284 are at the upper end of M. If one must be split: 0271 into "crate and model" and "builders";
  0275 into "assembly and solve" and "constraints and reactions"; 0284 into "mass" and "modal".

## 7. For the lead: roadmap entries

Category for the roadmap graphic: **"Structural solver"**, family **"Solver"** (a new row of `CATEGORIES` in
`scripts/roadmap_progress.py`). Placeholder ids SB-1 to SB-18, to be renumbered; status todo for all; phase 9 proposed (M6). The
existing items they depend on are done or partial: F-001, F-010, F-012, F-013, F-025, F-026, F-070, F-080, F-132 (partial),
F-139, F-237 (partial). Suggested edits of existing notes, for the lead to word: fan breakdown E5 and the roadmap overview's
"There is no structural solver" stay true until SB-5 is done.

Each acceptance and each note below is one quoted paragraph on a single line.

### Entries

**SB-1** (ADR 0271), status todo, deps: F-001, F-010, F-012

- title: Crate rcfd-solid: the data model of a solid model and the structured builders
- acceptance:

> Every limit not stated here is registered in the ADR before the run. A new crate rcfd-solid (dependencies rcfd-core, rcfd-mesh, rcfd-linalg only) holds the solid data model; the workspace graph test passes with the one registered line for the new crate, the architecture page lists it, and nothing existing changes (baseline hashes unchanged). On a box of n^3 hexahedra and on its six-tetrahedra split, for n = 1, 2, 5 with unequal edge lengths: every element has a positive Jacobian at every node, the element volumes sum to the box volume within 1e-12 relative, the area of each of the six face sets equals the box face within 1e-12, and every interior face is shared by exactly two elements; the quadratic upgrade has (nodes + edges) nodes and each mid-edge node is the mean of its two ends within 1e-15; an inverted element, a node repeated in an element, an empty set, E <= 0 and a Poisson ratio outside (-1, 0.5) are each refused with an error naming the element or the key.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0271; the registration is written by the implementation agent before any code.

**SB-2** (ADR 0272), status todo, deps: SB-1, F-010, F-012, F-132

- title: Solid mesh from the existing meshes: polyhedral mesh, tetrahedral mesher output, patches as surface sets, curved edges
- acceptance:

> Every limit not stated here is registered in the ADR before the run. A SolidMesh built from box_mesh (hexahedra), from the annulus generator (hexahedra with curved faces) and from the tetrahedral mesher's output of a box surface has the node and element counts of its source, element volumes summing to the source volume within 1e-12 relative (tetrahedra, planar hexahedra) or 1e-3 (annulus, registered from the chord error), one surface set per patch whose area equals the patch area within 1e-12 and whose face normals point along the source's outward area vectors; a prism, pyramid or general polyhedral cell is refused naming the cell. With mid-edge nodes projected onto the cylinder walls of a quarter annulus the projected nodes lie on their radius within 1e-14 and the volume error of the quadratic tetrahedra falls at an observed order of at least 3 under refinement.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0272; the registration is written by the implementation agent before any code.

**SB-3** (ADR 0273), status todo, deps: SB-1

- title: Element library: linear tetrahedron, trilinear hexahedron, quadratic tetrahedron; isotropic elasticity matrix
- acceptance:

> Every limit not stated here is registered in the ADR before the run. For Tet4, Hex8 (2 x 2 x 2 Gauss) and Tet10, on the reference element and on randomly distorted ones (registered seed): the shape functions sum to 1 and their derivatives to 0 within 1e-14 at 100 random points; the analytic derivatives agree with central differences within 1e-7; each quadrature rule integrates every monomial up to its stated degree within 1e-13; the element stiffness is symmetric within 1e-13 of its norm, gives no force for the six rigid-body motions within 1e-11 relative and has rank n - 6; each of the six unit strain states gives the strain energy V/2 eps:D:eps within 1e-12 relative; D gives stress E eps in uniaxial stress and G gamma in shear within 1e-14.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0273; the registration is written by the implementation agent before any code.

**SB-4** (ADR 0274), status todo, deps: F-025, F-026

- title: Sparse direct solver for symmetric positive definite matrices (envelope L D L^T after reverse Cuthill-McKee)
- acceptance:

> Every limit not stated here is registered in the ADR before the run. rcfd-linalg gains a direct factorisation of a symmetric positive definite CsrMatrix, reusable for many right-hand sides: on the 7-point Laplacian of 12^3 unknowns, on a randomly permuted copy (registered seed) and on a matrix with three unknowns per node built from it (Kronecker product with a 3 x 3 SPD block) the relative residual is <= 1e-12 and the solution agrees with cg (tolerance 1e-13) within 1e-9; the permuted matrix factorises with an envelope no larger than 1.2 times that of the unpermuted one; a zero or negative pivot, an unsymmetric matrix and a non-finite entry are refused with the row; the result is bit-identical for 1 and 8 threads; a heavy record factorises 30^3 unknowns and reports time and memory. No existing function of the crate changes.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0274; the registration is written by the implementation agent before any code.

**SB-5** (ADR 0275), status todo, deps: SB-1, SB-3, SB-4, F-025

- title: Assembly, displacement constraints, nodal forces and the linear static solve; patch test
- acceptance:

> Every limit not stated here is registered in the ADR before the run. The stiffness of a SolidModel is assembled into a CsrMatrix deterministically (bit-identical for 1 and 8 threads) and is symmetric within 1e-13 of its norm; displacement constraints per node and component, zero or not, are imposed by symmetric elimination and the reactions recovered. Patch test (Tet4, Hex8, Tet10; a box whose interior nodes are displaced by a registered seed; boundary displacements of a linear field with all six strains non-zero): interior displacements within 1e-10 of the field, relative to its maximum. Uniaxial tension by nodal forces with statically determinate constraints: elongation F L/(E A) and lateral contraction nu times the axial strain within 1e-10. The reactions balance the applied forces and their moment within 1e-10; a model that leaves a rigid-body motion free is refused with a message saying so, a constraint on a missing node with its id.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0275; the registration is written by the implementation agent before any code.

**SB-6** (ADR 0276), status todo, deps: SB-5

- title: Surface and body loads: traction, pressure, gravity, centrifugal load of a rotating part, body force field
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Consistent nodal loads for surface traction, pressure (positive pressing on the solid), gravity, the centrifugal load rho omega^2 r_perp about a given axis, and a body force field, for Tet4, Hex8 and Tet10. Resultants on a box: a uniform pressure on the closed surface gives zero force and moment within 1e-12 p A L; the traction resultant is t A, gravity rho V g and the centrifugal resultant rho omega^2 V r_c (r_c the centroid's distance from the axis), each within 1e-12. Closed forms with nu = 0, displacements only: hanging bar, u(x) = rho g (L x - x^2/2)/E: Tet10 nodal error <= 1e-9 relative, Hex8 and Tet4 tip displacement within 1 percent at 8 elements along; rotating radial bar clamped on the axis: tip displacement rho omega^2 L^3/(3 E) within 1 percent (Tet10 at 8, Hex8 at 16 elements along); thin rotating ring (thickness/radius 0.02): radial displacement rho omega^2 r^3/E within 2 percent.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0276; the registration is written by the implementation agent before any code.

**SB-7** (ADR 0277), status todo, deps: SB-5

- title: Stress and strain recovery: integration-point, element and nodal values; von Mises and principal stresses
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Strain and stress at integration points, element means and nodal averages; von Mises stress, principal stresses with directions, strain energy. For 1000 random symmetric tensors (registered seed) the principal values from the routine agree with an independent method (Jacobi rotations against the closed trigonometric form) within 1e-12 of the tensor norm, are ordered, have orthonormal directions within 1e-12 and are unchanged by a random rotation of the tensor within 1e-12; von Mises equals |s| in uniaxial stress, sqrt(3) tau in pure shear and 0 for a hydrostatic state within 1e-14 of the norm; a tensor with repeated roots gives finite values. On the patch test of the static solve every recovered stress equals D eps within 1e-10; the strain energy equals half the work of the loads within 1e-10 for a uniaxial and a bending load.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size S. Stub ADR 0277; the registration is written by the implementation agent before any code.

**SB-8** (ADR 0278), status todo, deps: SB-7, F-070

- title: VTK output of a solid result: finite-element cells with point and cell data
- acceptance:

> Every limit not stated here is registered in the ADR before the run. rcfd-io gains a writer of an unstructured grid of finite elements from plain arrays (VTK cell types 10, 12 and 24, point data and cell data); the existing writers are unchanged and their tests pass unedited. rcfd-post writes a SolidResult through it: displacement at the nodes, nodal von Mises and principal stresses, the six stress components per element, reactions. A test parses the file back: counts, offsets and cell types are consistent, every value has the bits that were written, and a box under uniform strain read back has displacement eps x at every node within 1e-15; a field of the wrong length is refused by name.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size S. Stub ADR 0278; the registration is written by the implementation agent before any code.

**SB-9** (ADR 0279), status todo, deps: SB-6, SB-7

- title: Verification by a manufactured solution: observed order of displacement and stress for the three elements
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Manufactured solution: a smooth trigonometric displacement field on the unit cube with nu = 0.3, the body force from the analytic divergence of the stress (checked against finite differences within 1e-6), displacements prescribed on all faces, and a second case with tractions on three faces. On at least three meshes, each halving h, the observed order of the L2 displacement error over the finest pair is >= 1.8 for Tet4 and Hex8 and >= 2.7 for Tet10, the observed order of the energy-norm error is >= 0.9 and >= 1.8, and both errors fall monotonically. Negative control: a body force with the sign of the lambda term reversed gives an order below 0.5.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0279; the registration is written by the implementation agent before any code.

**SB-10** (ADR 0280), status todo, deps: SB-2, SB-6, SB-7

- title: Closed-form benchmarks: cantilever bending and the thick-walled cylinder under pressure
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Cantilever of slenderness L/h = 20 (square section, nu = 0.3, clamped root, tip shear force): the tip deflection is within 1 percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A L^2)), kappa = 5/6, with Tet10 on a registered mesh, and within 5 percent with Hex8 at four elements through the thickness; the bending stress at the surface one section height from the clamp is within 3 percent of M c/I (Tet10); Tet4 on the same nodes is the registered locking control (deflection below 90 percent of the closed form). Thick-walled cylinder, b/a = 2, plane strain, internal pressure, quarter model: the radial displacement at the bore and at the outer wall is within 0.5 percent and the hoop stress at the bore within 2 percent of Lame's solution (Hex8 and Tet10 with curved edges on registered meshes), and the observed order of the radial displacement error with Hex8 is >= 1.8.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0280; the registration is written by the implementation agent before any code.

**SB-11** (ADR 0281), status todo, deps: SB-6, SB-7

- title: Thermal strain: nodal temperature field, expansion coefficient, thermal load and stress
- acceptance:

> Every limit not stated here is registered in the ADR before the run. With a nodal temperature field, an expansion coefficient and a reference temperature: a uniformly heated, statically determinately held box expands by alpha dT x at every node within 1e-10 with stresses below 1e-10 E alpha dT; a box held on all faces has the hydrostatic stress -E alpha dT/(1 - 2 nu) within 1e-10; a bar held at both ends with free sides has the axial stress -E alpha dT within 1e-10 (nu = 0.3); a linear temperature through the thickness of a free beam gives the curvature alpha dT/h within 1 percent and stresses below 1 percent of E alpha dT (Tet10); a model without temperature is bit-identical to the same model before this change.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size S. Stub ADR 0281; the registration is written by the implementation agent before any code.

**SB-12** (ADR 0282), status todo, deps: SB-5, F-026

- title: Iterative route for larger models: CG with block Jacobi and incomplete Cholesky on the stiffness matrix
- acceptance:

> Every limit not stated here is registered in the ADR before the run. CG on the stiffness matrix with (a) 3 x 3 block Jacobi and (b) incomplete Cholesky with zero fill on CSR and a diagonal shift on breakdown, selectable beside the direct solver: on a cantilever of about 5e3 unknowns (fast) and of at least 1e5 unknowns (heavy record) the relative residual reaches 1e-10; on the fast case the displacement agrees with the direct solution within 1e-7 relative, incomplete Cholesky needs at most half the iterations of block Jacobi, and the iteration count and the solution are bit-identical for 1 and 8 threads; a breakdown is reported with the shift used, never as a non-finite result. The default solver and its results are unchanged bit for bit.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0282; the registration is written by the implementation agent before any code.

**SB-13** (ADR 0283), status todo, deps: SB-4

- title: Eigensolver for K x = lambda M x: dense Jacobi and subspace iteration on the direct factorisation
- acceptance:

> Every limit not stated here is registered in the ADR before the run. rcfd-linalg gains a dense symmetric eigensolver and the lowest m eigenpairs of K x = lambda M x (K symmetric, M symmetric positive definite) by subspace iteration on the factorisation of K + sigma M, with a count of the eigenvalues below a value from the signs of D. Checks: the tridiagonal matrix (2, -1) of n = 200 with M = I: the lowest 10 eigenvalues equal 4 sin^2(j pi/(2 (n + 1))) within 1e-10 relative; the 5-point Laplacian on 30 x 30: repeated eigenvalues come with their multiplicity and the vectors are M-orthonormal within 1e-10; the same with a non-uniform diagonal M against the dense solver within 1e-9; residual |K x - lambda M x| <= 1e-8 |K x| for every returned pair; a singular K (free chain) with sigma > 0 returns its zero eigenvalue below 1e-9 of the first non-zero one; the count below the m-th eigenvalue equals m - 1; no convergence within the cap is an error stating the residuals.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0283; the registration is written by the implementation agent before any code.

**SB-14** (ADR 0284), status todo, deps: SB-5, SB-13

- title: Mass matrix and linear modal analysis of a solid model
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Consistent mass matrix for Tet4, Hex8 and Tet10: for a box, the total mass, the centre of mass and the inertia tensor obtained from rigid motions r^T M r equal the closed forms within 1e-12. Modal analysis: a free box has six frequencies below 1e-6 of the seventh; a fixed-free bar with nu = 0 has f_n = (2n - 1) sqrt(E/rho)/(4 L), n = 1, 2, 3, within 0.5 percent on a registered mesh, and the error of f_1 falls at an observed order >= 1.8 with Hex8; a cantilever of slenderness 20 and square section has its first bending frequency within 1 percent of (1.8751)^2 sqrt(E I/(rho A L^4))/(2 pi) and its second (4.6941) within 2.5 percent with Tet10, the Euler-Bernoulli value less the shear and rotary-inertia correction estimated in the registration (0.15 and 0.9 percent); on a symmetric Hex8 mesh the two bending planes give the same frequency within 1e-6 relative; mode shapes are M-orthonormal within 1e-9.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0284; the registration is written by the implementation agent before any code.

**SB-15** (ADR 0285), status todo, deps: SB-2, SB-8, SB-11, F-237

- title: One-way coupling 1: loads of the F-237 deck applied to the solid surface
- acceptance:

> Every limit not stated here is registered in the ADR before the run. A surface set of a solid mesh is turned into the shell surface that the F-237 export writes onto, and a deck read back (PLOAD4, TEMP) is applied to the solid: the resultant force and moment of the nodal loads equal deck_force_moment within 1e-12 relative; a deck element or node that matches no solid face or node within a stated tolerance is refused with its id. Tube: the wall pressure of the fluid mesh of the F-237 tube check, uniform, exported, written, parsed and applied to a thick solid tube (b/a = 2, plane strain) gives the radial displacement at the bore within 1 percent and the hoop stress within 2 percent of Lame's solution; with a pressure varying along the axis and around the circumference the solid's reactions equal the fluid's integrated force within 0.1 percent (the F-237 limit); TEMP of a uniform temperature gives the free expansion alpha dT r within 1e-6.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0285; the registration is written by the implementation agent before any code.

**SB-16** (ADR 0286), status todo, deps: SB-5, SB-8, F-139

- title: One-way coupling 2: fluid face forces in process, and a static solid as a co-simulation participant
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Fluid face forces on a wall are mapped to the nodes of a solid surface set by a given InterfaceMap and solved statically: resultant force, moment and work equal the fluid side's within 1e-12; for a matching planar interface under uniform pressure the solid's displacement equals the uniaxial closed form within 1e-10, and for a 3:2 non-matching one within a limit registered from the estimate of the mapping error. A StaticSolid implements cosim::Structure (advance is a static solve; restore returns the checkpoint bit for bit) and, with a test-only mock fluid whose force falls linearly with the wall displacement (stiffness k_f), coupled_step converges to u = F0/(k_s + k_f) within 1e-8 with the Aitken and with the least-squares update; a failing solve stops the step with the participant's name and both sides restored. No flow solver is coupled.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0286; the registration is written by the implementation agent before any code.

**SB-17** (ADR 0287), status todo, deps: SB-2, SB-8, SB-12, SB-14, F-013, F-080

- title: A solid case file of its own and the library entry that runs it
- acceptance:

> Every limit not stated here is registered in the ADR before the run. A solid case file (TOML of its own: mesh from a generator or a Gmsh file, material, constraints and loads on named sets, static or modal analysis, solver choice, output) is read and run by a library function of rcfd-api that writes the .vtu and a JSON report (displacement and stress extremes, reactions, frequencies, solver statistics): an unknown key, a missing set, a wrong unit or type each give an error with the key and the line; a file run gives the bits of the same model built by library calls (limit 1e-12, expected identical); the cantilever and the bar-frequency checks pass when run from files, and a Gmsh tetrahedral mesh of a box read through rcfd-io gives the uniaxial closed form within 1e-10; no flow case key, CLI help text, MCP tool list, hash table or golden changes (the existing schema, CLI and tool-list tests pass unedited).

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0287; the registration is written by the implementation agent before any code.

**SB-18** (ADR 0288), status todo, deps: SB-2, SB-6, SB-7, SB-14, SB-17, F-132

- title: Demonstration on a fan blade: static response to centrifugal and pressure load, and the modes at rest; user page
- acceptance:

> Every limit not stated here is registered in the ADR before the run. One blade of the parametric fan geometry, meshed in quadratic tetrahedra and clamped at the hub: under the centrifugal load at a registered speed the reactions equal rho omega^2 V r_c within 1e-8 and under a uniform pressure the integrated pressure force within 1e-8; the strain energy equals half the work of the loads within 1e-8; between two meshes (the second with at least twice the elements) the tip displacement and the first natural frequency change by less than 5 percent, both numbers recorded; as the flat-plate limit, an untwisted rectangular cantilever plate of the blade's span, mean chord and thickness has its first bending frequency between the Euler-Bernoulli beam value and that value divided by sqrt(1 - nu^2), with a 1 percent margin on each side. The page docs/structural-solver.md states the scope, the limits and that nothing is validated against a measured blade.

- note:

> Structural solver, basic tier (plan ADR 0270, 2026-11-12). Size M. Stub ADR 0288; the registration is written by the implementation agent before any code.

## Lead's note (2026-11-12): roadmap ids

The entries were added to docs/roadmap.json with these ids: SB-1 = F-370, SB-2 = F-371, SB-3 = F-372, SB-4 = F-373, SB-5 = F-374, SB-6 = F-375, SB-7 = F-376, SB-8 = F-377, SB-9 = F-378, SB-10 = F-379, SB-11 = F-380, SB-12 = F-381, SB-13 = F-382, SB-14 = F-383, SB-15 = F-384, SB-16 = F-385, SB-17 = F-386, SB-18 = F-387. The category "Structural solver" (family Solver) is in scripts/roadmap_progress.py.
