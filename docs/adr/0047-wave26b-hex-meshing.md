# ADR 0047: Wave 26B, sweep / thin-volume / multizone hexahedral meshing (F-235) and a Cartesian-core fill with prism layers (F-234, vertical slice)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave was written and
before any mesh of this wave was generated or any solver run. Limits are not changed, widened, replaced or deleted after a run.
A miss is reported as measured and the assertion moves to an ignored `*_known_miss` test with the registered limit intact; the cause
is diagnosed, not tuned away. A set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (measured
numbers, deviations) are appended at the end without touching a registered limit.

Scope. F-235 first (library sweep, thin volume, O-grid + core multizone, one case-tree generator `pipe_sweep`), then F-234 as a
time-boxed vertical slice (2:1 balanced octree fill, snapped hex-dominant layer shell, offset rules, cavity on a generated mesh).
Both items stay `partial` unless every roadmap acceptance point is genuinely met; F-234 is expected to stay `partial` (no sphere
drag run, no flat-plate y+/Cf run). Clean-room: first principles and published literature only; nothing from
vendor material. Other waves (26A transient solver, 26C post/export) work in the same tree; this wave touches only new
modules (`sweep.rs`, `octree.rs`) plus small additive hunks in `lib.rs`, `schema.rs`, `sim.rs`, docs.

## Audit of what exists (HEAD 4a5caa0)

* `gen.rs`: structured generators (`box_mesh`, `cylinder_ogrid`, `annulus_3d`, ...) all built through `Mesh::from_cell_faces`, which
  orients faces from the vertex average, pairs faces by sorted point set and handles polyhedral cells. `axis_coords(n, len, Grading)`
  gives the layer distributions (uniform, expansion, symmetric, wall layers).
* `inflate.rs` / `edit::extrude_core`: prismatic layers extruded from an existing mesh patch with `LayerSizing` (uniform, first +
  growth, aspect, last-ratio with a GIVEN first height). Not used for F-234 because the gap staircase-to-surface is not a prescribed
  thickness (see D6).
* `non_conformal.rs`: joins two PLANAR coplanar patches only. It cannot join a staircase to a curved layer cap, so the F-234 interface
  is made conforming by construction (D6); the non-conformal machinery is not used. This is disclosed, not hidden.
* `surface.rs` (`TriSurface`, `icosphere`, `box_surface`, `enclosed_volume`), `surface_remesh.rs::SurfaceQuery` (brute-force closest
  point). No point-in-solid test exists; added in `octree.rs`.
* Case tree: generator enum at `schema.rs` (`mesh.generator`), branch in `sim.rs::build_mesh`, option refusal in
  `check_mesh_options`. 3-D cyclic pairs are made inside the generator with `Mesh::set_periodic` (as `box_2d`). `physics.body_force`
  (steady algorithms) drives periodic flows; `mhd.rs` is the template for converged-solve settings.
* `docs/case-schema.md` fenced TOML blocks are parsed by `rules_w19c.rs`, so doc examples must be valid cases.

## Decisions, F-235

### D1 `rcfd_mesh::sweep`

* `BaseMesh`: quadrilateral surface mesh (`points`, `quads`, named boundary edge groups). Any boundary edge not in a group is an
  error; an interior edge used by more than two quads is an error. Constructors: `rect_base`, `annulus_base(n_theta, n_r, r_in,
  r_out, grading)` (polar, closed in theta), `disc_base(n, n_ring, r, core_fraction, grading)` (the multizone O-grid + core, D3),
  `cylinder_shell_base(...)` (a curved quad surface, for the thin-volume option).
* `Path`: `Line { dir, length }`, `Revolve { origin, axis, angle }` (angle `2 pi` closes with point ids modulo the step count, no
  seam duplicates, as `annulus_3d` does with `it % n_theta`) and `NormalOffset { thickness }` (vertex-normal offset of a possibly
  curved base, used for thin volumes). Not delivered: polyline / curve paths with a rotating frame (disclosed at the end).
* `SweepSpec { path, layers, grading, bottom, top }`: `layers >= 1` hexahedra through the sweep, station coordinates
  `axis_coords(layers, 1, grading)` (so the full `Grading` set, including `WallLayers`, is available), patch names for the two caps
  chosen by the caller. Patches: the two caps (absent for a closed revolution) plus one per base edge group, with the group's name
  and kind.
* A base that touches the revolution axis (any base point closer than `1e-9` of the base size) is a located error: it would create
  degenerate wedges and the mesh would not be all-hex.
* `thin_volume(base, thickness)` = one-layer `NormalOffset` sweep with caps named `base`/`top`.
* Every generated mesh is passed through `Mesh::validate` in the tests (closure, orientation, positive volume).

### D2 Discrete exactness targets

Boundary nodes on the circle and straight chords make the section an N-gon (area `N/2 R^2 sin(2 pi/N)`), whose deficit with respect
to the circle is `(2 pi)^2/(6 N^2)` relative (`2 pi^2/(3 N^2)`). The ADR compares against the circle where the roadmap asks for it
and against the exact polygon formulas where exactness is claimed. The equal-area-radius construction of ADR 0022 A3 is NOT used
here (it would change the geometry being compared); the geometry is the inscribed polygon and the deficit is quoted.

### D3 Multizone disc (O-grid + core)

`n x n` Cartesian core of half-width `a = core_fraction * R` (default 0.5) and four ring sectors. Sector 0 (the +x side) has nodes
`P_m + s_l (Q_m - P_m)` with `P_m = (a, -a + 2 a m/n)`, `Q_m = R (cos phi_m, sin phi_m)`, `phi_m = (pi/4)(-1 + 2 m/n)`, `s_l =
axis_coords(n_ring, 1, grading)`; the other three sectors are exact 90-degree rotations (`(x, y) -> (-y, x)`), and the shared
nodes are identified structurally (sector k, m = n is sector k+1, m = 0; l = 0 is the core boundary), not by coordinate tolerance.
Cells: `n^2 + 4 n n_ring`, all quadrilaterals, no degenerate cell. Core corners have valence three with angles 90, 135, 135
degrees. `n` odd puts a core cell centre exactly on the axis.

### D4 Case tree (minimal, additive)

New `mesh.generator = "pipe_sweep"`: circular pipe of radius `mesh.r_out`, length `mesh.lz`, along z; `mesh.nx` = core cells per
side (the pipe has `4 nx` cells around), `mesh.ny` = radial ring cells, `mesh.nz` axial layers, `mesh.grading` = last/first radial
ring cell ratio (1 = uniform; `Expansion`, first cell at the core interface), new `mesh.core_fraction` (float, default 0.5, range
[0.2, 0.8]) and new `mesh.periodic_z` (bool, default false; when true `zmin`/`zmax` are a cyclic pair like `box_2d`'s). Patches
`wall` (Wall), `zmin`, `zmax`. Unset sentinels equal the schema defaults. Located errors: `core_fraction` != 0.5 or `periodic_z`
with another generator (in `check_mesh_options`, path `mesh.core_fraction` / `mesh.periodic_z`); `nx`, `ny`, `nz` below 1 for
`pipe_sweep` (path `mesh.nx` etc.); `initial.method = "coarse"` with `pipe_sweep` already gives the existing located error.

## Registered checks, F-235 (all registered before any run)

Test meshes. PIPE: `R = 1`, `L = 1`, `n = 17` (odd), `n_ring = 12` uniform, `core_fraction = 0.5`, `nz = 8`; so `N = 68`, cells
`(289 + 816) * 8 = 8840`; polygon deficit `(2 pi)^2/(6 * 68^2) = 1.42e-3`. ANNULUS: `R1 = 0.5`, `R2 = 1`, `n_theta = 64`,
`n_r = 8`, `L = 2`, axial layers 5 with `Expansion(2)`; 2560 cells. REVOLVE: rectangle `r in [1,2]`, `z in [0,1]`, 4 x 2 quads.
PLATE: `20 x 10` quads of `0.1 x 0.1`, thickness `0.01`, one layer. SHELL: cylinder shell `R = 1`, 64 around, 8 axial of 0.125,
thickness 0.02, one layer.

* S1 all hex: every cell has six faces, each with four points, eight distinct points; cell counts exactly as above (pipe 8840,
  annulus 2560, plate 200, shell 512, revolve `4*2*48` for 48 steps).
* S2 `Mesh::validate()` Ok for every mesh; minimum cell volume > 0.
* S3 closure and patches: every internal face is in the face list of exactly two cells, every boundary face of exactly one; patch
  names are exactly the requested set with these face counts: pipe `wall = 68*8`, `zmin = zmax = 1105`; annulus `inner = outer =
  64*5`, caps `zmin = zmax = 512`; plate `base = top = 200` plus its four side groups; shell `base = top = 512`.
* S4 volume exactness (relative 1e-12): pipe `(N/2) sin(2 pi/N) R^2 L`; annulus `(N/2) sin(2 pi/N)(R2^2 - R1^2) L`; revolution
  `(N/2) sin(2 pi/N)(r2^2 - r1^2) H` for N = 48 steps and for a 90 degree partial sweep in 12 steps
  `12 * (1/2) sin(pi/24)(r2^2 - r1^2) H`; plate `2.0 * 1.0 * 0.01 = 0.02`; shell `(N/2) sin(2 pi/N)((R+t)^2 - R^2) H` with N = 64.
* S5 area exactness (relative 1e-12): pipe wall `N * 2 R sin(pi/N) L`; annulus inner/outer `N * 2 R sin(pi/N) L` of each radius;
  cap area = base area.
* S6 convergence to the analytic volume `pi (R2^2 - R1^2) L` (annulus with `n_theta = 16, 32, 64`, same n_r and layers): observed
  order between successive levels at least 1.9 (expected 2). Revolution against Pappus `2 pi r_bar A = 3 pi` with 24, 48, 96 steps:
  observed order at least 1.9.
* S7 layer counts exactly as prescribed: annulus axial station count = 6 distinct z values; last/first axial cell thickness =
  2.0 to 1e-12; revolve: number of distinct angular stations equals the steps; plate and shell: exactly one cell through the
  thickness (cells = base quads).
* S8 quality from the construction (no probe run): PIPE `check_mesh`: no errors, max non-orthogonality <= 45 degrees (the worst
  faces are the core-to-ring faces near the core corners, estimated 12 degrees), max skewness <= 0.5, max aspect (longest/shortest
  edge of a cell) <= 10 (largest edge 0.125 axial, smallest ring edge about 0.024); `measures.min_face_angle >= 40` and
  `max_face_angle <= 140` degrees (corner angles 90/135/135, so no degenerate or folded cell). ANNULUS: non-orthogonality <=
  0.01 degrees and skewness <= 1e-6 (symmetry of the polar grid: centres lie on bisectors), aspect <= 12 (0.549/0.049 = 11.2).
  PLATE: aspect <= 10.0001 (edge ratio 0.1/0.01); non-orthogonality <= 1e-6.
* S9 error handling: a revolve base touching the axis returns an error whose message names the axis; an unassigned base boundary
  edge is an error; layers = 0 is an error.
* S10 Poiseuille (the roadmap acceptance), through the case tree: `pipe_sweep` PIPE with `periodic_z = true`, `physics.nu = 0.1`,
  `rho = 1`, `physics.body_force = [0, 0, 1]`, `wall` no-slip, `simplec`, settings of `mhd.rs` (`h_operator = "from_solution"`,
  tight tolerances), no Symmetry walls, converged to a registered residual (`max du / u_c < 1e-9` over 200 iterations, cap 20000).
  Measured quantity: the u_z value of the core cell whose centre is on the axis (`|x|,|y| < 1e-12`, any z; all axial cells must
  agree to 1e-6 relative). Limit: relative error to the circle's `u_c = G R^2/(4 nu) = 2.5` at most 1 percent (expected geometric
  part 0.14 percent from the inscribed polygon, the rest discretisation). Also registered: bulk velocity `G R^2/(8 nu)` within 1.5
  percent; max |u_x|, |u_y| <= 1e-6 u_c.
* S11 case tree: `pipe_sweep` through `build_mesh` has the same `content_hash` as the library call with the same arguments;
  located errors of D4 at the stated paths; the baseline hash test file passes unchanged.

## Decisions, F-234 (vertical slice)

### D5 Octree and fill (`rcfd_mesh::octree`)

* Root grid of cubical cells over a box `[lo, hi]` (`roots = [a, b, c]`, the three spacings must agree to 1e-9), leaves stored as
  `(level, i, j, k)`. Refinement by a predicate (distance to the surface within `band * h_finest`, plus box regions); balancing is
  2:1 over the 26-neighbourhood (a leaf is split when any neighbour is more than one level coarser), iterated to a fixed point.
* Nodes live on the integer lattice of the finest level. A face of a leaf is emitted whole when the neighbour is the same or a
  coarser level, as four sub-faces when the neighbour is finer. After emission each face loop receives every registered lattice
  node lying on its edges (hanging nodes, applied recursively), so cells are topologically closed for `Mesh::validate`.
  Transition cells are therefore polyhedra; the rest are hexahedra (hex-dominant).
* Region classification of a leaf: `Region::Domain` (all kept), `ExteriorOf(&TriSurface)` (fluid outside a body),
  `InteriorOf(&TriSurface)` (fluid inside a closed surface). A leaf is KEPT when all eight corners have signed distance
  `sd >= margin - tol`, `sd` positive in the fluid, `tol = 1e-9 * domain size`; a corner that lies on the surface within `tol`
  passes (this is what keeps boundary cells of a box region). Distance uses a uniform-grid triangle index with a cutoff
  (`SurfaceIndex`), the sign comes from ray-casting parity along three generic (irrational-slope) directions with the majority
  taken, using the same grid in the ray-perpendicular plane. Not a winding number; documented limitation: surface must be closed,
  watertight and outward oriented (checked by `surface::diagnose` in the tests).
* Boundary faces of kept cells: a neighbour outside the box gives the box patch (default names `xmin..zmax`, caller may rename and
  set kinds, several faces may share a name); a neighbour that exists but is not kept gives the interface, which is the patch
  `body` (staircase) when no layers are requested.

### D6 Snapped layer shell (conforming interface)

With layers requested, each interface polygon (normally a quad: the band is refined to the finest level so interface cells all have
the finest size; any polygon with extra hanging nodes simply gives a prism over that polygon) is extruded to the true surface:
every interface node `P` has the closest surface point `S(P)` (`SurfaceQuery::closest`, so the true triangulated body), thickness
`T(P) = |S(P) - P|`, and layer node `P_k = P + (sum_{i<=k} h_i / T)(S(P) - P)` where the heights `h_i` of the column follow a rule.
The shell shares its inner nodes with the core, so the interface is conforming without the non-conformal machinery (which is
planar only). The outer cap is the patch `body`. `margin >= d_min = 0.75 h_f` guarantees `T >= 0.75 h_f` at every column, so no
column degenerates. Nodes of adjacent columns are shared, the cap-to-cap side faces pair by point set. For a convex body the
closest-point map is non-expansive, so columns do not cross; for non-convex bodies this is not guaranteed and a negative volume is
reported by `validate` as an error (limitation).

Rules for the heights of a column of thickness `T`, `N` layers and local lateral size `s` (= the interface quad's mean edge length):

* uniform: `h_i = T/N`.
* aspect ratio `a` (surface size over first height): `h_1 = s/a`, `h_i = h_1 g^(i-1)`, `g` the root of `h_1 (g^N - 1)/(g - 1) = T`
  (increasing in `g`, bisection on `g in [1e-3, 1e3]`, feasible iff `T > h_1`).
* last ratio `r` (last height over local surface size): `h_N = r s`, `h_i = h_N q^(N-i)`, with `q = 1/g`; `T/h_N = (1 - q^N)/(1 - q)`
  is increasing in `q` with range `(1, infinity)` so the root is unique (bisection on `q in [1e-6, 1e6]`, feasible iff
  `T > h_N`); the first-cell height is `h_1 = h_N q^(N-1)` and the growth ratio `g = 1/q`. Both follow from the prescribed
  last-layer height and the total thickness, which is the roadmap's "first-cell height from the last-ratio target, growth ratio
  from the total thickness". After the root, the heights are scaled by `T / sum(h)` (a round-off level correction), so the
  total is exact.

### D7 Cavity on a generated mesh

Lid-driven cavity: `Region::InteriorOf(box_surface)` of the unit square slab `[0,1]x[0,1]x[0,1/40]`, roots `40 x 40 x 1`, no
refinement, no layers, patches `lid` (ymax), `walls` (xmin, xmax, ymin), `frontAndBack` (zmin, zmax, Empty). This exercises the
whole fill path (classification, face emission, patches) but contains no refinement, no hanging nodes and no cut cell; a refined 2-D
cavity is impossible with cubical cells of a single z layer. Disclosed as such.

## Registered checks, F-234 (all registered before any run)

Test geometry. SPHERE in BOX: `icosphere(0.5, 4)` (2562 vertices, 5120 triangles), box `[-2,2]^3`, roots `8^3` (`h0 = 0.5`),
finest level `Lmax = 2, 3, 4` (`h_f = 0.125, 0.0625, 0.03125`), refinement band `3 h_f`, `margin = 0.75 h_f`, layers `N = 3`,
last ratio `r = 0.5` (main), plus uniform and aspect `a = 2` variants for validity.

* O1 octree: after balancing, every pair of 26-neighbour leaves differs by at most one level; leaves tile the box (sum of leaf
  volumes equals the box volume to 1e-12 relative); every interface polygon has four points at `Lmax = 3`.
* O2 point in solid: 4000 fixed pseudo-random points in the box; classification equals `|p| < R` for all points with
  `| |p| - R | > 0.005` (the icosphere sagitta is about 3e-4); 100 percent agreement, and agreement of the three ray directions for
  those points.
* O3 sphere in box valid at `Lmax = 2, 3, 4`: `Mesh::validate` Ok, minimum cell volume > 0, named patches `xmin..zmax` and
  `body` all non-empty, all patch face counts consistent (box faces count the sub-faces); every internal face is in two cells.
* O4 hex-dominant: fraction of hexahedral cells (six faces, four points each, eight points) at `Lmax = 3` at least 0.70; the layer
  shell is 100 percent hexahedra (interface quads); report the polyhedral transition count.
* O5 quality at `Lmax = 3`, last-ratio shell: `check_mesh`: no errors, max non-orthogonality <= 70 degrees, max skewness <= 4.0,
  max aspect ratio <= 100, max volume ratio <= 100 (the ADR 0018 default thresholds, with a tighter aspect than the 1000 default).
* O6 sphere volume: void volume `V_box - V_mesh` against `TriSurface::enclosed_volume` and against `(4/3) pi R^3`; relative error
  against the enclosed volume <= 3 percent at `Lmax = 3` and <= 1.0 percent at `Lmax = 4`; observed order between `Lmax = 3` and
  `Lmax = 4` at least 1.5. The error against the analytic sphere is reported (its icosphere part is 0.1 percent at level 4) with
  limit 3 percent at `Lmax = 4`.
* O7 offset rules exact to round-off: for 200 (T, s, N) triples per rule, uniform `|sum - T|/T <= 1e-12`; aspect ratio `h_1 = s/a`
  to 1e-12 and `sum = T` to 1e-12; last ratio `h_N = r s` to 1e-12, `sum = T` to 1e-12 and consistency
  `h_1 = h_N q^(N-1)`. On the built mesh (`Lmax = 3`, last ratio): for every column, `|P_N - P_0|` equals the independently computed
  closest-point distance to 1e-10 relative and the last layer height equals `r s` (`s` the mean quad edge length) to 1e-9
  relative, where `s` is the lateral size reported by the builder. Infeasible inputs (`T <= h_N`, `T <= h_1`) are errors with a
  message that names the rule.
* O8 spherical vessel (`InteriorOf`, `Lmax = 3`, same layers): valid, minimum volume > 0, `V_mesh` within 3 percent of
  `enclosed_volume`, body patch non-empty.
* O9 lid-driven cavity on the generated mesh (D7): Re = 100, 40 x 40 x 1, SIMPLEC, linear upwind, converged as the existing
  `cavity_re100_simplec` (`rcfd-fv/tests/cavity.rs`, 3000 iterations, tolerance 1e-7): at the tabulated Ghia centreline rows (the
  existing CSV files, Re 100 u and v), absolute error divided by the lid speed (1) at most 0.03 (3 percent of the lid speed, the
  F-028 convention; a relative error blows up at the zero crossings), using linear interpolation between the bracketing cell
  values. Also: 1600 cells and total volume `1/40` to 1e-12.
* O10 NOT attempted unless explicitly reached: flat-plate y+ within 10 percent and Cf within 3 percent of Blasius; drag of the
  sphere at Re 100 (not run). These stay unmet and are reported as such.

## References (status)

Poiseuille `u_c = G R^2/(4 nu)` and `U_b = G R^2/(8 nu)`: derived. Pappus' centroid theorem: standard result, unverified source.
Ghia et al. 1982 data: existing CSV files, "verified, two secondary copies" in `docs/reference-data.md`. Octree 2:1 balancing and
hanging-node face splitting, ray-casting parity: standard computational-geometry / mesh-generation practice, cited from memory and
unverified. No source was read for this ADR beyond the repository.

## Addendum (written after the runs): measured results, misses, deviations

The registered limits above are unchanged. Misses are ignored `*_known_miss` tests with the limit intact; corrected tests are new.

### F-235 measured (limit in brackets)

* Sweep checks S1-S7, S9 and S11 pass (`crates/rcfd-mesh/tests/sweep.rs`, `crates/rcfd-api/tests/pipe_sweep.rs`): Pappus
  volume order 1.996 / 1.999 (revolve), annulus volume order 1.992 / 1.998; all-hex, valid, positive volumes; case-tree mesh content
  hash equals the library mesh (8840 cells); located errors for `periodic_z`, `core_fraction` and `nx` below 1.
* Pipe quality: non-orthogonality 35.4 degrees, skewness 0.454, aspect 5.12, face angles 50.0-135.0 degrees. Annulus: non-orthogonality
  1.2e-12, skewness 2.1e-2, aspect 11.19.
* S8 MISS (`s8_annulus_skewness_known_miss`, ignored): annulus skewness 2.1e-2 against the registered 1e-6. Root cause: a
  derivation error in the ADR. On a polar block the cell-centre line and the face centre differ by the chord-to-arc offset of a
  curved cell, which is not zero for 64 azimuthal cells; the limit assumed a straight-sided cell. A diagnosis test records the value.
* S10 MISS (`s10_..._known_miss`, ignored): registered settings give a centreline error of 3.33 percent (limit 1). Root cause:
  the O-grid/core junction faces are 35 degrees non-orthogonal independently of refinement and the default
  `non_orthogonal_correctors = 0` leaves that error uncorrected; the error does not shrink with refinement (3.93, 3.46, 3.33, 3.30
  percent for nx = 5, 9, 17, 33; `pipe_sweep_diag.rs`).
* S10b (`non_orthogonal_correctors = 2`, registered 8-layer mesh) did NOT pass: the iteration is stable for about 400
  iterations, then an axial mode grows without bound (nz = 4 and nz = 8 diverge, nz = 2 is stable). The first two runs failed
  because the helper's convergence test read NaN as zero change; the helper now asserts finiteness. The cause of the growth is not
  isolated (a z-mode of the solver with the explicit non-orthogonal correction on the cyclic pair is suspected, not shown). The test
  is kept as the ignored `s10b_..._known_miss`.
* S10c (new corrected test, passes): same cross-section, nz = 2, `non_orthogonal_correctors = 2`, no-slip walls (no Symmetry):
  centreline error 0.134 percent (limit 1), bulk error 0.075 percent (limit 1.5), axial spread 0, transverse velocity 4.5e-21.
  Acceptance reading: the Poiseuille point is reached on the nz = 2 mesh, NOT on the registered nz = 8 mesh; the all-hex
  pipe/annulus point is reached.

### F-234 measured (status partial)

* O1-O4, O6-O9 pass: balanced tree, tiling exact; 12656 cells at Lmax 3 (hex fraction 0.889, layer shell 100 percent hex, 1400
  polyhedral transition cells); void-volume error 4.24e-3 (Lmax 3, limit 3 percent) and 1.18e-3 (Lmax 4, limit 1 percent), order
  1.843 (limit 1.5); offset rules exact to 1e-12, mesh columns thickness error 0 and last-layer error 7e-15; vessel 1.95e-3
  (limit 3 percent); cavity on the generated mesh max |u - Ghia| 0.00456, |v - Ghia| 0.00797 (limit 0.03).
* O5 MISS (`o5_non_orthogonality_known_miss`, ignored): non-orthogonality 77.4 degrees against 70 (skewness 0.61, aspect 11.4 and
  volume ratio 10.2 meet their limits). Root cause (diagnosis test): all 240 faces above 70 degrees are layer-to-layer faces of
  the snapped shell, where the extrusion direction (towards the closest surface point) varies between neighbouring columns;
  layer thickness ranges 0.049-0.150. Not tuned.
* Not delivered: sphere drag at Re 100 (not run), flat-plate y+/Cf (not attempted), anisotropic refinement, snapping of the core
  to the surface (the layer shell absorbs the gap). Conformity at the interface: the shell shares the core's interface nodes, so
  no non-conformal join is needed.

### Deviations from the plan

1. Poiseuille acceptance uses `non_orthogonal_correctors = 2` and nz = 2 (above), not the registered settings/mesh.
2. The layer lateral size `s` is the square root of the mean interface-polygon area (not the mean edge length).
3. The cavity O9 uses an unrefined 40 x 40 x 1 fill (a refined single-layer slab is impossible with cubic cells), so it exercises
   classification and face emission but not hanging nodes in a flow solve.
4. The 8-layer Poiseuille solve costs about 20 minutes, so the registered-mesh runs were limited.
