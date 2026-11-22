---
type: source
tags: [mesh, quality, reference]
sources: []
origin: docs/mesh-quality.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Mesh quality report and mesh editing (F-233, F-231)

**Origin:** [docs/mesh-quality.md](../../docs/mesh-quality.md) at `origin/main` 2e94e5ef
**Kind:** user reference page (96 lines)

## Summary

What the mesh quality report contains, how its poor-mesh list relates to pass/fail, and the library mesh-editing operations. Definitions, closed forms and registered checks are in `docs/adr/0018-mesh-quality.md`.

## Key points

- One `QualityReport` from `rcfd check-mesh <mesh|case> [--json]`, service `mesh.check`, MCP `check_mesh`, the workflow task `check_mesh` and the `check` event of `rcfd run --check-only`.
- The original fields (`n_cells`, `max_non_orthogonality`, `max_skewness`, `max_aspect_ratio`, `max_volume_ratio`, `warnings`, `errors`) alone decide `ok` and the exit code 4. New `measures.*`, `poor_counts`, `poor_list`, `poor_warnings` are separate "on purpose, so adding it did not change any existing pass/fail behaviour". Infinity is written `1e30`.
- Default `Thresholds` ("rules of thumb, not guarantees"): warp 15 deg; face angle below 20 or above 160 deg; inverse orthogonal quality 0.9; ortho-skew 0.85; face edge-length ratio 100; size change 100; at most 10 entries per measure. Library: `rcfd_mesh::quality::check_mesh_with`, `rcfd_mesh::measures::compute_measures`.
- Not done: GUI display of the new measures; case-level limits for the poor-mesh list.
- Mesh editing (`rcfd_mesh::edit`, library only): `Affine` + `transform_mesh`, `merge_meshes`, `copy_by_offset`, `extrude_patch`, `repair_face_orientation` / `repair_mesh`, `merge_patches`, `separate_patch`. Not done: baffles, cell-zone merge/split, any CLI / case-tree / GUI entry.
- Post-hoc note: non-orthogonality is now $\operatorname{atan2}(\lvert \mathbf{d}\times\mathbf{n}\rvert,\ \mathbf{d}\cdot\mathbf{n})$ instead of $\arccos(\cos)$; ADR 0018 E1 invariance test renamed `e1_non_orthogonality_invariance` and un-ignored, limit 1e-9 untouched; shipped meshes change by at most 1.21e-6 deg; baseline hashes pass without re-recording; ADR 0018 not edited.
- `rcfd inflate <case|mesh> --patch p --layers n <sizing> --json` prints the `InflateReport` (F-108, ADR 0021); exit 4 on quality errors.

## Contradictions with the wiki

- Mentions "the GUI" twice (not showing measures, no GUI entry point). [[doc-architecture]] records `rcfd-gui` as removed (F-449); the Studio is now the Electron app. Stale wording within the repo docs, not a claim conflict.

## Pages this source informs

- [[mesh-quality]]
- [[rcfd-mesh]]
- [[rcfd-cli]]
- [[known-miss]]
