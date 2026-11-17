# ADR 0732: F-836 proposal: a mesh preview the Studio can draw before a run (`mesh.preview`)

Status: **PROPOSED 2026-11-17.** Not accepted by the maintainer. Nothing is built: no Rust, TypeScript or test code was
written for this ADR, and nothing was run. The maintainer approves the command below before any code.

Roadmap item F-836 (Electron Studio: mesh preview before a run). Its acceptance asks for "an engine entry point that builds
the mesh without solving (ADR first if none exists)" and a test that the preview equals the solver's mesh, replacing
`preview_g1` and `adaptive_preview_f475`. Parents: [ADR 0800](0800-electron-studio-roadmap.md), guardrails G2, G3, G4, G15
and G16; the egui preview of [ADR 0138](0138-wave48g1-mesh-preview-from-case.md) and its adaptive case,
[ADR 0377](0377-f475-registration-studio-preview-of-an-adaptive-case.md). Number 0732 follows ADR 0731.

## Context

What exists:

- The solver's mesh comes from `rcfd_api::sim::build_mesh` (`crates/rcfd-api/src/sim.rs:565-568`): the generator or the
  mesh file, then the case's transform. It does not solve.
- The service method `mesh.check {session}` builds that mesh from a session's case and returns the quality report and the
  patch list (`crates/rcfd-api/src/service.rs:550-561`). The Studio's case editor already calls it
  (`apps/studio-web/src/renderer/case/CaseEditorPage.tsx:682`, action `apps/studio-web/src/shared/actions/registry.ts:257-262`).
  It returns no geometry.
- The project tool `preview_mesh {case}` (`crates/rcfd-workspace/src/meshes.rs:292-368`, ADR 0187) builds the same mesh
  and returns a PNG of one plane coloured by a quality measure. It reads the case file, not the editor's unsaved state, and
  gives no 3D surface and no patch visibility.
- A case with `[mesh.adapt]` runs on a different mesh after its first step. The only way to get that mesh is
  `rcfd_api::sim::adapted_mesh` (`crates/rcfd-api/src/sim/adapt.rs:547-563`), which runs the adaptive loop: `cycles + 1`
  solves, cancellable only between solves. `has_mesh_adapt` tells whether a case has the table (`adapt.rs:91-93`).
- The field store writer can write a mesh with no snapshot: `rcfd_tsstore::Writer::create(dir, &mesh, opts)` then
  `finish()` (`crates/rcfd-tsstore/src/lib.rs:539`, `:641`); the reader reports the mesh hash (`lib.rs:885`), which is
  `Mesh::content_hash` (`crates/rcfd-mesh/src/mesh.rs:757`).
- The Studio reads a run's mesh surface from `store.rts` with its own TypeScript reader (R2, ADR 0350 addendum 1), checked
  bit for bit against the Rust reader (G3). `rcfd serve` does not switch on the `data.*` channel (`crates/rcfd-cli/src/main.rs:851-890`
  never calls `enable_data`).
- The deleted egui preview (`crates/rcfd-studio/src/preview.rs` at b40c0816^) built with `build_mesh`, refused over a cell
  estimate (2 000 000) and a file size (256 MiB), and for `[mesh.adapt]` ran `adapted_mesh`. Its tests `preview_g1` and
  `adaptive_preview_f475` compared the preview's mesh hash, counts and patches with `Simulation::new`'s mesh.

## Proposal

A service method `mesh.preview`, read-only for the case, that writes one mesh-only field store.

- **Params:** `session` (string, required); `dir` (string, required, sandboxed like every service path); `adapt`
  (`"base"` default, or `"final"`).
- **Work:** build the mesh exactly as `mesh.check` does (`check_case_paths`, then `sim::build_mesh`). With `adapt: "final"`
  and a `[mesh.adapt]` table, run `sim::adapted_mesh` instead and use its final mesh. Write the mesh with
  `rcfd_tsstore::Writer` into a temporary folder beside `dir`, then rename it to `dir` (no snapshot, no field).
- **Reply:** `{path, mesh_hash, cells, faces, points, patches: [{name, kind, faces}], bounds: {min: [x,y,z], max: [x,y,z]},
  two_d, adapt: null | {pending: bool, cycles: n, solves: n}}`. `mesh_hash` is the hex of `Mesh::content_hash`.
  `adapt.pending: true` means the run will change this mesh in its first step (base mesh of a `[mesh.adapt]` case).
- **Errors:** 1001 unknown session; 1003 the mesh cannot be built (`data.diagnostics`, located, as `mesh.check`); 1004
  `dir` outside the sandbox, or the mesh over `max_cells` (`service.rs:131`), checked after the build and before writing;
  -32602 bad `adapt` or missing `dir`; -32000 the store cannot be written (nothing left at `dir`, the temporary folder
  removed).
- **Surface:** opt-in, in the same family as ADR 0730's `--ui` if that is accepted (Q1). Without the flag the method is
  -32601 and `service.info` is unchanged. CLI and scripts reach it with `rcfd call mesh.preview` (G16).
- **The Studio:** the case editor calls it through a named action with the editor's session and a folder under
  `.rcfd/previews/` (Q2), reads the store with its existing R2 reader, and draws the boundary surface with one colour per
  patch and a patch visibility list. A newer request makes the Studio ignore an older reply; the engine call itself runs to
  its end (as the egui preview did).

### How defaults and `baseline_hashes.rs` stay unchanged

- The mesh is built by the solver's own `build_mesh` and `adapted_mesh`, called as they are; neither changes. No default,
  case key, golden or reply of an existing method changes.
- The method writes only under `dir`. It writes nothing into a run or a case.
- `crates/rcfd-api/tests/baseline_hashes.rs`, the case hash tables and the MCP goldens are not edited; `preview_mesh` keeps
  its schema and result.

## Alternatives

1. **Return the surface through the `data.*` channel (a ticket like `data.surface`).** No file written, but `rcfd serve`
   does not turn that channel on and the Studio does not use it today. Recommendation: no for now; a later `data.preview`
   could add it.
2. **Return the triangles as JSON.** Simple, but a mesh of a million faces is far over any reply budget.
   Recommendation: no.
3. **Extend the project tool `preview_mesh` to write a store.** Reaches Claude Code too, but it reads the saved case file,
   not the editor's session, and changing its schema changes the project tool list golden. Recommendation: no; keep it.
4. **Studio builds the surface from the case.** That would be a mesher in the Studio (G2). Recommendation: no.
5. **A mesh-only store (this proposal).** The Studio reuses the reader that already matches the Rust reader bit for bit,
   and the mesh hash ties the preview to the solver's mesh. Recommendation: yes.

## Open questions for the maintainer

1. **Opt-in.** (a) the same `rcfd serve --ui` family as ADR 0730; (b) its own flag; (c) always on (changes
   `service.info`'s method list, needs your approval under G4). Recommendation: (a).
2. **Where previews go.** (a) `dir` from the caller; the Studio uses `.rcfd/previews/<mesh_hash>/` and reuses a store whose
   hash matches; (b) the engine chooses a folder under `.rcfd/`. Recommendation: (a). A cleanup command is a later item.
3. **`[mesh.adapt]` cases.** "Without solving" cannot give the adapted mesh. (a) default `base` with `adapt.pending: true`,
   `final` on request with its cost said first; (b) always `final`, as the egui Studio did; (c) `base` only. Recommendation:
   (a).
4. **Size limit before building.** (a) only `max_cells` after the build, as above; (b) also a cell estimate before
   building a generator mesh, as the egui preview did, refusing over the limit unless `force: true`. Recommendation: (b),
   so a typing slip (`nx = 100000`) does not build a huge mesh.
5. **Unsaved editor text.** `session` already holds the editor's state, so unsaved edits preview. (a) session only;
   (b) also a `case` path for a saved file without a session. Recommendation: (a).
6. **An MCP tool for Claude Code.** (a) none now: `rcfd call mesh.preview` and `preview_mesh` cover it; (b) a project
   tool `preview_mesh_store` (changes the project list golden by a registered restatement). Recommendation: (a).

## Tests that would prove it

- **M1 (replaces `preview_g1` G1)** for every shipped case: the store's mesh hash, cell, face and point counts, and patch
  names, kinds and sizes equal those of `Simulation::new(&case).geom.mesh`, bit for bit; where `Simulation::new` refuses,
  equal to `build_mesh` and said in the test output.
- **M2 (replaces `adaptive_preview_f475`)** for `cases/cavity/adapt_re100.toml`: `adapt: "base"` gives the base mesh and
  `pending: true`; `adapt: "final"` gives the mesh hash the run's store records after its first step.
- **M3** `service.info` and the method list without the opt-in equal the base commit's reply; `mesh.preview` is -32601.
- **M4** each error case above, with nothing left at `dir` afterwards.
- **M5** the store opens with `rcfd_tsstore::Reader` and with the Studio's TypeScript reader, and their surfaces are equal
  bit for bit (G3).
- **M6** `baseline_hashes.rs`, the case hash tables and the `preview_mesh` tests pass unedited.
- **Studio** e2e: the case editor shows a generator mesh, a file mesh and a `[mesh.adapt]` mesh with patch visibility;
  an edit updates the preview; a refused build shows the engine's located message.
