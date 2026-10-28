# ADR 0139: Wave 48G item 2, case starter: new case files from the shipped examples

Status: **PROPOSED 2026-10-28, implemented; results in the addendum** (sub-wave 48G, "first run to first picture", item 2 of 4;
the other three items are the mesh preview from a case, the store-fields default and one-click start-and-follow, done on other
branches). Checks registered below before any run of them. Written at main f3bf005. House rules as in ADR 0130: limits are fixed before
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

## Amendment 1 (2026-10-28, before the corrected run): set-up of CS1's validate-stage control

The first registered run of `case_starter_g2.rs` (23 of 24 tests passed; its numbers are in the addendum) stopped in a negative
control's set-up: `kepsilon_inlet.toml` without its `[boundaries.walls]` table does not open ("cases/kepsilon_inlet.toml:
turbulence.model: k-epsilon needs at least one wall patch (a boundary with a fixed zero velocity or turbulence = "wall");
reports.yplus_walls.patch: y_plus needs a wall patch with a boundary entry, 'walls' is not one"). The k-epsilon model and the y+
report refuse the text when it is parsed, so the control never reached `validate()`, the stage it was registered to show. The
editor's service did refuse the broken example, but at the open stage, which the other control already shows.

Corrected set-up (the limits and the open-stage control are unchanged): the validate-stage control uses
`rotating_channel/rotating_channel.toml` (laminar, no reports) without its `[boundaries.ymax]` table. It must open, and `validate()`
must be false with a diagnostic on `boundaries.ymax`. That diagnostic comes from the boundary-coverage loop of `Simulation::new`
(`crates/rcfd-api/src/sim.rs` lines 1053-1064). The loop skips cyclic and empty patches, so of this mesh's patches only `ymin` and
`ymax` need a boundary entry. What I knew when writing this: the message above and that code. The corrected text had not been run.

## Not in CI (by hand, recorded in the addendum)

The real window: the menu entry and the Project panel button, the dialog's layout at 1280 x 800 and high-DPI, keyboard focus in the
name field, both themes; the run of a created case (other 48G items).

## Out of scope

Copying data files next to a case; a JSON variant of the examples; a "duplicate this case" action; deleting or renaming cases;
adding examples to `cases/`; the mesh preview, store default and start-and-follow items of 48G.

## Addendum: results (2026-10-28, branch `worktree-agent-a38bcefbdba6707af`, release build on the gate host)

Commits on main f3bf005: 8b78d17 (this ADR, registration), b06f4a5 (implementation and tests), 0761022 (amendment 1), then this
addendum. Run 1 (at b06f4a5): 23 of 24 tests passed; the CS1 validate-stage control's set-up was defective (amendment 1). Run 2 (at
0761022, after the amendment): 24 of 24 passed. The numbers below are from run 2. Run 1's numbers were the same wherever run 1 got
that far, except the timings (CS13 7.386 ms, its control 212.327 ms).

| Id | Limit | Measured | Result |
|---|---|---|---|
| CS1 | 9 of 9 open; `validate()` true, 0 diagnostics | 9 of 9, 0 diagnostics each; 0.002 to 0.008 s per 2D case, 0.095 s (C-grid fan) and 0.123 s (sector fan) | pass |
| CS1 controls | open stage: refused, naming `mesh.nx`; validate stage (amendment 1): opens, `validate()` false on `boundaries.ymax` | "cases/sod.toml: mesh.nx: expected integer, got "abc""; the rotating channel without `[boundaries.ymax]` opens, `validate()` false, "boundaries.ymax: no boundary condition for mesh patch 'ymax'" | fail as required |
| CS2 | byte-equal, 9 of 9 | 9 of 9 equal to the embedded text and to the `cases/` file (664 to 2,041 bytes; for example `cases/sod.toml`, 1,058 bytes, fnv64:f4c2a21b32815512) | pass |
| CS2 control | the editor's save bytes differ, 9 of 9 | 9 of 9 differ: 16,122 to 22,089 bytes (canonical TOML with every default) | fails as required |
| CS3 | 40 refusals with their reasons, 8 acceptances | 40 refused, each message naming its rule ("a case needs a name", "'.' is not allowed: use letters, digits, '-' and '_' only (no spaces or dots)", "no path separators: ...", "start the name with a letter or a digit", "give the name without the extension (.toml is added)", "at most 64 characters (this name has 65)", "'con' is a reserved device name on Windows", "cases/Wedge.toml already exists"); 8 accepted with `cases/<name>.toml` | pass |
| CS3 control | the 5 names pass `Project::resolve` | `x.`, `my case`, `a/b`, `-x` and the 65-character name all resolve (`cases\x..toml`, `cases\a\b.toml`, ...). Reported: `cases/CON.toml` and `cases/LPT9.toml` also resolve on this host | fails as required |
| CS4 | (a), (b), (c) refused; hashes unchanged; no `*.tmp` | (a) "cases/sod.toml already exists", nothing sent (no message within 1 s); (b) "...\cases\sod.toml: already exists"; (c) the model accepted `late`, the worker refused with "...\cases\late.toml: already exists" (`Event::Failed`), no editor and no pending case; both files' hashes unchanged; no `*.tmp` in `cases/` | pass |
| CS4 control | `atomic_write` changes the hash | fnv64:62f6a715c336b05d to fnv64:f4c2a21b32815512 | fails as required |
| CS5 | 0 problems | 24 files in `cases/`: 9 offered, 8 excluded, 7 reference data; 0 problems; no path both offered and excluded, no duplicate, every reason given | pass |
| CS5 controls | exactly 1 problem each, naming the file | added: "extra/extra.toml: shipped but neither offered nor excluded"; deleted: "sod/sod.toml: offered but not in cases/"; stale: "channel/profile_inlet.csv: excluded but not in cases/ (stale exclusion)"; changed: "wedge/wedge.toml: the embedded copy differs from the file" | fail as required |
| CS6 | the 6 descriptors and `profile_inlet.toml` do not open | 7 of 7 refused: the descriptors on unknown settings (`description`, `id`, `boundary_conditions`, ...); `profile_inlet.toml` with "tables.inlet_u.file: cannot read table file .\profile_inlet.csv: The system cannot find the file specified." | pass |
| CS7 | cells, dimension and title, 9 of 9 | 9 of 9 equal; for example "Sod shock tube: Shock tube with the compressible Euler solver (...), one cell high. 2D, 100 cells, compressible, to t = 0.2." | pass |
| CS7 control | `cells + 1` and the other dimension fail | "sod/sod.toml: 101 cells listed, the mesh has 100"; "sod/sod.toml: listed 3D, the mesh is 2D" | fail as required |
| CS8 | listed exactly; editor clean with the source's hash; message | `cases/first.toml` listed alone, open, clean, `base_hash` of `sod.toml`, "created cases/first.toml"; 0.007 s from the call to the editor holding it | pass |
| CS8 control | not listed after 2.0 s | not listed | fails as required |
| CS9 | editor keeps `cases/a.toml`, dirty, `mesh.nx` 12; the message says not opened | as stated: "created cases/w.toml; not opened, because the open case has unsaved edits or a conflict (save or resolve them, then select cases/w.toml in the Project panel)"; `cases/w.toml` byte-equal to `wedge.toml` | pass |
| CS9 control | a clean editor switches | switched to `cases/w.toml` | as required |
| CS10 | empty and `title`-only text invalid with the 2 diagnostics | both: `validate()` false, `boundaries.lid` and `boundaries.walls` ("no boundary condition for mesh patch ...") | pass |
| CS10 control | with the 2 tables it validates | true, 0 diagnostics | fails as required |
| CS11 | at most 32,768 bytes, `.toml` only, no reference data | 11,908 bytes, 9 `.toml` files, none under `reference/` | pass |
| CS11 control | the reference CSV and 32,769 bytes fail | 2 problems ("not a .toml case file", "reference data"); 1 problem ("32769 bytes embedded, more than 32768") | fails as required |
| CS12 | 60 frames, no panic, 9 rows each | 60 frames, 9 rows in each, at least 174 shapes per frame; proposed name `kepsilon_channel` | pass |
| CS13 | at most 100 ms | 9.916 ms, the largest of 5 builds | pass |
| CS13 control | more than 100 ms | 244.453 ms | fails as required |
| CS14 | the other `rcfd-studio` tests pass; clippy and fmt clean | `cargo test -p rcfd-studio --release --no-fail-fast -- --test-threads 1` (with `rcfd-runner`, `rcfd` and `fake-claude` built in release next to the Studio): 21 integration binaries, 103 passed (CS1-CS13 included), 0 failed, 2 ignored. The 2 ignored are the existing known misses `parity_tutorials_name_the_gui_known_miss` (ADR 0131 B9) and `r6_tick_with_four_large_runs_is_within_5_ms_known_miss` (ADR 0133); B6's 4 child-process runs also passed. Wall time 2 min 18 s. An earlier attempt without `fake-claude` built failed the 14 tests of `chat_e.rs` in their set-up ("fake-claude.exe missing: cargo build -p rcfd-claude --bin fake-claude with this profile", ADR 0134 amendment 3) and stopped there. `cargo clippy -p rcfd-studio --all-targets -- -D warnings` (debug) and `--release`: clean. `cargo fmt -p rcfd-studio --check`: clean | pass |

No known miss.

**Deviations and notes.**
1. The task text asks for `cases/<name>.json` and says the user copies "a case.json". The shipped `case.json` files are benchmark
   descriptors that the editor refuses (CS6). The starter offers the 9 runnable TOML files and writes `cases/<name>.toml`, byte-equal
   to the source (D1, D3).
2. No "New empty case" (D7, CS10): the all-defaults case does not validate. CS10's control shows what a minimal valid text needs:
   the two boundary tables. That text is a 20 x 20 box with all walls fixed and no flow (question 4).
3. Amendment 1: CS1's validate-stage control moved from the k-epsilon inlet channel (refused at open) to the laminar rotating
   channel.
4. `watch.rs` is edited (a `Create` command and a `Created` event), besides the shared files the task named. The edits stay small:
   `lib.rs` +2 lines, `studio.rs` +2 and -1, `ui.rs` +12, `watch.rs` +47.
5. Found, not fixed: `Project::resolve` accepts Windows device names (`cases/CON.toml` and `cases/LPT9.toml` resolved on this
   Windows 11 host, CS3 report). The starter's name rule refuses them. Other writers of a project (for example `write_note` of ADR
   0135, which creates files under `notes/`) do not have that rule (question 9).
6. Creating a case does not write a line to `.rcfd/edits.jsonl` (D5, question 6).
7. The run-length text is derived from the case: "simplec, at most 6000 iterations" or "compressible, to t = 3". It is not a measured
   time.
8. Set-up detail not written in the table: the Studio models of CS2, CS4, CS8, CS9 and CS12 run without the run worker
   (`run_poll = None`), because these tests start no runs. CS4 and CS8 run with the watcher off, as registered. CS9 and CS12 also
   run with it off; the table does not say so, and neither check depends on the watcher. CS2 runs with the watcher on.
9. `scripts/limit_audit.py` hits in the new test file, each read:
   * five class (iii) hits, on the controls of CS2, CS5, CS9, CS10 and CS11. Each control asserts the failure it shows as a positive
     fact: 9 differing files, exactly 1 problem, the editor switched, `validate()` true where CS10 requires false, 2 problems and 1
     problem. The heuristic does not recognise this form, as with ADR 0131's controls;
   * two class (iv) hits: the debug-build skips of CS13 and its control, each with a `SKIP` line. The limit is for release, and the
     gate is release.

**Not run.** The "Not in CI" items (a person looking at the window). Also not run: a debug build of the new tests (CS13 and its
control print `SKIP` there by design, as B7 does); Linux; the workspace gate; workspace-wide clippy (only `-p rcfd-studio`, as
asked); `cargo deny` (no new dependency).

**View-and-report checklist for the maintainer.**
1. Build `cargo build --release -p rcfd-studio`. To run a created case, also build `-p rcfd-runner -p rcfd-cli`. Start
   `target\release\rcfd-studio.exe`.
2. Project > New project...: make an empty project. The Project panel shows "This project has no cases yet." and a "New case from
   example..." button. The Case menu has "New case from example...", enabled only while a project is open.
3. Click the button. A "New case from example" window lists 9 examples, cheapest first. Each shows its title in bold and one line
   below it: what it shows, 2D or 3D, cells, solver and run length. Check that the text is readable and that the window fits your
   screen.
4. Select "Sod shock tube". The name field proposes `sod`, and the line under it reads "creates cases/sod.toml from the example
   sod/sod.toml". Click Create. The window closes, the Project panel lists `cases/sod.toml` highlighted, the Case editor shows it,
   and the status bar says "created cases/sod.toml".
5. Open the window again and select Sod: the proposed name is now `sod-2`. Type `sod`: a red line says "cases/sod.toml already
   exists" and Create is disabled. Try `CON`, `a/b`, `my case` and `x.toml`: each gives its reason. Type a valid name and press
   Enter: the case is created.
6. Open `<project>\cases\sod.toml` in a text editor. It should equal `cases\sod\sod.toml` of the repository, comments included.
7. Edit a value in the Case editor without saving, then create another case. The new case appears in the Project panel, the editor
   keeps your edited case, and the status bar says the new case was not opened.
8. Switch the theme (View menu) and resize the window. The dialog should stay usable.
9. Report anything that looks wrong or confusing: the descriptions, the order, the proposed names, the wording. Say what you did.

**Questions for the maintainer.**
1. Is offering only the TOML case files right (deviation 1)? Or should the Studio also write JSON (a conversion, so not byte-equal
   and without the comments)?
2. `cases/` has no runnable lid-driven cavity, only its descriptor. The runnable text is `rcfd-mcp`'s template, and the Studio may
   not depend on `rcfd-mcp`. Add `cases/cavity/cavity.toml`? It would then be added to the list, as CS5 requires. Or move the
   template to a crate both can use?
3. `channel/profile_inlet.toml` needs its CSV. Options: copy both files (but `Project::cases()` lists every file under `cases/`, so
   the CSV would show as a case); keep it excluded; or offer it once the editor resolves data paths against the case's folder (ADR
   0131 question 3).
4. "New empty case": offer the minimal valid text (CS10 control: defaults plus fixed `lid` and `walls`, a box with no flow), offer the
   cavity of question 2 as the starter, or offer nothing?
5. The name rule allows ASCII letters, digits, `-` and `_`, up to 64 characters, and compares names without letter case. Should it
   also allow spaces, dots or non-ASCII letters?
6. Should creating a case write a line to `.rcfd/edits.jsonl`, like the cooperative edits of ADR 0135?
7. `channel/kepsilon_channel.toml` and `channel/sst_channel.toml` set `output.directory = "results"`. Does that interact with the
   runner's `--out runs/<id>/output`? This is for item 4 (start-and-follow); it is flagged here, not checked.
8. Is cheapest-first order right? The dialog proposes the first entry, `kepsilon_channel` (56 cells). Should another example come
   first, for example `wedge`, whose picture shows a shock?
9. Should `rcfd-project` refuse Windows device names for every writer (deviation 5)?

## Files

This ADR; `crates/rcfd-studio/src/{case_starter,case_starter_ui}.rs` (new), `crates/rcfd-studio/tests/case_starter_g2.rs` (new);
edits in `crates/rcfd-studio/src/{lib,watch,studio,ui}.rs`.
