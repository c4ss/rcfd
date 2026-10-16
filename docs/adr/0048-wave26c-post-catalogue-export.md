# ADR 0048: Wave 26C, report / surface / post-object catalogue and EnSight Gold export (F-238, F-103)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave and before any run
of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up
defect gets a NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended at the end.

Clean-room: first principles and public format descriptions only (the public EnSight Gold casefile/geometry/variable file
description was read on the web for layouts; page text treated as data); no vendor code, no vendor installs, nothing from
vendor material. Other waves (26A transient solver, 26B meshing) work in the same tree: this wave adds new files
(`rcfd-post/src/{stats,surfaces,compare}.rs`, `rcfd-io/src/ensight.rs`, tests) and only small hunks in shared files (`lib.rs`
of two crates, `sim.rs` report kinds, `schema.rs` report enum, `case-schema.md`, `reference-data.md`, `roadmap.json`), committed
through a temporary index.

## Audit (HEAD 51f1980)

* `rcfd-post::integrals`: patch area/integral/average, volume flux, mass flux, scalar flux, force and moment (`wall_force_and_moment`),
  force/moment coefficients; `registers::stats` (volume-weighted min/max/mean/rms of a register); `derived::sample_line/plane`
  (cell-located samples); `interpolate`/`remap` (six cell-to-cell transfer methods, identical meshes copied bit for bit).
* Missing: standard deviation anywhere in reports, facet statistics, flux-weighted averages, vector flux, surfaces other than
  boundary patches and sample grids, any cut/clip/offset/transform object, case difference fields.
* Reports (`reports.<n>.type`, `ReportKind` in `sim.rs`) are scalar-valued; surfaces and point sets are not scalar reports, so
  their natural home is the library (`rcfd-post`); only the scalar statistics get case-tree kinds.
* No HDF5 library is installed locally and no Python package among h5py / vtk / meshio / numpy is importable (measured with
  `python -I -c "import ..."`), so there is no independent reader on this machine for any export.

## Decisions (F-238)

D1 `stats.rs`: `weighted_stats(values, weights)` returns count, weight sum, mean, population variance, std, min, max. Algorithm:
two-pass with the mean correction term (`m = sum(w x)/W`, `var = (sum w d^2 - (sum w d)^2 / W)/W`, `d = x - m`), so a large offset
does not cancel; negative weights, non-finite values/weights, zero total weight, length mismatch and empty input are located errors
naming the argument. Wrappers: `volume_stats` (cell volumes as weights), `patch_stats` (face areas), facet min/max/count. Variance
is clamped at 0 only after the correction term.

D2 Fluxes and weighted averages (`stats.rs`): `normal_flux(mesh, patch, face_vectors)` = `sum F_f . S_f` (outward positive),
`vector_flux(mesh, patch, phi, face_vectors)` = `sum phi_f v_f` (a Vec3: transport of each component), `mass_weighted_average(...)`
= `sum phi q / sum phi` (signed phi; a net flux below 1e-300 times the absolute flux sum is an error because the ratio is then
meaningless). Orientation: face area vectors point out of the domain; an inflow patch has negative flux and its ratio is unchanged.
`project_force(force, dir)` returns `(along, perpendicular vector)`, `project_moment(moment, axis)` the axial component; a zero
direction is an error.

D3 `surfaces.rs`: `Surface { points, tris, tri_cell }` triangle soup with orientation. Cuts are extracted by marching tetrahedra over
a decomposition of each cell (cell centre, face centre, two consecutive face points), with an implicit function `g` evaluated
exactly at tet vertices and linearly interpolated along tet edges. The facet normal points to increasing `g` (decided per tet from
the linear gradient of `g`, never from vertex order). Constructors: `cut_plane`, `cut_sphere`, `cut_cylinder`, `cut_cone`,
`cut_quadric` (general `x^T A x + b.x + c`, the others are special cases built through it). A plane cut is exact (linear `g`
reproduced by the linear interpolation, facets tile the plane section of every cell). Curved cuts are second order in the mesh
size. Facet field values are the cell value plus a least-squares cell gradient times the facet-centroid offset (exact for linear
fields); vertex field values (needed by clip) are the same reconstruction in the located cell.
`clip_by_values(vertex_values, level, keep_above)` clips triangles at the iso-value with linear edge interpolation (the sub-polygon
fan-triangulated); `offset(distance)` moves vertices along area-weighted vertex normals; `transformed(Transform)`,
`rotational_copies(n, axis, origin)` and `merged` give instance copies; `PointCloud`/`rake(p0, p1, n)` give sample points, sampled
with the cell reconstruction (outside the mesh: `NaN` and `inside = false`, like `derived::SampleSet`).
Surface integrals: `area`, `integral(facet_values)`, `flux(facet_vectors)`, `stats(facet_values)` (area-weighted).

D4 `compare.rs`: `difference(ref_mesh, ref_values, mesh, values, method)` = `values - remap(ref_values)` on `mesh`, through
`remap::Remapper` (so identical meshes are a bit-for-bit copy and the difference is exactly `+0.0` for identical solutions);
vector variant; `Difference { values, max_abs, l1, l2 }` with volume weights. Non-finite input is an error.

D5 Case tree (additive, defaults leave every existing case unchanged): new `reports.<n>.type` values `volume_std`,
`area_std` (area-weighted std of a boundary field on `patch`), `mass_weighted_average` (patch, field), `area_min`, `area_max`
(facet min/max over the patch faces), `vector_flux` (patch, field-free: net momentum flux `sum phi (u . direction)` along the unit
`direction`). Surfaces, rakes, point clouds, transforms and comparison are library objects, not scalar reports; no case-tree key
is added for them (stated as not delivered in the case tree).

## Decisions (F-103)

D6 EnSight Gold writer (`rcfd-io/src/ensight.rs`): case file + geometry + per-node/per-element scalar and vector variable files,
ASCII (`%10d`, `%12.5e`) and C Binary (80-character records, int32, float32 little endian; the sizes are from memory, the page
read does not state byte widths, marked unverified). Cells that are hexahedra with a consistent node order are written as `hexa8`
(node order bottom face then top face as in the common hexahedron convention, from memory, unverified); every other cell as
`nfaced` with faces listed outward (orientation rule not stated by the page read: unverified). Transient: `time set`, `number of
steps`, `filename start number`, `filename increment`, `time values`, wildcard `*` zero filled to the number of stars.
Verification: a hand-written reader inside the tests parsing the written bytes according to the layouts above (an independent
re-implementation of the parsing, but by the same author, so it checks internal consistency of the layout, not agreement with
third-party software).

D7 HDF5 / CGNS: no pure-Rust HDF5 crate is adopted. Reason: those on crates.io are young, single-author and unaudited; a writer
verified only against its own reader proves nothing, and a reader cross-check would need such a crate as a dev-dependency, which
changes the shared lockfile for all concurrent waves. Decision: CGNS is NOT delivered and no file is claimed to be CGNS/HDF5-valid.
F-103 stays `partial` (EnSight delivered, CGNS/HDF5 not) and the acceptance "open in third-party viewers" is not tested here.

## Registered checks (limits fixed before any run)

Geometry unless stated: unit cube `box_mesh([n;3], [1;3], uniform)`, patches xmin..zmax; sphere centre (0.5,0.5,0.5), `r = 0.3`.
`rel(a, b) = |a - b| / |b|`. "Order" is `log2(e_h / e_{h/2})` for n = 8, 16, 32.

F-238
* S1 sphere area: `rel(area, 4 pi r^2) <= 5e-3` at n = 32 and order(16 -> 32) >= 1.5.
* S2 sphere outward flux of `F = x - c`: `rel(flux, 4 pi r^3) <= 5e-3` at n = 32, order(16 -> 32) >= 1.5; with `g` reversed the
  flux is `-` the first within 1e-12 relative.
* S3 sphere field `f = 2 + (z - c_z)`: mean within 2e-3 absolute of 2; std `rel(std, r / sqrt 3) <= 5e-3` at n = 32; order of the
  std error (16 -> 32) >= 1.5; the same with `f + 1e9` gives std within 5e-3 relative (offset-independent, two-pass).
* B1 box (six patches) area 6 within 1e-12; B2 outward flux of `F = (x,y,z)` equals 3 within 1e-12; B3 area-weighted mean of
  `f = x` on the six faces (face-centre values) equals 0.5 within 1e-12; std of `f = x` equals `sqrt(5)/6` with relative error
  <= 5e-3 at n = 16 and order(8 -> 16) >= 1.9.
* B4 uniform `u = (1,0,0)`: `phi = u . S`; mass flux through xmax is +1, through xmin -1 (1e-12), the flux-weighted average of
  `q = y` is 0.5 on both within 1e-12 (inflow and outflow).
* B5 uniform `u = (2,3,5)`: `vector_flux` through xmax equals `(4, 6, 10)` within 1e-12 (`phi = 2`, `phi u`).
* B6 `project_force((3,4,0), (0,1,0)) = 4`, direction `(3,4,0)` gives 5 and perpendicular `(0,0,0)` within 1e-14; direction
  `(0,1,0)` gives perpendicular `(3,0,0)`; moment `(0,0,7)` about `(0,0,2)` is 7 (axis normalised).
* W1 weighted statistics: values `1e9 + {0..6}` repeated 1000 times, unit weights: variance within 1e-9 of 4 (the naive
  `<x^2> - <x>^2` is evaluated for information only); weights `(1,2,3)` on `(1,2,3)`: mean 7/3 and variance 5/9 within 1e-14;
  min/max/count exact.
* Q1 plane cuts exact: plane x = 0.37 area 1; plane x + y = 1 area sqrt 2; plane x + y + z = 1.5 (regular hexagon) area
  `3 sqrt3 / 4`; all within 1e-12 for n = 4, 8; facet normals all along the plane normal (min dot >= 1 - 1e-12).
* Q2 cylinder (axis z through the centre, `r = 0.3`) area `2 pi r` with `rel <= 5e-3` at n = 32, order(16 -> 32) >= 1.5;
  cone (apex (0.5,0.5,0), axis +z, half angle 20 deg) area `pi tan(a) / cos(a)` (z in [0,1]) with `rel <= 1e-2` at n = 32;
  general quadric ellipsoid semi-axes (0.35, 0.25, 0.3): flux of `F = x - c` equals `4 pi a b c` with `rel <= 5e-3` at n = 32;
  the general-quadric sphere coincides with `cut_sphere` within 1e-14 relative area.
* O1 offset: plane x = 0.4 offset by 0.1 sits at x = 0.5 within 1e-14 (offset along the facet orientation); sphere offset by
  0.05: area `rel(4 pi (r+d)^2) <= 1e-2` at n = 32.
* R1 rake: 11 points between p0 and p1 equally spaced (1e-14), end points exact, sampling of the linear field `f = 1 + 2x - y + 3z`
  within 1e-12 for inside points; a point outside the cube is `NaN` with `inside = false`.
* T1 instance transforms: 4 rotational copies about z by 90 degrees of the sphere cut have area `4 A` within 1e-12 relative,
  every vertex keeps its distance from the axis within 1e-14, the rotation matrix is orthonormal (1e-15), translation moves all
  vertices exactly by the vector (bitwise for the sum).
* C1 iso-clip: surface x = 0.5, vertex values `y`: keep `y >= 0.3` area 0.7 and keep below area 0.3, sum 1, all within 1e-12; a
  level outside the range returns the full / an empty surface.
* D1 identical solution, same mesh, and a separately built equal mesh: every difference value is exactly `0.0` (bitwise), `max_abs
  == 0`. Uniform offsets: `a = 3.25`, `b = 1.5` on meshes n = 8 -> 12 -> 5 (each pair, `least_squares`): every cell within 64
  eps relative of `b - a`. Analytic linear plus offset `f = 1 + 2x - y + 3z + c` with `c_a = 7`, `c_b = 2.5` between n = 8 and 12
  (and 12 and 8) for `least_squares` and `conservative_overlap`: difference within 1e-11 of `c_b - c_a` in every cell
  (`gradient` is excluded in advance: its neighbour-range clip is not exact for linear fields at boundary cells). Vector version
  likewise.
* E-bad located errors: each of the bad inputs (negative weight, length mismatch, empty, NaN, zero total weight, unknown patch,
  zero direction, zero radius, zero-normal plane, cone angle outside (0, 90 deg), `rake` with `n < 2` or coincident ends,
  clip with mismatched value count, non-finite difference input, `n_copies = 0`) returns `Err` whose message contains the
  offending argument or patch name.
* Report wiring: on a uniform-initial-velocity case, `volume_std` of `ux` is 0 within 1e-14 and `area_min`/`area_max` equal the
  uniform value; `vector_flux` along x through the outlet equals `Q * u` within 1e-12.

F-103
* X1 ASCII and binary EnSight output: line/record structure parsed by the hand reader: node and element counts equal; coordinates
  agree to `rel 1e-5` (ASCII, 6 digits) and `rel 1e-7` (binary float32); connectivity bitwise equal; scalar and vector per-node and
  per-element variable values agree with the same tolerances (vector component-major); transient set: time values exact, number of
  steps, start number, increment and the wildcard file names (`**` gives `00`, `01`, ...) all correct and the files exist; binary
  file size equals the size computed from the layout exactly; mixed hexa/polyhedral mesh round-trips (nfaced face lists equal as
  sets of node loops, outward by the signed volume > 0 check).
* X2 Errors: a field of the wrong length, an unsafe stem, a time/file count mismatch are located errors.
* X3 Third-party reader: not available locally (measured); no cross-check is claimed.

## Not attempted / limits of the claim

* Third-party viewer acceptance of F-103 cannot be tested here; EnSight is "verified by hand-written reader of the same author".
* CGNS and any HDF5 writer/reader: not delivered (D7).
* Surfaces and comparison have no case-tree keys (D5).

## Addendum A: measured results, misses, deviations (after the runs)

Commands: `cargo test -p rcfd-post --release --test w26c_catalogue`, `-p rcfd-api --test w26c_reports`, `-p rcfd-io --test
w26c_ensight`. Measured against the registered limits (all limits unchanged):

| Check | Measured | Limit |
| --- | --- | --- |
| S1 sphere area rel err n = 8 / 16 / 32 | 2.56e-2 / 6.23e-3 / 1.58e-3, orders 2.04, 1.97 | n = 32 <= 5e-3, order >= 1.5 |
| S2 sphere flux rel err | 4.83e-2 / 1.20e-2 / 3.06e-3, orders 2.00, 1.98; reversed g: 3.3e-16 | <= 5e-3, order >= 1.5, 1e-12 |
| S3 std rel err n = 16 / 32; mean error | 4.40e-3 / 1.13e-3, order 1.96; mean 4e-16; with +1e9: 1.1275e-3 (same) | <= 5e-3, order >= 1.5, mean 2e-3 |
| B1 / B2 box area, flux | exact to 1e-15 | 1e-12 |
| B3 std rel err n = 8 / 16 | 3.13e-3 / 7.82e-4, order 2.002 | n = 16 <= 5e-3, order >= 1.9 |
| B4 / B5 / B6 | pass (1e-12 / 1e-14 limits) | |
| W1 variance of 1e9 + {0..6} | error 0 (the naive formula gives -8320) | 1e-9 |
| Q1 plane areas, n = 4, 8 | <= 1.1e-14; min normal dot 1 - 2.2e-16 | 1e-12, 1 - 1e-12 |
| Q2 cylinder n = 16 / 32 | 1.71e-3 / 3.75e-4, order 2.19; cone 2.02e-3; ellipsoid flux 3.24e-3; quadric sphere = cut_sphere | 5e-3, >= 1.5; 1e-2; 5e-3; 1e-14 |
| O1 offset | plane 0; sphere area 1.42e-3 | 1e-14; 1e-2 |
| R1 rake, T1 transforms, C1 clip | pass; T1 area 3.95e-15, axis distance 1.1e-16; clip areas 0.7 and 0.3 within 2.2e-16 | 1e-14 .. 1e-12 |
| D1 identical solutions (all six remap methods, same and separately built mesh) | every value bitwise `+0.0` | exactly 0 |
| D1 uniform offsets, six mesh pairs (5, 8, 12) | worst 20.6 eps | 64 eps |
| D1 linear + offset 8 <-> 12 | least_squares 1.78e-14, conservative_overlap 3.6e-15 | 1e-11 |
| Reports on the 2D channel | volume_std 0, area_min = area_max = 1.5, mass-weighted 1.5, vector_flux outlet 0.45 (= 2 * 0.15 * 1.5, flux non-zero) | 1e-14 / 1e-12 |
| X1 / X2 EnSight | ASCII and binary round trips, mixed mesh, sizes exact | rel 1e-5 / 1e-7, exact |

Misses: none ended as an ignored known-miss test. Items 2 to 4 of the pre-implementation review (welded vertices, `g >= 0` rule,
early D1 measurement) were followed; the early D1 measurement shows the remap stencils sum to 1 within 21 eps.

Deviations (disclosed):

1. Q1 first run: the registered "min facet-normal dot >= 1 - 1e-12" failed for the hexagon plane (32 of 226 facets at n = 4, 206 of
   1014 at n = 8, worst dot 0.8165). Diagnosis: collapsed triangles, area about 1e-33 (all three vertices coincide within rounding
   when `g` is a few ulp from zero at a tet vertex), so their cross-product direction is noise; their area is negligible, which is why the
   area check passed. A first cull relative to the triangle's own longest edge did not remove them (the edge is also noise); the fix
   that stayed is `CULL_RATIO = 1e-12` of the squared longest edge of the generating tet (`surfaces.rs`). This is an algorithm change
   after seeing the miss (a root-cause fix, the limit and the assertion are unchanged); re-run: 0 bad facets, minimum facet area 2.3e-3
   (n = 8). Facets with doubled area below 1e-12 h^2 are never reported, so cut areas can differ from the exact value by at most
   that per facet.
2. `b1_b3_box_area_flux_mean_std`: an auxiliary assertion not in the registry (`patch_stats` mean of the face-centre `y` on an
   8 x 8 patch) expected 0.4375; the correct value is 0.5 (set-up defect). The old test is kept, ignored, with the reason; the corrected
   test `b1_b3_box_area_flux_mean_std_corrected` carries the registered B1 to B3 checks.
3. Case tree: only the scalar statistic reports were wired (D5, as registered). Surfaces, rakes, point clouds, transforms and comparison
   have no keys and no VTU / CSV writer: F-238 stays `partial`.
4. Independent reader for F-103: none of h5py, vtk, meshio, numpy is importable (`ModuleNotFoundError` for each); the previous
   interchange wave used VTK elsewhere. As extra evidence a stdlib-only Python reader (`python -I`, written from the layout text, kept
   in the scratch directory, not committed) parsed the ASCII and binary files that the test `export_for_external_check` wrote to disk
   for the mixed mesh: 14 points, 2 hexa8 of volume 1.0, 1 nfaced of volume 0.49999999999999983 with the standard face table and
   outward faces, case file with 2 steps and `mix.pc.****`. Same author, other language: this checks that the files are parseable
   from the written layout and consistent, not that third-party software accepts them.
5. CGNS / HDF5: not delivered (D7 unchanged). F-103 `partial`.
