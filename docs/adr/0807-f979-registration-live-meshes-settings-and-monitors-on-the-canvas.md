# ADR 0807: F-979 registration: meshes, settings and monitors shown live on Claude's canvas

Status: **REGISTERED 2026-11-26, written before any code of F-979 and before any run of its checks.** Before this text no
library, Studio or test code of F-979 was written and no file of the repository was changed. What was done: reading
(below), searches of the source tree, and, on the unchanged tree, a release build of `rcfd`, `rcfd-mcp`, `rcfd-runner`
and the example `f447_bench` into the scratch target, and `npm ci` in `apps/studio-web` (neither is a check). Results go
in addenda at the end; nothing above them is changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a
background agent of the lead.

It answers the brief `docs/adr/briefs/f979-canvas-live-mesh-values.md` ([the file](briefs/f979-canvas-live-mesh-values.md))
and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f979-canvas-live-mesh-values`
from main 5bf77c58, scratch `w49i-work/f979-canvas-live-mesh-values-scratch`. The decision it comes from is
[ADR 0803](0803-the-studio-as-claudes-canvas-record.md) (ability 4: the live view of a mesh being generated or edited and
of values changing); it builds on [ADR 0805](0805-f976-registration-a-view-specification-and-show-view.md) (F-976: the
view specification `rcfd-view` 1, `show_view`, `read_view` and the Canvas tab).

## What is asked

Roadmap item F-979 (deps F-976, F-447, F-272), the binding acceptance, read with a script:

> Every limit not stated here is registered in the ADR before the run. With a view open, a mesh generated or edited by an MCP tool is drawn again in the open Studio, and a setting or a monitored value changed by an MCP client or the runner is updated in the shown view, each within a registered limit of the feed's poll (ADR 0188). The change names its actor and, where one is given, its reason. An e2e test drives both from the rcfd-mcp binary; the feed's format is unchanged.

Clauses: **a1** a mesh generated or edited by an MCP tool is drawn again in the open Studio within a registered limit of
the feed's poll; **a2** a setting changed by an MCP client is updated in the shown view within such a limit; **a3** a
monitored value changed by the runner is updated in the shown view within such a limit; **a4** each change names its
actor and, where one is given, its reason; **a5** an e2e test drives them from the `rcfd-mcp` binary; **a6** the feed's
format is unchanged (and, from the brief and G4, every engine interface changes by addition only).

## What was read

`AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, whole; [ADR 0803](0803-the-studio-as-claudes-canvas-record.md)
whole; [ADR 0805](0805-f976-registration-a-view-specification-and-show-view.md) whole (with addendum 1 and the answers);
of [studio-guardrails.md](../studio-guardrails.md) G3, G4, G12, G13, G16, G22 and G23; `crates/rcfd-workspace/src/view.rs`
whole; `apps/studio-web/src/renderer/canvas/Canvas.tsx` and `canvas.css` whole; `apps/studio-web/src/main/project/session.ts`
whole; of `apps/studio-web/src/shared/project.ts` the tool list, `FEED_POLL_MS`, `RUN_POLL_MS`, `RunEntry`, `ChangeEntry`,
`ProjectView` and the canvas types; `apps/studio-web/tests/e2e/canvas-f976.spec.ts` whole; of
[ADR 0188](0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) the checks C3-F, C3, C3-R and
C3-St; of [docs/mcp-tools.md](../mcp-tools.md) the rows of `read_feed`, `edit_case`, `check_case_mesh`, `preview_mesh`,
`import_mesh`, `list_meshes`, `refine_case_mesh`, `start_run`, `read_residuals` and `show_view`; of
[docs/service-protocol.md](../service-protocol.md) `session.create` and `mesh.preview`; of
[ADR 0732](0732-f836-proposal-a-mesh-preview-the-studio-can-draw-before-a-run.md) its decisions (searched for a time limit:
none is registered there); `apps/studio-web/src/renderer/case/MeshPreview.tsx` and `preview.ts` whole, of `case/open.ts`
`openCase`; of `src/renderer/workspace/RunWorkspace.tsx` `useSeries` and `useStore`, of `TimePanel.tsx` the series hooks, of
`src/renderer/plots/useRunFeed.ts` the head; of `crates/rcfd-workspace/src/write.rs` `edit_case`, of `facts.rs`
`read_case_text` and `refine_case_mesh`, of `feed.rs` `Change`, `journal_entry` and `run_entries`, of `runs.rs` the head;
of `crates/rcfd-api/src/case.rs` `Case::get` and `Case::set`; of `crates/rcfd-mcp/src/project.rs` the `show_view` and
`read_view` entries; of `crates/rcfd-mcp/tests/canvas_view_f976.rs` its assertions and the committed
`canvas_view_f976.mutation.patch`; the heads of `tests/support/project.ts`, `tests/support/f447.ts`, `playwright.config.ts`,
`tests/unit/project-home.spec.ts` (its fake project server) and `tests/e2e/word-budget.spec.ts`; of
`scripts/process_report.py` `sec_r`; the records of the example run `r-20261028T152641-f8c2` of
`example-projects/quick-start` (events, status, log). Bytes read: about 230 000.

## Findings of the reading (before any code)

1. **What the feed already says, and how.** Every journal line reaches every reader of the feed with its `path`, `who`,
   `actor` `{client, instance: {pid}}`, `reason` and `changes` [{`path`, `old`, `new`}] (`feed.rs` `journal_entry`). An
   `edit_case` writes one line with `path` the case and `changes` the settings set; `refine_case_mesh` writes through
   `edit_case` (`facts.rs`), so its line has `who` `edit_case`, the case's path and `changes` of `mesh.nx`, `mesh.ny`;
   `import_mesh` writes a `mesh_imported` line with `path` under `meshes/`. A run's state changes reach the feed as
   `run_state` entries (source `run`, the run id, the runner's reason; `run_entries`); a run's progress (its step and its
   residual and monitor lines) does not: the Studio reads it from `status.json` every `RUN_POLL_MS` (1000 ms) while a run
   is active (F-806, `session.ts` `poll`). The Studio keeps of each entry `who` and `actor` but not `reason` or `changes`
   (`changeEntry`). Nothing in F-979 needs a new feed entry or field: the feed's format stays as it is (a6).
2. **What the canvas of F-976 does on a change.** `session.ts` reads `read_view` only on a `view_shown` entry; the
   panels then read what they read once: `SeriesPanel` reads `runSeries` when it mounts (its effect depends on the run id
   alone), and a `mesh` panel shows the project home's row of a mesh entry, not a picture (ADR 0805 finding 6). The run
   workspace, by contrast, reads a run's series again whenever the run's state or step changes (`RunWorkspace.tsx`
   `useSeries`, its key `${state}/${step}`).
3. **A mesh changed by an MCP tool is a case's mesh.** `refine_case_mesh` and `edit_case` of a `mesh.*` setting change the
   mesh a case builds (its generator counts or its `mesh.file`); `import_mesh` adds a file that a case then names with
   `edit_case`; no tool rewrites an entry of `meshes/` (`import_mesh` never replaces a file). The mesh to draw is
   therefore the mesh of a case, as `check_case_mesh` and `preview_mesh` take it.
4. **The Studio already draws a case's mesh.** `MeshPreview` (F-836, ADR 0732) draws the mesh of a service session: the
   engine builds it (`mesh.preview`, the solver's own mesh code) and writes a mesh-only field store under
   `<project>/.rcfd/previews/<mesh hash>/`, which the Studio reads with its store reader (bit for bit the Rust reader's,
   G3) and draws on the field canvas, one colour per patch. A session made from a case file (`session.create {path}`, as
   `case/open.ts` makes one) reads the file as it is at that moment. `.rcfd/` is not watched, so the preview's store makes
   no feed entry. `MeshPreview` builds at once on its first mount and 500 ms after a change of its `settingsEpoch`.
5. **A setting's value comes from the engine.** `Case::get` gives the stored value of a dotted path, a default included,
   and an error for a path that is not a setting (`rcfd-api` `case.rs`); `read_case_text` reads a project case as `rcfd
   run` reads it. No project tool gives one setting's value today: `edit_case` with `dry_run` gives `values` only of the
   settings it sets. An addition to `read_view`'s reply (a new field, absent for views without a setting panel) gives the
   canvas each shown setting's value through the read-only server the Studio already asks, with no new tool, so the tool
   list and its pins (ADR 0805 addendum 1, L1 to L3) stay unchanged.
6. **The monitored value of a run that is not yet started.** `show_view` checks a `monitor` panel's name against the
   run's series, so a panel can only name a run that exists and has written that monitor. For a value the runner changes
   while Claude watches, the panel has to follow the run of a case that is started after the view: the newest run of the
   case. A monitor's name is a report name of the case (`reports.<name>`; `residuals.jsonl` writes `monitors.<name>` for
   each, as `r-20261028T152641-f8c2` shows for `lid_drag` of `cases/cavity-re100.toml`).
7. **The length of a short run.** `r-20261028T152641-f8c2` ran 121 steps of 1024 cells in 0.440 s of wall time (its
   checkpoint event, `wall_s` 0.4401226): 3.6 ms a step, 3.6 microseconds a cell and step. After a refinement by 2 the
   case has 4096 cells; at the same cost per cell a step takes about 14.5 ms, and 300 steps about 4.4 s, over four reads
   of the run's status at `RUN_POLL_MS`.
8. **The tool description of `show_view`** in `crates/rcfd-mcp/src/project.rs` lists the seven kinds of F-976 and their
   keys. Changing it changes an existing entry of the MCP tool list, which rule 3 and G4 leave to the maintainer (not the
   additive class). It is left as it is (question Q1).

## Design

### D1. The view specification gains three forms (additive, `rcfd-view` version 1)

`crates/rcfd-workspace/src/view.rs`. A specification that `show_view` accepted before is accepted now with the same
meaning; three forms it refused before are now accepted. The version stays 1.

| form | keys | what is checked | what the canvas shows |
|---|---|---|---|
| `mesh` with `case` (new) | `kind`, `title`?, `case` | exactly one of `mesh` and `case`; the case as F-976's `case` panel checks it (a `.toml` or `.json` file under `cases/` that exists) and it reads as a case (`read_case_text`) | the picture of the case's mesh, drawn by `MeshPreview` (D3) |
| `setting` (new kind) | `kind`, `title`?, `case`, `setting` | the case as above; `setting` a string that `Case::get` reads in that case | the setting's value as `read_view` gives it (D2) |
| `monitor` with `case` (new) | `kind`, `title`?, `case`, `monitor` | exactly one of `run` and `case`; the case as above; `monitor` a report of the case (`reports.<monitor>` reads) | the monitor's line of the newest run of the case (D3), followed as the runner writes it |

Errors stay located (`panels[i].case` for both keys given, with the reason "names its mesh or its case, not both" or
"names its run or its case, not both"; `panels[i].setting`, "is not a setting of the case", with the engine's message).
The kind `setting` is listed in the reason of an unknown kind after the seven of F-976. `KINDS` and every check of F-976
are unchanged; the text of `extra_keys` (the hunk the committed mutation patch of ADR 0805 changes) is unchanged.

### D2. `read_view` gives each shown setting's value

When the shown view has at least one `setting` panel, `read_view`'s reply gains `settings`: one object per such panel in
panel order, `{panel, case, setting, value}` with `value` what `Case::get` reads in the case file at the call, or
`{panel, case, setting, error}` when the file does not read or the path is no longer a setting. A view without a
`setting` panel gets the reply of F-976, key for key (no `settings`). `Workspace::read_view` and the tool give the same.

### D3. The Studio

Owned parts, named here:

- `src/shared/project.ts`: `ChangeEntry` gains `reason` and `changes` (optional); `CanvasKind` gains `setting`;
  `CanvasPanel` gains `setting`; `CanvasView` gains `settings` (D2's array, optional).
- `src/main/project/session.ts`: `changeEntry` keeps the entry's `reason` and `changes`; `readCanvas` keeps
  `settings`; `poll`: when a fresh entry's `path` is the case of a shown `setting` panel, `read_view` is read again
  before the change is put in the view (as for `view_shown`), so the new value and its change arrive together. The rule
  is a function of the file (`canvasReadsAgain`), tested by U1.
- `src/renderer/canvas/model.ts` (new, pure): `followedRun` (the newest run of a case among the listed runs, by `created`,
  then by id; paths compared with `/` for `\`), `changeOf` (the change a panel shows: for a `setting` panel the newest
  entry whose path is its case and whose `changes` name its setting, or that has no `changes`; for a `mesh` panel with a
  case the newest entry whose path is its case and whose `changes` name a `mesh.*` setting or that has no `changes`, or
  whose path is under `meshes/`; for a `monitor` or `plot` panel the newest entry of its run), and `actorOf` (the
  entry's actor client and pid, `runner` for an entry of source `run`).
- `src/renderer/canvas/Canvas.tsx`:
  - a `mesh` panel with `case`: a service session made from the case file (`session.create {path}`, out of the undo
    history, as `openCase` makes one) for each change `changeOf` names (and once at mount), and `MeshPreview` mounted
    for that session (keyed by it, so it builds at once); the session before it is closed when the new one is made, and
    the last at unmount. The picture, its counts, patches and failure are `MeshPreview`'s own (no drawing of the canvas's).
  - a `setting` panel: the setting's path and the value of `canvas.settings` for the panel, as JSON text.
  - a `monitor` (and `plot`) panel reads the run's series again whenever the run's state or step changes (the key of
    `useSeries`), and a `monitor` panel with `case` follows `followedRun`; the F-976 panel with `run` keeps its run.
  - each of these panels shows the change `changeOf` names, in one line: the command (`who`), the actor (client and
    pid, or `runner`), and the reason when the entry has one (for a setting, the old and new value of its `changes`).
  - Test hooks: the mesh panel's wrapper `data-mesh-build` (the feed number of the change it was built for, 0 at
    mount); the setting panel `data-setting` and `data-value`; the monitor panel `data-run`, `data-state`, `data-last-step`
    and `data-last-value` (the last point of the monitor's series as the engine sent it, `String(y)`); the change line
    `data-n`, `data-who`, `data-actor`, `data-pid`, `data-reason`, `data-old`, `data-new`.
- `src/renderer/canvas/canvas.css`: the change line in the Studio's tokens only.

The canvas still computes nothing (G16, G23): the picture is the engine's mesh store, the value the engine's `Case::get`,
the series the engine's `read_residuals`; the canvas chooses which run and which entry, and shows them.

### D4. The limits

Every change of a1 to a3 is reported by the feed (finding 1: the `edit_case` line for a1 and a2, the `run_state` entry
of the run's end for a3). The limit of each is **1.6 s** from the moment the MCP client has read the tool's result (for
a3: the result of the `wait_run` that reports the run's end) to the moment the shown view holds the change: 4 periods of
the Studio's feed poll (`FEED_POLL_MS` 400 ms), the limit of ADR 0188 C3-St ("within 4 Studio periods the editor holds
the new value and a message names `mcp`, the setting, the old and new values and the reason") at the Electron Studio's
period, as ADR 0805 D5 took it. The binary check takes ADR 0188 C3's limit, 4 polls of 250 ms.

## Checks (rule 14)

Release profile. New test files: unit tests in `crates/rcfd-workspace/src/view.rs`, `crates/rcfd-mcp/tests/canvas_live_f979.rs`
(the binary), `apps/studio-web/tests/unit/canvas-f979.spec.ts`, `apps/studio-web/tests/e2e/canvas-f979.spec.ts`. The e2e
view, on a copy of `example-projects/quick-start`: panels 0 `mesh` {`case` `cases/cavity-re100.toml`}, 1 `setting`
{that case, `physics.nu`}, 2 `monitor` {that case, `lid_drag`}, 3 `setting` {`cases/cavity-re400.toml`, `physics.nu`}, 4
`monitor` {`run` `r-20261028T152641-f8c2`, `lid_drag`}; two columns.

| Id | Clause | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| V3 | a1-a3 | `view.rs` unit test: a view of the three new forms (D1) with F-976's seven kinds, on the quick-start example | accepted, no error | F-979 acceptance; D1 | accepted | exact | code verification |
| V4 | a1-a3 | `view.rs` unit test: 8 invalid views: a mesh panel with `mesh` and `case`; a mesh panel with case `notes/README.md`; a setting panel without `setting`; setting `physics.nope`; a setting panel with a key `run`; a monitor panel with `run` and `case`; a monitor panel with case and monitor `drag`; a setting panel of `cases/nope.toml` | each refused with one error at the registered key (`panels[i].case`, `.case`, `.setting`, `.setting`, `.run`, `.case`, `.monitor`, `.case`) and a reason with the registered word; 8 of 8 | F-976 acceptance ("validated with located errors"); D1 | 8 of 8 | exact | code verification |
| B1 | a2 | binary: an `rcfd-mcp --project --canvas` process and a client process; `show_view` of a view with setting panels of re100 and re400 `physics.nu`, a mesh panel with case and a monitor panel with case | shown; the canvas's `read_view` `settings` is `[{panel 0, cases/cavity-re100.toml, physics.nu, 0.01}, {panel 1, cases/cavity-re400.toml, physics.nu, 0.0025}]` | D2; the case files' values | as stated | exact | code verification |
| B2 | a2, a4 | binary: the client's `edit_case` of re100 `physics.nu` to 0.005 with a reason | the canvas's `read_feed` holds its entry (path the case, `actor.client` `mcp`, `actor.instance.pid` the client's pid, the reason, `changes` [{`physics.nu`, 0.01, 0.005}]) within 4 polls of 250 ms; then its `read_view` gives 0.005 for panel 0 and 0.0025 for panel 1 | ADR 0188 C3 (4 polls of 250 ms) | 1 poll | 4 | code verification |
| B3 | a2 | binary: `show_view` of a setting panel with `physics.nope` | 1003 at `panels[0].setting`; `.rcfd/views/current.json` and the journal byte-identical after it | F-976 acceptance ("refused with the key and the reason and the shown view is unchanged") | as stated | exact | code verification |
| B4 | a6 | binary: a view without a setting panel (the six-kind view of `canvas_view_f976`) | `read_view`'s keys are exactly `exists`, `path`, `id`, `hash`, `shown`, `actor`, `reason`, `view` | rule 3 (defaults bit-identical); G4 | as stated | exact | unchanged default output |
| U1 | a2 | Studio unit: `ProjectSession` on a fake server with a shown view holding a setting panel of `cases/a.toml` | an entry with path `cases/a.toml` makes one more `read_view` call and the view's `canvas.settings` is its answer; an entry with path `cases/b.toml` makes none; the view's change entries keep the feed's `reason` and `changes` | D3 | as stated | exact | code verification |
| U2 | a3, a4 | Studio unit: `followedRun`, `changeOf`, `actorOf` on given lists | the newest run of the case (backslash paths, two runs of one `created` ordered by id, none when the case has none); a setting panel passes over an entry that changes another setting of its case; a mesh panel takes a `mesh.nx` change, an entry without `changes` and a `meshes/` entry, and passes over a `physics.nu` change; a run entry's actor is `runner` | D3 | as stated | exact | code verification |
| E0 | a5 | e2e: the packaged Studio has the copy open; the test, as MCP client, spawns `rcfd-mcp --project` and calls `show_view` of the e2e view | the canvas shows its 5 panels: the mesh drawn (`data-drawn` true) with 1,024 cells; 0.01 and 0.0025; panel 2 following `r-20261028T152641-f8c2` and panels 2 and 4 with the last `lid_drag` point equal to the client's `read_residuals` of that run; the time is recorded | F-979 acceptance; G3 (equal to the engine's value) | as stated | exact | code verification |
| E1 | a1, a4, a5 | e2e: `refine_case_mesh` of re100 with a reason | within 1.6 s of the tool's result: the mesh panel's `data-mesh-hash` differs from E0's, its counts say 4,096 cells (the tool's `cells_after`), `data-drawn` true; its change line `edit_case`, actor `mcp` with the client's pid, the reason | ADR 0188 C3-St (4 Studio periods) at `FEED_POLL_MS` 400 ms | 0.9 s | 1.8 | code verification |
| E2 | a2, a4, a5 | e2e: `edit_case` of re100 `physics.nu` to 0.005 with a reason | within 1.6 s: panel 1 shows 0.005, its change line `edit_case`, `mcp`, the client's pid, the reason, old 0.01 and new 0.005; controls at that moment: panel 3 still 0.0025 with no change line, the mesh panel's `data-mesh-build` and change line still E1's | ADR 0188 C3-St, as E1 | 0.6 s | 2.7 | code verification |
| E3a | a3, a5 | e2e: `start_run` of re100 with `max_steps` 300 and a reason | within 1.6 s of the tool's result panel 2's `data-run` is the new run | ADR 0188 C3-St (the `run_submitted` line), as E1 | 0.6 s | 2.7 | code verification |
| E3b | a3 | e2e: while the run runs (until the client reads its end from `wait_run`) | panel 2 shows at least one `data-last-step` above 0 and below the run's final step | F-979 acceptance ("updated in the shown view"); finding 7 | 3 | 3 | code verification |
| E3c | a3, a4, a5 | e2e: from the client's reading of the run's end (`wait_run` terminal) | within 1.6 s panel 2 shows the run's final state, its last step and its last `lid_drag` value equal (as text) to the last point of the client's `read_residuals`; its change line names the actor `runner` and the reason of the run's last `run_state` entry in the client's `read_feed`; control: panel 4 unchanged since E0 | ADR 0188 C3-St (the `run_state` entry), as E1 | 0.8 s | 2 | code verification |
| L1 | a6 | unedited existing tests that pin what F-979 must not change: `canvas_view_f976`, `feed_f273`, `project_list_f270`, `tool_list_f275`, `project_w48f`, `project_runs_w48f`, `default_f1`; `rcfd-workspace --lib` (ADR 0805 V1, V2); `rcfd-mcp --lib` | pass | rule 3; G4; ADR 0805 | pass | exact | unchanged default output |
| L2 | a5, a6 | `npm run typecheck` and `npm test` in `apps/studio-web` (every unit and e2e spec, `canvas-f976.spec.ts` unedited among them, against the branch's engine) | pass | rule 7; G13 | pass | exact | unchanged default output |
| L3 | - | `cargo fmt -p rcfd-workspace -p rcfd-mcp --check`; `cargo clippy -p rcfd-workspace -p rcfd-mcp --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check` | clean; pass | rule 7; rule 9 | pass | exact | code verification |

No shared solver file is touched, so `baseline_hashes` is not run. No tool, CLI flag or golden changes.
`python scripts/process_report.py --check` is run last and its result reported (rule 7).

### Negative controls and mutations (rule 16)

The controls are inside the rows: B2 and E2 (the other case's setting unchanged), E2 (the mesh not rebuilt for a
`physics.nu` change), E3c (panel 4 of a fixed run unchanged), U1 (an entry of another case reads nothing). Mutations run
on a scratch copy of the final commit, each restored afterwards:

| Id | Mutation | Must fail |
|---|---|---|
| M1 | the check that `setting` is a setting of the case removed from `view.rs` | B3 (and V4) |
| M2 | `session.ts` does not read `read_view` again on an entry of a setting panel's case | U1 |
| M3 | the monitor panel's series read only when the run changes, not on its state or step | E3c |
| M4 | the mesh panel's session made once at mount, not again on a change | E1 |
| M5 | `followedRun` takes the oldest run of the case | U2 |

M1 is committed as `crates/rcfd-mcp/tests/canvas_live_f979.mutation.patch`, first line
`# expect-fail: cargo test --release -p rcfd-mcp --test canvas_live_f979`. M2 and M5 are run with the Studio's unit
project only; M3 and M4 with a build and package of the scratch copy and `canvas-f979.spec.ts` alone.

## Size

Size (agent): done; 1; 2; 1; 260000

(Expected outcome done; one further registration, an amendment, expected; two heavy runs on the shared machine, the whole
`npm test` of the Studio and the two packaged builds of M3 and M4; one question for the maintainer, Q1, beside Q2 of
scope; about 260 000 bytes read.)

## Decision rule

**done** if V3, V4, B1 to B4, U1, U2, E0 to E3c and L1 to L3 hold and M1 to M5 are each caught; **partial** otherwise,
naming the check that missed. A missed limit is recorded with its number and held as a `*_known_miss` record (rule 2).

## Not covered

- The F-976 `mesh` panel with `mesh` (a file of `meshes/`) keeps its row; a picture of a mesh file that no case names
  would need a case to build it and is not drawn.
- A `plot` panel and a `field` panel that follow a case's newest run, and a field panel that follows new snapshots of a
  live run on the canvas (the run workspace's `run` panel already follows its run): Q2.
- `import_mesh` is not driven by the e2e test: a mesh it imports is drawn once a case names it, by the same path as E1
  (an `edit_case` of the case's `mesh.*`); a `meshes/` entry triggers a rebuild by `changeOf` (U2 checks the rule).
- Two Studios, Linux and macOS, as in ADR 0805.

## Open questions for the maintainer

- **Q1. The description of `show_view`.** Its text in `crates/rcfd-mcp/src/project.rs` lists the seven kinds of F-976;
  the three forms of D1 are not in it, so a client learns of them from `docs/mcp-tools.md` or a refusal's reason, not
  from the tool list. (a) the lead, with the maintainer's word, changes the description and re-records the tool list
  (an entry changed, not the additive class); (b) leave the description, and name the forms in `docs/mcp-tools.md` only.
  Recommendation: (a), since the description is what Claude reads. Not taken here (rule 3, G4).
- **Q2. More panels that follow (scope).** (a) none beyond D1; (b) a `plot` and a `field` panel with `case` that follow
  the newest run, and a field panel that follows a live run's newest snapshot, as an item after F-984. Recommendation:
  (b), entered as an item of its own.

## Amendment 1 (2026-11-26): the e2e set-up of E1 corrected

Written after the registered run of `npm test` and before any corrected code or run; nothing above is changed.

**What the run gave.** In the registered `npm test` (900 passed, 1 failed, 2 did not run, 15.0 min), E0 of
`tests/e2e/canvas-f979.spec.ts` passed (the view shown with its mesh drawn 1188 ms after the tool's result, recorded),
and E1 failed before it measured anything: the test called `refine_case_mesh` without `factor`, which the tool's schema
requires (error -32602 "missing required argument 'factor'"). E2 and E3 did not run, as the file is serial. The defect is
the test's set-up, not the canvas: no row of E1 to E3 was measured. L2 stays as it was run, a miss, and is reported so in
addendum 1. Before this amendment the schemas of every tool the e2e calls were read from the branch's binary
(`tools/list`): `factor` is the only required argument the test left out.

**The corrected test.** A new spec `apps/studio-web/tests/e2e/canvas-f979-a1.spec.ts`, the same as the first but for
`factor: 2` in the `refine_case_mesh` call (the factor `refine_case_mesh` takes, docs/mcp-tools.md). Its rows are E0 to
E3c of the table above with their limits, sources and estimates unchanged. The first spec stays as the record of the
defective set-up (rule 2): its tests are skipped with a line naming this amendment, and nothing else of it changes.

**The corrected run.** Once: `npm run build`, `npm run package:dir` and `npx playwright test tests/e2e/canvas-f979-a1.spec.ts`
in `apps/studio-web`, against the branch's engine, with nothing else of the Studio's suite run again. M3 and M4 are run
against the corrected spec. The decision rule is unchanged.

## Addendum 1: results (2026-11-26)

Written after the runs; nothing above is changed. Release profile, `CARGO_TARGET_DIR` in the scratch folder. Commits:
787c6603 (this registration), 866bb54c (engine), 77736d44 (Studio and tests), f7082fee (amendment 1), 67947cd8 (the
corrected spec), then the committed mutation, this addendum and the proposal. Each registered run was made once, except
as amendment 1 says. Deviations: a `npm run typecheck` was run once during the writing of the Studio code, before its
tests existed (it passed); `cargo fmt -p rcfd-workspace -p rcfd-mcp` (a write) was run before the code commit and the fmt
check after it.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| V3 | the live forms accepted | accepted (`rcfd-workspace --lib`: 6 passed, 0 failed) | yes |
| V4 | 8 of 8 refused at the registered key | 8 of 8 | yes |
| B1 | `settings` 0.01 and 0.0025 for panels 0 and 1 | as registered | yes |
| B2 | the edit in the canvas's feed within 4 polls of 250 ms, with actor, pid, reason, changes; then 0.005 and 0.0025 | 1 poll (6 ms); `mcp`, the client's pid, the reason, [{`physics.nu`, 0.01, 0.005}]; 0.005 and 0.0025 | yes |
| B3 | 1003 at `panels[0].setting`; file and journal unchanged | 1003, "'physics.nope' is not a setting of the case ..."; both byte-identical | yes |
| B4 | `read_view`'s keys those of F-976 | `actor, exists, hash, id, path, reason, shown, view` | yes |
| U1 | one more `read_view` on an entry of the case, none on another; reason and changes kept | as registered | yes |
| U2 | the rules of D3 | as registered | yes |
| E0 | the view as the engine gives it | registered run: shown, mesh drawn 1188 ms after the result (recorded); corrected run: 1636 ms | yes |
| E1 | redrawn within 1.6 s with its actor and reason | registered run: not measured (set-up defect, amendment 1); corrected run: 802 ms, 4,096 cells, `edit_case`, `mcp`, the pid, the reason | yes (corrected run) |
| E2 | 0.005 within 1.6 s with its change; controls unchanged | registered run: did not run; corrected run: 342 ms; old 0.01, new 0.005; re400 0.0025 with no change line; mesh build 2 and its reason unchanged | yes (corrected run) |
| E3a | the new run followed within 1.6 s | corrected run: 296 ms | yes (corrected run) |
| E3b | at least 1 step shown between 0 and the final step | 3 (16, 107, 221 of 300) | yes (corrected run) |
| E3c | the end within 1.6 s, values equal, actor `runner`, the reason; the fixed run unchanged | 306 ms; finished at step 300, `lid_drag` -0.0016167388960555745 equal; `runner`, "step_limit: 300 steps taken"; the fixed run -0.004911394168425979 before and after | yes (corrected run) |
| L1 | the named files and `--lib` pass unedited | `canvas_view_f976` 4, `feed_f273` 3, `project_list_f270` 1, `tool_list_f275` 8, `project_w48f` 10, `project_runs_w48f` 3, `default_f1` 2 passed; `rcfd-mcp --lib` 18 passed (1 ignored) | yes |
| L2 | `npm run typecheck` and `npm test` pass | typecheck exit 0; `npm test` 900 passed, 1 failed (E1 of the first spec, the set-up defect), 2 did not run (15.0 min); `canvas-f976.spec.ts` passed (E1 508 ms) | **no** |
| L3 | fmt, clippy, `docs_site`, `roadmap_proposals check` clean | fmt and clippy clean; `docs_site` and the proposal check: below | see below |

L2's miss has a diagnosed cause, the test's own set-up (amendment 1), and no other test of the Studio failed; the
corrected spec passed alone, but the whole `npm test` with it has not been run on this branch. Held as the run gave it.

### Mutations

| Id | Caught by |
|---|---|
| M1 (no check that `setting` is a setting; committed as `canvas_live_f979.mutation.patch`) | B3 (`canvas_live_f979`, line 145: `physics.nope` was shown) and V4 |
| M2 (no `read_view` on a setting panel's case) | U1 ("the entry of the setting panel's case did not read read_view") |
| M3 (series read only when the run changes) | E3 (E3c: the end not shown in 30 s) |
| M4 (the mesh session made once) | E1 (the refined mesh not drawn in 30 s) |
| M5 (`followedRun` takes the oldest) | U2 (r-1 for r-3) |

M3 and M4 were run twice on the scratch copy: the first runs did not decide (the copy's build failed: for M3 the edit left
a variable unused, which the typecheck refuses; for M4 the packager could not rename Electron through a linked
`node_modules`); after a compiling edit and the copy's own `npm ci`, each ran once and was caught.

Not run: `baseline_hashes` (no shared solver file). Found in the runs: the e2e's `start_run` leaves the temporary
project's `rcfd-runner` alive after the spec ends (twice, ended by hand); the spec's `afterAll` ends the MCP servers only.

Actual size: partial; 1 further registration (amendment 1); 3 heavy runs (`npm test` 15.0 min; the corrected spec with
its build; four packaged builds of the mutation copy); 2 questions; about 250 000 bytes read.

Decision by the registered rule: **partial**, because L2 missed (every behaviour row holds in the corrected run, and
every mutation was caught).

### For the lead: roadmap entries

> Status: partial.

> Note: ADR 0807, 2026-11-26, partial: the canvas follows live a case's mesh (drawn by MeshPreview, again 802 ms after refine_case_mesh), a setting (read_view settings, 342 ms after edit_case) and the monitor of a case's newest run (its end 306 ms after the client read it), each with its actor and reason (limits 1.6 s, ADR 0188 C3-St); 5 of 5 mutations caught, one committed; L2 missed: the whole npm test ran with the first e2e set-up, which left out refine_case_mesh's factor (amendment 1).

> Gap: Every behaviour row holds in the corrected e2e (amendment 1). Missing: a whole npm test that passes with it; the registered run had 1 failure, the first spec's set-up (refine_case_mesh without factor), now a skipped record. Also the show_view description still lists only F-976's kinds (Q1), and the e2e leaves the temporary project's runner alive after it ends.

> Follow-ups: none.

The same entries are `docs/roadmap/proposals/0807-F-979.json`.

## Answers of the maintainer (2026-11-26), written by the lead

1. show_view's description: the lead changes it and re-records the tool list, as F-987, its own merge; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
2. Plot and field panels that follow a case's newest run and a live run's snapshot: an item of their own after F-984, F-986; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
