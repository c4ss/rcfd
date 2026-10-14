# ADR 0011: interchange formats, cell registers and GLB validation (F-119, F-124, F-117)

Status: accepted, criteria registered before any code of this ADR was run.
Rule: no criterion below is changed, widened, replaced or deleted after results are seen. A miss goes to an
`#[ignore]`d `*_known_miss` test with the registered threshold and the measured value is recorded in
`docs/reference-data.md` and the roadmap note.

All formats are written from their public specifications (STL, Tecplot ASCII data format, Plot3D, XDMF, glTF 2.0).
No vendor install, binary or unpacked archive is read. Where a third-party reader is used to check conformance it is
named below; if it cannot be run, that is stated, not assumed.

## 1. Hostile-input rules (all new parsers: STL, Tecplot, Plot3D, XDMF reader helpers, GLB validator, registers)

H1. No panic, whatever the bytes. Test, per format and per parser: (a) every prefix of a valid sample file (all
    lengths 0..len) is parsed; (b) 3000 deterministic single-byte or small-burst mutations (xorshift64, fixed seed
    0x9E3779B97F4A7C15) of the valid sample are parsed; each call must return (`Ok` or `Err`) and not panic.
H2. No allocation scaled by an untrusted header count. Counts read from a header (triangles, nodes, elements,
    ni*nj*nk, accessor count) are checked, with checked arithmetic, against the bytes that remain before anything is
    reserved. Test: a 100-byte file whose header declares `u32::MAX` / `i32::MAX` items returns `Err` in under 1 s
    (and without the process growing by more than 64 MiB; the test only asserts the `Err` and the time).
H3. Hard caps: file size 1 GiB, 50 million triangles / nodes / elements / grid points per file, 4096 blocks, 4096
    tokens per header line. Exceeding a cap is an `Err` naming the cap.
H4. Located errors. ASCII formats report `line N` (1-based) and, when known, the token or keyword; binary formats
    report `byte offset N` (or `record N`). GLB reports a JSON pointer (`/accessors/3/count`). Test: a file with a
    garbled token on a known line 7 produces an error text containing `line 7`; a GLB with a bad count reports the
    exact pointer.
H5. Non-finite numbers (NaN, Inf) in coordinates are an error with location; in field data they are an error for
    registers and statistics (naming the first cell) and are written/read verbatim by the interchange writers only
    if the format can represent them (otherwise the writer refuses).

## 2. F-119 interchange formats

Scope delivered: STL (ASCII and binary, read and write, surface), Tecplot ASCII (write and read, FE zones of
triangle/quadrilateral/tetrahedron/brick, BLOCK and POINT packing, cell-centred variables, ordered I/J/K zones in
the reader), Plot3D structured (multi-block grid, ASCII and Fortran-unformatted binary, single and double
precision, either byte order on read, optional iblank; function files), XDMF (XML with Binary heavy data, writer).
HDF5 heavy data (the title's "XDMF/HDF5 writer") is NOT in scope of this ADR: it needs an HDF5 writer; the roadmap
item therefore stays `partial` whatever else passes. EnSight is not attempted.

Wiring: `rcfd convert <in> <out>`; output by extension `.stl` (patches chosen by `--patch a,b`, default all
non-empty boundary patches, `--ascii` for ASCII), `.dat` (Tecplot ASCII), `.xmf` (+ `.bin` next to it), input by
extension `.stl` (re-encode only), `.x`/`.xyz`/`.p3d`/`.grd` (Plot3D grid to a mesh with patches imin..kmax). Parser
errors are printed as `error: <file>: <located message>` with exit status 1.

### 2.1 STL
- S1 ASCII round trip of the vertices of a boundary patch is bit exact (max abs difference 0).
- S2 Binary round trip: per coordinate relative error <= 6.0e-8 (half a float32 ulp, 2^-24 = 5.96e-8); a coordinate
  that is 0 is exactly 0.
- S3 Patch geometry: total area of the written triangles equals the patch area to 1e-12 (ASCII) and 1e-6 (binary),
  for planar polygonal faces; the unit-box boundary (all 6 patches, 20x20x20 cells) has divergence volume equal to
  1 to 1e-12 (ASCII) / 1e-6 (binary).
- S4 Orientation: for every written triangle the computed normal has positive dot product with the outward face
  area vector of its source face; wrongly wound count = 0. The facet normal written equals the unit face normal to
  1e-12 (ASCII).
- S5 Welded read (exact bitwise vertex weld) of the box boundary (20^3 cells, all six patches): 21^3 - 19^3 = 2402
  distinct vertices, 6 * 400 * 2 = 4800 triangles, 0 boundary edges, 0 non-manifold edges, Euler characteristic
  V - E + F = 2 (E = 3 * 4800 / 2 = 7200).
- S6 Binary detection by exact size `84 + 50*n` is tried before the `solid` prefix test (binary headers may begin
  with `solid`). A binary header written by rcfd never begins with `solid`.
- S7 Malformed input: wrong facet size, vertex count != 3, missing `endloop`/`endfacet`/`endsolid`, truncated binary,
  declared count inconsistent with the length, non-finite vertex: each `Err` with location. Text after `endsolid` of
  the first solid is ignored only if it is whitespace; additional `solid` blocks are read as further solids
  (names kept).

### 2.2 Tecplot ASCII
- T1 Field round trip: cell-centred scalar and vector component fields of a mixed (tet/hex/prism/pyramid) and of a
  hex-only mesh are written, read back and equal bit for bit (max abs difference 0) in the cell order of the mesh;
  node coordinates bit exact; element count and node count equal.
- T2 Rebuilding a mesh from the zone gives cell volumes equal to the original to 1e-13 relative (sum of absolute
  differences / total volume) and total volume equal to 1e-13.
- T3 Both `DATAPACKING=BLOCK` and `POINT` are read and give identical data. `VARLOCATION=([n-m]=CELLCENTERED)` is
  honoured; the number of values read per variable equals nodes or elements as located, otherwise an error.
- T4 Cell shapes the writer cannot express (polyhedra with more than 6 faces, or faces with more than 4 nodes) are
  an error naming the cell, never a silently wrong file.
- T5 Reader errors: unknown keyword value, count mismatch, short data, non-numeric token (with `line N`).
- T6 Third-party check (if available): the VTK Tecplot reader opens the file and reports the same number of cells
  and the same cell-data range; if it is not installed or cannot parse it, that is reported as unverified.

### 2.3 Plot3D structured
- P1 ASCII double and binary double round trips of a 3-block grid (blocks 5x4x3, 2x2x2, 7x3x4) are bit exact;
  binary single is exact to 6.0e-8 relative; iblank integers exact; both little- and big-endian Fortran record
  markers are read (a big-endian file is produced by the test with `to_be_bytes`).
- P2 Precision and iblank are detected from the record length (per spec the record is 3*P*N or (3*P+4)*N bytes);
  a record length that fits no variant is an error with record number.
- P3 Grid to mesh: a 9x5x4 graded box block gives (8*4*3)=96 cells and volume equal to the box volume to 1e-12;
  6 patches imin..kmax, total boundary area equal to the box surface to 1e-12; the mesh passes `Mesh::validate`.
- P4 Function file round trip bit exact (binary double, ASCII).
- P5 "Reads a public sample": a Plot3D file obtained from a public repository (named, URL and SHA-256 recorded in
  `docs/reference-data.md`) is read and its block sizes equal the sizes stated by the independent reader (VTK's
  `vtkMultiBlockPLOT3DReader`) used as oracle. If no public file can be obtained, this criterion is reported as
  NOT demonstrated and the test is an `ignored` placeholder.

### 2.4 XDMF writer (XML + binary heavy data)
- X1 Heavy data are separate little-endian `.bin` files, `Float64` and `Int64`; the `Dimensions` product times
  `Precision` equals the file size of each referenced file exactly; values read back bit exact.
- X2 The XML is well formed (checked by a small tag-balance parser in the test) and every `DataItem` file reference exists next to the `.xmf` file; relative paths only, no `..`.
- X3 Mixed topology is written with the XDMF mixed-topology type numbers (triangle 4, quadrilateral 5, tetrahedron
  6, pyramid 7, wedge 8, hexahedron 9); cells outside these (polyhedra) are an error naming the cell.
- X4 Third-party check (if available): VTK `vtkXdmf3Reader` (or meshio) opens the file; cell count and a cell-data
  range equal the written ones. Otherwise reported as unverified. The acceptance wording "opens in third-party
  viewers" is demonstrated only for the readers actually run.

## 3. F-124 cell registers, regions, adaption marks, subdomain statistics

Library (`rcfd-post::registers`): a `Register` is a named boolean cell mask with constructors box, value threshold
on a cell field (`lo <= v <= hi`, both inclusive), expression of the cell centre (via the settings layer),
boundary adjacent (cells owning a face on the listed patches, then `layers - 1` grow steps), cell zone, and the
operations union, intersection, difference, complement, grow (face-neighbour dilation) and shrink.

Test mesh: unit cube, N x N x N = 20^3 uniform cells (h = 0.05, cell centres (i + 0.5) h).
- R1 Box [0.25,0.75]^3 selects exactly 1000 cells with volume 0.125 (relative error <= 1e-12).
- R2 Threshold on the field v = x with v >= 0.5 selects 4000 cells, volume 0.5 (1e-12); volume average of v over the
  region = 0.75 (1e-12); integral of v over the whole cube = 0.5 (1e-12); min and max of v in the region equal
  0.525 and 0.975 (1e-12).
- R3 Sphere by expression (x-0.5)^2+(y-0.5)^2+(z-0.5)^2 <= 0.3^2 on 40^3 cells: region volume within 3 % of
  4/3 pi 0.3^3 (voxel error expected O(h); the limit is registered a priori).
- R4 Boundary adjacent: patch xmin, layers 1 gives 400 cells; layers 2 gives 800; all six patches, layers 1 gives
  20^3 - 18^3 = 2168 cells. The interface area of the layer-1 xmin set (sum of areas of internal faces between a
  region cell and a non-region cell) is 400 faces * h^2 = 1.0 (1e-12).
- R5 Algebra: |A u B| + |A n B| = |A| + |B| for A = box, B = threshold; |not A| = N^3 - |A|; difference A\B =
  |A| - |A n B|; grow of the 10^3 central box by 1 layer is 12^3 = 1728 cells and shrink of that by 1 is 1000 cells.
- R6 Patching: `patch_field(values, mask, v)` changes exactly the cells of the mask (count = 1000 for R1) and leaves
  every other cell bit identical; the settings-tree patch `initial.patches` gives the same fields in `Simulation`.
- R7 Source restriction through the settings tree: `sources.<n>.zone = "register"` with `register = "<name>"`. For a
  porous source with uniform velocity the sink array is non-zero exactly on the register cells (count equal to the
  register count, zero elsewhere, bit exact zero).
- R8 Adaption-style marks: indicator = largest absolute face jump of a field between a cell and its face
  neighbours; mark when >= tau. For a step field (0 for x < 0.5, 1 for x >= 0.5) tau = 0.5 marks exactly the two cell
  columns next to the step, 2 * 400 = 800 cells; for the linear field v = x with tau = 0.5 no cell is marked.
  Marks restricted to a register mark only cells of the register. Output carries count and volume fraction.
- R9 Subdomain statistics per register: count, volume, min, max, volume-weighted mean, volume integral of a field,
  also for a vector field (component mean); empty register gives `None`, not NaN.
- R10 Errors (all located, setting path in the message): unknown register name in `sources`, `initial.patches` or a
  `combine` operand; cyclic combine; empty region where a source needs cells; inverted box (min >= max); expression
  that does not compile or is not finite; field length different from the cell count; NaN in a threshold field
  (first offending cell index); `layers` < 1; unknown patch for `boundary_adjacent`.
- R11 Registers are named objects in the settings tree: `registers.<name>` (schema documented, validated by
  `rcfd validate`, listed by `rcfd schema`).

## 4. F-117 GLB structural validation and external validator

New module `rcfd-post::glb_check` replaces the checks of `gltf::check_glb` (which now calls it). Rules, each a
distinct error with a JSON pointer, each with one negative test constructed by editing a valid GLB:
G1 header magic, version 2, length equals file length, chunk lengths 4-byte aligned, first chunk JSON, second BIN,
   BIN chunk length >= buffers[0].byteLength and < +4, no trailing bytes.
G2 `accessor.byteOffset % componentSize == 0` and `(view.byteOffset + accessor.byteOffset) % componentSize == 0`;
   `bufferView.byteStride` (when present) in 4..=252 and a multiple of 4, >= element size, only on vertex-attribute
   views; extent `offset + stride*(count-1) + elementSize <= view.byteLength` with checked arithmetic.
G3 view within buffer; `byteLength` >= 1; `count` >= 1; `type` and `componentType` valid; `normalized` only for
   integer component types; matrices not used.
G4 `min`/`max`: present and of length n for POSITION; for float accessors with data checked, min and max equal the
   data exactly (0 tolerance; JSON numbers are widened float32); `min <= max`.
G5 Indices: component type 5121, 5123 or 5125 only, element type SCALAR, all indices < POSITION count, none equal
   to the primitive restart value (255, 65535, 4294967295); triangle mode count % 3 == 0; strip and fan >= 3; line
   modes counts per spec; mode in 0..=6.
G6 Attributes: POSITION float VEC3; NORMAL float VEC3 with |n| = 1 within 1e-3 and finite; COLOR_0 VEC3/VEC4
   float or normalized unsigned byte/short; all attributes of a primitive have equal count; float data finite.
G7 References in range: scene, scenes[].nodes, nodes[].mesh, nodes[].children (acyclic, each node at most one
   parent), primitives[].material, indices, attribute accessor indices, accessor.bufferView, bufferView.buffer;
   `extensionsRequired` entries must be known (none are) else error; `sparse` accessors rejected as unsupported.
G8 No panic and no unbounded allocation on hostile input (H1, H2, H3) for the validator itself.
Registered measurement: every GLB produced by `write_glb` in the test-suite (box all-patches 4^3, 20^3 mesh, a
polygon-face mesh, no-scalar case) passes G1-G8 with 0 issues.
External: the Khronos `gltf-validator` (npm package `gltf-validator`, run with node) is run on the exported GLB of
a contour on a boundary: limit numErrors == 0 (warnings and infos are listed in the roadmap note, not limited,
because the registered acceptance is a validator that loads the file). No viewer is available here: "a viewer with
correct colours" is NOT demonstrated; only the stored per-vertex colour equals the colour map of its stored value
(existing test, unchanged).

## 5. Procedure

Tests are written first against these numbers, then run once; results are copied to `docs/reference-data.md`. A miss
is moved to an ignored `*_known_miss` test with the original assertion.
