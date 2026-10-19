# Surface meshing and tetrahedral volume meshing (wave 19B, ADR 0030)

Library-level tools in `rcfd-mesh`; there is no CLI or case-file hook yet. F-232, F-131 and F-132 are `partial`.
All literature attributions below are from memory and unverified (see the ADR).

## Pipeline

```
TriSurface (soup or generator)  ->  diagnose / repair   (surface.rs, F-232)
                                ->  remesh with a size field   (surface_remesh.rs, F-131)
                                ->  tetrahedralize   (tetmesh.rs, F-132)
                                ->  TetMesh::to_mesh  -> rcfd_mesh::Mesh  (check_mesh, solver)
```

## Surfaces and diagnostics (`rcfd_mesh::surface`)

* `TriSurface { points, tris, tags }` with `from_soup`, `area`, `enclosed_volume`, `euler`, `append`, `set_tag`, `flip_all`.
  Triangle tags become patches later. Generators: `icosphere`, `box_surface`, `cube`, `torus`, `cylinder`.
* `diagnose(&surface, &DiagOptions)` returns `Diagnostics`: free, non-manifold and misoriented edges, duplicate and degenerate
  triangles, slivers, small edges, open loops, multi-connected vertices, component count, enclosed volume, area, and
  `is_closed_manifold()`.
* `repair(&surface, &RepairOptions) -> (TriSurface, RepairReport)` removes degenerate and duplicate triangles, orients consistently by flood fill
  (optionally flipping closed components outward), fills holes by ear clipping, compacts the vertex array.
* Not done: self-intersection detection, wrapping of leaky models, spike removal.

## Size fields and surface remeshing (`rcfd_mesh::surface_remesh`)

* `SizeField`: `Uniform(h)`, `Curvature { theta, h_min, h_max }` (size theta/kappa), `Proximity` (thickness based), `Box`
  (region of influence) and `Min` (combination). `curvature`, `thickness`, `feature_edges` and `vertex_sizes` expose the pieces;
  sizes are smoothed with a gradient limit (`RemeshOptions::gradient`).
* `remesh(&surface, &field, &RemeshOptions)` does split / collapse / valence flip / tangential smoothing, projecting vertices on the
  input surface (vertices stay on it to 1e-9). Feature edges (dihedral angle above `feature_angle_deg`, and tag boundaries) and
  corners are preserved. `edge_ratios` gives edge length over target size for checking.
* Inscribed-polygon effect: a curvature size with angle theta loses about `1 - sin(theta)/theta` of the volume of a round tube.
* Closest-point queries are brute force: suitable for a few thousand triangles.
* Not done: anisotropic sizes, geodesic size fields, hard/soft controls, wrapping.

## Tetrahedral meshing (`rcfd_mesh::tetmesh`, `rcfd_mesh::predicates`)

* `predicates::orient3d` and `insphere` are exact (floating-point filter, expansion arithmetic fallback).
* `tetrahedralize(&closed_surface, &TetOptions) -> Result<TetMesh>`: Bowyer-Watson Delaunay triangulation, boundary recovery by
  Steiner points on missing surface edges, interior classification, Delaunay refinement (radius-edge bound `max_radius_edge`,
  default 2.0, and optional `size`), then bistellar improvement (`sliver_q`) and a flat-boundary-tetrahedron fix. The report
  (`TetReport`) counts Steiner points, flips and splits. The input must be a closed manifold: otherwise an error such as
  "3 free edge(s)", "N non-manifold edge(s)" or "N misoriented edge(s)" is returned.
* `TetOptions::split_boundary` (default false) lets refinement split surface edges; it can exhaust the Steiner budget on small
  angles and is experimental. With it off, tetrahedra whose circumcentres encroach on the surface are left (see T5 torus).
* `TetMesh::to_mesh(&names)` builds an `rcfd_mesh::Mesh`: cells are tetrahedra, boundary patches follow the triangle tags (tag 0
  is "wall", tag k "wall<k>" unless named). `tet_quality` reports the radius-edge fraction, minimum dihedral angle and minimum
  volume.
* Measured: sphere and cube meshes have all tetrahedra within radius-edge 2.0; the coarse anisotropic torus does not (0.634 of
  tetrahedra, known miss); after surface remeshing it does. Cylinder in a box: 0.927, min dihedral 3.3 degrees (slivers remain).
* Solver smoke (T10): a Laplace problem with Dirichlet data on a 1277-cell unit-sphere tetrahedral mesh gives relative L2 error
  1.1e-2; a linear field on a cube mesh is reproduced to 5e-14.
* Not done: polyhedral dual conversion (F-102), sliver
  optimisation beyond flips, size fields inside the volume beyond a uniform `size`.

## Sweep, thin-volume and multizone hex meshing (F-235, ADR 0047)

`rcfd_mesh::sweep`: a quad base mesh with named boundary edge groups is swept along `Path::Line`, `Path::Revolve` (closed when
the angle is 2 pi) or `Path::NormalOffset` with prescribed layer counts and grading; `thin_volume` offsets a shell along its
vertex normals. Results are all hex, oriented and positive volume (`Mesh::validate`). Bases: `rect_base`, `annulus_base`,
`disc_base` (n x n core + four ring sectors that are exact 90 degree rotations, shared nodes identified structurally),
`cylinder_shell_base`; `pipe_mesh` is the case-tree entry (`pipe_sweep`). A base touching the revolve axis and an unassigned
boundary edge are rejected with located errors.

* Inscribed-polygon deficit: an N-gon section has volume `(N/2) sin(2 pi/N) R^2 L`, wall area `N 2 R sin(pi/N) L`; the relative
  deficit is about `(2 pi)^2 / (6 N^2)`, so volume converges at order 2 (measured 1.99-2.00 for Pappus and annulus).
* Pipe (R = 1, n = 17, 12 rings, 8 layers, 8840 cells): non-orthogonality 35.4 degrees, skewness 0.45, aspect 5.1, face angles
  50-135 degrees. Annulus (2560 cells): non-orthogonality ~0, skewness 2.1e-2 (the registered 1e-6 limit was a derivation
  error; kept as ignored `s8_annulus_skewness_known_miss`).
* Poiseuille: derived `u_c = G R^2 / (4 nu)`, `U_b = G R^2 / (8 nu)`. See the ADR 0047 addendum for the measured errors.

## Cartesian-core fill with snapped prism layers (F-234, ADR 0047, status partial)

`rcfd_mesh::octree::fill`: a 2:1 balanced octree (integer lattice, refinement within `band * h_f` of the surface, balance over
the 26-neighbourhood) keeps leaves whose eight corners are fluid at distance >= `margin * h_f`. A face next to finer leaves is
split into four and hanging nodes are inserted into every face loop, so transition cells are polyhedra and the mesh is
conforming. Inside/outside by ray-cast parity along three irrational directions (majority vote).

* Boundary layers: each core-interface polygon is extruded to its closest surface point (`T = |S(P) - P|`), sharing the core's
  inner nodes, so the interface is conforming (no non-conformal join). Layer heights (wall first, rescaled to sum to `T`):
  uniform `T/N`; aspect ratio `h1 = s/a`, growth from `sum h1 g^i = T` by bisection; last ratio `h_N = r s`,
  `T/h_N = (1 - q^N)/(1 - q)`, `q = 1/g`. Layer cells form cell zone `layers`.
* Sphere (r = 0.5) in [-2,2]^3, Lmax 3: 12656 cells (hex fraction 0.889), non-orthogonality 77.4 degrees (limit 70, known miss),
  skewness 0.61, aspect 11.4; volume error 4.2e-3 (Lmax 3), 1.2e-3 (Lmax 4), order 1.84.
* Cavity on a generated mesh matches Ghia within 0.008 (limit 0.03). Not attempted: flat-plate y+/Cf. Not run: sphere drag.

## Adaptation criteria, conforming refinement and polyhedral dual (F-102, ADR 0054, status partial)

`rcfd_mesh::adapt` (library only, no case-file hook). `AmrMesh` is a forest of octrees (quadtrees along an unrefined axis) over a
tensor box with arbitrary monotone base coordinates; children are exact halves on a finest-level integer lattice. A face next to
finer leaves is replaced by the finer leaves' faces and every face loop includes all existing lattice vertices on its edges, so
cells are hanging-node polyhedra, every cell is closed and every face is shared by two cells for any level distribution (the 2:1
face balance is optional and measured with `max_face_level_jump`).

* Criteria on a cell field: `gradient_indicator` (least-squares gradient magnitude, optionally scaled by cell length),
  `hessian_indicator` (Frobenius norm of the least-squares Hessian), `zz_indicator` (`eta = |g* - g| sqrt(V)`, `g*` the
  volume-weighted patch mean of the gradient), `region_indicator` (box, sphere, slab, distance to a `SurfaceIndex`).
* `MarkSpec` turns an indicator into `Coarsen/Keep/Refine` with a dead band (`coarsen_below < refine_above`, the hysteresis) and
  min/max level limits; `combine_marks` takes the finest request. A parent merges only when all children are leaves marked
  Coarsen (and, with balance, no neighbour ends more than one level finer).
* `AmrMesh::adapt(&marks, balance)` returns the new mesh, a `Transfer` and an `AdaptReport`. `transfer_field` supports `Injection`
  (conservative) and `Linear` (least-squares gradient reconstruction in the parent; exact for linear fields, conservative);
  coarsening is the volume-weighted mean. Measured: linear-field transfer error 6.7e-16; face level jump <= 1 with balance.
* Solve test (`rcfd-fv/tests/amr_solve.rs`): manufactured tanh-front diffusion problem. Adapted beats the coarse mesh with fewer
  cells than the fine mesh, but the registered "adapted within 3x of the uniform fine error" missed (3.71, ignored
  `a4_adapted_vs_fine_known_miss`); a wider-band set-up registered afterwards passes (`a4b`). The acceptance wording "adapted
  cavity matches fine uniform" was NOT tested on a cavity.
* `rcfd_mesh::polydual::tet_to_polydual`: one polyhedron per tet vertex (edge rings through tet centroids, boundary rings closed
  through boundary-triangle centroids and edge midpoints, boundary quads carry the triangle tag). Volume agrees with the tet mesh
  to about 1e-15 and patch areas are exact (Kuhn box 80 cells up to 16 faces, tetrahedralised icosphere 163 cells). Merge-based
  conversion, anisotropic criteria, target-node-count and edge-length-weighted criteria, precoarsening for multigrid are NOT done.
* Solver use: the adapted mesh is a general polyhedral `Mesh`, so FV solves run on it; non-orthogonal correction needs deferred
  iterations (ADR 0054 addendum 1).

## Turbomachinery post-processing (F-123, ADR 0054, status partial)

`rcfd_post::turbo` (library only). Cylindrical frame `TurboAxis` (origin, axis, theta reference).

* `span_surface` / `span_surface_cylinder`: marching-tet cut of `r - R_s(z)`, then every vertex moved radially to the exact radius;
  `SpanFamily::Curves` revolves `R_h + s (R_t - R_h)` (hub and shroud need strictly increasing z, so radial impellers are not
  supported and give an error). `streamwise_surface`: revolved hub-to-shroud segment (plane, cone or cylinder) clipped between hub and
  shroud and snapped in the meridional plane; normals point downstream. `reconstruct_360` rotates a passage surface.
* Averages: `circumferential_mean` (arithmetic mean over equispaced angles of a pitch or the circle; aliases a harmonic when the
  sample count divides it), `StationGrid::average` (Simpson in span times `r dl`): area-weighted `sum f A / sum A` and
  mass-flow-weighted `sum rho(u.n) f A / sum rho(u.n) A`; zero net mass flow is an error.
* Performance: pump head `dp0/(rho g)` and `eta = rho g H Q/(T omega)`, compressor `PR`, `tau`, `eta_is = (PR^k-1)/(tau-1)`,
  `eta_p = k ln PR/ln tau`, head and flow coefficients, `torque_on_fluid` from `wall_force_and_moment`, `PerformanceReport` CSV
  (17 digits) and escaped HTML.
* Checks are geometric and algebraic (closed forms to 1e-13 to 1e-14; cylinder area error 8.8e-5 at 48 and 2.1e-5 at 96 sectors).
  NO turbomachinery flow solution was computed, and nothing was compared with published pump or compressor data, so the roadmap
  acceptance (public case, literature values) is not met. Registered plane-area expectation missed (ignored
  `t2_plane_area_polygon_known_miss`, corrected test compares with the circular annulus).

## Axial-fan blade geometry and periodic-sector hex mesh (F-133 partial, ADR 0065)

`rcfd_mesh::fan` (geometry) and `rcfd_mesh::fan_mesh` (mesh); case hook `mesh.generator = "axial_fan_sector"` (docs/case-schema.md).
A lower-fidelity mesh of one blade passage. It is NOT validated for accurate fan performance prediction: nothing was compared with
a published fan, and the registered quality gates are missed (below).

**Parameters** (`AxialFan`, validated by `validate()` which returns every problem as `FanError { path, message }`, for example
`stations[1].chord`): blade count `n_blades`, `r_hub`, `r_tip`, `tip_clearance` (shroud radius `r_tip + tip_clearance`),
`axial_extent`, `z_le_root`, a section family and a table of radial stations (first at `r_hub`, last at `r_tip`, linear
interpolation) with `chord`, `stagger`, `camber`, `camber_pos`, `thickness`, `sweep` (axial shift of the leading edge) and `lean`
(tangential shift of the leading edge). Sections lie on cylinders and are described in the unrolled plane (z, s = r theta); the
chord frame is `e_c = (cos g, -sin g)`, `e_v = (-sin g, -cos g)`, `v > 0` is the suction side (smaller theta), positive stagger
turns the trailing edge toward -theta; rotation about +z, the rotor turns toward +theta.

**Section families.** NACA 4-digit with the closed-trailing-edge coefficient (`yt = 5 t (0.2969 sqrt x - 0.1260 x - 0.3516 x^2 +
0.2843 x^3 - 0.1036 x^4)`, camber `yc = m/p^2 (2 p x - x^2)` before and `m/(1-p)^2 ((1-2p) + 2 p x - x^2)` after the maximum-camber
point, the thickness added along the camber normal); and a double circular arc (camber arc of radius `R = (1/4 + m^2)/(2 m)`,
thickness arc of radius `Rt = (1/4 + h^2)/(2 h)`, `h = t/2`, same normal offset). The section area is
`int 2 yt sqrt(1 + yc'^2) dx` (the curvature terms of the normal offset cancel) and the blade volume `int A(r) dr` exactly
(`dV = dr ds dz` on cylinders).

**Blade surface** (`AxialFan::blade_surface(n_chord, n_span)`): closed, oriented triangulation; checks `check_watertight` (every
directed edge once and its reverse once, Euler characteristic 2). Measured (ADR 0065 addendum): area and volume converge with order
1.96 and 2.00 (80/24 to 320/96 points: volume error 7.3e-4, 1.8e-4, 4.6e-5; area 6.4e-5, 1.6e-5, 4.2e-6).

**Mesh** (`build_sector_mesh(fan, spec)`): one structured hex index space (i axial, j pitchwise, k radial), an H-grid with a slit.
Per radial layer a 2-D grid in (z, s); the lower seam follows the camber line inside the chord range and is axial outside, the
upper seam is the lower seam translated by the pitch `2 pi r / N` (so it is the lower seam rotated by exactly `2 pi / N`, matched
to round-off, 1.1e-16 m measured). The blade is cut by axial planes at chord fractions `trunc_le` and `trunc_te`: its leading and
trailing faces are flat and blunt (the mesh is for a thin blade with blunt edges, not a sharp-edge or rounded-edge resolution).
Cells inside the footprint are deleted; the blade patch is the set of faces next to deleted cells. Patches `inlet`, `outlet`, `hub`,
`shroud`, `blade` (both sides, both end faces, tip cap) and the rotational pair `seam_lo`, `seam_hi`
(`set_periodic_rotation(seam_lo, seam_hi, 0, +z, 2 pi / N)`); cell zone `rotor` = all cells. The tip gap is `n_gap` radial layers
without a slit. Section sheets must be monotone in z (located error otherwise) and the blade must fit between its seams.

**Boundary-layer stack.** `n_bl` cells of exact geometric growth `h1 g^m` on the hub, the shroud, the tip cap, both end faces and
both blade sides; the first cell height is given or from a target y+: `h1 = 2 y+ nu / u_tau`, `u_tau = U sqrt(Cf / 2)`,
`Cf = 0.0576 Re_c^(-1/5)` (a flat-plate estimate at the mid-span chord). On the blade sides the offset in the unrolled plane is
`h1 / cos(alpha)` so that the wall-normal distance there equals `h1`; the 3-D normal differs by the twist and lean rates. Measured
first-cell and growth deviation: 4.5e-13 relative (y+ 30 set-up) and 9.9e-12 (y+ 1 set-up, limited by the f64 coordinate
resolution, a spacing of 7.3e-6 at coordinates of 0.3).

**Quality (registered gates are missed).** Set-up A (y+ 30, 138 192 cells): maximum non-orthogonality 73.4 deg (gate 70), mean 12.5,
99th percentile 47.1, 2 707 faces above 50 deg and 23 above 60; maximum skewness 7.57 (gate 2.0); maximum aspect ratio 810 (gate
300). Set-up B (y+ 1, 239 472 cells): 89.8 deg, skewness 66, aspect 19 123 (gate 3000). The worst cells of set-up A are the first columns directly downstream of the trailing plane, in the band behind the blunt
trailing face at mid-span (aspect 810, skewness 7.6, non-orthogonality 73 deg): the band is only the trailing-face thickness
high there while the axial spacing is the boundary-layer first cell; the axial columns are sheared by about the stagger angle
against the blade seams. The cause was located by cell index; a topology that turns the grid lines with the blade (O-grid or
C-grid) is the likely remedy and was not attempted. With a single cell across the blunt edge faces (`n_mid = 1`) set-up A reaches 68.3 deg and skewness 0.75 (both inside the gates) and aspect ratio 405 (gate 300 missed); more cells across the blunt faces make the shear worse. Two generator fixes (smoothing of the pitchwise line displacement, opening of
the band toward the inlet and outlet) reduced aspect from 2 562 and non-orthogonality from 88 deg but not below the gates.

**Solver use.** The sector is accepted by `FvGeometry::new` (the existing rotational periodicity). The absolute-formulation `mrf`
source is refused on it (the blade and seam faces are not surfaces of revolution; located diagnostic on `sources.<name>.zone`), so
smoke solves use the relative-frame `rotating_frame` source (cases/axial_fan_sector). Measured: an empty passage with walls moving at the
inflow reproduces uniform flow to 1.2e-12 of the speed; mass over all patches is conserved to 1e-14 in the rotating passage, but
the mass flows of the two seam patches do not cancel exactly (relative 3.5e-6 to 2.7e-5), so `m_in + m_out` alone shows 2e-6 to 7e-5 of `m_in`.

## Axial-fan C-grid passage mesh (ADR 0068)

`rcfd_mesh::fan_cgrid::build_cgrid_mesh(fan, spec)`; case hook `mesh.fan.topology = "c_grid"` (docs/case-schema.md). The default `h_slit` topology
of ADR 0065 is kept. Per radial layer, in the unrolled (z, s = r theta) plane, three blocks: a C block around the blade and its wake (the wake cut is a
set of shared interior nodes), an F block from the inlet to the front edge, and a core block inside the blade (used only in the tip gap; the leading and
trailing columns collapse, the trailing-edge wedge is one polyhedral cell per layer). The blade keeps its true edges. Boundary-layer stacks are exact: `n_bl`
cells of `h1 g^q` along the central-difference wall normal; beyond the stack, pursuit lines run to targets on the seams and the front edge. The upper seam
is the lower seam shifted by the pitch with an identical node list, so the pair is rotated by exactly 2 pi / N.

Measured (ADR 0068 addendum has the full list). Periodicity: node rotation mismatch 1.1e-16 m, area vectors |S_hi + R S_lo| / |S_lo| 3.9e-13,
closure 8e-16, stacks 3.2e-13 (A') and 1.7e-11 (B', limit 4.9e-11). Quality: set-up A' (y+ 30, 134 766 cells) non-orthogonality 81.6 deg (gate 70, missed),
skewness 0.86 (gate 2.0, met), aspect 177 (gate 300, met); set-up B' (y+ 1, 593 250 cells) 83.7 deg (missed), skewness 4.85 (gate 2.0, missed), aspect 2651
(gate 3000, met). The small case mesh (11 536 cells) has 79.7 deg.

**Limits of the generator.** It is not robust against changes of the controls: of 144 combinations of `n_chord`, `n_nose`, `n_side` scanned (unregistered), 44
built; the others fold in the leading-edge fan (located error on `front_gap`) or tangle in a way the fold check does not see (located by the mesh validation).
The registered refinement studies (envelope and blade volume, blade area) could not be run for that reason. The oblique leading-edge fan at
`front_gap` 0.3 and the sagitta of flat facets against a thin first cell on annular walls are the likely causes of the non-orthogonality and skewness misses
(not isolated by a test). Not validated for fan performance prediction.
