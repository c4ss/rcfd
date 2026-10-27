# rcfd parity matrix: Ansys Fluent / CFX class capabilities versus the rcfd roadmap

This page compares the capability lists of the two commercial CFD products named in the title with the rcfd roadmap, one row per
capability, and ranks the largest gaps. It is for people who plan roadmap work or want to check whether rcfd covers a given
capability. It is a first-pass inventory: the rcfd statuses are read from the roadmap, not guessed, and the sources section says
exactly what was read. For a short summary of what rcfd does today,
read the [roadmap overview](roadmap-overview.md); for the acceptance text of each item, see the [detailed roadmap](ROADMAP.md).

Trademarks and independence: all product and company names in this document are trademarks or registered trademarks of their respective owners and are used only to identify the products compared. rcfd is an independent open-source project, not affiliated with, endorsed by or sponsored by any of them.

Status: first-pass inventory, 2026-10-17. Roadmap statuses are read mechanically from `docs/roadmap.json` (158 items); the matrix does not guess them. Clean-room: every capability name and description below is a short paraphrase of the vendor's public product pages (S1, S2) and of contents pages of its online help (S3 to S10, which normally need a login); no vendor text or code was copied, no vendor binary or install was inspected or read. The [provenance record](provenance.md) says what was consulted and for what.

On this page:

- Sources actually read
- Coverage statement
- Roadmap statistics by area
- Ranked gap list
- Candidate new roadmap items
- Matrix, by area: geometry and meshing; turbulence modelling; heat transfer, CHT and radiation; species, combustion and chemistry; multiphase and discrete phase; compressible and high-speed; moving and dynamic mesh, rotation; FSI and co-simulation; turbomachinery; acoustics, electromagnetics and other physics; numerics and solvers; setup, UX and workflow; post-processing and reporting; validation and infrastructure
- Declined and not done

## Sources actually read (legend used in the "Source read" column)

All fetched through WebFetch, which returns a model-condensed extraction of the page, not the raw page. Chapter numbers below are from those extractions and may shift between releases.

| Code | Source | What was read |
|---|---|---|
| S1 | https://ansys.synopsys.com/products/fluids/ansys-fluent (redirect from ansys.com Fluent product page) | Product page feature list (read in full as extracted) |
| S2 | https://ansys.synopsys.com/products/fluids/ansys-cfx (redirect from ansys.com CFX page) | Product page feature list incl. 2026 R1 additions |
| S3 | vendor online help (login required; not linked) | Fluent Theory Guide 2025 R1 introduction: chapter-by-chapter overview |
| S4 | vendor online help (login required; not linked) | Fluent Theory Guide 2025 R1 contents, up to the multiphase chapter. Battery, fuel-cell and later sections were NOT read here |
| S5 | vendor online help (login required; not linked) | Fluent Theory Guide 2024 R2 ch.23 (solver theory): section headings only |
| S6 | vendor online help (login required; not linked) | CFX-Solver Modeling Guide 2025 R1 table of contents, chapters 1 to 18 and the start of 19 |
| S7 | vendor online help (login required; not linked) | Fluent User's Guide 2025 R1 contents (Parts I and II, most of Part III; later chapters not read) |
| S8 | vendor online help (login required; not linked) | Fluent GPU solver supported-features list, 2025 R2 |
| S9 | vendor online help (login required; not linked) | CFX-Solver Theory Guide 2026 R1 ch.4 (transient blade row) intro only. The CFX theory guide contents page itself was not read |
| S10 | vendor online help (login required; not linked) | CFX modeling guide 6.1.7 transient blade row guidelines |
| GK | General knowledge, unverified | No source read for this row |

Search-result snippets (not full pages) were seen for the Fluent GPU limitations pages and a vendor webinar summary; they are not cited as read sources.

Not read: CFD-Post/CFX-Pre/CFX-Post user guides, Fluent UDF/Customization manuals, Fluent Meshing standalone manual, Ansys Discovery/Icepak/Mechanical pages, any textbook or paper in this pass. Where the matrix touches those (post-processing details, UDF hooks) rows rely on Fluent User's Guide contents or GK.

## Coverage statement (honest)

- Rows in the matrix: 213. 194 rows cite at least one read source (S1 to S10), 16 rows are general knowledge only (GK), and 1 row (assistant workflows) is an rcfd-specific feature with no vendor counterpart. Two rows mix S and GK.
- What the sources establish: names and groupings of capabilities in the Fluent and CFX documentation (chapter and section titles) and the Fluent GPU solver's supported-feature list. They are tables of contents and feature lists, so they show that a capability exists, not how complete or accurate it is.
- What is general knowledge: the descriptions of what each model does, the priorities (P1/P2/P3) and the "reason" lines are my judgement, not source facts. The 16 GK-only rows are: Gmsh reader, OpenFOAM/CGNS readers, format registry, v2-f, turbulence plugin interface, mechanism input formats, viscoelastic models, Krylov solvers, diffusion correction, adaptive time stepping, shared-memory parallel, settings tree, solver service, VTK output, case comparison, CI. Roughly 90 percent of rows have a capability name backed by a read vendor contents list or feature list (names only); all descriptions, priorities and gap judgements are mine.
- Fluent areas not yet enumerated from sources: the second half of the theory guide (battery and fuel-cell theory), the UDF manual, Fluent Meshing manual details, Discovery and Icepak. CFX theory guide chapters other than ch.4 are represented only via the Modeling Guide contents.
- Priority labels are advisory and should be reviewed by the user before wave planning.
- rcfd status column: derived from roadmap.json item statuses. For rows with several ids the aggregate is: done if all done, todo if all todo, otherwise partial. "none" means no roadmap id was found, which was judged by reading all 158 titles; titles were truncated at 95 characters in my listing for ids below F-111, so a hidden sub-criterion could exist. Status is per roadmap item, not a measurement of how much of the vendor capability is covered (a "done" item can still be a subset of the vendor feature).

## Roadmap statistics by area

Counts are of roadmap items (not matrix rows), assigned to the area where they fit best by title.

| Area | Items | done | partial | todo |
|---|---|---|---|---|
| Meshing/geometry | 23 | 8 (34%) | 13 (56%) | 2 (8%) |
| Turbulence | 10 | 1 (10%) | 9 (90%) | 0 (0%) |
| Heat/CHT/radiation | 11 | 0 (0%) | 11 (100%) | 0 (0%) |
| Species/combustion/chemistry/properties | 12 | 2 (16%) | 10 (83%) | 0 (0%) |
| Multiphase/DPM | 7 | 0 (0%) | 5 (71%) | 2 (28%) |
| Compressible | 4 | 2 (50%) | 1 (25%) | 1 (25%) |
| Moving mesh/rotation/FSI/turbo | 10 | 0 (0%) | 4 (40%) | 6 (60%) |
| Numerics/solvers/parallel/GPU | 20 | 11 (55%) | 8 (40%) | 1 (5%) |
| Setup/UX/scripting/GUI/assistant | 40 | 19 (47%) | 17 (42%) | 4 (10%) |
| Post-processing/reporting/acoustics | 14 | 4 (28%) | 9 (64%) | 1 (7%) |
| Validation/infrastructure | 7 | 2 (28%) | 5 (71%) | 0 (0%) |

Whole roadmap: 158 items, 49 done (31%), 92 partial (58%), 17 todo (11%). Note "partial" is wide: several items were marked partial by the 2026-10-14 audit for reasons such as no hosted CI run, so partial does not mean half built.

## Ranked gap list: the 25 highest-priority gaps (none or partial rcfd coverage)

Ranking = priority (P1 first) weighted by how many downstream capabilities a gap blocks, then by how little exists. "No decision" scope can start from the roadmap text as written; "Needs decision" lists what the user must choose first.

| Rank | Gap | rcfd ids and status | Next-wave scope needing no user decision | Needs user decision |
|---|---|---|---|---|
| 1 | Distributed-memory parallelism (MPI) | F-100 todo | Halo-exchange abstraction over existing F-016 partitioning; distributed vector/matrix and a parallel Krylov solve behind a trait with a serial fallback; strong-scaling test on cavity. | Which MPI binding or pure-Rust transport; Windows MPI support policy. |
| 2 | Eulerian-Eulerian and mixture multiphase, cavitation | F-127 todo | Mixture model with drift flux and one cavitation mass-transfer model on the VOF infrastructure; acceptance on a bubble-column or hydrofoil public case. | None for mixture and cavitation; full multi-fluid is large, split in two waves. |
| 3 | Mixing-plane and frozen-rotor interfaces | F-126 todo | Frozen rotor on top of F-114 and F-031; then circumferential-averaging mixing plane; verify on the Rotor 37 or Hannover cases listed in F-125. | None. |
| 4 | Lagrangian extensions (two-way, dispersion, injections, breakup, wall film) | F-128 todo, F-056 partial | Two-way momentum/energy coupling, discrete random walk, injection distributions; breakup and wall film in a second wave. | None. |
| 5 | Coupled pressure-velocity solver and AMG completion | F-032, F-027 partial | Close the audit gaps in both; benchmark iteration counts against SIMPLE on the BFS case. | None. |
| 6 | RANS turbulence completeness (SA, k-epsilon family, SST, transition, RSM) | F-041..F-046, F-203, F-206 partial | Finish each model to its stated acceptance with a public-case check; then the near-wall catalogue. | None. |
| 7 | 3D viewport | F-151 todo, F-143 partial | wgpu viewport: surfaces, slices, iso-surfaces, vectors, picking. | wgpu versus alternative; GPU/driver floor. |
| 8 | Native GPU solver | F-101 partial | Port linear solve and flux assembly kernels behind the backend trait; CPU-GPU parity tests. | CUDA-only versus portable wgpu; GPU vendors in scope; f32/f64 policy. |
| 9 | Energy, CHT and radiation completeness | F-050, F-051, F-052, F-130 partial | Complete energy and CHT acceptance; S2S view factors and DO; then spectral WSGG. | None. |
| 10 | Boundary-condition catalogue | F-200, F-204 partial | Mass-flow, total-pressure with direction, outflow weights, backflow options; one regression case each. | None. |
| 11 | CAD import | no id | Not startable without a decision. | OpenCASCADE bindings or other open kernel and LGPL acceptability, or declare STL/mesh-only input. |
| 12 | 2D and axisymmetric meshing and solving | no id | Planar and wedge-axisymmetric treatment in mesh, operators and wall functions; verify on cavity and pipe. | None. |
| 13 | Surface and volume mesh generation | F-131, F-132, F-234, F-108 partial | Finish tet fill, prism layers and quality-driven improvement on closed STL; mesh-quality gates. | Own mesher versus mandating Gmsh/external. |
| 14 | Combustion models and chemistry acceleration | F-054, F-129, F-223 partial | Finish EDM and finite rate, flamelet table reader, premixed progress variable; validate with GRI-Mech cases from F-125. | None. |
| 15 | Pseudo-transient, adaptive stepping, accelerators | F-106, F-209, F-211 partial | Pseudo-transient with automatic timescale; CFL-based adaptivity; high-resolution scheme. | None. |
| 16 | Python bindings (scripting parity with PyFluent/PyCFX) | F-083 partial | Mirror settings tree, commands and numpy field access; example scripts. | Package name and PyPI policy. |
| 17 | Dynamic mesh: layering, remeshing, 6-DOF | F-230 todo, F-063, F-229 partial | Layering and displacement-diffusion smoothing first; 6-DOF coupling after. | None for the first step. |
| 18 | Overset meshes | F-134 todo | Donor search and interpolation on static cases; then moving bodies. | None. |
| 19 | Adjoint and shape morphing | F-137 todo | Design-observable plumbing and a morphing tool first; adjoint after. | Hand-coded discrete adjoint versus automatic differentiation. |
| 20 | Adaptive mesh refinement | F-102, F-124 partial | Criteria on registers, hanging-node refinement for hex/poly, conservative field transfer. | Hanging nodes versus remeshing. |
| 21 | Parametric studies and DOE/optimisation | F-095 partial, no optimisation id | Design-point table, concurrent updates, result tables, DOE sampling. | Optimiser dependency (built-in or external). |
| 22 | Post-processing catalogue (surfaces, derived fields, export formats) | F-072, F-238, F-103, F-119 partial | Iso-surfaces, rakes, offset surfaces, CGNS/EnSight writers. | None. |
| 23 | Two-way FSI and co-simulation | F-139 todo, F-237 partial | Participant interface with data transfer and coupling steps; finish one-way load export. | Which external structural solver to target first. |
| 24 | User-function hook catalogue and material library | F-239 todo, F-111, F-220 partial | Hook catalogue in the WASM API; piecewise and tabulated properties. | None. |
| 25 | Hosted CI and public validation pack | F-001, F-002, F-125 partial | Run CI on a hosted host; complete data-fetch manifests. | Which CI host. |

Considered but ranked lower: wet steam (F-221), acoustics options (F-243), transient blade row (F-138), immersed solid (F-219), FMI (F-242): all P2 or P3.

## Candidate new roadmap items (no matching item today)

Proposed ids only; docs/roadmap.json is not edited. "Decision" = needs a user choice before scheduling.

| Proposed id | Title | Acceptance text | Priority | Decision? |
|---|---|---|---|---|
| F-244 | Planar and axisymmetric (2D) mesh and solver support | 2D planar and wedge-axisymmetric meshes are read or generated, operators and wall functions honour them, and cavity plus pipe-flow cases match reference data. | P2 | No |
| F-245 | CAD import via an open kernel (STEP/IGES) | A STEP assembly imports with parts preserved, tessellates into the surface-mesh pipeline, and a round-trip test reproduces body volumes within tolerance. | P1 | Yes (kernel, licence) |
| F-246 | Subcooled wall boiling with heat-flux partitioning | A heated-wall boiling case reproduces a public benchmark wall temperature within stated tolerance; critical-heat-flux flag available. | P2 | No |
| F-247 | Granular and kinetic-theory solid phase | A fluidised-bed case runs with granular pressure and viscosity closures and matches a public bed-expansion data point. | P3 | No |
| F-248 | Eulerian wall film | Film thickness and mass balance on an inclined plate match an analytic or public case; the film exchanges mass with the discrete phase. | P3 | No |
| F-249 | Collision, coalescence and DEM, dense discrete phase | A particle-pile or hopper test reproduces public angle-of-repose data; collision statistics are logged. | P3 | No |
| F-250 | Reacting discrete phase (devolatilization, char) | Single-particle burnout matches a published curve and couples to species and energy sources. | P3 | No |
| F-251 | Engine ignition models (spark, autoignition, crevice) | A closed-cylinder test shows spark-kernel growth and autoignition timing against published numbers. | P3 | No |
| F-252 | Li-ion battery electro-thermal model | A cell discharge case reproduces a public voltage-time curve and temperature rise within tolerance. | P3 | Yes (electrochemistry scope) |
| F-253 | Fuel cell and electrolysis modules | A PEMFC single-channel case matches a published polarisation curve. | P3 | Yes |
| F-254 | Operating-map automation for turbomachinery | A performance map is produced from chained operating-point runs with restart, giving a pressure-ratio versus flow plot. | P2 | No |
| F-255 | Blade film-cooling hole model | The cooling-flow injection model reproduces a public flat-plate effectiveness curve. | P3 | No |
| F-256 | Aeromechanics modal force and forced response | A vibrating-blade case writes a modal-force history matching a reference single-mode response. | P3 | Yes (structural model source) |
| F-257 | DOE sampling and optimisation drivers | A DOE of N points runs concurrently, a gradient-free optimiser improves an objective on an analytic test problem, and a results table exports. | P2 | Yes (optimiser dependency) |
| F-258 | Solution steering and solver-settings presets | Steering advances schemes and relaxation by convergence state and brings the BFS case to convergence without hand tuning. | P2 | No |
| F-259 | Composition-PDF transport and ECFM combustion | A jet flame matches a public temperature profile with PDF transport; ECFM flame-surface density runs on a test case. | P3 | No |
| F-260 | Intrinsic structural solver for FSI | A clamped plate with pressure load matches analytic deflection and runs two-way against the flow solver. | P3 | Yes (scope) |
| F-261 | Hypersonic numerics and high-temperature air chemistry | A hypersonic cylinder predicts shock stand-off and stagnation heating within tolerance of published data. | P3 | No |
| F-262 | Reduced-order model export | A POD or linear reduced model of a thermal case exports and reproduces held-out states within tolerance. | P3 | No |
| F-263 | Narrow-gap handling and local coordinate frames | Thin-gap contact between zones runs without cell collapse; local frames are usable in BC definitions. | P3 | No |
| F-264 | High-quality headless rendering | A scene renders at a chosen resolution with shadows, deterministic output. | P3 | No |
| F-265 | PCB / ECAD thermal import | A board stackup file imports as layered conduction regions on a published electronics benchmark. | P3 | Yes (file format) |
| F-266 | Hosted web UI | The service shows the settings tree and monitors in a browser with read-only and run-control roles. | P3 | Yes (priority versus desktop GUI) |
| F-267 | Cluster scheduler job templates | Slurm and PBS job scripts generated from a case, tested on dry-run output. | P3 | No |

## Matrix

Columns: rcfd ids are listed with the roadmap status of each id. "rcfd status" is the aggregate (done if all done, todo if all todo, partial for any other mix, none if no id). Priority: P1 core, P2 common, P3 niche; the sentence after the colon is the one-line reason.

### Geometry and meshing

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 1 | CAD import and part management (STEP/IGES/native CAD) | Read CAD assemblies, manage parts and bodies, and tie them to meshing tasks. | S7(Part II ch.4, ch.7) | - | none | No CAD kernel; rcfd starts from faceted geometry or existing meshes. Needs an open-kernel decision (e.g. OpenCASCADE bindings, LGPL). | P1: Real geometry nearly always arrives as CAD; otherwise every case needs external prep. |
| 2 | Unstructured polyhedral mesh data model | General cell/face owner-neighbour mesh with boundary patches and zones. | S3,S7(Part III ch.6) | F-010:done | done |  | P1: Foundation of everything. |
| 3 | Structured block mesh generator | Analytic simple geometries (box, channel, step, O-grid). | S7(Part III ch.6 sources) | F-012:done | done | No general multi-block generator. | P2: Useful for verification and benchmarks. |
| 4 | Watertight-geometry guided meshing workflow | Task-by-task flow from import through sizing, surface mesh, enclosure, boundary layers and volume mesh. | S7(Part II ch.3) | F-120:done, F-131:partial, F-132:partial, F-108:partial | partial | Engine exists; the mesh tasks it would drive are incomplete. | P1: Primary vendor meshing path. |
| 5 | Fault-tolerant meshing workflow | Handles dirty CAD: gaps, leaks, overlaps, contact patches, gap covers, leakage thresholds. | S1,S7(Part II ch.4) | F-232:partial | partial | No gap-cover or contact-patch logic noted. | P2: Eases messy assemblies. |
| 6 | 2D meshing workflow | Dedicated 2D surface meshing and 2D boundary layers. | S7(Part II ch.5) | - | none | No roadmap item for planar or axisymmetric mesh generation or solving. | P2: 2D/axisymmetric is a staple of fast studies and validation. |
| 7 | Surface sizing and surface mesh generation | Local and global size functions and feature capture on faceted geometry. | S7(Part II ch.3,7) | F-131:partial | partial |  | P1: Surface mesh quality bounds volume mesh quality. |
| 8 | Tetrahedral volume meshing | Fill a closed surface with tetrahedra. | S7(Part II ch.7 mesh types) | F-132:partial | partial |  | P1: Robust automatic fill for arbitrary geometry. |
| 9 | Polyhedral meshing and conversion | Convert tets to polyhedra or generate polyhedra directly. | S7(Part II ch.7; Part III ch.6) | F-132:partial, F-102:partial | partial |  | P2: Fewer cells and better gradients. |
| 10 | Hex-core and poly-hexcore meshing | Hex interior joined to prism layers with a polyhedral transition. | S7(Part II ch.7) | F-234:partial | partial |  | P2: Cell-count efficiency for large cases. |
| 11 | Rapid octree meshing | Fast octree-based volume fill. | S7(Part II ch.7) | F-234:partial | partial |  | P3: Speed for very large models. |
| 12 | Boundary-layer (prism) inflation | Prism layers by first height or y+ target with growth law. | S7(Part II ch.3,4,5) | F-108:partial | partial |  | P1: Needed for wall-bounded accuracy. |
| 13 | Multizone and sweep hexahedral meshing | Block decomposition and sweeping. | S7(Part II ch.3 Multizone) | F-235:partial | partial |  | P3: Niche versus automatic fill. |
| 14 | Mesh improvement (smooth, swap, quality-driven) | Operations that repair poor cells. | S7(Part II ch.6) | F-233:partial, F-231:partial | partial |  | P2: Solver robustness depends on mesh quality. |
| 15 | Mesh quality statistics and report | Orthogonality, skewness, aspect ratio and further measures. | S7(Part II ch.6) | F-011:done, F-233:partial | partial | Extended measures incomplete. | P1: Gatekeeper before solving. |
| 16 | Mesh checking before solve | Topology, volume and face checks at load. | S7(Part III ch.6) | F-011:done, F-240:partial | partial |  | P1: Catches fatal mesh errors. |
| 17 | Mesh modification (merge, slit, separate, transform, extrude) | Edit zones and geometry of an existing mesh. | S7(Part III ch.6) | F-231:partial | partial |  | P2: Common housekeeping. |
| 18 | Mesh import: vendor .msh/.cas | Read the vendor mesh file format from public file-structure docs. | S7(Part III ch.3, Part II App.B) | F-118:partial, F-014:partial | partial | HDF5 .cas.h5 mesh only for the HDF5 variant. | P2: Migration path for users with existing meshes. |
| 19 | Mesh import: Gmsh | Open mesh format reader. | GK | F-013:done | done |  | P1: Open-source mesher interop. |
| 20 | Mesh import: OpenFOAM polyMesh, CGNS | Open formats. | GK | F-014:partial | partial |  | P2: Interop with other open solvers. |
| 21 | Mesh import from other third-party formats | Broader reader catalogue (Nastran, ABAQUS, I-deas, ICEM and similar). | S7(Part II App.A, mentions third-party meshes) | F-237:partial, F-014:partial | partial | Which formats are in scope is unspecified. | P3: Long tail. |
| 22 | Mesh partitioning | Split mesh across processes with halos. | S7(Part III ch.43) | F-016:done, F-236:partial | partial |  | P1: Prerequisite for parallel scaling. |
| 23 | Non-conformal / general grid interfaces | Couple mismatched meshes with conservative flux. | S7(Part III ch.6),S6(ch.5) | F-114:done | done |  | P1: Rotating machinery, assemblies. |
| 24 | Periodic boundaries (translational, rotational) | Conformal periodicity. | S3,S6(ch.5) | F-107:done | done |  | P1: Cyclic sectors, fully developed flow. |
| 25 | Overset (chimera) meshes | Overlapping component meshes with interpolation. | S7(Part III ch.6; Part II ch.4) | F-134:todo | todo |  | P2: Large relative body motion. |
| 26 | Solution-adaptive mesh refinement | Refine/coarsen on field criteria with hanging nodes. | S7(Part III ch.38) | F-102:partial, F-124:partial | partial |  | P2: Resolves shocks and interfaces efficiently. |
| 27 | Reference frames and curvilinear coordinates | Local frames for geometry and BC definition. | S7(Part III ch.6) | - | none | No dedicated item. | P3: Convenience. |
| 28 | Narrow-gap modelling | Special treatment of very thin gaps between zones. | S7(Part III ch.6) | - | none | No item. | P3: Rare. |
| 29 | Solution interpolation onto a new mesh | Map a previous solution to another mesh and restart. | S7(Part III ch.3) | F-115:partial, F-241:partial | partial |  | P2: Mesh-refinement workflows. |
| 30 | Blade-passage template mesher | Generate turbomachinery passage meshes from curves. | S2,GK | F-133:todo | todo |  | P3: Turbomachinery-specific. |
| 31 | Mesh format registry and conformance tests | Registry of supported formats with a test per format. | GK | F-237:partial | partial |  | P3: Housekeeping. |

### Turbulence modelling

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 32 | Spalart-Allmaras | One-equation RANS model. | S4(ch.4) | F-041:partial | partial |  | P1: Aerospace workhorse. |
| 33 | k-epsilon (standard, RNG, realizable) | Two-equation family. | S4,S6(4.1) | F-042:partial | partial |  | P1: Industrial default. |
| 34 | k-omega, BSL and SST | Two-equation family with low-Re handling. | S4,S6(4.1) | F-043:partial | partial |  | P1: Most used for external aero and CHT. |
| 35 | GEKO generalized two-equation | Two-equation model with tunable coefficients. | S2,S4,S6(4.1),S8 | F-206:partial | partial | Item names a generalised two-equation model. | P2: Vendor flagship tunable model. |
| 36 | Reynolds stress models (LRR/SSG; omega-based) | Seven-equation closures. | S4,S6(4.1) | F-046:partial | partial |  | P2: Swirling and separated flows. |
| 37 | Explicit algebraic Reynolds stress (EARSM) | Algebraic stress extension of two-equation models. | S4,S6(4.1) | F-206:partial | partial |  | P3: Niche. |
| 38 | Laminar-turbulent transition models | Predict transition onset (k-kl-omega, transition SST, intermittency, algebraic). | S4,S6(4.1) | F-046:partial | partial | Roadmap names gamma-family only. | P2: External aero, turbomachinery. |
| 39 | Zero-equation and mixing-length | Algebraic eddy viscosity. | S6(4.1) | F-206:partial | partial |  | P3: Simple and fast. |
| 40 | v2-f | Four-equation near-wall model. | GK | F-206:partial | partial |  | P3: Niche. |
| 41 | Curvature and corner corrections | Modifiers to two-equation models. | S6(4.1),S8 | F-206:partial | partial |  | P3: Improves swirl accuracy. |
| 42 | Buoyancy turbulence modifiers | Buoyancy production and dissipation terms. | S6(4.1) | F-206:partial | partial | Not explicit in the item. | P2: Natural convection flows. |
| 43 | Scale-adaptive simulation (SAS) | Unsteady RANS that resolves scales. | S4,S6(4.1) | F-045:partial | partial |  | P2: Cheaper than LES. |
| 44 | Detached eddy simulation incl. IDDES | Hybrid RANS-LES. | S4,S6(4.1) | F-045:partial, F-207:partial | partial |  | P2: Massive separation. |
| 45 | SBES and SDES hybrid models | Stress-blended hybrids. | S4,S6(4.1),S8 | F-207:partial | partial |  | P3: Newer vendor hybrids. |
| 46 | LES: Smagorinsky, WALE | Basic subgrid models. | S4,S6(4.1) | F-045:partial | partial |  | P1: Core scale-resolving capability. |
| 47 | LES: dynamic Smagorinsky, Vreman, sigma, WMLES | Advanced subgrid and wall-modelled LES. | S4,S8 | F-207:partial | partial |  | P2: High-Re LES. |
| 48 | Embedded LES and synthetic turbulence inlets | LES zones in RANS and inflow generators. | S4,S7 | F-207:partial | partial |  | P3: Niche. |
| 49 | Near-wall treatments (standard, scalable, enhanced, y+-insensitive) | Wall function catalogue. | S4,S6(4.2) | F-040:done, F-203:partial | partial | Standard exists, rest incomplete. | P1: Accuracy at practical y+. |
| 50 | Wall roughness | Roughness effect on the law of the wall. | S4 | F-203:partial | partial | Item names equivalent sand-grain roughness. | P2: Heat transfer, aero. |
| 51 | Turbulence in multiphase flow | Per-phase and mixture turbulence. | S6(7.12),S4 | F-127:todo | todo |  | P2: Follows multiphase. |
| 52 | Turbulence model plugin interface | Extensible model API. | GK | F-044:partial | partial |  | P2: Extensibility. |

### Heat transfer, CHT and radiation

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 53 | Energy equation and buoyancy | Temperature or enthalpy, Boussinesq, ideal-gas density. | S4(ch.5),S6(1.2) | F-050:partial | partial |  | P1: Core to thermal problems. |
| 54 | Natural convection | Buoyancy-driven flows. | S4(ch.5) | F-050:partial | partial |  | P1: Wide use. |
| 55 | Conjugate heat transfer | Solid-fluid coupling. | S4,S6(1.2),S8 | F-051:partial | partial |  | P1: Electronics, engines. |
| 56 | Thin-wall / shell conduction and thermal contact resistance | Layered shell conduction at walls and interfaces. | S6(5.5),S8 | F-215:partial | partial |  | P2: Practical electronics models. |
| 57 | Per-region model sets | Different physics models in different regions. | S6(ch.1 domains) | F-216:partial | partial |  | P2: Multi-domain realism. |
| 58 | Radiation: P1, S2S, discrete ordinates | Commonly used radiation models. | S4(ch.5),S6(ch.10),S8 | F-052:partial | partial |  | P1: Combustion, furnaces. |
| 59 | Radiation: Rosseland, Monte Carlo, discrete transfer, DTRM | Additional radiation models. | S4,S6(ch.10) | F-130:partial | partial |  | P3: Special cases. |
| 60 | Spectral bands, WSGG, scattering, solar load | Non-grey and solar radiation. | S4,S8,GK | F-130:partial | partial |  | P2: Automotive and building. |
| 61 | Heat exchanger models (macro, dual-cell) | Lumped exchanger cores. | S4(ch.6),S7 | F-217:partial, F-113:partial | partial |  | P2: HVAC and automotive. |
| 62 | Periodic heat transfer | Streamwise-periodic thermal fully developed flow. | S4,S7(16.4) | F-107:done | done | Thermal periodicity not confirmed in roadmap text. | P3: Compact exchanger cores. |
| 63 | Solidification and melting | Enthalpy-porosity. | S4(ch.15),S7 | F-212:partial | partial |  | P2: Casting, phase-change materials. |
| 64 | Electric potential and Joule heating | Potential equation with heating. | S4,S8 | F-213:partial | partial |  | P2: Busbars, heaters. |
| 65 | Virtual blade / actuator disk | Rotor momentum source. | S7(ch.18) | F-218:partial | partial |  | P3: Rotor and propeller studies. |
| 66 | Fans, porous jump, vents | Lumped flow components. | S7,S8 | F-113:partial, F-200:partial | partial |  | P2: HVAC. |
| 67 | Wall condensation model | Condensation on cold walls. | S6(1.7) | F-226:partial | partial |  | P3: Specialised. |
| 68 | PCB / ECAD thermal import | Electronics board thermal setup. | S7(16.6) | - | none | No item. | P3: Electronics niche. |

### Species, combustion and chemistry

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 69 | Species transport and multicomponent mixtures | Convection-diffusion of species. | S4(ch.7),S6 | F-053:done | done |  | P1: Base for reacting flow. |
| 70 | Finite-rate chemistry | Volumetric reaction rates. | S4,S6(9.4) | F-054:partial | partial |  | P1: General reacting flow. |
| 71 | Eddy dissipation model | Mixing-limited turbulence-chemistry interaction. | S4,S6(9.3) | F-054:partial | partial |  | P1: Industrial combustion. |
| 72 | Composition PDF transport | Particle-based PDF method. | S4(7.2) | - | none | No item. | P3: Research-grade. |
| 73 | Non-premixed combustion (flamelet / mixture fraction PDF) | Pre-tabulated chemistry. | S4(ch.8),S6(9.7) | F-129:partial | partial |  | P2: Efficient diffusion flames. |
| 74 | Premixed and partially premixed combustion | Progress-variable and burning-velocity models. | S4,S6(9.8) | F-129:partial | partial |  | P2: Gas turbines. |
| 75 | Flamelet-generated manifold (FGM) | Tabulated chemistry incl. GPU use. | S8,S7(ch.20) | F-129:partial | partial |  | P2: Fast modern combustion. |
| 76 | ECFM extended coherent flame model | Flame surface density. | S6(9.9) | - | none | No item. | P3: Engine-specific. |
| 77 | Chemistry acceleration (ISAT, dynamic reduction) | Speed up stiff integration. | S4(7.3),S7(19.3) | F-223:partial | partial |  | P2: Makes detailed mechanisms practical. |
| 78 | Mechanism input formats (CHEMKIN, thermo, transport, YAML) | Read chemistry decks. | GK | F-054:partial, F-224:partial | partial |  | P1: Open mechanisms are standard. |
| 79 | Reactor network and 0D reactor toolkit | PSR, PFR, ignition delay. | S4(ch.7) | F-112:partial, F-225:partial | partial |  | P3: Mechanism analysis. |
| 80 | NOx formation | Thermal, prompt and fuel NOx. | S4(ch.9),S6(9.13) | F-222:partial | partial |  | P2: Emissions. |
| 81 | Soot formation | Empirical and moment soot models. | S4(ch.9),S6(9.15) | F-222:partial | partial |  | P2: Emissions. |
| 82 | Engine ignition (spark, autoignition, crevice) | IC engine models. | S4(ch.10) | - | none | No item. | P3: Engine-specific. |
| 83 | Autoignition model | Ignition-delay based. | S6(9.12) | F-112:partial | partial | 0D only in roadmap. | P3: Niche. |
| 84 | Surface and CVD reactions | Wall reactions. | S4(7.1) | F-224:partial | partial | Only surface decks input is listed. | P3: Niche. |
| 85 | Particle reactions (devolatilization, char, coal) | Reacting discrete phase. | S4(ch.12) | - | none | F-128 lacks particle reactions. | P3: Coal and biomass. |
| 86 | Electrochemical reactions | Electrode reaction kinetics. | S4(7.1.4) | - | none | No item. | P3: Batteries and fuel cells. |
| 87 | Material property library | Temperature-dependent, mixing rules, real gas. | S6(1.4,1.5),S8 | F-111:partial, F-220:partial | partial |  | P1: Accurate properties. |
| 88 | Non-Newtonian viscosity laws | Power law, Carreau, Cross and others. | S6(ch.1) | F-110:done | done |  | P2: Polymers, blood. |
| 89 | Viscoelastic models | Oldroyd-B, Giesekus and similar. | GK | F-135:partial | partial |  | P3: Beyond vendor core. |

### Multiphase and discrete phase

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 90 | Volume of fluid (free surface) | Interface capturing. | S4(ch.14),S6(7.18) | F-055:partial | partial |  | P1: Most used multiphase model. |
| 91 | Surface tension and wall adhesion | Interfacial forces. | S4(ch.14) | F-055:partial | partial |  | P1: Droplets, capillarity. |
| 92 | Open-channel flow and wave boundary conditions | Waves and numerical beach. | S4(ch.14) | F-227:partial | partial |  | P2: Marine. |
| 93 | Mixture model (incl. algebraic slip) | Slip/drift mixture multiphase. | S4,S6(7.5,7.19) | F-127:todo | todo |  | P1: Cheaper multiphase. |
| 94 | Eulerian multi-fluid | Per-phase momentum with interphase exchange. | S4,S6(7.8) | F-127:todo | todo |  | P1: Bubble columns, fluidised beds. |
| 95 | Interphase forces (drag, lift, virtual mass, lubrication, dispersion) | Closure catalogue for Eulerian models. | S4,S6(7.8) | F-127:todo | todo |  | P1: Required by Eulerian. |
| 96 | Cavitation mass transfer | Vapour formation at low pressure. | S4,S6(7.15) | F-127:todo | todo |  | P1: Pumps, injectors. |
| 97 | Evaporation-condensation | Phase change. | S4,S6(7.15) | F-226:partial | partial |  | P2: Thermal management. |
| 98 | Subcooled wall boiling and critical heat flux | Heat-flux partitioning at heated walls. | S2,S4 | - | none | No item. | P2: Nuclear and cooling. |
| 99 | Population balance / MUSIG | Size distribution of dispersed phase. | S4(14.9),S6(7.11) | F-228:partial | partial |  | P2: Bubbly flow. |
| 100 | Granular flow / kinetic theory | Solid-phase granular properties. | S4(ch.14) | - | none | No item. | P3: Fluidised beds. |
| 101 | Interfacial area and regime models (AIAD, GENTOP) | Flow-regime-aware models. | S4(ch.14) | - | none | No item. | P3: Research level. |
| 102 | Multi-fluid VOF | VOF combined with Eulerian dispersed phase. | S4(ch.14) | - | none | No item. | P3: Niche. |
| 103 | Wet steam / non-equilibrium condensation | Condensing steam in turbines. | S2,S4(ch.14) | F-221:todo | todo |  | P3: Steam turbines. |
| 104 | Eulerian wall films | Thin film on walls. | S4(ch.17) | - | none | No item. | P3: Spray and icing. |
| 105 | Discrete phase: tracking and basic forces | One-way Lagrangian particles. | S4(ch.12) | F-056:partial | partial |  | P1: Sprays, dust. |
| 106 | Discrete phase: two-way coupling | Back-reaction on carrier phase. | S4(ch.12),S8 | F-128:todo | todo |  | P1: Needed for most real sprays. |
| 107 | Turbulent dispersion (random walk) | Stochastic tracking. | S4,S8 | F-128:todo | todo |  | P1: Particle accuracy. |
| 108 | Injection types and size distributions | Surface, cone, group; Rosin-Rammler. | S6(8.8),S8 | F-128:todo | todo |  | P1: Setup realism. |
| 109 | Droplet evaporation and boiling | Vaporisation laws. | S4,S8 | F-128:todo | todo |  | P2: Fuel sprays. |
| 110 | Breakup and atomisation models | Primary and secondary breakup. | S4(ch.12) | F-128:todo | todo |  | P2: Sprays. |
| 111 | Particle-wall interaction, erosion, accretion, wall film | Wall treatment of particles. | S4(ch.12) | F-128:todo | todo |  | P2: Erosion prediction. |
| 112 | Collision, coalescence and DEM | Particle-particle interaction. | S4(ch.12-13) | - | none | No item. | P3: Dense flows. |
| 113 | Macroscopic particle model | Resolved large particles. | S4(ch.13),S7(ch.25) | - | none | No item. | P3: Niche. |
| 114 | Continuous fibre model | Fibre spinning. | S4(ch.22),S7(ch.34) | - | none | No item. | P3: Specialised. |

### Compressible and high-speed

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 115 | Density-based compressible solver | Riemann solvers, explicit and implicit. | S5(23.5),S1 | F-060:done | done |  | P1: Transonic and supersonic. |
| 116 | Pressure-based compressible solver | Compressible via pressure-velocity coupling. | S1,S5(23.4) | F-061:done | done |  | P1: Low-Mach to compressible. |
| 117 | Real gas / EOS options | Peng-Robinson, Redlich-Kwong, IAPWS-IF97. | S2,S6(ch.12) | F-061:done, F-111:partial, F-220:partial | partial |  | P2: Property accuracy. |
| 118 | Real-gas property tables (REFPROP, IAPWS, RGP files) | Tabulated real fluids. | S2,S6(12.6-12.8) | F-220:partial | partial |  | P2: ORC, supercritical. |
| 119 | Equilibrium phase change | Saturation-based property treatment. | S6(12.3) | F-220:partial | partial |  | P3: Steam. |
| 120 | Compressible BCs (total conditions, non-reflecting) | Standard compressible BCs. | S6(2.10) | F-062:partial | partial |  | P1: Needed for well-posed compressible runs. |
| 121 | Hypersonic numerics | Shock handling and high-temperature models. | S7(ch.17) | - | none | Nothing beyond F-060. | P3: Niche. |

### Moving and dynamic mesh, rotation

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 122 | Single and multiple reference frames (MRF) | Rotating-frame source terms. | S3,S4(ch.2) | F-031:partial | partial |  | P1: Pumps, fans. |
| 123 | Sliding mesh | Transient rotor-stator. | S4(ch.3) | F-063:partial, F-114:done | partial |  | P1: Time-accurate rotation. |
| 124 | Mixing plane / stage interface | Circumferential averaging between rows. | S4(ch.2),S6(ch.5) | F-126:todo | todo |  | P1: Steady multistage turbomachinery. |
| 125 | Frozen rotor interface | Fixed relative position. | S6(ch.5) | F-126:todo | todo |  | P2: Initial turbomachinery solutions. |
| 126 | Dynamic mesh smoothing and deformation | Spring/diffusion smoothing. | S7(13.6) | F-063:partial, F-230:todo | partial |  | P1: Moving boundaries. |
| 127 | Dynamic layering and local remeshing | Layer add/remove and remesh. | S7(13.6) | F-230:todo | todo |  | P2: Large stroke motion. |
| 128 | Six-DOF rigid-body dynamics | Free-body motion from loads. | S4(ch.3),S6(ch.11) | F-229:partial | partial |  | P2: Store separation. |
| 129 | Motion definitions (profiles, zone motion) | Prescribed motion data. | S7(ch.11) | F-063:partial, F-105:done | partial |  | P2: Prescribed motion. |
| 130 | Immersed solids | Non-conforming solid domain. | S6(1.2) | F-219:todo | todo |  | P3: Alternative to remeshing. |

### FSI and co-simulation

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 131 | One-way FSI load export | Pressure and temperature mapped to structural mesh. | S7(ch.3),S1 | F-237:partial | partial |  | P1: Most common FSI. |
| 132 | Two-way FSI via co-simulation | Coupling with a structural solver. | S1,S6(14.1) | F-139:todo | todo |  | P2: Aeroelastic problems. |
| 133 | Intrinsic structural model | Built-in solid mechanics. | S4(ch.16) | - | none | No item; needs scope decision. | P3: Large scope. |
| 134 | FMI co-simulation | Functional mock-up coupling. | S6(14.2),S7(ch.3) | F-242:todo | todo |  | P3: Systems integration. |
| 135 | System coupling to other tools | Cross-product coupling. | S6(14.1) | F-139:todo | todo |  | P2: Multiphysics workflows. |

### Turbomachinery

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 136 | Turbomachinery guided workflow | Wizard from components to results. | S7(ch.14),S2 | F-120:done, F-123:partial | partial |  | P2: Faster setup. |
| 137 | Turbo post-processing | Spanwise averages and efficiency. | S7(14.10) | F-123:partial | partial |  | P2: Design evaluation. |
| 138 | Transient blade row (profile, time, Fourier transformation) | Pitch-change reduced-domain unsteady methods. | S9,S10,S2 | F-138:todo | todo |  | P2: Efficient unsteady rotor-stator. |
| 139 | Harmonic balance / harmonic analysis | Frequency-domain periodic flow. | S9,S2 | F-138:todo | todo |  | P2: Aeromechanics. |
| 140 | Operating maps and operating point cases | Automated performance-map runs. | S6(ch.13),S2 | - | none | F-095 is generic only. | P2: Performance curves. |
| 141 | Blade film cooling | Cooling hole models. | S6(6.2),S2 | - | none | No item. | P3: Hot-section niche. |
| 142 | Aeromechanics forced-response coupling | Generalised modal force. | S2 | - | none | No item. | P3: Specialised. |

### Acoustics, electromagnetics and other physics

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 143 | FW-H far-field acoustics | Integral acoustic propagation. | S4(ch.11),S8 | F-136:partial | partial |  | P2: Noise prediction. |
| 144 | Broadband noise sources | Turbulence-based noise estimates. | S4,S6(15.2) | F-243:todo | todo |  | P3: Quick estimates. |
| 145 | Wave-equation acoustics and sponge layers | Direct acoustic solution. | S4(ch.11),S8 | F-243:todo | todo |  | P3: Niche. |
| 146 | Magnetohydrodynamics | Lorentz force coupling. | S4(ch.21) | F-214:partial | partial |  | P3: Metallurgy. |
| 147 | Li-ion battery models (MSMD) | Electro-thermal cell models. | S1,S7(ch.31) | - | none | No item. | P3: Vendor vertical, niche for core CFD. |
| 148 | Fuel cells and electrolysis | PEMFC and SOFC models. | S7(ch.32) | - | none | No item. | P3: Niche vertical. |
| 149 | Reduced-order models | Export of surrogate models. | S7(ch.35) | - | none | No item. | P3: Systems use. |
| 150 | User-defined scalar transport | Extra transport equations. | S4(ch.1) | F-202:partial | partial |  | P2: Extensibility. |

### Numerics and solvers

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 151 | Pressure-based segregated solver (SIMPLE, SIMPLEC, PISO) | Standard segregated algorithms. | S5(23.4) | F-028:done, F-029:done | done |  | P1: Core. |
| 152 | Pressure-based coupled solver | Block-coupled momentum-pressure. | S5,S8 | F-032:partial | partial |  | P1: Robustness and speed. |
| 153 | Density-based coupled solver | Coupled explicit/implicit. | S5(23.5) | F-060:done | done |  | P1: Compressible. |
| 154 | Algebraic multigrid | Linear solver acceleration. | S5(23.7),S7(ch.36) | F-027:partial | partial |  | P1: Scalable solves. |
| 155 | Krylov solvers and preconditioners | CG, BiCGStab, GMRES. | GK | F-026:done | done |  | P1: Basics. |
| 156 | Full multigrid and hybrid initialisation | Better starting solutions. | S5(23.8,23.9) | F-205:partial | partial |  | P2: Convergence from rough starts. |
| 157 | Pseudo-transient under-relaxation | Local pseudo-time stepping. | S5(23.6) | F-106:partial | partial |  | P1: Robust steady solves. |
| 158 | Spatial discretisation schemes | Upwind, central, QUICK, limiters, gradients. | S5(23.3) | F-021:done, F-022:done, F-211:partial | partial |  | P1: Accuracy and boundedness. |
| 159 | Diffusion with non-orthogonal correction | Mesh-quality handling. | GK | F-023:done | done |  | P1: Robustness on real meshes. |
| 160 | Time integration schemes | Implicit Euler, BDF2, Crank-Nicolson. | S8 | F-024:done | done |  | P1: Transient accuracy. |
| 161 | Adaptive time stepping | CFL- and error-based steps. | GK | F-024:done, F-209:partial | partial |  | P1: Efficiency. |
| 162 | Non-iterative time advancement | Fractional step. | S7(ch.36) | F-210:partial | partial |  | P3: Faster LES. |
| 163 | Poor-mesh numerics | Robust treatment of bad cells. | S8 | F-208:partial | partial |  | P2: Practical meshes. |
| 164 | Solution steering and convergence aids | Automated solver-setting progression. | S7(ch.36) | F-209:partial | partial |  | P2: Ease of use. |
| 165 | Convergence monitoring and stopping criteria | Residuals and report convergence. | S7(ch.36) | F-030:done, F-109:done | done |  | P1: Core. |
| 166 | Native GPU solver | Pressure-based solver on multi-GPU. | S1,S8 | F-101:partial | partial |  | P1: Large speedups; vendor emphasis. |
| 167 | Distributed-memory parallel (MPI) | Multi-node scaling. | S6(ch.17),S7(ch.43) | F-100:todo | todo |  | P1: HPC requirement. |
| 168 | Shared-memory parallel (threads) | Rayon threading. | GK | F-025:done | done |  | P1: Single node. |
| 169 | Load balancing and partition control | Runtime balance. | S7(ch.43) | F-236:partial | partial |  | P2: Scaling. |
| 170 | HPC scheduler integration | Cluster launch templates. | S7(ch.43,44) | F-121:partial | partial |  | P3: Operational convenience. |
| 171 | Run-time statistics and sampling | Transient means and RMS. | S8 | F-116:done | done |  | P2: LES and unsteady post. |
| 172 | Checkpoint and restart | Case and data files. | S7(ch.3) | F-030:done | done |  | P1: Core. |

### Setup, UX and workflow

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 173 | Boundary-condition catalogue | Wide BC set. | S6(ch.2) | F-200:partial, F-020:done, F-204:partial | partial |  | P1: Breadth gate. |
| 174 | Initialisation and patching | Initial fields and region patches. | S6(ch.3) | F-205:partial, F-124:partial | partial |  | P1: Convergence. |
| 175 | Profiles and tabulated inputs | File profile BCs. | S7(7.9),S6(2.9) | F-105:done | done |  | P1: Real data inputs. |
| 176 | Expression language | Named expressions with units. | S7(ch.5),S2 | F-104:done | done |  | P1: Setup flexibility. |
| 177 | Zone sources | Sources for all equations. | S6(1.3) | F-201:partial | partial |  | P1: Modelling extensions. |
| 178 | Unit systems | Unit handling. | S7(ch.4) | F-003:done | done |  | P1: Avoid errors. |
| 179 | Material database | Library of fluids and solids. | S6(1.4) | F-111:partial | partial |  | P1: Setup speed. |
| 180 | Settings tree and schema | Typed settings. | GK | F-005:done, F-080:done | done |  | P1: Foundation. |
| 181 | Journals and transcripts | Record and replay. | S7(ch.3),S2 | F-084:done | done |  | P1: Automation. |
| 182 | Python scripting API | Scripting comparable to PyFluent and PyCFX. | S1,S2 | F-083:partial, F-082:done | partial |  | P1: Automation standard. |
| 183 | Text/CLI batch mode | Headless runs. | S7(ch.4) | F-081:done, F-121:partial | partial |  | P1: HPC. |
| 184 | Task-based guided workflows | Guided setup. | S7(Part II ch.2) | F-120:done | done |  | P2: Ease of use. |
| 185 | User-defined functions | Compiled or sandboxed hooks. | S1,S6(ch.19) | F-122:done, F-239:todo | partial |  | P1: Industrial customisation. |
| 186 | Setup validation | Pre-solve checks. | S7(ch.2,3) | F-240:partial | partial |  | P2: Catch errors early. |
| 187 | Graphical user interface | Desktop app with panels. | S7(ch.1),S2 | F-140:done, F-141:done, F-142:done | done |  | P1: Usability. |
| 188 | 3D viewport | Interactive 3D graphics. | S7(ch.40) | F-151:todo | todo |  | P1: Core of a GUI. |
| 189 | Monitor panels | Plots of residuals and reports. | S7(ch.40) | F-144:partial | partial |  | P1: Run feedback. |
| 190 | Solver service API | RPC control. | GK | F-085:done | done |  | P2: Integration. |
| 191 | Web interface / remote UI | Browser access. | S7(ch.47) | F-085:done | done | Service exists; no web UI item. | P3: Convenience. |
| 192 | Parametric studies / design points | Update parameters across runs. | S7(ch.46),S8 | F-095:partial | partial |  | P1: Engineering workflow. |
| 193 | DOE and optimisation drivers | Sampling and optimizers. | S7(ch.46),S1 | - | none | Only generic studies exist. | P2: Design exploration. |
| 194 | Adjoint solver and shape optimisation | Gradient of observables, morphing. | S7(ch.48),S1 | F-137:todo | todo |  | P2: Design optimisation. |
| 195 | Simulation reports | Auto-generated documents. | S7(ch.45) | F-074:done | done |  | P2: Deliverables. |
| 196 | Assistant (LLM) workflows | Natural-language setup. | rcfd-unique, not in vendor sources | F-094:partial, F-145:done, F-090:done | partial |  | P3: Differentiator, not parity. |

### Post-processing and reporting

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 197 | VTK output | Standard visualisation files. | GK | F-070:done | done |  | P1: Interop. |
| 198 | Probes, integrals, force coefficients | Quantitative reports. | S7(ch.41) | F-071:done, F-109:done | done |  | P1: Engineering answers. |
| 199 | Report definitions | Reusable reports. | S7(ch.41) | F-109:done | done |  | P1: Automation. |
| 200 | Derived fields | Vorticity, Q-criterion, y+. | S7(ch.42) | F-072:partial | partial |  | P1: Diagnosis. |
| 201 | Surfaces and cell registers | Planes, iso-surfaces, regions. | S7(ch.39) | F-124:partial, F-238:partial | partial |  | P1: Core visualisation. |
| 202 | Contours, vectors, pathlines, LIC | Graphics objects. | S7(ch.40) | F-073:partial, F-143:partial, F-151:todo | partial |  | P1: Core. |
| 203 | XY plots and FFT | Plotting and spectra. | S7(ch.40) | F-074:done, F-144:partial | partial |  | P2: Unsteady analysis. |
| 204 | Animations and transient post | Time series animation. | S7(ch.40) | F-070:done, F-117:partial | partial |  | P2: Presentation. |
| 205 | Ray tracing / high-quality rendering | Photoreal output. | S7(ch.40) | - | none | No item. | P3: Presentation only. |
| 206 | Field function definitions | Custom field variables. | S7(ch.42) | F-104:done | done |  | P2: Flexibility. |
| 207 | Export: EnSight, CGNS, Tecplot, FieldView, Patran | Interchange formats. | S1,S2 | F-103:partial, F-119:partial | partial |  | P2: Tool interop. |
| 208 | Transient export during run | Automatic export. | S1 | F-116:done | done |  | P2: Pipeline. |
| 209 | Case comparison and difference fields | Compare runs. | GK | F-238:partial | partial |  | P3: Convenience. |

### Validation and infrastructure

| # | Capability | What it is | Source read | rcfd ids | rcfd status | Gap note | Priority and reason |
|---|---|---|---|---|---|---|---|
| 210 | Validation case registry | Benchmark cases. | S1 | F-006:done | done |  | P1: Credibility. |
| 211 | Public-data validation pack | Fetch scripts for open benchmark data. | S1 | F-125:partial | partial |  | P1: Parity claim needs evidence. |
| 212 | Cross-platform CI | Hosted CI on Windows and Linux. | GK | F-001:partial, F-002:partial | partial |  | P2: Quality gate. |
| 213 | Documentation and tutorials | User docs and examples. | S7 | F-096:todo | todo |  | P2: Adoption. |

## Declined and not done

- No vendor binary or install was opened. No decompilation or disassembly tools were used.
- No Ansys Discovery, Icepak, Mechanical or Workbench documentation was read (only mentions on the Fluent product page).
- No textbook or paper was read in this pass; all textbook-level statements are unverified general knowledge.
- docs/roadmap.json was not modified; candidates above are proposals.
