# Mesh import: Fluent `.msh` / `.cas` (F-118)

`mesh.source = "file"` with `mesh.file = "x.msh"` (or `x.cas`) accepts Fluent files as well as Gmsh and
OpenFOAM meshes; the family is chosen from the first byte (`(` means Fluent, `$MeshFormat` Gmsh).
`rcfd check-mesh` and `rcfd convert` read the same files. The reader is written from the publicly documented
section layout; no Ansys text, tool or file was used.

## What is read

| Section | Use |
|---|---|
| 0, 1 | comments and header, ignored by design |
| 2 | dimension, 2 or 3 |
| 10 / 2010 / 3010 | nodes (ASCII, single and double precision binary) |
| 12 / 2012 / 3012 | cell zones (ids and names; shapes are rebuilt from the faces, so polyhedra work) |
| 13 / 2013 / 3013 | faces, uniform or mixed (`face type 0`, polygonal `5`), with the two neighbour cells |
| 45, 39 | zone names and boundary type names |

Zones map to patches by the type name in section 45 (or the numeric type id when no name is given):
`wall` to wall; `velocity-inlet`, `pressure-inlet`, `mass-flow-inlet`, `inlet-vent`, `intake-fan` to inlet;
`pressure-outlet`, `outflow`, `outlet-vent`, `exhaust-fan` to outlet; `symmetry` to symmetry; interior zones become
no patch; any other type (for example `pressure-far-field`, `interface`) becomes a generic patch whose kind is
guessed from its name. Cell zones become cell zones.

2D meshes (dimension 2, two-node faces) are extruded into one layer of prisms of depth 1 (Fluent's planar default;
`fluent::Options::thickness` changes it for library users). Front and back form the `frontAndBack` empty patch, as the
rcfd 2D generators do. `mesh.scale` applies to all three directions.

## What is reported or refused

- Sections the reader does not use (the setup sections of a `.cas` file: variables, materials, zone conditions and so
  on) are skipped and listed: `rcfd check-mesh` and `rcfd convert` print `note: Fluent sections not read: 33 x1, ...`
  on stderr; library users get `Report::ignored_sections`. Nothing is dropped silently.
- Refused with an explicit error: periodic zones and shadow faces (section 18, boundary types periodic and
  periodic-shadow), hanging-node parent faces, axisymmetric axis zones, a face that does not belong to a cell, cells that
  are not closed polygons (2D), gzip-compressed files (decompress first), and HDF5 files (`.cas.h5`).

## Not done

- HDF5 `.cas.h5` (the HDF5 container is not implemented); gzip.
- Periodic boundaries, hanging nodes, axisymmetric meshes, multi-region interfaces (non-conformal).
- Reading the case setup (materials, boundary values, models) from a `.cas`; only the mesh is imported.
- The binary section layout is implemented from memory of the public description and was not tested against a
  Fluent-written binary file (none was found among public sources); the tests use binary sections written by the
  test itself. Treat binary input as unverified until a real file has been read. Binary data are read as
  little-endian.

## Tests

`crates/rcfd-io/src/fluent.rs` unit tests (3D ASCII uniform and mixed faces, 2D extrusion, binary single and double,
skipped-section report, unsupported input), `crates/rcfd-cli/tests/fluent.rs` (check-mesh, convert and a laminar run
on a Fluent 2D channel), and `crates/rcfd-io/tests/fluent_public.rs`, which reads the real Fluent-written
`elbow.msh` (OpenFOAM-11 tutorial, GPL, fetched with `validation-data/fetch.py`, never vendored; the test skips when it
is absent): 537 nodes and 918 triangles as declared, five boundary zones plus `frontAndBack`, passes the mesh quality
check, and the extruded volume equals thickness times the shoelace area computed independently from the file's
boundary edges (relative difference 5e-16).
