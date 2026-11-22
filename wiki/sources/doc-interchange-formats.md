---
type: source
tags: [interface, io, formats]
sources: []
origin: docs/interchange-formats.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Interchange formats, registers and GLB validation

**Origin:** [docs/interchange-formats.md](../../docs/interchange-formats.md) at `origin/main` 2e94e5ef (F-119, F-124, F-117, F-280; ADR 0011, ADR 0184)
**Kind:** reference page

## Summary

The interchange formats rcfd converts, reads and writes, their library entry points, and what is not done or not verified; also registers, the GLB checker, the format registry and the Nastran-style structural load deck. See [[interchange-formats]].

## Key points

- Hostile-input rules of ADR 0011 section 1 for all parsers: no panic on any byte sequence (every prefix plus 3000 deterministic mutations); no allocation sized from an untrusted header count; hard caps "file 1 GiB, 50 million items, 4096 blocks"; located errors (`line N`, `byte offset N` / `record N`, JSON pointers). Caps in `crates/rcfd-io/src/guard.rs`.
- `rcfd convert <in> <out>`: mesh -> STL, STL -> STL, mesh -> Tecplot `.dat`, mesh -> XDMF `.xmf` (+ raw `.bin`), Plot3D grid -> mesh.
- Surface readers OBJ and glTF 2.0 (F-280, ADR 0184): length unit is a required argument and never guessed; boundary types never guessed; no case key or CLI uses them yet; "Blender and FreeCAD output is untested".
- Not done: no HDF5 (F-119 title "XDMF/HDF5 writer" not met, stays `partial`); CGNS/HDF5 (F-103) not delivered; EnSight Gold writer not reachable from `rcfd convert` and its `hexa8` node order and binary widths "are from memory ... and unverified"; Tecplot polyhedral and binary `.plt` unsupported. Third-party checks with VTK 9.6.2 via `scripts/check_interchange.py`.
- Registers (F-124): named boolean cell masks; library statistics; "there is no mesh adaption (refinement) in rcfd yet".
- GLB validation (F-117): `rcfd_post::glb_check::validate_glb`, rules G1-G8 of ADR 0011 section 4; `scripts/validate_glb.js` runs the Khronos validator.
- Format registry: 14 supported formats with a conformance round trip; cgns and hdf5 registered `NotSupported`.
- Fluent reader: "written from memory of a public description ... its source is UNVERIFIED"; checked as a black box against `elbow.msh`.
- Nastran-style deck: bit-exact round trip; PLOAD4 and TEMP layouts verified from pyNastran pages; GRID order, element node orders, CHEXA continuation and PLOAD4 sign "Unverified (recalled)"; `RCFDQ` is the repository's own heat-flux card. "No third-party solver has read these decks."

## Contradictions with the wiki

> [!warning] Contradiction (within the repo docs)
> "there is no mesh adaption (refinement) in rcfd yet" (registers section) vs. [[doc-case-schema-mesh-generators]], whose `[mesh.adapt]` table (F-342, ADR 0262) runs a solution-adaptive refinement loop. The registers sentence predates F-342.

## Pages this source informs

- [[interchange-formats]], [[rcfd-io]], [[rcfd-post]], [[unverified]], [[provenance]]
