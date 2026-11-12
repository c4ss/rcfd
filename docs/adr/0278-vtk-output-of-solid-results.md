# ADR 0278: VTK output of a solid result: finite-element cells with point and cell data

Status: **REGISTERED 2026-11-12, written before any code of F-377 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-377 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script
and no library code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Work package SB-8 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0278). Roadmap item F-377 (depends on F-376 and F-070). Size S. Number 0278, fixed by the plan; the file name of
the planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as written").
Branch `f377-vtk-solid-output` from main f0a5a6a3, worktree `w49i-work/f377`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: the existing writers of `crates/rcfd-io/src/vtk.rs` are not edited (new items are inserted before
  the file's test module; no existing line changes); no file of `rcfd-fem` is touched (another item, F-383, works there at
  the same time); no golden, no hash table, no shared solver file is touched;
- no new third-party crate; `rcfd-post` gains the workspace crate `rcfd-fem` as a dependency (layer 4 on layer 2, which the
  layer table of `crates/rcfd-cli/tests/workspace_graph.rs` permits without an edit); no unsafe code, the graph stays acyclic;
- clean-room: the cell type numbers, the node orders and the XML layout come from the public VTK documentation pages cited
  in D2 (read for this ADR on 2026-11-12, quoted below) and from the repository's existing writer; no VTK source code was
  read.

## What F-377 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. rcfd-io gains a writer of an unstructured grid of finite
> elements from plain arrays (VTK cell types 10, 12 and 24, point data and cell data); the existing writers are unchanged and
> their tests pass unedited. rcfd-post writes a SolidResult through it: displacement at the nodes, nodal von Mises and principal
> stresses, the six stress components per element, reactions. A test parses the file back: counts, offsets and cell types are
> consistent, every value has the bits that were written, and a box under uniform strain read back has displacement eps x at
> every node within 1e-15; a field of the wrong length is refused by name.

Clauses: (c1) a writer in `rcfd-io` from plain arrays, cell types 10, 12, 24, point data and cell data; (c2) the existing
writers unchanged, their tests pass unedited; (c3) `rcfd-post` writes a `SolidResult`: displacement at the nodes, (c4) nodal
von Mises and principal stresses, (c5) the six stress components per element, (c6) reactions; (c7) a test parses the file
back and counts, offsets and cell types are consistent; (c8) every value has the bits that were written; (c9) a box under
uniform strain read back has displacement `eps x` at every node within 1e-15; (c10) a field of the wrong length is refused by
name.

The brief adds: (b1) confirm the Hex8 and Tet10 node orders of ADR 0271 D2 (recalled there as VTK's without a check) from a
citable source and map them in the writer if one differs; (b2) the crate graph stays valid without editing its test; (b3) the
existing tests of `rcfd-io`, `rcfd-post` and `rcfd-fem` pass unedited (release); `baseline_hashes` is run because `rcfd-io`
and `rcfd-post` are touched.

## What was read (main f0a5a6a3, read only)

1. `AGENT_RULES.md`; ADR 0270 sections 2.1 (where the code lives, the layer table), 3 (the entry of 0278 and the stubs of
   0285 to 0287 that rely on this ADR's modules), 5 (the dependency notes); this ADR's stub; ADR 0271 D1, D2 (node orders and
   local faces), its remaining question 1 and its roadmap sentence for F-377; ADR 0273 (the order is taken as fixed); ADR 0275
   (the patch-test set-up and its measured interior error, P1: 4.4e-16, 6.1e-16, 1.2e-15 relative on perturbed 4 x 3 x 3
   boxes); ADR 0277 (its recovery API and its roadmap sentence for F-377, remaining questions 2 and 3).
2. Code: `crates/rcfd-io/src/vtk.rs` in full (the `.vtu` writer: XML, ASCII, `{:?}` shortest round-trip formatting,
   `DataArray` with `type`, `Name`, `NumberOfComponents`, `format="ascii"`, `offsets` as the end of each cell, `Int64`
   connectivity, `UInt8` types; all cells written as polyhedra, type 42, with their points sorted, so the existing writer
   fixes no node order of types 10, 12, 24); `crates/rcfd-io/Cargo.toml`; `crates/rcfd-post/{Cargo.toml, src/lib.rs}`;
   `crates/rcfd-fem/src/{mesh.rs (kinds, reference nodes, HEX_EDGES, TET_EDGES, faces), model.rs (SolidResult), recover.rs
   (public API), error.rs (FemError, the conversion into rcfd_core::Error), solve.rs (solve_static_with returns reactions),
   builders.rs (hex_box, tet_box, upgrade_to_tet10)}`; `crates/rcfd-fem/tests/sb_static.rs` (patch helpers);
   `crates/rcfd-cli/tests/workspace_graph.rs` (layer table: `rcfd-fem` 2, `rcfd-post` 4; a dependency must have a lower
   layer); `docs/architecture.md`.
3. Public VTK documentation (Kitware), read on 2026-11-12 through a fetch tool that returns quotations of the page text:
   - `https://vtk.org/doc/nightly/html/classvtkTetra.html`: "The tetrahedron is defined by the four points (0-3); where
     (0,1,2) is the base of the tetrahedron which, using the right hand rule, forms a triangle whose normal points in the
     direction of the fourth point."
   - `https://vtk.org/doc/nightly/html/classvtkHexahedron.html`: "The hexahedron is defined by the eight points (0-7) where
     (0,1,2,3) is the base of the hexahedron which, using the right hand rule, forms a quadrilaterial whose normal points in
     the direction of the opposite face (4,5,6,7)." The page does not list which top point lies over which base point.
   - `https://vtk.org/doc/nightly/html/classvtkQuadraticHexahedron.html`: "point ids 0-7 are the eight corner vertices of the
     cube; followed by twelve midedge nodes (8-19). Note that these midedge nodes correspond lie on the edges defined by
     (0,1), (1,2), (2,3), (3,0), (4,5), (5,6), (6,7), (7,4), (0,4), (1,5), (2,6), (3,7)."
   - `https://vtk.org/doc/nightly/html/classvtkQuadraticTetra.html`: "The ordering of the ten points defining the cell is
     point ids (0-3,4-9) where ids 0-3 are the four tetra vertices; and point ids 4-9 are the midedge nodes between (0,1),
     (1,2), (2,0), (0,3), (1,3), and (2,3)."
   - `https://docs.vtk.org/en/latest/vtk_file_formats/vtkxml_file_format.html`: the `Piece` of an `UnstructuredGrid` holds
     `PointData`, `CellData`, `Points`, `Cells`; "The second array specifies the offset into the connectivity array for the
     end of each cell"; `NumberOfComponents` is "The number of components per value in the array".
   - The legacy-format page and the VTK textbook chapters 5 and 8 were also opened; their node orders are in figures, which
     the tool does not read, so nothing is taken from them. The type numbers 10, 12, 24 are taken from the acceptance and
     the plan; the legacy page's example uses types 10 and 12 without naming them. They are VTK_TETRA = 10,
     VTK_HEXAHEDRON = 12, VTK_QUADRATIC_TETRA = 24, recalled (from memory, as in ADR 0271), consistent with the class names
     above; no page read here prints the table `vtkCellType.h`, so the numbers remain a recalled fact checked by no reader
     (see Limits).
4. **Not read:** any VTK or ParaView source code (including `vtkCellType.h` and the VTK examples); any other writer of
   finite-element results.

## Arithmetic before this registration (disclosed, by hand)

1. **Tet4 orientation.** ADR 0271 D2: `det(x1 - x0, x2 - x0, x3 - x0) > 0`, that is `((x1 - x0) x (x2 - x0)) . (x3 - x0) >
   0`: the right-hand normal of the base `(0,1,2)` points towards point 3. This is the statement of the vtkTetra page.
   Identical, no mapping.
2. **Hex8.** ADR 0271 D2 reference nodes: 0 `(-,-,-)`, 1 `(+,-,-)`, 2 `(+,+,-)`, 3 `(-,+,-)`, 4 to 7 the same at `+1`. The
   base loop 0, 1, 2, 3 runs counter-clockwise seen from `+zeta`: `(p1 - p0) x (p2 - p0) = (2,0,0) x (2,2,0) = (0,0,4)`,
   towards the face 4 to 7 (vtkHexahedron's statement). The vertical edges of rcfd-fem's `HEX_EDGES` are `(0,4), (1,5),
   (2,6), (3,7)` and its twelve edges are exactly the twelve pairs listed in the vtkQuadraticHexahedron sentence, in the same
   order. Reading: the corner numbering of VTK's quadratic hexahedron is its linear hexahedron's (the page calls them "the
   eight corner vertices"), so the twelve listed pairs are the edges of a VTK hexahedron, which fixes 4 over 0, 5 over 1 and
   so on; together with the base orientation this determines the order up to the choice of the first node, which carries no
   meaning for a viewer. Identical, no mapping. Disclosed: the link between the linear and quadratic pages is this reading,
   not a sentence of the linear page.
3. **Tet10.** rcfd-fem's `TET_EDGES`: mid-edge node `4 + k` on `(0,1), (1,2), (0,2), (0,3), (1,3), (2,3)`. VTK: `(0,1), (1,2),
   (2,0), (0,3), (1,3), (2,3)`. The third pair is the same unordered edge. Identical, no mapping. So ADR 0271's recalled
   orders are confirmed by the cited pages; the writer still carries a per-kind permutation table (the identity for all
   three) so that a later kind with a different order (Hex20, ADR 0292) is mapped in one place, and the node-order checks K1
   to K3 below read the documented pairs, not rcfd-fem's tables.
4. **Uniform-strain box, size of the error.** `u = eps x` with `|eps_ij| <= 1e-3` and `|x| <~ 1.6` gives `max |u| <~ 3e-3`.
   On an unperturbed 2 x 2 x 2 box (Hex8 27 nodes, 1 interior; Tet4 48 cells, 27 nodes, 1 interior; Tet10 125 nodes, the
   number not on the box faces counted by the test and reported, not registered) the solve gives the interior nodes to a few
   units of roundoff relative (ADR 0275 P1 measured at most 1.2e-15 relative on a larger, perturbed patch), so an absolute
   error `<~ 1.2e-15 * 3e-3 = 3.6e-18`; boundary nodes are prescribed and come back bit for bit (ADR 0275 P2). The file adds
   nothing (shortest round-trip formatting is exact, D1). The acceptance's 1e-15 (absolute, in metres; it names no
   normalisation) is met with a margin of about 300; relative to `max |u|` it is `3e-13`.
5. **Stress of the uniform strain.** `sigma = lambda tr(eps) I + 2 mu eps`, `lambda = E nu / ((1 + nu)(1 - 2 nu))`, `mu =
   E / (2 (1 + nu))`; for `E = 2e11, nu = 0.3`: `lambda = 1.1538e11`, `mu = 7.6923e10`. ADR 0277 S1 to S3 measured the
   recovered stresses equal to `D eps` within 3.2e-14 on the perturbed patch; the extension E1 below uses that ADR's limit
   1e-10.
6. **File size.** Per node the solid writer writes 3 coordinates and 3 + 6 + 1 + 3 + 9 + 3 = 25 point values; per element 6
   cell values plus its connectivity. At about 22 characters per value a model of 1e5 nodes is about 60 MB of ASCII. Noted
   under Limits; not a check.

## Design

### D1. The writer in `rcfd-io` (inserted into `vtk.rs` before its test module; nothing existing changes)

```rust
pub const VTK_TETRA: u8 = 10;
pub const VTK_HEXAHEDRON: u8 = 12;
pub const VTK_QUADRATIC_TETRA: u8 = 24;
pub fn vtk_cell_nodes(cell_type: u8) -> Option<usize>;          // 4, 8, 10; None otherwise

pub struct FeCellBlock<'a> { pub cell_type: u8, pub connectivity: &'a [usize] }   // VTK node order, flat
pub struct FeArray<'a> { pub name: &'a str, pub components: usize, pub values: &'a [f64] }  // flat, row-major
pub fn fe_vtu_string(points: &[Vec3], blocks: &[FeCellBlock], point_data: &[FeArray], cell_data: &[FeArray])
    -> Result<String>;
pub fn write_fe_vtu(path, points, blocks, point_data, cell_data) -> Result<()>;
```

The format is the existing writer's: XML `UnstructuredGrid`, version 1.0, `LittleEndian`, ASCII `DataArray`s, every `f64`
written with `{:?}` (Rust's shortest representation that parses back to the same bits), `Int64` connectivity and offsets,
`UInt8` types; the private helper `data_array` of the file is reused as it is. Order inside the `Piece`: `PointData`,
`CellData`, `Points`, `Cells` (the order of the XML page's template; VTK readers accept the order the existing writer uses as
well, so either is valid; the template order is taken). Cells are written block by block in block order; the offsets are
the end of each cell, cumulative. Refusals (each an `rcfd_core::Error::InvalidValue` whose `path` is the offending array's
name or the key `cell_type`, `connectivity`, `points`):

- a cell type other than 10, 12, 24 (`path` "cell_type", the block named in the reason);
- a connectivity length that is not a multiple of the type's node count, or an empty block;
- a node index `>= points.len()` (the block and the cell named);
- an array with `components == 0`, or `values.len() != components * n` with `n` the point count (point data) or the cell
  count (cell data): refused naming the array (`path` = its name, the reason with the length found and expected) (c10);
- a name that is empty, contains one of `< > & " '` (it would break the XML attribute), or repeats a name within the same
  section.

Values are not checked for finiteness (the existing writer does not either); a NaN is written as `NaN`.

### D2. Node order

All three kinds of rcfd-fem are in VTK's order (arithmetic 1 to 3, the pages cited under "What was read" 3). The solid writer
maps each element through a per-kind permutation `VTK_FROM_FEM` (identity for Tet4, Hex8, Tet10): VTK local node `a` of a
cell is rcfd-fem local node `perm[a]`.

### D3. `rcfd-post::solid_output`

```rust
pub struct SolidFields {                 // the arrays written, exposed so a test compares bits
    pub displacement: Vec<f64>,          // 3 per node
    pub stress: Vec<f64>,                // 6 per node: nodal average (Voigt xx yy zz xy yz zx)
    pub von_mises: Vec<f64>,             // 1 per node: von_mises(nodal stress)
    pub principal_stress: Vec<f64>,      // 3 per node: principal(nodal stress).values, descending
    pub principal_directions: Vec<f64>,  // 9 per node: directions 1, 2, 3 (three 3-vectors)
    pub reaction: Option<Vec<f64>>,      // 3 per node, when the result carries reactions
    pub element_stress: Vec<f64>,        // 6 per element: field.element_means(field.stress())
}
pub fn solid_fields(model: &SolidModel, result: &SolidResult) -> Result<SolidFields>;
pub fn solid_vtu_string(model: &SolidModel, result: &SolidResult) -> Result<String>;
pub fn write_solid_vtu(path: impl AsRef<Path>, model: &SolidModel, result: &SolidResult) -> Result<()>;
```

Point arrays: `displacement` (3), `stress` (6), `von_mises` (1), `principal_stress` (3), `principal_direction_1`,
`_2`, `_3` (3 each), `reaction` (3, only when present). Cell array: `stress` (6). The stresses come from ADR 0277 exactly as
its roadmap sentence says: `IntegrationPointField::from_result(model, result)`, `nodal_average(model, &field,
field.stress(), None)`, `field.element_means(field.stress())`; von Mises and principal values of the nodal-averaged tensor
(not averages of invariants). A node of no element has `None` from `nodal_average`; its nodal stress, von Mises and
principal arrays hold NaN. Refusals: a result whose number of unknowns differs from the model's (`displacement` named, the
message of `FemError::FieldLength`), a node with other than 3 components (`displacement` named), a material interface
(ADR 0277's refusal naming the node and both sets, passed through). Errors convert by the existing `From<FemError> for
rcfd_core::Error`.

`rcfd-post/Cargo.toml` gains `rcfd-fem.workspace = true`; `lib.rs` gains `pub mod solid_output; pub mod solid_loads; pub mod
solid_coupling;`, the last two one-line stubs naming ADR 0285 and ADR 0286 (plan section 5: so that those ADRs touch only
their own file). `docs/architecture.md`: the line of `rcfd-post` names `fem`.

## Registered checks

Test files: `crates/rcfd-io/tests/sb_vtk_elements.rs` (writer from plain arrays, its own parser) and
`crates/rcfd-post/tests/sb_solid_output.rs` (the solid writer, its own parser). Both parsers are written in the test files,
independently of the writer: they find each `DataArray` by `Name` inside its section (`PointData`, `CellData`, `Points`,
`Cells`), read the `NumberOfComponents`, `NumberOfPoints`, `NumberOfCells` attributes and split the text on whitespace.
Material `E = 2e11, nu = 0.3, density 7850`. Uniform strain `eps` (tensor, symmetric): `eps_xx = 1.0e-3, eps_yy = -0.5e-3,
eps_zz = 0.8e-3, eps_xy = 0.3e-3, eps_yz = -0.2e-3, eps_zx = 0.4e-3` (tensor shear), `u_i = sum_j eps_ij x_j`. Box origin
`(0.1, -0.2, 0.3)`, lengths `(1.2, 1.0, 0.8)`, divisions 2 x 2 x 2, not perturbed; Hex8 by `hex_box`, Tet4 by `tet_box`, Tet10
by `upgrade_to_tet10` of the Tet4 box; boundary nodes (the six face node sets) prescribed to `eps x`, solved by
`solve_static_with` (direct), which returns reactions.

| Check | Clauses | Quantity | Limit | Estimate |
|---|---|---|---|---|
| W1 | c1, c7 | rcfd-io, a hand-made grid of plain arrays with cells of types 10, 12, 24, 12 (in that order, 30 points, two point arrays of 1 and 3 components, two cell arrays of 1 and 6): `NumberOfPoints`, `NumberOfCells`, the connectivity equal to the given one entry for entry, offsets strictly increasing, equal to the cumulative node counts 4, 12, 22, 30 and the last equal to the connectivity length, types `10 12 24 12`, every index `< NumberOfPoints`, each array's value count equal to components times its count | exact | by construction |
| W2 | c8 | same grid: every point coordinate and every point and cell value read back has the bits written (`to_bits` equal), values including `-0.0`, `1/3`, `0.1`, `1e300`, `-2.5e-310` (subnormal), `f64::MAX`, `f64::MIN_POSITIVE` | exact | `{:?}` round-trips (the existing writer's own test relies on it) |
| W3 | c10 | a point array with one value too few, a cell array with one value too many, a 6-component array of length 6 n + 3: each refused, the error's `path` equal to the array's name and the message containing it | exact | D1 |
| W4 | c1 | refusals: cell type 42 and 13, a connectivity of 9 for type 12, an index equal to the point count, components 0, an empty name, a name with `"`, a repeated name: each refused, the error naming the key or array | exact | D1 |
| W5 | c2 | `git diff main --numstat -- crates/rcfd-io/src/vtk.rs` shows 0 deleted lines; the existing test modules of `rcfd-io` (`cargo test -p rcfd-io --release`) all pass unedited | 0; all pass | D1 |
| P1 | c3 to c6, c8 | rcfd-post, the three boxes: the file has the point arrays `displacement` 3, `stress` 6, `von_mises` 1, `principal_stress` 3, `principal_direction_1..3` 3, `reaction` 3 and the cell array `stress` 6; every value read back bit-equal to `solid_fields`, and `solid_fields` bit-equal to an independent assembly in the test from the library calls (`result.displacement(n)`, `result.reactions()`, `nodal_average`, `von_mises`, `principal`, `element_means`) | exact | D3 |
| P2 | c7 | rcfd-post, the three boxes: counts equal to the mesh's nodes and elements; types all 12, 10, 24 respectively; the connectivity read back equals the mesh's connectivity permuted by D2 (identity) | exact | D2 |
| B1 | c9 | the three boxes: `max_n max_k |u_read(n)_k - (eps x_n)_k|`, `eps x` evaluated in the test from the read-back point coordinates | `<= 1e-15` (absolute) | arithmetic 4: `<~ 4e-18` |
| K1 | b1 | every cell of type 10, and the corners of every cell of type 24, read back from the files of P1: `((x1 - x0) x (x2 - x0)) . (x3 - x0) > 0` (vtkTetra) | all positive | arithmetic 1 |
| K2 | b1 | every cell of type 24: the point at position `4 + k` is the midpoint of the documented pair `(0,1), (1,2), (2,0), (0,3), (1,3), (2,3)` (written into the test from the vtkQuadraticTetra page, not from rcfd-fem's table): `|x_{4+k} - (x_a + x_b)/2| / L_max` | `<= 1e-15` | upgrade at the mean exactly (ADR 0271: limit 1e-15 met) |
| K3 | b1 | every cell of type 12: the twelve documented pairs of vtkQuadraticHexahedron each have the length of one cell spacing `h_x, h_y, h_z` within `1e-14 L_max` (a face or space diagonal is longer by a factor `>= sqrt(2)`), the vertical pairs `(0,4)...(3,7)` are four equal vectors within `1e-14 L_max`, and the base loop's right-hand normal `(x1 - x0) x (x2 - x0) + (x2 - x0) x (x3 - x0)` has a positive product with `centroid(4..7) - centroid(0..3)` (vtkHexahedron) | as stated | arithmetic 2 |
| R1 | c10 | rcfd-post refuses a result of another mesh (the Hex8 result written with the Tet10 model): the error's `path` is `displacement` | exact | D3 |
| E1 | extension | the three boxes: von Mises read back equals `von_mises(D eps)` and element stress read back equals `D eps` (from `lambda`, `mu` in the test, arithmetic 5) within `1e-10` relative to `|D eps|` | `<= 1e-10` | ADR 0277 S: 3.2e-14 |
| G1 | b2 | `cargo test -p rcfd-cli --release --test workspace_graph`, the file unedited | pass | layer 4 on 2 |
| G2 | b3 | `cargo test -p rcfd-fem --release`, `cargo test -p rcfd-post --release`, `cargo test -p rcfd-io --release` (existing tests unedited), `cargo test -p rcfd-api --release --test baseline_hashes` | all pass | nothing shared changes |

Negative controls (in the tests, they must fail the check they control):

- N1: the parser of W1 run on the written text with one offset increased by 1: the consistency check reports it.
- N2: the K2 midpoint test on a cell whose positions 5 and 6 are swapped (the order `(0,1), (0,2), (1,2), ...`): it fails
  (on the boxes the mid-edge nodes of edges 12 and 20 are distinct points).
- N3: B1 evaluated against `eps^T x` with a non-symmetric part added to `eps` (`+1e-4` in xy only): the difference exceeds
  1e-15, so B1 can see a wrong field.

Planned mutations (scratch copy, each run once; the expected catcher in brackets):

| # | Mutation | Caught by |
|---|---|---|
| M1 | `fe_vtu_string`: offsets written as the start of each cell | W1 |
| M2 | the type of a Tet10 block written as 10 | W1 or P2 |
| M3 | `NumberOfPoints` written as `n - 1` | W1 |
| M4 | values written with `{:.15e}` instead of `{:?}` | W2, P1 |
| M5 | the length check of arrays removed | W3 |
| M6 | `VTK_FROM_FEM` of Tet10 with positions 5 and 6 swapped | K2 (and P2) |
| M7 | `VTK_FROM_FEM` of Hex8 with the base reversed `(0,3,2,1, 4,7,6,5)` | K3 (and P2) |
| M8 | element stress from the first integration point instead of the mean | P1 |
| M9 | principal values written ascending | P1 |
| M10 | displacement components written in the order y, x, z | B1, P1 |
| M11 | the reactions array filled from the displacement | P1 |

## Decision rule

**done**: every check W1 to W5, P1, P2, B1, K1 to K3, R1, E1, G1, G2 passes, every mutation is caught. **partial**: a check
of a clause (c1 to c10, b1 to b3) misses or is not run, or a mutation survives undiagnosed. **todo**: the writer is not
built.

## Limits (what is not covered)

- No reader other than the tests' own parsers reads the files: that ParaView or another viewer shows them correctly (the
  cell types, the node orders, NaN values) is the maintainer's visual check. The type numbers 10, 12, 24 are recalled, not
  read from a page (see "What was read" 3).
- ASCII only, no binary or appended data, no compression: large models give large files (arithmetic 6).
- One piece, no `.pvtu`, no time series of solid results (the existing `PvdSeries` can list such files).
- A multi-material model is refused by the nodal average (ADR 0277 remaining question 3); per-region nodal values are an
  open question below.
- Nodes of no element carry NaN in the nodal stress arrays; NaN values are not part of the bit check (their payload is not
  preserved by text).
- Only nodes with 3 components (the basic tier's solids); beams and shells (six unknowns, ADR 0294) need their own arrays.
- No strain arrays, no integration-point output, no element sets as cell data (a block or material id array is easy to add
  and is left to ADR 0287 if its case file wants it).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. XML `.vtu` ASCII as the existing writer (options: legacy `.vtk`; binary) — taken: XML ASCII, the bit round trip is free.
2. The nodal invariants are those of the averaged tensor, not averaged invariants — taken (the usual reading; documented).
3. Principal directions are written (three vector arrays) beside the values — taken: cheap, and ADR 0277's remaining
   question 2 noted a viewer needs only the vectors.
4. A material interface refuses the whole write rather than silently writing element values only — taken (ADR 0277's
   recommendation (a)).

## The planned stub, as written

# ADR 0278: VTK output of a solid result: finite-element cells with point and cell data

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-8 of the structural solver, basic tier. Size S.

## Goal

The existing `.vtu` writer writes the polyhedral finite-volume mesh with cell data only. Add, beside it and without changing it,
a writer of finite-element cells from plain arrays (so `rcfd-io` needs no new dependency), and in `rcfd-post` the function that
writes a `SolidResult`: displacement, nodal von Mises and principal stresses, element stress components, reactions. This ADR
also adds the `rcfd-fem` dependency of `rcfd-post` and declares its three new modules, so that ADRs 0285 and 0286 touch only
their own file.

## Dependencies

ADR 0277; existing roadmap items: F-070.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. rcfd-io gains a writer of an unstructured grid of finite
elements from plain arrays (VTK cell types 10, 12 and 24, point data and cell data); the existing writers are unchanged and
their tests pass unedited. rcfd-post writes a SolidResult through it: displacement at the nodes, nodal von Mises and principal
stresses, the six stress components per element, reactions. A test parses the file back: counts, offsets and cell types are
consistent, every value has the bits that were written, and a box under uniform strain read back has displacement eps x at every
node within 1e-15; a field of the wrong length is refused by name.

## Reference to check against

The public VTK file-format description (Kitware) for the cell type numbers and the node order of each type; the repository's
existing writer is the model for the XML layout. The node order of the quadratic tetrahedron is to be confirmed from that public
document by the implementer (recalled here, UNVERIFIED).

## Files

`crates/rcfd-io/src/vtk.rs` (appended functions only), `crates/rcfd-io/tests/sb_vtk_elements.rs`, `crates/rcfd-post/Cargo.toml`,
`crates/rcfd-post/src/lib.rs` (three `pub mod` lines), `crates/rcfd-post/src/ solid_output.rs`, stubs `solid_loads.rs` and
`solid_coupling.rs`, `crates/rcfd-post/tests/sb_solid_output.rs`.

## What could make it miss

No reader other than the test's own parser checks the file: that a viewer shows it correctly stays the maintainer's visual
check. Mid-edge node order of Tet10 differing between the element library and VTK.
