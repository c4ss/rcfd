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
