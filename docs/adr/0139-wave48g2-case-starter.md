# ADR 0139: Wave 48G item 2, case starter: new case files from the shipped examples

Status: **PROPOSED 2026-10-28** (sub-wave 48G, "first run to first picture", item 2 of 4; the other three items are the mesh
preview from a case, the store-fields default and one-click start-and-follow, done on other branches). Checks registered below
before any run of them; results go in the addendum. Written at main f3bf005. House rules as in ADR 0130: limits are fixed before
the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is corrected
by an amendment written before the corrected run; defaults of existing code stay bit-identical;
`crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded. Parents: [`docs/ui-restart-plan.md`](../ui-restart-plan.md) section
8 (project folder contract) and ADR 0131 (Studio shell: browser, case editor, worker threads, conflict-safe saves).

## Context (read at main f3bf005)

* Maintainer report: the Studio has no way to create a case. The Case menu has Save and Validate only
  (`crates/rcfd-studio/src/ui.rs` lines 161-180); a case reaches `<project>/cases/` only when the user copies a file there by hand.
* `cases/` holds two kinds of files ([`docs/tutorials/gallery.md`](../tutorials/gallery.md) "Two kinds of files"):
  * 10 runnable case files in TOML: `axial_fan_cgrid/axial_fan_cgrid.toml`, `axial_fan_sector/axial_fan_sector.toml`,
    `channel/{kepsilon_channel,kepsilon_inlet,profile_inlet,sst_channel}.toml`, `packed_bed/packed_bed.toml`,
    `rotating_channel/rotating_channel.toml`, `sod/sod.toml`, `wedge/wedge.toml`;
  * 6 benchmark descriptors named `case.json` (`backward_step`, `cavity`, `channel`, `flat_plate`, `heated_cavity`, `naca0012`):
    Reynolds numbers, meshes, citations and tolerances of validation cases, read by `rcfd cases`. They are not case files and
    `rcfd run` refuses them;
  * reference data under `<case>/reference/` (7 files) and one input table, `channel/profile_inlet.csv`, read by
    `profile_inlet.toml` through `[tables.inlet_u] file = "profile_inlet.csv"`.
* The case editor validates through an in-process `rcfd_api` service: `session.create` with `{"toml": text}` (or `{"settings": v}`
  for `.json`), then `settings.validate`, which is `Case::semantic_errors` plus the errors of `Simulation::new`, so the mesh is
  built (`crates/rcfd-studio/src/case_edit.rs` lines 126-136 and 255-278, `crates/rcfd-api/src/service.rs` lines 509-516). Text is
  parsed with the base folder `.` (service without a sandbox root, `service.rs` lines 1413-1415; ADR 0131 addendum, question 3), so
  a relative data path resolves against the working folder of the process.
* `rcfd_project::atomic::write_new` creates a file atomically and never replaces one: a temp file is hard-linked to the target, which
  fails if the target exists (`crates/rcfd-project/src/atomic.rs` lines 154-174; used for `run.json` and `case.snapshot`).
  `replace_if_hash(.., None, ..)` checks under the cooperative lock that the file is absent and then renames over it, so a writer
  that does not take the lock can still be replaced in between (its own doc comment, lines 317-325).
* `Project::resolve` refuses empty paths, absolute paths, `..`, `:` in a component, NUL and symlinks (`crates/rcfd-project/src/paths.rs`
  lines 20-88). It does not refuse Windows device names, names ending in a dot or a space, or separators that make a sub-folder;
  `Project::cases()` lists every file under `cases/` at any depth (`project.rs` lines 203-238).
* The project worker thread does every file operation of the browser and the editor; the UI thread only drains its channel
  (`crates/rcfd-studio/src/watch.rs`, ADR 0131 amendment 1 item 3). `Studio::open_case` asks the worker for the file and the
  `Loaded` event opens it in the editor when it is the pending case (`studio.rs` lines 201-208 and 537-546).
* `rcfd-studio` does not depend on `rcfd-mcp` (ADR 0131 Placement), so `rcfd-mcp`'s `lid_driven_cavity` template is not available
  to it. No runnable lid-driven cavity is in `cases/` (only its descriptor).

## What I already knew before registration (rule d smoke)

One throwaway test (`crates/rcfd-studio/tests/zz_smoke_g2.rs`, deleted, never committed) ran once in release on this branch at
f3bf005, before any starter code existed. For every file of `cases/` it called `CaseEditor::from_bytes` and `validate()` exactly as
the editor does, built the mesh with `rcfd_api::sim::build_mesh`, and tried three empty texts. Results:

| File | Editor open | `validate()` | Mesh | Patches of kind `Empty` | Run keys (with defaults) |
|---|---|---|---|---|---|
| `axial_fan_cgrid/axial_fan_cgrid.toml` | ok | true, 0 diagnostics (0.08 s) | 11,536 cells | 0 | simplec, max 6000 iterations |
| `axial_fan_sector/axial_fan_sector.toml` | ok | true, 0 (0.11 s) | 13,332 | 0 | simplec, max 6000 |
| `channel/kepsilon_channel.toml` | ok | true, 0 | 56 | 1 | simplec, max 8000, k-epsilon |
| `channel/kepsilon_inlet.toml` | ok | true, 0 | 288 | 1 | simplec, max 400, k-epsilon |
| `channel/sst_channel.toml` | ok | true, 0 | 240 | 1 | simplec, max 9000, k-omega SST |
| `packed_bed/packed_bed.toml` | ok | true, 0 | 320 | 1 | simplec, max 1700 |
| `rotating_channel/rotating_channel.toml` | ok | true, 0 | 160 | 1 | simplec, max 1700 |
| `sod/sod.toml` | ok | true, 0 | 100 | 1 | compressible, end time 0.2 |
| `wedge/wedge.toml` | ok | true, 0 | 1,200 | 1 | compressible, end time 3.0 |
| `channel/profile_inlet.toml` | **fails**: "tables.inlet_u.file: cannot read table file .\profile_inlet.csv: The system cannot find the file specified." | - | - | - | - |
| the 6 `case.json` | **fail**: unknown settings `id`, `description`, `reference`, `roadmap`, `physics.regime`, `mesh.params`, ... | - | - | - | - |
| empty text, `title` only, comment and `title` | ok | **false**, 2 diagnostics: "boundaries.lid: no boundary condition for mesh patch 'lid'", "boundaries.walls: ..." | 400 cells (`cavity_2d` 20 x 20), patches `lid`, `walls` (walls), `frontAndBack` (empty) | 1 | simplec |

Nothing else of the starter had been written or run.

## Decisions (proposed; none is a maintainer decision)

1. **D1 What is offered.** The runnable case files of `cases/` that open and validate in the editor: the 9 TOML files of the table
   above. Not offered, each with its reason in a list in the code (`EXCLUDED`): the 6 `case.json` descriptors (not case files),
   `channel/profile_inlet.toml` (it reads `profile_inlet.csv` by a relative path; a copy of the one file does not open in the editor,
   whose service resolves the path against the working folder) and `channel/profile_inlet.csv` (input table, not a case). Files under
   a folder named `reference` are reference data and are left out by rule. **Deviation from the task text**, which says the user
   copies "a case.json" and asks for `cases/<name>.json`: the shipped `case.json` files are not cases (smoke above).
2. **D2 Shipping: embedded in the binary.** Each offered file is compiled in with `include_str!` from `cases/` of the repository
   (about 12 KB in all). Options considered:
   * (a) embedded: works from any folder and in any install, cannot go stale against the binary, costs the bytes in the executable,
     and a change to an example needs a rebuild;
   * (b) located next to the executable (`<exe dir>/cases/`): examples can be changed without a rebuild, but every install and every
     `cargo run` needs the folder copied and found, and a missing or edited folder breaks the list at run time;
   * (c) read from the repository at run time: works only in a checkout.
   (a) is taken, as the task prefers. Reference and validation data are never embedded (rule checked by CS11).
3. **D3 Target file.** `cases/<name>.toml`, the bytes of the example unchanged (comments kept). Not `.json`: a conversion would not
   be byte-equal and would drop the comments, and plan section 8 says "case files in the existing case format". The editor's save
   is not used (it writes canonical TOML with every default, ADR 0131 addendum).
4. **D4 Name rule** (`case_starter::check_name`): 1 to 64 characters, ASCII letters, digits, `-` and `_` only, starting with a letter
   or digit; so no spaces, dots, path separators or characters that Windows refuses. A trailing `.toml` or `.json` is refused with
   "give the name without the extension". Windows device names `CON`, `PRN`, `AUX`, `NUL`, `COM0`-`COM9`, `LPT0`-`LPT9` are
   refused in any letter case. A name whose `cases/<name>.toml` or `cases/<name>.json` is already listed is refused, compared without
   letter case on every platform (names that differ only in case are one file on Windows); files in sub-folders of `cases/` do not
   collide. The dialog proposes the example's file stem, or `<stem>-2`, `-3`, ... when that is taken.
5. **D5 Writing.** On the project worker (the UI thread does no file I/O): a new `watch::Command::Create { rel, bytes, open, untrack }`.
   The worker re-checks that `rel` is `cases/<valid name>.toml`, resolves it with `Project::resolve`, creates `cases/` if it is
   missing, writes with `atomic::write_new` (never replaces), re-reads the lists (`Browser::load`) and sends them, then a new
   `Event::Created { rel }`, then, with `open`, tracks the file and sends `Event::Loaded`, which the existing path opens in the
   editor (the Studio sets `pending_case` first). `open` is false when the editor holds unsaved edits, a conflict or a save in
   flight: the case is created and listed, the editor is not replaced and the message says so. A refused write is
   `Event::Failed` with the located error and changes nothing. Creation is not written to `.rcfd/edits.jsonl` (that log records
   cooperative edits through `replace_if_hash`, ADR 0135).
6. **D6 The one-line description** of each example: the title is read from the file's `title` key and the run length from the case
   (`simple`, `simplec`, `coupled`, steady compressible: "at most N iterations"; `piso`, `pimple`, transient compressible: "to t = T"),
   both derived from the embedded text when the list is first built; "what it shows" is curated from the gallery's wording; the
   dimension (2D when the mesh has a patch of kind `Empty`, else 3D) and the cell count are curated, because deriving them builds
   the mesh (0.08 s and 0.11 s for the two fan cases in the smoke) on the UI thread, and a test checks them against the mesh build
   (CS7). The cost shown is the cell count with the run length, an estimate from the case, not a measured time. The list is shown
   cheapest first (by cell count).
7. **D7 No "New empty case".** The task asks for it "if the case schema has a minimal valid starter". The all-defaults case does not
   validate (smoke: no boundary condition for the default mesh's `lid` and `walls` patches), so the entry is not offered. CS10 keeps
   this fact under test; what to offer instead is a question below.
8. **D8 UI.** "New case from example..." in the Case menu (enabled with a project open) and as a button in the Project panel next to
   the case list; a project without cases shows "This project has no cases yet." above it. The dialog lists the examples with their
   one-line descriptions, takes the name, shows the refusal reason or the target path as it is typed, and creates on "Create".

## Placement

No new crate, no new third-party dependency, no new workspace edge: `rcfd-studio` already depends on `rcfd-project` and `rcfd-api`.
`crates/rcfd-cli/tests/workspace_graph.rs` and [`docs/architecture.md`](../architecture.md) are unchanged. New modules
`crates/rcfd-studio/src/case_starter.rs` (examples, exclusions, names, write; no egui) and `case_starter_ui.rs` (dialog and browser
entry); small, localised edits to `lib.rs`, `watch.rs`, `studio.rs` and `ui.rs` (other 48G items edit the same files).

## Registered checks (limits fixed here, before any run)

All in `crates/rcfd-studio/tests/case_starter_g2.rs`, release build, `--test-threads 1`. "The editor's service" is
`CaseEditor::from_bytes(Studio::in_memory().svc, "cases/<stem>.toml", bytes)` followed by `validate()`.

| Id | Check | Limit | Negative control (must fail) |
|---|---|---|---|
| CS1 | Every offered example through the editor's service | 9 of 9 open; `validate()` true with 0 diagnostics | `sod.toml` with `nx = 100` replaced by `nx = "abc"` does not open (message names `mesh.nx`); `kepsilon_inlet.toml` without its `[boundaries.walls]` table opens and `validate()` is false |
| CS2 | Each example created through the Studio model (worker path) into one fresh project | the created file is byte-equal to the embedded text and to the repository's `cases/` file, 9 of 9 | the editor's save bytes of the same example (`CaseEditor::to_bytes`, canonical TOML) differ from the source, 9 of 9 |
| CS3 | Name rule on a project listing `cases/sod.toml`, `cases/Wedge.toml`, `cases/old.json`, `cases/sub/deep.toml` | refused, each with its reason: `""`; `" "`; `"."`; `".."`; `"a/b"`, `"a\b"` (path separator); `"a:b"`, `"a*b"`, `"a?b"`, `"a<b"`, `"a>b"`, `"a\|b"`, `"a\"b"`, `a` NUL `b` (a NUL character between two letters), `"x."`, `"x "`, `"my case"`, `"é"`; `"-x"`, `"_x"`; `"foo.toml"`, `"foo.TOML"`, `"foo.json"` (extension); `"CON"`, `"con"`, `"Con"`, `"PRN"`, `"AUX"`, `"NUL"`, `"COM0"`, `"COM1"`, `"com9"`, `"LPT0"`, `"LPT1"`, `"lpt9"` (reserved); 65 characters (too long); `"sod"`, `"SOD"`, `"wedge"`, `"old"` (exists). Accepted with the path `cases/<name>.toml`: `"x"`, `"sod2"`, `"my-case_2"`, `"deep"`, `"CONX"`, `"COM10"`, `"LPT"`, 64 characters | `"x."`, `"my case"`, `"a/b"`, `"-x"` and the 65-character name pass `Project::resolve("cases/<name>.toml")` on the same project (`CON` and `LPT9` through `resolve` are reported, not limited) |
| CS4 | No overwrite: `cases/sod.toml` holds other bytes | (a) the model refuses the name `sod`, nothing is sent; (b) `case_starter::create` with `cases/sod.toml` is a located "exists" error; (c) with the browser not listing the file yet (watcher off, file written after the open), the model accepts the name and the worker refuses the write: an `Event::Failed` message names the file; in all three the file's hash is unchanged and no `*.tmp` is left in `cases/` | `rcfd_project::atomic::atomic_write` on the same path replaces it (hash changes) |
| CS5 | Sync of the list with `cases/` (`sync_problems(dir)`: every file under `dir` is an offered example with equal bytes, an `EXCLUDED` path, or under a `reference` folder; every offered and every excluded path exists) | 0 problems on the repository's `cases/` | on temp copies: an added `extra/extra.toml`, a deleted `sod/sod.toml`, a deleted `channel/profile_inlet.csv` (stale exclusion) and a changed byte in `wedge/wedge.toml` each give exactly 1 problem naming that file |
| CS6 | The exclusion reasons hold | the 6 `case.json` do not open through the editor's service; `channel/profile_inlet.toml` does not open and the message names `tables.inlet_u.file` | (CS1: the offered files open) |
| CS7 | Curated facts in sync with the mesh | for 9 of 9: `cells` equals `build_mesh(..).n_cells()`, `dim` is "2D" exactly when the mesh has a patch of kind `Empty`; the description contains the file's `title`, the dimension and the cell count | an entry with `cells + 1`, and one with the other dimension, fail the same comparison |
| CS8 | The browser shows the new case and the editor opens it, watcher off | after `create_case_from_example(sod, "first")` on a project without cases: the browser lists exactly `cases/first.toml`, the editor holds it, clean, with `base_hash` equal to the hash of `sod.toml`, and the message says "created cases/first.toml" (wait at most 30 s, as ADR 0131's tests; the time is reported) | a file written to `cases/direct.toml` by the test, not through the starter, is not listed after 2.0 s of draining |
| CS9 | Unsaved edits are kept | with `cases/a.toml` open and edited (`mesh.nx` 16 to 12), a case made from `wedge` is created and listed, the editor still holds `cases/a.toml`, dirty, `mesh.nx` 12, and the message says the new case was not opened | the same with a clean editor: the editor switches to the new case |
| CS10 | No empty starter (the fact behind D7) | the empty text and a `title`-only text open and `validate()` is false with exactly the 2 diagnostics on `boundaries.lid` and `boundaries.walls` | the empty text plus `[boundaries.lid]` and `[boundaries.walls]` with `velocity = "fixed"` validates (what a minimal starter would need) |
| CS11 | Embedded size and content | the embedded texts sum to at most 32,768 bytes; every offered source ends in `.toml` and none is under a `reference` folder | the same rule function fails on the list plus `cavity/reference/ghia1982_u_centerline.csv`, and on sizes summing to 32,769 |
| CS12 | Headless frames | 60 frames at 1280 x 800, theme alternating, a project without cases open, the dialog requested the way the browser button does: no panic, the dialog draws 9 example rows in every frame | (none; drawing only) |
| CS13 | First build of the description list (9 texts parsed with `Case::parse_toml`, on the UI thread at the first open of the dialog) | at most 100 ms, the largest of 5 uncached builds | the same build plus `settings.validate` of each example through the service (what deriving the cell count at run time costs) exceeds 100 ms |
| CS14 | Nothing else moves | the existing `rcfd-studio` tests pass (the process tests where `rcfd-runner` and `rcfd` of the same profile are built, else listed as not run); `cargo clippy -p rcfd-studio --all-targets -- -D warnings` and `cargo fmt -p rcfd-studio --check` clean | (their own controls) |

## Not in CI (by hand, recorded in the addendum)

The real window: the menu entry and the Project panel button, the dialog's layout at 1280 x 800 and high-DPI, keyboard focus in the
name field, both themes; the run of a created case (other 48G items).

## Out of scope

Copying data files next to a case; a JSON variant of the examples; a "duplicate this case" action; deleting or renaming cases;
adding examples to `cases/`; the mesh preview, store default and start-and-follow items of 48G.

## Files

This ADR; `crates/rcfd-studio/src/{case_starter,case_starter_ui}.rs` (new), `crates/rcfd-studio/tests/case_starter_g2.rs` (new);
edits in `crates/rcfd-studio/src/{lib,watch,studio,ui}.rs`.
