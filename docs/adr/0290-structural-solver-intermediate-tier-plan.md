# ADR 0290: Structural solver, intermediate tier: scope, architecture additions and the layout of ADRs 0291 to 0309

Status: **PROPOSED 2026-11-12** (planning only; no solver code written, nothing run). Written at main 21ec96a2 on the branch
`structural-intermediate-plan`. Not yet accepted by the maintainer. The decisions marked N1 to N9 in section 5 are open and are
the maintainer's; everything else is a proposal that the implementation agents' registrations may refine.

The structural solver is planned in three tiers. The basic tier is [ADR 0270](0270-structural-solver-basic-tier-plan.md) (linear
statics and modes at rest of isotropic solids, roadmap items F-370 to F-387, all todo). This document plans the intermediate
tier on top of it: what a rotating thin blade needs (elements that bend, stress stiffening, spin softening, a Campbell diagram,
cyclic symmetry, harmonic response), dynamics in time, the first two-way coupling with the flow solver, and the entry points.
It lays the work out as nineteen ADRs, 0291 to 0309, each one work package for a single implementation agent, each with a stub
file here with status PLANNED; the registration with its limits is written by the implementation agent before any code
(`AGENT_RULES.md` rule 1). It also gives the planner of the advanced tier a first cut of the boundary (section 1.3).

It is for the maintainer (section 5 first), for the lead who enters the roadmap items (section 7), and for the planner of the
advanced tier (sections 1.3 and 2.5).

**The basic tier is a plan, not code.** Every statement here about `rcfd-fem` is a statement about what ADR 0270 says it will
be. Section 2.4 lists what this tier needs from it; two of those points are requests to the basic tier's implementers and are
cheap only if they are made before ADR 0271 is written.

Rules and parents:

* The house rules of `w49i-work/AGENT_RULES.md` apply to every work package: registration before code with limits fixed in
  advance, known-miss records, defaults bit-identical, clean-room, no new third-party crate, no unsafe code, no golden or
  hash-table edit without the maintainer, no edit of an existing test.
* Parents: ADR 0270 (section 1.2 is the first cut of this tier; sections 2.2 to 2.5 are the conventions and interfaces used
  here unchanged); [fan project breakdown](../fan-project-breakdown.md) items E3 (unsteady blade-load harmonics as forcing) and
  E5 (blade and housing response, modal analysis, FSI); [fan milestone](../milestone-fan.md).
* What the coupling connects to: `rcfd_fv::cosim` and `docs/cosimulation-protocol.md`
  ([ADR 0256](0256-f139-registration-cosimulation-participant.md), F-139), the strong coupling and its added-mass argument
  ([ADR 0250](0250-f251-registration-strong-fluid-body-coupling.md), F-251), the wall pressure on a prescribed-gradient wall
  ([ADR 0252](0252-f351-registration-added-mass-deficit-wall-pressure.md), F-351), the least-squares update
  ([ADR 0261](0261-f361-registration-quasi-newton-coupling.md), F-361), and the interface mapping built from two surfaces (ADR
  0263, F-362: on the branch `f362-interface-mapping`, registered, code written, not yet run).

## What was read (disclosed)

* **Read:** `AGENT_RULES.md`; ADR 0270 in full and the stub of ADR 0284 (for the stub style); `docs/cosimulation-protocol.md` in
  full; `docs/milestone-fan.md`; the vibration part of `docs/fan-project-breakdown.md`; the heads (status, parents, house rules)
  of ADR 0250, 0261 and, from its branch, ADR 0263 to its derivation D1; the passages of ADR 0257 on optional tables and its
  check K0; the public signatures of `rcfd-fv/src/cosim.rs` and `fsi.rs` and of `rcfd-linalg` (`amg.rs`, `solvers.rs`,
  `precond.rs`, `matrix.rs`); the category row of `scripts/roadmap_progress.py`.
* **Not read:** ADR 0251 (overset donors: not relevant to a body-fitted moving wall); the bodies of ADR 0250, 0252, 0256 and
  0261 beyond their heads (the protocol page was read instead); how a flow case describes a moving wall (`rcfd-api/src/dynmesh.rs`);
  `rcfd-mesh::fan`; the CLI and MCP sources. Nothing was built or run.
* **Findings that shape the plan:**
  1. The protocol exchanges one force vector and one displacement vector per structural node. A dynamic solid fits
     `cosim::Structure` as it is; nothing in `rcfd-fv::cosim` has to change. Rotations of shell nodes are not exchanged, so a
     shell is coupled through its mid-surface displacement only.
  2. F-362 builds the weights from the nodes and faces of the two surfaces (`InterfaceMap::build`, `interface_rows`). The solid
     side needs only what ADR 0270 section 2.5 item 2 already promises (nodes and faces of a surface set).
  3. The protocol page states the added-mass bound of the plain iteration and that Aitken's factor and the least-squares update
     exist. A structure with mass is therefore enough for a real incompressible flow; the static participant of ADR 0286 is not.
  4. `rcfd-linalg`'s eigensolver (ADR 0283, planned) solves `K x = lambda M x` with `M` positive definite. Buckling has an
     indefinite second matrix, and cyclic modes are complex Hermitian: the first needs one new function, the second is turned
     into a real problem of twice the size so that it needs none.
  5. ADR 0257 established, and checked (K0), that an optional table can be added to the flow case with no hash table or golden
     edited. A CLI subcommand and an MCP tool cannot be added that way.
  6. The fan's flow side is not validated (FAN-01 misses, F-126 not met), and the unsteady blade loads of item E3 do not exist
     yet. A coupled or forced-response number for the real fan can therefore only be a demonstration in this tier.

## 1. Scope of the intermediate tier and the boundary to the advanced tier

### 1.1 Intermediate tier (this plan)

Ordered by what the fan goal needs first.

* **Rotating parts (the fan blade at speed):** geometric stiffness of solid elements from a static stress state; pre-stressed
  modal analysis; spin softening; frequencies over speed with mode tracking and the engine-order crossings (Campbell table);
  linear buckling as the second use of the same matrix. In the rotating frame, without the Coriolis term.
* **Bladed disc:** linear multi-point constraints by a transformation; rigid sets with a reference point; elastic supports;
  cyclic symmetry for statics and for modes by harmonic index.
* **Elements that bend:** Hex20; Hex8 with B-bar and with incompatible modes; six unknowns per node with a beam element; a
  four-node shell.
* **Dynamics:** linear transient (Newmark, generalised-alpha, Rayleigh damping); harmonic response by mode superposition with
  modal damping and a static correction; effective masses; Fourier harmonics of a sampled force history (the forcing of E3).
* **Geometric nonlinearity:** total Lagrangian solids, Newton with load steps, dead loads.
* **Two-way coupling:** a dynamic solid behind `cosim::Structure` on weights built by F-362, checked with mock fluids; then
  with the flow solver on a case with a closed form (a piston on an elastic bar); a `[solid]` table in the flow case and the
  library entry that runs it.
* **Scale:** aggregation multigrid with rigid-body modes as the preconditioner of CG.
* **Entry points:** the analyses above in the solid case file; apart from the library work and waiting for the maintainer, a
  CLI subcommand and MCP tools.
* **Demonstration:** the fan blade's Campbell table, the bladed hub by cyclic symmetry, the response to blade-passing forcing.
* **Verification:** closed forms and equivalences only (sector against full rotor, file against library, transformed against
  eliminated constraints). No published benchmark value is used (decision N7).

### 1.2 Departures from the first cut of ADR 0270 section 1.2, and why

The cap is nineteen packages (0291 to 0309) and the fan comes first. What was dropped from the first cut goes to the advanced
tier unless said otherwise.

1. **J2 plasticity is moved to the advanced tier** (decision N1). The fan does not need it, its verification is not cheap
   (ADR 0270 said so), and it belongs with the other materials that carry history. The nonlinear package (ADR 0300) calls the
   material through the state slot with commit and roll-back, so plasticity later adds a material, not a solver.
2. **Tied (non-matching) meshes are moved** to the advanced tier with bonded contact: a tie is a node-to-surface search plus
   the linear constraints of ADR 0296, and the search is the contact search. A bladed disc sector is one conforming mesh.
3. **Wedge and pyramid elements, a triangular shell, a mixed displacement-pressure element** are moved: no mesh source of the
   repository produces a solid wedge or pyramid (ADR 0272 refuses them), B-bar covers near incompressibility for linear
   materials, and the triangle is a second shell formulation (N6).
4. **Geometric stiffness of beams and shells is left out.** Stress stiffening is done for solid elements, which is the blade
   route of the basic tier (quadratic tetrahedra) and of ADR 0292 and 0293. A shell blade therefore gets modes at rest and a
   harmonic response in this tier, not a Campbell diagram.
5. **Follower pressure, arc-length control, nonlinear transient dynamics** are moved. In a coupled run the fluid force is
   already evaluated on the deformed wall, so the follower term matters only for stand-alone pressure loads at large rotation.
6. **Orthotropic elasticity and temperature-dependent properties are moved** to the advanced tier with laminates. A moulded
   glass-filled blade is anisotropic, so this is the departure most likely to be questioned for the fan; it is one S package
   (a material variant) and can be pulled forward without touching anything else.
7. **Superconvergent stress recovery, an error estimate and adaptation, a fill-reducing ordering and an iterative eigensolver
   are moved.** The blade models of this tier stay within the direct solver's reach (a few times 1e4 unknowns); the multigrid
   of ADR 0305 serves the static and the nonlinear solve only.
8. **The transport of the protocol** (the calls over the JSON-RPC service) is not structural work and is left to an item of the
   coupling category.
9. **Added:** a scaffold package (ADR 0291), so that the waves do not collide in shared files; effective masses and the
   Fourier harmonics of a force history (inside ADR 0302), which is the link from fan item E3 to a harmonic load; the
   pre-stressed modal analysis and buckling share one package because they share one matrix.
10. **Flutter margin of the fan is not delivered.** The tier delivers the coupled machinery checked on a case with a closed
    form. A coupled run on the rotating blade passage needs the rotating frame and the moving wall together on a flow that is
    not validated (finding 6); it is named in section 1.3.

### 1.3 Advanced tier (first cut for its planner)

From ADR 0270 section 1.3, unchanged: contact (bonded, frictionless, frictional), composites and laminates with failure
criteria, hyperelastic, viscoelastic and creep materials, fatigue and fracture parameters, flexible-rotor dynamics,
substructuring and component mode synthesis, explicit dynamics, distributed-memory and GPU solves, coupled thermo-mechanical
problems, topology optimisation.

Added by this plan:

* **Materials:** J2 plasticity (first, on the Newton driver and the state slot of ADR 0300), orthotropic elasticity,
  temperature-dependent properties, a mixed displacement-pressure element.
* **Constraints:** tied non-matching meshes (with bonded contact), cyclic symmetry with nodes on the axis, non-matching cyclic
  boundaries.
* **Elements:** wedge, pyramid, a triangular shell, geometric stiffness of beams and shells (so that a shell blade gets a
  Campbell diagram), beam-to-solid and shell-to-solid joints checked.
* **Rotation:** the Coriolis and gyroscopic terms with a solver for the damped (complex) eigenproblem, the Campbell diagram in
  the stationary frame with forward and backward whirl, the join to `rcfd-post::rotordyn`; pre-stress from a nonlinear solve
  (untwist).
* **Nonlinear:** follower pressure, arc-length control, nonlinear transient, snap-through.
* **Dynamics:** direct harmonic solve, damped modes, response spectra, random vibration.
* **Coupling:** a public coupled benchmark (the flexible flag behind a cylinder) once its values are in the repository; a
  coupled run on the rotating fan passage and the aerodynamic damping of a blade mode (flutter margin); more than one interface;
  shells wetted on both sides; heat across the interface; the transport.
* **Scale and quality:** a fill-reducing ordering and a sparse Cholesky, an iterative eigensolver on the multigrid, stress
  recovery with an error estimate, adaptation.

The boundary I am least sure of: orthotropy (fan-relevant, cheap), and whether the Coriolis term can wait (section 6).

## 2. Architecture additions

### 2.1 Where the code lives: no new crate

Everything stays in the crates of ADR 0270 section 2.1. The graph is unchanged:

```text
rcfd-linalg  gains: eigen.rs (one function, buckling form), amg_elastic.rs (new)
rcfd-fem   gains: twelve modules (section 2.2); additions to mesh.rs, model.rs, element.rs, assemble.rs, constraint.rs
rcfd-post    gains: solid_dynamic.rs, solid_fsi.rs (the participant and the coupling helper); additions to solid_output.rs
rcfd-api     gains: solid_coupled.rs; additions to solid_case.rs and (one optional table) schema.rs
rcfd-cli, rcfd-mcp: only ADR 0308, only with the maintainer's approval
```

Decision M5 of ADR 0270 asked this planner to reconsider a coupling crate. I recommend against it (N4): the glue is two files,
`rcfd-post` already depends on `rcfd-fv`, `rcfd-io` and `rcfd-fem`, and a new crate costs another approved line of
`workspace_graph.rs` for no separation that a module does not give.

### 2.2 New modules of `rcfd-fem`

Declared once by ADR 0291 as stubs, so that no later package edits `lib.rs`:

| Module | Content | Filled by |
|---|---|---|
| `hex20` | quadratic hexahedron, its rule and faces, the quadratic upgrade of a hexahedron box | 0292 |
| `hex8x` | Hex8 with B-bar and with incompatible modes | 0293 |
| `beam` | two-node beam, sections, section forces | 0294 |
| `shell` | four-node shell, resultants and surface stresses | 0295 |
| `mpc` | transformation `u = T q + g`, linear constraints, rigid sets, supports | 0296 |
| `cyclic` | sector constraints, modes by harmonic index, expansion | 0297 |
| `geometric`, `buckling` | geometric stiffness, pre-stressed modes, buckling factors | 0298 |
| `rotating` | spin softening, modes over speed, mode tracking, Campbell table | 0299 |
| `nonlinear` | total Lagrangian internal force and tangent, Newton driver | 0300 |
| `transient` | Newmark and generalised-alpha, damping, load histories, state | 0301 |
| `harmonic` | mode superposition, effective mass, force harmonics | 0302 |

### 2.3 Data model additions (plain data, readable from a file)

* **`ElementKind`** (non-exhaustive in ADR 0270) gains `Hex20`, `Beam2`, `Shell4`.
* **Property of an element block:** `Solid { formulation: Full | BBar | Incompatible }` (default `Full`), `Beam { section,
  orientation }`, `Shell { thickness }`.
* **`Constraint`** gains `Linear { terms, value }`, `RigidSet { nodes, reference }`, `Cyclic { low, high, axis, origin,
  sectors }`; supports are a new list (`ElasticSupport`: per node and direction, or per area on a surface set).
* **`DofMap`:** three or six unknowns per node, six where a beam or shell is attached. Rotations are small rotations about the
  global axes.
* **Analyses are functions with their own settings and result types**, in their own modules; `SolidModel` does not learn about
  them: `geometric::prestressed_modes`, `buckling::factors`, `rotating::campbell`, `cyclic::modes`, `nonlinear::solve`,
  `transient::Integrator`, `harmonic::response`. Each takes the operators of ADR 0270 section 2.5 (`assemble::stiffness`,
  `mass::consistent`, the reduction) and the linear algebra of `rcfd-linalg`.
* **`Element` trait** gains, beside the existing entry points and with defaults that refuse: `geometric_stiffness(stress)`,
  `internal_force(displacement, state)`, `tangent(displacement, state)`.
* **Assembly hook:** one generic function that assembles a symmetric per-element matrix into a `CsrMatrix` in the fixed order,
  so that geometric stiffness, the spin-softening mass and the tangent do not each copy the assembly loop.

### 2.4 What must change in the basic tier's design, said plainly

1. **How models are built (request to ADR 0271, before it is written).** This tier adds a property to element blocks, variants
   to `Constraint` and a list of supports to the model. If the basic tier's tests build `SolidModel`, the blocks or
   `SolidResult` by struct literals, any new field breaks them, and existing tests may not be edited. The basic types should be
   built through constructors (and enums marked non-exhaustive, as `ElementKind` already is). If that is not done, ADR 0291
   has to carry the new data in a side structure next to the model: it works, and it is worse.
2. **`constraint::reduce` gets a second path.** ADR 0270 section 2.4 foresaw it ("which a transformation-matrix method can
   replace"). The elimination stays for models with prescribed displacements only, so their results keep their bits; any other
   constraint goes through `u = T q + g` (ADR 0296). The two paths agree to rounding, not to the bit, and the ADR says which one
   a model takes.
3. **`DofMap` must really be the only place that knows the number of unknowns per node** (ADR 0270 section 2.2 promises it).
   ADR 0294 is the first to test the promise; assembly, constraints, loads, mass, recovery and output are all in its file list
   for that reason.
4. **The eigensolver of ADR 0283 is not general enough for buckling** (finding 4): one function is appended to `eigen.rs` by
   ADR 0298. The factorisation's report of the signs of `D` (ADR 0274) is used by ADR 0299 to report a loss of stiffness.
5. **Element dispatch.** Wherever the basic tier maps an `ElementKind` to its routines (element, face, mass, output cell type),
   the map is extended once, by ADR 0291, with arms that refuse until the element's package lands.
6. **Nothing changes in `rcfd-fv::cosim`.** The static participant of ADR 0286 stays as it is; the dynamic one is a second type.

### 2.5 Interfaces the advanced tier builds on

1. **Newton driver** (`nonlinear`): residual, tangent, step control, and the material state with commit and roll-back per step.
   A material with history, contact (as an extra residual and tangent) and nonlinear dynamics plug in here.
2. **Transformation** (`mpc`): `T` and `g` as data. Ties, contact constraints handled by elimination, substructure interfaces
   and component mode synthesis (which is a transformation) reuse it.
3. **Effective operators:** `K + K_sigma`, `K_eff(Omega)`, the modes of either, as functions returning matrices and mode sets;
   the gyroscopic matrix of the advanced tier is one more operator beside them.
4. **Integrator state** (`transient`): displacement, velocity, acceleration and time, saved and restored; the participant and
   any explicit or nonlinear integrator share it.
5. **Participant:** `rcfd_post::solid_dynamic::DynamicSolid` behind `cosim::Structure`; `rcfd_api::solid_coupled` as the one
   place that builds a coupled run from files.
6. **Multigrid:** `rcfd_linalg::amg_elastic` takes a matrix with a block size and a near-null space; six unknowns per node and
   an iterative eigensolver can be added without changing its interface.

### 2.6 Coupling and the case files

* **Two files, one optional table.** The solid keeps its own case file (ADR 0287, extended by ADR 0306). A coupled case is a
  flow case with an optional table `[solid]` that names the solid file and the interface (ADR 0307). The table follows the
  route of ADR 0257, whose check K0 is repeated as this package's gate.
* **A separate entry, not `Simulation`.** The coupled run is a library function of its own that owns the time loop and calls
  `coupled_step`; the ordinary run path is not taught about solids. The one change to it is a refusal: a case that has the
  table and is run the ordinary way must stop, not run uncoupled (N3).
* **What needs the maintainer is kept apart.** The CLI subcommand and the MCP tools are ADR 0308 and nothing depends on it.
* **Time.** Both participants cover the coupling step; the solid may sub-step. The flow side is implicit Euler (protocol page),
  so the coupled result is first order in time whatever the solid does; ADR 0304 registers its limits from that.

## 3. The ADR list

Nineteen work packages, ADR 0291 to 0309. Each is sized for one implementation agent working alone under `AGENT_RULES.md`: S
is about a session, M a few hours; nothing is L (where a package is at the upper end of M, its stub says how to split it). Each
has a stub file in this directory. The acceptance texts are drafts in the roadmap's form; the implementation agent's
registration fixes every limit before its code. SI-n is the placeholder id of the roadmap entry of ADR 0290 + n.

| ADR | Package | Title | Size | Depends on |
|---|---|---|---|---|
| [0291](0291-intermediate-scaffold-modules-and-model-extensions.md) | SI-1 | Scaffold of the intermediate tier: module layout, model extension data, element-trait extension points | S | F-370, F-372, F-374, F-377 |
| [0292](0292-quadratic-hexahedron-hex20.md) | SI-2 | Quadratic hexahedron (Hex20) with its face loads, mass and the quadratic upgrade of a hexahedron box | S | SI-1, F-372, F-374, F-375, F-376, F-383 |
| [0293](0293-locking-free-hex8-b-bar-and-incompatible-modes.md) | SI-3 | Hex8 without locking: mean-dilatation (B-bar) and incompatible-mode formulations, opt-in per element block | M | SI-1, F-372, F-374, F-376, F-379 |
| [0294](0294-six-unknowns-per-node-and-beam-element.md) | SI-4 | Six unknowns per node: rotations in the DofMap, nodal moments, and a two-node shear-flexible beam element | M | SI-1, F-374, F-375, F-376, F-377, F-383 |
| [0295](0295-quadrilateral-shell-element.md) | SI-5 | Four-node shell element (flat facet, assumed shear strain): plates, housings and thin blades as surfaces | M | SI-4 |
| [0296](0296-linear-multi-point-constraints-rigid-sets-elastic-supports.md) | SI-6 | Linear multi-point constraints by a transformation, rigid sets with a reference point, elastic supports | M | SI-1, F-374, F-375 |
| [0297](0297-cyclic-symmetry-static-and-modal.md) | SI-7 | Cyclic symmetry: one sector of a rotor for statics and for modal analysis by harmonic index | M | SI-6, F-375, F-383 |
| [0298](0298-geometric-stiffness-prestressed-modal-and-linear-buckling.md) | SI-8 | Geometric (initial-stress) stiffness of solid elements: pre-stressed modal analysis and linear buckling | M | SI-1, F-374, F-376, F-382, F-383 |
| [0299](0299-rotating-part-spin-softening-and-campbell-diagram.md) | SI-9 | Modes of a rotating part: stress stiffening by the centrifugal pre-stress, spin softening, Campbell diagram | M | SI-8, F-375, F-383 |
| [0300](0300-geometric-nonlinearity-total-lagrangian-newton.md) | SI-10 | Geometric nonlinearity: total Lagrangian solid elements, Newton iteration with load steps | M | SI-8, F-374, F-375 |
| [0301](0301-transient-dynamics-newmark-generalised-alpha.md) | SI-11 | Linear transient dynamics: Newmark and generalised-alpha integration, Rayleigh damping, load histories | M | SI-1, F-373, F-374, F-383 |
| [0302](0302-harmonic-response-by-mode-superposition.md) | SI-12 | Harmonic response by mode superposition: modal damping, static correction, effective mass, force harmonics | M | SI-1, F-374, F-383 |
| [0303](0303-dynamic-structural-participant.md) | SI-13 | Dynamic solid as a co-simulation participant, on interface weights built from the two surfaces | M | SI-11, F-139, F-361, F-362, F-385 |
| [0304](0304-two-way-coupling-with-the-flow-solver.md) | SI-14 | Two-way coupling of an elastic solid with the incompressible flow solver: the piston on an elastic bar | M | SI-13, F-251, F-351, F-361, F-362 |
| [0305](0305-aggregation-amg-with-rigid-body-modes.md) | SI-15 | Aggregation multigrid for elasticity: rigid-body modes as near-null space, as a preconditioner of CG | M | F-026, F-381 |
| [0306](0306-solid-case-file-intermediate-analyses.md) | SI-16 | Solid case file: the analyses and model data of the intermediate tier | M | SI-6, SI-7, SI-8, SI-9, SI-11, SI-12, F-386 |
| [0307](0307-solid-table-in-the-flow-case-coupled-run.md) | SI-17 | A [solid] table in the flow case and the library entry that runs the coupled case | M | SI-14, SI-16, F-386 |
| [0308](0308-solid-cli-subcommand-and-mcp-tools.md) | SI-18 | Command-line subcommand and MCP tools for the solid solver (needs the maintainer: two golden files change) | S | SI-16, F-386 |
| [0309](0309-rotating-fan-blade-campbell-cyclic-and-blade-pass-response.md) | SI-19 | Demonstration on the fan rotor: Campbell diagram of the blade, the bladed disc by cyclic symmetry, response to blade-passing forcing | M | SI-7, SI-9, SI-12, SI-16, F-387 |

### ADR 0291 (SI-1): Scaffold of the intermediate tier: module layout, model extension data, element-trait extension points

**Goal.** Prepare the shared files once, so that every later package of the tier fills a file of its own and no two packages of a
wave edit the same file. Declare the new modules of `rcfd-fem` (section 2.2 of ADR 0290) and of `rcfd-post` as stubs; add the
element kinds `Hex20`, `Beam2` and `Shell4` and the per-block property data (Hex8 formulation, beam section, shell thickness); add
the constraint and support data (linear constraint, rigid set, cyclic pair, elastic support); give the `Element` trait the
defaulted entry points `geometric_stiffness`, `internal_force` and `tangent`; add one generic assembly hook that turns a
per-element symmetric matrix into a `CsrMatrix`; admit the VTK cell types 3, 9 and 25 in the output mapping. Everything new is
refused with a message naming it until its package lands. No numerics.

**Dependencies.** none in this list; roadmap items: F-370, F-372, F-374, F-377.

**Acceptance (draft).** The modules, element kinds, property data, constraint and support data and the three defaulted element
entry points of ADR 0290 section 2 exist in rcfd-fem and rcfd-post; every test of the basic tier (the sb_ files of rcfd-fem,
rcfd-linalg, rcfd-io, rcfd-post and rcfd-api) compiles and passes unedited, and a registered set of basic models (patch test,
cantilever, bar modes) gives displacements, stresses and frequencies with the same bits as before the change; the generic assembly
hook called with the element stiffness reproduces assemble::stiffness bit for bit for 1 and 8 threads; each new element kind,
formulation, constraint and support variant is refused by the static solve with an error naming the variant and the set, never
ignored; the baseline hashes are unchanged.

**Checked against.** No physics: the reference is the basic tier itself (bit identity) and the refusals. The model for a
declare-once scaffold is ADR 0271.

**Files.** `crates/rcfd-fem/src/{lib.rs, mesh.rs, model.rs, element.rs, assemble.rs}` (additions only), one-line stubs of
`hex20.rs`, `hex8x.rs`, `beam.rs`, `shell.rs`, `mpc.rs`, `cyclic.rs`, `geometric.rs`, `buckling.rs`, `rotating.rs`,
`nonlinear.rs`, `transient.rs`, `harmonic.rs`; `crates/rcfd-post/src/lib.rs` (two `pub mod` lines), stubs `solid_dynamic.rs`,
`solid_fsi.rs`, the kind-to-cell-type mapping in `solid_output.rs`; `crates/rcfd-io/src/vtk.rs` only if the finite-element writer
of ADR 0278 restricts the cell types; `crates/rcfd-fem/tests/si_scaffold.rs`.

**Size.** S. **What could make it miss.** The basic tier's types are not written yet. If its tests build `SolidModel` or the
element blocks by struct literals, a new field breaks them and house rule 2 forbids editing them: the new data then go into a side
structure passed to the new entry points, which is uglier. The lead should ask the implementer of ADR 0271 to build the model
through constructors (section 2.4 of ADR 0290).

### ADR 0292 (SI-2): Quadratic hexahedron (Hex20) with its face loads, mass and the quadratic upgrade of a hexahedron box

**Goal.** The 20-node serendipity hexahedron: shape functions, 3 x 3 x 3 Gauss rule, stiffness, consistent mass, the consistent
load of its 8-node faces, strain recovery, and the builder that upgrades a box of Hex8 to Hex20 by mid-edge nodes (with a
coordinate map for curved edges). It is the element for structured meshes of thin parts: one element through the thickness bends
correctly.

**Dependencies.** ADR 0291; roadmap items: SI-1, F-372, F-374, F-375, F-376, F-383.

**Acceptance (draft).** Hex20 on the reference element and on randomly distorted ones (registered seed): shape functions sum to 1
and derivatives to 0 within 1e-14, analytic derivatives agree with central differences within 1e-7, the 3 x 3 x 3 rule integrates
every monomial of degree <= 5 per direction within 1e-13, the stiffness is symmetric within 1e-13, has rank 54 and gives no force
for the six rigid motions within 1e-11. Patch test with all six strains on a box with displaced interior nodes: 1e-10. A uniform
pressure on an 8-node face gives the nodal loads -1/12 (corners) and 1/3 (mid-edge) of p A within 1e-13, and uniaxial tension the
closed form within 1e-10. Cantilever L/h = 20, nu = 0.3, ONE element through the thickness and 10 along: tip deflection within 1
percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A L^2)). Thick cylinder b/a = 2 in plane strain on a curved mesh: bore displacement
within 0.2 percent and bore hoop stress within 1 percent of Lame. Manufactured solution of F-378: observed L2 order >= 2.7. Mass:
total mass and inertia tensor of a box within 1e-12; first axial frequency of a fixed-free bar (nu = 0) within 0.1 percent at 8
elements. The upgrade has (nodes + edges) nodes.

**Checked against.** Closed forms of ADRs 0275, 0280 and 0284 (Timoshenko cantilever, Lame, bar frequency); exact monomial
integrals over the cube; the consistent load of a quadratic serendipity face (derivable: integral of the corner and mid-side
functions over the square). Method: Zienkiewicz and Taylor, The Finite Element Method, vol. 1 (serendipity family; from memory,
the functions are checked by the partition and derivative tests).

**Files.** `crates/rcfd-fem/src/hex20.rs` (the element, its rule, its face functions and the upgrade builder),
`crates/rcfd-fem/tests/si_hex20.rs`.

**Size.** S. **What could make it miss.** The negative corner load of a quadratic face surprises a reader and any code that
assumes positive nodal weights. Node order against VTK type 25 (the mid-edge order; to be confirmed from the public VTK format
description). Nodal stress extrapolation from 27 points is not unique: the package registers its rule.

### ADR 0293 (SI-3): Hex8 without locking: mean-dilatation (B-bar) and incompatible-mode formulations, opt-in per element block

**Goal.** The fully integrated Hex8 of the basic tier locks in bending (parasitic shear) and near incompressibility. Add two
formulations selected per element block, the default staying full integration bit for bit: the mean-dilatation B-bar (volumetric
locking) and nine incompatible modes condensed in the element, modified so that the patch test holds on distorted elements (shear
locking in bending; this is the enhanced-strain element in its simplest form).

**Dependencies.** ADR 0291; roadmap items: SI-1, F-372, F-374, F-376, F-379.

**Acceptance (draft).** Two opt-in formulations of Hex8, default unchanged bit for bit. Both pass the patch test of F-374 on
distorted meshes within 1e-10 and a single element of either has exactly six zero eigenvalues (the seventh above 1e-6 of the
largest) for aspect ratios 1, 10 and 100. Cantilever of F-379 (L/h = 20, tip shear force) with ONE element through the thickness
and 20 along: incompatible modes within 1 percent of the closed form at nu = 0 and within 3 percent at nu = 0.3, while full
integration on the same mesh is the registered locking control (below 80 percent); at L/h = 100 with elements of aspect ratio 5
incompatible modes stay within 2 percent. Thick cylinder b/a = 2 in plane strain with nu = 0.4999: B-bar gives the bore
displacement within 1 percent of Lame on the mesh on which full integration is below 90 percent (control). Recovered stress on the
patch test equals D eps within 1e-10; the condensed internal modes are recovered for the stress.

**Checked against.** Closed forms: the Timoshenko cantilever and Lame's cylinder of ADR 0280; the locking factor of a fully
integrated bilinear element in pure bending, derived in the registration (for aspect ratio 1 and nu = 0 the bending energy is
over-estimated by one half, hence the control). Methods, from memory: Hughes, Int. J. Numer. Meth. Engng 15 (1980) 1413 (B-bar);
Wilson, Taylor, Doherty and Ghaboussi (1973) and Taylor, Beresford and Wilson, Int. J. Numer. Meth. Engng 10 (1976) 1211
(incompatible modes and the patch-test correction); Simo and Rifai, Int. J. Numer. Meth. Engng 29 (1990) 1595 (the enhanced-strain
view).

**Files.** `crates/rcfd-fem/src/hex8x.rs`, `crates/rcfd-fem/tests/si_hex8x.rs`.

**Size.** M. **What could make it miss.** The 3 percent at nu = 0.3 is a draft (the transverse strain in bending is only partly
relieved by the modes). The control numbers are estimates. The two formulations are not combined in this package. Mass uses the
standard Hex8 mass. The incompatible element is not offered to the nonlinear package (ADR 0300).

### ADR 0294 (SI-4): Six unknowns per node: rotations in the DofMap, nodal moments, and a two-node shear-flexible beam element

**Goal.** Make the unknown numbering carry three or six unknowns per node (six where a beam or shell is attached), with prescribed
rotations, nodal moments, moment reactions and rotations in the output, and exercise it with the simplest element that needs it: a
straight two-node beam in space with axial, torsional, bending and shear stiffness (the exact stiffness of a prismatic Timoshenko
beam), consistent mass, section data given as numbers (A, I_y, I_z, J, shear factors, orientation vector) with helpers for
rectangular and circular sections, and section forces as results.

**Dependencies.** ADR 0291; roadmap items: SI-1, F-374, F-375, F-376, F-377, F-383.

**Acceptance (draft).** Nodes carry three or six unknowns; a model without structural elements gives the bits it gave before.
Beam, ONE element, any orientation in space (registered seed): tip deflection P L^3/(3 E I) + P L/(kappa G A), tip rotation P
L^2/(2 E I), twist T L/(G J) and elongation F L/(E A) within 1e-10, section forces at the root equal to the applied loads within
1e-10, reactions (forces and moments) balancing the loads within 1e-10; a rotation of the whole model leaves the results unchanged
within 1e-10. Quarter-circle arch of radius R clamped at one end, 32 elements, slender: deflection under the tip load pi P R^3/(4
E I) within 0.5 percent. Free beam: six zero frequencies below 1e-6 of the seventh. Pinned-pinned beam, 20 elements, slenderness
50: f_n = (n pi/L)^2 sqrt(E I/(rho A))/(2 pi), n = 1, 2, 3, within 0.5, 1 and 2 percent after the registered shear and
rotary-inertia correction. A beam node shared with a solid element is refused (no rotational stiffness from the solid) with the
node id.

**Checked against.** Closed forms: Euler-Bernoulli and Timoshenko beam theory; Castigliano's theorem for the arch (bending energy
only, derivable in a few lines); the pinned-pinned frequencies. Method: the exact stiffness matrix of a prismatic shear-flexible
beam (Przemieniecki, Theory of Matrix Structural Analysis, 1968; from memory, re-derived from the beam equations in the
registration).

**Files.** `crates/rcfd-fem/src/beam.rs`, and the generalisation in `model.rs` (`DofMap`), `assemble.rs`, `constraint.rs`,
`loads.rs`, `mass.rs`, `recover.rs`; `crates/rcfd-post/src/solid_output.rs` (rotations as point data, line cells);
`crates/rcfd-fem/tests/si_beam.rs`.

**Size.** M. **What could make it miss.** It edits many basic files, so it runs alone among packages that touch them (wave 3). ADR
0270 promises that no code outside `DofMap` assumes three unknowns per node; if that did not hold, this package finds out and
grows. Joining a beam to a solid needs the rigid set of ADR 0296 (a follow-up check, not in this package). The torsion constant of
a rectangle is a series: it is an input.

### ADR 0295 (SI-5): Four-node shell element (flat facet, assumed shear strain): plates, housings and thin blades as surfaces

**Goal.** A four-node quadrilateral shell with six unknowns per node: bilinear membrane, Reissner-Mindlin bending with the
transverse shear strain interpolated from the edge mid-points (free of shear locking in the thin limit), a small registered
stiffness on the drilling rotation, a rigid offset for warped facets, consistent mass with rotary inertia, pressure and body
loads, and results as stress resultants and surface stresses. For housings and for blades meshed as mid-surfaces.

**Dependencies.** ADR 0294; roadmap items: SI-4.

**Acceptance (draft).** Membrane patch test and constant-curvature bending patch test on a distorted flat mesh within 1e-8. Simply
supported square plate under uniform load, 16 x 16 elements, nu = 0.3: centre deflection within 1 percent of the Navier series
(0.004062 q a^4/D, recomputed in the registration) at thickness/side 0.01 and within 1.5 percent at 0.001 (no shear locking), the
fully integrated shear control being below 20 percent at 0.001. Cantilever strip, nu = 0: tip deflection P L^3/(3 E I) within 0.5
percent and surface stress 6 M/t^2 at mid-span within 2 percent. Curved: a ring strip of radius R pinched by two opposite forces,
nu = 0, 32 elements per quarter: change of the loaded diameter (pi/4 - 2/pi) P R^3/(E I) within 2 percent; a cylinder under
internal pressure with free ends: radial displacement p R^2/(E t) within 0.5 percent. Frequencies of the simply supported plate:
omega_mn = pi^2 (m^2 + n^2)/a^2 sqrt(D/(rho t)) within 1 percent for (1,1) and 3 percent for (1,2) and (2,2). A free plate has six
zero frequencies; a facet warped beyond the registered limit is refused with the element.

**Checked against.** Closed forms: Navier's double series for the simply supported Kirchhoff plate (summed in the registration),
Castigliano for the pinched ring, membrane equilibrium of a pressurised cylinder, the plate frequencies (all in Timoshenko and
Woinowsky-Krieger, Theory of Plates and Shells; derivable). Method, from memory: Dvorkin and Bathe, Eng. Comput. 1 (1984) 77 and
Bathe and Dvorkin, Int. J. Numer. Meth. Engng 21 (1985) 367 (the assumed shear strain). The published shell obstacle course
(MacNeal and Harder, Finite Elem. Anal. Des. 1 (1985) 3) is public but its values are not in the repository; it is not used
(decision N7).

**Files.** `crates/rcfd-fem/src/shell.rs`, `crates/rcfd-fem/tests/si_shell.rs`.

**Size.** M. **What could make it miss.** At the upper end of M; if it must be split: statics first, then mass and warped facets.
The drilling stiffness is a fiction with a tuning constant (registered from an estimate, not searched). Membrane locking of a
flat-facet element on a curved surface is mild but real: the ring limit is a draft. No geometric stiffness for shells in this
tier, so a shell blade gets no Campbell diagram (ADR 0290 section 1.3). No triangle.

### ADR 0296 (SI-6): Linear multi-point constraints by a transformation, rigid sets with a reference point, elastic supports

**Goal.** Replace the reduction of the system by a transformation `u = T q + g` whenever a constraint other than a prescribed
displacement is present (the elimination of the basic tier stays for models that have none, bit for bit). On it: the general
linear constraint `sum a_i u_i = b`, a rigid set of nodes that follows a reference point with six unknowns (small rotation) and
takes a force and a moment there, and elastic supports (a spring to ground per node and direction, and a foundation stiffness per
area on a surface set). This is what cyclic symmetry (ADR 0297) is built on.

**Dependencies.** ADR 0291; roadmap items: SI-1, F-374, F-375.

**Acceptance (draft).** A model with prescribed displacements only gives the bits of the basic tier; the same constraints written
as linear constraints give the same displacements within 1e-12. The reduced matrix is symmetric within 1e-13 and the constraint
forces do no work on any admissible motion within 1e-12. Skew roller: a block in uniaxial stress along a direction 30 degrees off
the axes, held by the constraint u.n = 0 on its inclined faces, has the rotated uniaxial solution within 1e-10. Periodic pair
u(high) - u(low) = eps L on a box with displaced interior nodes: uniform strain within 1e-10. Rigid set on the end face of a bar:
an axial force at the reference point gives F L/(E A) within 1e-10 (nu = 0), a moment the tip rotation M L/(E I) within 1 percent
(Tet10, slenderness 20), and the constraint forces have the resultant and moment applied within 1e-10. Supports: a bar on an end
spring k has the tip displacement F (1/k + L/(E A)) within 1e-10; a block on a foundation k_f under pressure p settles by p/k_f
within 1e-10. A redundant constraint is dropped with a report, a contradictory one and a chain that closes on itself are refused
naming the constraints.

**Checked against.** Closed forms: uniaxial Hooke's law in rotated axes, springs in series, beam bending by an end moment. Method:
master-slave elimination written as a congruence transformation (Cook, Malkus, Plesha and Witt, Concepts and Applications of
Finite Element Analysis, the chapter on constraints; from memory); derivable from the principle of virtual work.

**Files.** `crates/rcfd-fem/src/mpc.rs`, `crates/rcfd-fem/src/constraint.rs` (the dispatch to the transformation),
`crates/rcfd-fem/tests/si_mpc.rs`.

**Size.** M. **What could make it miss.** `T^T K T` fills the matrix where a rigid set couples many nodes (the direct solver's
envelope grows; reported, not hidden). Choosing the eliminated unknown of each constraint so that chains resolve; detecting
redundancy needs a rank decision with a registered threshold. Tied non-matching meshes are NOT included (advanced, with bonded
contact).

### ADR 0297 (SI-7): Cyclic symmetry: one sector of a rotor for statics and for modal analysis by harmonic index

**Goal.** A bladed disc of N identical sectors is solved on one sector. Statics under a load that repeats in every sector: the
high boundary equals the low boundary rotated by 2 pi/N, a real linear constraint (ADR 0296). Modal analysis for each harmonic
index k = 0 .. N/2: the boundaries differ by the phase k 2 pi/N, solved as a real problem of twice the size, giving single modes
for k = 0 and N/2 and pairs otherwise, expanded to the full rotor for output. The two boundary surface sets must match node for
node under the rotation.

**Dependencies.** ADR 0296; roadmap items: SI-6, F-375, F-383.

**Acceptance (draft).** Equivalence: for a ring and for a disc with N = 12 sectors, the frequencies of the sector for every
harmonic index 0 to 6 equal the frequencies of the full 360 degree model built from the same sector mesh within 1e-8 relative,
with the same count below a registered cut-off (single for k = 0 and 6, double otherwise); the expanded mode shapes satisfy the
full model's eigen-equation within 1e-7. Closed forms: in-plane bending of a thin ring (thickness/radius 0.05, nu = 0), omega_n^2
= E I n^2 (n^2 - 1)^2/(rho A R^4 (n^2 + 1)) for n = 2 and 3 found at harmonic index 2 and 3 within 2 percent; a thin annular disc
b/a = 4 spinning about its axis (statics, k = 0): rim displacement rho omega^2 b ((3 + nu) a^2 + (1 - nu) b^2)/(4 E) within 1
percent. Boundaries that do not match under the rotation within the registered tolerance are refused with the first unmatched
node; N < 2 and a harmonic index above N/2 are refused.

**Checked against.** The full model is the reference for the equivalence (Bloch-Floquet decomposition of a rotationally periodic
structure: Thomas, Int. J. Numer. Meth. Engng 14 (1979) 81, from memory; derivable from the block-circulant structure of K and M).
Closed forms: the inextensional ring frequencies (Hoppe; Timoshenko, Vibration Problems in Engineering; re-derived by Rayleigh's
method in the registration) and the rotating annular disc in plane stress (Timoshenko and Goodier; re-derived).

**Files.** `crates/rcfd-fem/src/cyclic.rs`, `crates/rcfd-fem/tests/si_cyclic.rs`.

**Size.** M. **What could make it miss.** The doubled real problem has every eigenvalue twice: the subspace must be large enough
and the pairs must be recognised, not reported as two modes. Nodes on the axis belong to both boundaries (refused in this package;
the disc check is an annulus). The full model of the equivalence check must stay within the direct solver's reach. Pre-stress and
spin softening on a sector (a rotating bladed disc) combine this package with ADR 0299 only in the demonstration (ADR 0309).

### ADR 0298 (SI-8): Geometric (initial-stress) stiffness of solid elements: pre-stressed modal analysis and linear buckling

**Goal.** From the stress of a linear static solution, the geometric stiffness `K_sigma = int G^T S G dV` of Tet4, Hex8, Tet10
(and Hex20 when present). With it: the modal analysis of a pre-stressed part (`K + K_sigma`), and linear buckling, the lowest
positive load factors of `(K + lambda K_sigma) x = 0`, for which `rcfd-linalg::eigen` gains an iteration on the factorisation of K
(the mass-matrix form of ADR 0283 does not apply: `K_sigma` is indefinite).

**Dependencies.** ADR 0291; roadmap items: SI-1, F-374, F-376, F-382, F-383.

**Acceptance (draft).** K_sigma is symmetric within 1e-13 of its norm, gives no force for a rigid translation within 1e-12, and
for a uniform uniaxial stress s and a linear displacement field w gives x^T K_sigma x = s V |grad w . e|^2 within 1e-12.
rcfd-linalg: for K = tridiagonal (2, -1), n = 200, and K_sigma = -I the ten lowest buckling factors equal 4 sin^2(j pi/(2 (n +
1))) within 1e-10; a K_sigma with no negative direction returns no factor and says so. Pre-stressed modes: a beam on knife-edge
supports at its neutral axis (slenderness 20, nu = 0) under an axial tension stress s has (omega_n^2 - omega_n,0^2) = (n pi/L)^2
s/rho within 1 percent for n = 1 and 2, and under compression the frequency falls by the same amount. Buckling: the same beam in
compression has the critical load pi^2 E I/L^2 reduced by the shear term (0.6 percent at this slenderness, registered) within 1
percent, the second factor four times the first within 2 percent; a clamped-free column pi^2 E I/(4 L^2) within 1 percent (Tet10
or Hex20).

**Checked against.** Closed forms: Euler's column with Engesser's shear correction P_E/(1 + P_E/(kappa G A)); the beam under axial
force, omega_n^2 = (n pi/L)^4 E I/(rho A) + (n pi/L)^2 T/(rho A) (derivable from the beam equation with the term T w''). The
geometric stiffness is the second variation of the Green-Lagrange strain energy at fixed stress (Bathe, Finite Element Procedures,
the total Lagrangian formulation; Zienkiewicz and Taylor, vol. 2; from memory, derived in the registration).

**Files.** `crates/rcfd-fem/src/{geometric.rs, buckling.rs}`, `crates/rcfd-linalg/src/eigen.rs` (appended function),
`crates/rcfd-fem/tests/si_geometric.rs`, `crates/rcfd-linalg/tests/si_buckling_eigen.rs`.

**Size.** M. **What could make it miss.** Knife-edge supports on a solid mesh need nodes on the neutral axis and leave a local
stress concentration. The frequency-difference check is built to cancel the discretisation error of the unstressed beam; if the
two runs do not share the mesh it does not. Beams and shells get no geometric stiffness here. Buckling under a load that has a
fixed and a scaled part (pre-load) is not included.

### ADR 0299 (SI-9): Modes of a rotating part: stress stiffening by the centrifugal pre-stress, spin softening, Campbell diagram

**Goal.** The modal analysis of a part rotating at constant speed, in the rotating frame: `K_eff = K + Omega^2 (K_sigma,1 -
M_perp)`, with `K_sigma,1` the geometric stiffness of the centrifugal stress at unit speed (ADR 0298, one static solve) and
`M_perp` the mass matrix of the motion perpendicular to the axis (spin softening). Frequencies over a list of speeds with the
modes tracked by shape, the crossings with the engine-order lines `f = k Omega/(2 pi)`, and the table of a Campbell diagram as
data (CSV and JSON). The Coriolis coupling is neglected and the ADR says so (decision N5).

**Dependencies.** ADR 0298; roadmap items: SI-8, F-375, F-383.

**Acceptance (draft).** At zero speed the result has the bits of the modal analysis of F-383. Radial bar clamped on the axis, nu =
0, axial modes: (omega_n,0^2 - omega_n^2)/Omega^2 = 1 within 1e-3 for n = 1, 2 at a speed with centrifugal stress below 1e-4 E.
Radial beam of square section clamped on the axis (slenderness 20): the first out-of-plane (flap) and in-plane (lag) frequencies
satisfy omega_flap^2 - omega_lag^2 = Omega^2 within 1e-4 Omega^2 on a symmetric mesh; the stiffening (omega_flap^2 -
omega_0^2)/Omega^2 equals at Omega = 0.2 omega_0 the Rayleigh quotient of the centrifugal tension with the mode at rest (1.19,
recomputed in the registration) within 2 percent and stays between 1 and that value up to Omega = 5 omega_0. Campbell table: on a
synthetic family omega^2 = omega_0^2 + S Omega^2 the crossings with orders 1 to 4 equal omega_0/sqrt(k^2 - S) within 1e-8, and on
the beam within 1 percent; a flap and a lag mode of a rectangular section that cross are tracked without exchange (modal assurance
above 0.9). A speed at which K_eff is not positive definite is reported as a loss of stiffness with the speed, never as a
non-finite frequency.

**Checked against.** Closed forms and bounds, all derivable: the radial bar in the rotating frame, rho u_tt = E u_rr + rho Omega^2
u; the flap-lag identity of a symmetric section; the first-order perturbation of an eigenvalue (the slope of omega^2 against
Omega^2 at rest is the Rayleigh quotient of the added stiffness); Southwell's lower bound omega^2 >= omega_0^2 + omega_string^2
with the flapping frequency Omega of a rotating string clamped on the axis (Legendre's equation). Southwell and Gough, British ARC
R&M 766 (1921), from memory.

**Files.** `crates/rcfd-fem/src/rotating.rs`, `crates/rcfd-fem/tests/si_rotating.rs`.

**Size.** M. **What could make it miss.** Without the Coriolis term the frequencies of modes that move in the plane of rotation
are approximate; for a blade on a stiff hub the error is small, for a disc it is not, and the package cannot measure it. The
pre-stress is linear in Omega^2 (no large untwist). The 1e-4 of the flap-lag identity assumes the two bending planes are
discretised identically.

### ADR 0300 (SI-10): Geometric nonlinearity: total Lagrangian solid elements, Newton iteration with load steps

**Goal.** Large displacement and rotation with small strain: Green-Lagrange strain and second Piola-Kirchhoff stress with the
linear material (St. Venant-Kirchhoff) for Tet4, Tet10, Hex8 (full integration) and Hex20, internal force and consistent tangent
(material part plus the geometric part of ADR 0298), and a Newton solve with load steps, a residual and an energy criterion, step
halving on divergence and a located refusal when halving is exhausted. The material is called through `stress(strain, state)` with
commit and roll-back of the state per step, so that a material with history (advanced tier) fits without changing the driver. Dead
loads only.

**Dependencies.** ADR 0298; roadmap items: SI-8, F-374, F-375.

**Acceptance (draft).** The tangent agrees with central differences of the internal force within 1e-6 relative on distorted
elements at displacements of 20 percent of the element size (registered seed). A rigid rotation by 90 degrees with a translation
gives an internal force below 1e-10 E L^2. Uniform stretch lambda = 1.2 of a bar with nu = 0: nominal stress E lambda (lambda^2 -
1)/2 within 1e-9; under load control the last three residuals of each step show an order >= 1.8. A load factor of 1e-6 reproduces
the linear solution within 1e-5 relative. Elastica: a cantilever of slenderness 50, nu = 0, with tip force P L^2/(E I) = 1 and 2
has the tip deflection and shortening within 2 percent of the elliptic-integral solution (recomputed in the registration by
integrating the elastica equation; recalled 0.302 L, 0.056 L and 0.494 L, 0.161 L). An element inverted during the iteration and a
step that does not converge after the registered halvings are errors with the element or the step.

**Checked against.** Closed forms: the uniaxial St. Venant-Kirchhoff bar (one line from S = E E_GL and P = F S); invariance of the
Green-Lagrange strain under rotation; the elastica (Bisshopp and Drucker, Q. Appl. Math. 3 (1945) 272, from memory; the reference
numbers are computed in the registration from the ordinary differential equation, not taken from the paper). Method: Bathe, Finite
Element Procedures, chapter 6 (total Lagrangian formulation), from memory, derived in the registration.

**Files.** `crates/rcfd-fem/src/nonlinear.rs`, `crates/rcfd-fem/tests/si_nonlinear.rs`.

**Size.** M. **What could make it miss.** The elastica compares a 3-D solid with small but finite strain (1 percent at this
slenderness) against an inextensible rod: the 2 percent is a draft. Tet4 locks and is only checked on the uniform states. Follower
pressure, arc-length control and nonlinear dynamics are not included (advanced). J2 plasticity is not in this tier (decision N1).

### ADR 0301 (SI-11): Linear transient dynamics: Newmark and generalised-alpha integration, Rayleigh damping, load histories

**Goal.** Time integration of `M a + C v + K u = f(t)` for a linear model: the average-acceleration Newmark method and the
generalised-alpha method with the high-frequency spectral radius as its one parameter, Rayleigh damping `C = a M + b K`, loads
scaled by tabulated histories, initial displacement and velocity, one factorisation reused over the steps, and a state that can be
saved and restored bit for bit (the co-simulation participant of ADR 0303 needs that).

**Dependencies.** ADR 0291; roadmap items: SI-1, F-373, F-374, F-383.

**Acceptance (draft).** Free vibration started in the first axial mode of a bar (the discrete eigenvector): with Newmark the
amplitude after 100 periods is within 1e-9 and the total energy constant within 1e-10; the measured period is long by (omega
dt)^2/12 within 5 percent of that amount at omega dt = 0.2 and 0.1, observed order >= 1.9. A step load proportional to M phi_1
gives the peak displacement twice the static one within 0.5 percent at 40 steps per period. Rayleigh damping with a registered
ratio 0.02 on that mode: logarithmic decrement 2 pi zeta/sqrt(1 - zeta^2) within 1 percent. Generalised-alpha: with spectral
radius 1 the result equals Newmark within 1e-12; with 0.5 and 0.8 a mode at omega dt = 1e3 decays per step by that factor within
1e-3 while the period error of the first mode keeps order >= 1.9. A run split in two by save and restore has the bits of the
unsplit run; results are bit-identical for 1 and 8 threads; a non-positive step, a table that does not cover the run and a
singular effective matrix are refused.

**Checked against.** Closed forms: the single oscillator (free, damped, step-loaded); the period error of the average-acceleration
method, from tan(omega_h dt/2) = omega dt/2 (derivable). Methods, from memory: Newmark, J. Eng. Mech. Div. ASCE 85 (1959) 67;
Chung and Hulbert, J. Appl. Mech. 60 (1993) 371 (the parameters in terms of the spectral radius are re-derived or checked by the
decay test, not taken on trust).

**Files.** `crates/rcfd-fem/src/transient.rs`, `crates/rcfd-fem/tests/si_transient.rs`.

**Size.** M. **What could make it miss.** Starting accelerations need a solve with M; a constrained model must do it on the
reduced system. The decay test at omega dt = 1e3 needs a mode that is isolated in the discrete model (a two-unknown model is
used). Nonlinear transient (Newton inside the step) is not included.

### ADR 0302 (SI-12): Harmonic response by mode superposition: modal damping, static correction, effective mass, force harmonics

**Goal.** The steady response to loads `Re(f e^{i omega t})` from the modes of ADR 0284: modal damping ratios, a static correction
for the truncated modes, a sweep over frequencies, amplitude and phase of displacement and stress at chosen nodes; modal
participation factors and effective masses; and the Fourier harmonics of a periodically sampled force history, which is how the
unsteady blade loads of fan breakdown item E3 (blade-passing forcing) become harmonic loads.

**Dependencies.** ADR 0291; roadmap items: SI-1, F-374, F-383.

**Acceptance (draft).** A load proportional to M phi_1 at omega_1 gives the amplitude of the static response divided by 2 zeta
within 1e-8 and a phase lag of 90 degrees within 1e-6 rad; the half-power width of that peak is 2 zeta omega_1 within 1 percent
(zeta = 0.01). Fixed-free bar with nu = 0 under a harmonic tip force, undamped, at 0.5 and 1.5 times the first frequency: tip
amplitude F tan(k L)/(E A k), k = omega/c, within 0.5 percent with 10 modes and the static correction, the same without the
correction being the registered control (error above 1 percent at 0.5). Effective mass: the first axial mode of the bar carries
8/pi^2 of the mass within 0.5 percent, the first bending mode of a cantilever 0.613 (recomputed in the registration) within 1
percent, and the sum over all modes of a small model equals the unconstrained mass within 1e-10. Force harmonics of a sampled
signal with three harmonics: amplitudes and phases within 1e-12; a record that is not a whole number of periods is refused.

**Checked against.** Closed forms: the damped oscillator at resonance and its half-power width; the bar under a harmonic end force
(wave equation, one line); modal expansions of the bar and the Euler-Bernoulli cantilever for the effective masses; the discrete
Fourier sum. Method: mode superposition with the mode-acceleration (static) correction (Craig and Kurdila, Fundamentals of
Structural Dynamics; from memory; derivable from the spectral expansion of the inverse of K).

**Files.** `crates/rcfd-fem/src/harmonic.rs`, `crates/rcfd-fem/tests/si_harmonic.rs`.

**Size.** M. **What could make it miss.** Modal damping is an input: no number for a moulded plastic blade is in the repository,
so every response level scales with an assumed ratio. A direct (full) harmonic solve and damped or complex modes are not included
(advanced). With pre-stress the modes of ADR 0299 can be passed in; that pairing is checked only in the demonstration.

### ADR 0303 (SI-13): Dynamic solid as a co-simulation participant, on interface weights built from the two surfaces

**Goal.** `DynamicSolid` implements `cosim::Structure` with the time integration of ADR 0301: `advance` integrates over the
coupling step with the nodal forces held, in one or more sub-steps; checkpoint and restore are bit for bit. The interface weights
are no longer given: they are built by the geometric mapping of F-362 from the nodes and faces of a solid surface set and the
fluid wall. Checked against mock fluids only (an added mass, a spring); the flow solver is ADR 0304.

**Dependencies.** ADR 0301; roadmap items: SI-11, F-139, F-361, F-362, F-385.

**Acceptance (draft).** A bar (nu = 0) whose end face is the interface, coupled by coupled_step to a mock fluid that returns the
force -m_a times the wall acceleration: the first coupled frequency equals the root of k L tan(k L) = rho A L/m_a within 1 percent
at 100 steps per period, for mass ratios m_a/(rho A L) of 0.5 and 4; at ratio 4 the plain iteration (factor 1) diverges as the
protocol predicts (registered control) while Aitken's factor and the least-squares update converge to 1e-8 within registered
iteration caps. With a mock spring k_f and a slow load the response tends to F_0/(k_s + k_f) of F-385 within 1e-6. Mapping built
by F-362 for a matching and for a 3:2 non-matching interface: resultant force, moment and work conserved within 1e-12, and the
coupled frequency changes by less than 1e-3 between the two. Restore returns the checkpoint bit for bit; a failing structural step
stops the coupled step with the participant's name and both sides restored. No flow solver is coupled.

**Checked against.** Closed form: the longitudinal vibration of a bar with a mass on its end, k L tan(k L) = rho A L/m_a
(derivable from the wave equation with the boundary condition E A u_x = -m_a u_tt). The stability bound of the plain iteration is
the one of `docs/cosimulation-protocol.md` (ADR 0256), the updates those of ADR 0261, the weights those of ADR 0263.

**Files.** `crates/rcfd-post/src/solid_dynamic.rs`, `crates/rcfd-post/tests/si_solid_dynamic.rs`.

**Size.** M. **What could make it miss.** F-362 is registered and not yet run: if its builder misses, this package falls back to
given weights and the mapping clause is not run (partial). The mock's acceleration is a difference of displacements over the
coupling step, which adds a lag the registration must estimate before fixing the 1 percent. Shell interfaces (mid-surface, both
sides wetted) are not covered.

### ADR 0304 (SI-14): Two-way coupling of an elastic solid with the incompressible flow solver: the piston on an elastic bar

**Goal.** The first run in which the flow solver and the structural solver exchange force and displacement to convergence within
each time step: `cosim::FlowWall` (the moving-mesh PIMPLE step with the wall pressure of ADR 0252) coupled by `coupled_step` to
`DynamicSolid`. The case has a closed form: a channel of incompressible fluid, open at the far end, closed at the near end by the
end face of an elastic bar, so that the fluid is an added mass `rho_f A L_f` on the bar (the spring-mounted piston of F-244 and
F-251 with the spring replaced by a solid). Adds the helper that turns interface displacements into mesh point positions for this
geometry.

**Dependencies.** ADR 0303; roadmap items: SI-13, F-251, F-351, F-361, F-362.

**Acceptance (draft).** Elastic bar (nu = 0) closing a channel of incompressible fluid of length L_f, started from a displaced
state, mass ratios rho_f L_f/(rho_s L_s) of 0.5 and 4: the coupled iteration converges in every step to 1e-6 with Aitken's factor
and with the least-squares update, iteration counts recorded; the frequency of the first coupled mode equals the root of k L tan(k
L) = rho_s L_s/(rho_f L_f) within 2 percent after the registered estimate of the time-step and viscous effects; over ten periods
the work of the fluid on the interface equals the change of the solid's energy within 1e-4 of its amplitude; the loose coupling
(one pass) at ratio 4 grows (registered control). With a 3:2 non-matching interface mapped by F-362 the frequency changes by less
than 2e-3. A failure of either participant leaves both at the start of the step. The existing co-simulation tests (cosim_f139,
cosim_f361, cosim_f362) pass unedited.

**Checked against.** Closed form: the bar with an end mass (as ADR 0303) with m_a = rho_f A L_f, the added mass of a column of
incompressible fluid moved as a plug (momentum equation of the column; this is the model M6 of ADR 0211 and ADR 0250, whose
measured coupling lag is the basis of the time-step estimate). No published coupled benchmark is used: the flexible flag behind a
cylinder (Turek and Hron, 2006) is public but needs large mesh motion and a nonlinear structure and its values are not in the
repository (advanced tier).

**Files.** `crates/rcfd-post/src/solid_fsi.rs`, `crates/rcfd-post/tests/si_fsi_piston.rs`; `docs/cosimulation-protocol.md` (the
limits section: a structural solver has been connected).

**Size.** M. **What could make it miss.** The riskiest package of the tier. The flow side is implicit Euler: its damping of the
oscillation and the lag of the pairing may exceed the 2 percent (the F-244 piston measured 0.13 percent in frequency and 4.7
percent in amplitude per turning point at T/100). The run time of a moving-mesh PIMPLE case with several coupling iterations per
step decides the mesh; the check is a heavy record with a short fast variant. A wall that bends (non-uniform motion) is recorded,
without a limit.

### ADR 0305 (SI-15): Aggregation multigrid for elasticity: rigid-body modes as near-null space, as a preconditioner of CG

**Goal.** The one-level preconditioners of ADR 0282 lose on slender bodies and large models. Add, beside the scalar AMG and
without touching it, a smoothed-aggregation multigrid for matrices with three unknowns per node: aggregation on the node graph, a
tentative prolongator that carries the six rigid-body modes of each aggregate (from the node coordinates, by a local QR), a
damped-Jacobi smoothed prolongator, a block smoother, a dense coarsest solve; used as the preconditioner of CG and selectable in
the static solve. The default solver is unchanged.

**Dependencies.** none in this list; roadmap items: F-026, F-381.

**Acceptance (draft).** On every level the rigid-body modes are in the range of the prolongator: |B - P B_c| <= 1e-12 |B|. CG
preconditioned by one V-cycle reaches a relative residual of 1e-10 on a clamped cube of Hex8 with about 5e3, 4e4 and (heavy
record) 3e5 unknowns in a number of iterations that grows by at most 1.5 times from the smallest to the largest and stays below
40; on a cantilever of slenderness 20 with about 1e5 unknowns it needs at most one fifth of the iterations of the incomplete
Cholesky of F-381; the same on a Tet10 mesh within twice the Hex8 count. The solution agrees with the direct solver within 1e-7 on
the smallest case; operator complexity <= 1.5; iteration counts and solution are bit-identical for 1 and 8 threads. The scalar AMG
and its tests are unchanged; the default solver's results are unchanged bit for bit.

**Checked against.** The reference is the direct solution and the iteration counts themselves. Method, from memory: Vanek, Mandel
and Brezina, Computing 56 (1996) 179 (smoothed aggregation with a near-null space); the rigid-body modes of a node set are
translations and x cross e_i (first principles).

**Files.** `crates/rcfd-linalg/src/amg_elastic.rs` (new), `crates/rcfd-linalg/src/lib.rs` (one `pub mod` line),
`crates/rcfd-fem/src/solve.rs` (the solver choice and the modes from the coordinates),
`crates/rcfd-linalg/tests/si_amg_elastic.rs`, `crates/rcfd-fem/tests/si_amg.rs`.

**Size.** M. **What could make it miss.** Every iteration bound is a draft; none is derived. Constrained unknowns break the block
structure (the reduced matrix has nodes with fewer than three unknowns). Aggregates with too few nodes cannot carry six modes (the
local QR is rank deficient and must drop columns by a registered rule). Six unknowns per node (shells) and the modal solver (which
factorises) do not profit: an iterative eigensolver is left to the advanced tier.

### ADR 0306 (SI-16): Solid case file: the analyses and model data of the intermediate tier

**Goal.** Make the intermediate tier usable without writing Rust: the solid case file of ADR 0287 gains the element properties
(formulation, section, thickness), linear constraints, rigid sets, cyclic pairs and supports, and the analysis kinds pre-stressed
modal, rotating modal with a speed list (Campbell table), buckling, cyclic modal with a list of harmonic indices, transient and
harmonic; the JSON report and the files written gain their results. Every key is optional: a file of the basic tier runs as
before. No flow case key, no CLI subcommand, no MCP tool.

**Dependencies.** ADR 0296, 0297, 0298, 0299, 0301, 0302; roadmap items: SI-6, SI-7, SI-8, SI-9, SI-11, SI-12, F-386.

**Acceptance (draft).** Each new analysis run from a file gives the bits of the same model run by library calls (limit 1e-12,
expected identical), and the closed-form checks of the library pass from files: the Euler column (ADR 0298), the rotating radial
bar and a Campbell table with its crossings (ADR 0299), the ring at harmonic index 2 (ADR 0297), the resonance peak (ADR 0302),
the free vibration of the bar (ADR 0301), the skew roller (ADR 0296). An unknown key, an analysis that lacks a required table, a
harmonic index above N/2, a speed list that is not increasing and a property that names a missing set are each refused with the
key and the line. Every solid case file of the basic tier's tests gives the same report and the same .vtu bytes as before. No flow
case key, CLI help text, MCP tool list, hash table or golden changes (the existing schema, CLI and tool-list tests pass unedited).

**Checked against.** The closed forms of the library packages, re-run through the file; the bit comparison with library calls. ADR
0287 is the model for the file and for the check that nothing existing changes.

**Files.** `crates/rcfd-api/src/solid_case.rs`, `crates/rcfd-api/tests/si_solid_case.rs`, `docs/structural-solver.md` (the file
format section).

**Size.** M. **What could make it miss.** Six dependencies: if one of them ends partial its analysis is left out of the file and
this package is partial with it. Nonlinear statics (ADR 0300), beams and shells are parsed as data but checked only if their
packages are merged when this one starts. Time histories of many steps need an output cadence and a size limit that the basic file
did not need.

### ADR 0307 (SI-17): A [solid] table in the flow case and the library entry that runs the coupled case

**Goal.** A flow case can name a solid: an optional table `[solid]` (the solid case file, the fluid patch and the solid surface
set of the interface, the mapping method, the coupling tolerance, iteration cap, update and initial relaxation), added by the
route of ADR 0257 so that no hash table or golden changes, and a library function of `rcfd-api` that builds both participants and
runs `coupled_step` over the time steps. The ordinary run path refuses a case that has the table instead of ignoring it. Library
only: the CLI and MCP exposure is ADR 0308.

**Dependencies.** ADR 0304, 0306; roadmap items: SI-14, SI-16, F-386.

**Acceptance (draft).** Gate (as check K0 of ADR 0257): with the optional table in the schema the tests case_hashes_w48u,
baseline_hashes, w48v_adaptive_correctors, rules_w19c, mesh_transform_w48u and physics_schema of rcfd-api, cli of rcfd-cli, and
default_f1, project_list_f270 and tool_list_f275 of rcfd-mcp pass with no file edited. The piston of ADR 0304 written as a flow
case with a [solid] table and a solid case file and run by the library entry gives the interface displacement history of the
library set-up within 1e-12 (expected identical) and so its frequency; the report carries the iterations and residuals of every
step. A case with the table run through the ordinary path is refused with the key; a missing patch, a missing surface set, a solid
file without a transient analysis and a time step that differs between the two files are each refused with the key and the line. A
flow case without the table gives the bytes it gave before.

**Checked against.** ADR 0257 (reading 3 and check K0: an optional table needs no golden edit) and ADR 0155; the closed form is
that of ADR 0304, reached through the files.

**Files.** `crates/rcfd-api/src/schema.rs` (the optional table), `crates/rcfd-api/src/solid_coupled.rs` (new),
`crates/rcfd-api/src/lib.rs` (one `pub mod` line), the one refusal in the ordinary case-building path (a shared file: baseline
hashes are run), `crates/rcfd-api/tests/si_solid_coupled.rs`, `docs/cosimulation-protocol.md`, `docs/structural-solver.md`.

**Size.** M. **What could make it miss.** I did not read how a moving wall and its mesh smoothing are described in a flow case
(`dynmesh.rs`): if `FlowWall` cannot be built from existing keys, the table needs more keys than listed or the package is partial.
The refusal in the ordinary path touches a shared file (decision N3). If the gate fails the table is removed and the library entry
takes the two files as arguments.

### ADR 0308 (SI-18): Command-line subcommand and MCP tools for the solid solver (needs the maintainer: two golden files change)

**Goal.** Expose the solid case file to users and to the assistant: a subcommand `rcfd solid <file>` (run, with `--check` to
validate only) and two MCP tools (validate a solid case, run it and return the JSON report). This is option (b) of decision M3 of
ADR 0270. It CANNOT be done under house rule 3 without the maintainer: a subcommand changes the CLI help golden and a tool changes
the MCP tool-list golden. It is kept apart from every library package so that none of them waits for it.

**Dependencies.** ADR 0306; roadmap items: SI-16, F-386.

**Acceptance (draft).** NEEDS THE MAINTAINER'S APPROVAL of two golden edits before it starts. rcfd solid <file> runs a solid case
file and writes the files and the report of the library entry with the same bytes; --check validates without solving; exit codes
and located errors follow the existing subcommands (an unknown key exits non-zero with the key and the line). The MCP tools
solid_check and solid_run return the validation result and the JSON report of the same run. The CLI help golden and the MCP
tool-list golden change by exactly the added subcommand and the two added tools (the diff is listed in the addendum for the
maintainer); every other golden, the hash tables and the flow-case behaviour are unchanged; the coupled run of ADR 0307 is
reachable only through the library until a further decision.

**Checked against.** The existing subcommands and tools are the model (their tests in `crates/rcfd-cli/tests/cli.rs` and
`crates/rcfd-mcp/tests`); the numbers are those of the library, compared by bytes.

**Files.** `crates/rcfd-cli/src` (the subcommand), `crates/rcfd-mcp/src` (the tools), their tests `si_solid_cli.rs` and
`si_solid_tools.rs`, and, by the maintainer's approval only, the CLI help golden and `crates/rcfd-mcp/tests/golden` (tool list).

**Size.** S. **What could make it miss.** No approval: the package does not start. Whether the assistant layer (`rcfd-assistant`)
keeps its own list of tools that must change with the MCP list was not read.

### ADR 0309 (SI-19): Demonstration on the fan rotor: Campbell diagram of the blade, the bladed disc by cyclic symmetry, response to blade-passing forcing

**Goal.** Bring the tier to the target of fan breakdown item E5 for a rotating 120 mm fan: the blade of ADR 0288 at speed
(Campbell table of its lowest modes with the engine orders of the blade and strut counts), one sector of the bladed hub by cyclic
symmetry, and the harmonic response of the blade to a pressure harmonic at the blade-passing frequency with an assumed damping.
There is no measured blade, so the checks are consistency, bounds and mesh change; the result is a demonstration, not a
validation. Extends the user page.

**Dependencies.** ADR 0297, 0299, 0302, 0306; roadmap items: SI-7, SI-9, SI-12, SI-16, F-387.

**Acceptance (draft).** Blade of the parametric fan geometry in quadratic tetrahedra, clamped at the hub, at registered speeds
from rest to the registered maximum: the Campbell table of the four lowest modes is written from a solid case file, with every
crossing of the engine orders 1 to the blade count listed; at rest the frequencies have the bits of F-387; for each mode the slope
of omega^2 against Omega^2 at the lowest speed equals the Rayleigh quotient phi^T (K_sigma,1 - M_perp) phi of the mode at rest
within 1 percent and lies between -1 and the registered flap bound; between two meshes (the second with at least twice the
elements) the first frequency at the maximum speed changes by less than 5 percent, both recorded. Bladed hub with Z blades: the
sector frequencies for harmonic indices 0 to Z/2 equal those of the full rotor on the coarse mesh within 1e-6 (heavy record).
Blade-passing response: at a registered Campbell crossing the tip amplitude under a unit pressure harmonic equals the modal
estimate (participation over 2 zeta omega^2) within 1 percent and falls to the static value within 1 percent at one tenth of the
frequency. The page docs/structural-solver.md states the scope, that the Coriolis term is neglected, that the damping is assumed
and that nothing is validated against a measured blade.

**Checked against.** Consistency only: the first-order perturbation of an eigenvalue, the full rotor as the reference of the
sector, the single-mode resonance estimate. No reference value exists for the blade (docs/fan-project-breakdown.md, part G).

**Files.** `crates/rcfd-api/tests/si_fan_rotor.rs`, `docs/structural-solver.md`, the E5 status text of
`docs/fan-project-breakdown.md`.

**Size.** M. **What could make it miss.** It inherits the risk of ADR 0288 (whether the tetrahedral mesher gives a usable blade);
if the blade cannot be meshed the flat twisted plate is used and the item is partial. The hub-and-blade sector needs matching
cyclic boundaries from a mesher that does not promise them. A coupled (flutter) run on the rotating fan passage is NOT part of
this tier (ADR 0290 section 1.3).

## 4. Critical path and parallel work

Dependencies inside the tier (an arrow means "must be merged before"):

```text
0291 -> 0292
0291 -> 0293
0291 -> 0294 -> 0295
0291 -> 0296 -> 0297 ---------------------> 0306 -> 0309
0291 -> 0298 -> 0299 ---------------------> 0306 -> 0309
        0298 -> 0300
0291 -> 0301 -> 0303 -> 0304 -> 0307 (with 0306)
0291 -> 0302 -----------------------------> 0306 -> 0308 (maintainer)
0305 (needs only the basic tier)
```

From the basic tier: 0291 needs F-370, F-372, F-374 and F-377 merged; wave 2 needs F-375, F-376, F-382 and F-383 as well; 0303
needs F-385 and F-362; 0306 needs F-386; 0309 needs F-387. In practice the intermediate tier starts when the basic tier's
waves 1 to 5 are merged.

**Critical paths** (five packages each):

* the fan: 0291 -> 0298 -> 0299 -> 0306 -> 0309 (and beside it 0291 -> 0296 -> 0297 -> 0306). The first frequency of a
  rotating blade exists after 0299.
* the coupling: 0291 -> 0301 -> 0303 -> 0304 -> 0307. The first run with the flow solver is 0304.

**Waves.** Within a wave no two packages edit the same file; each wave starts when the packages it depends on are merged.

| Wave | Packages in parallel | Files, by package |
|---|---|---|
| 1 | 0291, 0305 | 0291: `rcfd-fem` `lib.rs`, `mesh.rs`, `model.rs`, `element.rs`, `assemble.rs`, stubs; `rcfd-post` `lib.rs`, `solid_output.rs`. 0305: `rcfd-linalg` `amg_elastic.rs`, `lib.rs`; `rcfd-fem` `solve.rs` |
| 2 | 0292, 0293, 0296, 0298, 0301, 0302 | `hex20.rs`; `hex8x.rs`; `mpc.rs`, `constraint.rs`; `geometric.rs`, `buckling.rs`, `rcfd-linalg` `eigen.rs`; `transient.rs`; `harmonic.rs` |
| 3 | 0294, 0297, 0299, 0300, 0303 | `beam.rs` with `model.rs`, `assemble.rs`, `constraint.rs`, `loads.rs`, `mass.rs`, `recover.rs`, `rcfd-post` `solid_output.rs`; `cyclic.rs`; `rotating.rs`; `nonlinear.rs`; `rcfd-post` `solid_dynamic.rs` |
| 4 | 0295, 0304, 0306 | `shell.rs`; `rcfd-post` `solid_fsi.rs`, `docs/cosimulation-protocol.md`; `rcfd-api` `solid_case.rs`, `docs/structural-solver.md` |
| 5 | 0307, 0309 (and 0308 once approved) | `rcfd-api` `schema.rs`, `solid_coupled.rs`, `lib.rs`; test file and documentation; `rcfd-cli`, `rcfd-mcp` |

Notes on the table:

* 0294 edits the basic files that 0296 edits (`constraint.rs`): they are in different waves for that reason, and 0296 comes
  first because the fan path runs through it. 0294 must keep the transformation path of 0296 working for six unknowns per node.
* Wave 2 is six builds at once on a shared machine. If that is too many, 0296 and 0298 go first (the fan path), then 0301,
  then the rest. The heavy record of 0305 should not run beside wave 2.
* 0307 and 0309 both write to `docs/structural-solver.md` in wave 5: 0307 adds one section on the coupled case, 0309 the
  rotor demonstration; the lead merges 0309 second. If that is not acceptable, 0307's text goes to the protocol page only.
* Each package adds its own test file (`si_*.rs`); none edits another's or a basic one (`sb_*.rs`).
* Tests-only findings: if a package finds a defect in a basic-tier file it does not own, it records the miss and reports it;
  the fix is a separate change by the lead's choice.

## 5. For the maintainer: decisions

Each with options and a recommendation. None is taken here.

* **N1. J2 plasticity.** The brief lists it for this tier; this plan moves it to the advanced tier (section 1.2, point 1).
  Options: (a) as proposed; (b) keep it in this tier as one M package after ADR 0300 (uniaxial bilinear response, the
  elastic-plastic thick sphere with its closed form, the plastic moment of a beam), which needs a twentieth number or replaces
  the shell (0295) or the multigrid (0305). Recommendation: (a).
* **N2. The CLI subcommand and the MCP tools (ADR 0308).** They change the CLI help golden and the MCP tool-list golden.
  Options: (a) approve both edits now, so 0308 can run as soon as 0306 is merged; (b) approve them when the tier is otherwise
  done; (c) not in this tier. Recommendation: (a): the assistant cannot use the solver without a tool, and the fan milestone is
  assistant-driven. This is also the open decision M3(b) of ADR 0270.
* **N3. The `[solid]` table and the refusal.** Options: (a) an optional table in the flow case (no golden, by ADR 0257) plus one
  refusal in the ordinary run path, a shared file, checked by the baseline hashes; (b) the table without the refusal (a case
  with the table then runs uncoupled without a word: not recommended); (c) no table: a third, coupled case file that names a
  flow case and a solid case. Recommendation: (a); (c) if any edit of the shared path is unwelcome.
* **N4. No coupling crate.** Options: (a) the participant and the helper stay in `rcfd-post`; (b) a new crate `rcfd-coupling`
  (another line in `workspace_graph.rs`). Recommendation: (a).
* **N5. Rotating modes without the Coriolis term.** Options: (a) as proposed: frequencies in the rotating frame from `K +
  Omega^2 (K_sigma,1 - M_perp)`, the omission stated on the user page; (b) hold the Campbell diagram until the advanced tier
  has the gyroscopic matrix and a complex eigensolver. Recommendation: (a): for a blade on a stiff hub the term is small, and
  (b) would leave the fan without any rotating frequency.
* **N6. Which shell.** Options: (a) a four-node quadrilateral only (ADR 0295), triangle and shell stress stiffening in the
  advanced tier; (b) a triangle instead, because the surface meshers of the repository produce triangles; (c) both, the second
  replacing another package. Recommendation: (a), and the registration of 0295 should say which generator gives it
  quadrilaterals of a blade mid-surface; if none does, (b).
* **N7. Reference values.** House rule 4 allows the repository and closed forms. Public benchmark values (the shell problems of
  MacNeal and Harder, the NAFEMS targets, the Turek-Hron flag) are not in `docs/reference-data.md`. Options: (a) closed forms
  and equivalences only, as planned; (b) the maintainer adds named public values to `docs/reference-data.md`, and the element
  packages gain them as extra records. Recommendation: (a) now, (b) welcome at any time.
* **N8. The scaffold edits basic-tier files once (ADR 0291)** and asks ADR 0271 to build models through constructors (section
  2.4, point 1). Options: (a) pass the request to the basic tier's implementers now; (b) accept the side structure.
  Recommendation: (a).
* **N9. Roadmap placement.** The existing category "Structural solver" (family "Solver"), phase 9, as the basic tier. A
  separate row would only be worth it if the progress graphic should show the tiers apart; I do not argue for it.

## 6. What I am unsure of

* **The basic tier does not exist yet.** File names, the element dispatch, how models are constructed and whether `DofMap`
  keeps its promise are read from a plan. The file lists of 0291, 0294 and 0296 are the most exposed.
* **Numbers.** Every limit in section 3 is a draft from standard theory or an estimate; nothing was run. The ones I trust
  least: every iteration bound of the multigrid (0305); the 2 percent of the coupled piston (0304), where the flow side's
  implicit Euler damping may dominate; the 3 percent of the incompatible-mode cantilever at nu = 0.3 and both locking controls
  (0293); the pinched ring with flat facets (0295); the 2 percent of the elastica (0300); the 1e-4 of the flap-lag identity
  (0299).
* **Recalled constants**, each marked for recomputation in its stub: the Navier coefficient 0.004062, the pinched-ring factor
  pi/4 - 2/pi (0.1488), the elastica deflections, the Rayleigh quotient 1.19 of the rotating cantilever, the effective mass
  fraction 0.613, the shear correction of the Euler load (0.64 percent at slenderness 20, computed here by hand). All citations
  are from memory.
* **Whether neglecting the Coriolis term is acceptable for the bladed hub** (as opposed to the single blade): for disc-like
  modes with in-plane motion it is not small. The cyclic package itself is at rest and unaffected; the demonstration combines
  the two and can only state the omission.
* **Shell on a blade.** Whether any generator of the repository gives a quadrilateral mid-surface mesh of the blade was not
  read. Without one, the shell serves plates and housings only and the blade stays on solid elements, which this tier's fan
  path assumes anyway.
* **The flow case of a moving wall from a file** (0307): `dynmesh.rs` was not read. If a `FlowWall` cannot be built from
  existing keys the package grows or ends partial.
* **F-362** is registered with one amendment and not yet run. 0303 and 0304 say what they do if it misses.
* **Sizes.** 0295 (shell) and 0294 (six unknowns and beam) are at the upper end of M; 0304 has the longest run times. Splits:
  0295 into statics and then mass with warped facets; 0294 into the unknown numbering and then the beam; 0298 into pre-stressed
  modes and then buckling.
* **Damping.** No damping value for a moulded blade is in the repository; harmonic amplitudes in 0309 scale with an assumed
  ratio.

## 7. For the lead: roadmap entries

Category for the roadmap graphic: the existing **"Structural solver"**, family **"Solver"** (the row of `CATEGORIES` in
`scripts/roadmap_progress.py` that holds 370 to 387; the new ids are appended to it). Placeholder ids SI-1 to SI-19 for ADR 0291
to 0309, to be renumbered; status todo for all; phase 9 as the basic tier (N9). Dependencies are given as SI ids or F ids. The
existing items they depend on: F-026 and F-139 (done or done pending acceptance), F-251, F-351, F-361 (done pending
acceptance), F-362 (in progress), and the basic tier F-370 to F-387 (todo). SI-18 is blocked on decision N2 and nothing depends
on it. Suggested edit of an existing note, for the lead to word: fan breakdown E5 may name this plan as the route to the
rotating-blade and coupled parts once SI-9 is done.

Each acceptance and each note below is one quoted paragraph on a single line.

### Entries

**SI-1** (ADR 0291), status todo, deps: F-370, F-372, F-374, F-377

- title: Scaffold of the intermediate tier: module layout, model extension data, element-trait extension points
- acceptance:

> Every limit not stated here is registered in the ADR before the run. The modules, element kinds, property data, constraint and support data and the three defaulted element entry points of ADR 0290 section 2 exist in rcfd-fem and rcfd-post; every test of the basic tier (the sb_ files of rcfd-fem, rcfd-linalg, rcfd-io, rcfd-post and rcfd-api) compiles and passes unedited, and a registered set of basic models (patch test, cantilever, bar modes) gives displacements, stresses and frequencies with the same bits as before the change; the generic assembly hook called with the element stiffness reproduces assemble::stiffness bit for bit for 1 and 8 threads; each new element kind, formulation, constraint and support variant is refused by the static solve with an error naming the variant and the set, never ignored; the baseline hashes are unchanged.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size S. Stub ADR 0291; the registration is written by the implementation agent before any code.

**SI-2** (ADR 0292), status todo, deps: SI-1, F-372, F-374, F-375, F-376, F-383

- title: Quadratic hexahedron (Hex20) with its face loads, mass and the quadratic upgrade of a hexahedron box
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Hex20 on the reference element and on randomly distorted ones (registered seed): shape functions sum to 1 and derivatives to 0 within 1e-14, analytic derivatives agree with central differences within 1e-7, the 3 x 3 x 3 rule integrates every monomial of degree <= 5 per direction within 1e-13, the stiffness is symmetric within 1e-13, has rank 54 and gives no force for the six rigid motions within 1e-11. Patch test with all six strains on a box with displaced interior nodes: 1e-10. A uniform pressure on an 8-node face gives the nodal loads -1/12 (corners) and 1/3 (mid-edge) of p A within 1e-13, and uniaxial tension the closed form within 1e-10. Cantilever L/h = 20, nu = 0.3, ONE element through the thickness and 10 along: tip deflection within 1 percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A L^2)). Thick cylinder b/a = 2 in plane strain on a curved mesh: bore displacement within 0.2 percent and bore hoop stress within 1 percent of Lame. Manufactured solution of F-378: observed L2 order >= 2.7. Mass: total mass and inertia tensor of a box within 1e-12; first axial frequency of a fixed-free bar (nu = 0) within 0.1 percent at 8 elements. The upgrade has (nodes + edges) nodes.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size S. Stub ADR 0292; the registration is written by the implementation agent before any code.

**SI-3** (ADR 0293), status todo, deps: SI-1, F-372, F-374, F-376, F-379

- title: Hex8 without locking: mean-dilatation (B-bar) and incompatible-mode formulations, opt-in per element block
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Two opt-in formulations of Hex8, default unchanged bit for bit. Both pass the patch test of F-374 on distorted meshes within 1e-10 and a single element of either has exactly six zero eigenvalues (the seventh above 1e-6 of the largest) for aspect ratios 1, 10 and 100. Cantilever of F-379 (L/h = 20, tip shear force) with ONE element through the thickness and 20 along: incompatible modes within 1 percent of the closed form at nu = 0 and within 3 percent at nu = 0.3, while full integration on the same mesh is the registered locking control (below 80 percent); at L/h = 100 with elements of aspect ratio 5 incompatible modes stay within 2 percent. Thick cylinder b/a = 2 in plane strain with nu = 0.4999: B-bar gives the bore displacement within 1 percent of Lame on the mesh on which full integration is below 90 percent (control). Recovered stress on the patch test equals D eps within 1e-10; the condensed internal modes are recovered for the stress.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0293; the registration is written by the implementation agent before any code.

**SI-4** (ADR 0294), status todo, deps: SI-1, F-374, F-375, F-376, F-377, F-383

- title: Six unknowns per node: rotations in the DofMap, nodal moments, and a two-node shear-flexible beam element
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Nodes carry three or six unknowns; a model without structural elements gives the bits it gave before. Beam, ONE element, any orientation in space (registered seed): tip deflection P L^3/(3 E I) + P L/(kappa G A), tip rotation P L^2/(2 E I), twist T L/(G J) and elongation F L/(E A) within 1e-10, section forces at the root equal to the applied loads within 1e-10, reactions (forces and moments) balancing the loads within 1e-10; a rotation of the whole model leaves the results unchanged within 1e-10. Quarter-circle arch of radius R clamped at one end, 32 elements, slender: deflection under the tip load pi P R^3/(4 E I) within 0.5 percent. Free beam: six zero frequencies below 1e-6 of the seventh. Pinned-pinned beam, 20 elements, slenderness 50: f_n = (n pi/L)^2 sqrt(E I/(rho A))/(2 pi), n = 1, 2, 3, within 0.5, 1 and 2 percent after the registered shear and rotary-inertia correction. A beam node shared with a solid element is refused (no rotational stiffness from the solid) with the node id.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0294; the registration is written by the implementation agent before any code.

**SI-5** (ADR 0295), status todo, deps: SI-4

- title: Four-node shell element (flat facet, assumed shear strain): plates, housings and thin blades as surfaces
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Membrane patch test and constant-curvature bending patch test on a distorted flat mesh within 1e-8. Simply supported square plate under uniform load, 16 x 16 elements, nu = 0.3: centre deflection within 1 percent of the Navier series (0.004062 q a^4/D, recomputed in the registration) at thickness/side 0.01 and within 1.5 percent at 0.001 (no shear locking), the fully integrated shear control being below 20 percent at 0.001. Cantilever strip, nu = 0: tip deflection P L^3/(3 E I) within 0.5 percent and surface stress 6 M/t^2 at mid-span within 2 percent. Curved: a ring strip of radius R pinched by two opposite forces, nu = 0, 32 elements per quarter: change of the loaded diameter (pi/4 - 2/pi) P R^3/(E I) within 2 percent; a cylinder under internal pressure with free ends: radial displacement p R^2/(E t) within 0.5 percent. Frequencies of the simply supported plate: omega_mn = pi^2 (m^2 + n^2)/a^2 sqrt(D/(rho t)) within 1 percent for (1,1) and 3 percent for (1,2) and (2,2). A free plate has six zero frequencies; a facet warped beyond the registered limit is refused with the element.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0295; the registration is written by the implementation agent before any code.

**SI-6** (ADR 0296), status todo, deps: SI-1, F-374, F-375

- title: Linear multi-point constraints by a transformation, rigid sets with a reference point, elastic supports
- acceptance:

> Every limit not stated here is registered in the ADR before the run. A model with prescribed displacements only gives the bits of the basic tier; the same constraints written as linear constraints give the same displacements within 1e-12. The reduced matrix is symmetric within 1e-13 and the constraint forces do no work on any admissible motion within 1e-12. Skew roller: a block in uniaxial stress along a direction 30 degrees off the axes, held by the constraint u.n = 0 on its inclined faces, has the rotated uniaxial solution within 1e-10. Periodic pair u(high) - u(low) = eps L on a box with displaced interior nodes: uniform strain within 1e-10. Rigid set on the end face of a bar: an axial force at the reference point gives F L/(E A) within 1e-10 (nu = 0), a moment the tip rotation M L/(E I) within 1 percent (Tet10, slenderness 20), and the constraint forces have the resultant and moment applied within 1e-10. Supports: a bar on an end spring k has the tip displacement F (1/k + L/(E A)) within 1e-10; a block on a foundation k_f under pressure p settles by p/k_f within 1e-10. A redundant constraint is dropped with a report, a contradictory one and a chain that closes on itself are refused naming the constraints.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0296; the registration is written by the implementation agent before any code.

**SI-7** (ADR 0297), status todo, deps: SI-6, F-375, F-383

- title: Cyclic symmetry: one sector of a rotor for statics and for modal analysis by harmonic index
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Equivalence: for a ring and for a disc with N = 12 sectors, the frequencies of the sector for every harmonic index 0 to 6 equal the frequencies of the full 360 degree model built from the same sector mesh within 1e-8 relative, with the same count below a registered cut-off (single for k = 0 and 6, double otherwise); the expanded mode shapes satisfy the full model's eigen-equation within 1e-7. Closed forms: in-plane bending of a thin ring (thickness/radius 0.05, nu = 0), omega_n^2 = E I n^2 (n^2 - 1)^2/(rho A R^4 (n^2 + 1)) for n = 2 and 3 found at harmonic index 2 and 3 within 2 percent; a thin annular disc b/a = 4 spinning about its axis (statics, k = 0): rim displacement rho omega^2 b ((3 + nu) a^2 + (1 - nu) b^2)/(4 E) within 1 percent. Boundaries that do not match under the rotation within the registered tolerance are refused with the first unmatched node; N < 2 and a harmonic index above N/2 are refused.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0297; the registration is written by the implementation agent before any code.

**SI-8** (ADR 0298), status todo, deps: SI-1, F-374, F-376, F-382, F-383

- title: Geometric (initial-stress) stiffness of solid elements: pre-stressed modal analysis and linear buckling
- acceptance:

> Every limit not stated here is registered in the ADR before the run. K_sigma is symmetric within 1e-13 of its norm, gives no force for a rigid translation within 1e-12, and for a uniform uniaxial stress s and a linear displacement field w gives x^T K_sigma x = s V |grad w . e|^2 within 1e-12. rcfd-linalg: for K = tridiagonal (2, -1), n = 200, and K_sigma = -I the ten lowest buckling factors equal 4 sin^2(j pi/(2 (n + 1))) within 1e-10; a K_sigma with no negative direction returns no factor and says so. Pre-stressed modes: a beam on knife-edge supports at its neutral axis (slenderness 20, nu = 0) under an axial tension stress s has (omega_n^2 - omega_n,0^2) = (n pi/L)^2 s/rho within 1 percent for n = 1 and 2, and under compression the frequency falls by the same amount. Buckling: the same beam in compression has the critical load pi^2 E I/L^2 reduced by the shear term (0.6 percent at this slenderness, registered) within 1 percent, the second factor four times the first within 2 percent; a clamped-free column pi^2 E I/(4 L^2) within 1 percent (Tet10 or Hex20).

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0298; the registration is written by the implementation agent before any code.

**SI-9** (ADR 0299), status todo, deps: SI-8, F-375, F-383

- title: Modes of a rotating part: stress stiffening by the centrifugal pre-stress, spin softening, Campbell diagram
- acceptance:

> Every limit not stated here is registered in the ADR before the run. At zero speed the result has the bits of the modal analysis of F-383. Radial bar clamped on the axis, nu = 0, axial modes: (omega_n,0^2 - omega_n^2)/Omega^2 = 1 within 1e-3 for n = 1, 2 at a speed with centrifugal stress below 1e-4 E. Radial beam of square section clamped on the axis (slenderness 20): the first out-of-plane (flap) and in-plane (lag) frequencies satisfy omega_flap^2 - omega_lag^2 = Omega^2 within 1e-4 Omega^2 on a symmetric mesh; the stiffening (omega_flap^2 - omega_0^2)/Omega^2 equals at Omega = 0.2 omega_0 the Rayleigh quotient of the centrifugal tension with the mode at rest (1.19, recomputed in the registration) within 2 percent and stays between 1 and that value up to Omega = 5 omega_0. Campbell table: on a synthetic family omega^2 = omega_0^2 + S Omega^2 the crossings with orders 1 to 4 equal omega_0/sqrt(k^2 - S) within 1e-8, and on the beam within 1 percent; a flap and a lag mode of a rectangular section that cross are tracked without exchange (modal assurance above 0.9). A speed at which K_eff is not positive definite is reported as a loss of stiffness with the speed, never as a non-finite frequency.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0299; the registration is written by the implementation agent before any code.

**SI-10** (ADR 0300), status todo, deps: SI-8, F-374, F-375

- title: Geometric nonlinearity: total Lagrangian solid elements, Newton iteration with load steps
- acceptance:

> Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences of the internal force within 1e-6 relative on distorted elements at displacements of 20 percent of the element size (registered seed). A rigid rotation by 90 degrees with a translation gives an internal force below 1e-10 E L^2. Uniform stretch lambda = 1.2 of a bar with nu = 0: nominal stress E lambda (lambda^2 - 1)/2 within 1e-9; under load control the last three residuals of each step show an order >= 1.8. A load factor of 1e-6 reproduces the linear solution within 1e-5 relative. Elastica: a cantilever of slenderness 50, nu = 0, with tip force P L^2/(E I) = 1 and 2 has the tip deflection and shortening within 2 percent of the elliptic-integral solution (recomputed in the registration by integrating the elastica equation; recalled 0.302 L, 0.056 L and 0.494 L, 0.161 L). An element inverted during the iteration and a step that does not converge after the registered halvings are errors with the element or the step.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0300; the registration is written by the implementation agent before any code.

**SI-11** (ADR 0301), status todo, deps: SI-1, F-373, F-374, F-383

- title: Linear transient dynamics: Newmark and generalised-alpha integration, Rayleigh damping, load histories
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Free vibration started in the first axial mode of a bar (the discrete eigenvector): with Newmark the amplitude after 100 periods is within 1e-9 and the total energy constant within 1e-10; the measured period is long by (omega dt)^2/12 within 5 percent of that amount at omega dt = 0.2 and 0.1, observed order >= 1.9. A step load proportional to M phi_1 gives the peak displacement twice the static one within 0.5 percent at 40 steps per period. Rayleigh damping with a registered ratio 0.02 on that mode: logarithmic decrement 2 pi zeta/sqrt(1 - zeta^2) within 1 percent. Generalised-alpha: with spectral radius 1 the result equals Newmark within 1e-12; with 0.5 and 0.8 a mode at omega dt = 1e3 decays per step by that factor within 1e-3 while the period error of the first mode keeps order >= 1.9. A run split in two by save and restore has the bits of the unsplit run; results are bit-identical for 1 and 8 threads; a non-positive step, a table that does not cover the run and a singular effective matrix are refused.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0301; the registration is written by the implementation agent before any code.

**SI-12** (ADR 0302), status todo, deps: SI-1, F-374, F-383

- title: Harmonic response by mode superposition: modal damping, static correction, effective mass, force harmonics
- acceptance:

> Every limit not stated here is registered in the ADR before the run. A load proportional to M phi_1 at omega_1 gives the amplitude of the static response divided by 2 zeta within 1e-8 and a phase lag of 90 degrees within 1e-6 rad; the half-power width of that peak is 2 zeta omega_1 within 1 percent (zeta = 0.01). Fixed-free bar with nu = 0 under a harmonic tip force, undamped, at 0.5 and 1.5 times the first frequency: tip amplitude F tan(k L)/(E A k), k = omega/c, within 0.5 percent with 10 modes and the static correction, the same without the correction being the registered control (error above 1 percent at 0.5). Effective mass: the first axial mode of the bar carries 8/pi^2 of the mass within 0.5 percent, the first bending mode of a cantilever 0.613 (recomputed in the registration) within 1 percent, and the sum over all modes of a small model equals the unconstrained mass within 1e-10. Force harmonics of a sampled signal with three harmonics: amplitudes and phases within 1e-12; a record that is not a whole number of periods is refused.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0302; the registration is written by the implementation agent before any code.

**SI-13** (ADR 0303), status todo, deps: SI-11, F-139, F-361, F-362, F-385

- title: Dynamic solid as a co-simulation participant, on interface weights built from the two surfaces
- acceptance:

> Every limit not stated here is registered in the ADR before the run. A bar (nu = 0) whose end face is the interface, coupled by coupled_step to a mock fluid that returns the force -m_a times the wall acceleration: the first coupled frequency equals the root of k L tan(k L) = rho A L/m_a within 1 percent at 100 steps per period, for mass ratios m_a/(rho A L) of 0.5 and 4; at ratio 4 the plain iteration (factor 1) diverges as the protocol predicts (registered control) while Aitken's factor and the least-squares update converge to 1e-8 within registered iteration caps. With a mock spring k_f and a slow load the response tends to F_0/(k_s + k_f) of F-385 within 1e-6. Mapping built by F-362 for a matching and for a 3:2 non-matching interface: resultant force, moment and work conserved within 1e-12, and the coupled frequency changes by less than 1e-3 between the two. Restore returns the checkpoint bit for bit; a failing structural step stops the coupled step with the participant's name and both sides restored. No flow solver is coupled.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0303; the registration is written by the implementation agent before any code.

**SI-14** (ADR 0304), status todo, deps: SI-13, F-251, F-351, F-361, F-362

- title: Two-way coupling of an elastic solid with the incompressible flow solver: the piston on an elastic bar
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Elastic bar (nu = 0) closing a channel of incompressible fluid of length L_f, started from a displaced state, mass ratios rho_f L_f/(rho_s L_s) of 0.5 and 4: the coupled iteration converges in every step to 1e-6 with Aitken's factor and with the least-squares update, iteration counts recorded; the frequency of the first coupled mode equals the root of k L tan(k L) = rho_s L_s/(rho_f L_f) within 2 percent after the registered estimate of the time-step and viscous effects; over ten periods the work of the fluid on the interface equals the change of the solid's energy within 1e-4 of its amplitude; the loose coupling (one pass) at ratio 4 grows (registered control). With a 3:2 non-matching interface mapped by F-362 the frequency changes by less than 2e-3. A failure of either participant leaves both at the start of the step. The existing co-simulation tests (cosim_f139, cosim_f361, cosim_f362) pass unedited.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0304; the registration is written by the implementation agent before any code.

**SI-15** (ADR 0305), status todo, deps: F-026, F-381

- title: Aggregation multigrid for elasticity: rigid-body modes as near-null space, as a preconditioner of CG
- acceptance:

> Every limit not stated here is registered in the ADR before the run. On every level the rigid-body modes are in the range of the prolongator: |B - P B_c| <= 1e-12 |B|. CG preconditioned by one V-cycle reaches a relative residual of 1e-10 on a clamped cube of Hex8 with about 5e3, 4e4 and (heavy record) 3e5 unknowns in a number of iterations that grows by at most 1.5 times from the smallest to the largest and stays below 40; on a cantilever of slenderness 20 with about 1e5 unknowns it needs at most one fifth of the iterations of the incomplete Cholesky of F-381; the same on a Tet10 mesh within twice the Hex8 count. The solution agrees with the direct solver within 1e-7 on the smallest case; operator complexity <= 1.5; iteration counts and solution are bit-identical for 1 and 8 threads. The scalar AMG and its tests are unchanged; the default solver's results are unchanged bit for bit.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0305; the registration is written by the implementation agent before any code.

**SI-16** (ADR 0306), status todo, deps: SI-6, SI-7, SI-8, SI-9, SI-11, SI-12, F-386

- title: Solid case file: the analyses and model data of the intermediate tier
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Each new analysis run from a file gives the bits of the same model run by library calls (limit 1e-12, expected identical), and the closed-form checks of the library pass from files: the Euler column (ADR 0298), the rotating radial bar and a Campbell table with its crossings (ADR 0299), the ring at harmonic index 2 (ADR 0297), the resonance peak (ADR 0302), the free vibration of the bar (ADR 0301), the skew roller (ADR 0296). An unknown key, an analysis that lacks a required table, a harmonic index above N/2, a speed list that is not increasing and a property that names a missing set are each refused with the key and the line. Every solid case file of the basic tier's tests gives the same report and the same .vtu bytes as before. No flow case key, CLI help text, MCP tool list, hash table or golden changes (the existing schema, CLI and tool-list tests pass unedited).

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0306; the registration is written by the implementation agent before any code.

**SI-17** (ADR 0307), status todo, deps: SI-14, SI-16, F-386

- title: A [solid] table in the flow case and the library entry that runs the coupled case
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Gate (as check K0 of ADR 0257): with the optional table in the schema the tests case_hashes_w48u, baseline_hashes, w48v_adaptive_correctors, rules_w19c, mesh_transform_w48u and physics_schema of rcfd-api, cli of rcfd-cli, and default_f1, project_list_f270 and tool_list_f275 of rcfd-mcp pass with no file edited. The piston of ADR 0304 written as a flow case with a [solid] table and a solid case file and run by the library entry gives the interface displacement history of the library set-up within 1e-12 (expected identical) and so its frequency; the report carries the iterations and residuals of every step. A case with the table run through the ordinary path is refused with the key; a missing patch, a missing surface set, a solid file without a transient analysis and a time step that differs between the two files are each refused with the key and the line. A flow case without the table gives the bytes it gave before.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0307; the registration is written by the implementation agent before any code. The optional table follows the route of ADR 0257 (no golden edit); the one refusal in the ordinary run path is decision N3 of ADR 0290.

**SI-18** (ADR 0308), status todo, deps: SI-16, F-386

- title: Command-line subcommand and MCP tools for the solid solver (needs the maintainer: two golden files change)
- acceptance:

> Every limit not stated here is registered in the ADR before the run. NEEDS THE MAINTAINER'S APPROVAL of two golden edits before it starts. rcfd solid <file> runs a solid case file and writes the files and the report of the library entry with the same bytes; --check validates without solving; exit codes and located errors follow the existing subcommands (an unknown key exits non-zero with the key and the line). The MCP tools solid_check and solid_run return the validation result and the JSON report of the same run. The CLI help golden and the MCP tool-list golden change by exactly the added subcommand and the two added tools (the diff is listed in the addendum for the maintainer); every other golden, the hash tables and the flow-case behaviour are unchanged; the coupled run of ADR 0307 is reachable only through the library until a further decision.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size S. Stub ADR 0308; the registration is written by the implementation agent before any code. Blocked until the maintainer approves the edits of the CLI help golden and the MCP tool-list golden (decision N2 of ADR 0290); no library package depends on it.

**SI-19** (ADR 0309), status todo, deps: SI-7, SI-9, SI-12, SI-16, F-387

- title: Demonstration on the fan rotor: Campbell diagram of the blade, the bladed disc by cyclic symmetry, response to blade-passing forcing
- acceptance:

> Every limit not stated here is registered in the ADR before the run. Blade of the parametric fan geometry in quadratic tetrahedra, clamped at the hub, at registered speeds from rest to the registered maximum: the Campbell table of the four lowest modes is written from a solid case file, with every crossing of the engine orders 1 to the blade count listed; at rest the frequencies have the bits of F-387; for each mode the slope of omega^2 against Omega^2 at the lowest speed equals the Rayleigh quotient phi^T (K_sigma,1 - M_perp) phi of the mode at rest within 1 percent and lies between -1 and the registered flap bound; between two meshes (the second with at least twice the elements) the first frequency at the maximum speed changes by less than 5 percent, both recorded. Bladed hub with Z blades: the sector frequencies for harmonic indices 0 to Z/2 equal those of the full rotor on the coarse mesh within 1e-6 (heavy record). Blade-passing response: at a registered Campbell crossing the tip amplitude under a unit pressure harmonic equals the modal estimate (participation over 2 zeta omega^2) within 1 percent and falls to the static value within 1 percent at one tenth of the frequency. The page docs/structural-solver.md states the scope, that the Coriolis term is neglected, that the damping is assumed and that nothing is validated against a measured blade.

- note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. Stub ADR 0309; the registration is written by the implementation agent before any code.

## Lead's note (2026-11-12): roadmap ids

The entries were added to docs/roadmap.json with these ids: SI-1 = F-388, SI-2 = F-389, SI-3 = F-390, SI-4 = F-391, SI-5 = F-392, SI-6 = F-393, SI-7 = F-394, SI-8 = F-395, SI-9 = F-396, SI-10 = F-397, SI-11 = F-398, SI-12 = F-399, SI-13 = F-400, SI-14 = F-401, SI-15 = F-402, SI-16 = F-403, SI-17 = F-404, SI-18 = F-405, SI-19 = F-406, in the category "Structural solver".
