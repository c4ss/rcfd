# Interchange formats, registers and GLB validation (F-119, F-124, F-117; ADR 0011)

This page lists the interchange formats rcfd converts, reads and writes, the library entry points for them, what is not done or
not verified for each, and related library features: registers (named cell sets), the GLB checker, the format registry and a
structural load-deck writer. It is for people who
move meshes and results between rcfd and other tools, or call the readers and writers from Rust. Some formats are only partly
delivered; the section "What is NOT done or not verified" comes right after the conversion table. For importing a mesh into a case,
see [mesh import](mesh-import.md); for the case keys that use registers, see the [case schema](case-schema.md).

All parsers share the hostile-input rules of ADR 0011 section 1: no panic on any byte sequence (tested with every
prefix of a valid file plus 3000 deterministic mutations), no allocation sized from an untrusted header count (counts
are checked against the bytes that remain before anything is reserved), hard caps (file 1 GiB, 50 million items,
4096 blocks) and located errors (`line N` for text, `byte offset N` / `record N` for binary, JSON pointers for GLB).
`crates/rcfd-io/src/guard.rs` holds the caps and the fuzz helper.

## `rcfd convert <in> <out>`

| Direction | Extension | Notes |
| --- | --- | --- |
| mesh -> STL | `.stl` | boundary patches only: `--patch a,b` (default: all non-empty, non-cyclic patches); binary unless `--ascii`; polygonal faces are fan-triangulated outward-wound |
| STL -> STL | `.stl` | re-encode only (ASCII <-> binary, vertex weld); an STL surface is not a volume mesh and cannot be converted to one |
| mesh -> Tecplot | `.dat` | ASCII FE zone (tri, quad, tet, brick; pyramids and prisms as degenerate bricks), `BLOCK` packing, mesh geometry only (no fields from the CLI; the library writes cell-centred variables); cells with more than 6 faces or faces with more than 4 nodes are refused with the cell number |
| mesh -> XDMF | `.xmf` | XML plus raw little-endian `Float64` / `Int64` `.bin` files next to it; mixed topology (triangle, quad, tet, pyramid, wedge, hex); polyhedra refused with the cell number |
| Plot3D grid -> mesh | `.x` `.xyz` `.p3d` `.grd` | `--block n` (1-based, default 1); the block becomes a hexahedral mesh with patches `imin ... kmax` |

Library entry points: `rcfd_io::stl`, `rcfd_io::tecplot` (also a reader for FE and ordered zones, BLOCK and POINT
packing, `VARLOCATION`), `rcfd_io::plot3d` (multi-block, ASCII and Fortran-unformatted binary, single / double
precision, either byte order, iblank, function files, headerless C-binary single-grid files via `read_grid_raw`),
`rcfd_io::xdmf` (writer).

## What is NOT done or not verified

* No HDF5 heavy data: the XDMF writer uses raw binary data items only. The title of F-119 ("XDMF/HDF5 writer") is
  therefore not met and the item stays `partial`.
* EnSight Gold (wave 26C, ADR 0048): `rcfd_io::ensight` writes the case, geometry and per-node / per-element scalar and
  vector variable files, ASCII (`%10d`, `%12.5e`) and C Binary, one part, `hexa8` for hexahedra and `nfaced` for every other
  cell, time series as a time set with `*` wildcard file names. Not reachable from `rcfd convert`. Verified only by a
  hand-written reader in the tests and a stdlib-Python reader of the files; no third-party viewer or library was available.
  The `hexa8` node order, the outward `nfaced` face orientation and the 4-byte little-endian binary widths are from memory
  (the public page read does not state them) and unverified. Structured parts, ids, ghost data and ASCII reals beyond five
  digits are not written.
* CGNS and HDF5 (F-103): not delivered. No HDF5 library is installed and none of h5py, vtk, meshio, numpy is importable locally
  (`python -I -c "import ..."` measured in wave 26C), so no layout could be checked against an independent reader; no file
  is claimed to be CGNS or HDF5 valid.
* Tecplot polyhedral zones and binary `.plt` are not supported.
* Verification against third-party readers was done with VTK 9.6.2 (`vtkSTLReader`, `vtkTecplotReader`,
  `vtkXdmfReader`, `vtkMultiBlockPLOT3DReader`) through `scripts/check_interchange.py` (meshio is installed but not used by the checks); no commercial
  viewer was available. VTK's Tecplot reader stores float32 coordinates, so a hand-built mixed mesh reads back with a
  volume 3.2e-9 relative off (script tolerance 1e-7).
* Plot3D function files and ASCII Plot3D were only checked against rcfd's own writer and VTK on small generated files.
* The public sample `bluntfinxyz.bin` (NASA blunt fin, hosted in the pyvista vtk-data repository; see
  `validation-data/manifest.json` and `docs/reference-data.md`) is read by `read_grid_raw` with the same block size
  and bounds as VTK. Turning it into a mesh with `block_to_mesh` is refused ("degenerate face": the grid has a
  collapsed edge), which is a recorded limitation.
* VTK's XDMF reader gives negative cell volumes for wedges listed in the VTK/Exodus node order unless the triangle
  winding is reversed, so the writer emits wedges as `[v0, v2, v1, v3, v5, v4]`; this was found by running the
  reader and is covered by the external check.

## Registers (F-124)

A register is a named boolean cell mask (`rcfd_post::registers::Register`) built from

* `type = "box"`: cell centres in `[box_min, box_max]` (inclusive; every component of `box_max` must exceed `box_min`),
* `type = "expression"`: `above <= expression(x, y, z) <= below` at the cell centre (limits inclusive, `+-1e300` means
  unbounded; lengths need units, for example `(x - 0.5 [m])*(x - 0.5 [m])`),
* `type = "boundary_adjacent"`: cells owning a face on the listed `patches`, grown to `layers` face-neighbour layers,
* `type = "cell_zone"`: a mesh cell zone,
* `type = "combine"`: `operation` union / intersection / difference (first operand minus the others) / complement over
  the registers named in `operands` (cycles are diagnosed),

then optionally dilated by `grow` and eroded by `shrink` layers with `neighbours = "faces"` (cells sharing a face) or
`"points"` (cells sharing a vertex). A 10^3 block grows to 1600 cells with face neighbours and to 12^3 = 1728 with point
neighbours.

Use in the settings tree:

* `sources.<n>.zone = "register"` with `register = "<name>"` restricts a momentum source to the register;
* `initial.patches.<n>` with `register`, `variable` (velocity, pressure, temperature) and `value` (3 numbers for velocity,
  1 otherwise) overwrites the initial field inside the register, in name order, after the uniform and expression
  initialisation; cells outside the register are not touched;
* every register is built against the mesh when the simulation is created, so a bad register is reported even when
  nothing uses it.

Post-processing in the library (not yet exposed as case reports): `stats` / `stats_component` / `stats_magnitude` give
count, volume, min, max, volume-weighted mean and integral of a cell field over a register (`None` for an empty
register, never NaN); `Register::interface_area` is the area of internal faces between the register and the rest;
`face_jump_indicator` is the largest absolute face jump of a field per cell and `mark_by_jump(values, tau, within)` marks
cells with indicator `>= tau` (optionally only inside a register) and reports the count and the marked volume
fraction. These marks are adaption-style flags only: there is no mesh adaption (refinement) in rcfd yet.

## GLB validation (F-117)

`rcfd_post::glb_check::validate_glb` implements rules G1-G8 of ADR 0011 section 4 (container, accessor and bufferView
bounds and alignment, exact min / max of position data, index range and primitive-restart values, normal length, colour
types, reference ranges, hostile input) with a JSON pointer in every error; `gltf::check_glb` delegates to it.
`scripts/validate_glb.js` runs the Khronos `gltf-validator` (npm, run with node) on exported files.

## Format registry and structural load deck (F-237, ADR 0051)

`rcfd_io::registry` lists every format with its direction and a conformance case: `registry::run_all()` writes a small
known data set per supported format, reads it back where a reader exists, and checks the values. Supported (12): vtu, vtp,
vtk_legacy, openfoam, ensight, stl, tecplot, xdmf, plot3d, gmsh, fluent, nastran_deck. Registered as `NotSupported` with a
reason: cgns and hdf5 (not delivered). A tampered data value makes the case fail with the format name.

Fluent `.msh` / `.cas` (reader `rcfd_io::fluent`): no public specification document is named for this format in the record. The
reader was written from memory of a public description of the section layout, so its source is UNVERIFIED; it was checked only as a
black box against the public `elbow.msh` sample (OpenFOAM tutorial, GPL, fetched, never committed). See
[mesh import](mesh-import.md) and the [provenance record](provenance.md).

`rcfd_io::nastran::Deck` writes and parses a free-field (comma separated) Nastran-style load deck in full double
precision (`{:?}` formatting, so the round trip is bit-exact): `GRID`, `CTRIA3`, `CQUAD4`, `CHEXA` (second line `,G7,G8`),
`PLOAD4,SID,EID,P1..P4` (blank P2..P4 = uniform), `TEMP,SID,G1,T1,G2,T2,G3,T3`. Parse errors carry `line N`.
Verified (pyNastran documentation pages, read): the PLOAD4 and TEMP field layouts. Unverified (recalled): GRID field order,
element node orders, CHEXA continuation form, free-field syntax in a given solver, and the PLOAD4 sign convention (here a
positive value is fluid pressure pressing on the structure). No third-party solver has read these decks; acceptance by
one cannot be tested here and is not claimed. Heat-rate (QBDY) loads are not delivered.

Point loads (ADR 0120): `FORCE,SID,G,CID,F,N1,N2,N3` and `MOMENT` (same fields, load = F N; written with CID 0 and F 1.0, any
other CID is refused on read). The field layout is recalled and unverified. `rcfd_post::rotordyn::load_deck` uses them for the
bearing and mount loads of a rotor-dynamics response: two static load sets per harmonic (real and imaginary part).

`rcfd_post::structural` maps wall pressure by conservative parameter-space overlap (x,y for planes, theta,z for cylinders
with 2 pi periodicity) and temperature through `remap` followed by a linear nodal recovery.

## Where to go next

- [Mesh import](mesh-import.md): reading external mesh files into a case.
- [Case schema](case-schema.md): the register and initial-patch keys of the case file.
- [Reference data](reference-data.md): where the public grid sample read above comes from.
