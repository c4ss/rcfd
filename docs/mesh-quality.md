# Mesh quality report and mesh editing (F-233, F-231)

Definitions, closed forms and registered checks are in `docs/adr/0018-mesh-quality.md`; this page is the user view.

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
