# Mesh quality report and mesh editing (F-233, F-231)

This page explains what the mesh quality report contains, how its poor-mesh list relates to the pass/fail result, and which
mesh editing operations the library offers. It is for users who check a mesh before a run and for library users who build or
repair meshes in code. Status in short: the extended measures are reported by the command line, the service and MCP, but the GUI
does not show them and their limits cannot be set from a case; the editing operations exist in the library only, with no command
line, case-tree or GUI entry point. The quality and editing sections each end with what is not done.

Definitions, closed forms and registered checks are in `docs/adr/0018-mesh-quality.md`; this page is the user view. For the
exact case keys of the mesh section, see [case-schema.md](case-schema.md).

## The quality report

`rcfd check-mesh <mesh|case> [--json]`, the service method `mesh.check` (also MCP `check_mesh` and the workflow task
`check_mesh`) and the `check` event of `rcfd run --check-only` return one `QualityReport`. The original fields
(`n_cells`, `max_non_orthogonality`, `max_skewness`, `max_aspect_ratio`, `max_volume_ratio`, `warnings`, `errors`, ...)
are unchanged and alone decide `ok` and the exit code 4. New fields:

| Field | Content |
|---|---|
| `measures.max_warp` | largest face warp, degrees |
| `measures.min_face_angle`, `max_face_angle` | smallest and largest interior face angle over all cells, degrees |
| `measures.max_ortho_skew` | largest ortho-skew (intra-cell alignment of face normals with centre-to-face vectors, `1 - min cos`) |
| `measures.max_inverse_orthogonal` | largest inverse orthogonal quality (`1 - min cos`, also counting centre-to-neighbour vectors); 0 is perfect |
| `measures.max_size_change` | largest neighbour-to-cell volume ratio (equals `max_volume_ratio`) |
| `measures.max_edge_length_ratio` | largest face edge-length ratio (longest over shortest edge of one face) |
| `poor_counts` | number of offending elements per measure (only measures with offenders) |
| `poor_list` | the worst offenders per measure, worst first: `{measure, element: face or cell, index, value, limit}` |
| `poor_warnings` | one text line per measure with offenders |

Infinity is written as `1e30`. The poor-mesh list is separate from `warnings` on purpose, so adding it did not change any
existing pass/fail behaviour. Default limits (`Thresholds`, rules of thumb, not guarantees): warp 15 deg, face angle below 20
or above 160 deg, inverse orthogonal quality 0.9, ortho-skew 0.85, face edge-length ratio 100, size change 100, at most 10
entries per measure. Library users can pass their own `Thresholds` to `rcfd_mesh::quality::check_mesh_with`; the raw measure
arrays are `rcfd_mesh::measures::compute_measures`, and the polygon functions (`polygon_warp`, `polygon_angles`,
`polygon_edge_ratio`, `cell_orthogonality`) work on plain coordinates, including degenerate ones.

Not done: GUI display of the new measures; case-level limits for the poor-mesh list (library `Thresholds` only).

## Mesh editing (library, `rcfd_mesh::edit`)

`Affine` (translation, rotation about an axis through a point, scaling, mirror, composition) with `transform_mesh`
(mirrors reverse the face loops so volumes stay positive); `merge_meshes` and `copy_by_offset` (coincident boundary faces
are stitched into internal faces, same-name patches and cell zones are combined, face zones are dropped);
`extrude_patch` (vector or planar-normal extrusion in layers; new patches `<patch>` for the cap and `<patch>_side`);
`repair_face_orientation` / `repair_mesh` (reverse face loops that point into their owner cell, on a raw topology that
`Mesh::from_topology` would reject); `merge_patches` and `separate_patch` (regroup boundary faces, remap face zones).

Not done: slit (baffle) creation, cell-zone merge or split, face zones through merge/extrude, repair of owner/neighbour
or face ordering, and any CLI, case-tree or GUI entry point for these operations.

## Post-hoc note: non-orthogonality angle formulation

The registered ADR 0018 E1 check (non-orthogonality unchanged by rigid and uniform maps to 1e-9) was first recorded as a known miss
(1.479e-6 degrees) because `compute_metrics` evaluated `acos(cos)` of the angle between the cell-centre vector and the face normal,
which has conditioning 1/sin(angle) and so loses half the digits for (nearly) orthogonal faces. It now uses
`atan2(|d x n|, d . n)`, the formulation the extended measures already used. This was the only `acos`-of-cosine geometric angle in
the mesh metrics. The definition is unchanged: it is the same angle, evaluated stably. Degenerate inputs keep their old values (zero
centre vector: 0 degrees; zero-area face: 90 degrees).

Effect, measured with `rcfd check-mesh --json` before and after on the shipped case meshes (channel variants, packed bed, rotating
channel, sod, wedge) and the Gmsh test meshes (hex, prism, tet): max and mean non-orthogonality change by at most 1.21e-6 degrees; no
warning, error or exit code changed (all exit 0, identical warning and error lists). The solver does not read this angle (the
`non_orthogonal_correctors` setting uses the geometric face vectors), and the baseline hash tests pass unchanged without
re-recording. The invariance test is now `e1_non_orthogonality_invariance` (renamed from `..._known_miss`, un-ignored; the
assertion and the 1e-9 limit are untouched); the worst change under the E1 maps fell from 1.479e-6 to 3.5e-13 degrees. ADR 0018 itself
is not edited: its results text records the miss as it was at the time.

## Inflation report from the command line (F-108, ADR 0021)

`rcfd inflate <case|mesh> --patch p --layers n <sizing> --json` prints the `InflateReport` of `rcfd_mesh::inflate::inflate_patch`
(first and last layer heights, thickness, growth, and the quality report of the resulting mesh) together with the cell counts; the service
method `mesh.inflate` returns the same document. Details and options are in [case-schema.md](case-schema.md), section "Wave 16W". The exit
code is 4 when the inflated mesh has quality errors. Mesh partitioning reports (`rcfd partition`) are described there as well.

## Where to go next

* [meshing.md](meshing.md): the mesh generators and their measured quality.
* [mesh-import.md](mesh-import.md): reading meshes from other formats.
