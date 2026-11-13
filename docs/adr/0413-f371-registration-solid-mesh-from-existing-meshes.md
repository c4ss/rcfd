# ADR 0413: F-371 registration: a solid mesh from the repository's meshes (hexahedra and tetrahedra recovered from face lists, the tetrahedral mesher's output, patches as surface sets, mid-edge nodes projected onto curved walls)

Status: **REGISTERED 2026-11-13, written before any code of F-371 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-371 was written, compiled or run: the files listed under "What was read" were read and
the arithmetic below was done by hand (disclosed). Results go in the addendum at the end; nothing above the addendum is changed
after a run.

Parent: the stub [ADR 0272](0272-solid-mesh-from-existing-meshes.md) (SB-2 of the plan [ADR 0270](0270-structural-solver-basic-tier-plan.md)),
the data model [ADR 0271](0271-rcfd-fem-crate-and-solid-data-model.md) (its remaining question 2: a moved mesh is rebuilt
through `SolidMesh::new` and `add_block`, option (a)), the quadratic hexahedron upgrade of [ADR 0292](0292-quadratic-hexahedron-hex20.md) (`upgrade_to_hex20_mapped`).
Roadmap item F-371 (deps F-370, F-010, F-012, F-132). Size M. Number 0413 given by the lead. Branch
`f371-solid-mesh-from-meshes`, worktree `w49i-work/f371`, scratch `w49i-work/f371-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the hash tables or of a golden file; no new dependency, no unsafe code; clean-room. No mesh generator is
changed (`rcfd-mesh` is not edited at all), so the mesh hash tables (`case_hashes_w48u.rs`, `w48v_case_hashes.rs`) cannot move.
`rotating.rs` (F-499) and `rcfd-post` (F-502), parallel packages, are not touched.

## What F-371 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A SolidMesh built from box_mesh (hexahedra), from the
> annulus generator (hexahedra with curved faces) and from the tetrahedral mesher's output of a box surface has the node and
> element counts of its source, element volumes summing to the source volume within 1e-12 relative (tetrahedra, planar
> hexahedra) or 1e-3 (annulus, registered from the chord error), one surface set per patch whose area equals the patch area
> within 1e-12 and whose face normals point along the source's outward area vectors; a prism, pyramid or general polyhedral cell
> is refused naming the cell. With mid-edge nodes projected onto the cylinder walls of a quarter annulus the projected nodes lie
> on their radius within 1e-14 and the volume error of the quadratic tetrahedra falls at an observed order of at least 3 under
> refinement.

Clauses: (a1) sources box_mesh, annulus generator, tetrahedral mesher on a box surface; (a2) node and element counts of the
source; (a3) volume sum within `1e-12` relative of the source volume for tetrahedra and planar hexahedra, within `1e-3` for the
annulus against its closed form, the `1e-3` derived from the chord error (arithmetic 1); (a4) one surface set per patch, area
equal to the patch area within `1e-12`; (a5) face normals along the source's outward area vectors; (a6) prism, pyramid, general
polyhedron refused naming the cell; (a7) quarter annulus, mid-edge nodes of the cylinder walls projected onto the walls: on their
radius within `1e-14`; (a8) volume error of the quadratic tetrahedra at an observed order of at least 3 under refinement.

Reading of the clauses, registered:

- **(a2)** "counts of its source": the SolidMesh has exactly `Mesh::n_points()` nodes (same indices, same coordinates bit for
  bit) and `Mesh::n_cells()` elements (element `i` is cell `i`); from a `TetMesh`, `points.len()` nodes and `cells.len()`
  elements.
- **(a3)** "source volume": for box_mesh and the tet mesher, the closed-form box volume **and** the source's own volume
  (`Mesh::cell_volumes`, per element); for the annulus, the closed form `pi (b^2 - a^2) h / 4` (quarter) or `pi (b^2 - a^2) h`
  (full ring) within `1e-3`, and in addition (stronger, registered here) the source's cell volumes and the closed form of the
  chord polygon within `1e-12` (the generator's faces are planar chord rectangles, arithmetic 1).
- **(a4)** "patch area": the sum of `Mesh::face_area` over the patch's faces (for the `TetMesh` source: the sum of its surface
  triangles of that tag) and, where it exists, the closed form (box faces).
- **(a5)** per face: `|A_solid - A_source| <= 1e-12 |A_source|` for the area vectors (`SolidMesh::face_area_vector` against
  `Mesh::face_area_vectors()[f]`, for the `TetMesh` source `(p1 - p0) x (p2 - p0) / 2` of its outward triangle). This bounds the
  angle between them by `1e-12` and so gives the direction.
- **(a6)** the error is a new variant `FemError::UnsupportedCell { cell, shape, faces }` with `shape` one of `"prism"`,
  `"pyramid"`, `"polyhedron"` and its message contains `"cell <i>"` and the shape.
- **(a7)** `| |q_xy| - r | <= 1e-14` (absolute; `r = 1` and `r = 2`, the axis is z through the origin) for every projected node
  at every level.
- **(a8)** observed order `log2(e_k / e_2k)` of the relative volume error `e_k = |V_k - V| / V` for each of the three successive
  pairs of the four levels of D6.

## What was read (no run)

- `AGENT_RULES.md`; the brief; `docs/roadmap.json` entry F-371; ADR 0272 (whole), ADR 0270 section on SB-2, ADR 0271 (D2 to D4,
  remaining questions, roadmap text).
- Code of `rcfd-fem`: `mesh.rs` (whole: node orders, `HEX_FACES`, `TET4_FACES`, `add_block` and its det J check at every node,
  sets, `face_area`, `face_area_vector`, `face_incidence`, `element_volume` with the degree-3 rule for Tet10), `builders.rs`
  (`hex_box`, `tet_box`, `upgrade_to_tet10`, `MidEdgeNode`), `hex20.rs` (`upgrade_to_hex20_mapped`), `error.rs` (whole),
  `lib.rs`, the stub `convert.rs` (one doc line); `tests/si_scaffold.rs` (no row refers to `convert`).
- Code of `rcfd-mesh`: `gen.rs` (`box_mesh`, `hex_faces`, `annulus_3d`, `annulus_3d_sector`), `mesh.rs` (public API of `Mesh`,
  `Patch`, `Zone`, `from_cell_faces`, `polygon_geometry`), `tetmesh.rs` (`TetMesh`, `tetrahedralize` and its compaction of
  unused points, `to_mesh`, `FACE`, the orientation convention `det(b - a, c - a, d - a) > 0` of `check_output`), `surface.rs`
  (`TriSurface`, `box_surface`, all tags 0).
- Crate graph: `rcfd-fem` depends on `rcfd-core`, `rcfd-mesh`, `rcfd-linalg`; `rcfd-mesh` depends on `rcfd-core` only; only
  `rcfd-post` depends on `rcfd-fem`; `rcfd-api` does not. The conversion therefore lives in **`rcfd-fem`** (`convert.rs`,
  declared by ADR 0271), which already sees `rcfd_mesh`; putting it in `rcfd-mesh` would make a cycle.
- **Not read:** any finite-element or meshing code or manual outside this repository.

## Arithmetic done before the code (by hand, disclosed)

1. **Chord error of the annulus generator.** `annulus_3d` / `annulus_3d_sector` place the nodes on the circles and join them by
   chords; every face is planar (r faces: chord rectangles, z faces: planar trapezoids, theta faces: planar). The solid is
   therefore exactly the prism over the chord polygon annulus, of volume `V_c = (h / 2) (b^2 - a^2) n sin(delta)` for `n` chords
   of angle `delta`, against `V = (h / 2) (b^2 - a^2) n delta`. Relative error `e(delta) = 1 - sin(delta) / delta = delta^2 / 6 -
   delta^4 / 120 + ...`. For `e < 1e-3`, `delta < sqrt(6e-3) = 0.0775`: a quarter needs `n >= 21`. Chosen: quarter with `n = 24`
   and full ring with `n = 96`, both `delta = pi / 48 = 0.06545`, `e = 7.1395e-4 - 1.5e-7 = 7.138e-4`. The `1e-3` of (a3) holds
   with a margin of 1.4.
2. **Why planar hexahedra are exact.** The Hex8 volume is the integral of det J by 2 x 2 x 2 Gauss points. For a trilinear map
   whose six faces are planar the volume is the volume of the polyhedron bounded by those faces (divergence theorem: the volume
   is fixed by the boundary surface, and a planar bilinear face is its plane quadrilateral), and det J of a trilinear map is a
   polynomial of degree at most 2 in each reference variable, integrated exactly. The face area of a planar bilinear quad,
   `|x_u x x_v|`, is the absolute value of a function linear in `(u, v)` that does not change sign on a convex quad, so the 2 x 2
   rule is exact for trapezoids as well (the annulus z faces). The finite-volume `polygon_geometry` is exact for planar polygons.
   So `1e-12` is a rounding limit for box_mesh and for the annulus against its own cells.
3. **Order of the projected quadratic tetrahedra (a8).** The sum of the Tet10 volumes equals the volume enclosed by the
   piecewise-quadratic boundary surface when the elements conform (shared faces have the same six nodes), whatever the interior
   mid-edge nodes are, and `element_volume` integrates the cubic det J of a Tet10 exactly. Radial projection of the chord
   midpoint of an edge on the cylinder `r` between `(theta_1, z_1)` and `(theta_2, z_2)` lands exactly at the parameter midpoint
   `((theta_1 + theta_2) / 2, (z_1 + z_2) / 2)`, so the boundary is the P2 interpolant, over triangles in `(theta, z)`, of the
   cylinder map. The interpolation error of P2 integrates to `O(h^5)` per triangle from the cubic terms; on a Kuhn split each
   wall quad is two triangles symmetric under point reflection through its centre, under which the cubic terms are odd, so the
   pair cancels to `O(h^6)` and the total to `O(h^4)`. The planar z faces are bounded by quadratic arcs through three points of
   the circle, symmetric about their midpoint: area error `O(delta^5)` per arc, `O(delta^4)` in total. Expected order: **4**,
   pre-asymptotic correction of relative size `delta^2 ~ 0.15` at the coarsest level (`delta = pi / 8`). The limit 3 of the
   acceptance has a margin of about 1.
4. **Negative control of (a8): no projection.** Without the projection the mid-edge nodes are at the chord midpoints, every
   element is straight, and the volume is the chord polygon volume of arithmetic 1: `e(pi / 8) = 0.025505`, `e(pi / 16) =
   0.006413`, `e(pi / 32) = 0.0016056`, `e(pi / 64) = 4.0155e-4`; orders `log2` of the ratios `1.992`, `1.998`, `1.9995`.
5. **Radius after projection (a7).** For the axis z through the origin, `q = (r x / rho, r y / rho, z)` with `rho =
   hypot(x, y)`: the relative rounding of `|q_xy|` is a few units of `eps = 2.2e-16`, so `| |q_xy| - r | <= ~4 r eps = 1.8e-15`
   for `r = 2`; the `1e-14` limit has a margin above 5.
6. **Node and element counts of the curved levels.** Level `k`: `(n_theta, n_r, n_z) = (4k, k, k)`, `4 k^3` hexahedra, `24 k^3`
   tetrahedra; at `k = 8`, 12288 Tet10. No solve, only volumes: well under a second each.

## Design

### D1. Where and what

All new code in `crates/rcfd-fem/src/convert.rs` (declared by ADR 0271) and one new variant in `crates/rcfd-fem/src/error.rs`
(appended to `FemError`, with its `Display` arm; `FemError` is `#[non_exhaustive]`, no match outside the crate is exhaustive, the
`From<FemError> for rcfd_core::Error` arm is `_`). No existing function changes, so every existing result keeps its bits. Public
functions:

- `from_mesh(mesh: &rcfd_mesh::Mesh) -> Result<SolidMesh>`;
- `from_tet_mesh(tm: &TetMesh, names: &BTreeMap<u32, String>) -> Result<SolidMesh>`;
- `split_hexahedra(mesh: &SolidMesh) -> Result<SolidMesh>` (Hex8 to six Tet4 each, for the quadratic tetrahedra of (a7), (a8));
- `project_mid_edge_nodes<F: FnMut(&MidEdgeNode, Vec3) -> Option<Vec3>>(mesh, mids, project) -> Result<SolidMesh>`;
- `onto_cylinder(p, origin, axis, radius) -> Vec3` (the radial projection, a helper).

### D2. `from_mesh`: element node order from the face lists

Each cell `c` is classified from its faces (`Mesh::cell_faces`, each face's points; the loop of a face is outward for its owner
and reversed for its neighbour, the convention of `Mesh`):

| faces | points per face | distinct points | shape |
| --- | --- | --- | --- |
| 4 | all 3 | 4 | tetrahedron |
| 6 | all 4 | 8 | hexahedron (if the topology check below passes) |
| 5 | two 3, three 4 | 6 | `prism`, refused |
| 5 | four 3, one 4 | 5 | `pyramid`, refused |
| any other, or a 6-quad cell failing the check | | | `polyhedron`, refused |

- **Tetrahedron.** The first face's outward loop `(p, q, s)` is local face 0 of Tet4, `[0, 2, 1]`: nodes `[p, s, q, t]` with `t`
  the fourth point. Positive orientation follows from the outward loop (right-hand normal away from `t`).
- **Hexahedron.** The first face's outward loop `(p, q, s, u)` is local face 0 of Hex8, `[0, 3, 2, 1]`: `n0 = p, n3 = q, n2 = s,
  n1 = u`. The cell's edges are the consecutive pairs of its face loops; each bottom node has exactly one edge to a point not on
  the bottom face, which gives `n4..n7` above `n0..n3`. Check: eight distinct nodes, and the corner sets of the cell's six faces
  equal those of the six local faces `HEX_FACES`; otherwise the cell is a `polyhedron` (refused). This works on warped cells,
  because it is topological; inversion is caught by `add_block` (det J at every node, ADR 0271).
- A mesh mixing tetrahedra and hexahedra is refused at the first cell whose shape differs from cell 0's
  (`UnsupportedCell { shape: "tetrahedron in a mesh of hexahedra" }` or the converse): element `i` is cell `i` only in one block.
  Mixed meshes are a limit (L2).
- One block (`Tet4` or `Hex8`) in cell order. Sets: element set `all`, node set `all`; per patch with at least one face, a
  surface set and a node set of the patch's name (the faces' points, sorted); per cell zone with at least one cell, an element
  set of the zone's name. A boundary face is located by its sorted corner nodes in `SolidMesh::face_incidence` (exactly one
  member, otherwise refused with `InvalidValue { key: "patch" }` naming the face). Face zones are not converted (L3). A patch or
  zone named `all` collides and is refused by the set validation (`DuplicateSet`).

### D3. `from_tet_mesh`

`TetMesh::cells` are positively oriented with `det(b - a, c - a, d - a) > 0` (its `check_output`), the Tet4 convention: copied as
they are. Nodes: `points` (compacted by `tetrahedralize`). Surface sets by tag, named as `TetMesh::to_mesh` names its patches
(`names[tag]`, else `wall` for tag 0 and `wall<k>`); the triangle is located by its sorted nodes in `face_incidence` (exactly one
member). Node sets per tag (the triangles' nodes), `all` sets as D2.

### D4. `split_hexahedra`

Each Hex8 element `e` becomes Tet4 elements `6 e + t`, `t = 0..5`: the Kuhn split along the diagonal from the corner `m` of
**smallest global node index** to its opposite corner (in reference coordinates `s` and `-s`): for each of the six axis
permutations `(p0, p1, p2)` (in the order `012, 021, 102, 120, 201, 210`), the tetrahedron `s, s` flipped in `p0`, then in `p1`,
then `-s`, swapped to positive orientation by the sign of its reference determinant. A face of a hexahedron is cut along the
diagonal through `m` (faces at `m`) or through `-s` (the far faces). Two neighbours cut their common face the same way when its
diagonal passes through the face's smallest node, which holds on the structured generators (lexicographic numbering) but not for
every numbering; the result is therefore **checked**: every face key of the output has at most two members and the faces with one
member are exactly two per boundary face of the input; otherwise it is refused naming the hexahedron
(`InvalidValue { key: "split_hexahedra" }`). Element sets map `e` to its six; node sets are kept; a surface face `(e, f)` becomes
the two faces of `e`'s tetrahedra whose three nodes lie on it. Another kind than Hex8 is refused (`UnsupportedKind`).

### D5. `project_mid_edge_nodes` and `onto_cylinder`

For each `MidEdgeNode` `m` (as returned by `upgrade_to_tet10` or `upgrade_to_hex20`), `project(&m, x_m)` returns the new position
or `None` (left). The mesh is rebuilt through `SolidMesh::new` and `add_block` (ADR 0271 remaining question 2, option (a)), so
det J is checked again at every node and an element inverted by the projection is **refused, not repaired**
(`InvertedElement`); block properties and all sets are copied. A node index out of range is refused (`InvalidValue`).
`onto_cylinder(p, o, a, r)`: `d = p - o`, axial part `(d . a) a` with `a` normalised, radial part `d - (d . a) a` scaled to length
`r`; a point on the axis or a non-positive radius gives a non-finite or refused result (the rebuild refuses the non-finite node).

### D6. The set-ups

- **B (box_mesh):** `box_mesh([3, 4, 5], [1.0, 2.0, 3.0], [Uniform; 3])`; volume 6; patches `xmin..zmax` with areas 6, 6, 3, 3,
  2, 2.
- **BG (graded box):** the same with `Grading::Expansion(3.0)` in x (planar hexahedra of unequal sizes).
- **AQ (quarter annulus):** `annulus_3d_sector(24, 3, 2, 1.0, 2.0, 1.0, pi / 2, Uniform)`: 144 cells, 4 x 25 x 3 = 300 points;
  closed form `3 pi / 4`; patches `inner, outer, zmin, zmax, theta_min, theta_max`.
- **AF (full annulus):** `annulus_3d(96, 2, 2, 1.0, 2.0, 1.0, 0.0, Uniform)`: 384 cells, `3 x 96 x 3 = 864` points; closed form
  `3 pi`; patches `inner, outer, zmin, zmax`.
- **T (tetrahedral mesher):** `box_surface([0; 3], [1.0, 2.0, 3.0], [2, 3, 4])` with each triangle tagged by the box face it lies
  on (tags 0..5 for `xmin..zmax`, set in the test; the generator's output is not changed), `tetrahedralize(&s,
  &TetOptions::default())`, names `xmin..zmax`; converted by `from_tet_mesh` and, a second route, by `from_mesh(tm.to_mesh)`.
- **Refusals:** single cells built by `Mesh::from_cell_faces`: a triangular prism, a square pyramid, a hexagonal prism (8 faces),
  and a mesh of one tetrahedron and one hexahedron (mixed).
- **CQ (curved quarter annulus):** levels `k = 1, 2, 4, 8`: `annulus_3d_sector(4k, k, k, 1.0, 2.0, 1.0, pi / 2, Uniform)`,
  `from_mesh`, `split_hexahedra`, `upgrade_to_tet10`, then every mid-edge node that is a node of the surface set `inner` moved by
  `onto_cylinder(., 0, z, 1.0)` and of `outer` by `onto_cylinder(., 0, z, 2.0)`. Exact volume `3 pi / 4`.

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| C1 | a1, a2 | B, BG, AQ, AF: node count = `n_points`, coordinates equal bit for bit; element count = `n_cells`; one block of Hex8 | exact | D2 |
| C2 | a3 | B, BG: `|sum V_e - 6| / 6`; every element `|V_e - V_cell| / V_cell` | `1e-12` | arithmetic 2 |
| C3 | a3 | AQ, AF: `|sum V_e - V| / V` against the closed form of the annulus | `1e-3` | arithmetic 1: `7.14e-4` |
| C3b | a3 (stronger) | AQ, AF: against the chord closed form `V_c` and every element against its cell | `1e-12` | arithmetic 1, 2 |
| C4 | a4 | B, BG, AQ, AF, T (both routes): surface set names = names of the non-empty patches; faces per set = patch size; `|sum A_solid - A_patch| / A_patch` (and against the closed form on B, BG, T) | `1e-12` | arithmetic 2 |
| C5 | a5 | every face of every set: `|A_s - A_src| <= 1e-12 |A_src|` (vectors) | `1e-12` | arithmetic 2 |
| C6 | a1, a2, a3 | T: node count = `points.len()`, element count = `cells.len()` (both routes); `|sum V_e - 6| / 6` | exact; `1e-12` | rounding of a few hundred determinants |
| C7 | a6 | prism, pyramid, hexagonal prism: `UnsupportedCell { cell: 0, shape }` with shapes `prism`, `pyramid`, `polyhedron`, message contains `cell 0` and the shape; mixed mesh refused naming cell 1 | exact | D2 |
| C8 | a7 | CQ, every level: every projected node `| |q_xy| - r |` | `1e-14` | arithmetic 5: `~2e-15` |
| C9 | a8 | CQ: observed order of the projected Tet10 volume error, each of the pairs (1,2), (2,4), (4,8) | `>= 3` | arithmetic 3: about 4 |
| C10 | (rest) | CQ, every level: the conversion and the projection are accepted (no element refused as inverted); element count `24 k^3`; the projected mesh's surface sets have the areas of the split mesh's within the chord area change (not limited, printed) | accepted | arithmetic 3 |
| C11 | D2, D3 | T: the two routes give the same element volumes (per element, sorted node sets equal) within `1e-12` relative | `1e-12` | same geometry |
| C12 | D2 | zones: AQ with a cell zone `half` (cells with `k = 0`) gives an element set `half` with those elements | exact | D2 |

Negative controls:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | CQ without the projection (`None` for every node): order of the volume error | about 2 (arithmetic 4) | each pair in `[1.9, 2.1]`, so C9's check discriminates |
| N2 | CQ without the projection: volume equals the chord closed form | arithmetic 4 | `1e-12` relative |
| N3 | a projection that moves the mid-edge nodes of the inner wall of a level-1 mesh by `-0.6` radially (into the hole, past the corners of its tetrahedra): refused as `InvertedElement` | refused | exact |
| N4 | `split_hexahedra` of a two-hexahedron mesh renumbered so that the shared face's smallest node sits on the other diagonal for one of the two cells: refused naming a hexahedron, not silently non-conforming | refused | exact |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| M1 | tetrahedron order `[p, q, s, t]` instead of `[p, s, q, t]` | C6 (refused as inverted) |
| M2 | hexahedron: `n1` and `n3` exchanged | C2 / C1 (inverted or face check) |
| M3 | surface face located by the wrong member (local face `(f + 1) % n`) | C4, C5 |
| M4 | the prism case removed from the classification (falls to `polyhedron`) | C7 |
| M5 | `onto_cylinder` returns `p` | C9, C8 |
| M6 | `split_hexahedra` without the orientation swap | C10 (refused as inverted) |
| M7 | `split_hexahedra` from the largest node index instead of the smallest | N4 or C10 (expected: still conforming on the structured mesh, caught only by N4 if at all; recorded as found) |
| M8 | cell zones not converted | C12 |

## Decision rule

- **done:** C1 to C9 (every clause of the acceptance) met, C10 to C12 and N1 to N4 as stated, every mutation except possibly M7
  caught by a registered check.
- **partial:** a clause of the acceptance missed (recorded as a `*_known_miss`) or not run, with the cause diagnosed.
- **todo:** the conversion does not build.

## Limits (not covered)

- L1. Quadratic **hexahedra** on curved walls (`upgrade_to_hex20_mapped`, ADR 0292) are not checked here; the projection of D5
  accepts their `MidEdgeNode` list as well, untested.
- L2. Mixed tetrahedron and hexahedron meshes, prisms (inflation layers), pyramids and polyhedra (octree, dual) are refused, not
  converted.
- L3. Face zones and periodic pairs carry no meaning in the solid mesh: periodic patches become plain surface sets.
- L4. `split_hexahedra` is conforming only when every shared face's smallest node lies on the diagonal the split chooses (true for
  the structured generators); other numberings are refused (N4), not repaired.
- L5. The Gmsh route (`rcfd-io`) is ADR 0287's, since `rcfd-fem` does not depend on `rcfd-io`.
- L6. Only the volume (and radius) are checked under refinement; stresses on curved meshes are later packages' (ADR 0280).

## Amendment 1 (2026-11-13, before any run of any check; no test has been written or run)

Written while writing the tests, after the library code compiled and before the first run of anything. **N3 as registered is not
a sure negative control.** Moving every inner-wall mid-edge node by `-0.6` radially need not invert an element: at a corner of a
Tet10 the column of `J` along an edge to node `j` is `(x_j - x_0) + 4 (m_0j - (x_0 + x_j) / 2)`, so a radial shift adds `-2.4
r_hat` to the columns of the wall edges, and with a third column along `r_hat` (the radial edge of the Kuhn tetrahedra) these terms
drop out of the determinant (worked for the tetrahedron corner, circumferential edge, axial edge, radial edge: `det` unchanged;
for a diagonal third column the sign is kept as well). Whether some element inverts is therefore not known beforehand.

- **N3** is still run once as registered and its outcome recorded, but it is **not** part of the decision rule.
- **N3b** (replaces N3 in the decision rule): on the level-1 mesh of CQ (after the radial projection), the first mid-edge node of
  the `inner` wall in the order of `upgrade_to_tet10`'s list is moved further along its edge by `0.4 (x_b - x_a)` (towards its
  second end `b`). The column of `J` at `b` along the edge becomes `(x_a - x_b) + 4 * 0.4 (x_b - x_a) = -0.6 (x_a - x_b)` for
  every tetrahedron containing the edge (the other mid-edge nodes at `b` are unchanged), so `det J` at `b` is `-0.6` times its
  value with the chord: **refused as `InvertedElement`** at a local node that is the corner `b` (registered: the error variant is
  `InvertedElement` and its `jacobian < 0`).
- M7 (split from the largest node index) is an **equivalent mutant**: the diagonal from the largest corner to its opposite is the
  same main diagonal as from the smallest, when the largest is opposite the smallest (true on the lexicographic boxes, where
  every hexahedron's largest node is the corner opposite its smallest); it is run and recorded as not caught if so.
