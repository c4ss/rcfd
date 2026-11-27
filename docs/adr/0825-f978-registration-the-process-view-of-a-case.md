# ADR 0825: F-978 registration: the process view of a case, from its creation to its last result

Status: **REGISTERED 2026-11-27, written before any code of F-978 and before any run of its checks.** Before this text no
library, server, Studio or test code of F-978 was written and no file of the repository was changed. What was done:
reading (below), searches of the source tree, and, on the unchanged tree, a release build of `rcfd`, `rcfd-mcp`,
`rcfd-runner` and the `f447_bench` example into the scratch target and `npm ci` in `apps/studio-web` (neither is a
check). Results go in addenda at the end; nothing above them is changed afterwards. Written by Claude Opus 5.5
(`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f978-process-view.md` ([the file](briefs/f978-process-view.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f978-process-view` from dcb5cb3e, scratch
`w49i-work/f978-process-view-scratch`. The decision it comes from is [ADR 0803](0803-the-studio-as-claudes-canvas-record.md)
(ability 3, guardrail G23 of [studio-guardrails.md](../studio-guardrails.md)); it builds on
[ADR 0805](0805-f976-registration-a-view-specification-and-show-view.md) (`rcfd-view` 1, `show_view`, `read_view`),
[ADR 0807](0807-f979-registration-live-meshes-settings-and-monitors-on-the-canvas.md) (the live canvas, `settings` in
`read_view`) and [ADR 0188](0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) (the journal
and the feed).

## What is asked

Roadmap item F-978 (deps F-976, F-273), the binding acceptance, read with a script:

> Every limit not stated here is registered in the ADR before the run. A view shows, for a case, the ordered path from its creation through mesh, set-up and each run (state changes, residuals, monitors) to its results, read from the journal, the run records and their events (the feed of ADR 0188); each step opens the view of its state. An e2e test builds a case through the MCP tools from new to a finished run, and the view lists every journaled step in order, none missing against the journal.

Clauses: **a1** a view shows, for a case, the ordered path from creation through mesh, set-up and each run (state
changes, residuals, monitors) to its results; **a2** read from the journal, the run records and their events; **a3** each
step opens the view of its state; **a4** an e2e test builds a case through the MCP tools from new to a finished run;
**a5** the view lists every journaled step in order, none missing against the journal. From the brief and G4: the
specification changes by addition, the new kind is named in `show_view`'s description, every other default unchanged
(**a6**).

## What was read

`AGENTS.md`, the rules (version 5) and the brief, whole; [ADR 0803](0803-the-studio-as-claudes-canvas-record.md) and
[ADR 0805](0805-f976-registration-a-view-specification-and-show-view.md) whole; of the guardrails G3, G4, G12, G13, G16
and G23; of `crates/rcfd-workspace/src/view.rs` the constants, `view_errors`, `panel_errors`, the reference checks,
`show_view`, `read_view`, `setting_values`, `export_view` and `panel_html`'s dispatch; `apps/studio-web/src/renderer/canvas/Canvas.tsx`
and `model.ts` whole; of `session.ts` `canvasReadsAgain`, the feed poll and `readCanvas`; of `src/shared/project.ts` the
canvas types; of `crates/rcfd-workspace` `journal.rs` (`append`, `Reader`), `feed.rs` (the module text, `Change`,
`journal_text`, `journal_entry`), `cases.rs` `create_by`, `runs.rs` (the `run_submitted` and `run_control` lines),
`write.rs` `edit_case`, `facts.rs` `refine_case_mesh`, `create.rs` `prepare_case`, `series.rs` `residual_series`; of
`rcfd-project` `time.rs`, `atomic.rs` (the `edit` line) and `records.rs` (`RunRecord`, `RunEvent`, `EventKind`); the
quick-start example's run records; `tests/e2e/canvas-f979-a1.spec.ts` whole, the checks of `canvas-f976.spec.ts`;
`crates/rcfd-mcp/tests/canvas_live_f979.rs` (head and B1), `project_list_f270.rs` (the blocks of ADRs 0805 and 0823),
`tool_list_f275.rs` (`PROJECT_TOOLS`); `tests/unit/hand-only-retired.spec.ts`; the capability inventory's kinds; of
ADR 0807 finding 8 and Q1, of ADR 0823 finding 5 and its L6 miss; `scripts/process_report.py` `sec_r`. Bytes read: about
290 000.

## Findings of the reading (before any code)

1. **The journal lines of a case.** `case_created` (`path` the case, `from`; `new_case`), `edit` (`path`, `who`
   `edit_case`, `changes` [{path, old, new}]; `refine_case_mesh` writes through `edit_case`, so its line is an `edit` with
   `mesh.*` changes), `run_submitted` (`run`, `case`), `run_control` (`run`, `requested`), and the trash's `trashed` and
   `restored`. Each has `t` (`rfc3339_ms`), `seq`, and `actor` and `reason` where known. The feed's entry text of a line
   is `feed::journal_entry(line).text`.
2. **The run records.** `runs/<id>/run.json` names the case (`case`); `events.jsonl` holds `state` (`from`, `to`,
   `reason`), `warning`, `stop` and other kinds (`estimate`, `launched`, `solver`), with `t` in nanoseconds
   (`rfc3339_ns`); the feed takes `state`, `warning` and `stop` only (ADR 0188 J6, `feed.rs` point 3). `residuals.jsonl`
   holds per step `residuals.*` and `monitors.*` (`residual_series`). `fields/` is the run's field store.
3. **Times are of two widths** (ms in the journal, ns in run events), so their text order is not time order; they are
   compared as parsed numbers (D3).
4. **`read_view` already carries engine answers per panel** (`settings`, ADR 0807 D2) and the session reads it again on
   a feed entry that changes them; a process panel takes the same road, with no new tool.
5. **No test pins `show_view`'s description text** (`project_list_f270.rs` takes the canvas tools off the end by name,
   access and required keys; no JSON golden holds them; the inventory holds names and access). A description that gains
   the new kind therefore changes no golden file and no count; with no new tool, flag or method, `npm run capabilities`
   should leave the inventory and `docs/studio-coverage.md` as they are (L3).
6. **`export_view` of a kind it does not draw** fails with -32000 (`INTERNAL`) before writing; a process panel there is
   given a located refusal instead (D5).

## Design

### D1. The panel kind `process`

`rcfd-view` 1 gains, by addition, the kind `process` with the keys `kind`, `title`, `case`; `case` is checked as a mesh
or setting panel's case is (`case_read`: a file under `cases/` that exists and reads as a case). Any other key is
refused at its dotted key, as for the other kinds. `KINDS_F978 = ["process"]` is added beside `KINDS_F979`, and the
reason of an unknown kind lists it. `show_view`'s description gains, after the table kind: `process {case} (the case's
steps from its creation to its last result, which read_view gives as process; each step names the view of its state)`.

### D2. The steps (the engine: `Workspace::process_steps(case)`)

For case `C` (project-relative, `/`): the run set `R(C)` is every run id `r` with a `run_submitted` line of `case` `C`,
and every `runs/<r>/run.json` whose `case` is `C`. A journal line **concerns** `C` when (a) its `path` is `C`, or (b) it
is `run_submitted` with `case` `C`, or (c) its `run` is in `R(C)`. Every journal line of `.rcfd/edits.jsonl` that
concerns `C` is a step, of the kind:

| journal line | step | phase | opens (D4) |
|---|---|---|---|
| `case_created` | `created` | creation | `case {case: C}` |
| `edit` with a change whose path starts `mesh.` | `mesh` | mesh | `mesh {case: C}` |
| any other `edit` | `setup` | set-up | `case {case: C}` |
| `run_submitted`, `run_control` | `run` | run | `run {run}` |
| any other kind | `other` | other | `case {case: C}` |

For each run `r` of `R(C)` whose `run.json` exists, in order of `events.jsonl`: a `state` event is a step `state`
(`from`, `to`, `reason`), a `warning` a step `warning`, a `stop` a step `stop`, phase run, opening `table {runs: [r]}`;
then, if `residuals.jsonl` holds a sample, one step `residuals` (phase run): the sample count and the last point
`[step, value]` of every `residuals.*` and `monitors.*` series of `residual_series` (the values `read_residuals` gives),
opening `plot {run: r}` and one `monitor {run: r, monitor}` per monitor (at most 7, by name); then, if the field store
holds a snapshot, one step `results` (phase results): the snapshot count and the field names of the last snapshot,
opening one `field {run: r, field}` per field (at most 8, in the store's order). These two take the `t` of the run's
last event.

A step is `{i, step, phase, source ("journal" or "run"), t, text, opens}` with, where they apply, `seq`, `kind` (the
line's or event's kind), `run`, `who`, `actor`, `reason`, `changes`, `from`, `to`, `message`, `samples`, `last`,
`snapshots`, `fields`. `text` is the feed's text for a journal line, and `run <r>: <from> to <to>`, `run <r>: warning:
<message>`, `run <r>: stopped: <reason>`, `run <r>: <n> samples of residuals and monitors`, `run <r>: <n> snapshots of
<fields>` for the others. At most `MAX_STEPS` = 1000 steps are given (the feed's `KEEP`), the first ones, with
`steps_total` all of them.

### D3. The order

A merge of ordered sequences: the concerned journal lines in file order, and each run's steps (D2) in their order. At
each point the sequence whose next step has the earliest time goes next; on equal times the journal first, then the runs
by id. A time `YYYY-MM-DDTHH:MM:SS[.fraction]Z` is compared as (the seconds, the fraction as nanoseconds); a step whose
`t` is absent or does not parse takes the time of the step before it in its own sequence (zero at the start). So the
journal's steps keep the journal's order (by `seq`), a run's steps keep the order of its records, and the times of the
list never decrease where every time parses.

### D4. Each step opens the view of its state

Each step's `opens` is a view specification `{version: 1, panels}` built from its row of D2; every panel is checked
alone by `view_errors` and kept only if valid, and `opens` is null when none is. A client shows it with
`show_view {view: opens}` (G16: the engine's action); in the Studio, picking a step draws `opens` under it with the
canvas's own panels (the short way to look of G23), and picking it again closes it. Nothing is written by picking.

### D5. `read_view`, the session and the canvas

- `read_view` gains `process`, only for a view with a process panel: `[{panel, case, steps, steps_total}]` (as
  `settings`, ADR 0807 D2). A view without one gets the reply it had, key for key.
- `export_view` of a view with a process panel is refused (1003) at `panels[<i>].kind` with the reason that the
  exported page does not draw it, before anything is written (Q1).
- `src/shared/project.ts`: `process` in `CanvasKind`, the types `ProcessStep` and `CanvasProcess`, `process` in
  `CanvasView` (additions). `session.ts`: `readCanvas` keeps `process`; `canvasReadsAgain` is also true when the view has
  a process panel and a fresh entry is not `view_shown` or `view_exported` (so the list follows the journal and the runs'
  state changes).
- `Canvas.tsx`: the kind `process` draws `ProcessPanel`: an ordered list `ol.cv-process` (`data-steps`,
  `data-steps-total`), one `li` per step (`data-step` its index, `data-step-kind`, `data-phase`, `data-source`,
  `data-seq`, `data-run`, `data-t`) holding a button (`aria-expanded`) with the phase and the text, then the actor and
  reason, the changes (old to new), the state change, the last values (residuals) or the fields (results), as the
  engine gave them. A picked step draws `div.cv-opened` (`data-opened-step`, `data-panels`) with one
  `[data-opened-panel][data-canvas-kind]` per panel of `opens`, each drawn by the canvas's `Panel`. `canvas.css` gains
  the list's rules in the Studio's tokens only.

## Checks (rule 14)

Release profile. New: unit tests in `crates/rcfd-workspace/src/view.rs`, `crates/rcfd-mcp/tests/canvas_process_f978.rs`
(the binary), `apps/studio-web/tests/e2e/canvas-f978.spec.ts`. The registered build of B1 and E1: on a copy of the
quick-start project, through the MCP tools: `new_case {name: process-cavity, copy: cases/cavity-re100.toml}`;
`edit_case` of `mesh.nx` and `mesh.ny` to 16 (mesh); `edit_case` of `physics.nu` to 0.02 (set-up); `start_run
{max_steps: 40}` and `wait_run` until terminal; `edit_case` of `solver.max_iterations` to 500 (a set-up after the run);
each with a reason. The journal filter of the tests is the rule of D2 (a) to (c), written again in each test, not taken
from the engine.

| Id | Clause | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| V7 | a6 | unit: a view with a process panel of `cases/cavity-re100.toml` (and a title) | accepted | F-978 acceptance; G4 | accepted | exact | code verification |
| V8 | a6 | unit: 4 invalid process panels: no `case`; a case that does not exist; a key `run`; a case outside `cases/` | each refused at exactly `panels[0].case`, `panels[0].case`, `panels[0].run`, `panels[0].case`, code 1003 | F-976 acceptance (located errors), as D1 | 4 of 4 | exact | code verification |
| P1 | a1, a2, a5 | unit, quick-start copy: `edit_case` of `physics.nu` and `refine_case_mesh` of `cases/cavity-re100.toml` through `Workspace::call`, then `process_steps` | the journal steps' `seq` equal the concerned lines' `seq` in file order; per run of the case, the `state`, `warning` and `stop` steps equal its events of those kinds in file order, then one `residuals` and one `results` step; the two edits are `setup` and `mesh`; times never decrease; every `opens` is valid | F-978 acceptance ("in order, none missing against the journal"); D2, D3 | as stated | exact | code verification |
| B1 | a1 to a5 | binary: the registered build from a client, a `--canvas` process, `show_view` of `{panels: [process]}`, the canvas's `read_view` | `process[0].steps`: journal steps' `seq` equal the concerned lines' `seq`; first step `created`; steps `mesh`, `setup`, `run`, then the run's state steps equal to its `events.jsonl` state events (from, to) in order, ending at a terminal state; `residuals` `last` equal to `read_residuals`' last point of every series; a `results` step; the after-run `setup` last; times never decrease; every step's `opens` non-null and accepted by `show_view` | F-978 acceptance; G3 (values traced to the solver); D2 to D4 | as stated | exact | code verification |
| B2 | a6 | binary: `export_view` of the process view | 1003 at `panels[0].kind`; no `exports/` file; the journal byte-identical | D5; rule 3 | as stated | exact | code verification |
| B3 | a6 | binary: the `tools/list` description of `show_view`; `read_view` of a view without a process panel | contains `process {case}`; no key `process` in the reply | brief (the kind named in the description); rule 3 | as stated | exact | unchanged default output |
| E1 | a1, a4, a5 | e2e: the packaged Studio has the project open; the test, as MCP client of `rcfd-mcp --project`, makes the registered build, then `show_view` of the process view | within 1.6 s of the tool's result the list is shown; its journal steps' `data-seq` equal, in order, the `seq` of the lines of `.rcfd/edits.jsonl` that concern the case (none missing, none extra); its steps equal `read_view`'s `process[0].steps` one for one (index, kind, phase, run, seq, text); its `data-t` never decrease; the residuals step's values equal `read_residuals`' last points | ADR 0188 C3-St (4 Studio periods, `FEED_POLL_MS` 400 ms) for the time; F-978 acceptance | 0.6 s | 2.7 | code verification |
| E2 | a3 | e2e: each step picked in turn | its `cv-opened` shows `data-panels` equal to its `opens`' panel count, each panel of the kind of `opens` with that kind's existing view in it (case: its `tr[data-path]`; mesh: `.cv-mesh .mp`; table: `tr[data-run]`; run: the heading `Run workspace: <id>`; field: `section.fv`; plot, monitor: `.plot-frame`); every step of the build has `opens` | F-978 acceptance ("each step opens the view of its state"); D4 | all steps | exact | code verification |
| E3 | a1 | e2e: with the view shown, one more `edit_case` with a reason | within 1.6 s a new last step `setup` with that line's `seq` and reason | ADR 0188 C3-St | 0.6 s | 2.7 | code verification |
| L1 | a6 | unedited on the branch: `rcfd-workspace --lib`, `rcfd-mcp --lib`; `canvas_view_f976`, `canvas_live_f979`, `canvas_layout_f977`, `export_view_f980`, `project_list_f270`, `tool_list_f275`, `project_w48f`, `default_f1`, `feed_f273` | pass | rule 3; rule 7 | pass | exact | unchanged default output |
| L2 | a4, a6 | `npm run typecheck`, then `npm test` in `apps/studio-web` once (every spec unedited, against the branch's engine and its `f447_bench`) | typecheck exit 0; every test passes but main's 6 registered skips (ADRs 0807, 0818) | rule 7; G13 | pass | exact | unchanged default output |
| L3 | a6 | `npm run capabilities` (`RCFD_BIN` the branch's `rcfd`) | `capabilities/inventory.json` and `docs/studio-coverage.md` unchanged (`git diff` empty) | rule 3; finding 5 | unchanged | exact | unchanged default output |
| L4 | a6 | `cargo test -p rcfd-cli --release --test docs_site` | pass | rule 7 | pass | exact | unchanged default output |

No shared solver file is touched, so `baseline_hashes` is not run. fmt and clippy of `rcfd-workspace` and `rcfd-mcp`.

### Negative controls and mutations (rule 16)

V8 and B2 are the negative controls. Mutations, each on a scratch copy of the final commit, restored afterwards:

| Id | Mutation | Must fail |
|---|---|---|
| M1 | rule (a) of D2 leaves out `edit` lines | B1 (journal seq), P1 |
| M2 | the run steps appended after the journal's instead of merged (D3) | P1 (times), B1 (the after-run set-up not last) |
| M3 | a residuals step's `last` taken from the first sample | B1 |
| M4 | no `opens` for a `results` step | B1 |
| M5 | the Studio does not draw a picked step's `opens` | E2 |

M1 is committed as `crates/rcfd-mcp/tests/canvas_process_f978.mutation.patch`, first line
`# expect-fail: cargo test --release -p rcfd-mcp --test canvas_process_f978 --target-dir target/mutation`.

## Size

Size (agent): done; 0; 1; 1; 290000

(Expected done; no further registration; one heavy run, the whole `npm test`; one question; about 290 000 bytes read.)

## Decision rule

**done** if V7, V8, P1, B1 to B3, E1 to E3 and L1 to L4 hold and M1 to M5 are each caught; **partial** otherwise, naming
the check that missed. A missed limit is recorded with its number and held as a `*_known_miss` record (rule 2).

## Not covered

- A mesh imported with `import_mesh` that the case names: its `mesh_imported` line has the mesh's path, not the case's,
  so it is not a step by D2; the case's edit that names it is.
- The steps show the state as the journal and the records hold it; a step opens the view of the case or run as it is
  now (the engine keeps no earlier copy of a case besides a run's `case.snapshot`).
- Two runs of a case at once, a paused run, a run moved to the trash: covered by the rule, not driven end to end.
- Linux and macOS.

## Open questions for the maintainer

- **Q1. A process panel in an exported page** (scope): (a) `export_view` refuses it, as here (D5); (b) a follow-up item
  draws the step list in the page of ADR 0823. Recommendation: (b), entered as an item of its own after F-978; until then
  (a), taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
