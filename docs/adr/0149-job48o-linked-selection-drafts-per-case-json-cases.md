# ADR 0149: Job 48O, the case and its runs are one selection, unsaved edits are kept per case, Run reads a JSON case as `rcfd run` does, and the case starter says "simulated time"

Status: **registered 2026-10-28**, before any registered run (the code and the tests below were written and compiled
first; what had run before is listed under "What I already knew"); amendment 1 (the case editor's fields per case, O9b)
before any registered run; amendment 2 (the set-ups of O1 and O8's control) and amendment 3 (O1's control) before their
corrected runs; not yet accepted by the maintainer. Job 48O, on local
main 9e857d8 (waves 48A-48N merged). House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits
are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a
defective set-up gets an amendment written before the corrected run; defaults of existing code stay bit-identical outside
the Studio; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new crate, no unsafe code, the crate graph
stays acyclic. Clean-room: our own Studio code and the egui public API.

## The maintainer's decisions (2026-10-28, answers to the questions of [ADR 0148](0148-job48n-selected-case-middle-drag-units-scale-refine.md), relayed)

1. **Question 1 (R5): link both ways.** A click on a run in the Runs panel selects its case (`Studio::selected_case`,
   found from the run's `run.json`; when the case file no longer exists, nothing new is selected and the run's caption
   says so) and switches to the Run side; a click on a case shows that case's newest run on the Run side (newest by
   `created`, ADR 0147's order) when it has one, else the Run side stays disabled with its reason. ADR 0144's toggle
   stays: opening a case switches to Mesh; how a case click interacts is mine to decide and register, the tabs, the Run
   button and the Cases list always agreeing on the case (suggested: Mesh as today, the Run side showing the case's
   newest run when toggled). The state machine registered and every transition tested, a run whose case was deleted or
   renamed, two runs of one case and runs started while another case is selected included; ADR 0148's R4 stays.
2. **Question 2: drafts per case.** Switching cases keeps each case's unsaved editor state (unsaved edits, the field
   being typed, a conflict) in memory until it is saved, or closed or discarded explicitly; returning to the case
   restores it; the Cases list marks the cases with unsaved state; Run and Start stay blocked per case ("save it first");
   the preview of a case with a draft shows the draft (ADR 0144) when it is selected; closing the project with drafts
   asks save, discard or cancel (or keeps the close as it is if it asks already: checked and reported); a file changed on
   disk while its draft is kept: ADR 0131's conflict logic on return, with a test; the memory bounded; ADR 0148's "the
   edits are dropped at the click" removed.
3. **Question 6: a JSON case as `rcfd run` reads it**, reusing the function the command line uses, no second parser;
   the start plan, Run and the Start run dialog accept `.json` cases; a benchmark descriptor (`cases/<id>/case.json`,
   not a case file) gets ADR 0147's one line in Run's refusal and hover text and in the Start dialog and is not
   startable. Registered: a JSON case written from a TOML case by the existing writer runs from the Studio model and its
   run snapshot matches; each descriptor under the repository's `cases/` is refused with the one line.
4. Questions 3, 4 and 5 of ADR 0148: its recommendations (a): the step axis kept, no time estimate for "Refine x2", no
   "Refine x2" for `axial_fan_sector`. Nothing changes.
5. The stray bare `to t = X` of the case starter's dialog says "simulated time"; ADR 0139's CS7 reads that line, so CS7
   is amended first ([ADR 0139 amendment 3](0139-wave48g2-case-starter.md), written with this registration), then the
   text changes.

## Context (read at 9e857d8)

* The selection (ADR 0148): `Studio::selected_case` (the click in the Project panel, a case the starter opened) and
  `Studio::selected_run` (the plots, the log, the strip, the Runs panel's highlight, and on the Run side the Viewport)
  are separate. `Studio::open_case` does not touch the run; the Runs panel's click calls `Studio::select_run`, which
  switches to the Run side and does not touch the case (ADR 0144). A run that Run queued after another case was
  selected is selected without switching the Viewport (`select_run_keep_view`, ADR 0148 decision 1 (d)).
* The editor (ADR 0148 decision 1 (a)): `open_case` drops the case editor at once, its unsaved edits with it. A field's
  text lives in `UiState::buffers`, keyed by the settings path for every case alike, and is committed when the field
  loses the focus (`ui.rs` `leaf_ui`); while typed it is the editor's `draft` (ADR 0144).
* Closing (checked, as the maintainer asked): nothing asks. Project > Close project calls `Studio::close_project`
  (documented "unsaved edits are dropped"); Project > Open project..., Open typed path..., Recent and New project... call
  `open_project` / `create_project`, whose `start` calls `close_project` first; the window's close saves the
  preferences and stops a Claude turn only (`StudioApp::ui`).
* A failed save comes back as `watch::Event::Failed(message)` without the file it was for.
* `rcfd run` reads a case text in `load_case_in` (`crates/rcfd-cli/src/main.rs`, private to that program; the runner
  starts `rcfd run <run>/case.snapshot --case-dir <folder>`): JSON when the text starts with `{`, else TOML, through
  `Case::parse_json_with` and `Case::parse_toml_with` of `rcfd-api`. `rcfd-mcp` (`project.rs` line 1062) and the
  Studio's `preview::parse_case` (ADR 0144, "Parse case text as `rcfd run <file> --case-dir <dir>` does") use the same
  rule with the same two functions. The start plan (`runs::case_facts`, used by Run, the Start dialog and ADR 0140's
  re-run) reads TOML only (`toml::from_str`, `Case::parse_toml_in`), as does the dialog's cell count
  (`store_plan::count_cells`), so a `.json` case is refused with `not TOML: ...`, a benchmark descriptor too (ADR 0148
  deviation 4).
* The case starter (`case_starter::Example::describe`) gives a transient example's run length as `<algorithm>, to t = T`.
* The in-process service (`rcfd_api::service`) has no limit on its sessions; each case editor holds one.

## Decisions (as implemented)

1. **One selection: the case and its runs** (`Studio::open_case`, `Studio::open_run` (new), `newest_run_of`,
   `run_case`, `case_listed`, `view_refusal`, `studio::no_run_of`, `view_toggle::gone_case_text`; `quick_run::apply`).
   * A click on a case (`open_case`, also a case the starter opens) makes it the selected case as ADR 0148 does, and
     makes its newest run (by `created`, then id, among the runs listed and those the run worker knows; the run Run
     queued last counts as the newest of its case until it is listed) the selected run, or none; the Viewport goes to
     the Mesh side (ADR 0144). The plots, the log and the strip follow the new selected run (none: they empty).
   * A click on a run's row in the Runs panel (`open_run`) selects its case (its `run.json`'s `case`) as a case click
     does, without the Mesh side and without choosing its newest run: the run clicked is the selected run and the Viewport
     goes to the Run side; the Mesh side shows the case's mesh when toggled. The same case: nothing is read again.
   * A run whose case is not among the project's case files as last listed (removed or renamed; ADR 0148's `.toml` and
     `.json` rule): the selected case does not change, the status bar says `run <id>: its case <case> is not in the
     project any more (deleted or renamed), so the selected case is unchanged`, and the Run side's readout names it as
     `case <case> (not in the project any more: deleted or renamed; the selected case is unchanged)`. The listing
     follows the watcher or Project > Refresh: with the watcher off, a case removed after the last listing still counts
     as listed until a refresh.
   * The toggle: unchanged (ADR 0144); the Run side's refusal names the selected case when it has no run: `<case> has
     no run yet: press Run (F5) to start one, or select a run in the Runs panel.` (`NO_RUN` with no case selected).
   * Run of the selected case (ADR 0141): its run is selected and shown, as before (it is the case's newest run).
   * **A run that Run queued after another case was selected is not selected** (changes ADR 0148 decision 1 (d), which
     selected it without switching the Viewport): selecting it would make the Run side, the strip and the plots name
     another case than the Project panel and Run. ADR 0148's R4 fix stays: the Viewport stays on the selected case's
     mesh and no panel comes to the front. The run is listed in the Runs panel and named in the status bar (`queued run
     <id> of <case> ...`); a click on its case or on its row selects it (question 1).
   * `Studio::select_run` is unchanged (it is Run's auto-select, and the call many tests make): it selects a run and
     goes to the Run side without changing the case.
   * Test switch `QuickRun::switches.case_run_unlinked`: ADR 0148's route (a case click keeps the run, a run click keeps
     the case, Run's run queued after another case was selected is selected without the switch).
2. **Unsaved state kept per case** (`Studio::parked`, `max_parked`, `MAX_PARKED` = 32, `has_unsaved`, `select_case`,
   `leave_case`, `take_back`, `case_editor`, `unsaved_cases`, `discard_edits`, `request_close`, `answer_close`,
   `CloseAsk`, `CloseAnswer`, `AfterClose`; `CaseEditor::returned`; `watch::Event::SaveFailed`; `ui.rs`).
   * The unsaved state of a case editor: an edit not saved, a field being typed, a conflict, or a save in flight. When
     another case is selected (a click on a case, a click on a run of another case, the starter opening a new case), an
     editor with unsaved state is kept (`Studio::parked`) and no longer watched; a clean one is dropped as before. Back
     on the case (any of those selections), the kept editor is the case editor again at once (the same edits, typed text,
     conflict; the same case clicked again keeps it too), and the file is read again: its bytes are compared by ADR
     0131's rule with the field being typed counted as an edit (`CaseEditor::returned`): the same bytes change nothing;
     other bytes, or no file, while the editor holds an edit, a typed field or a conflict put it in ADR 0131's conflict
     state with everything kept (the disk version offered; "Keep my edits" or "Take the disk version"); a clean editor
     reloads. A typed field counts as an edit here so that it is never dropped by the reload of a clean editor (question
     5).
   * A field typed in a case left and taken back shows the typed text (the editor's `draft`) until it is finished; the
     field buffers are shared by all cases (`UiState::buffers`), so without this the field would show the stored value
     while Run says "finish editing" (`ui.rs` `leaf_ui`, `Field`).
   * Bound: at most `Studio::max_parked` (`MAX_PARKED` = 32) editors kept; a selection that would keep one more is
     refused, nothing changes, and the status bar says `<case> was not opened: <n> other cases hold unsaved edits, the
     most the Studio keeps; save or discard the edits of one of them first (marked * in the Project panel)`. Taking a kept
     case back is never refused (one leaves the store as one enters). The starter does not open its new case over a field
     being typed when the store is full (as over unsaved edits, ADR 0139 D5).
   * The Project panel draws `*` after each case whose editor holds unsaved state (the open one and the kept ones), a
     separate text after the label, with the hover text `unsaved edits: kept while other cases are selected, until you
     save them (Case > Save) or discard them (Discard edits in the case editor)`.
   * Run, F5 and the Start run dialog: `quick_run::editor_blocks` looks at the editor of the case wherever it is (open or
     kept), so a kept case is refused by the dialog with Run's text; Run of the selected case as before.
   * The preview of the selected case: its editor is the open one again at once, so ADR 0144's live preview builds the
     kept text (no change in the preview).
   * "Discard edits" in the case editor (enabled with unsaved state and no save in flight): the edits, the field being
     typed and the conflict are dropped and the file is read again (a clean editor); the preview builds the file. With
     closing, it is the explicit way to discard.
   * A save's result reaches the editor of its case wherever it is: `Saved`, `SaveConflict` and the new `SaveFailed {
     rel, message }` (a failed write; before: `Failed(message)`, the status bar text unchanged); a kept editor that a save
     left clean is let go (its mark goes).
   * **Closing asks** (it did not ask before; Context): Project > Close project, Project > Open project..., Open typed
     path..., Recent, New project... and the window's close call `Studio::request_close(<what follows>)`; while any case
     holds unsaved state the question `Unsaved edits` (an egui `Modal`) names the cases and offers `Save all and close`,
     `Discard and close`, `Cancel` (Escape: Cancel); the window's close is cancelled (`ViewportCommand::CancelClose`)
     while it asks. Save all: in every case with unsaved state a field being typed is committed first (a refused commit
     stops the close with `<case>: <path>: <reason>: correct the field, or close with Discard`); a case in conflict stops
     it (`<case>: resolve the conflict in the case editor first (Keep my edits or Take the disk version), or close with
     Discard`); the others are saved with the base-hash rule, and when every save is written the project closes and the
     action asked for follows (the other project opens, the window closes); a save that meets a conflict or fails stops it
     with `<case>: not saved, the file changed on disk; resolve the conflict` (or `<case>: not saved: <message>`), the
     question staying with the reason. Discard: the project closes, the edits are dropped, the action follows. Cancel:
     nothing changes. `Studio::close_project` itself (tests, the inner path) does not ask.
   * Test switch `QuickRun::switches.drop_drafts`: ADR 0148's route (the editor of the case left dropped at the click,
     closing without a question).
3. **A JSON case as `rcfd run` reads it** (`preview::is_json_text`, `preview::parse_case`; `runs::case_facts_with`,
   `StartPlan::new_with`, `store_plan::case_extra`, `count_cells`; `quick_run::blocked`, `Studio::not_a_case`).
   * The function: `rcfd run`'s reader is private to the `rcfd` program; the Studio's `preview::parse_case` is the same
     rule with the same two `rcfd-api` functions (`Case::parse_json_with` for a text that starts with `{`, else
     `Case::parse_toml_with`), so the start plan and the cell count now call it; no parser is added (question 6). The
     keys a text sets (ADR 0140's store keys, the `output` keys) are read from the same text as a tree, JSON or TOML by
     the same rule (`is_json_text`).
   * A benchmark descriptor: when the plan's read fails and the text is a descriptor, the plan's error is ADR 0147's one
     line (`preview::descriptor_message`) as it is. So: the Start run dialog shows that line and offers no Start; Run
     before the descriptor was read refuses with `<case> was not started: <line>`; once read (the editor's `Loaded`), the
     selected descriptor greys Run out with the line as its hover text, and F5 or a click says `Run: <line>`.
   * Test switches `QuickRun::switches.cases_as_toml` (Run) and `poll::Switches::cases_as_toml` (the Start dialog's
     plan): the route before (TOML only, no descriptor line, Run not greyed out for a descriptor).
4. **The starter's run length** (`case_starter::Example::describe`, `describe_with`): `<algorithm>, to simulated time T
   s` (ADR 0148 decision 6's words; T as Rust prints the case's `solver.end_time`). `describe_with(.., true)` gives the
   words before (`to t = T`) for the control.

## The state machine (registered)

S is the selected case, R the selected run, V the Viewport's side. "Listed" is ADR 0148's case-file rule on the last
listing.

| From | Event | To |
|---|---|---|
| any | a click on case C | S = C; R = C's newest run, or none; V = Mesh; the editor: C's kept editor at once, else C read; the case left kept with its unsaved state (refused at the bound: nothing changes) |
| any | a click on run X whose case K is listed | S = K (as above, the Mesh side previews K; the same case: not read again); R = X; V = Run |
| any | a click on run X whose case K is not listed | S unchanged; R = X; V = Run; the status bar and the Run side's readout name K as not in the project any more |
| any | the toggle | V = the side asked for when it has something to show; else refused, the Run side's reason naming S when S has no run |
| S = C | Run of C queued while S = C | R = the new run; V = Run (ADR 0141) |
| S = C | Run of C queued after S became D | R, V unchanged (D's selection); the run listed and named in the status bar |
| any | the Start run dialog's Start (any case) | unchanged (ADR 0141 question 3: no auto-select) |
| any | the starter opens a new case N | S = N; R = N's newest run (none for a new file); V = Mesh |
| any | the project closed (after the question) | S, R none; V = Mesh; nothing kept |

## Placement

`crates/rcfd-studio` and `docs/`. Hunks in `src/{studio,case_edit,quick_run,view_toggle,viewport_ui,case_starter,runs,
store_plan,preview,poll,watch,ui}.rs`. No new crate, no new dependency, no new workspace edge; `Cargo.toml`, `Cargo.lock`,
`crates/rcfd-cli/tests/workspace_graph.rs` and `docs/architecture.md` unchanged. Nothing outside the Studio changes
(`rcfd-api`, `rcfd-cli`, `rcfd-mcp` are read, not edited).

## Registered checks (limits fixed here, before any of them ran)

Release builds, `--test-threads 1 --nocapture`, output to files, this host; `rcfd-runner` and `rcfd` of the same build
next to the test's `rcfd-studio` (`common::binaries`). "Settled": nothing being read (`pending_case` none) and the preview
neither building nor waiting for its debounce (`lj::settled`). Drawn frames are 1280 x 800 at scale 1, the default
layout; hover checks with egui's tooltip delay 0 (ADR 0148 amendment 1). Every wait has a 30 s limit (60 s for a run to
finish), an upper bound, not a timing claim. "Agree" (O1, O3): `quick_run::selected_case`, the Mesh side's case and the
case editor's case are S, and R's case is S unless R's case is not listed (then the Run side's readout starts with
`gone_case_text(<case>)`), and on the Run side the Viewport shows R. The fixture of O1 and O2: ADR 0138's project
(`cases/cavity.toml` 16 x 16, `cases/hex.toml`) with `cases/b.toml` (the cavity at 10 x 10), `cases/gone.toml` and
`cases/old.toml`, and five finished runs without stores made by `Project::create_run` 10 ms apart: r1, r2 of the cavity,
r3 of b, r4 of gone, r5 of old; watcher off, run worker on. The fixture of O4-O9: ADR 0138's project with `cases/b.toml`
(10 x 10) and `cases/c.toml` (12 x 12), watcher and run worker off unless said. Files: `crates/rcfd-studio/tests/
linked_o.rs` (O1-O3), `drafts_o.rs` (O4-O9), `json_o.rs` (O10, O11); O12 in `case_starter_g2.rs`.

| Id | Check | Limit | Negative control (must fail) |
|---|---|---|---|
| O1 | The state machine (model): (a) nothing selected; (b) hex clicked; (c) the cavity clicked, then the toggle to Run; (d) r1 clicked in the Runs panel (`open_run`); (e) the toggle to Mesh; (f) r3 clicked; (g) the toggle to Mesh; (h) the cavity clicked; (i) `cases/gone.toml` removed and `cases/old.toml` renamed to `cases/new.toml`, Project > Refresh until listed so, r4 clicked; (j) r5 clicked; (k) `cases/new.toml` clicked; (l) the project closed. Each step settled | every step agrees; (S, R, V, the Mesh side's case): (a) (none, none, Mesh), the Run side's refusal `NO_RUN`; (b) (hex, none, Mesh, hex), the Run side's refusal exactly `no_run_of("cases/hex.toml")`; (c) (cavity, r2, Mesh, cavity), no run in the Viewport; after the toggle (cavity, r2, Run, cavity); (d) (cavity, r1, Run, cavity), nothing read again at the click, the editor the cavity; (e) (cavity, r1, Mesh, cavity); (f) (b, r3, Run, b), the editor b; (g) (b, r3, Mesh, b); (h) (cavity, r2, Mesh, cavity); (i) (cavity, r4, Run, cavity), the status bar exactly `run <r4>: its case cases/gone.toml is not in the project any more (deleted or renamed), so the selected case is unchanged`; (j) (cavity, r5, Run, cavity); (k) (new.toml, none, Mesh, new.toml), the refusal `no_run_of("cases/new.toml")`; (l) (none, none, Mesh), `NO_RUN` | ADR 0148's route (`case_run_unlinked`): after (c) R is none; after (f) S is the cavity while the Run side shows r3, and the parts disagree |
| O2 | The same drawn (the fixture without r4 and r5, tooltips at once): (a) a drawn click on the cavity's label; (b) a drawn click on the toggle's Run; (c) a drawn click on r3's row in the Runs panel; (d) a drawn click on hex's label, then the pointer on the toggle's Run (at most 30 frames). After each, settled, a drawn frame | the Project panel's highlight and the case right of Run are exactly: (a) the cavity, Mesh, the toggle's Run enabled; (b) the cavity, Run, `stored fields of run <r2>` drawn; (c) b, Run, `stored fields of run <r3>` drawn and a drawn text starting `case cases/b.toml`; (d) hex, Mesh, the toggle's Run disabled, `no_run_of("cases/hex.toml")` drawn on hover | ADR 0148's route: after (c) the highlight and the label are the cavity while `stored fields of run <r3>` is drawn |
| O3 | Runs started (a served project with `cases/a.toml` and `cases/c.toml`, ADR 0148 N1's cavities at `nx` 8 and 14, 30 iterations; the Studio model with the watcher and the run worker on): (a) a selected, Run (F5's call): A1; (b) Run again: A2; (c) a clicked; (d) A1 clicked in the Runs panel; (e) Run, and c clicked at once (before the run is queued): A3; (f) until A3 finished (60 s) and listed; (g) a clicked | every step agrees; (S, R, V): (a) (a, A1, Run); (b) (a, A2, Run); (c) (a, A2, Mesh); (d) (a, A1, Run); (e) (c, none, Mesh), the Mesh side c, a status-bar message starting `queued run <A3> of cases/a.toml`; (f) A3 finished; (c, none, Mesh); (g) (a, A3, Mesh) | ADR 0148's rule: after (e) R is A3 while S is c |
| O4 | Kept and taken back (model): (a) the cavity opened, `mesh.nx` 20 committed; (b) b selected; (c) `mesh.ny` of b typed "12" (`type_text`), hex selected; (d) hex's `physics.nu` 0.02 committed, `cases/hex.toml` replaced on disk (`max_iterations = 6`), saved (a save conflict), the cavity selected; (e) the cavity's editor at the click and settled; (f) b selected; (g) hex selected; (h) hex clicked again | (b) kept exactly the cavity; the editor holds nothing before b is read; `editor_blocks(cavity)` exactly `cases/cavity.toml has unsaved edits: save it first (Case > Save); a run uses the file on disk`; Run of b not refused; (d) the cases with unsaved state exactly b, cavity, hex; (e) at the click and settled: (cavity, dirty, no typed field, `mesh.nx` 20); Run's refusal the cavity's "save it first"; the preview 320 cells from the editor; b's refusal starts `cases/b.toml: finish editing the field mesh.ny`, hex's exactly `cases/hex.toml: resolve the conflict in the case editor first`; (f) (b, clean, the typed field `("mesh.ny", "12")`, `mesh.nx` 10), Run's refusal starts `cases/b.toml: finish editing the field mesh.ny`, the preview 120 cells (10 x 12) from the editor; (g) hex in conflict with the disk's bytes, `physics.nu` 0.02; (h) at the click still in conflict | ADR 0148's route (`drop_drafts`): (b) nothing kept; (e) settled: `mesh.nx` 16, clean |
| O5 | The file changed while kept (model): the cavity (`mesh.nx` 20 committed), b (`mesh.ny` typed "12") and c (`mesh.nx` 14 committed) left in turn, hex selected; then `cases/cavity.toml` replaced by the cavity at 24 x 16, `cases/b.toml` by 10 x 14, `cases/c.toml` removed; each selected again until read | the cavity: in conflict with the new file's bytes offered, `mesh.nx` 20 kept, the status bar exactly `cases/cavity.toml: changed on disk while its edits were kept; resolve the conflict`; b: in conflict with its new bytes, the typed field `("mesh.ny", "12")` kept, `mesh.ny` 10 in the settings; c: in conflict without a file, `mesh.nx` 14 kept | ADR 0148's route: the cavity clean with `mesh.nx` 24 (the new file read, the edit gone). Reported, not a control: ADR 0131's rule as it is (`external_change`) reloads b and drops the typed field |
| O6 | Drawn (run worker on): (a) a drawn click on the cavity, `mesh.nx` 20 committed; (b) a drawn click on b: the marks, then the pointer on the cavity's mark (30 frames); (c) the Start run dialog of the cavity (b selected): its Start; (d) a drawn click on the cavity: the marks and the Mesh side's source line; (e) a drawn click on "Discard edits" | (b) the case labels drawn with `*` right of them on their row are exactly the cavity; the hover text drawn; (c) the dialog's Start greyed with exactly the cavity's "save it first" text; Run (of b) not refused; (d) exactly the cavity marked; `(built from the unsaved edits in the case editor; the solver is not run)` drawn; (e) no mark; the editor clean with `mesh.nx` 16; `(built from the case file on disk; the solver is not run)` drawn; 256 cells | ADR 0148's route: (b) no mark; (c) Start enabled |
| O7 | Closing (model, then drawn): (a) the cavity's edit kept and b's field typed, Project > Close project (`request_close(Nothing)`); (b) Cancel; (c) another project asked for (`Open`), Discard; (d) the cavity's edit and b's typed field kept and hex's `physics.nu` 0.02 committed: close, Save all; (e) c's `mesh.nx` typed "1e" and kept: close, Save all; (f) hex in conflict (as O4 (d)): close, Save all; (g) the cavity (`mesh.nx` 22) kept, b's `mesh.nx` 11 committed, `cases/cavity.toml` replaced on disk (watcher off): close, Save all; (h) drawn: the cavity's edit, a frame carrying the window's close request; the question drawn; a drawn click on `Discard and close`; another close request | (a) asked (not closed): the cases exactly b, cavity; the project open; (b) open, both cases still unsaved; (c) the other project open, nothing kept, the cavity's file unchanged (16); (d) closed; the files hold `mesh.nx` 20, `mesh.ny` 12 and `physics.nu` 0.02; (e) the error exactly `cases/c.toml: mesh.nx: '1e' is not a whole number: correct the field, or close with Discard`, open, c's file unchanged; (f) exactly `cases/hex.toml: resolve the conflict in the case editor first (Keep my edits or Take the disk version), or close with Discard`, open; (g) when both saves ended: exactly `cases/cavity.toml: not saved, the file changed on disk; resolve the conflict`, open, the cavity's editor in conflict, b's file `mesh.nx` 11; (h) the request frame gives `CancelClose`; `Unsaved edits`, `cases/cavity.toml`, `Save all and close`, `Discard and close`, `Cancel` drawn; the click's frames give `Close`; the next request gives no `CancelClose` | ADR 0148's route: (a) closes at once without asking; (h) no `CancelClose` |
| O8 | The bound, lowered to 2 (model): the cavity, b and c each edited (`mesh.nx` 20, 11, 13) and left in turn; hex selected; the cavity selected; the cavity saved, hex selected | the refusal exactly `cases/hex.toml was not opened: 2 other cases hold unsaved edits, the most the Studio keeps; save or discard the edits of one of them first (marked * in the Project panel)`; S still c; kept b, cavity; after the cavity: kept b, c; after its save and hex: kept b, c. `MAX_PARKED` is 32. Reported: the largest kept settings tree as JSON in bytes | ADR 0148's route: nothing kept, hex opened, no refusal |
| O9 | Typed and left in the window (drawn, the editor filtered to `mesh.nx`): the cavity clicked; "20" typed into `mesh.nx` (two Backspaces, `2`, `0`, the field focused); a drawn click on b's label; settled; a drawn click on the cavity's label; settled | the text 20 is kept: either the typed field `("mesh.nx", "20")` or `mesh.nx` 20 committed and dirty (which one is reported: it depends on whether the field or the Project panel sees the click first); the field drawn shows `20`; Run refused; the preview 320 cells from the editor | ADR 0148's route: the cavity clean with `mesh.nx` 16, nothing typed, the field shows `16` |
| O10 | A JSON case runs (a served project): `cases/t.toml` the 12 x 12 cavity with 30 iterations, `cases/j.json` its `Case::to_json_string()` (`rcfd-api`'s writer); (a) the settings of both; (b) `StartPlan::new` of the JSON text; (c) the JSON case selected, settled, Run; (d) its run's `case.snapshot` and end; the TOML case selected and Run; both runs' `residuals.jsonl`; (e) the Start run dialog's plan of the JSON case | (a) `parse_case` of the JSON text and `Case::load` of the TOML file give the same settings (`to_json_string` equal); (b) planned: `simplec`, 30 iterations, 144 cells (what it says of its store is reported); (c) queued; (d) the snapshot byte-equal to `cases/j.json`, finished within 60 s; the residual lines of the two runs equal bit for bit (step, time, residuals, monitors), at least one; (e) `Ok`, the plan of `cases/j.json` | the route before (`cases_as_toml`): (c) refused with a text starting `cases/j.json was not started: cases/j.json: not TOML: `; (e) an error starting `cases/j.json: not TOML: ` |
| O11 | Benchmark descriptors (a project, not served, with the repository's 6 `cases/<id>/case.json` copied to the same paths; the run worker on, Run searching this build's runner): for each, (a) `StartPlan::new`; (b) selected and Run at once (before the read); (c) settled: Run's state, then Run; (d) the Start run dialog's plan; then (e) the cavity's descriptor selected, the pointer on Run (30 frames) | for each (`line` = `descriptor_message(<case>, <text>)`, one line holding `: not a case file: `): (a) `Err(line)`; (b) exactly `<case> was not started: <line>`; (c) `blocked` exactly `line`, Run exactly `Run: <line>`; (d) `Err(line)`; no run folder; (e) the line drawn on hover | the route before: (c) Run not greyed out, the start's error starts `<case> was not started: <case>: not TOML: `; (d) the error starts `<case>: not TOML: ` |
| O12 | ADR 0139 CS7 as amended (amendment 3): the descriptions of the 10 examples | as before, and each run length exactly `<algorithm>, to simulated time T s` (transient: `piso`, `pimple`, transient `compressible`; T as Rust prints `solver.end_time`) or `<algorithm>, at most N iterations`, the line holding it followed by `.`; no line holds ` t = `; at least one transient example | the words before (`describe_with(.., true)`): each transient example's run length holds ` t = ` and differs from the limit |
| O13 | The rest: the whole `rcfd-studio` suite (with the changed earlier tests below), `workspace_graph`, `cargo clippy -p rcfd-studio --release --all-targets -- -D warnings`, `cargo fmt -p rcfd-studio -- --check`, `scripts/third_party_licenses.py --check` | pass; the four known misses ignored as before (ADR 0131 B9, ADR 0133 R6, ADR 0141 G1, ADR 0144 J3b) | (their own) |

How the registered runs are made: `cargo test --release -p rcfd-studio --test linked_o -- --test-threads 1 --nocapture`,
the same for `drafts_o`, `json_o` and `case_starter_g2` (O12), each output to a file; then the whole `rcfd-studio` suite
once with `--no-fail-fast -- --test-threads 1 --nocapture` (`fake-claude` built first), output to a file (O13); clippy,
fmt, `workspace_graph` and the licence check on the final commit.

## Changes to earlier tests (registered here, none run yet)

Each follows from a decision of the maintainer above; limits not named are unchanged.

* **ADR 0148 N1 (c)** (`selection_n.rs`): the descriptor, clicked and settled, greys Run out; Run's refusal is exactly
  `Run: ` and the descriptor's one line (before: the start's `cases/cav/case.json was not started: ...`). Its control
  (the route before ADR 0148) unchanged.
* **ADR 0148 N3 (c)** (`selection_n.rs`): back on `a`, its editor holds `mesh.nx` 9, dirty (kept; before: 8 from the
  file, clean). (a), (b), (d) and the control unchanged.
* **ADR 0148 N4** (`selection_n.rs`): after the first start, the queued run is not the selected run and the selected run
  is none (c's newest run; before: the queued run selected); the side, the run shown and the Mesh side's case as
  registered; the tuple gains the selected run. (b) and the control unchanged.
* **ADR 0148 N11** (`scale_n.rs`): (d) and (e) open the cavity again with its edits discarded (`discard_edits`; the
  helper `open_fresh`), because a click no longer drops them: (a)'s 32 x 32 would otherwise come back. Limits unchanged.
* **ADR 0144 J1, the J1 controls and J11** (`live_preview_j.rs`): run with `case_run_unlinked`, so that their limits keep
  checking ADR 0144's toggle as registered (with the linking a case of the B1 fixture selects its newest run: J1 (b)'s
  `NO_RUN`, J1 (f)'s "the run still selected" and J11 (b)'s disabled Run no longer hold by design). O1 and O2 check the
  linked machine.
* **ADR 0140 SF1** (`store_plan_sf.rs`): the 6 shipped descriptors are refused by `StartPlan::new` with exactly
  `descriptor_message(<case>, <text>)` (before: an error starting `<case>: not TOML`).
* **ADR 0139 CS7** (`case_starter_g2.rs`): amendment 3 of ADR 0139 (O12): the run length limited; a new control test.
* Earlier tests whose path changes while their limits hold (from the reading below; reported, not changed): ADR 0144 J9
  and ADR 0147 M4 (the starter's new case: the selected run becomes none), ADR 0144 J12, ADR 0147 M10, ADR 0142 H1, ADR
  0131 B5, B6 and the parity tests (an opened case selects its newest run), ADR 0131 B4 and B7 (a run selected for a
  moment before the test writes its own), ADR 0147 M2 (e) (the cavity comes back with its kept edits, `mesh.nx` 20 and
  `mesh.lx` typed `2`, before its `mesh.source` change) and M3 (e) (the cavity's edits kept).

## What I already knew (rule d)

* The code reading above.
* **Baseline before any change of this job**, on main 9e857d8: a release build in this job's target folder (260 s with
  `rcfd-runner`, `rcfd`, `fake-claude` and the test binaries), then `cargo test --release -p rcfd-studio --no-fail-fast --
  --test-threads 1 --nocapture`, output to a file: 52 test binaries, **212 passed, 0 failed, 4 ignored** (the four known
  misses), 418 s (10.8 GB free at the start, 13.6 GB at the end).
* A read-only prediction of the earlier tests these decisions touch (a subagent's reading of every test file against a
  description of the changes; nothing built or run): N1 (c), N3 (c), N4, N11, J1, the J1 controls, J11 and SF1 would
  fail, the tests listed above change values without failing, the rest are not touched; its conditions (N1's control
  needs `blocked` to test the case `quick_run::selected_case` returns, as implemented; ADR 0144 J2 (c) needs a run Run
  just queued to count as the case's newest before it is listed, which is why `newest_run_of` counts it).
* The code and the tests of this ADR were compiled (`cargo test --release -p rcfd-studio --no-run`), `cargo clippy -p
  rcfd-studio --release --all-targets -- -D warnings` is clean after one `manual_contains` and six `type_complexity`
  findings in the new tests were fixed, `cargo fmt -p rcfd-studio` applied. No registered check of this ADR, and none of
  the changed earlier tests, has run.

## Amendment 1 (2026-10-28, found in review after the registration c057aee; before any registered run)

A review of the registration found a hazard decision 2 opens: a case editor taken back at the click is drawn in the
frame of the click, and the case editor's text fields had ids from their place in the panel and one text buffer per
settings path for every case (`UiState::buffers`). A field of the case left that still had the focus in that frame would
then be the same widget as the field of the same path of the case taken back: the case taken back could show the text
typed in the other case and, when that field lost the focus, commit it as its own. Before decision 2 the editor showed
`Loading <case>...` in that frame (no field), so the focus simply went.

Change (decision 2 gains it): each text field of the case editor has an id salted with its case and path, and its text
buffer is kept per case and path (`ui.rs` `buffer_key`, `Field`); test switch `UiState::shared_field_ids` (the route
before). Behaviour is otherwise unchanged.

What I knew when writing this: a throwaway probe (`crates/rcfd-studio/tests/zz_diag_o.rs`, run once in release on the
changed code with and without the switch, deleted, never committed): b kept with `mesh.ny` 12 committed, "20" typed into
the cavity's `mesh.nx` with the field focused, then a drawn click on b's label. In both routes the press on the label
took the focus from the field first, so the 20 was committed to the cavity (kept with `mesh.nx` 20, dirty, no field
typed) and b kept `mesh.nx` 10 and no field typed: with the default layout and a pointer the hazard does not show, so no
control of the route before can fail. The probe also shows how O9 will most likely report (committed); O9's limit takes
either outcome, as registered.

**O9b** (`o9b_a_typed_text_stays_with_its_case`, `drafts_o.rs`, registered here before its run; a guard, no control): b
kept (`mesh.ny` 12 committed and left); in the cavity "20" typed into `mesh.nx` with the field focused; a drawn click on
b's label; settled. Limits: b's `mesh.nx` 10 and b has no field being typed; the cavity is kept holding its 20, either as
the field being typed or committed and dirty (which one is reported). Every registered file (`linked_o`, `drafts_o`,
`json_o`, `case_starter_g2`) and the whole suite run on the commit of this amendment.

## Amendment 2 (2026-10-28, two set-up defects found by the registered runs; before the corrected runs)

The registered runs of the four files on 894b659 (amendment 1): `json_o.rs` 4 of 4 and `case_starter_g2.rs` 26 of 26
passed; `drafts_o.rs` passed 15 of 16; `linked_o.rs` passed O2, O3 and their controls and stopped O1 and its control.

* **O1 and its control** stopped at step (a), `did not settle: pending None, preview Idle`. The test's `settle` waited for
  `lj::settled`, which does not count an idle preview as settled; with no case selected the preview is idle for good. The
  registered "settled" (nothing being read, the preview neither building nor waiting for its debounce) holds then. A
  defect of the helper, not of the code: none of O1's limits was read.
* **O8's control** stopped with `cases/cavity.toml: nothing to save`: the procedure, shared with O8, saves the cavity
  after selecting it again; in ADR 0148's route the cavity comes back clean from its file, so the save is refused and the
  test's `unwrap` ends the control before its limits are read. O8 itself passed.

Corrected set-ups (the same test names; every limit and control unchanged): `settle` in `linked_o.rs` waits for nothing
being read and, when a case is asked for, `lj::settled`; O8's procedure saves the cavity only when its editor holds
unsaved edits (O8's own path, where it does, is unchanged). What I knew when writing this: the output of those runs (the
values are in the addendum); the corrected tests had not run. Every registered file and the whole suite run again on the
commit of this amendment.

## Amendment 3 (2026-10-28, O1's control after amendment 2; before its corrected run)

The corrected runs on 3198a8e (amendment 2): `drafts_o.rs` 16 of 16, `json_o.rs` 4 of 4, `case_starter_g2.rs` 26 of 26,
`linked_o.rs` 5 of 6: O1 passed (every limit), and **O1's control** stopped at (c'), `called Result::unwrap() on an Err
value: "No run is selected: ..."`: the procedure, shared with O1, toggles to the Run side after (c) and stopped on a
refusal, but in ADR 0148's route the cavity's click selects no run, so the toggle is refused there by design. None of the
control's limits had been read (it measured (c): no run selected, as the control requires). A defect of the procedure.

Corrected set-up (the same test name; every limit and control unchanged): a refused toggle is printed and the procedure
goes on (O1's limits read the side after each toggle, so a refused toggle in O1 itself still fails its limit). What I knew
when writing this: the output of that run (O1's values are in the addendum); the corrected control had not run. The four
registered files and the whole suite run again on the commit of this amendment.

## Not in CI (by hand)

A person at the real window: a case clicked and a run clicked in turn (the highlight, the name next to Run, the toggle
and the caption agree), the `*` marks and their hover, typing into a field and clicking another case, the close
question (menu, another project, the window's close button), "Discard edits", a `.json` case run, Run greyed out on a
descriptor.

## View-and-report checklist for the maintainer

1. Build and start: `powershell -ExecutionPolicy Bypass -File scripts\studio.ps1 example-projects\wind-tunnel` (or a copy
   of it).
2. Click a case that has runs: the Mesh side shows it; click the toggle's Run: its newest run, the caption naming it and
   the case. Click an older run of the same case in the Runs panel: the Run side shows that one; the Project panel still
   highlights the case. Click a run of another case: the Project panel, the name next to Run and the case editor go to
   that case; the toggle shows Run.
3. Click a case without runs: the toggle's Run is greyed out and says `<case> has no run yet ...`.
4. Start a run with Run (F5) and click another case at once: the Viewport stays on that case's mesh; the run appears in
   the Runs panel; click its case: it is that case's newest run.
5. Edit a field of a case (Enter), click another case: a `*` follows the first case in the Project panel. Type into a
   field without Enter and click another case: back on it, the field shows what you typed. Run and Runs > Start run...
   refuse a case with a `*` ("save it first"). "Discard edits" in the case editor drops them.
6. With a `*` showing, use Project > Close project, open another project, or close the window: the question "Unsaved
   edits" lists the cases; try Cancel, then "Save all and close" (or Discard).
7. Copy a case to `cases/<name>.json` with the case editor (or write one) and run it: it runs. Click a repository
   descriptor copied in as `cases/<id>/case.json`: Run is greyed out with the one line "not a case file"; Runs > Start
   run... shows the same line.
8. Case > New case from example...: a transient example says "to simulated time ... s".
9. Report anything slow, confusing or wrong, with the case and what was done.

## Questions for the maintainer (not decided here)

1. **A run Run queued after another case was selected** is not selected now (decision 1, changing ADR 0148 decision 1
   (d)). Options: (a) keep (the selection stays one case); (b) select it and its case (the selection goes back to the
   case Run was pressed on, a moment after the click elsewhere); (c) ADR 0148's rule (selected, its case not: the Run
   side, the strip and the plots then name another case than the Project panel). Recommendation: (a).
2. **A case without runs empties the strip and the plots**: a click on it deselects the run selected before, also a live
   run of another case (it keeps running and is listed; a click on its row selects it with its case). Options: (a) keep;
   (b) the strip keeps a live run of another case while the plots and the Run side follow the selection.
   Recommendation: (a) until it is missed.
3. **A run of a case that is gone** keeps the selected case ("select nothing new", read as "do not change the
   selection"). Options: (a) keep; (b) select no case (the editor and the Mesh side empty). Recommendation: (a).
4. **The case starter** still does not open its new case over unsaved edits or a conflict (ADR 0139 D5, CS9), although
   such edits are kept per case now (it does park a field being typed). Options: (a) keep; (b) keep the edits of the
   case left and open the new case (CS9 changed). Recommendation: (b).
5. **A typed field and a change on disk.** Back on a kept case, a typed field counts as an edit (a conflict, the text
   kept). The open case keeps ADR 0131's rule: a clean editor with a typed field reloads when the file changes (the field
   keeps showing the text and commits it on focus loss). One rule for both? Recommendation: yes, the typed field counted
   as an edit everywhere.
6. **One reader of case text.** The `{`-rule is written three times (`rcfd-cli`'s `load_case_in`, `rcfd-mcp`'s
   `project.rs`, the Studio's `preview::parse_case`); the Studio now uses its own everywhere. Options: (a) keep; (b) one
   `rcfd-api` function (for example `Case::parse_text_with`) called by all three (touches `rcfd-api`, `rcfd-cli` and
   `rcfd-mcp`). Recommendation: (b), as its own small job.
7. **The bound** of 32 kept cases, refusing the next switch with a message. Accept, or another number?
8. **The mark** `*` after a case label (the status bar already writes `<case> *` for the open case). Accept, or a dot?

## Files

This ADR; hunks in `crates/rcfd-studio/src/{studio,case_edit,quick_run,view_toggle,viewport_ui,case_starter,runs,
store_plan,preview,poll,watch,ui}.rs`; new tests `crates/rcfd-studio/tests/{linked_o,drafts_o,json_o}.rs`; changed tests
`crates/rcfd-studio/tests/{selection_n,scale_n,live_preview_j,store_plan_sf,case_starter_g2}.rs`; ADR 0139 amendment 3;
dated notes in ADRs 0140, 0144 and 0148.
