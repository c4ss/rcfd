---
type: concept
tags: [interface, io, formats]
sources: [doc-interchange-formats, doc-case-schema-registers, doc-case-schema-viscoelastic-structural]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Interchange formats and parser limits

The mesh, surface and result formats rcfd converts, reads and writes, all behind one set of hostile-input rules ([[doc-interchange-formats]]).

## How rcfd uses it

- Parser rules (ADR 0011 section 1): no panic on any byte sequence (every prefix of a valid file plus 3000 deterministic mutations); no allocation sized from an untrusted header count; hard caps "file 1 GiB, 50 million items, 4096 blocks"; located errors (`line N`, `byte offset N` / `record N`, JSON pointers for GLB); caps in `crates/rcfd-io/src/guard.rs` ([[doc-interchange-formats]]).
- `rcfd convert`: mesh -> STL / Tecplot / XDMF, STL re-encode, Plot3D grid -> mesh ([[doc-interchange-formats]]).
- Format registry `rcfd_io::registry`: 14 supported (vtu, vtp, vtk_legacy, openfoam, ensight, stl, tecplot, xdmf, plot3d, gmsh, fluent, nastran_deck, obj, gltf), each with a conformance round trip; cgns and hdf5 `NotSupported` ([[doc-interchange-formats]]).
- Unverified layouts are labelled as such: the EnSight `hexa8` node order and binary widths, the Fluent reader's source, several Nastran field layouts ([[doc-interchange-formats]]); see [[unverified]].
- Not delivered: HDF5 heavy data (F-119 stays `partial`), CGNS (F-103), Tecplot polyhedral / binary `.plt` ([[doc-interchange-formats]]).
- The structural load deck (`rcfd_io::nastran`) and viscoelastic library have no case keys ([[doc-case-schema-viscoelastic-structural]]).
- Registers (named cell masks) are documented on the same page ([[doc-interchange-formats]], [[doc-case-schema-registers]]).

## Related

- [[rcfd-io]], [[rcfd-post]], [[located-error]], [[provenance]]
