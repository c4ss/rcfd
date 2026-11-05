# ADR 0184: M-FAN-B registration (surface import and meshing pipeline) and job J4, the OBJ and glTF surface readers (F-280)

Status: **registered 2026-11-04**, before any implementation result of this job and before any run. Amendments follow
this text, each written before the runs it covers; results go in an addendum. Not yet accepted by the maintainer.

The M-FAN-B pass condition (G9) is that a case meshes and runs the FAN-01 blade from a surface file with units, with
prism layers and a hash-keyed rebuild. At registration the library parts existed, but no case reached them, binary STL
lost patch names and no reader took a unit. This ADR fixes the acceptance limits A1 to A7 for step 5 and the checks R1 to
R12 for step 1, job J4, the OBJ and glTF surface readers. Addendum 1 reports that all registered checks pass at their
first run except R3, whose extra sub-case had a set-up error; no limit moved. No file written by a third-party exporter
was read; the only independent writer is rcfd's own F-117 GLB exporter. After step 1, OBJ and glTF surfaces can be read
in code with names and units, but no case or CLI command reads them yet. The acceptance (A1 to A7) cannot run until Q2
(the FAN-01 surface) and Q3 (the domain) are answered.

Rules and parents:

- Job J4, on local main edb4839 (worktree branch `j4-mfanb-readers`).
- Design: [`docs/designs/m-fan-b.md`](../designs/m-fan-b.md).
- Parent design: `docs/designs/unblock-roadmap-fan01-fea.md` (goal G9), which with ADR 0182, `docs/milestones.json` and
  roadmap items F-276 to F-283 is on branch `m0-gate-roadmap`, not yet on main.
- Number: 0182 is used on `m0-gate-roadmap`, 0183 is reserved for another job, 0173 to 0181 are on unmerged job branches;
  no branch has 0184 (checked over all local branches just before writing).

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):

- limits are fixed here before the registered tests and runs;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact;
- a defective set-up gets an amendment before the corrected run;
- defaults of existing code stay bit-identical: this job adds modules and registry entries only; no existing reader,
  writer, case key or result changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no new third-party crate (`Cargo.toml` and `Cargo.lock` unchanged), the crate graph stays
  acyclic, no commit trailers, nothing pushed.

Clean-room: the readers are written from the public Wavefront OBJ format description and the public Khronos glTF 2.0
specification, from memory; no other reader's source is consulted. Sources are recorded in `docs/provenance.md`.

## Context

G9 (M-FAN-B pass condition): *a case meshes and runs the FAN-01 blade from a surface file with units, with prism layers
and a hash-keyed rebuild.* The library parts exist (STL reader, surface diagnose and repair, remesh, tetrahedral mesher,
octree fill, prism inflation) but no case reaches them, binary STL loses patch names, and no reader takes a unit
(design, "The problem"). The parent design asks each milestone design to split its partly done items; the split is in the
design's table.

## Decisions

**D1. Milestone design.** `docs/designs/m-fan-b.md` defines goals B1 to B5, five steps that merge separately, the item
split, and open questions Q1 to Q4 for the maintainer (volume-mesh route, the FAN-01 surface file, the acceptance domain,
`geometry.variant`). Steps 2 to 5 each get their own registration ADR before their first run. Those ADRs may tighten the
acceptance limits A1 to A7 below; loosening one needs an amendment to this ADR that the maintainer accepts.

**D2. Step 1 output.** A reader returns `rcfd_io::common::SurfaceImport { surface: TriSurface, patches: Vec<String>,
notes: Vec<String> }`: coordinates in metres, tag `k` = `patches[k]`, patches in order of first appearance, only patches
with triangles. This is the input of `tetrahedralize` and `TetMesh::to_mesh` as they are.

**D3. Units.** The length unit is a required argument parsed by `rcfd_core::units::Unit::parse`; it must be a length with
zero offset and a finite positive factor. No default, for glTF too. `"m"` leaves coordinates bit-identical.

**D4. Boundary types.** The readers return names only and never call `common::guess_kind`.

**D5. OBJ rules.** As in the design (step 1, OBJ):

- `o` objects are patches (`default` before the first `o`, repeated names merge);
- `g` is ignored unless `ObjPatches::Groups` is asked for;
- polygons are ear-clipped in the plane of their Newell normal, with a fan from the first vertex and a note when no ear
  exists;
- texture, normal, smoothing, material, line, point and curve statements are ignored with a note;
- `surf` is refused;
- negative indices are relative;
- line continuation, CRLF and a byte-order mark are accepted.

**D6. glTF rules.** As in the design (step 1, glTF):

- `.gltf` with data-URI or sidecar buffers and `.glb`;
- version `2.x`;
- every `extensionsRequired` entry refused;
- sparse accessors refused;
- modes 4, 5, 6 read, 0 to 3 skipped with a note;
- `POSITION` must be `VEC3` float;
- indices unsigned 8, 16 or 32 bit;
- byte stride and bounds checked;
- one patch per node with a mesh (node name, else mesh name, else `mesh<i>`; equal names merge);
- node transforms applied, winding reversed under a negative determinant;
- node cycles refused;
- sidecar URIs resolved inside the file's folder only.

**D7. Welding.** Both readers merge bitwise-equal vertices after scaling (first occurrence in vertex order kept), drop
unused vertices, and report both counts in the notes.

**D8. Registry.** `obj` (`.obj`) and `gltf` (`.gltf`, `.glb`) join `rcfd_io::registry` as `Read` formats with
conformance cases; a `tamper` run of each must fail with the format name.

**D9. Out of this step.** The `geometry.*` case keys, the intake function, the CLI command and any change to
`docs/case-schema.md` belong to later steps.

**D10. Roadmap.** F-280 exists only in `docs/roadmap.json` on `m0-gate-roadmap`; main's file has 184 items and no F-280.
This job does not add the item on main (that would clash with the m0 roadmap track). The note to add when both have
merged is in the addendum.

## Registered checks, step 1 (job J4)

All fixtures are written by the tests (`crates/rcfd-io/tests/j4_surface_readers.rs`, and R11 in
`crates/rcfd-post/tests/j4_glb_reader_cross.rs`). "Equal" means bitwise equal unless a tolerance is given. The reference
cube is the unit cube with corners at 0 and 1, 8 vertices and 6 outward counter-clockwise quads.

| Id | Check | Limit |
|---|---|---|
| R1 | OBJ cube, unit `m` | 8 vertices, 12 triangles, one patch named as its `o` line; closed manifold (0 free, 0 non-manifold, 0 misoriented edges), Euler characteristic 2; area 6 and enclosed volume 1, each within 1e-14 |
| R1b | Same file, side 1000, unit `mm` | every coordinate equals the file value times `Unit::parse("mm").factor`; enclosed volume 1 within 1e-12 |
| R2 | Polygons | quads give 2 triangles each; a convex octagon gives 6; a non-convex planar L-shaped hexagon of area 3 in a tilted plane gives 4 triangles whose areas sum to 3 within 1e-12 and whose normals all point the polygon's way (positive dot with its Newell normal); a self-intersecting quad gives 2 triangles and a fan note, no error |
| R3 | Negative indices | the cube written with relative indices reads equal (`SurfaceImport` ==) to the positive-index file |
| R4 | Normals and texture coordinates | the cube with `vt`, `vn`, `s`, `mtllib`, `usemtl` lines and `v/vt`, `v//vn`, `v/vt/vn` references reads with the same surface and patches as the bare cube; the notes name the ignored statements |
| R5 | Several objects | a file with faces before any `o`, then `o a`, `o b`, `o a` gives patches `default`, `a`, `b` in that order and every triangle has its object's tag; a group-only file read with `ObjPatches::Groups` gives one patch per group name, and with the default one patch `default` and a note |
| R6 | glTF cube variants | (i) `.gltf` with a base64 data URI and `u16` indices; (ii) `.gltf` with a sidecar `.bin` through the resolver and `u32` indices; (iii) `.glb` with `u8` indices; (iv) positions interleaved with normals in one buffer view with `byteStride` 24 (positions at accessor `byteOffset` 0, normals at 12); (v) 24 vertices split at the face seams, indexed; (vi) non-indexed, 36 vertices; (vii) a quad as a strip (mode 5) and as a fan (mode 6). (i) to (vi) give the same set of triangles (coordinate triples, cyclic order kept) and tags as the OBJ cube, a closed manifold with Euler characteristic 2; (vii) gives 2 triangles of total area 1, both with the normal of the mode-4 quad |
| R7 | glTF node transforms | translation (1, 2, 3), rotation 90 degrees about z, scale 2: every vertex within 1e-12 of the transformed reference, volume 8 within 1e-12; the same as a `matrix` within 1e-12; a child node composes with its parent; scale (-1, 1, 1): volume +1 within 1e-14 and 0 misoriented edges; one mesh used by two named nodes gives two patches |
| R8 | Malformed input | each of the listed cases returns `Err` (never panics) and its message names the place: OBJ by `line N`; glTF by JSON path (for example `accessors[1]`); GLB by byte offset. OBJ: index 0; index past the end (positive and negative); a 2-vertex face; `v` with 2 numbers; a `nan` or `inf` coordinate; a non-numeric index; `surf`; no faces. glTF: version `1.0`; a required extension; an accessor past its view; a stride below the element size; an index not below the vertex count; `POSITION` as `VEC2` or as unsigned short; a bad base64 character; a missing sidecar; a URI with `..`; a sparse accessor; a node cycle; a triangle index count not divisible by 3; JSON that is not an object. GLB: bad magic; version 1; declared length not equal to the byte count; first chunk not JSON; a chunk past the end |
| R9 | No panic | `guard::fuzz_no_panic` over every prefix and 3000 mutations of the OBJ cube, the data-URI `.gltf` and the `.glb` |
| R10 | Round trip through `TriSurface` | (a) a tagged two-patch surface (a box and a translated icosphere) written to OBJ by the test with shortest-representation floats reads back equal in points, triangles and tags, names kept; (b) the box part with dyadic coordinates written to `.glb` by the test reads back equal; (c) the OBJ import, one `StlSolid` per patch, through `stl::write_ascii` and `stl::read_bytes`, gives per solid the same triangles as the import's patch |
| R11 | Independent writer | a box mesh's boundary patches exported by `rcfd_post::gltf::glb_patches` read by `rcfd_io::gltf`: triangle count equal to the exporter's count; welded closed manifold, Euler characteristic 2; area and enclosed volume within 1e-6 relative of the mesh's (positions are `f32`) |
| R12 | Registry | `obj` and `gltf` listed as supported; their conformance cases pass; with `tamper` both fail and name the format; the existing W27C registry tests pass unchanged |

Gate of this job: `cargo fmt -p rcfd-io -p rcfd-post -- --check`, `cargo clippy -p rcfd-io -p rcfd-post --all-targets
-- -D warnings`, `cargo test --release -p rcfd-io`, and the R11 test of `rcfd-post`. No other package changes. The
fast tier is not used (it is not on main yet).

## Registered acceptance of M-FAN-B (G9), for step 5

Fixed now, before steps 2 to 5 exist. The surface is the FAN-01 blade turned from `cad_model.igs` into a surface outside
rcfd (design Q2), in the CAD frame (axis +x), read with its stated unit. The domain is the one the maintainer picks in Q3.

| Id | Check | Limit |
|---|---|---|
| A1 | Units and placement | largest distance of the imported blade surface from the x axis within 0.5 mm of 247.5 mm (tip cap in `validation-data/reference/w34d_fan01_cad_sections.csv`), computed in metres |
| A2 | Intake | after repair: closed manifold (0 free, 0 non-manifold, 0 misoriented edges); every patch of the file present; total area changed by repair by at most 0.5 % |
| A3 | Volume mesh | `check_mesh` reports no error; no cell of non-positive volume; maximum face non-orthogonality at most 70 degrees (the target of ADR 0079) |
| A4 | Prism layers | on at least 95 % of the blade's wall faces the requested layer count is present and the first-layer height is within 10 % of the requested value (the F-108 tolerance); faces outside are listed in the mesh report |
| A5 | Run | the case reaches its configured end with finite fields and no divergence-guard stop; inlet and outlet mass flow differ by at most 1 % of the inlet flow at the end. No performance number is graded |
| A6 | Hash-keyed rebuild | a second run with nothing changed reuses the mesh (same hash, no rebuild); moving one vertex of the file, changing the unit, and changing each meshing setting in turn each change the hash and rebuild; the hash is in the run record |
| A7 | Patch diff and default | renaming one object reports one removed and one added patch; when a boundary condition names the removed patch the run is blocked and the message names the patch and the boundary-condition entry; a case without a `geometry` key keeps its case hash (`baseline_hashes.rs` passes unchanged) |

## Consequences

* After step 1, OBJ and glTF surfaces can be read in code with names and units, but no case or CLI command reads them yet.
* glTF positions are `f32` by the specification, so an imported glTF surface carries about 7 significant digits.
* The acceptance (A1 to A7) cannot run until Q2 (the FAN-01 surface) and Q3 (the domain) are answered.

## Addenda

### Addendum 1 (2026-11-04): step 1 results, job J4

Branch `j4-mfanb-readers`, release builds. All registered checks pass at their first run except R3, whose extra sub-case
had a set-up error (below). No limit moved.

| Id | Result | Verdict |
|---|---|---|
| R1 | 8 vertices, 12 triangles, patch `cube`, closed manifold, Euler 2; area 6 exactly, volume 1 - 1.1e-16; unit `m` coordinates bit-identical | pass |
| R1b | factor 1e-3, every coordinate bit-equal to file value times factor; volume 1 - 1.1e-16; units `""`, `s`, `degC`, `1`, `m/s`, `kg` refused | pass |
| R2 | octagon 6 triangles; L hexagon 4 triangles, area 3 (error 0), no inverted triangle, every centroid inside the L, no fan note; bow tie 2 triangles with a fan note | pass |
| R3 | relative-index cube equal to the positive-index cube | pass (see the set-up note) |
| R4 | same surface and patches; notes "1 mtllib, 1 s, 1 usemtl, 2 vn, 4 vt" | pass |
| R5 | patches `default`, `a`, `b`, tags 0, 1, 2, 1; groups give `left`, `right`; the default reading gives `default` with "3 g" in the notes | pass |
| R6 | (i) to (vi) equal triangle sets and tags, closed, Euler 2 (split cube: "16 vertices ... merged"; soup: 28); (vii) modes 4, 5, 6 each 2 triangles of area 1 with normal +z | pass |
| R7 | TRS worst vertex error 8.9e-16, volume 8; matrix equal within 1e-12; child composition exact; mirror volume 1 - 1.1e-16, 0 misoriented; two named instances give two patches, 24 triangles | pass |
| R8 | 12 OBJ, 18 glTF edits, 1 base64, 6 URI and 6 GLB cases return errors naming line, JSON path or byte offset; no buffers, JSON root not an object and truncated JSON return errors | pass |
| R9 | every prefix plus 3000 mutations of each fixture, no panic (accepted: OBJ 349 of 3192, glTF 140 of 4211, GLB 148 of 3681) | pass |
| R10 | (a) two-patch box and icosphere OBJ round trip equal (54 points); (b) GLB box equal; (c) STL solids equal per patch | pass |
| R11 | exporter GLB of a 3 x 2 x 2 box: 64 triangles written and read, 34 vertices after welding, closed, Euler 2; area error 0, volume error 4.4e-16 relative; same with a cell scalar | pass |
| R12 | `obj` and `gltf` supported, conformance passes; tampered: "obj: area 7.414... and volume 1.333..." and "gltf: area 5.618... and volume 0.833..."; W27C tests pass (4) | pass |

Gate: `cargo fmt -p rcfd-io -p rcfd-post -- --check` clean; `cargo clippy -p rcfd-io -p rcfd-post --all-targets --locked
-- -D warnings` clean; `cargo test --release -p rcfd-io`: 88 passed, 0 failed, 7 ignored (all 7 ignored before this job);
`rcfd-post` `j4_glb_reader_cross`: 1 passed. The same `rcfd-io` suite in a debug build (overflow checks on, fuzz
included): same counts. `rcfd-cli` `docs_site` (relative links in `docs/`): 2 passed, after the two links of the design to
files that exist only on `m0-gate-roadmap` were turned into code paths. `Cargo.toml` and `Cargo.lock` unchanged.

**Set-up note (R3).** A sub-case added to R3 beyond the registered text (a triangle written right after its vertices)
expected `f -4 -2 -3` after four vertices to give `[0, 3, 1]`; by the relative rule it gives `[0, 2, 1]`. The fixture was
corrected to `f -4 -1 -3` before the registered comparison was judged; the reader was not changed.

**Beyond the registration.**

1. A non-convex OBJ polygon with more than 1024 vertices is refused (ear clipping is cubic in the worst case; a convex
   polygon of any size is fanned).
2. glTF indices equal to the primitive-restart value of their type are refused (the specification forbids them).
3. An OBJ `v` line with more than 7 values is refused.
4. `gltf::read_file` and `obj::read_file` were checked once on files in a temporary folder (sidecar in a subfolder, unit
   `cm`).

**Not shown.** No file written by Blender, FreeCAD or another third-party exporter was read; the only independent writer is
rcfd's own F-117 GLB exporter.

**Roadmap note for F-280** (to add to `docs/roadmap.json` when `m0-gate-roadmap` has merged, since F-280 is not on main):
"Job J4 (ADR 0184): rcfd_io::obj and rcfd_io::gltf read OBJ, .gltf (data URI or sidecar buffers) and .glb into a TriSurface
with a required length unit and one patch per OBJ object or glTF mesh node; boundary types are never guessed; registered
checks R1 to R12 pass (crates/rcfd-io/tests/j4_surface_readers.rs, crates/rcfd-post/tests/j4_glb_reader_cross.rs). Not
tested on files from third-party exporters." Proposed status: done, with this ADR and the two test files as evidence.
