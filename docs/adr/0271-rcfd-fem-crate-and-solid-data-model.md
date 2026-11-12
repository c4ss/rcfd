# ADR 0271: Crate rcfd-fem: the data model of a solid model and the structured builders

Status: **REGISTERED 2026-11-12, written before any code of F-370 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-370 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand (closed
forms of a structured grid and the moment conditions of one tetrahedral rule); no script and no library code was run. Results
go in the addendum at the end; nothing above the addendum is changed after a run.

Work package SB-1 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0271). Roadmap item F-370 (depends on F-001, F-010, F-012). Size M. Number: 0271, fixed by the plan; the file name
of the planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as written"). Branch
`f370-rcfd-fem-scaffold` from main e1a08ce4, worktree `w49i-work/f370`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: nothing existing changes. A new crate is added; the only edits of existing files are the workspace
  `Cargo.toml` (one member line, one `[workspace.dependencies]` line), `Cargo.lock` (the new package only), ONE line of
  `crates/rcfd-cli/tests/workspace_graph.rs` (approved by the maintainer in person on 2026-11-12, ADR 0270 decision record, M1)
  and `docs/architecture.md`; no solver file, no golden, no hash table;
- no new third-party crate, no unsafe code (the crate inherits the workspace lints, `unsafe_code = "forbid"`), the crate graph
  stays acyclic;
- clean-room: everything below is geometry derived from first principles (isoparametric maps, the Kuhn split of a cube, Gauss
  quadrature, one tetrahedral rule whose moments are verified below by hand); sources are cited from memory and nothing of them
  is used beyond what is derived here.

## What F-370 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. A new crate rcfd-fem (dependencies rcfd-core, rcfd-mesh,
> rcfd-linalg only) holds the solid data model; the workspace graph test passes with the one registered line for the new
> crate, the architecture page lists it, and nothing existing changes (baseline hashes unchanged). On a box of n^3 hexahedra
> and on its six-tetrahedra split, for n = 1, 2, 5 with unequal edge lengths: every element has a positive Jacobian at every
> node, the element volumes sum to the box volume within 1e-12 relative, the area of each of the six face sets equals the box
> face within 1e-12, and every interior face is shared by exactly two elements; the quadratic upgrade has (nodes + edges) nodes
> and each mid-edge node is the mean of its two ends within 1e-15; an inverted element, a node repeated in an element, an
> empty set, E <= 0 and a Poisson ratio outside (-1, 0.5) are each refused with an error naming the element or the key.

Clauses: (a1) a crate `rcfd-fem` with the dependencies `rcfd-core`, `rcfd-mesh`, `rcfd-linalg` and no other; (a2) it holds the
solid data model; (a3) the workspace graph test passes with the one registered line; (a4) the architecture page lists the
crate; (a5) nothing existing changes, baseline hashes unchanged; for the hexahedron box and its six-tetrahedra split, n = 1, 2,
5, unequal edge lengths: (a6) positive Jacobian at every node of every element; (a7) element volumes sum to the box volume
within 1e-12 relative; (a8) the area of each of the six face sets equals the box face within 1e-12; (a9) every interior face is
shared by exactly two elements; (a10) the quadratic upgrade has (nodes + edges) nodes; (a11) each mid-edge node is the mean of
its two ends within 1e-15; refused with an error naming the element or the key: (a12) an inverted element, (a13) a node
repeated in an element, (a14) an empty set, (a15) E <= 0, (a16) a Poisson ratio outside (-1, 0.5).

The brief and the plan add: `lib.rs` declares every module of the basic tier (ADR 0270 section 2.3) so that no later ADR edits
it; the data model leaves room for ADRs 0272 to 0288, 0291 and 0311; the request appended to the stub (ADR 0290): tests build
models through constructors, not struct literals, so that a field added later breaks nothing.

## What was read (main e1a08ce4, read only)

1. `AGENT_RULES.md`; ADR 0270 in full (section 2.2 conventions: SI, `DofMap`, tensor order xx yy zz xy yz zx, outward
   normals, determinism, located refusals; 2.3 the module table; 2.4 the data model; 2.5 the interfaces; the decision record
   of 2026-11-12: M1 (a) and the name `rcfd-fem`); this ADR's stub; the stubs 0272 to 0288 through their entries in ADR 0270
   section 3 (what each needs from the model: 0272 patches as surface sets with outward normals and zones as element sets, a
   projection of mid-edge nodes; 0273 the Tet10 node order matching this upgrade; 0275 constraints per node and component;
   0276 loads on surface sets, gravity, centrifugal; 0277 integration-point values in the order of 2.2; 0278 the VTK cell
   types 10, 12, 24; 0281 the temperature field; 0284 density; 0286 the nodes and faces of a surface set; 0287 loads and
   constraints readable from a file); ADR 0290 sections 2.2 to 2.4 (Hex20, Beam2, Shell4 to come; a property per element
   block; constraint variants; `DofMap` with three or six unknowns per node; the constructor request); ADR 0291 and ADR 0311
   (material variants with a point context and a state size come later; additions only to `mesh.rs`, `model.rs`); ADRs 0274
   and 0283 (house style; their modules `rcfd_linalg::skyline` and `eigen` exist).
2. Code: `Cargo.toml` (members, `[workspace.dependencies]`, `[workspace.lints]`: `unsafe_code = "forbid"`, `missing_docs =
   "warn"`, clippy `all`); `crates/rcfd-linalg/Cargo.toml` and `rcfd-mesh/Cargo.toml` (`[lints] workspace = true`);
   `crates/rcfd-cli/tests/workspace_graph.rs` (the layer table, panics on an unknown name); `docs/architecture.md`;
   `rcfd-core/src/lib.rs`, `error.rs` (`Error` with `Mesh`, `InvalidValue { path, reason }`), `vec3.rs` (`Vec3 = [f64; 3]`
   and helpers); `rcfd-mesh/src/tetmesh.rs` (`TetMesh { points, cells: Vec<[usize; 4]> }`, "positively oriented
   tetrahedra (`orient3d(a,b,c,d) > 0`)"); `rcfd-mesh/src/gen.rs` (box patches named `xmin, xmax, ymin, ymax, zmin, zmax`).
3. **Not read:** the bodies of the stubs 0272 to 0288 beyond ADR 0270's entries (identical text by the plan); `rcfd_mesh::Mesh`
   internals (ADR 0272's business).

## Arithmetic before this registration (disclosed, by hand)

1. **Counts of the boxes** (n cells per direction, used as exact expected values): nodes `(n+1)^3`; hexahedra `n^3`; boundary
   quadrilaterals `6 n^2`; interior quadrilaterals `3 n^2 (n - 1)` (planes `x = const` interior: `n - 1` of them with `n^2`
   faces, three directions). Tetrahedra `6 n^3`; boundary triangles `12 n^2`; interior triangles `(4 x 6 n^3 - 12 n^2) / 2 =
   12 n^3 - 6 n^2`. Edges of the Kuhn split (D3): axis edges `3 n (n+1)^2`, one diagonal per grid square `3 n^2 (n+1)`, one
   body diagonal per cube `n^3`: `E(n) = 3 n (n+1)^2 + 3 n^2 (n+1) + n^3`. Check by Euler's relation for one cube: `V - E + F -
   T = 8 - 19 + 18 - 6 = 1` (with `F = 12 + 6`). Values: `E(1) = 19`, `E(2) = 98`, `E(5) = 1205`; Tet10 nodes `8 + 19 = 27`,
   `27 + 98 = 125`, `216 + 1205 = 1421`.
2. **The degree-3 rule for the quadratic tetrahedron's volume (D6).** Centroid weight `w0 = -4/5`, four points with
   barycentric coordinates a permutation of `(1/2, 1/6, 1/6, 1/6)`, weight `w1 = 9/20` each (fractions of the volume). Exact
   barycentric moments over a tetrahedron (as a fraction of its volume): `int L1^a L2^b L3^c / V = 3! a! b! c! / (a+b+c+3)!`.
   Checked by hand for every symmetric class up to degree 3: `1`: `w0 + 4 w1 = 1`; `L1^2`: `-4/5 x 1/16 + 9/20 x 1/3 = 1/10`
   (exact `2 x 6/120`); `L1 L2`: `-1/20 + 9/20 x 2/9 = 1/20` (exact `6/120`); `L1^3`: `-1/80 + 9/20 x 30/216 = 1/20` (exact
   `36/720`); `L1^2 L2`: `-1/80 + 9/20 x 14/216 = 1/60` (exact `12/720`); `L1 L2 L3`: `-1/80 + 9/20 x 10/216 = 1/120` (exact
   `6/720`). Degree-1 moments hold by symmetry. The rule is therefore exact for every cubic polynomial; recalled from memory as
   Keast's 5-point rule (P. Keast, Comput. Methods Appl. Mech. Engrg 55 (1986) 339), used only as verified here.
3. **Rounding of the volume and area sums** (limits K2, K3). Each element volume is a sum of at most 8 products of three
   coordinate differences: relative error `<= ~10 eps = 2.2e-15`. Recursive summation of `N` positive terms adds at most `(N -
   1) eps` relative: `N = 750` (tetrahedra, n = 5) gives `8.3e-14`. Coordinates are `x0 + L (i / n)` (D3), each within one
   ulp, the box extent within `2 eps` relative. Worst total `~1e-13`, typical `~1e-15`; the limit 1e-12 has a margin of 10 on
   the worst case. The same for the face areas (`N <= 50` faces, `<= 6e-15`).
4. **Mid-edge node** (limit Q2). The node is computed as `0.5 (a + b)` component-wise (D5) and the test recomputes the same
   expression: expected difference exactly 0; the limit 1e-15 (absolute, coordinates of magnitude at most 3.4) catches any
   other placement (a quarter point is `0.25 |b - a| >= 0.03` away).

## Design

### D1. The crate

`crates/rcfd-fem`, `[lints] workspace = true`, `publish = false`, dependencies `rcfd-core`, `rcfd-mesh`, `rcfd-linalg` (the
latter two used from ADR 0272 and 0275 on; declared now so that later packages do not edit the manifest). No third-party
dependency, not even the workspace's `serde` or `thiserror`: the acceptance says "dependencies rcfd-core, rcfd-mesh,
rcfd-linalg only", so the error type implements `Display` and `std::error::Error` by hand, and file reading (ADR 0287) maps the
plain data in `rcfd-api`, which already has serde. Layer 2 of `docs/architecture.md` (beside `rcfd-gpu`, `rcfd-dist`); the
graph test gets the line `"rcfd-fem" => 2` in the arm of layer 2. The workspace `[workspace.dependencies]` gains `rcfd-fem = {
path = "crates/rcfd-fem" }` so that `rcfd-post` and `rcfd-api` use it as the others (ADR 0278, 0287).

`lib.rs` declares every module of ADR 0270 section 2.3, and one more, `error`:

| Module | Here | Filled by |
| --- | --- | --- |
| `error` | `FemError`, `Result`, `SetKind`, conversion to `rcfd_core::Error` | 0271 |
| `mesh` | `SolidMesh`, `ElementKind`, `ElementBlock`, `FaceRef`, sets, validation, geometry | 0271 |
| `model` | `Material`, `Constraint`, `Load`, `SolidModel`, `SolidResult`, `SolverStats`, `DofMap` | 0271 |
| `builders` | `hex_box`, `tet_box`, `upgrade_to_tet10` | 0271 |
| `convert`, `element`, `quadrature`, `material`, `assemble`, `constraint`, `solve`, `loads`, `recover`, `thermal`, `mass`, `modal` | one-line stubs (a module doc naming the ADR that fills it) | 0272 to 0284 |

### D2. Element kinds, node order, local faces (fixed here for all tiers)

`ElementKind` is `#[non_exhaustive]` with `Tet4`, `Hex8`, `Tet10` (ADR 0290 adds `Hex20`, `Beam2`, `Shell4`; ADR 0311 `Shell3`).
Each kind gives its node count, its corner count, its local faces (local node lists) and the reference coordinates of its
nodes.

* **Hex8**: reference cube `[-1, 1]^3`; node order 0 `(-,-,-)`, 1 `(+,-,-)`, 2 `(+,+,-)`, 3 `(-,+,-)`, 4 to 7 the same at
  `zeta = +1` (the order of VTK cell type 12, recalled from memory; ADR 0278 confirms it). Faces with nodes counter-clockwise
  seen from outside (right-hand normal outward): 0 `(0,3,2,1)` at `zeta = -1`, 1 `(4,5,6,7)` at `zeta = +1`, 2 `(0,1,5,4)` at
  `eta = -1`, 3 `(1,2,6,5)` at `xi = +1`, 4 `(2,3,7,6)` at `eta = +1`, 5 `(3,0,4,7)` at `xi = -1`. Each checked by hand on the
  unit cube: `(p1 - p0) x (p2 - p0)` is `-e_z, +e_z, -e_y, +e_x, +e_y, -e_x`.
* **Tet4**: positive orientation `det(x1 - x0, x2 - x0, x3 - x0) > 0` (as `TetMesh`). Faces outward: 0 `(0,2,1)` (opposite 3), 1
  `(0,1,3)` (opposite 2), 2 `(1,2,3)` (opposite 0), 3 `(0,3,2)` (opposite 1); checked on the reference tetrahedron (normal of
  face 2 is `(1,1,1)`, the opposite node lies behind it).
* **Tet10**: corners 0 to 3 as Tet4, mid-edge nodes 4 `(0,1)`, 5 `(1,2)`, 6 `(0,2)`, 7 `(0,3)`, 8 `(1,3)`, 9 `(2,3)` (the order of
  VTK cell type 24, recalled from memory, UNVERIFIED: ADR 0278 confirms it from the public file-format document; ADR 0273 uses
  this order). Faces: the corners of Tet4's face, then the mid-edge nodes of its edges (c0 c1), (c1 c2), (c2 c0): 0
  `(0,2,1,6,5,4)`, 1 `(0,1,3,4,8,7)`, 2 `(1,2,3,5,9,8)`, 3 `(0,3,2,7,9,6)`.

Elements are numbered globally across blocks in block order. A face of the mesh is `FaceRef { element, face }`.

### D3. `SolidMesh`

Private fields (no struct literal outside the crate): `nodes: Vec<Vec3>`, `blocks: Vec<ElementBlock>` (`ElementBlock { kind,
connectivity: Vec<usize> }`, flat, private fields with accessors; ADR 0291 adds the block property beside them),
`node_sets`, `element_sets`, `surface_sets` as `BTreeMap<String, Vec<_>>` (deterministic order). Constructors:
`SolidMesh::new(nodes)`, `add_block(kind, connectivity) -> Result<usize>`, `add_node_set`, `add_element_set`,
`add_surface_set`. Validation, each refusal a `FemError` naming the element, node, block or set:

* nodes finite (`NonFiniteNode { node }`);
* a block: not empty, length a multiple of the node count; per element: every node index in range (`NodeOutOfRange`), no node
  twice (`RepeatedNode { element, node }`, checked before the Jacobian), and `det J > 0` at every node of the element
  (`InvertedElement { element, local_node, jacobian }`; refused when `det J <= 0` or not finite; no size threshold: element
  quality is not this ADR's);
* a set: non-empty name, not already defined for its kind (`DuplicateSet`), not empty (`EmptySet { kind, name }`), members in
  range, no member twice, a surface member's face index below the kind's face count.

Geometry (for the checks and for later packages): `jacobians_at_nodes(element)`, `element_volume(element)`, `face_nodes(FaceRef)`,
`face_area(FaceRef)`, `face_area_vector(FaceRef)` (the integral of `x_u x x_v`, outward), `surface_set_nodes(name)` (sorted,
unique), `face_incidence()` (map from the sorted corner nodes of a face to the `FaceRef`s that have it). The Jacobian `J =
sum_a x_a (dN_a/dxi)^T`:

* Hex8: trilinear `N_a = (1 + xi xi_a)(1 + eta eta_a)(1 + zeta zeta_a)/8`, evaluated at the eight corners; the volume `int det J`
  by 2 x 2 x 2 Gauss (points `+-1/sqrt(3)`, weights 1), exact because `det J` of a trilinear map has degree at most 2 in each
  variable (each column of `J` is of degree 1 in the two other variables) and the rule is exact to degree 3 in each.
* Tet4: `det J = det(x1 - x0, x2 - x0, x3 - x0)`, volume `det J / 6`.
* Tet10: `N_i = L_i (2 L_i - 1)` at the corners, `N_ij = 4 L_i L_j` at the mid-edge nodes, `L_0 = 1 - xi - eta - zeta`;
  `det J` at the ten nodes; volume by the rule of D6 (`det J` is cubic).
* Faces: Quad4 bilinear, area by 2 x 2 Gauss of `|x_u x x_v|` (exact on a plane parallelogram, the box case; an approximation
  on a warped face); the area vector by the same rule (exact: the integrand is of degree 1 in each variable). Tri3: half the
  cross product. Tri6: the 3-point rule at `(1/6, 1/6), (2/3, 1/6), (1/6, 2/3)`, weights 1/6 (degree 2; exact for straight
  faces, where the integrand is constant).

### D4. The boxes

`hex_box(origin, lengths, divisions)` and `tet_box(origin, lengths, divisions)`: nodes `x0 + L_k (i_k / n_k)` (the far face
exactly at `x0 + L`), numbered `i + (n_x + 1)(j + (n_y + 1) k)`; one block; element set `all`; for each face of the box a
surface set and a node set named `xmin, xmax, ymin, ymax, zmin, zmax` (the names of `rcfd_mesh::gen` box patches), plus the
node set `all`. Lengths finite and positive, divisions at least 1 (refused naming the key `lengths` or `divisions`).

The six-tetrahedra split is the Kuhn (Freudenthal) split along the cube diagonal from `(0,0,0)` to `(1,1,1)`: one tetrahedron
per permutation `p` of the three axes, vertices `v0 = c`, `v1 = v0 + e_p1`, `v2 = v1 + e_p2`, `v3 = v2 + e_p3`. Its
orientation is `det(e_p1, e_p1 + e_p2, e_p1 + e_p2 + e_p3) = det(e_p1, e_p2, e_p3) = sign(p)` (times the positive cell
lengths), so for an odd permutation `v1` and `v2` are swapped. It is conforming between neighbouring cubes: every square of
the grid is cut along the diagonal through its lowest-index corner (the face of a cube in the plane `x_k = const` is cut by
the diagonal that increases both other coordinates), the same from both sides. Permutations in lexicographic order: `(0,1,2),
(0,2,1), (1,0,2), (1,2,0), (2,0,1), (2,1,0)`. Boundary faces of a tetrahedron that lie on a box face are found as the local
face whose three nodes all lie on that box face (by grid index, not by coordinate).

`upgrade_to_tet10(&SolidMesh) -> Result<(SolidMesh, Vec<MidEdgeNode>)>` (`MidEdgeNode { node, ends: [usize; 2] }`, for the
projection of ADR 0272): every block must be Tet4 (another kind is refused naming the block and the kind; Hex20 is ADR 0292).
Edges are visited in element order and local edge order (D2) and numbered at first appearance by the key `(min, max)`; the
node of edge `k` is `nodes.len() + k` at `0.5 (x_a + x_b)` component-wise. Element and surface sets keep their members
(elements keep their numbers, a local face keeps its number); a node set gains the mid-edge node of an edge whose two ends are
both in it. The new mesh passes the full validation of D3.

### D5. The model (`model.rs`)

All structs have private fields or are `#[non_exhaustive]`; enums and their variants are `#[non_exhaustive]`; every value is
made through a constructor that validates:

* **`DofMap`**: offsets per node; `uniform(n_nodes, per_node)`, `from_counts(&[usize])`, `for_mesh(&SolidMesh)` (three
  translations per node for the kinds of this tier; ADR 0294 makes it six where a beam or shell is attached),
  `equation(node, component) -> Option<usize>`, `components(node)`, `n_equations()`, `node_component(eq)`. No other code of the
  crate assumes three unknowns per node.
* **`Material`**: `IsotropicElastic { e, nu, density, expansion }` by `Material::isotropic_elastic(e, nu, density,
  expansion) -> Result`: refused `InvalidMaterial { key, value, reason }` with key `e` when `!(e > 0)` or not finite, `nu`
  when not in the open interval `(-1, 0.5)` (also NaN), `density` when negative or not finite, `expansion` when not finite.
  Accessors `youngs_modulus()`, `poisson_ratio()`, `density()`, `expansion()`. The constitutive call is ADR 0273's.
* **`Constraint`**: `Displacement { set, component, value }` by `Constraint::displacement(set, component, value)`.
* **`Load`**: `NodalForce { set, force }` (force on each node of a node set), `Traction { set, traction }`, `Pressure { set,
  pressure }` (surface sets; positive pressing on the solid), `Gravity { acceleration }`, `Centrifugal { axis, origin, omega }`,
  `BodyForceNodal { values }` (force per volume at each node), `Temperature { nodal, reference }`; one constructor each. Plain
  data: no closures (ADR 0270 2.4).
* **`SolidModel`**: `SolidModel::new(mesh)`, `add_material(element_set, material)`, `add_constraint(c)`, `add_load(l)` (each
  checks the sets it names exist with the right kind, components against `DofMap::for_mesh` for every node of the set, values
  finite, a centrifugal axis non-zero, nodal fields of the node count) and `validate()` (every element has exactly one
  material: `MaterialUnassigned { element }`, `MaterialOverlap { element, first, second }`); `material_of(element)`.
* **`SolidResult`** and **`SolverStats`**: `SolidResult::new(dofs, values)` (length `n_equations`), `with_reactions(values)`,
  `with_integration_points(offsets, strain, stress)` (six components xx yy zz xy yz zx, engineering shear strains, ADR 0270 2.2;
  offsets per element, monotone, last equal to the point count), `with_stats(SolverStats)`; `displacement(node)` returns the
  node's values through the `DofMap`.

### D6. Error type

`FemError` (`#[non_exhaustive]`), `Display` with the located noun first, for example "element 5: inverted (det J = -0.0546 at
local node 0)", "element 3: node 12 appears twice", "node set 'xmin' is empty", "material key 'nu' = 0.5: Poisson ratio must
lie in the open interval (-1, 0.5)". `From<FemError> for rcfd_core::Error` gives `Error::Mesh` for mesh errors and
`Error::InvalidValue { path: key, .. }` for value errors, so callers in `rcfd-api` can use the workspace error.

## Set-ups

- Boxes: origin `(0.25, -0.5, 1.0)`, lengths `(1.3, 0.7, 2.1)` (unequal), divisions `[n, n, n]` for n = 1, 2, 5; `hex_box`,
  `tet_box`, and `upgrade_to_tet10(tet_box)`. Box volume `1.3 x 0.7 x 2.1`, face areas `0.7 x 2.1` (x faces), `1.3 x 2.1` (y),
  `1.3 x 0.7` (z), computed in the test.
- Refusal fixtures: built from the box nodes through the public constructors (`SolidMesh::new`, `add_block`): `tet_box` n = 2
  connectivity with element 5's local nodes 1 and 2 swapped; `hex_box` n = 1 with the bottom and top node quadruples swapped;
  element 3 of the `tet_box` n = 2 connectivity with its local node 3 replaced by its local node 0; a Tet10 element (of the
  upgraded n = 1 box) whose mid-edge node 4 is moved to `x0 + 1.5 (x1 - x0)` (beyond the edge, det J < 0 at a corner).

## Registered checks

All in `crates/rcfd-fem/tests/sb_model.rs`, release profile, one run.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| G1 | a1, a3 | `cargo test -p rcfd-cli --release --test workspace_graph`: all four tests pass, with the one added line | pass | layer 2 above core, mesh, linalg (layers 0, 1) |
| G2 | a1 | `cargo tree -p rcfd-fem -e normal --depth 1`: exactly `rcfd-core`, `rcfd-mesh`, `rcfd-linalg` | as stated | D1 |
| G3 | a5 | `cargo test -p rcfd-api --release --test baseline_hashes` unedited: passes | pass | no existing source changes |
| G4 | a4, a5 | `git diff main --stat`: existing files touched are only `Cargo.toml` (2 lines), `Cargo.lock` (the new package and nothing else), `workspace_graph.rs` (1 line), `docs/architecture.md`, this ADR; the architecture page lists `rcfd-fem -> core, mesh, linalg` and layer 2 | as stated | D1 |
| G5 | rule 5 | the crate's manifest has `[lints] workspace = true`; no `unsafe` in its sources; mutation M10 shows the forbid applies | as stated | D1 |
| K1 | a6 | each box (hex, tet; n = 1, 2, 5): `min` over elements and nodes of `det J` | `> 0` | hex `h_x h_y h_z / 8`, tet `h_x h_y h_z` |
| K2 | a7 | `|sum V_e - L_x L_y L_z| / (L_x L_y L_z)` | `<= 1e-12` | arithmetic 3: worst 1e-13 |
| K3 | a8 | for each of the six surface sets: `|sum A_f - A_box| / A_box`; face counts `n^2` (hex) and `2 n^2` (tet) | `<= 1e-12`; as stated | arithmetic 3 |
| K3b | D2 (extension) | for each face of each set, `|a_f / |a_f| - n_box|` (area vector against the box face's outward normal) | `<= 1e-14` | exact up to rounding of a cross product of axis-aligned vectors |
| K4 | a9 | `face_incidence()`: every face key has 1 or 2 members; the keys with 1 are exactly the faces of the six sets (`6 n^2` hex, `12 n^2` tet); keys with 2: `3 n^2 (n - 1)` hex, `12 n^3 - 6 n^2` tet; the two members of a shared face list its corners in opposite cyclic order | as stated | arithmetic 1 |
| K5 | a6 (counts) | nodes `(n+1)^3`; elements `n^3` and `6 n^3` | as stated | arithmetic 1 |
| Q1 | a10 | upgrade of `tet_box` n = 1, 2, 5: node count `= (n+1)^3 + E(n)` (closed form) and `=` nodes plus the distinct edges counted by the test from the Tet4 connectivity; `MidEdgeNode` count `E(n)` | as stated | arithmetic 1: 27, 125, 1421 |
| Q2 | a11 | every element, every local edge `(a, b)` of D2: `max_k |x_m,k - 0.5 (x_a,k + x_b,k)|` for its node `m`; the same for every `MidEdgeNode` | `<= 1e-15` | arithmetic 4: exactly 0 |
| Q3 | D3 on Tet10 (extension) | the upgraded boxes: K1, K2, K3 and K4 with the Tet10 geometry (6-node faces, keys from corners) | as K1 to K4 | D3, D6; Tri6 exact on straight faces |
| Q4 | D4 (extension) | `upgrade_to_tet10(hex_box)`: refused naming block 0 and `Hex8` | as stated | D4 |
| R1 | a12 | the inverted Tet4 element 5: `Err(InvertedElement { element: 5, .. })`, message contains "element 5"; the inverted hexahedron: element 0; the Tet10 with the moved mid-edge node: element 0 | as stated | det J sign flips (D3) |
| R2 | a13 | element 3 with a repeated node: `Err(RepeatedNode { element: 3, .. })`, message contains "element 3" | as stated | checked before the Jacobian (D3) |
| R3 | a14 | an empty node set, element set and surface set, each named `empty_set`: `Err(EmptySet { kind, name: "empty_set" })`, message contains `'empty_set'` | as stated | D3 |
| R4 | a15 | `isotropic_elastic` with `e` = 0, -1, NaN, +inf: `Err(InvalidMaterial { key: "e", .. })`, message contains `'e'`; control `e = 1e-300` accepted | as stated | D5 |
| R5 | a16 | `nu` = -1, 0.5, -1.5, 0.7, NaN: `Err(InvalidMaterial { key: "nu", .. })`, message contains `'nu'`; controls -0.999, 0, 0.499 accepted | as stated | D5, open interval |
| R6 | D3, D5 (extension) | refused and located: a node index out of range (element named), a connectivity of wrong length (block named), a non-finite node, a duplicate set name, a surface face index 6 on a Hex8, a negative density (`density`), a material on an unknown element set (set named), an element without material and one with two (element named), a constraint component 3 (node and set named), a nodal force on an unknown set, a centrifugal axis of zero (`axis`), a temperature field of wrong length (`nodal`), a `SolidResult` of wrong length; controls: a valid model of `hex_box` n = 2 with material, constraints on `xmin` and every load kind validates | as stated | D3, D5 |
| D1 | D5 (extension) | `DofMap::uniform(4, 3)`: `equation(n, c) = 3 n + c`, `equation(3, 3) = None`, `equation(4, 0) = None`; `from_counts(&[3, 6, 3])`: offsets 0, 3, 9, `n_equations = 12`, `node_component(5) = (1, 2)`; `for_mesh(hex_box n = 1)` equals `uniform(8, 3)` | as stated | D5 |

**Negative controls** inside the checks: the accepted materials of R4 and R5 (the refusal is the bound, not a blanket
refusal); the valid model of R6; Q4.

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f370-scratch`, own target directory; each applied
alone by exact text replacement, `sb_model` run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | Kuhn split: the swap for odd permutations removed | K1 (the builder's own validation refuses: the box tests fail) |
| M2 | Hex8 face 5 listed as `(3,7,4,0)` (inward) | K3b, K4 (orientation) |
| M3 | Jacobian refusal `det <= 0` replaced by `det < -1e300` | R1 |
| M4 | repeated-node check removed | R2 (the error becomes `InvertedElement`) |
| M5 | Poisson bound `nu >= 0.5` replaced by `nu > 0.5` | R5 |
| M6 | edge key `(a, b)` not sorted | Q1 |
| M7 | Tet10 local edges 5 and 6 swapped in the edge table | Q2 |
| M8 | centroid weight of D6 `-4/5` replaced by `-3/4` | Q3 (volume) |
| M9 | empty-set check removed | R3 |
| M10 | `fn _m() { unsafe {} }` appended to `lib.rs` | G5: the crate does not compile (`unsafe_code` forbidden) |

**Decision rule.** F-370 is proposed **done** (pending the maintainer's acceptance) if G1 to G5, K1 to K5, Q1, Q2 and R1 to
R5 are met for every box and n (every acceptance clause) and every mutation M1 to M7, M9, M10 fails its test. K3b, Q3, Q4, R6
and D1 are registered extensions: a miss among them is a `*_known_miss` record and a follow-up item and does not alone block
done, except that M8 is then uncaught, which makes the result partial. A miss or an unrun check among the done set, or an
uncaught mutation (after checking that the test covers it): **partial**, with the miss named. No crate that builds: **todo**.

## Limits (what is not covered)

- No solver, no element matrices, no loads computed: data and geometry only. The constitutive call, quadrature for stiffness
  and mass and the `Element` trait are ADR 0273.
- Element quality: only the sign of `det J` at the nodes is checked; a positive Jacobian at the nodes does not prove it
  positive inside a Tet10 or a warped Hex8 (the extremum of a polynomial can lie inside). Slivers are accepted.
- Face areas of warped bilinear faces and curved Tri6 faces are quadrature approximations (D3); the boxes have planar faces.
- Node sets after the upgrade follow the both-ends rule (D4); a node set that is not the node set of a surface set may gain a
  node of an interior edge whose two ends are in it. Constraints on a face should use `surface_set_nodes`.
- The VTK node orders of D2 are recalled from memory; ADR 0278 confirms them, and if Tet10 differs, the mapping goes into the
  writer, not into this order.
- Plain data is not serialisable here (no serde, D1); ADR 0287 maps it.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. A crate-local error type with hand-written `Display`, no `thiserror` or `serde` dependency (the acceptance names the three
   dependencies), with a conversion to `rcfd_core::Error` (D1, D6).
2. A module `error` beside the table of ADR 0270 2.3 (declared here, so no later ADR edits `lib.rs` for it).
3. Node and face orders of D2 (VTK order for Hex8 and Tet10), the Kuhn diagonal of D4, the set names of the box patches.
4. The refusal threshold `det J <= 0` with no relative size test (D3).
5. Density 0 is accepted (a static model needs none); negative is refused.

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-1 of the structural solver, basic tier. Size M.

### Goal

Create the crate `rcfd-fem` (depends on `rcfd-core`, `rcfd-mesh`, `rcfd-linalg` only) with the data model that the whole tier
shares, as validated data without any solver: `SolidMesh` (nodes, element blocks of kind Tet4, Hex8, Tet10, node, element and
surface sets), `DofMap`, `Material` (isotropic linear elastic), `Constraint`, `Load`, `SolidModel`, `SolidResult`. Add the
builders that need nothing else: a box of hexahedra, the same box in tetrahedra (six per hexahedron, conforming), and the
upgrade of linear to quadratic tetrahedra by mid-edge nodes. `lib.rs` declares every module of the tier (section 2.3 of ADR
0270) so that each later ADR fills its own file and none edits `lib.rs`.

### Dependencies

none in this list; existing roadmap items: F-001, F-010, F-012.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A new crate rcfd-fem (dependencies rcfd-core, rcfd-mesh,
rcfd-linalg only) holds the solid data model; the workspace graph test passes with the one registered line for the new crate,
the architecture page lists it, and nothing existing changes (baseline hashes unchanged). On a box of n^3 hexahedra and on its
six-tetrahedra split, for n = 1, 2, 5 with unequal edge lengths: every element has a positive Jacobian at every node, the
element volumes sum to the box volume within 1e-12 relative, the area of each of the six face sets equals the box face within
1e-12, and every interior face is shared by exactly two elements; the quadratic upgrade has (nodes + edges) nodes and each
mid-edge node is the mean of its two ends within 1e-15; an inverted element, a node repeated in an element, an empty set, E <= 0
and a Poisson ratio outside (-1, 0.5) are each refused with an error naming the element or the key.

### Reference to check against

Geometry only: volume and face areas of a box, the edge count of a structured grid (derivable). No published benchmark.

### Files

`Cargo.toml` (members, workspace.dependencies), `Cargo.lock` (the new workspace member only), `crates/rcfd-fem/Cargo.toml`,
`crates/rcfd-fem/src/{lib.rs, mesh.rs, model.rs, builders.rs}` and one-line stubs of the other modules,
`crates/rcfd-fem/tests/sb_model.rs`, `docs/architecture.md`, and ONE line of the existing test
`crates/rcfd-cli/tests/workspace_graph.rs` (the layer of the new crate; needs the maintainer, decision M1).

### What could make it miss

The edit of the existing graph test is not approved (the crate then panics that test). A data model too narrow for the next
tier: three unknowns per node hard-coded instead of going through `DofMap`, or loads as closures that cannot be read from a
file.

### Request from the intermediate-tier plan (ADR 0290, 2026-11-12)

The intermediate tier adds fields to the model data (ADR 0290, scaffold ADR 0291). Build models in this ADR's tests through constructors, not struct literals, so that a later field does not break them.
