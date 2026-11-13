# ADR 0291: Scaffold of the intermediate tier: module layout, model extension data, element-trait extension points

Status: **REGISTERED 2026-11-13, written before any code of F-388 and before any run of its checks; planned by ADR 0290**.
Not yet accepted by the maintainer. Before this text nothing of F-388 was written, compiled or run: the code and documents
under "What was read" were read; no script and no library code was run. Results go in the addendum at the end; nothing above
the addendum is changed after a run.

Work package SI-1 of the structural solver, intermediate tier ([ADR 0290](0290-structural-solver-intermediate-tier-plan.md),
sections 2 and 3, the entry for ADR 0291). Roadmap item F-388 (depends on F-370, F-372, F-374, F-377). Size S. Number 0291,
fixed by the plan; the file name of the planned stub is kept and the stub's text is kept unchanged at the end ("The planned
stub, as written"). Branch `f388-intermediate-scaffold` from main ee449e26, worktree `w49i-work/f388`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (every
model that does not use a new kind, property, constraint or support gives the bytes it gave before); no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited; clean-room (there is no algorithm here: the package declares data and refusals).

## What F-388 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The modules, element kinds, property data, constraint
> and support data and the three defaulted element entry points of ADR 0290 section 2 exist in rcfd-fem and rcfd-post; every
> test of the basic tier (the sb_ files of rcfd-fem, rcfd-linalg, rcfd-io, rcfd-post and rcfd-api) compiles and passes unedited,
> and a registered set of basic models (patch test, cantilever, bar modes) gives displacements, stresses and frequencies with
> the same bits as before the change; the generic assembly hook called with the element stiffness reproduces
> assemble::stiffness bit for bit for 1 and 8 threads; each new element kind, formulation, constraint and support variant is
> refused by the static solve with an error naming the variant and the set, never ignored; the baseline hashes are unchanged.

Clauses: (c1) the modules of ADR 0290 section 2.2 (twelve in `rcfd-fem`) and section 2.1 (two in `rcfd-post`) exist; (c2) the
element kinds `Hex20`, `Beam2`, `Shell4` exist; (c3) the property data (Hex8 formulation `Full | BBar | Incompatible`, beam
section and orientation, shell thickness) exist; (c4) the constraint data (`Linear`, `RigidSet`, `Cyclic`) and the support data
(elastic support per node and direction, per area on a surface set) exist; (c5) the `Element` trait has the defaulted entry
points `geometric_stiffness`, `internal_force`, `tangent`; (c6) every `sb_` test of the five crates compiles and passes
unedited; (c7) the registered basic models give displacements, stresses and frequencies with the bits of main ee449e26; (c8) the
generic assembly hook with the element stiffness reproduces `assemble::stiffness` bit for bit, 1 and 8 threads; (c9) each new
element kind, formulation, constraint variant and support variant is refused by the static solve with an error naming the
variant and the set; (c10) never ignored; (c11) the baseline hashes are unchanged.

The brief adds: reuse `assemble::assemble_with` of ADR 0275 if it is the hook (it is: finding 3) and register its bit-for-bit
check instead of adding a duplicate; per-node prescribed values and forces stay arguments of `solve::solve_static_with`; record
the bits of the basic models from main before the change and compare after; check the dependent crates for exhaustive matches;
run `baseline_hashes` in release.

## What was read (main ee449e26, read only)

1. `AGENT_RULES.md`; ADR 0290 sections 2.1 to 2.5 (layout, data model additions, what changes in the basic tier, interfaces)
   and the entries for 0291 to 0302 in its section 3; this ADR's stub; the stubs 0293, 0294, 0295, 0296, 0297 (goal and
   acceptance: the data they will read); ADR 0275 (D1, D2: `assemble_with`, the batches in element order, its addendum and open
   question 3, which recommends that this package re-use `assemble_with`).
2. Code: `crates/rcfd-fem/src/{lib.rs, mesh.rs, model.rs, element.rs, assemble.rs, error.rs, constraint.rs (prescribed_equations),
   solve.rs (solve_static_report), modal.rs (modal_analysis_with), recover.rs (fit_basis, extrapolation_matrix), quadrature.rs
   (rules), builders.rs (hex_box, tet_box, upgrade_to_tet10, BOX_FACES), mass.rs, loads.rs, thermal.rs (uses of element_for)}`;
   `crates/rcfd-post/src/{lib.rs, solid_output.rs (vtk_from_fem)}`; `crates/rcfd-io/src/vtk.rs` (the finite-element writer:
   `vtk_cell_nodes` admits only 10, 12, 24); the test lists of `rcfd-fem`, `rcfd-linalg`, `rcfd-io`, `rcfd-post`, `rcfd-api`;
   the head of `crates/rcfd-fem/tests/sb_modal.rs` (bar set-up, helpers); `rcfd-io/tests/sb_vtk_elements.rs` (the refused cell
   types 42 and 13).
3. **Not read:** any finite-element code outside this repository; the VTK documentation (the node order of type 25 below is
   from memory of the public VTK file-format description and is only declared here; ADR 0292 confirms it, finding 6).

Findings that shape the design:

1. The basic tier is built through constructors with private fields, and `ElementKind`, `Constraint`, `Load`, `Material`,
   `FemError` are `#[non_exhaustive]` (the request of ADR 0290 section 2.4 was followed). New fields and variants therefore break
   no test crate; the side structure of the stub's "what could make it miss" is not needed.
2. Inside `rcfd-fem` the matches on `ElementKind` and `Constraint` are exhaustive (non-exhaustive only binds other crates):
   `mesh.rs` (nine methods), `element.rs` (`element_for`), `recover.rs` (`fit_basis`, `extrapolation_matrix`), `model.rs`
   (`add_constraint`), `constraint.rs` (`prescribed_equations`). Each gets arms; outside the crate `rcfd-post::vtk_from_fem` has a
   wildcard that refuses by name. No other crate of the workspace names `rcfd_fem` (only `rcfd-post`).
3. `assemble::assemble_with(model, threads, f)` (ADR 0275 D1) is the generic hook of ADR 0290 section 2.3: any square
   node-major per-element matrix, scattered sequentially in element order on the stiffness pattern, bit-identical for any thread
   count; `stiffness_with` is `assemble_with` with the element stiffness. It is re-used; no second hook is added (open point 1).
4. `element_for(kind) -> &'static dyn Element` is infallible and is called by the `sb_` tests; its signature stays. A kind whose
   package has not landed gets a placeholder element whose every fallible entry point refuses.
5. `SolidMesh::add_block` checks `det J > 0` at the nodes of `kind.reference_nodes()` through `kind.shape_gradients`. The
   structural kinds have no volume map; their element checks belong to ADR 0294 and 0295.
6. `rcfd_io::vtk::fe_vtu_string` refuses every cell type but 10, 12, 24, so the file list's condition ("only if the writer
   restricts the cell types") holds and `vtk.rs` gains 3, 9, 25.
7. `rcfd-api` has no `sb_` test file (the basic tier's API entry, ADR 0287, is not on main ee449e26): clause c6 covers four
   crates in fact; the addendum says so.

## Design

### D1. Modules (c1)

`rcfd-fem/src/lib.rs` gains, in alphabetical order with the existing ones, `pub mod` lines for `beam`, `buckling`, `cyclic`,
`geometric`, `harmonic`, `hex20`, `hex8x`, `mpc`, `nonlinear`, `rotating`, `shell`, `transient`, each a file with a module
comment naming its ADR (0292 to 0302 as in ADR 0290 section 2.2). The modules that own a pending variant also hold the
constant `pub const PENDING: Option<&str>` (`Some("ADR 0292")` etc.): the one place that says the variant is not yet available,
so that the landing package flips it in its own file. The element modules `hex20`, `beam`, `shell` also hold the kind-specific
data the dispatch of D2 delegates to (empty until their package): `reference_nodes()`, `faces()`, `shape_gradients(xi)`,
`fit_basis(xi)`, and `element() -> &'static dyn Element` (the placeholder of D4). `rcfd-post/src/lib.rs` gains `pub mod
solid_dynamic;` and `pub mod solid_fsi;`, one module comment each (ADR 0303, 0304).

### D2. Element kinds (c2)

`ElementKind` gains `Hex20`, `Beam2`, `Shell4` (20, 2, 4 nodes; 8, 2, 4 corners). New methods: `is_solid()` (Tet4, Hex8, Tet10,
Hex20) and `pending() -> Option<&'static str>` (None for the basic kinds; the stub module's `PENDING` for the new ones). Arms:
`name` "Hex20", "Beam2", "Shell4"; `corners` 8, 2, 4; `face_corners` 4, 0, 0; `edges` the twelve hexahedron edges, `[[0, 1]]`,
the four quadrilateral edges; `faces`, `reference_nodes`, `shape_gradients` from the stub modules (empty until the package);
`element_volume` NaN for a pending kind (documented: undefined until its package). Hex20 node order, declared for the output
mapping: corners 0 to 7 as Hex8, mid-edge nodes 8 to 19 on the edges in the order of the existing `HEX_EDGES` ((0,1), (1,2),
(2,3), (3,0), (4,5), (5,6), (6,7), (7,4), (0,4), (1,5), (2,6), (3,7)), which is VTK type 25's order to my memory (finding 6).
`add_block` keeps every existing check; the `det J` check runs only for solid kinds (for Hex20 it is vacuous while
`hex20::reference_nodes()` is empty). A mesh with only basic kinds runs exactly the code it ran before.

### D3. Property data (c3)

In `mesh.rs`: `SolidFormulation { Full (default), BBar, Incompatible }`; `BeamSection::new(area, i_y, i_z, torsion, shear_y,
shear_z)` (each finite and positive, refused by key otherwise); `BlockProperty { Solid { formulation }, Beam { section,
orientation }, Shell { thickness } }` with constructors `solid`, `beam` (orientation finite, non-zero), `shell` (thickness
finite, positive). `ElementBlock` gains a private `property: Option<BlockProperty>` (None: the default of the kind, i.e. `Full`
for a solid), the accessors `property()` and `formulation()`; `SolidMesh::set_block_property(block, property)` refuses a block
that does not exist and a property that does not fit the kind (Solid on a non-solid kind, Beam on any kind but Beam2, Shell on
any kind but Shell4) as `UnsupportedKind { block, kind, operation }`. All types non-exhaustive, all enums with `name()`.

### D4. Element-trait entry points and the placeholder element (c5)

`Element` gains three provided methods that refuse, with an opaque `ElementState` (a vector of history values, constructors
`empty()` and `new(values)`):

- `geometric_stiffness(&self, coords, stress: &[[f64; 6]]) -> Result<ElementMatrix>` (stress at the points of the stiffness
  rule; refused naming "geometric_stiffness", the kind and "ADR 0298");
- `internal_force(&self, coords, material, displacement, state) -> Result<Vec<f64>>` (refused, "ADR 0300");
- `tangent(&self, coords, material, displacement, state) -> Result<ElementMatrix>` (refused, "ADR 0300").

`element::Pending` (kind, package) implements `Element`: `shape_functions` and `shape_derivatives` return empty vectors,
both rules return `gauss_1` (never used), and `point` is overridden to refuse; since `integration_points`, `stiffness`,
`stiffness_with_rule` and `strain_at` all go through `point`, every fallible entry refuses. `element_for` maps the new kinds to
`hex20::element()`, `beam::element()`, `shell::element()`, which return a `Pending` now.

### D5. Constraint and support data (c4)

In `model.rs`: `Constraint::Linear { terms: Vec<LinearTerm>, value }` with `LinearTerm { node, component, coefficient }`
(`sum a_i u_i = b`, ADR 0296); `Constraint::RigidSet { set, reference }` (a node set following a reference point, ADR 0296);
`Constraint::Cyclic { low, high, axis, origin, sectors }` (two surface sets, ADR 0297); constructors `linear`, `rigid_set`,
`cyclic` and `Constraint::name()`. `ElasticSupport { NodalSpring { set, component, stiffness }, Foundation { set, stiffness } }`
(a spring to ground per node and direction on a node set, N/m; a normal foundation stiffness per area on a surface set, Pa/m),
constructors `nodal_spring`, `foundation`, `name()`. `SolidModel` gains a private `supports` list, `add_support` (validating) and
`supports()`. `add_constraint` validates the new variants: Linear needs terms, each node existing (`MissingNode`, role "linear
constraint") with the component (`MissingComponent`), finite coefficients not all zero and a finite value; RigidSet a node set
and a finite reference; Cyclic two surface sets, a finite non-zero axis, a finite origin, `sectors >= 2`. `add_support`: the set
of the right kind, the component, a finite positive stiffness. Validation is the same pattern as the existing variants and is not
a refusal: the refusal is D6.

### D6. Refusals (c9, c10)

`FemError` gains (appended) `NotYetSupported { item, variant, location, package }`, displayed as "`{item} {variant}
({location}) is not supported yet: it is added by {package}`". Each new variant is refused in exactly one place:

| Variant | Refused by | item / variant / location (registered strings) | package |
|---|---|---|---|
| kind Hex20, Beam2, Shell4 | `SolidModel::check_elements_supported` | "element kind" / name / "block {b}" | 0292, 0294, 0295 |
| formulation BBar, Incompatible | same | "formulation" / name / "block {b}" | 0293 |
| Linear | `constraint::prescribed_equations` | "constraint" / "Linear" / "constraint {i}, first node {n}" | 0296 |
| RigidSet | same | "constraint" / "RigidSet" / "constraint {i}, node set '{set}'" | 0296 |
| Cyclic | same | "constraint" / "Cyclic" / "constraint {i}, surface sets '{low}' and '{high}'" | 0297 |
| NodalSpring | `SolidModel::check_supported` | "support" / "NodalSpring" / "support {i}, node set '{set}'" | 0296 |
| Foundation | same | "support" / "Foundation" / "support {i}, surface set '{set}'" | 0296 |

A Linear constraint names no set: its location is its index in the model's list and its first node (the interpretation of "the
set" for this variant, open point 3). `check_supported` = `check_elements_supported` then the supports. Call sites, each one
line: `solve_static_report` after `model.validate()`; `modal_analysis_with` after `model.validate()` (so that the modal analysis
does not silently ignore a support or a formulation either); `stiffness_with` (so that `assemble::stiffness` does not compute a
B-bar block as full integration). `assemble_with` refuses nothing new: the later packages assemble through it. A model of basic
kinds, default properties, displacement constraints and no support passes all three checks without changing any number.

### D7. Output mapping (c1 for rcfd-post, the stub's VTK clause)

`rcfd-io/src/vtk.rs` gains `VTK_LINE = 3`, `VTK_QUAD = 9`, `VTK_QUADRATIC_HEXAHEDRON = 25`, admitted by `vtk_cell_nodes` (2, 4,
20 nodes); the refusal message lists the admitted types. `rcfd-post::vtk_from_fem` maps Hex20 to (25, identity of 20), Beam2 to
(3, (0, 1)), Shell4 to (9, (0, 1, 2, 3)).

### D8. Files, and what stays

Touched beyond the stub's list (finding 2): `rcfd-fem/src/error.rs` (one variant appended), `constraint.rs` (three arms of
`prescribed_equations`), `solve.rs`, `modal.rs` (one call each), `recover.rs` (arms of `fit_basis` and `extrapolation_matrix`:
the new kinds delegate to their modules or give an empty matrix). New test files `si_scaffold.rs`, `si_scaffold_bits.rs`
(rcfd-fem) and `si_scaffold_output.rs` (rcfd-post).

`assemble_with`, `stiffness_with`'s arithmetic, `DofMap::for_mesh` (three unknowns per node; ADR 0294 changes it),
`prescribed_equations` for `Displacement`, the solvers, recovery, loads, mass: unchanged. `solve_static_with` keeps per-node
prescribed values and forces as arguments.

## Registered checks

Fast checks in `crates/rcfd-fem/tests/si_scaffold.rs` and the bit record in `crates/rcfd-fem/tests/si_scaffold_bits.rs`, release
profile, each run once (`cargo test -p rcfd-fem --release --test si_scaffold --test si_scaffold_bits -- --nocapture
--test-threads=1`); the VTK checks in `crates/rcfd-post/tests/si_scaffold_output.rs`.

**Bit record procedure (c7).** `si_scaffold_bits.rs` uses only the API of main ee449e26. Before any library code it is run on
a `git archive` copy of main ee449e26 in `w49i-work/f388-scratch` (own target directory) and prints one FNV-1a 64 digest of the
`to_bits()` of each quantity (in order, with the count) for each model; the printed digests are written into the file as
constants and committed before the library code. After the change the same file runs in the worktree and compares.

Models (all E = 2e11, nu = 0.3, rho = 7850, direct solver unless stated):

- **P (patch)**: `hex_box([0.1, -0.2, 0.3], [1.2, 1.0, 0.8], [3, 3, 3])` (Hex8) and its Tet10 form (`tet_box` + `upgrade_to_tet10`,
  same box, [2, 2, 2]); every boundary node (any of the six `BOX_FACES` sets) prescribed (per node, `solve_static_with`) with
  `u = 1e-3 (x + 2y - z, 0.5x - y + 3z, -x + y + 0.7z)`. Quantities: displacement, reactions, integration-point stress.
- **C (cantilever)**: `hex_box([0,0,0], [1.0, 0.1, 0.1], [20, 2, 2])` Hex8, `xmin` clamped (three constraints), `Load::nodal_force
  ("xmax", (0, 0, -100))`; solved by the direct solver with threads 1 and by `PreconditionedCg { BlockJacobi, 1e-12, 5000 }`.
  Quantities: displacement, reactions, integration-point stress, nodal-average stress (`nodal_average`, region None).
- **B (bar modes)**: `hex_box([0,0,0], [1.0, 0.1, 0.1], [10, 1, 1])` Hex8 and the Tet10 bar (`tet_box` [6, 1, 1] + upgrade),
  `xmin` clamped; `modal_analysis(ModalOptions::new(4))`. Quantities: eigenvalues, frequencies, modes.
- **K (stiffness)**: the assembled `stiffness` of P (both kinds) and C: `row_ptr`, `col`, `val`.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| B1 | c7 | P, two kinds: displacement, reactions, stress digests equal main's | equal | D8: no arithmetic changes on these paths |
| B2 | c7 | C direct and C PCG: displacement, reactions, stress, nodal stress digests equal main's | equal | D8 |
| B3 | c7 | B, two kinds: eigenvalue, frequency, mode digests equal main's | equal | D8 |
| B4 | c7 | K: pattern and value digests equal main's | equal | D8 |
| B5 | c7, D3 | C with `BlockProperty::solid(Full)` set explicitly on its block: displacement bit-equal to C without it | equal | the default is Full |
| H1 | c8 | P (Hex8, Tet10), C, and a Tet4 box (`tet_box`, [2, 2, 2]): `assemble_with(model, Some(t), \|ctx\| element_for(ctx.kind).stiffness(ctx.coords, &D))` against `stiffness(model)` and `stiffness_with(model, Some(t))`, t = 1 and 8: `row_ptr`, `col` equal, every value `to_bits` equal | equal | finding 3: the same function |
| S1 | c6 | `cargo test --release` of every `sb_*` test file of rcfd-fem (17), rcfd-linalg (3), rcfd-io (1), rcfd-post (4); rcfd-api has none | 0 failed, 0 files edited (`git diff --stat main -- '*/tests/sb_*'` empty) | finding 1 |
| S2 | c6 | the other test files of rcfd-fem (`f470_defaults`, `si_amg`, `si_amg_quadratic`) and the unit tests of rcfd-fem, rcfd-io, rcfd-post, release | 0 failed | they cover lines of `solve.rs`, `assemble.rs` |
| S3 | c6 | `cargo check --workspace --all-targets` | no error | finding 2 |
| X1 | c1..c5 | `si_scaffold` compiles against every item of D1 to D5 by path (twelve fem modules with their `PENDING` values `Some("ADR 0292")`, ... as in D1; the two post modules; the three kinds with node counts 20, 2, 4; the property, constraint and support constructors; the three entry points) | compiles; each `PENDING` equals the ADR of ADR 0290 section 2.2 | D1 |
| X2 | c5 | `geometric_stiffness`, `internal_force`, `tangent` of Tet4, Hex8, Tet10 on their reference element: 9 calls, each `Err(NotYetSupported)` with variant the entry name, location containing the kind name, package "ADR 0298" / "ADR 0300" | as stated | D4 |
| X3 | c2, c10 | `element_for(Hex20 / Beam2 / Shell4)`: `stiffness`, `integration_points` with `gauss_2`, `strain_at` each `Err(NotYetSupported)` naming the kind | as stated | D4 |
| R1 | c9 | static solve of a model whose only block is Hex20 (a 2 x 1 x 1 m box of one element, 20 distinct nodes: the 8 corners and the 12 edge mid-points, material on "all", `xmin` nodes fixed): `Err(NotYetSupported { item "element kind", variant "Hex20", location "block 0", package "ADR 0292" })`, its message contains "Hex20" and "block 0" | as stated | D6 |
| R2 | c9 | a Hex8 box [2, 1, 1] plus a Beam2 block (two of its nodes) as block 1: variant "Beam2", location "block 1", "ADR 0294" | as stated | D6 |
| R3 | c9 | the same with a Shell4 block on the four `xmax` nodes: "Shell4", "block 1", "ADR 0295" | as stated | D6 |
| R4 | c9 | the Hex8 cantilever C with `BlockProperty::solid(BBar)`: "formulation" / "BBar" / "block 0" / "ADR 0293"; R4b the same with `Incompatible` | as stated | D6 |
| R5 | c9 | C plus `Constraint::linear([(node 5, comp 0, 1.0), (node 7, comp 0, -1.0)], 0.0)` as constraint 3: "constraint" / "Linear" / "constraint 3, first node 5" / "ADR 0296" | as stated | D6 |
| R6 | c9 | C plus `Constraint::rigid_set("xmax", (1, 0.05, 0.05))`: "RigidSet" / "constraint 3, node set 'xmax'" / "ADR 0296" | as stated | D6 |
| R7 | c9 | C with surface sets "low" (`ymin` faces) and "high" (`ymax` faces) plus `Constraint::cyclic("low", "high", (1, 0, 0), (0, 0, 0), 12)`: "Cyclic" / "constraint 3, surface sets 'low' and 'high'" / "ADR 0297" | as stated | D6 |
| R8 | c9 | C plus `ElasticSupport::nodal_spring("xmax", 2, 1e6)`: "support" / "NodalSpring" / "support 0, node set 'xmax'" / "ADR 0296" | as stated | D6 |
| R9 | c9 | C plus `ElasticSupport::foundation("zmin_faces", 1e9)` (surface set of the `zmin` faces): "Foundation" / "support 0, surface set 'zmin_faces'" / "ADR 0296" | as stated | D6 |
| R10 | c10 | R1 to R9 through `solve_static_with` with the CG solver and with `threads(8)`: the same error; and through `solve_static_report` | same error | D6 call site |
| R11 | c10 | `modal_analysis` of the models of R4 and R8: the same error; `assemble::stiffness` of R4's model: the same error | as stated | D6 |
| V1 | D3, D5 | refused data: `set_block_property` with Shell on a Hex8 block, Beam on a Hex8 block, Solid on a Beam2 block (`UnsupportedKind` naming the block), block 9 of a one-block mesh; `BeamSection::new` with area 0; `BlockProperty::shell(-1)`; Linear with no term, with node n + 7 (`MissingNode` naming it), with all coefficients 0; Cyclic with `sectors` 1; RigidSet on an unknown set; a NodalSpring with stiffness 0 and component 3 | each `Err` | D3, D5 |
| O1 | D7 | `vtk_from_fem(Hex20) = (25, 0..20)`, `(Beam2) = (3, [0, 1])`, `(Shell4) = (9, [0, 1, 2, 3])`; `vtk_cell_nodes(3, 9, 25) = 2, 4, 20`; `fe_vtu_string` accepts one block of each and writes types "3", "9", "25" in `types`; type 42 still refused | as stated | D7 |
| G1 | c11 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes; file unchanged | D8 |

**Negative controls** (each must hold, else the check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | B1's digest of the Hex8 patch displacement with one entry replaced by its `next_up` value (`f64::from_bits(bits + 1)`) | differs from main's |
| N2 | H1's comparison with the hook's element stiffness multiplied by `1 + 2^-52` | reports unequal |
| N3 | C (Hex8, default property, three displacement constraints, no support) through `solve_static`, `modal_analysis`, `stiffness`: no refusal | Ok |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f388-scratch`, own target directory; each applied alone
by exact text replacement, the named test files run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `check_elements_supported`: the kind test skipped | R1 |
| M2 | `check_elements_supported`: the formulation test skipped | R4 |
| M3 | `prescribed_equations`: the Linear arm returns nothing (constraint ignored) | R5 |
| M4 | `check_supported`: the support loop skipped | R8 |
| M5 | `hex20::element()` returns `&Hex8` instead of the placeholder | X3 |
| M6 | `vtk_cell_nodes`: 25 gives 8 nodes | O1 |
| M7 | `Element::geometric_stiffness` default returns `Ok(ElementMatrix::zeros(3 n))` | X2 |
| M8 | `stiffness_with`: the check removed | R11 |
| M9 | `modal_analysis_with`: the check removed | R11 |
| M10 | `assemble_with`: the results of a batch scattered in reverse element order | B4 (and B1 to B3) |
| M11 | `solve_static_report`: the check removed | R8 |

## Decision rule

**Done** if every check meets its limit, every negative control holds and every mutation is caught by its registered check on
the first run; **partial** if a check misses or is not run, or a mutation survives; **todo** if the scaffold does not compile or
a basic-tier bit changes and cannot be repaired without changing a default.

## Limits (what is not covered)

- No numerics: no element, formulation, constraint or support computes anything; their packages do (0292 to 0302).
- The Hex20 node order and the VTK type-25 order are declared from memory; ADR 0292 confirms them.
- `DofMap` stays three unknowns per node; ADR 0294 extends it.
- The two `rcfd-post` modules are empty; `solid_coupling.rs` and the static participant are unchanged.
- `rcfd-api`: no `sb_` file exists on main, so c6 is checked on four crates; nothing in `rcfd-api` is touched.
- Loads, recovery and mass of a pending kind refuse through the placeholder element (D4), but only the static solve, the modal
  analysis and `stiffness` are registered checks of the refusal by data (R1 to R11).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The generic hook: `assemble_with` of ADR 0275 is re-used, no new function (ADR 0275 open question 3, option a). Taken on
   the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. Where the block property lives: a private optional field of `ElementBlock` set by `SolidMesh::set_block_property`, not a side
   structure (finding 1). Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
3. "Naming the set" for a Linear constraint, which names no set: its index and its first node. Taken on the maintainer's
   standing instruction of 2026-11-11 (take the recommendation).
4. The modal analysis and `assemble::stiffness` also refuse (beyond the acceptance's static solve), so that nothing silently
   ignores a support or a formulation. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-1 of the structural solver, intermediate tier. Size S.

### Goal

Prepare the shared files once, so that every later package of the tier fills a file of its own and no two packages of a wave edit
the same file. Declare the new modules of `rcfd-fem` (section 2.2 of ADR 0290) and of `rcfd-post` as stubs; add the element
kinds `Hex20`, `Beam2` and `Shell4` and the per-block property data (Hex8 formulation, beam section, shell thickness); add the
constraint and support data (linear constraint, rigid set, cyclic pair, elastic support); give the `Element` trait the defaulted
entry points `geometric_stiffness`, `internal_force` and `tangent`; add one generic assembly hook that turns a per-element
symmetric matrix into a `CsrMatrix`; admit the VTK cell types 3, 9 and 25 in the output mapping. Everything new is refused with a
message naming it until its package lands. No numerics.

### Dependencies

none in this list; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): F-370, F-372, F-374, F-377.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. The modules, element kinds, property data, constraint and
support data and the three defaulted element entry points of ADR 0290 section 2 exist in rcfd-fem and rcfd-post; every test of
the basic tier (the sb_ files of rcfd-fem, rcfd-linalg, rcfd-io, rcfd-post and rcfd-api) compiles and passes unedited, and a
registered set of basic models (patch test, cantilever, bar modes) gives displacements, stresses and frequencies with the same
bits as before the change; the generic assembly hook called with the element stiffness reproduces assemble::stiffness bit for bit
for 1 and 8 threads; each new element kind, formulation, constraint and support variant is refused by the static solve with an
error naming the variant and the set, never ignored; the baseline hashes are unchanged.

### Reference to check against

No physics: the reference is the basic tier itself (bit identity) and the refusals. The model for a declare-once scaffold is ADR
0271.

### Files

`crates/rcfd-fem/src/{lib.rs, mesh.rs, model.rs, element.rs, assemble.rs}` (additions only), one-line stubs of `hex20.rs`,
`hex8x.rs`, `beam.rs`, `shell.rs`, `mpc.rs`, `cyclic.rs`, `geometric.rs`, `buckling.rs`, `rotating.rs`, `nonlinear.rs`,
`transient.rs`, `harmonic.rs`; `crates/rcfd-post/src/lib.rs` (two `pub mod` lines), stubs `solid_dynamic.rs`, `solid_fsi.rs`, the
kind-to-cell-type mapping in `solid_output.rs`; `crates/rcfd-io/src/vtk.rs` only if the finite-element writer of ADR 0278
restricts the cell types; `crates/rcfd-fem/tests/si_scaffold.rs`.

### What could make it miss

The basic tier's types are not written yet. If its tests build `SolidModel` or the element blocks by struct literals, a new field
breaks them and house rule 2 forbids editing them: the new data then go into a side structure passed to the new entry points,
which is uglier. The lead should ask the implementer of ADR 0271 to build the model through constructors (section 2.4 of ADR
0290).

## Amendment 1 (2026-11-13, written and committed before the corrected record run)

The first record run of `si_scaffold_bits.rs` on the archive of main ee449e26 (the step that writes the digests, before any
library code) stopped at the PCG solve of C: `cg-block-jacobi` did not reach the tolerance 1e-12 (true residual 2.4e-9 after
529 iterations and 3 restarts; the slender cantilever, L/h = 10, is too ill-conditioned for 1e-12 with block Jacobi). A defective
set-up of the record, not a check against a limit. Corrected: the PCG of C uses `tolerance: 1e-8, max_iterations: 20000`. The
digests printed for P and C-direct by the first run are not used; the whole record is made again once with the corrected file,
and those digests become `MAIN`. Nothing else changes.

## Addendum 1: results (2026-11-13)

Runs, each once, release profile: the bit record on the archive of main ee449e26 (amendment 1, then the corrected record);
`cargo test -p rcfd-fem --release --test si_scaffold --test si_scaffold_bits -- --nocapture --test-threads=1` and `cargo test -p
rcfd-post --release --test si_scaffold_output -- --nocapture --test-threads=1` in the tree (commit 12ebe2c6); then S1 to S3 and
G1; then the mutations on a `git archive` copy of 12ebe2c6 with its own target directory.

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| B1 | P (Hex8, Tet10): displacement, reactions, stress digests equal main's | 6 of 6 equal | yes |
| B2 | C direct and PCG: displacement, reactions, stress, nodal stress equal | 8 of 8 equal | yes |
| B3 | B (Hex8, Tet10): eigenvalues, frequencies, modes equal | 6 of 6 equal | yes |
| B4 | K of P (two kinds) and C: pattern and values equal | 6 of 6 equal | yes |
| B5 | explicit `Solid(Full)` gives C's displacement bit for bit | bit-equal | yes |
| H1 | `assemble_with` with the element stiffness = `stiffness` = `stiffness_with(t)`, t = 1, 8, four models | bit-equal in all 8 cases (2007 to 26901 entries) | yes |
| S1 | every `sb_` file of rcfd-fem (17), rcfd-linalg (3), rcfd-io (1), rcfd-post (4) passes; none edited | 0 failed; `git diff ee449e26 -- 'crates/*/tests/sb_*'` empty; rcfd-api has no `sb_` file | yes (four crates) |
| S2 | all tests of rcfd-fem, the unit tests of rcfd-io and rcfd-post | rcfd-fem 125 passed, 24 ignored (records); rcfd-io 28; rcfd-post 52, 1 ignored; rcfd-linalg `sb_` 35, 2 ignored; 0 failed | yes |
| S3 | `cargo check --workspace --all-targets` | no error | yes |
| X1 | every item of D1 to D5 by path; `PENDING` = ADR 0292, 0293, 0294, 0295, 0296, 0297 | compiles, all equal; post modules exist | yes |
| X2 | 9 default entry points refuse naming entry, kind, package | 9 of 9 | yes |
| X3 | placeholder of Hex20, Beam2, Shell4: stiffness, integration points, strain refuse naming the kind | 9 of 9 | yes |
| R1 to R9 | static solve refuses each variant with the registered item, variant, location, package | 10 of 10 (R4b included), strings exact | yes |
| R10 | the same error by CG, with `threads(8)` and per-node values, and by `solve_static_report` | 30 of 30 | yes |
| R11 | modal of R4 and R8, `stiffness` of R4: the same error | 3 of 3 | yes |
| V1 | 13 invalid data refused | 13 of 13 (`MissingNode` names node n + 7 = 196) | yes |
| O1 | mapping (25, 0..20), (3, [0, 1]), (9, [0..4]); nodes 2, 4, 20; types 3, 9, 25 written; 42 refused | as stated | yes |
| G1 | `baseline_hashes` passes, file unchanged | 3 passed, 1 ignored (its existing record); file unchanged | yes |

Negative controls: N1 one ulp changes the digest (0xb63760e800d5386b to 0xb63ca3c50967aefa); N2 the hook with `1 + 2^-52`
reports unequal; N3 C is accepted by `solve_static`, `modal_analysis` and `stiffness`. All hold.

Mutations (each alone, first run):

| Id | Caught by | Result |
| --- | --- | --- |
| M1 kind test skipped | R1 to R9, R10 (R1: the placeholder's refusal arrives wrapped as `Element`, not the registered error) | caught |
| M2 formulation test skipped | R1 to R9, R10, R11 | caught |
| M3 Linear arm ignores | R1 to R9 (R5), R10 | caught |
| M4 support loop skipped | R1 to R9 (R8, R9), R10, R11 | caught |
| M5 `hex20::element()` is Hex8 | X3 | caught |
| M6 type 25 has 8 nodes | O1 | caught |
| M7 `geometric_stiffness` returns zeros | X2 | caught |
| M8 `stiffness_with` unchecked | R11 | caught |
| M9 `modal_analysis_with` unchecked | R11 (R8's support; R4 is still refused by `stiffness_with`) | caught |
| M10 batch scattered in reverse | B1 to B4 (every digest after the first Hex8 patch values differs) | caught |
| M11 `solve_static_report` unchecked | R1 to R9 (R8, R9), R10 | caught |

fmt: `cargo fmt -p rcfd-fem --check`, `-p rcfd-post`, `-p rcfd-io`: clean. clippy: `cargo clippy -p rcfd-fem -p rcfd-post -p
rcfd-io --all-targets -- -D warnings`: clean after one test-only change (below).

**Deviations.** (1) Amendment 1: the record's PCG tolerance (record step, before any library code). (2) After the runs, clippy
refused `vec_init_then_push` in the list of refusal cases of `si_scaffold.rs`; an `#[allow]` was added (commit c960bbce), no
change of behaviour, the checks were not re-run for it. (3) N1 also ran on the archive of main as part of the record runs, and
the `si_scaffold` tests ran again inside S2's full rcfd-fem suite; same outcomes, nothing selected. (4) c6 names rcfd-api, which
has no `sb_` file on main ee449e26; nothing in rcfd-api is touched and G1 passes. (5) Files beyond the stub's list, as D8
registered: `error.rs`, `constraint.rs` (lines 38 to 46, 54, 64 to 89), `solve.rs` (line 166), `modal.rs` (line 122),
`recover.rs` (lines 231 to 232, 292 to 296, 301 to 302), `assemble.rs` (lines 182 to 185); `rcfd-io/src/vtk.rs` (lines 364
to 371, 378 to 380, 486).

**Not run.** The whole workspace test suite (the lead's gate). No golden file, hash table or roadmap file was touched.

**Decision by the registered rule: done.** Every check met its limit, every negative control holds, and all eleven mutations
were caught on their first run.

**Remaining questions** (each with options and a recommendation):

1. The landing packages' file lists. ADR 0292's stub lists only `hex20.rs`; with this scaffold that holds for the element, the
   faces, the gradients, the volume and the fit basis (all delegated), and for the static solve (it flips `hex20::PENDING`).
   ADR 0293 must also dispatch on `ElementBlock::formulation()` in `assemble::stiffness_with` and in the recovery, which this
   package does not delegate. Options: (a) ADR 0293 adds `assemble.rs` and `recover.rs` to its file list (no other package of its
   wave edits them); (b) a further scaffold step delegates the stiffness per block to `hex8x`. Recommendation: (a).
2. The Hex20 node order and VTK type 25 are declared from memory. Options: (a) ADR 0292 confirms them from the public VTK
   documentation before its code; (b) treat them as fixed. Recommendation: (a).
3. `SolidMesh::element_volume` is NaN for Beam2 and Shell4 (no section in a mesh-level function). Options: (a) ADR 0294 and
   0295 compute it from the block property (one arm each in `mesh.rs`); (b) leave NaN and give the volume of structural elements
   in their own modules. Recommendation: (a), when those packages land.

## For the lead: roadmap entries

Proposed status of F-388: **done** (pending the maintainer's acceptance).

Note for F-388:

> ADR 0291, 2026-11-13: the intermediate tier's scaffold. rcfd-fem declares hex20, hex8x, beam, shell, mpc, cyclic, geometric, buckling, rotating, nonlinear, transient, harmonic and rcfd-post solid_dynamic, solid_fsi; ElementKind gains Hex20, Beam2, Shell4 (is_solid, pending; their data and element delegated to their module, a placeholder element whose every fallible entry refuses); BlockProperty (Solid with SolidFormulation Full, BBar, Incompatible; Beam with BeamSection and orientation; Shell with thickness) set by SolidMesh::set_block_property; Constraint::Linear, RigidSet, Cyclic and ElasticSupport (NodalSpring, Foundation) with SolidModel::add_support, all validated when added; Element::geometric_stiffness, internal_force, tangent refusing by default; the generic hook is ADR 0275's assemble_with. Each new kind, formulation, constraint and support is refused by the static solve (also by the modal analysis and assemble::stiffness) with FemError::NotYetSupported naming the variant, its block or set and the package, every string exact (10 cases, 30 more paths); VTK cell types 3, 9, 25 admitted and mapped. Patch test, cantilever (direct and PCG) and bar modes give displacements, reactions, stresses, nodal stresses, eigenvalues, frequencies, modes and stiffness with the bits of main ee449e26 (26 digests equal); assemble_with with the element stiffness equals assemble::stiffness bit for bit for 1 and 8 threads on four models; every sb_ test of rcfd-fem, rcfd-linalg, rcfd-io, rcfd-post passes unedited (rcfd-api has none); baseline hashes unchanged. Eleven mutations, all caught.

Note for ADR 0292's item (Hex20), to append:

> ADR 0291 (2026-11-13): hex20.rs holds PENDING, element(), reference_nodes(), faces(), shape_gradients(), fit_basis() and volume(), to which mesh.rs, element.rs and recover.rs delegate; landing Hex20 is filling these and setting PENDING to None in hex20.rs. The node order (corners as Hex8, mid-edge nodes on the edges in HEX_EDGES order) is declared as VTK type 25's and mapped by identity in solid_output.rs; confirm it.

Note for ADR 0293's item (Hex8 formulations), to append:

> ADR 0291 (2026-11-13): the formulation is ElementBlock::formulation() (set by SolidMesh::set_block_property with BlockProperty::solid); hex8x::PENDING gates it. The stiffness and the recovery do not delegate to hex8x: the package needs assemble.rs (stiffness_with) and recover.rs in its file list.

Note for ADR 0296's and ADR 0297's items, to append:

> ADR 0291 (2026-11-13): Constraint::Linear { terms: Vec<LinearTerm>, value }, Constraint::RigidSet { set, reference }, Constraint::Cyclic { low, high, axis, origin, sectors } and ElasticSupport::{NodalSpring, Foundation} exist and are validated when added; constraints are refused in constraint::prescribed_equations (gated by mpc::PENDING and cyclic::PENDING), supports in SolidModel::check_supported.
