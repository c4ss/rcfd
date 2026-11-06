# ADR 0187: ADR 0173 amendment 3, F-272 registration: mesh set-up from the MCP server in project mode

Status: **REGISTERED 2026-11-05**, written before any code of F-272. Not yet accepted by the maintainer.
Parent: [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) (proposed, not accepted),
requirement R2, work item W4, check C8, decisions D2 and D8, roadmap item F-272; it builds on
[ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) (F-270, merged at a662c4a) and
[ADR 0186](0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md) (F-271, merged at ec4dace).
ADR 0173 says that every limit of its checks is fixed by an amendment before any test or run; this is that amendment for F-272.
Results go in the addendum at the end.

Read at main ec4dace. Number: 0186 is the highest on main; no branch and no worktree holds a 0187.

ADR 0173 is not accepted. Where it gives a recommendation, this text uses it and says so; that is a recommendation taken, not a
maintainer decision. Where the code read below raises a question ADR 0173 does not answer, the options and a recommendation are
listed (Q1 to Q6), and work proceeds only on a choice that is reversible.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)): defaults stay bit-identical; no solver result, case
hash or byte of an existing case changes; no case-schema key and no run-record field is added;
`crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new third-party crate; no commit trailers.

## What F-272 asks

The roadmap entry: in project mode the MCP server checks a case's mesh (the report of `rcfd check-mesh` and the patch list),
draws it with the CPU renderer, imports a Gmsh, Fluent or OpenFOAM file from the project folder into `meshes/` with its cell, face
and patch counts, lists `meshes/` with the cases that use each file, and returns the mesh check when `edit_case` changes a mesh
setting; an invalid mesh setting is refused with a located diagnostic and writes nothing (ADR 0173 R2, W4, C8, D8).

ADR 0173 D8 (a), its recommendation, is used: the mesh of a case is set up by its settings (`mesh.generator` and its parameters,
`mesh.source = "file"` with `mesh.file`, `mesh.scale`, `[mesh.transform]`, `mesh.wall_layers`) through `edit_case`; new commands
check, draw, import and list. D8 (b), a mesh recipe format and the surface-to-volume pipeline, is not in F-272.

## What the code read shows (main ec4dace, nothing run)

1. **The name `check_mesh` is taken.** `rcfd-mcp`'s session tool `check_mesh` (`src/lib.rs` line 96) is listed before the
   project tools; `call_tool` checks the arguments against the first tool of a name and `dispatch` asks the project tools first
   (ADR 0186 item 1), so a project tool of that name could not work. `preview_mesh`, `import_mesh` and `list_meshes` are free.
   ADR 0173 R2 has no project `load_mesh`: a case's mesh file is a setting changed through `edit_case`.
2. **The report to match.** `rcfd check-mesh <case>` (`crates/rcfd-cli/src/main.rs` lines 658 and 685) reads the case with
   `Case::load` (by extension: `.json` as JSON, anything else as TOML), builds its mesh with `rcfd_api::sim::build_mesh` and
   prints `rcfd_mesh::quality::check_mesh` of it (`--json`: the `QualityReport` as pretty JSON). The session `mesh.check`
   (`crates/rcfd-api/src/service.rs` line 517) returns `{ok, report, patches: [{name, faces, kind}]}` from the same two calls.
3. **`mesh.file` is not confined by the case.** `build_mesh` (`crates/rcfd-api/src/sim.rs` line 328) reads `mesh.file`, absolute
   or relative to the case's folder, with no limit; the case's sandbox covers data files only (tables, chemistry, UDF modules;
   `case.rs` line 711), and the service confines `mesh.file` with its own `check_case_paths` (`service.rs` line 357), which the
   workspace does not use. `mesh.file` is the only file a mesh build reads. A case in `cases/` names a mesh in `meshes/` as
   `../meshes/x.msh`, which `rcfd_project::paths::resolve_in` refuses as written (a `..` component).
4. **`edit_case` does not build the mesh.** It validates through `Case::set` and `semantic_errors` (`crates/rcfd-workspace/src/write.rs`).
   Refusals that live inside `build_mesh` are not reached: a missing or unreadable mesh file, an option the generator does not
   apply (`check_mesh_options`, `sim.rs` line 184: `mesh.grading` on `cavity_2d`, `mesh.wall_layers` outside `box_2d` and
   `channel_2d`, ...), the 70-degree limit of cylindrical interface faces. Such an edit is written today.
5. **The readers a case uses.** `build_mesh` reads a directory with `rcfd_io::openfoam::read_dir` (the files `points`, `faces`,
   `owner`, `neighbour`, `boundary`, and `cellZones` and `faceZones` when present, directly in that directory) and any other path
   with `rcfd_io::read_msh_file` (Fluent when the first non-blank byte is `(` or the file starts with the gzip or HDF5 magic, else
   Gmsh; `read_msh_with_notes` gives the same mesh with notes such as "Fluent 2D mesh extruded to one layer"). `rcfd check-mesh`
   also accepts the folder above a `polyMesh` folder.
6. **The samples of C8.** `crates/rcfd-io/tests/data` holds 12 Gmsh files (2.2 and 4.1, ASCII and binary, of a tetrahedral, a
   hexahedral and a prismatic box) read by `gmsh_samples.rs` (1139, 64 and 336 cells; patches `inlet`, `outlet`, `walls`; cell
   zone `fluid`). The Fluent file of `fluent_public.rs` (`elbow.msh`) is fetched by `validation-data/fetch.py`, not vendored, and
   is absent on this machine; that test skips with a message when it is absent. `crates/rcfd-cli/tests/fluent.rs` writes ASCII
   Fluent channel meshes in the test.
7. **The CPU renderer.** `rcfd_post::render::render_scalar` (`render.rs` line 460) draws the cells that a plane cuts (normal
   `axis`, default the thinnest bounding-box direction; at `slice`, default its middle), flat-shaded, with the colour range
   `range` or else the minimum and maximum of the drawn values. Per-cell measures: `rcfd_mesh::quality::compute_metrics` gives
   non-orthogonality and skewness per face and the aspect ratio per cell.
8. **Size limits.** The Studio's preview (ADR 0138) refuses, unless asked "anyway", a generator mesh estimated above 2 000 000
   cells (`preview::estimate`, in `rcfd-studio`) and a mesh file above 256 MiB. The session `check_mesh` and `rcfd check-mesh`
   have no limit.
9. **`meshes/`.** `Project::meshes` lists every file under `meshes/`, so a `polyMesh` folder appears as its five or more files.
   `cases::check_name` is the name rule of new cases (letters, digits, `-`, `_`; no dots).
10. **Tests that count the project tools.** `project_list_f270.rs` (C-L of ADR 0186: 51 tools and the registered list),
   `project_runs_w48f.rs` (51, twice) and `project_w48f.rs` F3 (`33 + 18`). F2 and F12 of `project_w48f.rs` read a local table of
   the 17 tools of ADR 0135, not the live list; F2 compares its read tools with `rcfd_claude::allow::PROJECT_READ_TOOLS`, the 11
   read tools the Studio's Claude Code bridge allows without asking (ADR 0134 W48-11 (a)). The plugin's three skills name tools in
   `allowed-tools`. `docs/mcp-tools.md` says "adds 18 tools".
11. **The crate.** `rcfd-workspace` depends on neither `rcfd-io` nor `rcfd-mesh`; ADR 0185 E1 said `rcfd-io` joins at W4. Both are
   in the workspace and below `api` in the layering rule.
12. **The maintainer's worktree** `studio-case-lineage` changes Studio files; F-272 changes no Studio file.

## Choices of this registration

**M1. Four project commands and tools (ADR 0173 W4, used; Q1 (a) for the first name).** `check_case_mesh` (read),
`preview_mesh` (read), `import_mesh` (write_files) and `list_meshes` (read), listed in that order right after `new_case`.
`rcfd_workspace::COMMANDS` gains them. No session tool is hidden (D2 (b) is left to F-275, W8, as in ADR 0186). New errors none:
the codes are 1003 (validation, with located `data.diagnostics`), 1004 (refused path or name), 1202 (no such file), -32602
(arguments) and -32000 (internal), as for the other project tools.

**M2. Where a case's mesh file may be.** Every command that builds or resolves a case's mesh first resolves `mesh.file` (when
`mesh.source = "file"`): a relative name is joined to the case's folder (as `build_mesh` joins it) and normalised (`.` dropped, `..`
removing one component, refused when it would leave the project root), and the result is checked with `resolve_in` (no links,
no drive or stream syntax); an absolute name must lie under the project root after the same normalisation. A refusal is 1004 with
`data.diagnostics` `[{path: "mesh.file", ...}]`, before anything is read or written. The file itself is read by `build_mesh`, so
a missing file is its located diagnostic (1003). `list_meshes` uses the same resolution and lists what it refuses.

**M3. `check_case_mesh {case}`.** `case` is a project-relative `.toml` or `.json` file (else 1003, as `edit_case`). It is read as
`rcfd check-mesh` reads it (by extension), with its folder as the base and the project as the data-file sandbox; a case that does
not parse is 1003 with the diagnostics. Then M2, `build_mesh` (failure: 1003 with its located diagnostics), and `check_mesh`.
Result: `case`, `mesh` {`source` `generator` with `generator`, or `file` with `file` (project-relative)}, `ok` (no error in the
report), `report` (the `QualityReport` as `rcfd check-mesh --json` prints it), `patches` [{`name`, `faces`, `kind`}] (as the
session `check_mesh`), `patches_total`. Size: when the text is over `TEXT_BUDGET`, patches are dropped from the end of the list
(`patches_total` keeps the count), and if it is still over, `report.poor_list` is emptied and `poor_list_omitted` says how many
entries were dropped.

**M4. `preview_mesh {case, measure?, width?, axis?, slice?}`.** The case is read and built as M3. Each cell gets the value of
`measure` (Q5): `non_orthogonality` (default; the largest non-orthogonality of the cell's faces, degrees), `skewness` (the largest
skewness of its faces) or `aspect_ratio` (the cell's), from `compute_metrics`. The plane: `axis` (`x`, `y` or `z`) and `slice`
as given, else the thinnest bounding-box direction (the first of equal ones) and its middle, the rule of `render_scalar`, computed
here so that the result can name them. Colour range: from the measure's ideal value (0 for non-orthogonality and skewness, 1 for
the aspect ratio) to the largest finite value over all cells; when that is within 1e-9 of the ideal, the range is ideal to ideal
+ 1, so a perfect mesh is one colour instead of rounding noise stretched over the map. Drawn by `render_scalar` with width
`width` (64 to 4096, default 640) and its colour bar. Result: an image block (PNG) and the text {`case`, `measure`, `min`, `max`
(finite values over all cells), `non_finite`, `range` [lo, hi], `cells`, `cells_drawn`, `axis`, `slice`, `width`, `height`,
`png_bytes`, and `reduced_from` when the width was lowered}. A plane that cuts no cell is 1003. Over the budget: Q4 (a).

**M5. `import_mesh {path, name?}`.** `path` is a project-relative file or folder (refused paths 1004; missing 1202). A folder is an
OpenFOAM mesh: its `polyMesh` subfolder when it has one, else itself, read with `openfoam::read_dir`. A file is read with
`rcfd_io::read_msh_with_notes` (the reader `build_mesh` uses), `format` `fluent` or `gmsh` by the rule of item 5. A file no reader
reads is 1003 with the reader's message, before anything is written. The target is `meshes/<name>`: `name` defaults to the
source's file name (for a folder, the nearest folder name that is not `polyMesh` or `constant`); it is one path component of
letters, digits, `-`, `_` and `.`, starting with a letter or digit, at most 128 characters, not a Windows device name, and a file
keeps its source's extension (else 1004). Writes never replace: a file by `rcfd_project::atomic::write_new`; an OpenFOAM mesh as
`meshes/<name>/` holding copies of the files `read_dir` reads, each by `write_new`, the ones written removed again on a failure.
When the target is there with the same bytes (every file, for a folder), nothing is written and `copied` is false; with other
bytes, or another name differing only in letter case, it is 1004. No `.rcfd/edits.jsonl` line (a mesh import line is ADR 0173
W6, F-273). Result: `source`, `path` (`meshes/<name>`), `copied`, `format` (`gmsh`, `fluent` or `openfoam`), `files`
[{`path`, `bytes`, `hash`, `written`}], `cells`, `faces`, `internal_faces`, `points`, `patches` [{`name`, `faces`, `kind`}],
`patches_total`, `zones` [{`name`, `kind` (`cell` or `face`), `size`}], `bounds` {`min`, `max`} (so mm and m can be told apart),
`notes` (the reader's), `mesh_file` (the `mesh.file` value for a case directly in `cases/`: `../meshes/<name>`). Patches and zones
are cut to the size budget as M3.

**M6. `list_meshes {offset?, limit?}`.** One entry per file under `meshes/`, except that a folder holding `points`, `faces`,
`owner`, `neighbour` and `boundary` is one entry (`kind` `openfoam`, with its file count) and its files are not listed; others are
`kind` `file`. Each entry: `path`, `kind`, `bytes`, `used_by` [case paths]. A case uses an entry when its text (every `.toml` and
`.json` under `cases/`, read as TOML or JSON by extension, without validation, so a case with other errors still counts) sets
`mesh.source = "file"` and its `mesh.file` resolves (M2) to the entry's path. Also `outside` [{`case`, `mesh_file`, `path` (or
null), `exists`, `refused` (or null)}]: case mesh files that are not under `meshes/` or that M2 refuses, and `unread` [{`path`,
`message`}]: cases whose text is not TOML or JSON. Paged as `list_runs` (`offset`, `limit` 1 to 100, default 50, `total`,
`next_offset`), and cut to the size budget.

**M7. `edit_case` with a mesh setting (ADR 0173 R2 and C8, used).** When a key of `set` is `mesh` or starts with `mesh.`, the
edited case (the one validated today) has its mesh resolved (M2: 1004) and built (`build_mesh` failure: 1003 with its located
diagnostics, `data.diagnostics`), before anything is written; then the quality check, whose result is the `mesh` key of the result
{`ok`, `report`, `patches`, `patches_total`, as M3}. This holds for `dry_run` too. A mesh that builds but fails the check is
written, with `mesh.ok` false (Q2 (a)). An edit without a mesh key builds nothing and returns exactly today's keys, so its result
and the bytes it writes are unchanged. The descriptor of `edit_case` is not changed.

**M8. The MCP descriptors and the restated counts.** Four descriptors in `rcfd-mcp/src/project.rs` (M1's names, order and
access); the project sentence of `initialize`'s instructions names them. The C-L check of ADR 0186 is restated for F-272 (C-L
below). The counts of item 10 become 55 (`project_list_f270.rs`, `project_runs_w48f.rs` twice) and `33 + 22`
(`project_w48f.rs` F3); nothing else in those files changes, and F2 and F12 keep their table (Q6). The read-only refusal of
`import_mesh` is checked in the new test file, not in F3's loop. `docs/mcp-tools.md` says 22.

**M9. Dependencies.** `rcfd-workspace` gains `rcfd-io` and `rcfd-mesh` (`.workspace = true`); `Cargo.lock` changes only in
`rcfd-workspace`'s dependency list; the architecture page's line for the crate names them. The layer order is unchanged
(`workspace` above `api`, both below it).

**M10. Separable commits.** (1) this text; (2) the workspace commands, the `edit_case` change and their tests; (3) the MCP
descriptors and their tests; (4) documents; (5) the addendum. Each builds and passes on its own.

## Open questions

**Q1. The name of the mesh check (item 1, D2).** (a) `check_case_mesh`, the session `check_mesh` and `load_mesh` stay until F-275
(W8) decides the project-mode list, which may rename it; (b) a project `check_mesh` replaces the session one under `--project`
now (D2 (b) for this tool; `load_mesh` with it, since it calls `mesh.check`); (c) one `check_mesh` with both meanings, chosen by
its arguments. Recommendation (a), as ADR 0186 Q1 did for `new_case`; reversible (a rename). Proceeding on (a).

**Q2. A mesh that builds but fails the quality check, in `edit_case`.** ADR 0173 says an invalid `mesh.*` edit is refused with a
located diagnostic; the check's errors (a non-orthogonality over 85 degrees, a cell of no volume, ...) are about the mesh, not a
setting, and `rcfd run` refuses such a mesh (exit 4). (a) Write the edit; the result's `mesh.ok` is false and `mesh.report.errors`
says why; (b) refuse it with 1003. Recommendation (a): the refusal of ADR 0173 is for a located diagnostic, and the run gate stays.
Proceeding on (a); reversible (one constant).

**Q3. Size limits of a mesh build in the MCP server (item 8).** (a) None, as the session `check_mesh` and `rcfd check-mesh`; (b) the
Studio's limits (2 000 000 estimated cells, 256 MiB) with `preview::estimate` moved from the Studio into the workspace and the
Studio's preview using it (a Studio change, after the maintainer's Studio worktree lands); (c) a file-size limit only.
Recommendation (b), later; proceeding on (a), so no limit is copied from the Studio and none is invented. A large mesh blocks
the server for as long as `rcfd check-mesh` takes on it.

**Q4. A preview over the size budget.** (a) Draw it again at half the width until the result's text fits (down to 64 pixels;
`reduced_from` gives the width asked for; 1003 if even 64 does not fit); (b) write it under `.rcfd/previews/` and return the
path, as `render_field` does under `runs/<id>/renders/` (a read tool writing outside a run folder); (c) refuse with 1003.
Recommendation (a); proceeding on (a); reversible.

**Q5. The preview's measure.** ADR 0173 says "a cell-quality measure" without naming one. (a) Non-orthogonality by default, with
skewness and aspect ratio by argument: the check's one error limit and its first warning are on non-orthogonality; (b) another
default. Recommendation (a); proceeding on (a); reversible (one constant).

**Q6. The new read tools and the Studio bridge's allow-list (item 10).** (a) Leave `rcfd_claude::allow::PROJECT_READ_TOOLS` and the
plugin's skills as they are: in the Studio's sidebar `check_case_mesh`, `preview_mesh` and `list_meshes` ask before they run;
(b) add the three read tools to that list (allowed without asking, like the other read tools). Recommendation (b), since ADR 0134
W48-11 (a) made the first list the read-class project tools; it changes what runs without asking, so it is the maintainer's.
Proceeding on (a), unchanged.

## Acceptance checks and limits (fixed here, before any code)

* **C8-I (import, workspace).** For each of the 12 Gmsh files of item 6, copied into a new project's `incoming/`: `format`
  `gmsh`; `cells` 1139, 64 or 336 as `gmsh_samples.rs`; `cells`, `faces`, `internal_faces`, `points`, every patch's name, face
  count and kind, and every zone's name and size equal to `rcfd_io::read_msh_file` of the source; patch names `inlet`,
  `outlet`, `walls`; `meshes/<file name>` byte-equal to the source; `copied` true. A Fluent ASCII 2D channel written by the test
  (8 x 4 quads, as `rcfd-cli/tests/fluent.rs`): `format` `fluent`, the counts equal the reader's (32 cells; patches `walls`,
  `inlet`, `outlet`, `frontAndBack`), `notes` holds the extrusion note. The elbow of `fluent_public.rs`, when fetched: 918 cells
  and its five zone patches plus `frontAndBack`; when absent the test says "skipped" and passes, as `fluent_public.rs` does. An
  OpenFOAM mesh written by `rcfd_io::openfoam::write_dir` (a `box_2d` mesh), given as the folder holding `polyMesh` and as the
  `polyMesh` folder (second import named `foam2`): `format` `openfoam`, counts equal to `read_dir`'s, the five files copied
  byte-equal into `meshes/<name>/`. Again the same file: `copied` false, nothing changed. Refused, with `meshes/` unchanged: a text
  file no reader reads (1003), a missing source (1202), `../outside.msh` and an absolute path outside the project (1004), a name
  with a separator (1004), a name with another extension (1004), a file whose name differs only in letter case from an existing
  one with other bytes (1004).
* **C8-C (check).** In a project served by the `rcfd-mcp` binary, for (i) the cavity of the project tests (a generator), (ii) a
  case with `mesh.source = "file"` and `mesh.file = "../meshes/hex_v4.1_ascii.msh"` after its import, (iii) the cavity with
  `[mesh.transform]` `rotate_z_deg = 30`: `check_case_mesh`'s `report` equals the value `rcfd check-mesh <case> --json` prints
  (the `rcfd` binary next to `rcfd-mcp`), and `ok`, `report` and `patches` equal the session `check_mesh`'s for the same case
  (`create_case {path}`, in-process). Refused: a case whose `mesh.file` is `../../outside.msh` and one with an absolute path
  outside the project (1004, the diagnostic at `mesh.file`); a case naming a missing file (1003, located at `mesh.file`).
* **C8-P (preview).** For a `wedge_2d` case (skewed cells) and the default measure: an image block; `max` equals the check
  report's `max_non_orthogonality` (exact); `min` and `max` equal the smallest and largest per-cell values the test computes from
  `compute_metrics` by M4's rule; `range` is [0, `max`]; the PNG's bytes equal `render_scalar` plus `to_png` of the same mesh,
  values, plane and range made in the test (0 differing bytes), and equal again when the test passes `axis: None` and `slice:
  None` to `render_scalar` (the plane rule is the renderer's). The uniform cavity: `min` and `max` below 1e-9, `range` [0, 1].
  `aspect_ratio` on the wedge: other PNG bytes than the default (control). A 3D `annulus_3d` case: `cells_drawn` > 0 and below
  `cells`, `axis` the thinnest direction (z for lz 0.2 and r_out 1), bytes equal to the reference. Width 4096 on the wedge: the
  result's text is at most `TEXT_BUDGET` and `reduced_from` is 4096, or the image fits at 4096 and there is no `reduced_from` (the
  test says which). A plane outside the mesh (`slice` beyond the bounds): 1003.
* **C8-E (`edit_case`).** On the cavity: `mesh.nx = 10` is written, its result has `mesh.ok` true and `mesh.report.n_cells` 120;
  `mesh.grading = 2` (not used by `cavity_2d`) is refused with 1003 and a diagnostic at `mesh.grading`; `mesh.source = "file"`
  with `mesh.file = "../meshes/missing.msh"` is refused with 1003 and a diagnostic at `mesh.file`; `mesh.file =
  "../../outside.msh"` and an absolute path outside the project are refused with 1004. Each refusal leaves the file's bytes and
  `.rcfd/edits.jsonl` unchanged. A `dry_run` with a mesh key has the `mesh` key and writes nothing. A Fluent channel sheared by a
  factor 20 (non-orthogonality about 87 degrees) set as the mesh file is written with `mesh.ok` false and `mesh.report.errors`
  not empty (Q2 (a)). Control: `physics.nu = 0.02` gives a result without the `mesh` key, with the same keys as before F-272.
* **C8-L (list).** A project with `meshes/a.msh` (used by `cases/x.toml` and by `cases/sub/y.toml` through `../../meshes/a.msh`),
  `meshes/b.msh` (unused), `meshes/foam/` (an OpenFOAM folder used by `cases/z.json`), `cases/w.toml` using `cases/local.msh`,
  `cases/v.toml` whose mesh file is `../../outside.msh`, and `cases/u.toml` that is not TOML: three entries in path order with
  those `used_by` lists and `foam` as one `openfoam` entry; `outside` holds `w` (in the project, exists) and `v` (refused);
  `unread` holds `u`. `limit 1` returns the first entry and `next_offset` 1.
* **C-M (MCP).** Through the `rcfd-mcp` binary: one call of each new tool succeeds (an image block for `preview_mesh`). Under
  `Policy::read_only()`, `import_mesh` is refused with 1100 and the project folder is unchanged; the three read tools work.
* **C-L (tool list).** Under `--project`: 55 tools; without the four new ones, the list is the one ADR 0186's C-L requires
  (the golden of F-270 with the F-271 changes), byte for byte; the four come right after `new_case` in M1's order, with access
  `read`, `read`, `write_files`, `read`, `additionalProperties` false, and required `["case"]`, `["case"]`, `["path"]`, `[]`.
  The initialize instructions under `--project` name the four. Without `--project`, `default_f1.rs` passes unchanged.
* **C-1 (parity).** With no edit to them: `runner_stdio_w48l.rs`, `sv301_mcp_w48z.rs`, `correctors_w49j.rs`,
  `new_simulations_f271.rs` and `default_f1.rs` of `rcfd-mcp`, every test of `rcfd-studio` and `rcfd-workspace` that exists
  now; `project_w48f.rs` and `project_runs_w48f.rs` pass with only their counts changed (M8), `project_list_f270.rs` with C-L;
  `workspace_graph.rs` passes.
* **Can fail.** Two mutations, each restored and the file compared byte for byte with the kept one afterwards: (1) M2's
  confinement removed: the escape refusals of C8-C or C8-E fail; (2) M7's build removed: the `mesh.grading` and missing-file
  refusals of C8-E fail. `scripts/limit_audit.py` is run on the new tests.
* **Defaults.** No solver result, case hash or existing case byte changes; `baseline_hashes.rs` and the H1 hashes untouched;
  no case-schema key or run-record field added; `Cargo.lock` differs from main only in `rcfd-workspace`'s dependency list.
  The full gate (fmt per package, clippy with `-D warnings`, every test in release on a `git archive` copy, one process per test,
  three jobs) reports no failure; a flaky failure is reported as a failure with its name.

## Not in F-272

A mesh recipe format and the surface-to-volume pipeline (D8 (b)), hiding session tools (W8, F-275), journal lines for imports and
the feed (W6, F-273), Studio changes (its preview keeps its own builder and limits), and the bridge allow-list (Q6).

## Where to go next

* [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) for the layer and its decisions.
* [ADR 0186](0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md) for the tool list this extends.
* [Mesh import](../mesh-import.md) and [mesh quality](../mesh-quality.md) for the readers and the report.
* [MCP tools](../mcp-tools.md) for the project tools.

## Addendum 1: what was built, and the results (2026-11-05)

Branch `f272-mesh-setup`, after this text: the workspace commands with the `edit_case` change and their tests, then the MCP
descriptors and their tests, then the documents. Not merged.

**Built as registered.** M1 to M10 as written. `rcfd-workspace` gains `src/meshes.rs` (the four commands, the mesh-file rule of
M2 as `resolve_mesh_file`, the per-cell measures and the check object), the mesh check in `edit_case` (`write.rs`: built only
when a key of `set` is a mesh setting; the result's other keys unchanged), four names in `COMMANDS` and the dependencies
`rcfd-io` and `rcfd-mesh`; `cases::RESERVED` became crate-visible for the mesh name rule. `rcfd-mcp` gains the four descriptors
after `new_case` and the sentence of the instructions; `edit_case`'s descriptor is unchanged. No Studio file changed. Existing
tests edited: `project_list_f270.rs` (C-L restated), the counts of `project_runs_w48f.rs` (two) and `project_w48f.rs` (one).
New tests: `crates/rcfd-workspace/tests/mesh_f272.rs` (7) and `crates/rcfd-mcp/tests/mesh_setup_f272.rs` (2).

**Differences found while building.** (1) The session tools cannot load a project case that names its mesh as
`../meshes/x.msh` when the server has a sandbox root, as it has under `--project`: the service refuses any `..` component of a
path ("leaves the sandbox", `sandbox_path`, `crates/rcfd-api/src/service.rs`), and `create_case {path}` checks `mesh.file`
that way. C8-C's comparison with the session `check_mesh` therefore uses a session server without a root and the same case by
its absolute path; the refusal under the project root is asserted in the test as an observation. Not changed (session tools
are outside F-272). (2) `list_meshes`' `outside` also lists a case whose mesh file is under `meshes/` but is not one of its
entries (a missing file), not only files outside `meshes/` and refused ones as M6 says. (3) `import_mesh` also returns
`zones_total`, and every `list_meshes` row has `files` (1 for a file). (4) The mesh name rule of M5 also refuses a name
ending in `.` (Windows drops it). (5) `check_case_mesh` and `preview_mesh` answer 1202 when `case` is a folder.

**Results.**

| Check | Result |
|---|---|
| C8-I | the 12 Gmsh files: `format` gmsh, 1139, 64 and 336 cells, counts, patches (`inlet`, `outlet`, `walls`) and zones equal to `read_msh_file`'s, bounds [0, 2] x [0, 1] x [0, 1] within 1e-9, files byte-equal. The Fluent channel written by the test: 32 cells, patches `frontAndBack`, `inlet`, `outlet`, `walls`, the extrusion note, counts equal to the reader's. OpenFOAM (18 cells): counts equal to `read_dir`'s, its files byte-equal under `meshes/foam/`; as the `polyMesh` folder named `foam2`, written again; unnamed, the existing `meshes/foam` (`copied` false). The same Fluent file again: `copied` false, nothing written. Nine refusals with `meshes/` unchanged: no reader 1003, missing 1202, `../outside.msh` and an absolute path outside 1004, a separator, another extension, another letter case of an existing name, an existing name with other bytes and a device name 1004; control: the same file under a free name imported. The public elbow is not fetched on this machine: its test printed "skipped" (not checked) |
| C8-C | through the binary, `report` equal to `rcfd check-mesh --json` and `ok`, `report`, `patches` equal to the session `check_mesh` for the cavity (144 cells), the imported hexahedral mesh by `../meshes/` (64 cells) and the cavity turned by 30 degrees (144 cells); refused: `../../outside.msh` and an absolute path outside 1004, a missing file 1003, each located at `mesh.file`; in the workspace test (`mesh_f272.rs`) the same refusals, and a relative and an absolute path inside the project accepted |
| C8-P | the wedge: `max` 19.911 degrees, equal to the report's `max_non_orthogonality` and to the test's own per-cell maximum, `min` equal, `range` [0, max], the PNG (6896 base64 bytes) equal to `render_scalar` with the reported plane and with the renderer's own plane rule; `aspect_ratio` another image with range starting at 1; the uniform cavity below 1e-9 with range [0, 1]; the annulus 128 of 256 cells drawn across z, equal to the reference; width 4096 drawn at 2048 pixels (`reduced_from` 4096) within the budget; a plane outside the mesh 1003 |
| C8-E | `mesh.nx = 10` written with `mesh.ok` true and 120 cells, the comment kept; a mesh `dry_run` with the check and nothing written; refused with the file and `.rcfd/edits.jsonl` unchanged: `mesh.grading = 2` on `cavity_2d` 1003 at `mesh.grading`, a missing mesh file 1003 at `mesh.file`, `../../outside.msh` and an absolute path outside 1004 at `mesh.file`; the sheared Fluent mesh (max non-orthogonality 87.1 degrees) written with `mesh.ok` false and an error; control: `physics.nu` gives exactly the seven keys of before |
| C8-L | `meshes/a.msh` used by `cases/sub/y.toml` and `cases/x.toml`, `meshes/b.msh` unused, `meshes/foam` one `openfoam` entry used by `cases/z.json`; `outside` `cases/v.toml` (refused, leaves the project) and `cases/w.toml` (`cases/local.msh`, exists); `unread` `cases/u.toml`; `limit 1` gives `next_offset` 1 |
| C-M | through the binary each new tool answers (`preview_mesh` an image block, 2528 bytes); the instructions name the four; read-only: `import_mesh` 1100 with the folder unchanged, the read tools answer; control: the default policy imports |
| C-L | 55 tools, 45 592 bytes; without the four the F-271 list byte for byte (the F-270 golden, 38 978 bytes, not re-recorded, with the F-271 changes); the four after `new_case` with the registered access and required lists; `default_f1.rs` passes |
| C-1 | every test of `rcfd-mcp` passes in release (67, the known-miss tests ignored as before), `project_w48f.rs` and `project_runs_w48f.rs` with only their counts changed; `cargo test --release -p rcfd-workspace -p rcfd-studio`: 319 passed, 1 failed, 21 ignored; the failure is `chat_e.rs` `e1_session_list_open_closed_unknown` (the order of the Claude Code session rows, line 75), which reads session files and no workspace code, failed once in that parallel run and passed 3 of 3 run alone; `workspace_graph.rs` 4 of 4, including that `rcfd-studio` does not depend on `rcfd-mcp` |
| Can fail | two mutations of the workspace, each restored and compared byte for byte with the committed file afterwards (equal). (1) M2's confinement removed from the mesh build: `c8c_mesh_file_confinement` and `c8e_edit_case_with_mesh_settings` failed (`../../outside.msh` gave 1003, the build's missing file, not 1004); the other five passed. (2) M7's build removed from `edit_case`: `c8e_edit_case_with_mesh_settings` failed at its first mesh assertion (no `mesh` key); a probe (a test file made for it and deleted) showed that under this mutation `mesh.grading = 2` on `cavity_2d` and a missing mesh file are written, and without it both are refused with 1003, so the two registered refusals depend on the build |
| Defaults | `Cargo.lock`: only `rcfd-io` and `rcfd-mesh` in `rcfd-workspace`'s list; `baseline_hashes.rs` and the H1 hashes untouched; no case-schema key or run-record field added; an `edit_case` without a mesh key returns the keys and writes the bytes it did before. `scripts/limit_audit.py` reports one hit in the new tests, the elbow test's data-gated `return` (class iv, with its skip message), the same as `fluent_public.rs` line 91 |
| Gate | `d397600` (the code of this branch and the documents before this row), `git archive` copy, three jobs (`scripts/gate_test.py`, one process per test): fmt 0, clippy `-D warnings` 0, 2819 passed, 0 failed, 800 ignored, 541 binaries, 77 min wall (60 min of tests). Against the F-271 gate (2810 / 0 / 800, 539 binaries): the 7 workspace and 2 MCP tests of F-272 in 2 new binaries. `chat_e` `e1_session_list_open_closed_unknown` passed in the gate |

**Questions for the maintainer.** Q1 to Q6 stand as registered, each proceeding on its reversible choice. Q6 changes what the
Studio's sidebar runs without asking and was not acted on. The finding of difference (1) is a further one: under the project's
sandbox root, `create_case {path}` of a case that names `meshes/` with `..` is refused, so no session, and no session
`check_mesh`, can be made from it (the session `load_mesh` was not tested); this bears on D2's hiding of the session mesh
tools (F-275).

## Decision record 2026-11-06

The maintainer's answers of 2026-11-06 to the open questions of this ADR. A question marked *not asked* was not put to the
maintainer that day; it stays on the reversible choice the work proceeded on, which is not a maintainer decision. This
record changes no limit, check or result above.

* **Q1 (the name `check_case_mesh`).** Not asked; proceeding on (a). F-275 kept the name under `--project` (ADR 0195 Q1 (a)).
* **Q2 (a mesh that builds but fails the quality check).** Not asked; proceeding on (a).
* **Q3 (size limits of a mesh build).** Answered: (b), later. The MCP server takes the Studio's limits (2 000 000 estimated
  cells, 256 MiB), with `preview::estimate` moved from the Studio into the workspace and the Studio's preview using it, once
  the maintainer's Studio worktree `studio-case-lineage` has landed (roadmap item F-293). Until then (a): no limit, and none
  is invented.
* **Q4 (a preview over the size budget).** Not asked; proceeding on (a).
* **Q5 (the preview's measure).** Not asked; proceeding on (a).
* **Q6 (the new read tools and the Studio bridge's allow-list).** Not asked. This ADR proceeded on (a); F-275 then took (b)
  (ADR 0195, Decisions needed item 6), and (b) is what main holds.
* The further finding of addendum 1 (under the project's sandbox root, `create_case {path}` refuses a case that names
  `meshes/` with `..`) is not a numbered question and was not asked. Under `--project`, F-275 hides `create_case` and the
  session mesh tools.
