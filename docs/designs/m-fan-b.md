# M-FAN-B: surface import and meshing pipeline

*   **Author:** job J4 (agent session), for review by the maintainer
*   **Status:** Proposed
*   **Date:** 2026-11-04
*   **Parent design:** `docs/designs/unblock-roadmap-fan01-fea.md`, Part 2, milestone M-FAN-B (goal G9).
    That file, ADR 0182 (`docs/adr/0182-tiered-gate-and-nightly.md`), `docs/milestones.json` and roadmap items F-270 to
    F-277 are on branch `m0-gate-roadmap` and not yet on main, so they are named here as paths, not links, until that
    branch merges.
*   **Registration:** [ADR 0184](../adr/0184-m-fan-b-registration-and-obj-gltf-readers.md) fixes the limits of this
    milestone's acceptance and of step 1 (F-274) before any run.

## Overview

M-FAN-B is the second milestone in the build order of the parent design: after the EXP-ROT experiment, before the
rotor-only FAN-01 run (M-FAN-A1), which runs on the blade mesh this milestone produces.

Pass condition (G9, from `docs/milestones.json` on `m0-gate-roadmap`): *a case meshes and runs the FAN-01 blade from a
surface file with units, with prism layers and a hash-keyed rebuild.*

Items: F-273 (surface file as case geometry, hash-keyed rebuild), F-274 (OBJ and glTF readers), F-108 (prism layers),
F-132 (tetrahedral volume mesh), F-232 (surface diagnostics and repair), F-234 (Cartesian-core fill with prism layers).

**What this document details.** The whole milestone at the level of problem, goals, user journey and architecture; the
split of each partly done item into the part G9 needs and the part it does not; and a file-level plan for step 1 only
(F-274, the readers). Steps 2 to 5 each get a registration ADR, written before their first run, that fixes their file plan
and limits. The reason is the same as in the parent design: step 3's route (open question Q1) decides the files that step 4
touches.

**Proposals, not decisions.** Rules this document introduces (which OBJ statement makes a patch, how glTF nodes map to
patches, vertex welding, the order of steps, the acceptance domain) are proposals of job J4. Where the maintainer has
decided something, the decision is cited. The open questions at the end need the maintainer's answer.

## The problem

*   **The pieces exist but a case cannot reach them.** `rcfd-io` reads STL (`stl.rs`, F-119). `rcfd-mesh` has the surface
    pipeline as library code: `surface::diagnose` and `surface::repair` (F-232), `surface_remesh::remesh` (F-131),
    `tetmesh::tetrahedralize` and `TetMesh::to_mesh` (F-132), `octree::fill` with a snapped prism shell (F-234),
    `inflate::inflate_patch` (F-108), `polydual` (F-102). `docs/meshing.md` states: "there is no CLI or case-file hook for
    these yet".
*   **Patch names are lost on the common route.** A binary STL file holds one unnamed solid (`stl::read_bytes` returns one
    `StlSolid`). Boundary conditions are set per patch, so a blade, hub and casing exported to one binary STL arrive as one
    patch. OBJ objects and glTF nodes carry names, and both formats are exported by Blender and FreeCAD; rcfd reads neither.
*   **Units.** STL and OBJ carry no length unit. glTF defines metres. A CAD model in millimetres read as metres is 1000 times
    too large, and nothing would say so.
*   **Prism layers are not merged into a fluid mesh on unstructured walls.** `inflate_patch` builds an outward shell that is
    not part of a fluid domain (F-108 note). `octree::fill` makes a conforming mesh with prism layers, but only the sphere in
    a box was tried, and it missed the non-orthogonality limit (77.4 degrees against 70, known miss).
*   **The FAN-01 blade.** The FAN-01 mesh in use is a NACA-section fit to the published CAD, not the CAD itself. The CAD
    (`cad_model.igs`, Zenodo 10.5281/zenodo.20037409, CC BY 4.0) is not vendored and has no fetch entry
    (`validation-data/manifest.json`, entry `w34d-fan01-fau-eaa-axial-fan`). Only cylindrical sections derived from it are
    committed (`validation-data/reference/w34d_fan01_cad_sections.csv`).

## Goals

Each goal names the step that delivers it. A step merges on its own.

*   **B1. Readers (step 1, F-274).**
    *   `rcfd_io::obj` and `rcfd_io::gltf` read a surface into an `rcfd_mesh::surface::TriSurface` whose tag `k` is patch
        `k`, plus the list of patch names and a list of notes on what was ignored.
    *   The length unit is a required argument. It is never guessed, and glTF's metre is not assumed.
    *   Boundary types are never guessed: the readers return names only (`common::guess_kind` is not called).
    *   Malformed input gives an error with a line, byte offset or JSON path. No input makes a reader panic.
    *   No new dependency: `serde_json` is already a dependency of `rcfd-io`; base64 and the GLB container are written in
        the crate.
*   **B2. Surface intake (step 2, F-232 part).** One library function takes a file, a unit and repair options. It returns
    the repaired `TriSurface`, the patch names, the size in metres (bounding box), the diagnostics before and after repair,
    and the repair report. A surface that is not a closed manifold after repair is refused, with the counts of free,
    non-manifold and misoriented edges and the patches that hold them. The same report is printed by a CLI command, so a
    user can check an import before writing a case.
*   **B3. Volume mesh with merged prism layers (step 3, F-108, F-132 or F-234 part).** From a closed `TriSurface` and an
    outer domain, build one conforming `rcfd_mesh::Mesh` whose wall patches carry prism layers sized by first-layer height
    and growth (or a y+ target through `layers::first_cell_height_from_y_plus`), with the layers part of the fluid mesh. The
    route is open question Q1.
*   **B4. Case geometry and rebuild (step 4, F-273).**
    *   New case keys under `geometry` (parent design: `geometry.source = "surface"`, `geometry.file`, `geometry.units`,
        `geometry.variant`). Their schema text is written in step 4, not here.
    *   The mesh is rebuilt only when the hash of the file bytes, the unit and every meshing setting changes. The hash is
        recorded in each run.
    *   On re-import the patch list is compared with the previous import (added, removed, changed). A removed patch that a
        boundary condition names blocks the run, with a message that names the patch and the boundary-condition entry
        (CUJ 4 of the parent design).
    *   A case without a `geometry` key behaves as today. `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.
*   **B5. Acceptance (step 5, G9).** The FAN-01 blade, turned from the CAD into a surface outside rcfd, is meshed and run
    through the case keys of B4. ADR 0184 registers the limits (A1 to A7 there).

## Non-goals

*   CAD kernels: STEP and IGES are turned into surfaces outside rcfd (parent design).
*   Wrapping of leaky surfaces (F-232 remainder). It ships before the pipeline is called usable for downloaded models
    (parent design), but G9 runs on a closed CAD-derived surface and does not need it.
*   Writers for OBJ or glTF as user-facing export. The tests of step 1 write their own fixtures. `rcfd-post` already exports
    GLB for viewing (F-117).
*   glTF features that do not describe the surface: materials, textures, images, cameras, animation, skins, morph targets.
    They are ignored with a note.
*   Compressed glTF (Draco, meshopt, quantization) and sparse accessors: refused with a message (B1).
*   The as-manufactured variant (`geometry.variant = "manufactured"`, parent design G12).
*   Grading fan performance. M-FAN-B grades the pipeline; torque and power are graded in M-FAN-A1.

## Critical user journey

CUJ 4 of the parent design, unchanged: the user exports the FAN-01 blade from FreeCAD to OBJ in millimetres, one object per
boundary patch, edits it in Blender, points the case's geometry at the file with units `mm`, and runs. rcfd shows the size
in metres and the patch list, repairs the surface, meshes with prism layers and runs. After another edit it shows which
patches changed, and blocks the run if a patch that carries a boundary condition is gone.

Steps deliver it in this order: B1 reads the file; B2 shows the size, patches and repair; B3 meshes; B4 connects the case
and the rebuild and patch diff; B5 is the journey run on the real blade.

## Technical plan (architecture)

```text
 file (.stl / .obj / .gltf / .glb) + unit
        |
        v
 rcfd-io readers  ->  TriSurface (tag k = patch k) + patch names + notes          [step 1, B1]
        |
        v
 intake: weld, diagnose -> repair -> diagnose, size in metres, refuse if open     [step 2, B2]
        |
        v
 volume mesh with merged prism layers (route Q1) -> rcfd_mesh::Mesh, patches = names [step 3, B3]
        |
        v
 rcfd-api case tree: geometry.* keys, hash(file bytes, unit, settings) -> cached mesh,
   patch diff against the previous import, run blocked on a removed BC patch      [step 4, B4]
        |
        v
 FAN-01 blade acceptance run                                                      [step 5, B5]
```

### Step 1: readers (F-274), detailed below

The output type is the one the existing pipeline already takes: `tetrahedralize` reads a `TriSurface`, and
`TetMesh::to_mesh(&BTreeMap<u32, String>)` turns tags into named patches. `surface::repair` carries tags through. So a
reader returns:

```rust
pub struct SurfaceImport {
    pub surface: TriSurface,   // coordinates in metres; tag k = patches[k]
    pub patches: Vec<String>,  // in order of first appearance; only patches with triangles
    pub notes: Vec<String>,    // what was ignored or changed (counts, not one line per item)
}
```

**Units.** `unit` is a string parsed by `rcfd_core::units::Unit::parse`. It must have dimension length and no offset. Each
coordinate is multiplied by the factor (`"m"` leaves values bit-identical).

**Welding.** Both readers merge vertices whose coordinates are bitwise equal after scaling (`-0.0` equals `0.0`), keep the
first occurrence in vertex order, and drop vertices no triangle uses. Each count goes into the notes. glTF exporters split
vertices at normal and texture seams, so without welding a cube exported with flat normals has 24 vertices and 12 free
edges per face loop. Welding only merges identical points, so it never moves geometry.

**OBJ** (Wavefront OBJ, public description of the format):
*   `v x y z` with an optional `w` or vertex colours (4 to 7 numbers): `x y z` are used.
*   `f` with `v`, `v/vt`, `v//vn`, `v/vt/vn` references. Positive indices count from 1. Negative indices count back from
    the last vertex defined so far. Index 0, or an index past either end, is an error. Texture and normal references are
    checked to be integers and otherwise ignored.
*   A face with more than three vertices is triangulated by ear clipping in the plane of its Newell normal, so a non-convex
    planar polygon gives triangles that cover it. A polygon with no ear (self-intersecting or degenerate) is triangulated as
    a fan from its first vertex, with a note.
*   **Patches.** Each `o` object is one patch (parent design). Faces before the first `o` go to the patch `default`. A
    repeated object name adds to the same patch. `g` statements are ignored with a note, unless the caller asks for
    patches by group (`ObjOptions::patches = ObjPatches::Groups`), for files whose exporter writes only groups. With
    groups, a face takes the first name of its `g` line.
*   Ignored with a note: `vt`, `vn`, `vp`, `s`, `mtllib`, `usemtl`, `l`, `p`, `curv`, `curv2`, unknown statements.
*   Refused: `surf` (a free-form surface cannot be read as triangles; the message asks for a polygon export), a face with
    fewer than three vertices, a non-finite or non-numeric coordinate.
*   Lines may end with `\` to continue; CRLF and a UTF-8 byte-order mark are accepted.

**glTF 2.0** (Khronos specification, public):
*   `.gltf` (JSON) with buffers in base64 data URIs or in sidecar files; `.glb` (12-byte header, JSON chunk, optional BIN
    chunk; buffer 0 with no URI is the BIN chunk).
*   Sidecar files are loaded through a resolver closure. `read_file` resolves a relative URI against the folder of the
    `.gltf` file and refuses absolute paths, `..` segments and URI schemes other than `data:`.
*   `asset.version` must start with `2.`; any entry in `extensionsRequired` is refused with its name.
*   Triangles come from mesh primitives with mode 4 (triangles, the default), 5 (strip) or 6 (fan), indexed or not. Modes
    0 to 3 (points and lines) are skipped with a note. `POSITION` must be `VEC3` of `FLOAT`. Indices are `SCALAR` of
    unsigned byte, short or int, each below the vertex count.
*   Accessors: `byteOffset`, `bufferView.byteStride` (interleaved data), bounds of the view and buffer, all with checked
    arithmetic. An accessor with no buffer view is all zeros. Sparse accessors are refused.
*   **Patches.** The scene (`scene`, else scene 0, else every root node) is walked from its roots. Each node with a mesh is
    one patch, named by the node's `name`, else the mesh's `name`, else `mesh<i>`. Nodes with the same name add to the same
    patch, as repeated OBJ objects do. All primitives of the node's mesh go to its patch.
*   Node transforms (`matrix`, or translation, rotation and scale) are applied down the hierarchy. When the determinant of a
    node's world transform is negative the triangle order is reversed, so outward normals stay outward. A cycle in the node
    graph is an error.

**Registry.** `obj` and `gltf` join `rcfd_io::registry` as `Read` formats with a conformance case each, so
`registry::run_all()` covers them.

### Step 2: surface intake (outline)

A function in `rcfd-mesh` (or `rcfd-io`, decided in the step's ADR) composes reader, `diagnose`, `repair` and `diagnose`
again, and returns one report. The CLI prints it; it is the "size in metres and patch list" of CUJ 4. The refusal message
for an open surface lists, per patch, the free and non-manifold edge counts, so the user knows where to look.

### Step 3: volume mesh with merged prism layers (outline)

Two routes exist from library parts (open question Q1):
*   **Route T (tetrahedral).** Extrude prism layers outward from the wall triangles into the fluid (the sizing rules of
    `inflate.rs`), take the outer cap of the layers as an inner surface, tetrahedralize the region between it and the outer
    domain (`tetrahedralize` keeps surface triangles when `split_boundary` is off, so the cap is shared), and join the two
    meshes along the cap.
*   **Route O (octree).** `octree::fill`, which already makes a conforming mesh with prism layers that share the core's
    nodes (F-234).

### Step 4: case geometry and rebuild (outline)

*   Keys as in B4. The hash uses `rcfd_core::hash` over the file bytes, the unit and the meshing settings in a fixed order.
    The cached mesh and the previous import's patch list are stored with the case's run data, keyed by the hash.
*   The patch diff compares names, and for a kept name the triangle count and the area.
*   A run whose case has no `geometry` key follows today's code path. The case hash of such a case must not change.

### Step 5: acceptance (outline)

The FAN-01 blade surface (open question Q2) in the domain of open question Q3, meshed and run through the case keys,
graded by ADR 0184 limits A1 to A7.

## How the partly done items split against G9

| Item | Done part (on main) | Part G9 needs (in M-FAN-B) | Not needed for G9 (stays on the item) |
|---|---|---|---|
| F-274 | nothing | all of it (step 1) | nothing |
| F-273 | nothing | all of it (steps 4 and 5) | nothing |
| F-232 | diagnose; repair of seeded-defect sphere and cube to closed manifolds, volume within 0.5 % | intake report and refusal of an open surface (step 2) | self-intersection detection, spikes, leak-tolerant wrapping, a public defective surface set, exact defect counts on it |
| F-108 | block-mesh wall layers; solved y+ on the flat plate (ADR 0094); `inflate_patch` outward shell with sizing rules | prism layers on unstructured wall patches merged into the fluid mesh, first-layer height checked on the merged mesh (step 3) | a schema report surface for layer metrics beyond what step 4 prints |
| F-132 | tetrahedralization of closed surfaces with tags to patches; opt-in quality pass; polydual (F-102) | volume mesh of an imported closed surface with boundary layers, if route T (step 3) | sliver guarantees, the superlinear cost of the quality pass |
| F-234 | octree fill with snapped prism shell; sphere in a box valid; cavity within 0.008 of Ghia | the fill on the blade, if route O (step 3) | sphere drag at Re 100, flat-plate y+ and Cf, the 70-degree non-orthogonality known miss on the sphere |

Each step's ADR repeats the split for its items, with the item notes to add to `roadmap.json` when it merges.

## Alternatives considered

### Alternative A: STL only

*   **Pros:** the reader exists and is checked against VTK's reader (F-119).
*   **Cons:** a binary STL file has one unnamed solid, so patches are lost. Multi-solid ASCII STL keeps names but is a
    less common export option.
*   **Reason for rejection:** CUJ 4 needs one patch per object, which OBJ and glTF carry.

### Alternative B: third-party crates for OBJ and glTF

*   **Pros:** maintained parsers with wider coverage of the formats.
*   **Cons:** a new dependency changes `Cargo.lock`, which makes the fast tier of ADR 0182 select every package, and the
    house rules of ADR 0130 allow no new third-party crate without a decision. The subset needed (triangles, names,
    transforms) is small.
*   **Reason for rejection:** both readers are written in the crate from the public specifications.

### Alternative C: patches from OBJ groups (`g`) by default

*   **Pros:** some exporters write only groups.
*   **Cons:** the parent design states that each OBJ object is one patch.
*   **Reason for rejection:** objects are the default; groups are an option of the reader.

### Alternative D: one glTF patch per primitive

*   **Pros:** in Blender, primitives follow material slots, so a user could mark patches by material.
*   **Cons:** primitives have no name in glTF 2.0, so patch names would have to be made up from material names or indices.
    A Blender object becomes a node with its name, which matches the OBJ rule.
*   **Reason for rejection:** one patch per node. Material-based patches can be added later as an option.

### Alternative E: guess boundary types from patch names

*   **Pros:** the Fluent and Gmsh readers do it (`common::guess_kind`), so cases need fewer lines.
*   **Cons:** a guess can set a wrong boundary type silently.
*   **Reason for rejection:** F-274's acceptance says boundary types are never guessed. The case names them.

### Alternative F: no welding in the readers

*   **Pros:** the surface keeps the file's vertex numbering.
*   **Cons:** glTF splits vertices at seams by design, so most exported closed bodies would arrive with free edges, and
    `tetrahedralize` refuses a surface with free edges.
*   **Reason for rejection:** bitwise welding merges only identical points, so it changes numbering, never geometry.

## Non-functional requirements

*   **Robustness.** Readers follow the parser rules of ADR 0011 (H1 to H5 in `crates/rcfd-io/src/guard.rs`): file size cap
    1 GiB, item caps, checked arithmetic before any allocation sized from the file, no panic on any input (checked by
    `guard::fuzz_no_panic` on every fixture).
*   **Determinism.** The same bytes and unit give the same `TriSurface`, bit for bit. No result depends on hash order.
*   **Security.** No network access. Sidecar buffers resolve only inside the folder of the `.gltf` file.
*   **Rollout.** Steps 1 to 3 add library code and a CLI command; no existing default changes. Step 4 adds optional case
    keys; cases without them keep today's behaviour and case hash.
*   **Cost.** Step 1 adds no dependency and no heavy test; its tests take seconds.

## Risks and mitigations

| Risk | Impact | Mitigation | Step |
|---|---|---|---|
| Unit error on import (mm read as m). | A body 1000 times too large meshes and runs, and gives wrong answers. | The unit is a required argument with no default; the intake prints the size in metres; acceptance A1 checks the blade's tip radius. | 1, 2, 5 |
| Wrong coordinate frame (glTF is +Y up by its specification, and exporters such as Blender's rotate a Z-up model to Y-up by default). | The blade sits rotated in the domain; a check of distance from one axis can still pass. | The readers apply no axis change; the intake prints the bounding box; the case states the rotation axis; step 5's ADR may tighten A1 with a check of the blade's extent along that axis (ADR 0184 D1 allows tightening). | 2, 4, 5 |
| Leaky or non-manifold surface. | Meshing fails or meshes the wrong region. | Repair, then refusal with counts per patch; wrapping before the pipeline is called usable for downloaded models. | 2 |
| Vertices split at seams (glTF, some OBJ exporters). | Free edges on a closed body. | Bitwise welding in the readers, with the count in the notes. | 1 |
| Coordinates that should meet differ in the last bits (two CAD faces tessellated separately). | Free edges that bitwise welding does not close. | The intake reports them per patch; a tolerance weld (`TriSurface::from_soup` with `tol > 0`) is an option of step 2, off by default. | 2 |
| Patch names differ between exporters or after an edit (renamed object). | A boundary condition refers to a patch that is gone. | Patch diff on re-import; the run is blocked with the patch and boundary-condition names. | 4 |
| Thin trailing edge of the blade. | Prism layers collide or collapse at the edge; tetrahedra degenerate. | Layer thickness limited by local proximity (`surface_remesh::thickness`), checked by the step's ADR limits; acceptance A4 to A6. | 3 |
| Boundary recovery fails on slivers of a CAD tessellation. | `tetrahedralize` errors. | Surface remeshing (F-131) before the volume mesh is an option of step 3. | 3 |
| Stale cache (a setting not in the hash). | A changed setting runs on an old mesh. | Every meshing setting is in the hash; step 4 has a test that changes each setting and expects a rebuild. | 4 |
| The FAN-01 surface cannot be committed or fetched. | The acceptance cannot be repeated by others. | Open question Q2. | 5 |

## Open questions (for the maintainer)

*   **Q1. Volume-mesh route for step 3.** Route T (prism extrusion plus tetrahedra, joined at the layer cap) or route O
    (octree fill with snapped layers). Route O exists end to end but missed the 70-degree non-orthogonality limit on the
    sphere and its inside test is a ray-cast parity that a thin blade edge stresses. Route T needs new join code, but its
    surface triangles stay as imported and the layer cells are prisms on the wall. **Recommendation:** route T, with route
    O kept as a cross-check on the same blade, because the blade's thin edges fit a surface-conforming mesh better than an
    octree core. This is a recommendation only.
*   **Q2. The FAN-01 acceptance surface.** Who turns `cad_model.igs` into a surface, with which tool and tessellation
    tolerance, and is the result committed (CC BY 4.0 requires attribution in `NOTICE`), added as a fetch entry (zenodo.org
    is not on the fetch host list), or kept local with its SHA-256 recorded? **Recommendation:** the maintainer exports it
    once with FreeCAD to OBJ in millimetres, one object per face group, and the file is committed under
    `validation-data/reference/` with the attribution, so the acceptance test can run on any machine.
*   **Q3. The acceptance domain.** G9 says "meshes and runs the FAN-01 blade". (a) One blade in a straight box duct,
    stationary, which grades the pipeline only; (b) the periodic blade passage, which M-FAN-A1 will need but which adds
    periodic surface meshing; (c) the full rotor. **Recommendation:** (a) for M-FAN-B, and (b) designed in M-FAN-A1, once
    EXP-ROT has decided the rotor model.
*   **Q4. `geometry.variant`.** The parent design lists it with the values `nominal` and `manufactured`; the second belongs
    to G12. **Recommendation:** add the key when the as-manufactured option exists, not as a one-value key in step 4.

## Detailed implementation plan (step 1 only)

Registration first: this document and ADR 0184 are committed before any reader code.

### Target files

#### 1. `crates/rcfd-io/src/common.rs` (modify)

*   `SurfaceImport { surface, patches, notes }` as above, with `patch_names() -> BTreeMap<u32, String>` for
    `TetMesh::to_mesh`.
*   `length_factor(unit) -> Result<f64>`: `Unit::parse`, dimension length, zero offset, finite positive factor.
*   `finish_import(points, tris, tags, names, notes) -> Result<SurfaceImport>`: bitwise weld, drop unused vertices, drop
    patches with no triangles, renumber tags, error when no triangle is left.

#### 2. `crates/rcfd-io/src/obj.rs` (new)

*   `ObjOptions { patches: ObjPatches }` (`Objects` default, `Groups`).
*   `read_bytes(bytes, unit, &ObjOptions) -> Result<SurfaceImport>`, `read_file(path, unit, &ObjOptions)`.
*   Ear clipping with the Newell normal; fan fallback with a note.

#### 3. `crates/rcfd-io/src/gltf.rs` (new)

*   `read_gltf(json_bytes, unit, resolver) -> Result<SurfaceImport>`, `read_glb(bytes, unit, resolver)`,
    `read_bytes` (detects GLB by its magic), `read_file(path, unit)` with the folder resolver.
*   Base64 decoder (RFC 4648 alphabet, padding optional at the end, nothing else allowed), percent-decoding of relative
    URIs, accessor reader with stride and bounds checks, node walk with transforms and determinant sign.

#### 4. `crates/rcfd-io/src/lib.rs`, `crates/rcfd-io/src/registry.rs` (modify)

*   Module list and doc line; registry entries `obj` (`.obj`) and `gltf` (`.gltf`, `.glb`), both `Read`, with
    conformance cases built in code and a `tamper` path that must fail.

#### 5. Tests (new)

*   `crates/rcfd-io/tests/j4_surface_readers.rs`: the registered checks R1 to R10 of ADR 0184. Every fixture is written
    by the test, in memory or in a per-test temporary folder.
*   `crates/rcfd-post/tests/j4_glb_reader_cross.rs`: a GLB written by `rcfd_post::gltf::glb_patches` (an independent
    writer, checked against the Khronos validator in F-117) is read by `rcfd_io::gltf` (check R11).

#### 6. Documentation (modify)

*   `docs/interchange-formats.md`: the two readers, their rules and limits; the registry count.
*   `docs/meshing.md`: the readers as inputs of the pipeline.
*   `docs/provenance.md`, "File formats": the sources of the two readers.
