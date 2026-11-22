---
type: concept
tags: [mesh, quality]
sources: [doc-mesh-quality, doc-meshing, doc-dynamic-mesh]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Mesh quality

The measures by which rcfd judges a mesh before a run: non-orthogonality, skewness, aspect ratio, volume ratio and an extended set (warp, face angles, ortho-skew, inverse orthogonal quality, size change, face edge-length ratio). Definitions, closed forms and registered checks are in ADR 0018 ([[doc-mesh-quality]]).

## How rcfd uses it

- **One report everywhere.** `rcfd check-mesh <mesh|case> [--json]`, service `mesh.check`, MCP `check_mesh`, the workflow task `check_mesh` and the `check` event of `rcfd run --check-only` return one `QualityReport` ([[doc-mesh-quality]]).
- **Pass/fail is the original fields only.** `n_cells`, `max_non_orthogonality`, `max_skewness`, `max_aspect_ratio`, `max_volume_ratio`, `warnings`, `errors` decide `ok` and exit code 4. The poor-mesh list (`poor_counts`, `poor_list`, `poor_warnings`) is separate "on purpose", so adding it changed no pass/fail behaviour ([[doc-mesh-quality]]).
- **Default thresholds of the poor-mesh list** ("rules of thumb, not guarantees"): warp 15 deg; face angle below 20 or above 160 deg; inverse orthogonal quality 0.9; ortho-skew 0.85; face edge-length ratio 100; size change 100; at most 10 entries per measure. Library users pass their own `Thresholds` to `rcfd_mesh::quality::check_mesh_with`; there are no case-level limits ([[doc-mesh-quality]]).
- **Angle formulation.** Non-orthogonality is evaluated as `atan2(|d x n|, d . n)` (was `acos(cos)`, which lost half the digits near orthogonal faces); shipped meshes changed by at most 1.21e-6 deg and no baseline hash was re-recorded ([[doc-mesh-quality]]).
- **`check_mesh` error threshold:** one FAN-01 C-grid mesh is "above the 85 deg error threshold of `check_mesh`" ([[doc-meshing]]).
- **Registered quality gates for fan passages** are typically non-orthogonality 70 deg / skewness 2.0 / aspect 300 (3000 for y+ 1 set-ups), later 85 / 4.0 / 1000 and 84.5 / 4.0 / 1000 for the camber-seam S constructions; most are missed. See [[fan-passage-mesh]] ([[doc-meshing]]).
- **After every mesh move** the moved mesh is validated; a failure stops the run with a located "non-positive volume" error ([[doc-dynamic-mesh]]).
- Tetrahedral meshes report the radius-edge fraction, minimum dihedral angle and minimum volume (`tet_quality`); the opt-in `TetOptions::quality_sweeps` (ADR 0104, default 0) improves them ([[doc-meshing]]).

## Related

- [[mesh-generators]]
- [[fan-passage-mesh]]
- [[rcfd-mesh]]
- [[known-miss]]

## Open questions

- The doc says the GUI does not show the extended measures; whether the Electron [[studio]] shows them is not stated in these sources.
