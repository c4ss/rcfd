# ADR 0148: Job 48N, the selected case is a piece of state of its own, the middle drag orbits, the Run page says its units and offers a finer mesh, and three answers to ADR 0147 (data files, a save not built again, the Start dialog waits)

Status: **registered 2026-10-28**, before any registered run (the code and the tests below were written and compiled
first; what had run before is listed under "What I already knew"); not yet accepted by the maintainer. Job 48N, on local
main 7eb8b79 (waves 48A-48M merged). House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits
are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a
defective set-up gets an amendment written before the corrected run; defaults of existing code stay bit-identical outside
the Studio; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new crate, no unsafe code, the crate graph
stays acyclic. Clean-room: our own Studio code and the egui and egui_plot public APIs.

## What the maintainer reported and asked (2026-10-28, relayed)

The maintainer opened the real window on `example-projects/wind-tunnel` and `example-projects/quick-start`:

1. "Run and mesh tabs aren't linked to the selected Case toml file - it should be because I can't run any other case but
   the first." Asked: reproduce with a headless test first; find the real cause; make the selection a piece of state set
   by the click, followed by Run, the Mesh preview, the Project panel's highlight, the toolbar's label and the case
   editor; when the editor cannot open the selected case, it shows that case's error while the Mesh side and Run use the
   file on disk; registered checks with three or more cases, selected in turn, also before the case was read and with a
   failing case or a benchmark descriptor between, in both orders, and with unsaved edits in the case left (the "save it
   first" rule holds for it; what happens to its edits on a switch stays as it was unless it is the cause, and is
   reported).
2. "Middle click should be to rotate the camera around." The middle drag orbits, the secondary (right) drag pans, the
   primary drag still orbits (2D: as today); the mouse texts and the ADRs that describe the mapping are updated; the
   mapping is tested as a function and by drawn drags.
3. The accepted small items of [ADR 0147](0147-job48m-run-gate-camera-skip-starter-data-files-run-order.md)'s questions:
   Q3 (b) (the Project panel lists `.toml` and `.json` files as cases and the other files under "Data files"; the Studio
   only, not `rcfd-project`), Q2 (b) (a save's rebuild is skipped when the saved bytes are the shown text and the mesh file
   is unchanged; its own registered check), Q4 (b) (the Start run dialog's Start is greyed out with Run's text while the
   case has unsaved edits or a field being typed). Q1 (a) (the camera's orientation kept on a refit) and Q5 (the runner
   list's order): left as they are.
4. Extra scope, relayed later the same day (the maintainer looked at the Run page): "What scale are we at? can i increase
   the resolution? the scale isn't clear. what does 1t represent in time?" Asked: find the element that reads "1t" and
   report it; (a) every time shown says what it is (`time (s)`, the word "simulated time"; a steady run's iterations, not a
   time), the slider and the snapshot line in the same words, with hover text; (b) a scale bar or axis ticks with units on
   a 2D view, and the domain size with the cells in the Mesh caption and the Run caption, in the case's length unit (SI:
   metres); (c) the cell counts in the Mesh caption and a "Refine x2" next to them that doubles `nx`, `ny` (`nz`) of the
   open case in the case editor (an unsaved edit; the preview follows; Run still asks for a save), only for generator
   meshes ("resolution is set by the mesh file" for a mesh file), with the memory and time warning of the existing size
   limit (2M cells). Tested headlessly.

## Context and diagnosis (read at 7eb8b79, and measured before any change, see "What I already knew")

* `quick_run::selected_case` (Run, F5, the toolbar's label) was "the case editor's case, else the case being read":
  `st.editor.rel` first, `st.pending_case` second. The Project panel's highlight used the same rule (`ui.rs`). The preview
  took the clicked case at once (`Studio::open_case` calls `show_case_mesh`, which sets `Preview::case`); the case editor
  changed only when the worker's `Event::Loaded` of that case arrived.
* `Event::Loaded` with an editor error (a benchmark descriptor, a data file listed under Cases such as
  `cases/profile_inlet.csv`, a case that does not parse or validate) kept the editor's old case.
* `Event::Failed` of any cause (the failed read of a file asked for, a failed save, a watcher error) cleared
  `pending_case`; a later `Loaded` of the case clicked then did not match it and was dropped.
* `Msg::Queued` (Run's start) selected the queued run and switched the Viewport to it (`select_run`).
* Selecting a run in the Runs panel changes neither the case editor nor the Mesh side nor Run's case (ADR 0144's toggle);
  the Run side shows the selected run, whatever case it is of.
* The case starter refuses to open a new case over a case editor with unsaved edits; a click in the Project panel drops
  them (the editor is replaced when the new case arrives).
* `Project::cases` lists every file under `cases/` (ADR 0147 decision 4 lists the copied table there).
* After a save, the editor's base hash changes and the preview builds the file again; the watcher's notice of the written
  file asks for a build too (ADR 0147 decision 3: only the debounce's builds are compared).
* The Start run dialog plans and starts the file on disk whatever the case editor holds (ADR 0147 question 4).
* Times on the Run page: the slider's label `time` (or `step`), the line under it `snapshot i/n  step s  t = <time>`, the
  strip `step N  t <time:.4e>`, the Runs panel's column `time` (`{:.4e}`), the Outputs panel `step N, time T`; the plots'
  x axis has no label. A steady run stores time 0 at every snapshot, and these lines show `t = 0`. The settings carry SI
  units (`docs/case-schema.md`: "The settings carry SI units"; `solver.dt`, `solver.min_dt` in s; lengths have the SI
  dimension of length), so simulated time is in seconds and lengths in metres (a mesh file's numbers times `mesh.scale`).
* The Mesh side shows the counts of the built mesh (`preview_ui::counts`: cells, faces, patches, surface) and no domain
  size; the Run side's caption is `stored fields of run <id>`; neither view shows a scale. The preview's size limit is
  2,000,000 estimated cells (`preview::MAX_CELLS`); the runner's memory estimate of a run is `mem_base_mb +
  mem_kb_per_cell` per cell (64 MB + 4 KB per cell, `rcfd_runner::runner::Config::default`).
* Mouse (`viewport_ui::image`): the primary drag orbits, the secondary or the middle drag pans.

### Root cause of report 1 (what the probes showed, "What I already knew" gives the numbers)

The Studio had no selected case of its own: Run, F5, the toolbar's label and the highlight followed the case editor,
the Mesh side followed the click. That has three consequences, each reproduced headlessly:

* **(R1)** Between a click on B and B's `Loaded`, Run, F5, the label and the highlight are still A while the Mesh side
  shows B (a few milliseconds to one or two frames on the example projects; longer while the project worker is busy).
* **(R2)** When the case editor cannot open B, it keeps A, and so do Run, F5, the label and the highlight, for good,
  while the Mesh side shows B's failure (reproduced with a benchmark descriptor).
* **(R3)** A failed read of any file ends the wait for the case clicked after it: B's `Loaded` is dropped and A stays in
  the editor and in Run, for good, while the Mesh side shows B (reproduced with a listed case removed from disk, clicked,
  then B clicked before a drain).
* **(R4)** A run that Run queued after another case was clicked takes the Viewport from that case's mesh to the run (a
  probe: Run on a, c clicked before the run was queued: the Viewport showed a's run while c was selected).

**Not reproduced:** on copies of both example projects, all 10 cases open in the case editor, and a click (through the
model and drawn) made the editor, the Mesh side, Run and the highlight follow within one read; Run then queued a run of
the case clicked. So R1-R3 alone do not make "only the first case" on those projects. **(R5)**, ADR 0144's rule, does:
after a run of another case is selected in the Runs panel (each row names its case), the Mesh side and Run keep the case
clicked last in the Project panel, and the Run side keeps showing the run selected, whatever case is clicked (a probe's
dump had the case editor on `naca0012-aoa4` while the Run side showed a cylinder run). Which of R1-R5 the maintainer met
is not known; the ones that keep the first case for good are R2, R3 and R5. R1-R4 are fixed here; R5 is a question
(question 1), not changed.

**"1t":** no element draws `1t` (probes at 1280 x 800 at scales 1, 1.25 and 1.5, at 1024 x 768, at 1920 x 1080 at 1 and
1.5, at the first snapshot of `cylinder-re100`; egui_plot 0.37's hover label is `x = 1.000` / `y = ...`, its axis labels
numbers). The candidates, all without a unit: the line under the slider `snapshot 1/80  step 16  t = 1` (the most likely
reading: a bare `t` beside a 1), the slider's value `1.0` followed by its label `time`, the strip's `t 8.0000e1`, and the
Runs panel's time column `8.0000e1`. Decision 6 words every one of them.

## Decisions (as implemented)

1. **The selected case** (`Studio::selected_case`; `quick_run::selected_case` returns it). Set by a click in the Project
   panel (`Studio::open_case`) and by a case the case starter made and opened (`case_starter::on_created`); cleared when
   the project closes. Run and F5, the toolbar's label, the Project panel's highlight, the Mesh side (unchanged: it took
   the click already), the Runs panel's "Start run..." dialog (its case) and the case editor follow it.
   * (a) The click empties the case editor at once (`Loading <case>...`); the editor opens the selected case when its
     bytes arrive. The unsaved edits of the case left are dropped at the click (before: when the new case arrived, and
     kept when it could not be opened). Not the cause of the report; question 2.
   * (b) When the case editor cannot open the selected case (`Studio::editor_error`), it holds nothing and shows
     `<case> cannot be opened in the case editor:` with the reason (a descriptor: ADR 0147's one-line message) and `The
     Mesh side and Run use the file on disk.`; the status bar says the reason as before. Run starts the file on disk: a
     file that is not a case is refused by the start with its located message (`<case> was not started: ...`).
   * (c) The worker reports a failed read of a file asked for as `Event::LoadFailed { rel, message }` (new); only the
     failure of the file asked for last ends the wait. `Event::Failed` no longer touches the wait.
   * (d) A run that Run queued after another case was selected is selected (the plots, the log and the strip follow it),
     the Viewport stays on the selected case's mesh and no panel is brought to the front (`Studio::select_run_keep_view`,
     `keep_view_side`). Run of the selected case behaves as before (ADR 0141: the Viewport follows its run).
   * (e) Run's refusals for the case being edited (a conflict, a field being typed, unsaved edits:
     `quick_run::editor_blocks`) apply when the case editor holds the selected case, as before; right after a click the
     editor holds nothing, so the edits of the case left do not hold up Run of the new one.
   * Test switch `QuickRun::switches.selection_from_editor`: the route before (the editor-first selection and highlight,
     the editor kept until another case loads and when another cannot be opened, a failure of any kind ending the wait, a
     queued run switching the Viewport).
2. **The middle drag orbits** (`viewport::drag_input`, `DragButton`, `MOUSE_HINT`; the maintainer's request): the
   primary and the middle drag orbit, the secondary drag pans, in 3D and in 2D (the primary drag orbited a 2D view before
   and still does). `Input`'s documentation says so; the hints under the image (`left or middle drag: rotate; right drag:
   pan; wheel: zoom; double click: fit`) are on both sides of the Viewport and on the Mesh side's control line (3D and
   2D). ADR 0132 and ADR 0138 get dated notes. Test switch `viewport::Switches::middle_pans` (ADR 0132's mapping).
3. **Data files** (ADR 0147 Q3 (b); `project_view::is_case_file`, `split_cases`): the Project panel lists `.toml` and
   `.json` files (any case of the extension) under `Cases (n)` and the other files under `Data files (n)` (plain labels
   with a hover note, not selectable); the Start run dialog lists the case files only. `rcfd-project`'s `Project::cases`
   is unchanged (every file). Test switch `view_toggle::Switches::data_files_as_cases`.
4. **A save of the shown text is not built again** (ADR 0147 Q2 (b); `preview::build_unless`, `Preview::want_check`,
   `Stats::skipped_saved`). When the case editor's base hash changes to the shown mesh's `case_hash` (a save of the text on
   screen), and when the watcher reports the case file while a clean case editor's base is the shown text (the notice of
   that save), the build compares the file with the shown mesh on the build thread first: when its bytes hash to the
   shown mesh's `case_hash` and its mesh file is unchanged (`mesh_input_unchanged`, ADR 0147), nothing is parsed or built
   (`Refusal::Unchanged`); the shown mesh is the file's from then on (`from_editor` false: the source line says it is
   built from the case file on disk; the same surface, no upload, the camera kept; `read_hash` the saved hash). Any other
   change of the file, a save of another text, "Rebuild", "anyway" and a new case build as before. Test switch
   `preview::Switches::no_save_skip` (ADR 0147's rule).
5. **The Start run dialog waits for the case editor** (ADR 0147 Q4 (b); `quick_run::start_blocked`, `editor_blocks`):
   while the case editor holds the dialog's case in conflict, with a field being typed or with unsaved edits, Start is
   greyed out with Run's text as its hover text; another case in the dialog is not held up. Test switch
   `QuickRun::switches.start_ignores_editor`.
6. **Every time says what it is** (the extra scope (a); `viewport::RunKind`, `run_kind`, `run_kind_of_time`,
   `slider_words`, `step_time_text`, `time_hover`, `fmt_num`, `TIME_UNIT`). A run is transient when a stored snapshot's
   time is not 0 (or, for a run's row, its last time is finite and not 0), else steady (a transient run whose only
   snapshot is at time 0 reads as steady). The slider's label: `simulated time (s)` on a time axis, `time step` on the
   step axis of a transient run (fewer than two snapshots), `iteration` for a steady run. The snapshot line:
   `snapshot i/n  time step N  simulated time T s`, or `snapshot i/n  iteration N` (no time: a steady run has none).
   Both have the hover text (`Simulated time of the stored snapshots, in seconds: the case settings are SI units ...` or
   `A steady run has no simulated time: ...`). The strip: `time step N  simulated time T s` or `iteration N` in place of
   `step N  t T`. The Runs panel's header `time (s)` (hover note), with `-` for a run without simulated time. The Outputs
   panel the same words. The plots' x axis: `time step` (transient), `iteration` (steady), `step` (no run row). Numbers:
   at most 4 significant digits, no trailing zeros. Test switch `viewport::Switches::bare_time` (the words before).
7. **A scale and the domain** (the extra scope (b); `viewport::scale_bar`, `ScaleBar`, `domain_text`, `LENGTH_UNIT`,
   `Viewport::cells`, `Studio::run_readout`, `preview::resolution_text`, `Built::bounds`). A 2D view (an orthographic
   camera) of either side has a scale bar in the image's bottom-left corner: the longest of 1, 2 or 5 times a power of
   ten metres that is at most a quarter of the image wide, with its length above it (`0.2 m`), on a translucent plate,
   drawn after the preview's outlines; it follows the zoom. A 3D (perspective) view has none (its scale changes with
   depth). The Mesh side gets a resolution line under its caption: `domain 1 x 1 m; 16 x 16 = 256 cells` (when the counts
   multiply to the cells), `domain ... m; N cells (nx 4, ny 3, nx_right 5, ny_right 2)` (otherwise), `domain ... m; N
   cells; the resolution is set by the mesh file`, `domain ... m; N cells (generator axial_fan_sector)`. The Run side's
   caption gets the readout `case <case>, domain 30 x 30 m, 3,072 cells`, from the run's own stored mesh (not the
   selected case). The domain is the mesh's bounding box: x and y of a 2D case, x, y and z of a 3D one. Test switch
   `viewport::Switches::no_scale` (no bar, no domain, no resolution line, so no "Refine x2").
8. **Refine x2** (the extra scope (c); `preview::count_keys`, `resolution_of`, `Resolution`, `refine_offer`,
   `Refinement`, `refine_blocked`, `refine`, `refine_hover`, `runner_mem_mb`; `preview_ui::REFINE`). On the resolution
   line of a generator mesh whose cells are products (or sums of products) of its counts: the 2D generators and
   `cylinder_ogrid` (`mesh.nx`, `mesh.ny`), the 3D annulus generators and `pipe_sweep` (`mesh.nx`, `mesh.ny`, `mesh.nz`),
   with the `_right` counts of a second block that are set (0 means "as the first block" and follows it). Not offered
   for `axial_fan_sector` (its counts are `mesh.fan.n_*`; said on hover) or a mesh file (the line says the resolution is
   the file's). A click doubles every count in the case editor through `set_value`, after checking each against its
   schema maximum (100,000): an unsaved edit, so the preview builds it after the debounce and Run says "save it first".
   Its hover text: the counts before and after, the cells (4 times in 2D, 8 times in 3D; exact for these generators,
   every count doubling), "each iteration does about that much more work", the runner's memory estimate of a run of it
   (64 MB + 4 KB per cell) and, when the finer mesh is over the preview's cell limit (2,000,000), that the preview asks
   before building it (the button is then in the warning colour). The message: `<case>: refined x2 (mesh.nx 16 -> 32,
   mesh.ny 16 -> 32): 256 -> 1,024 cells, unsaved (save the case before Run)` (and the limit, when over it). Greyed out,
   with the reason, while the case editor does not hold the case, holds it in conflict or with a field being typed, or
   while the mesh shown is not the case's current text (stale). No time estimate beyond the per-iteration factor (none
   was measured).

## Placement

`crates/rcfd-studio` and `docs/`. Hunks in `src/{studio,watch,quick_run,case_starter,view_toggle,project_view,ui,
preview,preview_ui,viewport,viewport_ui}.rs`. No new crate, no new dependency, no new workspace edge (`rcfd-runner` was a
dependency already); `Cargo.toml`, `Cargo.lock`, `crates/rcfd-cli/tests/workspace_graph.rs` and `docs/architecture.md`
unchanged. Nothing outside the Studio changes.

## Registered checks (limits fixed here, before any of them ran)

Release builds, `--test-threads 1 --nocapture`, output to files, this host; `rcfd-runner` and `rcfd` of the same build
next to the test's `rcfd-studio` (`common::binaries`). "Settled": nothing being read and the preview neither building
nor waiting for its debounce (`lj::settled`). "Run before it was read": `run_selected_case` (the call F5 makes) right
after `open_case`, with no drain between. Drawn frames are 1280 x 800 at scale 1, the default layout. The runner-served
fixture (N1, N3, N4): ADR 0133's `RunnerProject` (budgets `max_runs` 3) with `cases/a.toml`, `b.toml` and `c.toml` (the
12 x 12 cavity of ADR 0133 at `nx` 8, 10 and 14, 30 iterations) and `cases/cav/case.json` (the shipped
`cases/cavity/case.json`), served. Every wait has a 30 s limit (an upper bound, not a timing claim). Files:
`crates/rcfd-studio/tests/selection_n.rs` (N1-N4), `followups_n.rs` (N5-N8), `scale_n.rs` (N9-N11).

| Id | Check | Limit | Negative control (must fail) |
|---|---|---|---|
| N1 | The selected case, drawn app (watcher and run worker on): (a) `a` clicked in the Project panel, settled, Run clicked; (b) `b` selected and Run before it was read; (c) the descriptor clicked, settled, Run; (d) `c` selected and Run before it was read; (e) `a`, `b`, `a` selected in a row with no frame between, then Run. After each, settled, a drawn frame | in each step: `selected_case` is the case of the step; the Project panel highlights exactly that case label (drawn on the selection colour); the toolbar draws exactly that case right of Run; the Mesh side's case is it. (a), (b), (d), (e): Run queued a run whose `case.snapshot` is byte-equal to that case's file; the Viewport on the Run side (that run, ADR 0141); the editor holds that case, no editor message; the shown mesh's hash is `build_mesh(Case::load(<the file>))`'s. (c): no run queued; the start's error starts `cases/cav/case.json was not started: `; the Viewport on the Mesh side; the editor holds nothing and its message is for `cases/cav/case.json` | the route before (`selection_from_editor`): (b)'s run holds `cases/a.toml`, (c)'s run holds `cases/b.toml` |
| N2 | A failed read and a file the editor cannot open (ADR 0138's project with `cases/b.toml`, the cavity at 10 x 10, `cases/gone.toml` and `cases/profile_inlet.csv`; watcher and run worker off; the cavity open): (a) `cases/gone.toml` removed, selected, then `b` selected before a drain, 1.5 s of drains; (b) `cases/profile_inlet.csv` selected (through the model), 1.5 s; (c) `b`, 1.5 s | after each: (selected, editor, the case of the editor's message, the Mesh side's case, the shown mesh is that case's own) is (a) (b, b, none, b, yes), (b) (the csv, none, the csv, the csv, no), (c) (b, b, none, b, yes) | the route before: after (a) the selection and the editor are `cases/cavity.toml` while the Mesh side is `b` |
| N3 | Unsaved edits of the case left (N1's fixture, drawn app): (a) `a` opened, `mesh.nx` set to 9; (b) `b` selected: Run's refusal at once, then Run before it was read; (c) `a` selected again, settled; (d) `mesh.ny` of `a` typed "11" (not committed), `b` selected and Run at once | (a) Run's refusal is exactly `cases/a.toml has unsaved edits: save it first (Case > Save); a run uses the file on disk`; (b) no refusal at the switch; the run holds `cases/b.toml`; (c) the editor's `mesh.nx` is 8 (the file) and it is clean (the edits were dropped: the behaviour before, reported); (d) the refusal while typed starts `cases/a.toml: finish editing the field mesh.ny`; the run holds `cases/b.toml` | the route before: (b)'s refusal at the switch starts `cases/a.toml has unsaved edits` |
| N4 | Another case selected while Run starts (N1's fixture): `a` open and settled; Run, then `c` selected at once; the start ended and settled; then Run of `c` | after the first: the queued run is the selected run, it holds `cases/a.toml`, the Viewport is on the Mesh side, shows no run, and the Mesh side's case is `c`; after Run of `c`: its run selected, holding `cases/c.toml`, the Viewport on the Run side showing a run | the route before: after the first, the Viewport is on the Run side |
| N5 | The mouse (ADR 0138's project, watcher and run worker off): `drag_input` for each button; drawn drags over the image (pressed at its centre, 4 moves of (12, 7) points in 4 frames, released) with the primary, the middle and the secondary button, on the hex box (3D) and the cavity (2D) | `drag_input`: primary `Orbit`, middle `Orbit`, secondary `Pan` (with `middle_pans`: middle `Pan`); the drawn primary and middle drags change yaw or pitch and leave the target bit for bit, the secondary drag changes the target and leaves yaw and pitch bit for bit, in 3D and 2D | ADR 0132's mapping (`middle_pans`): both middle drags change the target and leave yaw and pitch |
| N6 | Data files (ADR 0138's project with `cases/b.json`, `cases/profile_inlet.csv`, `cases/notes.txt`; watcher off): the drawn Project panel; a click on the csv label; the Start run dialog opened with no case, its case list opened by a click (3 frames) | the headers drawn are exactly `Cases (3)` and `Data files (2)`; after the click nothing is selected; with the list open each case file is drawn twice (panel and list) and each data file once; `Project::cases` lists all 5 | the route before (`data_files_as_cases`): the header is `Cases (5)`, no `Data files`, the csv drawn twice |
| N7 | A save of the shown text (ADR 0138's project, watcher on at 500 ms): (a) the cavity: `mesh.nx` typed "20", built (320 cells), committed, settled; saved; 2.0 s of frames after the save was applied; (b) the file replaced with other bytes (`nx = 24`), up to 5 s; (c) the hex box: `physics.nu` typed "0.02", built, committed; the mesh file's modification time moved 10 s ahead; saved; 2.0 s, then up to 5 s to settle | (a) `meshes_built` and `installs` unchanged and the same surface (`Arc`) since before the save; `skipped_saved` grew; the shown mesh was the editor's before and is the file's after (`from_editor`); not stale, `Ready`; the source line `(built from the case file on disk; the solver is not run)` drawn. (b) 384 cells shown. (c) `meshes_built` grew | ADR 0147's rule (`no_save_skip`): after (a)'s save `meshes_built` grew and `skipped_saved` is 0 |
| N8 | The Start run dialog (ADR 0138's project, run worker on, no runner): the cavity open; (a) `mesh.nx` typed "20": the dialog of the cavity until its plan and Start are drawn; the hover over Start (frames until drawn, at most 30); a click on Start, 1 s; (b) the field committed; (c) saved; (d) `mesh.nx` set to 24 (unsaved), the dialog on `cases/hex.toml` | (a) Start's refusal starts `cases/cavity.toml: finish editing the field mesh.nx` and equals Run's (`blocked`); the refusal is drawn on hover; the click queued no run (no `queued run` message, no run folder); (b) exactly `cases/cavity.toml has unsaved edits: save it first (Case > Save); a run uses the file on disk`, equal to Run's; (c) none; (d) none | the route before (`start_ignores_editor`): (a)'s Start is enabled |
| N9 | Every time says what it is (a copy of `example-projects/wind-tunnel`, watcher off, run worker on): the run of `cases/cylinder-re100.toml` (transient: 80 snapshots at 1-80 s, 1280 steps) at its newest and its first snapshot, then the run of `cases/step-re400.toml` (steady: 11 snapshots, 429 iterations), each until shown with its residuals plotted; the hover over the slider's label; and the pure functions | transient: the slider's label (drawn above the image) exactly `simulated time (s)`; the snapshot lines exactly `snapshot 80/80  time step 1280  simulated time 80 s` and `snapshot 1/80  time step 16  simulated time 1 s`; the strip holds `  time step 1280  simulated time 80 s  `; the Runs panel's header holds `time (s)`; the run's row's 4th field `8.0000e1`; the plots' x-axis label (below the image) `time step`. Steady: the slider's label `iteration`; the snapshot line exactly `snapshot 11/11  iteration 429`; the strip holds `  iteration 429  ` and no `time`; the row's 4th field `-`; the x-axis label `iteration`. Both hover texts drawn. No drawn text holds ` t = ` or `  t `. Pure: `run_kind` of times (0, 0) steady, (0.5) transient; `run_kind_of_time(NaN)` steady; `fmt_num` 80 `80`, 0.0625 `0.0625`, 1.6666666666666667e-7 `1.667e-7` | the words before (`bare_time`): the snapshot line `snapshot 80/80  step 1280  t = 80`, a bare `t` drawn, the steady slider label `step` |
| N10 | A scale and the domain: Mesh side (ADR 0138's project): the cavity, its scale bar, then 3 wheel notches in; the hex box. Run side (a copy of the wind tunnel): the runs of `cylinder-re100` (an O-grid to r 15 m, 64 x 48 cells) and `step-re400` (15 x 1 m, 200 x 40) | drawn lines holding `domain `: exactly `domain 1 x 1 m; 16 x 16 = 256 cells`; `domain 2 x 1 x 1 m; 64 cells; the resolution is set by the mesh file`; `case cases/cylinder-re100.toml, domain 30 x 30 m, 3,072 cells`; `case cases/step-re400.toml, domain 15 x 1 m, 8,000 cells`. Each 2D view's bar: 1, 2 or 5 times a power of ten metres; its length in points equals metres over (2 half height / image height) within 1e-3; between 0.1 and 0.25 of the image width; its label drawn inside the image; zoomed in, its length in metres not larger. The hex box (3D): no bar | before (`no_scale`): no drawn line holds `domain `, no bar |
| N11 | Refine x2 (ADR 0138's project with `cases/ring.toml`, an `annulus_3d` of 12 x 2 x 3 cells; watcher and run worker off): (a) the cavity: the hover over Refine x2, a drawn click, settled; (b) the ring: a click, settled; (c) the hex box; (d) the preview's cell limit set to 1,000, the cavity opened again: the hover, a click, settled; (e) the limit back, the cavity opened again, `mesh.ly` typed "2", settled: the hover, a click | (a) the hover drawn holds `mesh.nx 16 -> 32, mesh.ny 16 -> 32 (256 -> 1,024 cells`; the message exactly `cases/cavity.toml: refined x2 (mesh.nx 16 -> 32, mesh.ny 16 -> 32): 256 -> 1,024 cells, unsaved (save the case before Run)`; the editor's `mesh.nx`, `mesh.ny` 32, 32, unsaved; the preview from the editor with 1,024 cells, its mesh hash that of the cavity at 32 x 32; Run's refusal starts `cases/cavity.toml has unsaved edits: save it first`; the line drawn exactly `domain 1 x 1 m; 32 x 32 = 1,024 cells`. (b) 72 cells, then 576. (c) no Refine x2 drawn. (d) the hover holds `Over the preview limit of 1,000 cells`; after the click the preview is over its limit with "Build anyway" (`TooLarge`, `rebuild`). (e) the greyed button's hover holds `cases/cavity.toml: finish editing the field mesh.ly first`; after the click `mesh.nx`, `mesh.ny` are 16, 16 | before (`no_scale`): no Refine x2 drawn, so no hover and no message |
| N12 | The rest: the whole `rcfd-studio` suite (with the changed earlier tests below), `workspace_graph`, `cargo clippy -p rcfd-studio --release --all-targets -- -D warnings`, `cargo fmt -p rcfd-studio -- --check`, `scripts/third_party_licenses.py --check` | pass; the four known misses ignored as before (ADR 0131 B9, ADR 0133 R6, ADR 0141 G1, ADR 0144 J3b) | (their own) |

How the registered runs are made: `cargo test --release -p rcfd-studio --test selection_n -- --test-threads 1
--nocapture`, the same for `followups_n` and `scale_n`, each output to a file; then the whole `rcfd-studio` suite once
with `--no-fail-fast -- --test-threads 1 --nocapture` (`fake-claude` built first), output to a file (N12); clippy, fmt,
`workspace_graph` and the licence check on the final commit.

## Changes to earlier tests (registered here, none run yet)

* ADR 0141 G1 and G1b (`oneclick_e2e_g4.rs`, the shared `g1`): limit (f)'s text `step N` of the strip and of the drawn
  strip becomes `iteration N` (its cavity is steady; decision 6). The limits are otherwise unchanged; G1 stays ignored
  (its known miss).
* ADR 0138's unregistered `rebuilds_after_a_save_and_an_external_change` (`preview_g1.rs`): its comment and one
  assertion message now say that after a save the mesh shown is the file's (decision 4: not built again); its
  assertions are unchanged and hold either way.

## What I already knew (rule d)

* The code reading above.
* **Probes before any change** (a throwaway `crates/rcfd-studio/tests/zz_diag_n.rs`, run twice in release, deleted, never
  committed):
  * Round 1. Copies of `example-projects/wind-tunnel` (6 cases) and `quick-start` (4), the Studio with the watcher (500
    ms) and the run worker on: each of the 10 cases opened in the case editor and its preview settled 0.007-0.054 s
    after the click (0.017, 0.018, 0.044, 0.033, 0.054, 0.043; 0.012, 0.007, 0.012, 0.012 s); just after a click
    (before its `Loaded`) `selected_case`, the editor and `blocked` named the case before while `Preview::case` named the
    new one; three cases clicked in a row without drains ended on the last; drawn clicks on 4 cases the same. The Run page
    drawn: the cylinder (transient) slider value `80.0` and label `time`, the line `snapshot 80/80  step 1280  t = 80`;
    steady runs `2000` and `step`, `snapshot 10/10  step 2000  t = 0`; the Runs rows `... finished 1280 8.0000e1 ...`
    wrapping in the left panel. In that dump the case editor held `cases/naca0012-aoa4.toml` (the last case clicked)
    while the Run side showed the cylinder runs selected by the probe.
  * Round 2. P1: a served project with three small cavities, drawn app: Run on `a` queued a run of `a`; `b` clicked then
    Run clicked (the click helper's first frame applied `b`'s read) queued `b`; `a` selected, Run clicked, and `c`
    clicked before the run was queued: the run was `a`'s, and when it was queued the Viewport switched from `c`'s mesh to
    `a`'s run (R4); the toolbar's label then `c`; Options... opened with `c` (the dialog keeps the case it was opened
    with; its window then covered the list in that set-up). P3: the cavity open, a listed case removed from disk,
    selected, then `b` selected before a drain: 1.5 s later the editor and Run's case were the cavity, nothing being
    read, the Mesh side `b` (R3); a benchmark descriptor clicked: the editor and Run stayed on the cavity, the Mesh side
    on the descriptor (R2). P4: at 1280 x 800 (scale 1, 1.25, 1.5), 1024 x 768 and 1920 x 1080 (1, 1.5), the cylinder's
    first snapshot: slider `1.0` and `time`, `snapshot 1/80  step 16  t = 1`, the strip `... step 1280  t 8.0000e1 ...`;
    no text `1t`.
* egui_plot 0.37.0's default hover label (`label.rs`: `x = {:.3}` / `y = {:.3}`) and axis formatter (numbers), read.
* No baseline suite run of this job before the change. The last whole-suite run on this code is ADR 0147's addendum (bf33373,
  merged as main 7eb8b79): 189 passed, 0 failed, 4 ignored.
* The code and the tests of this ADR were compiled (`cargo test --release -p rcfd-studio --no-run`), `cargo clippy -p
  rcfd-studio --release --all-targets -- -D warnings` is clean, `cargo fmt -p rcfd-studio` applied. No registered check
  of this ADR, and none of the changed earlier tests, has run.

## Not in CI (by hand)

A person at the real window: the case clicked and Run, the middle drag, the scale bar and the words on the Run page, the
Refine x2 button and its hover, the "Data files" list, the Start dialog's greyed Start.

## View-and-report checklist for the maintainer

1. Build and start: `powershell -ExecutionPolicy Bypass -File scripts\studio.ps1 example-projects\wind-tunnel` (or a copy
   of it).
2. Click a case, then another, then a third: the highlight, the name next to Run, the case editor and the Mesh side follow
   each click; Run (F5) runs the case highlighted. Copy a `cases/<id>/case.json` of the repository into the project's
   `cases/` and click it: the case editor says it cannot open it and why; the Mesh side says "not a case file"; Run says
   it was not started.
3. Select a run in the Runs panel: the Run side shows it, and beside its caption the case it is of, its domain and its
   cells. Then Run: it runs the case highlighted in the Project panel, not the run's case (question 1).
4. Middle drag over the mesh: it rotates; right drag pans; left drag rotates as before.
5. On the Run side of `cylinder-re100`: the slider says `simulated time (s)`, the line under it `snapshot ... time step
   ... simulated time ... s`; hover them. On a steady run (`step-re400`): `iteration`. The plots' x axis says the same.
6. A 2D case (or run): a scale bar in the bottom-left corner of the picture; zoom: it follows. The line under "Mesh of
   ..." gives the domain and the cells; hover "Refine x2", click it: the counts double in the case editor (unsaved), the
   mesh follows after the pause, Run says "save it first".
7. The Project panel of a project with `cases/profile_inlet.csv`: it is under "Data files".
8. Type into a field, open Run > Options...: Start is greyed out and says why, as Run does; save: it is enabled.
9. Report anything slow, confusing or wrong, with the case and what was done.

## Questions for the maintainer (not decided here)

1. **The Run side and the case** (R5). The Run side shows the run selected in the Runs panel, whatever case is selected;
   selecting a run does not select its case. Options: (a) keep, with the run's case named beside the Run side's caption
   (done here); (b) selecting a run also selects its case (the case editor, the Mesh side and Run follow it; a case with
   unsaved edits asks first); (c) selecting a case also selects its newest run for the Run side (none: the Run side says
   that the case has no run yet); (d) both (b) and (c). Recommendation: (d): one selection, the case, with its newest run
   on the Run side, and a run click selecting both.
2. **Unsaved edits on a click.** A click on another case (or on the same one) drops the case editor's unsaved edits
   without asking (before this job: when the new case arrived; now at the click, also when the new case cannot be
   opened). The case starter already refuses to open a new case over unsaved edits. Options: (a) keep; (b) ask (save,
   discard, cancel) before switching away from unsaved edits; (c) refuse the switch with a message, as the starter does.
   Recommendation: (b).
3. **The step axis of a transient run with one snapshot** reads "time step" and its only snapshot at time 0 reads as
   steady (decision 6 tells the kind from the stored times). Options: (a) keep; (b) read the kind from the run's case
   (`solver.algorithm`, `solver.dt`) on the viewport's worker. Recommendation: (a) until it is met.
4. **Refine x2's time estimate.** The hover says "each iteration does about that much more work" and gives the runner's
   memory estimate; a transient run on a finer mesh may also need a smaller `solver.dt` (CFL), which is not said in
   numbers. Options: (a) keep; (b) add the CFL estimate of the refined mesh from the case's velocity scale. Recommendation:
   (a).
5. **Refine for `axial_fan_sector`** (counts under `mesh.fan`). Options: (a) not offered (now); (b) double its `n_*`
   counts too. Recommendation: (a): its mesh is fragile against count changes (`docs/case-schema.md`).

## Files

This ADR; hunks in `crates/rcfd-studio/src/{studio,watch,quick_run,case_starter,view_toggle,project_view,ui,preview,
preview_ui,viewport,viewport_ui}.rs`; new tests `crates/rcfd-studio/tests/{selection_n,followups_n,scale_n}.rs`; changed
tests `crates/rcfd-studio/tests/{oneclick_e2e_g4,preview_g1}.rs`; dated notes in ADRs 0132, 0138 and 0147.
