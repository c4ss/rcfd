# ADR 0805: F-976 registration: a view specification, `show_view`, and the open Studio that renders it

Status: **REGISTERED 2026-11-26, written before any code of F-976 and before any run of its checks.** Before this text no
library, server, Studio or test code of F-976 was written and no file of the repository was changed. What was done:
reading (below), searches of the source tree, and, on the unchanged tree, a release build of `rcfd`, `rcfd-mcp` and
`rcfd-runner` into the scratch target and `npm ci` in `apps/studio-web` (neither is a check). Results go in addenda at the
end; nothing above them is changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of
the lead.

It answers the brief `docs/adr/briefs/f976-canvas-view-spec.md` ([the file](briefs/f976-canvas-view-spec.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f976-canvas-view-spec` from main
482348e4, scratch `w49i-work/f976-canvas-view-spec-scratch`. The decision it comes from is
[ADR 0803](0803-the-studio-as-claudes-canvas-record.md) (the Studio as Claude's canvas, guardrail G23 of
[studio-guardrails.md](../studio-guardrails.md)).

## What is asked

Roadmap item F-976 (deps F-270, F-814, F-447), the binding acceptance, read with a script:

> Every limit not stated here is registered in the ADR before the run. A versioned JSON view specification, with its schema in the engine and validated with located errors, names what to show (case, run, field, mesh, plot, monitor, table), of which case or run, and its layout. The project-mode MCP tool show_view, and a service method of the same, sends it to the open Studio of the project, which renders it with its existing views in its theme within a registered time of the call. An invalid specification is refused with the key and the reason and the shown view is unchanged; with no Studio open the tool says so. The MCP tool list golden changes by the added tools only (additive class) and every other default is unchanged. Shown by an e2e test that drives the open Studio from the rcfd-mcp binary.

Clauses: **a1** a versioned JSON specification, its schema in the engine, validated with located errors, naming the seven
kinds, the case or run, and the layout; **a2** `show_view` (project mode) and a service method of the same send it to the
open Studio, which renders it with its existing views in its theme within a registered time; **a3** an invalid
specification is refused with the key and the reason, and the shown view is unchanged; **a4** with no Studio open the tool
says so; **a5** the tool list golden changes by the added tools only, every other default unchanged; **a6** shown by an e2e
test that drives the open Studio from the `rcfd-mcp` binary.

## What was read

`AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, whole; [ADR 0803](0803-the-studio-as-claudes-canvas-record.md)
whole; `docs/studio-guardrails.md` whole (G15 and G19 retired, G23 added on 2026-11-26); of
[docs/mcp-tools.md](../mcp-tools.md) the conventions, the tool list and the project tools section whole (the design tool
sections by heading); `apps/studio-web/src/main/project/session.ts` whole; of
[ADR 0188](0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) the acceptance checks and limits
(C3, C3-St) and its results table; of [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md)
its registered checks; [ADR 0784](0784-f339-registration-mcp-tools-for-tolerance-and-sobol-studies-of-the-reduced-order-fan.md)
for the form of an additive tool-list edit (findings 3 and 4, rows G1 to G3); `crates/rcfd-mcp/src/project.rs` whole, its
`main.rs` argument parsing and help; `crates/rcfd-workspace/src/lib.rs` whole, of `queries.rs` `sample` and
`field_summary`, of `journal.rs` `append`, of `feed.rs` `journal_text`; of `crates/rcfd-runner/src/sys.rs` `pid_alive`,
`lock_held` and `held`; the heads of `crates/rcfd-mcp/tests/project_list_f270.rs` and `tool_list_f275.rs` (where the tool
counts are held); of the Studio `src/main/project/server.ts` whole, `src/shared/project.ts` (`FEED_POLL_MS`),
`src/renderer/main.tsx`, of `src/renderer/project/ProjectHome.tsx` `OpenProject`, `CasesTable`, `MeshesTable` and
`RunsTable`, of `src/renderer/workspace/RunWorkspace.tsx` and `door.ts` the heads, the props of `FieldViewport`,
`TimePanel`, `PlotFrame` and `MeshPreview`; `tests/e2e/claude-code.spec.ts` (the MCP client stand-in, first 150 lines),
`tests/support/project.ts`, `tests/support/rcfd.ts`, `playwright.config.ts`, the head of
`tests/unit/capabilities.spec.ts`; of `scripts/process_report.py` the registration checks (`sec_r`). Bytes read: about
330 000.

## Findings of the reading (before any code)

1. **The feed already carries a change from one process to another.** Every journal line reaches every reader of the
   project's feed (`read_feed`), whatever its kind: `journal_text` gives an unknown kind the text `<who> by <actor>:
   <path>`. The Studio's session polls `read_feed` every `FEED_POLL_MS` = 400 ms and reads its lists again when an entry
   comes. A journal line of a new kind is therefore the way a request reaches an open Studio, as G12 asks (the folder is
   the contract), with no new channel between processes. `.rcfd/` is not watched, so a file written there makes no
   `external` entry.
2. **The limit of the feed.** ADR 0188 registered, and met, C3: a change made through the `rcfd-mcp` binary appears in a
   second process's feed within 4 polls of 250 ms; and C3-St: the Studio shows a change within 4 of its own periods
   (2 s at 500 ms then; measured 0.322 s and 0.503 s). The Electron Studio's period is 400 ms, so 4 periods are 1.6 s.
3. **Whether a Studio is open cannot be read from the folder today.** The Studio writes nothing to the folder (G12;
   `session.ts`), and `tests/e2e/first-path.spec.ts` line 180 lists every file of a project the Studio made and opened,
   `project.spec.ts` lines 104 to 135 hold the whole folder byte for byte while the Studio has it open. A presence file in
   the project folder would change both existing tests. Process liveness is read with `rcfd_runner::sys::pid_alive`
   (std only, `tasklist` or `ps` with the executable's name), which `rcfd-workspace` can call (it depends on
   `rcfd-runner`).
4. **The tool lists are held by tests with counts in their source** (ADR 0784 finding 4): `project_list_f270.rs` takes the
   project list apart from its end and holds counts, `tool_list_f275.rs` holds the project list's count, `default_f1.rs`
   the list without a project. Two new project tools make `project_list_f270` and `tool_list_f275` fail on this branch on
   their count and order assertions until the lead's re-recording; `default_f1` is not touched (no session tool is
   added). The Studio's capability inventory (`apps/studio-web/capabilities/inventory.json`, `coverage.json`, the G15
   coverage test of F-824 that ADR 0803 retires in a later item) lists every MCP tool, so `tests/unit/capabilities.spec.ts`
   fails on this branch by the two new tools' rows until the lead adds them, as at the merge of F-339.
5. **"A service method".** The service of `rcfd-service/1` (`rcfd serve`, crate `rcfd-api`) has no project: it is below
   `rcfd-workspace` in the crate graph, so a method of it cannot call the workspace without a cycle (rule 5). The project's
   commands and queries are the public methods of `rcfd_workspace::Workspace`, which the MCP adapter wraps and the
   Studio's servers run (ADR 0173 W2). The service method of F-976 is therefore registered as the workspace's public
   method `Workspace::show_view` (and `read_view`), taken on the maintainer's standing instruction of 2026-11-11 (take
   the recommendation); whether a method of `rcfd-service/1` is wanted as well is question Q1.
6. **The existing views that fit each kind, as the renderer has them.** Project runs: `RunWorkspace` (the run's field,
   its time panel with residual and monitor lines), `FieldViewport` (one stored field of a run's store at a snapshot),
   `PlotFrame` (the residual and monitor lines as `TimePanel` draws them, from `read_residuals` through the run door),
   the project home's `RunsTable`, `CasesTable` and `MeshesTable`. `MeshPreview` draws a mesh only from a service session
   (the case editor's), not from a project mesh, so a `mesh` panel shows the project home's mesh entry (MeshesTable), not
   a picture; a mesh picture is for F-979 (the live view of a mesh) and is listed under "Not covered".

## Design

### D1. The view specification `rcfd-view` version 1 (the schema in the engine)

A JSON object, checked in `crates/rcfd-workspace/src/view.rs` (new), with no key beyond those below at any level:

- `version`: required, the integer 1.
- `title`: optional string, 1 to 200 characters, no control character.
- `layout`: optional object `{columns}`, `columns` an integer 1 to 4; default the smaller of 2 and the number of panels.
  Panels fill the grid in order, row by row.
- `panels`: required array of 1 to 8 panels. A panel is an object with `kind` and, optionally, `title` (as above), and
  by kind:

| kind | keys | what is checked | rendered by |
|---|---|---|---|
| `case` | `case` | a project-relative file under `cases/`, `.toml` or `.json`, that exists | the project home's `CasesTable`, the one row |
| `run` | `run` | a run id whose `runs/<id>/run.json` exists | `RunWorkspace` with that run selected |
| `field` | `run`, `field`, `snapshot`? | the run exists, has a field store with at least one snapshot; `field` is stored there; `snapshot` (default the last) an index below the snapshot count | `FieldViewport` on `runs/<id>/fields` |
| `mesh` | `mesh` | a path under `meshes/` that `list_meshes` lists as an entry | the project home's `MeshesTable`, the one row |
| `plot` | `run` | the run exists | `PlotFrame` with the run's residual lines (`residuals.*` of `read_residuals`), as the time panel draws them |
| `monitor` | `run`, `monitor` | the run exists and `monitors.<monitor>` is one of its series | `PlotFrame` with that monitor's line |
| `table` | `runs` | 1 to 10 distinct run ids, each exists | the project home's `RunsTable`, those rows |

Errors are located: each is `{key, reason}` where `key` is the dotted path of the value (`version`, `layout.columns`,
`panels[2].kind`, `panels[0].snapshot`, `panels[1].colour` for a key that is not allowed). Every error is collected (at
most 20 listed, `errors_total` all of them); the call fails with code 1003 (the workspace's `VALIDATION`), the message
`<key>: <reason>` of the first, and `data` `{errors, errors_total}`.

### D2. The tools and the workspace methods

Two project tools, listed last, after `empty_trash` (so the lead's restatement of `project_list_f270.rs` takes them off
the end), and two public methods of `Workspace` with the same names, which `Workspace::call` answers:

- `show_view {view, reason?}`, access `write_files`: (1) validate (D1); refused, nothing is written. (2) Count the live
  canvases of the project (D3); none: error 1002 "no Studio has this project open ...", nothing written. (3) Write
  `.rcfd/views/current.json` by `atomic_write`, `{format: "rcfd-view-shown", version: 1, shown, actor?, reason?, view}`,
  and append one journal line of kind `view_shown` with `path`, `hash` (`fnv64:` of the file's bytes), `panels` (the
  kinds in order) and `studios`. Result: `{shown: true, id, path, hash, studios, panels}`, where `id` is the hash (the
  file holds the time, so every call has its own).
- `read_view {}`, access `read`: `{exists, path, id, hash, shown, actor, reason, view}` of the current file, or
  `{exists: false, path, view: null}`.

The specification is what the MCP client gave, byte for byte as JSON value; the file and the journal are the record
(G16, G17).

### D3. Which Studios are open: the canvas registry

`rcfd-mcp --project <dir> --canvas` (new, opt-in; refused without `--project`, exit 2) registers its process as a canvas
of the project for as long as it runs: it writes `<temp>/rcfd-canvas/<16 hex of fnv64 of the canonical project root>/<pid>.json`
(`std::env::temp_dir()`; `{pid, root, started}`) at start and removes it when it ends normally. `show_view` counts the
files of that folder whose `pid` is alive with the executable name `rcfd-mcp` (`pid_alive`) and removes the others. The
folder is outside the project (finding 3): presence is a fact of this machine's processes, not project state, and the
project folder stays as the existing tests hold it. The Studio starts its reading project server (the read-only server of
`ProjectSession`) with `--canvas`; no other server of the Studio, and no client, passes it.

Two Studios open: both are counted (`studios` 2), both read the same journal line and show the same view. No Studio open:
the tool says so (D2 (2)), and nothing is written; a Studio opened later shows the last view shown (it reads
`read_view` when it opens the project).

### D4. The Studio

- `src/main/project/server.ts`: `StartOptions.canvas` adds `--canvas`. `src/main/project/bridge.ts` (or where the
  session's `startServer` is made) passes it for the session's reading server only.
- `src/main/project/session.ts`: `read_view` once when the project opens, and again when a feed entry of kind
  `view_shown` arrives; the answer is the view's new field `canvas` (`{id, shown, actor, view}` or null) with
  `canvasError`. `src/shared/project.ts`: those fields and `read_view` in `ProjectTool`.
- `src/renderer/canvas/Canvas.tsx` and `canvas.css` (new): the grid of D1 in the Studio's tokens (no colour, size or font
  of its own beyond the tokens), each panel rendered by the view of D1's table; `ProjectHome.tsx`: `CasesTable`,
  `MeshesTable` and `RunsTable` exported, and in `OpenProject` a tab "Canvas" that is there while a view is shown and is
  selected when a view with a new `id` arrives. No panel is built by hand for the canvas; the canvas computes nothing
  (G23, G16): every value is what the existing views read from the engine.
- Test hooks: the canvas root carries `data-canvas-view="<id>"` and `data-panels`, each panel `data-canvas-kind` and
  `data-canvas-panel="<index>"`; the main process notes when a view first reached the renderer
  (`window.studioProject.state()`, the view's `canvas`).

### D5. The time limit

From the moment the MCP client has read `show_view`'s result to the moment the canvas root with that `id` is in the
document with every panel's existing view mounted: at most **1.6 s**, 4 periods of the Studio's feed poll
(`FEED_POLL_MS` 400 ms), the limit of ADR 0188 C3-St (a change shown within 4 Studio periods) at the Electron Studio's
period. Estimate: up to one period of waiting, one `read_feed`, one `read_view` and a React render, about 0.6 s.

## Checks (rule 14)

Release profile. New test files: `crates/rcfd-mcp/tests/canvas_view_f976.rs` (the binary), unit tests in
`crates/rcfd-workspace/src/view.rs`, `apps/studio-web/tests/e2e/canvas-f976.spec.ts`.

| Id | Clause | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| V1 | a1, a3 | `view.rs` unit test: 16 invalid specifications (version 2; version missing; panels missing, empty, 9 panels; layout.columns 5; a key `colour` in a panel; kind `chart`; a run that does not exist; a case outside `cases/`; a case that does not exist; a mesh not listed; a field not stored; snapshot past the last; a monitor not a series; table with a repeated run) | each refused with exactly the registered key, a reason that names the problem, code 1003; all 16 | F-976 acceptance ("validated with located errors", "refused with the key and the reason") | 16 of 16 | exact | code verification |
| V2 | a1 | `view.rs` unit test: a valid specification with one panel of each of the seven kinds and a layout, on the quick-start example | accepted, no error | F-976 acceptance | accepted | exact | code verification |
| S0 | a4 | binary: `show_view` of the valid specification with no canvas process (and with a registry file of a pid that is not alive) | error 1002 whose message says no Studio has the project open; no `.rcfd/views/`; the journal byte-identical; the dead pid's file removed | F-976 acceptance ("with no Studio open the tool says so") | as stated | exact | code verification |
| S1 | a2 | binary: one `rcfd-mcp --project --canvas` process, `show_view` from a second | `shown` true, `studios` 1; one journal line `view_shown`; the canvas process's `read_feed` holds it within 4 polls of 250 ms; its `read_view` returns the given specification equal as JSON | ADR 0188 C3 (4 polls of 250 ms); F-976 acceptance | 1 poll | 4 | code verification |
| S2 | a2 | binary: two canvas processes | `studios` 2 | F-976 acceptance ("the open Studio of the project"); D3 | 2 | exact | code verification |
| S3 | a3 | binary, one canvas: after S1, `show_view` of each of three invalid specifications of V1 | each error 1003 with `data.errors[0].key` the registered key; `.rcfd/views/current.json` and the journal byte-identical after them | F-976 acceptance ("the shown view is unchanged") | as stated | exact | code verification |
| W1 | a2 | `Workspace::show_view` and `read_view` called as Rust methods (the service method of finding 5) give the same results as the tools | equal JSON (ids aside) | F-976 acceptance ("a service method of the same") | equal | exact | code verification |
| E1 | a2, a6 | e2e: the open Studio (packaged, its engine the branch's), the test as MCP client spawning `rcfd-mcp --project`, `show_view` of a seven-kind specification | the canvas shows the view's `id` with 7 panels, each holding its kind's existing view (the row of the case, of the mesh, of each table run; the workspace heading of the run; the field viewport's canvas; a plot frame for plot and monitor), within 1.6 s of the tool's result | ADR 0188 C3-St (4 Studio periods), at `FEED_POLL_MS` 400 ms | 0.6 s | 2.7 | code verification |
| E2 | a3, a6 | e2e: then `show_view` of an invalid specification (`panels[0].kind` `chart`) | an error with that key and reason; 1.6 s later the canvas still shows the first `id` and its 7 panels | F-976 acceptance; ADR 0188 C3-St for the wait | as stated | exact | code verification |
| E3 | a2 | e2e: the canvas in the Studio's theme | the canvas root's computed `font-family`, `color` and `background-color` equal those of the project home's open view (`.ph-open`), and `canvas.css` names no colour literal (only `var(--...)` tokens) | F-976 acceptance ("in its theme"); G23 | equal | exact | code verification |
| L1 | a5 | the project tool list of the branch's binary, with `show_view` and `read_view` taken off its end | the two are last, `show_view` `write_files` required `[view]`, `read_view` `read` required `[]`, both `additionalProperties` false; on a scratch copy with the additive edit applied (a block of `project_list_f270.rs` taking the two off the end, the counts of `tool_list_f275.rs` and `project_list_f270.rs` raised by 2) `project_list_f270`, `tool_list_f275` and `default_f1` pass | rule 3 (additive class); F-976 acceptance | pass | exact | unchanged default output |
| L2 | a5 | on the branch, unedited: `default_f1`, `project_w48f`, `feed_f273`, `project_runs_w48f`, `tool_list_adr0730`; `rcfd-mcp --lib`, `rcfd-workspace --lib` | pass; `project_list_f270` and `tool_list_f275` fail on their count or order assertions only (finding 4) | rule 3; rule 7 | as stated | exact | unchanged default output |
| L3 | a5 | `npm run typecheck` and `npm test` in `apps/studio-web` (every unit and e2e spec, unedited, against the branch's engine) | pass, but `capabilities.spec.ts` failing on the rows of `mcp:show_view` and `mcp:read_view` only (finding 4) | rule 3; rule 7; G13 | as stated | exact | unchanged default output |
| L4 | a5 | `rcfd-mcp` without `--canvas` writes nothing to the registry folder; `--canvas` without `--project` exits 2 | as stated | rule 3 (defaults bit-identical) | as stated | exact | unchanged default output |

No shared solver file is touched, so `baseline_hashes` is not run. `cargo test -p rcfd-cli --release --test docs_site`
checks the links of this file and of `docs/mcp-tools.md`.

### Negative controls and mutations (rule 16)

Run on a scratch copy of the final commit, each restored afterwards:

| Id | Mutation | Must fail |
|---|---|---|
| M1 | the check of keys that are not allowed removed from `view.rs` | V1 (`panels[n].colour`) and S3 |
| M2 | the liveness check of D3 removed (every registry file counted) | S0 (the dead pid's file) |
| M3 | `show_view` writes the file before validating | S3 (file changed) |
| M4 | the session does not read the view on a `view_shown` entry (only at open) | E1 |

M1 is committed as `crates/rcfd-mcp/tests/canvas_view_f976.mutation.patch`, first line
`# expect-fail: cargo test --release -p rcfd-mcp --test canvas_view_f976`.

## Size

Size (agent): done; 1; 1; 1; 330000

(Expected outcome done; one further registration, an amendment, expected; one heavy run on the shared machine, the whole
`npm test` of the Studio; one question for the maintainer beyond Q1's scope question; about 330 000 bytes read.)

## Decision rule

**done** if V1, V2, S0 to S3, W1, E1 to E3 and L1 to L4 hold, with the failures that L2 and L3 name as the only failures
of the existing tests, and M1 to M4 are each caught; **partial** otherwise, naming the check that missed. A missed limit is
recorded with its number and held as a `*_known_miss` record (rule 2).

## Not covered

- Two Studios rendering at once is not driven end to end: S2 counts two canvas processes; each Studio renders by the same
  code that E1 checks.
- A picture of a mesh (finding 6): the `mesh` panel shows the mesh's entry; the picture is for F-979.
- The case categories and their layouts (F-977), the process view (F-978), the export as an HTML page (ADR 0803), Linux
  and macOS.
- The registry's recycled-pid shortcut of `pid_alive` (a dead Studio's pid reused by another `rcfd-mcp`) is not tested.

## Open questions for the maintainer

- **Q1. A method of `rcfd-service/1` as well?** (a) the workspace method is the service method of F-976 (finding 5) and
  nothing more; (b) also a `view.show` method of `rcfd serve`, which would need the service to open a project (a new
  dependency of the CLI's service on the workspace) and a new protocol minor version (G4). Recommendation: (a); taken here
  on the maintainer's standing instruction of 2026-11-11 (take the recommendation), and asked so that the reading is his.
- **Q2. What the canvas shows beyond F-976's seven kinds** (scope, not decided here): (a) nothing until F-977 to F-981
  name it; (b) also derived pictures (`derive_picture`: iso-surfaces, streamlines, slices) and a mesh picture as kinds of
  version 2 of the specification. Recommendation: (b), entered as items of their own after F-979, since the live mesh view
  needs the same mesh picture.

## Addendum 1: results (2026-11-26)

Written after the runs; nothing above is changed. Release profile, `CARGO_TARGET_DIR` in the scratch folder. Code
commits: c0a906c9 (engine), 0f0e9d9a (Studio and e2e), be51edb0 (docs and the committed mutation). Each registered run
was made once. Between the Rust runs and the commit of the Studio code, `cargo fmt -p rcfd-workspace -p rcfd-mcp`
changed whitespace only in `view.rs` and `canvas_view_f976.rs` (no statement changed); the fmt check was run after it.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| V1 | 16 of 16 refused at the registered key, code 1003 | 16 of 16 at their key, each with a reason naming the problem (`rcfd-workspace --lib`: 4 passed, 0 failed) | yes |
| V2 | the seven-kind view accepted | no error | yes |
| S0 | 1002 saying no Studio is open; nothing written; the dead pid's file removed | 1002 "no Studio has this project open ..."; no `.rcfd/views/`; journal byte-identical; the stale file removed | yes |
| S1 | `view_shown` in the canvas's feed within 4 polls of 250 ms; `read_view` equal | 1 poll (6 ms); `studios` 1; one journal line; `read_view` equal to the view given | yes |
| S2 | `studios` 2 | 2 | yes |
| S3 | 1003 at `panels[0].kind`, `panels[1].colour`, `panels[3].run`; file and journal unchanged | all three; both byte-identical | yes |
| W1 | the workspace methods equal the tools | `read_view` equal; `show_view` `studios` 1 and its view read back | yes |
| E1 | 7 panels with their existing views within 1.6 s | 481 ms; the Canvas tab selected | yes |
| E2 | refused at `panels[0].kind` with the reason; the first view shown 1.6 s later | 1003, `'chart' is not a kind ...`; the first id and its 7 panels | yes |
| E3 | font, ink and ground equal to `.ph-open`; no colour literal in `canvas.css` | Archivo, rgb(29, 58, 95), transparent for both; none | yes |
| L1 | on a scratch copy with the registered edit, `project_list_f270`, `tool_list_f275`, `default_f1` pass | `project_list_f270` 1 passed, `default_f1` 2 passed; `tool_list_f275` failed at line 153: its own `PROJECT_TOOLS` list also needs the two names | **no** |
| L2 | named files pass but `project_list_f270`, `tool_list_f275` on counts or order | `rcfd-mcp --lib` 18 passed (1 ignored); `default_f1` 2, `feed_f273` 3, `tool_list_adr0730` 2 passed; `project_list_f270` a2 failed at line 151 (order) and `tool_list_f275` c5l at line 129 (57 against 55) as registered; but `project_w48f` f2 (the read set gains `read_view`) and f3 (57 against 55) and `project_runs_w48f` f3_f11 and f13 (57 against 55) failed too | **no** |
| L3 | typecheck passes; `npm test` passes but `capabilities.spec.ts` on the two tools' rows | typecheck exit 0; `npm test` 866 passed, 3 failed, 28 did not run (14.7 min): `capabilities.spec.ts` on three rows, `mcp:read_view`, `mcp:show_view` and `cli:rcfd-mcp --canvas`; `data-routes.spec.ts` P12 tiny raw (`lengthUnit` null against `m`; the 28 serial tests after it did not run) and `e2e/data.spec.ts` open (the same) | **no** |
| L4 | `--canvas` alone exits 2; no registry without `--canvas` | exit 2; no registry folder | yes |

Diagnosis of the misses. L1 to L3 are one cause: finding 4 named too few of the places that pin the project tool list
and the inventory. The full additive edit for the lead: in `project_list_f270.rs` a block that takes `show_view` and
`read_view` off the end; in `tool_list_f275.rs` 55 to 57 (line 131) and the two names appended to its `PROJECT_TOOLS`;
in `project_w48f.rs` `read_view` in the read set (line 115) and `19 + 36` to `19 + 38`; in `project_runs_w48f.rs` 55 to
57 twice (lines 103 and 356); and the inventory rows of the three capabilities with their coverage entries. On the scratch
copy with the first three files edited, `project_w48f` passed (10) and `project_runs_w48f` f3_f11 stopped at `binaries()`
because that copy's target had no `rcfd` and `rcfd-runner`, so it was not decided there. The two data failures are not
of this item: no `f447_bench` was in this work's target, and the tests found the main checkout's copy of 2026-11-16, older
than ADR 0738's length unit; this item changes no data reader.

### Mutations

| Id | Caught by |
|---|---|
| M1 (unknown keys let through; committed as `canvas_view_f976.mutation.patch`) | S3 (`canvas_view_f976`, line 78: the `colour` view was shown) and V1 |
| M2 (no liveness check) | S0 (line 78: the dead pid counted, the view shown) |
| M3 (file written before validating) | S0 (line 111) and S3 (line 198) |
| M4 (no read on `view_shown`) | E1 (not shown within 20 s) |

fmt check of `rcfd-workspace` and `rcfd-mcp`: clean. `cargo clippy -p rcfd-workspace -p rcfd-mcp --all-targets -- -D
warnings`: clean. No shared solver file changed; `baseline_hashes` not run.

Actual size: partial; 0 further registrations; 1 heavy run (`npm test`, 14.7 min; the M4 Studio build and one spec on the
scratch copy besides); 2 questions; about 360 000 bytes read.

Decision by the registered rule: **partial**, because L1, L2 and L3 missed (every behaviour row and every mutation holds).

### For the lead: roadmap entries

> Status: partial.

> Note: ADR 0805, 2026-11-26, partial: the view specification rcfd-view 1 (seven kinds, layout, located errors), show_view and read_view (tools and workspace methods) and the canvas registry of rcfd-mcp --canvas; the open Studio shows a seven-kind view with its existing views 481 ms after the tool's result (limit 1.6 s), refuses an invalid one at its key and keeps the shown view; 4 of 4 mutations caught, one committed; L1 to L3 missed: the registration named too few of the tests and inventory rows that pin the tool list.

> Gap: Every behaviour row holds. Missing: the additive edit of the tool lists is wider than ADR 0805 registered, so L1 to L3 missed: besides project_list_f270 and tool_list_f275 (counts and its PROJECT_TOOLS list), project_w48f (count, read set) and project_runs_w48f (two counts) pin the list, and the capability inventory gains three rows (mcp:show_view, mcp:read_view, cli:rcfd-mcp --canvas). The lead's re-recording closes it.

> Follow-ups: none.

The same entries are `docs/roadmap/proposals/0805-F-976.json`.
