---
type: source
tags: [mesh, import, io, reference]
sources: []
origin: docs/mesh-import.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Mesh import: Fluent `.msh` / `.cas` (F-118)

**Origin:** [docs/mesh-import.md](../../docs/mesh-import.md) at `origin/main` 2e94e5ef
**Kind:** user reference page (79 lines)

## Summary

rcfd imports the mesh (not the case setup) from Fluent `.msh` and `.cas` files. Binary input is unverified against real files.

## Key points

- `mesh.source = "file"` with `mesh.file = "x.msh"` (or `.cas`) accepts Fluent, Gmsh and OpenFOAM meshes; family chosen from the first byte (`(` Fluent, `$MeshFormat` Gmsh). `rcfd check-mesh` and `rcfd convert` read the same files.
- Provenance: written "from memory of a public description of the section layout; which description is not recorded"; no vendor software or source used ([[provenance]], [[clean-room]]).
- Sections read: 0/1 (ignored), 2 (dimension), 10/2010/3010 nodes, 12/2012/3012 cell zones (shapes rebuilt from faces, so polyhedra work), 13/2013/3013 faces (mixed and polygonal), 45/39 zone names.
- Zone types map to wall / inlet / outlet / symmetry; unknown types become a generic patch whose kind is guessed from its name. 2-D meshes are extruded one layer of prisms of depth 1 (`frontAndBack` empty patch).
- Unused sections are listed (`note: Fluent sections not read: ...`), "Nothing is dropped silently".
- Refused with explicit errors: periodic zones and shadow faces, hanging-node parents, axisymmetric axis zones, gzip, HDF5 (`.cas.h5`).
- Binary layout "was not tested against a Fluent-written binary file"; little-endian assumed.
- Tests: `crates/rcfd-io/src/fluent.rs`, `crates/rcfd-cli/tests/fluent.rs`, `crates/rcfd-io/tests/fluent_public.rs` reading the OpenFOAM-11 tutorial `elbow.msh` (GPL, fetched, never vendored): 537 nodes, 918 triangles; extruded volume matches the shoelace area to 5e-16.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-io]]
- [[mesh-generators]]
- [[provenance]]
