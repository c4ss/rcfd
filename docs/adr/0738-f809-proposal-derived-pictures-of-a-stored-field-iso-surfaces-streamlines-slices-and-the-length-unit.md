# ADR 0738: F-809 proposal: derived pictures of a stored field: iso-surfaces, streamlines, slices, the Studio's cut from the CLI, and the length unit

Status: **ACCEPTED 2026-11-17** (all eight recommended options, see "Answers"). Built (see "Built"): the engine half
and the Studio half. Not built: P3's first bullet (`render_field` with `axis`, `slice`, `vectors`; R1) and `rcfd post
render`.

Roadmap item F-809 (Electron Studio: derived pictures), ADR 0801 Addendum 7 and "Engine follow-ups of the Studio items",
rows E-809a to E-809e; E-809e is also F-837's length unit (ADR 0801 Addendum 13). F-809's acceptance: "Each derived
picture computed by the engine (rcfd-render or a registered opt-in service method) or, if computed in the Studio,
matched against the engine's reference value for value within a tolerance registered before the code". Parents:
[ADR 0124](0124-wave47c-time-series-field-store.md) (the field store),
[ADR 0350](0350-electron-studio-performance-spike-and-rcfd-studio-retirement.md) amendment 1 (the `data.*` channel),
guardrails G2, G3, G4, G6 and G16. Number 0738 follows ADR 0737.

Clean room: the algorithms below are from the public literature cited in "References". No other program's source, binary
or documentation of internals was read or is to be read for this work.

## Context

What the engine computes on a stored field today (ADR 0801 Addendum 7, checked against main):

- **No iso-surface.** `rcfd_post::surfaces::cut_implicit` (`crates/rcfd-post/src/surfaces.rs:201`) cuts a mesh at `g =
  0` for a function of position by marching tetrahedra over a decomposition of every cell into tets (cell centre, face
  centre, two consecutive face points; module comment `surfaces.rs:1-8`). It evaluates `g` at the points, face centres
  and cell centres. A `Surface` keeps the cell of each triangle (`tri_cell`) and of each vertex (`surfaces.rs:23-35`).
  `Surface::clip_by_values` clips a surface by vertex values (`surfaces.rs:540`). Nothing turns a stored cell field into
  an iso-surface.
- **No streamlines.** There is no integrator in the workspace. Pieces exist: seed rakes `PointCloud::rake`
  (`surfaces.rs:674`), `rcfd_post::derived::locate_cell` (`crates/rcfd-post/src/derived.rs:114`).
- **No slice of a stored 3D field.** The project tool `render_field` draws a stored cell field with `render_scalar` but
  refuses a 3D case ("2D cases only (one cell layer)", `crates/rcfd-workspace/src/queries.rs:422-450`). `render_scalar`
  itself already slices a 3D mesh along `axis` at `slice` (`crates/rcfd-post/src/render.rs:1-8`, `:90-116`, `:460`);
  `render_field` passes neither. `render.png` (`crates/rcfd-api/src/service.rs:736-767`) adds arrows with
  `overlay_vectors` (`render.rs:535`) but on a live session only; `field.slice` (`service.rs:706-723`) is a strided list
  of cell centres of a live session, not a slice. `cut_plane` (`surfaces.rs:320`) cuts any mesh exactly.
- **No CLI or MCP picture of the Studio's cut.** The cut rule the Studio follows was rcfd-render's `View::clip` ("keep
  `n . p <= d`"), and `crates/rcfd-render` is removed (f5903048, ADR 0800 section 10). Today the rule lives only in the
  Studio (`apps/studio-web/src/renderer/field/derived.ts`, T2), and the arrows by `overlay_vectors`' rule carried to the
  boundary surface (T3). Claude Code cannot make that picture (G16).
- **No length unit.** The store's MESH frame has counts and the mesh bytes, no unit
  (`crates/rcfd-tsstore/src/lib.rs:7-15`). The stored mesh is the solver's: `build_mesh` applies `mesh.scale` to a mesh
  file (`crates/rcfd-api/src/sim.rs:686-712`), and the engine keeps every quantity in coherent SI
  (`crates/rcfd-core/src/units.rs:1-3`), so its coordinates are metres. Field units are written per field
  (`crates/rcfd-api/src/fieldstore.rs:14-22`), coordinates have none.
- **The `data.*` channel** (`crates/rcfd-api/src/data_service.rs:1-35`) is off unless the program calls `enable_data`
  (`data_service.rs:81`); `rcfd serve` does not. Its D2 test pins the method list exactly (`METHODS` then
  `DATA_METHODS`, `crates/rcfd-api/tests/data_service_f447.rs:119-143`).
- **A glTF writer** exists for boundary patches (`crates/rcfd-post/src/gltf.rs:60`, `:302`) with a structural validator
  (`gltf.rs:317`, `crates/rcfd-post/src/glb_check.rs:124`).

## Proposal

### P1. Iso-surfaces of a stored cell field (E-809a)

- Library: `rcfd_post::surfaces::cut_values(mesh, values_at_tet_vertices) -> Surface`, the marching-tetrahedra core of
  `cut_implicit` taking the values at the points, face centres and cell centres directly. `cut_implicit` becomes
  `cut_values(mesh, g evaluated at those positions)`, with the same arithmetic in the same order, so its output is bit
  for bit unchanged (test I3).
- `rcfd_post::iso::iso_surface(mesh, cell_values, level, colour_by) -> IsoSurface {surface, colour}`: the values at the
  tet vertices are the cell value at a cell centre, the mean of the two cells at an interior face centre (the owner's at
  a boundary face), and the mean of the cells sharing a mesh point (Q2). Marching tetrahedra (Doi and Koide 1991) has no
  ambiguous case, unlike marching cubes (Lorensen and Cline 1987). `colour` is a second stored field at the vertices by
  the same reconstruction, or the iso field's level.
- Input fields: any stored cell field, and `speed` = |(ux, uy, uz)| from the stored components (Q8).

### P2. Streamlines of a stored velocity field (E-809b)

- `rcfd_post::streamlines::trace(mesh, u_cells, seeds, opts) -> Vec<Streamline>`; each line has `points`, `cells` (the
  cell of each point), `speed` per point, `direction` (forward, backward) and `stop` (`left_domain`, `stagnant`,
  `max_steps`, `max_length`, `cell_walk_failed`).
- Velocity at a point: the tet of the P1 decomposition that holds it, linear (barycentric) in the values at its four
  vertices, reconstructed as in P1 (Q4). This makes the field continuous across faces, which a cell-constant field is
  not.
- Integration: classical fourth-order Runge-Kutta (Runge 1895, Kutta 1901; Hairer, Nørsett and Wanner 1993) in arc
  length, step `h = step_fraction x` the current cell's length (`rcfd_mesh::adapt::cell_lengths`,
  `crates/rcfd-mesh/src/adapt.rs:682`), default 0.25 (Q3). The point is followed from cell to neighbour across the face
  it leaves by (Löhner and Ambrosiano 1990), falling back to `locate_cell`, in tets as in Kenwright and Lane 1996;
  Darmofal and Haimes 1996 for the error of the integrators on interpolated fields.
- Seeds: `{rake: {p0, p1, n}}`, `{points: [..]}`, or `{patch: name, n}` (face centres of a patch, every k-th to give n).
  Options: `direction: forward | backward | both`, `max_steps` (default 2000), `max_length` (default 4 x the bounding
  box diagonal), `min_speed` (default 1e-9 x the largest stored speed).

### P3. Slices of a stored 3D field (E-809c)

- `render_field` gains `axis` (`x`, `y`, `z`), `slice` (a coordinate, m), `vectors` (bool) and `range`. With `axis` or
  `slice` given it draws `render_scalar`'s slice of a 3D case and does not refuse; with `vectors` it adds
  `overlay_vectors` from the stored `ux`, `uy`, `uz` (density 24, as `render.png`). Without the new arguments the reply,
  including the 2D refusal, is as now.
- A slice with values as geometry: `derive_picture {kind: "slice"}` (P4) gives `cut_plane` of the stored mesh with the
  cell value of each triangle (piecewise constant, as `render_scalar` colours a cell) and, with `vectors`, the stored
  vector of each triangle's cell at its centroid.

### P4. Delivery: one project tool, one CLI command (E-809a to E-809d)

- MCP project tool `derive_picture {run, snapshot | step | time, kind: "iso" | "streamlines" | "slice" | "cut_surface",
  ...kind's arguments}`, class `read` (it writes a derived file, as `render_field` writes an image over budget,
  `queries.rs:460-470`). It writes `runs/<id>/derived/<kind>-<fnv64 of the arguments>.glb`, reuses an existing file with
  that name, and returns `{path, kind, snapshot, triangles | lines | points, bounds, length_unit: "m", range, stats}`.
- The GLB: triangles (mode 4) or line strips (mode 3) with `POSITION` (f32, metres), `_CELL` (u32, the cell of each
  vertex or point) and `_VALUE` (f32); valid under `validate_glb`.
- `kind: "cut_surface"` (E-809d) is the Studio's picture: the boundary surface triangles of `data.surface` that keep `n
  . p <= d` (the rule of T2, now the engine's), coloured by their cell, plus the arrows by T3's rule (24 bins per axis,
  0.8 of a lattice step for the largest vector, the head 0.3 back and 0.5 out). The Studio then draws this geometry, and
  the engine is the reference for T1 to T3.
- CLI: `rcfd post derive <store dir> --kind iso|streamlines|slice|cut_surface [--field f] [--level v] [--seeds ...]
  [--axis x --at v --keep below|above] [--vectors] [--snapshot n] --out <file.glb>`, the same library and the same file
  (G16). `rcfd post render <store dir> --field f [--axis x --at v] [--vectors] --out <file.png>` is P3's image without
  a project.
- Errors: 1202 no such run, field or snapshot; 1003 a point field, a missing velocity component, a level outside the
  field's range (with the range); 1004 over the limits (Q7); -32602 a bad argument.

### P5. The length unit (E-809e)

- `rcfd_tsstore::LENGTH_UNIT = "m"`, documented: the stored mesh is the solver's mesh after `mesh.scale` and
  `[mesh.transform]`, in metres.
- Additive reply members: `data.open` gains `length_unit: "m"` (and `time_unit: "s"`, ADR 0734 P2); `derive_picture` and
  `render_field` say `length_unit`. The store's bytes do not change.

### How defaults and `baseline_hashes.rs` stay unchanged

- Nothing here touches a solver path; `crates/rcfd-api/tests/baseline_hashes.rs` is not edited or re-recorded.
- `cut_implicit` keeps its output bit for bit (I3); every existing `rcfd-post` test passes unedited.
- `render_field` without the new arguments answers byte for byte as now. `DATA_METHODS` and `service.info` do not
  change, so `data_service_f447.rs` D2 passes unedited; `data.open` gains members (Q6).
- `derive_picture` is one new project tool: a registered restatement of `project_list_f270.rs` and `tool_list_f275.rs`,
  no golden edited. `rcfd post derive` and `rcfd post render` are new subcommands; existing `rcfd` commands are
  unchanged.

## Alternatives

1. **Compute the pictures in the Studio** and register tolerances (F-809's second route). Iso-surfaces and streamlines
   are numerical methods the engine should own (G2), and Claude Code would not get them (G16). Recommendation: no.
2. **`data.iso`, `data.streamlines`, `data.slice` as tickets on the data channel.** Fast for a remote store, but the
   channel is off in `rcfd serve`, adding methods restates D2, and the CLI gets nothing. Offered as Q1 (b).
3. **Marching cubes** (Lorensen and Cline 1987). Needs a lattice and has ambiguous cases on unstructured cells.
   Recommendation: marching tetrahedra on the decomposition `cut_implicit` already uses.
4. **A CPU 3D rasteriser for a PNG of the Studio's view.** rcfd-render was removed; a GLB of the geometry is enough for
   Claude Code and the Studio and is testable value for value. Offered as Q5 (c).
5. **The length unit in the store's MESH frame** (format version 2). Changes the bytes of every new store.
   Recommendation: no; the unit is fixed by the engine.

## Open questions for the maintainer

1. **Delivery.** Recommendation: (a). (a) the project tool `derive_picture` writing `runs/<id>/derived/*.glb`, and the
   CLI `rcfd post derive`; (b) `data.*` tickets behind a second opt-in; (c) both.
2. **Values at points and face centres for iso-surfaces.** Recommendation: (a). (a) the mean of the cells sharing the
   point (owner's value at a boundary face); (b) inverse-distance weights (Shepard 1968); (c) a least-squares gradient
   reconstruction (needs boundary values the store does not have).
3. **Integrator.** Recommendation: (a). (a) classical RK4, step 0.25 of the local cell length; (b) an embedded adaptive
   pair (Dormand and Prince 1980); (c) explicit Euler.
4. **Velocity inside a cell.** Recommendation: (a). (a) linear on the tet decomposition (continuous); (b) the cell's
   value (piecewise constant; lines kink at faces).
5. **The Studio's cut from the CLI (E-809d).** Recommendation: (a). (a) `cut_surface` geometry in a GLB, which the
   Studio draws too; (b) only `render_field` with `axis`, `slice` and `vectors` (a 2D image of a slice, not the Studio's
   view); (c) a CPU 3D rasteriser for a PNG.
6. **`data.open` members.** Recommendation: (a). (a) add `length_unit` and `time_unit` always (two additive members);
   (b) only behind `rcfd serve --ui` (ADR 0730).
7. **Size limits.** Recommendation: (a). (a) refuse over 2 000 000 triangles or 1 000 seeds x `max_steps` 10 000 with
   1004 and the numbers; (b) none.
8. **Fields.** Recommendation: (a). (a) stored cell fields and `speed`; (b) stored fields only; (c) also derived fields
   such as the Q-criterion (`rcfd_post::derived::q_criterion`, needs boundary values for its gradients).

## Answers (maintainer, 2026-11-17)

1. (a) the project tool `derive_picture` and `rcfd post derive`, writing `runs/<id>/derived/*.glb`; no new `data.*`
   method.
2. (a) the mean of the neighbouring cells at points and face centres.
3. (a) classical RK4, step 0.25 of the local cell length.
4. (a) velocity linear on the tet decomposition of each cell.
5. (a) the engine writes the Studio's cut and arrows; the Studio draws the engine's output, the engine is the reference.
6. (a) `data.open` gains `length_unit: "m"`; `time_unit` was already there (ADR 0734 P2), so only `length_unit` is new.
7. (a) refuse over 2 000 000 triangles, or over 1 000 seeds or 10 000 steps, with 1004.
8. (a) the stored cell fields and `speed`.

## Tests that would prove it

- **I1** a cell field linear in y on a box mesh (uniform and graded): the iso-surface at a level more than one cell from
  the y bounds equals `cut_plane` at that y to 1e-12 relative (the reconstruction is exact there).
- **I2** the field `|x - c|` on 16^3, 32^3, 64^3: the iso-surface's area converges to 4 pi R^2 with an observed order in
  [1.8, 2.2] (second order, as the module comment states for curved cuts).
- **I3** `cut_implicit` after the split equals the base commit's on every `rcfd-post` test case bit for bit.
- **S1** uniform flow: lines are straight to 1e-12 and stop `left_domain` at the outlet; `cells[i]` equals
  `locate_cell(points[i])` for every point.
- **S2** solid-body rotation u = (-y, x, 0) on a disc mesh: one revolution closes to a relative radius error that falls
  with the order of the interpolation (second) under refinement, and with an analytic velocity (interpolation bypassed)
  with order in [3.8, 4.2] in the step (RK4).
- **S3** a stagnation point stops `stagnant`; `max_steps` and `max_length` stop as named.
- **R1** `render_field` without new arguments equals the base commit's reply byte for byte on a 2D run, and the refusal
  text on a 3D run; with `axis` and `slice` on a 3D run, the drawn cells are those whose extent contains the slice.
- **G1** every GLB passes `validate_glb`; positions, cells and values read back equal the library's to f32 rounding.
- **G2** `cut_surface` geometry equals the Studio's T2 and T3 sets (`tests/unit/field-derived.spec.ts` fixtures)
  exactly, so the Studio can switch to the engine's geometry with its tests as they are.
- **U1** `data.open` gains `length_unit: "m"` and `time_unit: "s"`; D1 to D3 of `data_service_f447.rs` pass unedited.
- **B1** `baseline_hashes.rs` passes unedited; the tool lists equal the restated goldens.

## Unblocks

F-809 rows E-809a (iso-surfaces), E-809b (streamlines), E-809c (slices with values of a stored 3D field), E-809d (the
Studio's cut and arrows from the CLI and MCP), E-809e (the length unit), and F-837's length unit in the cut panel,
domain size and scale bar.

## References

- Lorensen, W. E. and Cline, H. E. (1987). Marching cubes: a high resolution 3D surface construction algorithm. Computer
  Graphics (SIGGRAPH '87 Proceedings) 21(4), 163-169.
- Doi, A. and Koide, A. (1991). An efficient method of triangulating equi-valued surfaces by using tetrahedral cells.
  IEICE Transactions E74(1), 214-224.
- Shepard, D. (1968). A two-dimensional interpolation function for irregularly-spaced data. Proceedings of the 23rd ACM
  National Conference, 517-524.
- Runge, C. (1895). Ueber die numerische Auflösung von Differentialgleichungen. Mathematische Annalen 46, 167-178.
- Kutta, W. (1901). Beitrag zur näherungsweisen Integration totaler Differentialgleichungen. Zeitschrift für Mathematik
  und Physik 46, 435-453.
- Hairer, E., Nørsett, S. P. and Wanner, G. (1993). Solving Ordinary Differential Equations I: Nonstiff Problems, 2nd
  ed. Springer.
- Dormand, J. R. and Prince, P. J. (1980). A family of embedded Runge-Kutta formulae. Journal of Computational and
  Applied Mathematics 6(1), 19-26.
- Löhner, R. and Ambrosiano, J. (1990). A vectorized particle tracer for unstructured grids. Journal of Computational
  Physics 91(1), 22-31.
- Kenwright, D. N. and Lane, D. A. (1996). Interactive time-dependent particle tracing using tetrahedral decomposition.
  IEEE Transactions on Visualization and Computer Graphics 2(2), 120-129.
- Darmofal, D. L. and Haimes, R. (1996). An analysis of 3D particle path integration algorithms. Journal of
  Computational Physics 123(1), 182-195.

## Built (2026-11-17, engine half)

- `rcfd-post`: `iso.rs` (P1: `tet_vertex_values`, `iso_surface` with an optional colour field interpolated along the
  same tet edges), `streamlines.rs` (P2: `Locator`, `TetVelocity`, `trace`, `trace_with`, `rake`, `patch_seeds`, stops
  `left_domain`, `stagnant`, `max_steps`, `max_length`, `cell_walk_failed`), `picture.rs` (P3/P4: `store_surface`, the
  surface `data.surface` gives, `clip_surface` with T2's rule `n . p <= d`, `vector_glyphs` with T3's rule, `slice`, and
  a GLB writer and reader). `surfaces.rs`: the marching core of `cut_implicit` is split out (`march`) and shared by
  `cut_values` and `cut_values_edges`; `cut_implicit` is the base commit's bit for bit (I3).
- `rcfd-workspace::derive` and the project tool `derive_picture` (class `read`, after `render_field`; 44 tools with
  `--project`); `rcfd post derive` calls the same library (`derive_dir`) and writes the same bytes (G16).
- `data.open` gains `length_unit: "m"` (`rcfd_tsstore::LENGTH_UNIT`); `DATA_METHODS` is unchanged (D2).
- Tests: `crates/rcfd-post/tests/derived_pictures_adr0738.rs` (I1, I2, I3, S1, S2, S3, T1, T2, T3, G1),
  `crates/rcfd-mcp/tests/derive_picture_adr0738.rs` (P4, Q7), `crates/rcfd-cli/tests/post_derive_adr0738.rs` (P4, G16),
  `crates/rcfd-api/tests/data_units_adr0738.rs` (U1, G2: `data.surface` is `store_surface` byte for byte). The tool-list
  tests are restated (27 project tools, 44 with `--project`); `baseline_hashes.rs` passes unedited (B1).

Deviations and decisions while building:

- **S2's measure.** On a uniform disc mesh the radius error with the interpolated velocity converges at order 3.2
  (superconvergence of the symmetric means), not 2, so S2 asserts at least 1.8 there and checks the second order on a
  graded mesh (expansion 3.0: order 2.0, asserted in [1.8, 2.2]). A closed-revolution distance was not used: it mixes in
  the chord sum of the stop. The RK4 order is measured as the phase error of the analytic velocity at step fractions
  0.4, 0.2, 0.1 (order 3.9, asserted in [3.8, 4.2]).
- **T3 in 2D.** The arrows are binned per surface triangle, as the Studio's `vectorGlyphs`, so a 2D cavity of n x n
  cells gives up to two arrows per lattice bin (448 on the test cavity, not 256).
- A missing `uz` is taken as zero for the velocity of a 2D store; `_CELL` is u32; the GLB's `asset.extras` holds the
  reply's summary.
- `derive_picture` is class `read` but is not added to the first allow-list of `project_w48f` F2: that list is the
  maintainer's and was not asked about.
- Not built: P3's first bullet (`render_field` with `axis`, `slice`, `vectors`, `range`; R1) and `rcfd post render`.
- G2 from the Studio's side: `apps/studio-web/tests/unit/field-derived.spec.ts` runs `rcfd post derive --kind
  cut_surface --vectors` on the quick-start cavity and the axial-fan passage and finds the Studio's surface triangles,
  cells and corners bit for bit, the arrows of `vectorGlyphs` within 2^-20 of the largest extent (T3's tolerance) with
  the same count, and, for a cut through the middle of the longest axis kept below and above, the triangles T2 keeps
  whole unchanged, no triangle T2 drops, and no drawn corner outside `n . p <= d`. So the Studio can switch to the
  engine's geometry with its T2 and T3 tests as they are.

## Built (2026-11-17, Studio half)

- The data port gains `derive` (`apps/studio-web/src/main/data/derive.ts`, `bridge.ts`, `studio-data.ts`): main checks
  the page's arguments member by member (`parseDeriveArgs`), runs `rcfd post derive` on a local store inside a data root
  (rcfd from `RCFD_STUDIO_RCFD`, else beside `RCFD_STUDIO_MCP`, else the shipped engine) and hands back the GLB and the
  reply; a cancel stops the command; the engine's codes (1003, 1004, 1202) reach the page unchanged; the service route
  and a Studio without an engine refuse with the reason. The page's bridge is `window.studioDerived` (one function).
- `src/renderer/field/glb.ts` reads the engine's GLB. The field viewport draws the engine's cut surface in place of the
  surface (each piece coloured by the value of the surface triangle it was cut from; the readout names that triangle
  through `_TRIANGLE`), the engine's arrows, the iso-surface of the shown field (coloured by its level through the same
  table; the surface hidden while it is shown) and streamlines (deep ink over the surface). The surface is no longer
  masked on the GPU; the mesh grid keeps the mask. The camera code is unchanged.
- Controls: the iso-surface (a 3D case, the shown values a stored cell field; a panel sets the level in the shown range)
  and streamlines (a store with the velocity components; a panel sets the axis of the seed line through the middle of
  the domain, the mid-plane of a 2D case, and the number of seeds; both directions, at most 2 000 steps). Every derived
  picture of a store read through the service is unavailable with the reason (G6). The frame names what is drawn:
  `data-cut`, `data-arrows`, `data-iso`, `data-streamlines` (the engine's counts), once drawn on the GPU.
- Tests: `tests/unit/derive-bridge.spec.ts` (the request's shape and refusals, the flags, where rcfd is, the GLB is the
  CLI's file of the same arguments, the data roots, the refusals and the engine's codes, a cancel);
  `tests/unit/field-derived.spec.ts` (availability restated for the engine's pictures, G2 through the viewport's GLB
  reader, the iso-surface and streamlines read back); `tests/e2e/field-derived.spec.ts` (T2 and T3 pixel tests unchanged
  in their checks, now waiting for the engine's cut; the iso-surface's colour and level, the streamlines' ink and
  seeds, on the 1e5 box and the 2D cavity).
- Decision: the iso-surface is of the shown field only (its colour is then the legend's); a difference of two runs has
  no iso-surface (said in the reason). The `speed` field and other seeds (`points`, `patch`) are the CLI's and the
  tool's, not the viewport's.
