# ADR 0823: F-980 registration: a view exported as one self-contained styled HTML page

Status: **REGISTERED 2026-11-27, written before any code of F-980 and before any run of its checks.** Before this text no
library, server, Studio or test code of F-980 was written and no file of the repository was changed. What was done:
reading (below) and searches of the source tree; nothing was built or run. Results go in addenda at the end; nothing
above them is changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f980-export-view-html.md` ([the file](briefs/f980-export-view-html.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f980-export-view-html` from efce0cf1, scratch
`w49i-work/f980-export-view-html-scratch`. The decision it comes from is
[ADR 0803](0803-the-studio-as-claudes-canvas-record.md) (answer 2: the same view specification can be written as a
self-contained styled HTML page; guardrail G23). It builds on [ADR 0805](0805-f976-registration-a-view-specification-and-show-view.md)
(F-976: `rcfd-view` 1, `show_view`, `read_view`, the canvas registry),
[ADR 0807](0807-f979-registration-live-meshes-settings-and-monitors-on-the-canvas.md) (F-979: the mesh of a case, the
setting kind, the monitor of a case) and [ADR 0817](0817-f977-registration-layouts-chosen-by-the-case-category.md)
(F-977: the view of a run, the category layouts).

## What is asked

Roadmap item F-980 (deps F-976), the binding acceptance, read with a script:

> Every limit not stated here is registered in the ADR before the run. export_view writes a view specification as one self-contained HTML file (no network fetch, the Studio's theme and styling) with the data of the views it names and their provenance. The page opens offline and shows the values the Studio shows for the same specification, checked by a registered comparison over a registered set of specifications; the size of the file of a registered case is recorded.

Clauses: **a1** `export_view` writes a view specification as one self-contained HTML file; **a2** no network fetch;
**a3** the Studio's theme and styling; **a4** with the data of the views it names and their provenance; **a5** the page
opens offline and shows the values the Studio shows for the same specification, by a registered comparison over a
registered set of specifications; **a6** the size of the file of a registered case is recorded. From the brief and G4:
the engine's interfaces change only by addition; the tool list changes by the added tool only (**a7**).

## What was read

`AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, whole; [ADR 0803](0803-the-studio-as-claudes-canvas-record.md),
[ADR 0805](0805-f976-registration-a-view-specification-and-show-view.md) and
[ADR 0817](0817-f977-registration-layouts-chosen-by-the-case-category.md) whole; of
[studio-guardrails.md](../studio-guardrails.md) G3, G4, G13, G15 (retired), G16, G17 and G23; of
`crates/rcfd-workspace/src/view.rs` the constants, `view_errors`, `show_view`, `read_view`, `layout_of_run`, the
registry and the test fixtures `quick_start`, `valid` and `live`; `apps/studio-web/src/renderer/canvas/Canvas.tsx` whole;
`apps/studio-web/tests/e2e/canvas-f977.spec.ts` whole; of the Studio: `design/tokens.css` whole, the font lines of
`base.css` and `main.tsx`, the CSP of `index.html`, `field/colormap.ts` (lines 1 to 50), `surface.ts` (`shownRange`,
`rangeOf`), the data hooks of `FieldViewport.tsx`, `Legend.tsx`, `PlotFrame.tsx` (and its extent logic, lines 40 to 120),
`plots/range.ts` (lines 1 to 56, `Extent`), `RunWorkspace.tsx` (lines 346 to 530), `MeshPreview.tsx` (its hooks),
`ProjectHome.tsx` (`RunsTable`, lines 905 to 937), `canvas/model.ts` (`panelRun`, `followedRun`), `main/run/series.ts`
(`readRunSeries`), `shared/run.ts` (`SERIES_POINTS` = 800); of `crates/rcfd-workspace/src/lib.rs` `COMMANDS` and
`call`; of `queries.rs` the shapes of `list_runs` and `field_summary`; of `meshes.rs` `build_case_mesh`; of
`crates/rcfd-post/src/picture.rs` `StoreSurface`, `store_surface`, `Layers`, `FRONT_SIGN`; of `render.rs` `colormap`;
of `crates/rcfd-tsstore/src/lib.rs` the reader's public functions, `FieldInfo` and `SnapInfo`; of
`crates/rcfd-mcp/src/project.rs` the entries of `show_view` and `read_view` and `ProjectTools::call`; of the tests that
pin the project tool list: `project_list_f270.rs` (lines 100 to 160, 331, 407), `tool_list_f275.rs` (lines 50 to 175),
`project_w48f.rs` (lines 105 to 140), `project_runs_w48f.rs` (lines 103, 353 to 360), `canvas_view_f976.rs` (lines 205
to 235), `design_uq_sobol_f339.rs` (lines 790 to 862), and the lead's re-record of F-976 (`git show --stat 5eb2be45`);
[docs/process/tolerances.md](../process/tolerances.md) (the rows); `apps/studio-web/package.json` (scripts); the file
list of the example runs `3900`, `d5cb` and `0489`. Bytes read: about 280 000.

## Findings of the reading (before any code)

1. **Who writes the page.** `export_view` is an MCP tool of `rcfd-mcp`; no Studio need be open, so the page is built by
   the engine (`rcfd-workspace`), from the same records and the same engine calls the Studio's views read: the field
   store through `rcfd_tsstore::Reader` and `rcfd_post::picture::store_surface` (the surface the Studio's field view is
   handed, F-447), the residual and monitor series through `read_residuals` with the Studio's arguments (`readRunSeries`:
   the names first with `max_points` 2, then each series with `max_points` 800), runs through `list_runs`, settings
   through the function `read_view` uses (`setting_values`), the mesh of a case through `build_case_mesh`.
2. **The Studio's theme is its CSS.** `design/tokens.css` (generated from `tokens.ts`, every colour, size and font of
   DESIGN.md, with the dark and contrast themes), `base.css` (the body's font, size and ink from the tokens) and
   `canvas/canvas.css` (the canvas grid). They have no `url(`, `@import` or `@font-face`. The fonts are loaded by
   `main.tsx` from the npm package `@fontsource/archivo`, which is not in the repository, so a page built by the engine
   cannot embed them: the page names the same font stack (`--font-family`: Archivo, then Helvetica Neue, Arial,
   sans-serif) and falls back where Archivo is not installed (question Q1).
3. **What the Studio shows, by kind, and where a test reads it** (the canvas, `Canvas.tsx`): a field panel's
   `FieldViewport` carries `data-range` (`lo hi`, the colour range of the shown snapshot: `shownRange`, the least and
   greatest `f32` value over the owner cells of the surface triangles), `data-snapshot` and `data-step` (default the last
   snapshot); a plot or monitor panel's `PlotFrame` carries `data-drawn` and `data-skipped` (`Extent.add`: a sample is
   drawn when x and the plotted y are finite, y above zero on a log scale; the residual plot is log, a monitor linear),
   and a monitor panel's `.cv-series` `data-last-step` and `data-last-value`; a case or mesh entry is a row
   `tr[data-path]`; a table a row `tr[data-run]` per run with its step in the first `td.ph-num`; a run panel the
   workspace `.ws[data-run]` with the steps and the snapshots stored as the first two `dd` of `.ws-grid`; a setting
   `.cv-setting[data-value]`; a case's mesh `MeshPreview`'s `.mp[data-triangles]`; each panel's title `.cv-panel-title`.
4. **Colours.** The Studio colours a field by a 256-entry table, entry k = `rcfd_post::render::colormap(k / 255)`, and a
   value v takes entry `floor(t * 255 + 0.5)` with `t = clamp((v - lo) * f32(1 / (hi - lo)), 0, 1)` (`colormap.ts`,
   lines 10 to 13). The page uses the same rule from the same Rust function.
5. **The tool list is pinned in more places than the golden files** (ADR 0805 addendum 1, the lead's re-record
   5eb2be45): a new project tool, listed last, makes these assertions fail on the branch until the lead's additive
   re-record: `canvas_view_f976.rs` line 216 (`empty_trash` third from the end), `project_list_f270.rs` line 134 (the
   block that takes `show_view` and `read_view` off the end), `tool_list_f275.rs` line 132 (57 tools) and its
   `PROJECT_TOOLS` (line 50), `project_w48f.rs` line 137 (`19 + 38`), `project_runs_w48f.rs` lines 104 and 359 (57);
   in the Studio `tests/unit/capabilities.spec.ts`, by the missing inventory row `mcp:export_view` of
   `apps/studio-web/capabilities/inventory.json`. `project_w48f.rs`'s read set (line 116) does not change (the tool is
   `write_files`); `default_f1.rs` does not (no session tool). The JSON goldens under `crates/rcfd-mcp/tests/golden/`
   are not edited by anyone (their tests say so).
6. **Where the dispatch goes.** `Workspace::call` answers the names of `COMMANDS` (`lib.rs`, not a file of this item).
   `ProjectTools::call` of `rcfd-mcp` can answer `export_view` before it, calling a public method
   `Workspace::export_view` (the service method, as ADR 0805 finding 5), so `lib.rs` is not changed.

## Design

### D1. The tool `export_view {view?, out?, reason?}` (project mode, access `write_files`, listed last)

- `view`: a view specification of `rcfd-view` 1 in any form `show_view` takes (panels, F-979's forms, F-977's
  `{version: 1, run}`, which is exported as its category's layout, `layout_of_run`). Checked by `view_errors`; refused
  with error 1003 and every located error in `data.errors`, as `show_view`. Absent: the view shown last
  (`.rcfd/views/current.json`'s `view`); none shown: error 1202 (`NOT_FOUND`) "no view has been shown ...".
- `out`: optional, a project-relative path `exports/<name>.html`, 1 to 200 characters, one name of `A-Z a-z 0-9 . _ -`
  after `exports/`, no `..`; otherwise 1003 at `out`. Default `exports/view-<16 hex>.html`, the hex of the page's id.
- The page's id: for the shown view, `read_view`'s id (the hash of the shown file); for a given view, `fnv64:` of the
  specification's JSON text.
- Where it writes: the file at `out`, replaced if it exists, by `atomic_write`; one journal line `view_exported` with
  `path`, `hash` (`fnv64:` of the file's bytes), `bytes`, `panels` (the kinds). Nothing is written for a refusal (the
  checks come before any write). No Studio need be open.
- Result: `{exported: true, id, path, hash, bytes, panels}`.
- `rcfd-mcp/src/project.rs`: the entry after `read_view`, and `ProjectTools::call` answers it by
  `Workspace::export_view` (finding 6). No other tool, flag, description or golden changes.

### D2. The page (`crates/rcfd-workspace/src/view.rs`, additive)

One file: `<!doctype html>`, `<meta charset="utf-8">`, a Content-Security-Policy meta `default-src 'none'; style-src
'unsafe-inline'; img-src data:`, one `<style>` holding the bytes of the Studio's `design/tokens.css`, `base.css`,
`canvas/canvas.css` and the page's own `canvas/export.css` (new, under `apps/studio-web/src/renderer/`, only `var(--...)`
tokens, no colour literal), read by `include_str!` at compile time; no script, no link, no image file, no font file. The
body is the canvas's markup: `section.cv` with the view's title, `div.cv-grid[data-columns]` and one `div.cv-panel` per
panel with its `.cv-panel-title`, as `Canvas.tsx` lays them out, and in each panel:

| kind | the page shows | its hooks |
|---|---|---|
| field `{run, field, snapshot?}` | an inline SVG of the store's surface triangles that face the viewer along the thin axis of the store's `Layers` (z for a 3D store; the sign of `FRONT_SIGN`), far to near, each filled with its owner cell's colour by finding 4; the legend (the 256 entries) with lo and hi; the field, snapshot, step and time | `data-lo`, `data-hi` (least and greatest `f32` value over the owner cells of all surface triangles, as `shownRange`), `data-snapshot` (default the last), `data-step` |
| mesh `{mesh}` | the entry's path | `data-path` |
| mesh `{case}` | an inline SVG of the surface of the case's mesh (`build_case_mesh`, `store_surface`), drawn as the field's but in the viewport tint with hair edges; its cell and triangle counts | `data-triangles` (`n_triangles`) |
| plot `{run}` | an inline SVG of the residual lines (log y), each named with its last value | `data-drawn`, `data-skipped` (by `Extent.add`, finding 3) |
| monitor `{run}` or `{case}` | an inline SVG of the line (linear y); the last step and value | `data-last-step`, `data-last-value`, `data-drawn`, `data-skipped`; `data-run` (for `{case}` the run of `followedRun`: the newest `created`, then the greatest id) |
| case `{case}` | the case's path and its file's text | `data-path` |
| run `{run}` | the run's id, case, state, steps and snapshots stored | `data-run`, `data-step`, `data-snapshots` |
| table `{runs}` | one row per run: id, case, state, step | `tr[data-run]`, each `data-step` |
| setting `{case, setting}` | `path = value` | `data-setting`, `data-value` (the JSON text `read_view` gives) |

Each panel has `data-kind` and `data-sources`: the project-relative files it read (`runs/<id>/fields/store.rts`,
`runs/<id>/residuals.jsonl`, `runs/<id>/run.json`, `runs/<id>/status.json`, the case file), and lists them with their
`fnv64` hashes (G17). The page's header names the project, the time of the export, the actor, the engine version
(`rcfd-workspace` `CARGO_PKG_VERSION`), the page's id (`data-export-view`) and the specification in a `<details>`.
Numbers are written as JSON writes them (`serde_json`: the shortest text that reads back as the same `f64`). The page
computes nothing in the browser (no script); every value is the engine's, as G16 and G23 ask.

### D3. The registered set and the comparison

The **f980 project** is a copy of `example-projects/quick-start` with `meshes/box.msh` (the stub of ADR 0805 V2) and, from
`wind-tunnel`, `cases/cylinder-re100.toml` and run `r-20261028T152128-d5cb`, from `axial-fan`,
`cases/fan01-passage-upwind.toml` and run `r-20261030T223519-0489`. The registered specifications:

| Id | specification | panels |
|---|---|---|
| X1 | ADR 0805 V2's view (`valid()` of `view.rs`): case, run 3900, field p of 3900, mesh `meshes/box.msh`, plot 3900, monitor nu_hot of 3900, table [3900, 5cc1] | 7 |
| X2 | ADR 0807 V3's view (`live()`): case, field p, mesh `meshes/box.msh`, monitor nu_hot, mesh {case cavity-re100}, setting physics.nu of cavity-re100, monitor {case cavity-re100, lid_drag}, table | 8 |
| X3 | `{version: 1, run: d5cb}` (external aerodynamics: field ux, p, monitor cd, cl, plot, table) | 6 |
| X4 | `{version: 1, run: 3900}` (heat transfer: field T, ux, monitor q_hot, nu_hot, plot, table) | 6 |
| X5 | `{version: 1, run: 0489}` (the fan, a 3D store: mesh {case}, field ux, p, monitor m_in, m_out, p_in, plot) | 7 |

The comparison: for each panel, the page's value against the open Studio's value of the same panel of the same
specification (finding 3): kind and title; field lo, hi, snapshot, step; mesh triangles; plot drawn and skipped; monitor
last step and last value; case and mesh path; run id, steps and snapshots stored; table run ids in order and steps;
setting value. Limit: a number agrees to a relative difference of at most **1e-12** (the
[tolerance table](../process/tolerances.md), the row "Same discrete problem by two routes", where only rounding
separates the two); counts, ids, paths and text exactly. Estimate: equal (the same records read by the same engine
functions, written as the shortest round-trip text).

## Checks (rule 14)

Release profile. New tests: unit tests in `crates/rcfd-workspace/src/view.rs`, `crates/rcfd-mcp/tests/export_view_f980.rs`
(the binary), `apps/studio-web/tests/e2e/export-view-f980.spec.ts`.

| Id | Clause | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| R1 | a1 | binary, the f980 project: `export_view {view: Xn}` for X1 to X5 | each `{exported: true, id, path, hash, bytes, panels}`; the file at `path` under `exports/` has `bytes` bytes and that `fnv64`, starts `<!doctype html>`, has one `.cv-panel` per panel of the view (X3 to X5: of the layout); 5 of 5 | F-980 acceptance; D1 | 5 of 5 | exact | code verification |
| R2 | a4, a5 | binary: the hooks of the pages of X1 to X4 against the engine's tools | field lo and hi equal `field_summary`'s min and max of that snapshot as `f32` (the 2D stores of X1 to X4, where every cell owns surface triangles), snapshot the last, step that snapshot's; monitor last step and value the last sample of `read_residuals {names: [monitors.<m>], max_points: 800}`; plot drawn and skipped by the rule of finding 3 over `read_residuals` of each `residuals.*` (800); run and table steps `list_runs`'s; the setting value `read_view`'s `settings` after `show_view` of X2; every panel | D3 (1e-12; exact) | equal | exact | code verification |
| R3 | a2, a3 | unit (`view.rs`) on X1, and binary on the five files: the page's text | no `<script`, `<link`, `<iframe`, `<object`, `<embed`, `<img`, `@import`, `http:`, `https:`, `src=` or `href=`; every `url(` followed by `#` or `data:`; the CSP meta of D2; the bytes of `tokens.css`, `base.css` and `canvas.css` in its style; 5 of 5 | F-980 acceptance ("no network fetch", "the Studio's theme and styling"); D2 | 5 of 5 | exact | code verification |
| R4 | a4 | binary: provenance of the pages of X1 to X5 | every file of every panel's `data-sources` exists in the project and its listed hash equals `hash_project_file`'s; the header names the engine version and the page's id | G17; F-980 acceptance ("their provenance") | all | exact | code verification |
| R5 | a1 | binary: with a canvas process (`rcfd-mcp --project --canvas`), `show_view` of X4, then `export_view {}` | the page's `data-export-view` equals `read_view`'s id and its panels are the shown layout's (kinds and titles in order) | D1 | as stated | exact | code verification |
| R6 | a1 | binary, negative controls: `export_view` of `{view: X1 with panels[0].kind 'chart'}`; `{view: X1, out: '../x.html'}`; `{view: X1, out: 'exports/x.txt'}`; `{}` with no view shown | 1003 at `panels[0].kind`, at `out`, at `out`; 1202; after each no `exports/` folder and the journal byte-identical; 4 of 4 | D1; F-976 acceptance (refused with the key and the reason) | 4 of 4 | exact | code verification |
| R7 | a1, a4 | binary: the journal after R1 | one line `view_exported` per export, with its `path`, `hash` and `bytes` equal to the reply's | D1 | 5 of 5 | exact | code verification |
| E1 | a5 | e2e, the packaged Studio with the f980 project open, the test as MCP client: `show_view` of X1; once every panel holds its view, the Studio's values (finding 3); `export_view {}`; the file opened in a new window of the Studio's Electron in its own session, with every request other than the page's own `file:` URL cancelled and counted | every value of D3 equal within its limit, every panel; 0 requests other than the page's own | D3; F-980 acceptance ("opens offline") | equal; 0 | exact | code verification |
| E2 | a5 | e2e: the same for X2 | as E1 | as E1 | as E1 | exact | code verification |
| E3 | a5 | e2e: the same for X3 | as E1 | as E1 | as E1 | exact | code verification |
| E4 | a5 | e2e: the same for X4 | as E1 | as E1 | as E1 | exact | code verification |
| E5 | a5 | e2e: the same for X5 (3D store) | as E1 | as E1 | as E1 | exact | code verification |
| E6 | a3 | e2e: the theme | the page's `.cv` computed `font-family`, `color` and `background-color` equal the Studio's canvas `.cv`; `export.css` names no colour literal (only `var(--...)`) | F-980 acceptance ("the Studio's theme"); ADR 0805 E3 | equal | exact | code verification |
| Z1 | a6 | the size of the file of X1 (and of X2 to X5, reported) | recorded | F-980 acceptance ("recorded") | X1 about 0.4 MB | - | code verification |
| L1 | a7 | unedited existing tests: `canvas_layout_f977`, `canvas_live_f979`, `feed_f273`, `default_f1`, `tool_list_adr0730`; `rcfd-mcp --lib`, `rcfd-workspace --lib` (ADR 0805 V1, V2, ADR 0807 V3, V4, ADR 0817 R1, R2, L1, L2, V5, V6 among them); and `canvas_view_f976`, `project_list_f270`, `tool_list_f275`, `project_w48f`, `project_runs_w48f` | pass, but these five failing only at the assertions finding 5 names | rule 3; G4 | as stated | exact | unchanged default output |
| L2 | a7 | on a scratch copy of the final commit with the additive re-record applied (export_view taken off the end in `canvas_view_f976.rs` l1 and the block of `project_list_f270.rs`; 57 to 58 in `tool_list_f275.rs` and `project_runs_w48f.rs` twice; `export_view` appended to `PROJECT_TOOLS`; `19 + 39` in `project_w48f.rs`) | the five files pass | rule 3 (additive class) | pass | exact | unchanged default output |
| L3 | a3, a7 | `npm run typecheck` and `npm test` in `apps/studio-web` (every unit and e2e spec, unedited, against the branch's engine; main skips 6 tests, the registered records of ADRs 0807 and 0818) | pass, but `capabilities.spec.ts` failing on the row `mcp:export_view` only | rule 7; G13 | as stated | exact | unchanged default output |
| L4 | - | `cargo fmt -p rcfd-workspace -p rcfd-mcp --check`; `cargo clippy -p rcfd-workspace -p rcfd-mcp --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check` of the proposal file | clean; pass | rule 7; rule 9 | pass | exact | code verification |

No shared solver file is touched, so `baseline_hashes` is not run. `python scripts/process_report.py --check` is run last
and its result reported (rule 7). The CPU load is recorded with each timed run.

### Negative controls and mutations (rule 16)

The controls are R6 (four refusals, nothing written) and the counted requests of E1 to E5. Mutations, run on a scratch
copy of the final commit with the named test only, each restored afterwards:

| Id | Mutation | Must fail |
|---|---|---|
| M1 | a monitor's last value taken from its first sample | R2 |
| M2 | a `<link rel="stylesheet" href="https://...">` added to the page's head | R3 |
| M3 | the file written before the view is checked | R6 |
| M4 | a field's default snapshot the first, not the last | R2 |
| M5 | `tokens.css` left out of the page's style | R3 |

M1 is committed as `crates/rcfd-mcp/tests/export_view_f980.mutation.patch`, first line
`# expect-fail: cargo test --release -p rcfd-mcp --test export_view_f980`.

## Size

Size (agent): done; 1; 2; 1; 280000

(Expected outcome done; one further registration, an amendment, expected; two heavy runs on the shared machine, the
whole `npm test` of the Studio and the build of the scratch copy for the mutations and L2; one question for the
maintainer, Q1; about 280 000 bytes read.)

## Decision rule

**done** if R1 to R7, E1 to E6 and L1 to L4 hold, with the failures that L1 and L3 name as the only failures of the
existing tests, Z1 is recorded, and M1 to M5 are each caught; **partial** otherwise, naming the check that missed or did
not run. A missed limit is recorded with its number and held as a `*_known_miss` record (rule 2).

## Not covered

- The Archivo font files (finding 2, Q1): the page names the Studio's font stack and does not embed the fonts.
- The page is a picture of the view at the time of the export: nothing in it is live (no script), and a person cannot pan,
  zoom, scrub or pick in it; the derived pictures of the field view (cut, arrows, iso-surfaces, streamlines) and its
  range modes other than the shown snapshot's are not exported.
- The dark and contrast themes: the page carries the Studio's tokens with their themes, follows the system's colour
  scheme as the Studio does before a theme is set, and is compared in the theme the e2e Studio runs in.
- Linux and macOS; two Studios.

## Open questions for the maintainer

- **Q1. The Studio's font in the page (scope).** (a) leave the fonts out: the page names the font stack, and shows
  Helvetica Neue or Arial where Archivo is not installed; (b) the Studio's build writes the Archivo files (400, 500, 700;
  the npm package `@fontsource/archivo`) into a folder the engine embeds as `data:` fonts, which makes the engine's build
  read a file of the Studio's build. Recommendation: (a) for this item, and (b) as a follow-up if the exported pages
  are to be shown outside the machine they were made on. Taken here: (a), on the maintainer's standing instruction of
  2026-11-11 (take the recommendation); asked because (b) is a change of scope.

## Amendment 1 (2026-11-27): the set-up of R2 corrected

Written after the first run of `export_view_f980` and before any corrected run; nothing above is changed.

The first run of `r1_r2_r3_r4_r7_the_pages_of_the_registered_set` stopped inside R2 at the test's own call
`list_runs {limit: 1000}`, which the tool refuses (-32602, "'limit' is above the maximum 100"): a defect of the test's
set-up, not of the page. Before it stopped it had measured R1, R3 and R4 for X1 only; R5 and R6 passed in the same run.
That test stays as an ignored record (`#[ignore]`, its code unchanged). The corrected test
`r1_r2_r3_r4_r7_a1_the_pages_of_the_registered_set` is the same but asks `list_runs` with `limit: 100` (the f980 project
has 6 runs). No row, limit, source, mutation or the decision rule changes. The binary was run a second time, with the
same code, only to read the text of the failure (the first run's output had been filtered); it gave the same failure.

## Amendment 2 (2026-11-27): the table check of R2 corrected

Written after the run of the test of amendment 1 and before any further run; nothing above is changed.

The run of `r1_r2_r3_r4_r7_a1_the_pages_of_the_registered_set` measured R1, R3, R4 and R2 for X1 and X2 and R1, R3, R4
for X3, every value equal, then stopped at the test's own assertion that a table has two rows: true of X1 and X2 (two
runs), not of the layouts of X3 to X5, whose table lists one run. A defect of the set-up, not of the page. That test
stays as an ignored record. The corrected test `r1_r2_r3_r4_r7_a2_the_pages_of_the_registered_set` asks of a table one
or more rows, as many steps as rows, each step `list_runs`'s, which is what R2 registered. No row, limit, source, mutation
or the decision rule changes.

## Addendum 1: results (2026-11-27)

Written after the runs; nothing above is changed. Release profile, `CARGO_TARGET_DIR` in the scratch folder, 8 build
jobs. Commits: 6675b7b3 (this registration), c7e965cb (engine and page), 6885c8db (tests), 98cc4433 and 7362b802
(amendment 1 and its test), 6cc7cd96 and c813878c (amendment 2 and its test), then this addendum, the committed mutation
and the proposal. CPU load (Windows `LoadPercentage`): 53 to 66 % at the binary runs, 68 to 97 % at the `--lib` and
named runs, 84 % at the typecheck, 70 % at the start and 80 % at the end of `npm test`, 85 % at the mutations.
Deviations: `cargo fmt` (a write) before the code commit; two set-up defects of the binary test, corrected by amendments
1 and 2 before their runs, the earlier tests kept as ignored records; the first binary was run a second time only to read
its failure text (amendment 1).

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| R1 | 5 of 5 pages as the reply says | 5 of 5: X1 268 157 bytes, X2 409 488, X3 956 756, X4 501 487, X5 527 715; 52 to 507 ms each | yes |
| R2 | every value equal to the engine's tools (1e-12; exact) | X1 to X4, 22 panels: every field range equal to `field_summary` as `f32` (e.g. p of 3900 -152.87271118164063 to 1078.4739990234375, snapshot 27, step 278), every monitor's last sample, plot 831 drawn 3 skipped (3900), 2400 and 0 (d5cb), run and table steps, the setting 0.01 | yes |
| R3 | no reference, the CSP, the styles; 5 of 5 | 5 of 5, no reference found; the unit test on X1 passed (`rcfd-workspace --lib` 13 passed) | yes |
| R4 | every source exists with its hash | 11, 11, 7, 7, 7 files, every hash equal to `hash_project_file`'s; engine version and id in the header | yes |
| R5 | the shown view exported | page id `fnv64:1dab47f6fda7721e` equal to `read_view`'s; the six panels of the heat-transfer layout in order | yes |
| R6 | 4 refusals, nothing written | 1003 at `panels[0].kind`, `out`, `out`; 1202; no `exports/`, journal unchanged | yes |
| R7 | one `view_exported` line per export | 5 of 5 | yes |
| E1 | X1: every value equal; 0 other requests | 0 requests beyond the page's file; 14 of 15 values equal; **the monitor's last step: Studio `278`, page `278.0`** (the page writes the step with the JSON text of an `f64`; the registered comparison holds a step to exact text) | **no** |
| E2 to E5 | X2 to X5 | not run: the spec is serial and stops at E1's failure | **not run** |
| E6 | the theme | not run (after E1) | **not run** |
| Z1 | recorded | X1 268 157 bytes (about 0.27 MB; the estimate was 0.4 MB) | recorded |
| L1 | pass but the assertions of finding 5 | `canvas_layout_f977` 1, `canvas_live_f979` 2, `feed_f273` 3, `default_f1` 2, `tool_list_adr0730` 2 passed; `rcfd-mcp --lib` 18 (1 ignored), `rcfd-workspace --lib` 13 passed; failing only at `canvas_view_f976.rs:216`, `project_list_f270.rs:134`, `tool_list_f275.rs:132`, `project_w48f.rs:137`, `project_runs_w48f.rs:104` and `:354` (the assertion whose `57` is on line 359) | yes |
| L2 | the five pass with the re-record | 4, 1, 8, 10 and 3 passed on the scratch copy | yes |
| L3 | typecheck; `npm test` but `capabilities.spec.ts` | typecheck exit 0; `npm test` 887 passed, 3 failed, 6 skipped (main's registered skips, ADRs 0807 and 0818), 10 did not run, 14.3 min: `capabilities.spec.ts` on `mcp:export_view` (as registered), E1 above, and `run-control.spec.ts:268` (the engine's answer to the renderer 23.5 ms against 20 ms, a timing limit under 70 to 80 % load in a file this item does not touch) | **no** |
| L4 | fmt, clippy, `docs_site`, the proposal check | fmt and clippy clean; `docs_site` and the proposal check after this addendum, in the report | see the report |

### Mutations

| Id | Caught by |
|---|---|
| M1 (a monitor's first sample; committed as `export_view_f980.mutation.patch`) | R2 (`export_view_f980.rs` line 698) |
| M2 (a link to https in the head) | R3 (line 815) |
| M3 (a file written before the check) | R6 (line 929) |
| M4 (the first snapshot by default) | R2 (line 662) |
| M5 (`tokens.css` left out) | **not caught**: R3 compares the page with the same constant the mutation empties; E6 (the computed colours) would see it, but was not run on the scratch copy |

Not run: E2 to E6, `baseline_hashes` (no shared solver file). The E1 miss is not held as a `*_known_miss` record in the
spec in this work; the follow-up of the proposal names the cause and the fix.

Actual size: partial; 2 further registrations (amendments 1 and 2); 2 heavy runs (`npm test` 14.3 min; the scratch
copy's build for the mutations and L2); 1 question; about 300 000 bytes read.

Decision by the registered rule: **partial**: E1 missed (the monitor's last step written `278.0` against `278`), E2 to
E6 did not run, L3 had a failure beyond the one it named, and M5 was not caught. Every engine row (R1 to R7, L1, L2)
held.

### For the lead: roadmap entries

> Status: partial.

> Note: ADR 0823, 2026-11-27, partial: export_view writes a view (any form show_view takes, or the view shown last) as one self-contained HTML page under exports/ (no script, a CSP of default-src 'none', the Studio's stylesheets inline, fields and a case's mesh as inline SVG of the store's surface, provenance per panel); the engine's tools agree with every value of the five registered pages (268 to 957 kB); in the open Studio E1 made no request beyond its file and agreed on 14 of 15 values, the monitor's last step written 278.0 against 278; E2 to E6 did not run.

> Gap: The page writes a monitor's last step as the JSON text of an f64 (278.0) where the Studio shows the integer (278), so E1 of ADR 0823 missed on that value (14 of 15 equal) and the serial e2e did not run E2 to E6 (the other four specifications and the theme). The tool list re-record (additive) and the inventory row mcp:export_view are the lead's.

> Follow-up: The exported page writes a monitor's last step as the Studio does, and the e2e of ADR 0823 runs whole. Deps F-980. Acceptance: Every limit not stated here is registered in the ADR before the run. The page of export_view writes each step and count as an integer, as the Studio shows it; the e2e of ADR 0823 (E1 to E6 on X1 to X5) holds every value of the registered comparison and makes no request beyond the page's own file.

The same entries are `docs/roadmap/proposals/0823-F-980.json`. Additive golden edit for the lead (rule 3): the six
assertions of L1 with the edits of L2, and the row `mcp:export_view` of `apps/studio-web/capabilities/inventory.json`.

## Open questions for the maintainer (addendum)

Q1 above stands (the fonts; recommendation (a), taken). No other.

## Amendment 3 (2026-11-27): the additive re-record of the tool list, by the lead's approval

Written after addendum 1 and before any edit or run it covers; nothing above is changed. Written by Claude Opus 5.5
(`claude-opus-5-5`), as a background agent of the lead. The lead's follow-up of 2026-11-27, decided on the maintainer's
standing instruction of 2026-11-26 (take the recommended option): the branch cannot merge while the assertions of
finding 5 fail, so the lead approves the additive re-record (rule 3) on this branch; Q1 is answered (a), the font stack
named and no fonts embedded, with (b) as a follow-up; E1's miss comes back here as a further registration (rule 13,
amendments 4 and 5).

The re-record, each file and line verified at b32f0343 before the edit, and nothing else in these files changed:
`crates/rcfd-mcp/tests/canvas_view_f976.rs` line 216 and `project_list_f270.rs` line 134 take `export_view` off the end
of the list (an assertion that it is last, then the list without it); `tool_list_f275.rs` line 132 57 to 58 and
`export_view` appended to `PROJECT_TOOLS` (38 to 39); `project_w48f.rs` line 137 `19 + 38` to `19 + 39`;
`project_runs_w48f.rs` lines 104 and 359 57 to 58; the row `{"id":"mcp:export_view","kind":"mcp_tool","source":"tools/list","access":{"project":"write_files"}}`
added to `apps/studio-web/capabilities/inventory.json` in its sorted place. The JSON goldens are not edited.

| Id | Check | Limit | Source | Kind |
|---|---|---|---|---|
| L5 | the five re-recorded Rust files, release, on the branch | pass | rule 3 (additive class); L2 | unchanged default output |
| L6 | `capabilities.spec.ts` in the one `npm test` run of amendment 4 | pass | rule 3; G13 | unchanged default output |

The decision rule is unchanged, except that L1 and L3 no longer name these failures as allowed: after this amendment
the five files and `capabilities.spec.ts` must pass.

## Amendment 4 (2026-11-27): the page writes each number as the Studio writes it

Written after amendment 3 and before any code or run it covers; nothing above is changed. Written by Claude Opus 5.5
(`claude-opus-5-5`). A further registration of E1's miss (rule 13), by the lead's follow-up of 2026-11-27.

**Diagnosis of E1's miss.** The page wrote every `f64` with `num()` of `crates/rcfd-workspace/src/view.rs` (line 1005),
`serde_json`'s text, which writes an integral `f64` with `.0` (`278.0`). The Studio writes a number as JavaScript's
`String(x)` writes it (ECMA-262, `Number::toString`): `278`. The monitor's last step is an `f64` in `read_residuals`
(the series' `x`), so the two texts differed though the values are equal. The old E1 stays as its record (addendum 1).

**The rule, by the kind of value the page shows**, each as the Studio writes it (lines at b32f0343 of
`apps/studio-web/src/renderer`):

| Value | The Studio writes it | The page writes it |
|---|---|---|
| field lo, hi | `${drawnRange.lo} ${drawnRange.hi}`, `field/FieldViewport.tsx` line 774: `String` of a number | `num` |
| field snapshot, step | `data-snapshot={shown.snapshot}`, `data-step={snap?.step}`, `FieldViewport.tsx` lines 768, 769: React writes a number attribute as `String` | integers, `num` of the step |
| mesh triangles | `data-triangles={drawn.values.length}`, `case/MeshPreview.tsx` line 128 | a count (`usize`) |
| plot drawn, skipped | `data-drawn`, `data-skipped`, `plots/PlotFrame.tsx` lines 114, 115 | counts |
| monitor last step, last value | `data-last-step={last?.step}`, `String(last.value)`, `canvas/Canvas.tsx` line 246 | `num` of each |
| run steps, snapshots stored | `{row.step}`, `{store.snapshots.length}`, `workspace/RunWorkspace.tsx` lines 505, 519 | `js_json` of `list_runs`'s step; a count |
| table step | `{r.step}`, `project/ProjectHome.tsx` line 928 | `js_json` of `list_runs`'s step |
| setting value | `JSON.stringify(got.value)`, `canvas/Canvas.tsx` line 200 | `js_json` (JSON, each number by `num`) |
| the facts' text (time, legend ends, residuals' last values) | not a hook of the comparison | `num` |

`num(x)` becomes JavaScript's `Number::toString` of the `f64` x, written in Rust from the shortest round-trip digits
Rust gives (`format!("{:e}", x)`: digits d of length k, exponent n with x = 0.d * 10^n): `0` for zero of either sign; a
leading `-` for x < 0; digits and n - k zeros when k <= n <= 21; `d[..n].d[n..]` when 0 < n <= 21; `0.` and -n zeros
then the digits when -6 < n <= 0; otherwise `d[0]`, `.d[1..]` when k > 1, `e`, the sign `+` or `-` and |n - 1|; `null`
when not finite (the engine's JSON carries no other). The rule is ECMA-262's `Number::toString` as recalled from
memory: no copy of the standard was read in this work, so it is checked against the Studio's runtime instead (U1).
`js_json(v)`: JSON as `serde_json` writes it, each number by `num` (a `u64` or `i64` by its digits).

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| U1 | unit (`view.rs`): `num` of 20 numbers (278, 0.01, -152.87271118164063, 1078.4739990234375, 1, 0, -0, 2400, 1e-7, 1.5e-7, 1e-6, 2.5e-6, 1e21, 1.2345678901234568e20, 123.456, -0.5, 5e-324, 1.7976931348623157e308, 100, 0.1 + 0.2) | each equal to the text `String(x)` gives in Node (the Studio's runtime), printed once after this amendment's commit and before the code, written into the test | ECMA-262 via its runtime; E1's miss | 20 of 20 | exact | code verification |
| E1 to E6 | as registered, the spec unedited, run once in the one `npm test` run after amendments 3 to 5, CPU load recorded | as registered | as registered | equal; 0 | exact | code verification |
| R1 to R7 | `export_view_f980` (the a2 test, R5, R6), run once after the code as rule 7's named test; the ignored records stay ignored | as registered | as registered | pass | exact | code verification |

Mutation M6: `num` of an integral `f64` written with `.0` again; must fail U1. Run on the scratch copy with M5's rerun
(amendment 5). The decision rule is the registration's over the checks as they stand after amendments 3 to 5: E1 to E6
are this amendment's run, U1 and M6 are added to it, and `run-control.spec.ts`'s timing miss, if it recurs, is a
failure of L3 recorded as such.

## Amendment 5 (2026-11-27): a check of the styles against their files, and M5 again

Written after amendment 4's code and before the test and runs it covers; nothing above is changed. Written by Claude
Opus 5.5 (`claude-opus-5-5`). M5 (`tokens.css` left out of the page) was not caught (addendum 1): R3 compares the page
with `EXPORT_STYLES`, the constant the mutation empties. A new test of `crates/rcfd-mcp/tests/export_view_f980.rs`,
`r3_a5_the_page_carries_the_studio_styles_read_from_their_files`, exports X1 in the f980 project and checks the page
against sources the mutation does not change: the bytes of `apps/studio-web/src/renderer/design/tokens.css`, `base.css`,
`canvas/canvas.css` and `canvas/export.css`, read from the files at the time of the test.

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| R3f | the four files' bytes, each non-empty, each in the page of X1 | 4 of 4 | F-980 acceptance ("the Studio's theme and styling"); M5's miss | 4 of 4 | exact | code verification |

Then M5 runs again, once, on a scratch copy (`git archive` of the commit of this test) with
`cargo test --release -p rcfd-mcp --test export_view_f980`: it must fail R3f. M6 (amendment 4) runs on the same copy
with `cargo test --release -p rcfd-workspace --lib u1_f980`. The old M5 row stays as its record. The decision rule is
amendment 4's, with R3f added and M5 counted by this run.

## Addendum 2: results of amendments 3 to 5 (2026-11-27)

Written after the runs; nothing above is changed. Written by Claude Opus 5.5 (`claude-opus-5-5`). Release profile,
`CARGO_TARGET_DIR` `w49i-work/f980-export-view-html-scratch/target` (the copy: `target-copy`), 8 build jobs; `rcfd`,
`rcfd-mcp`, `rcfd-runner` and `f447_bench` built into it before `npm test`. Commits: 72fe3636 (amendment 3), 43f126e0
(the re-record), f3b3da15 (amendment 4), its code, 4785f4ce (amendment 5), f6c8fc17 (R3f), then this addendum and the
proposal. CPU load (`LoadPercentage`): 90 % at the build, 71 % at clippy, 99 % at the `--lib` runs, 90 % at the named
tests, 100 % at the mutations, 100 % at the typecheck, 85 % at the start and 97 % at the end of `npm test` (14.1 min).

Deviation: Node printed `-152.87271118164062` for the f64 that Rust's shortest digits write `-152.87271118164063`, an
exact tie between two 17-digit texts; ECMA-262 takes the even one. This was seen in Node's output before the code, and
`num` takes, of two equally near texts, the even one (the rule of amendment 4, "JavaScript's `Number::toString`"); no
limit or number of U1 changed.

| Id | Limit | Measured | Met |
|---|---|---|---|
| U1 | 20 of 20 equal to Node v22.19.0's `String(x)` | 20 of 20 (`rcfd-workspace --lib` 14 passed) | yes |
| R1 to R7 | as registered | `export_view_f980` 4 passed, 2 ignored (the records of amendments 1 and 2): X1 268 147 bytes, X2 409 474, X3 956 742, X4 501 473, X5 527 702; R2 5, 5, 6, 6 panels equal; R4 11, 11, 7, 7, 7 files; R5 id `fnv64:3af9f4fbf864a7a0` equal; R6 1003, 1003, 1003, 1202; R7 5 lines | yes |
| R3f | 4 of 4 files in the page | 4 of 4 (`tokens.css` 7 534 bytes, `base.css` 3 402, `canvas.css` 1 396, `export.css` 2 504) | yes |
| E1 | X1: every value equal; 0 other requests | 0 values differ; 0 requests beyond the page's file (268 309 bytes) | yes |
| E2 | X2 | 0 differ; 0 requests | yes |
| E3 | X3 | 0 differ; 0 requests | yes |
| E4 | X4 | 0 differ; 0 requests | yes |
| E5 | X5 (3D store) | 0 differ; 0 requests | yes |
| E6 | the theme equal; no colour literal | both `Archivo, "Helvetica Neue", Arial, sans-serif`, `rgb(29, 58, 95)`, `rgba(0, 0, 0, 0)`; 0 literals | yes |
| L5 | the five re-recorded files pass | `canvas_view_f976` 4, `project_list_f270` 1, `tool_list_f275` 8, `project_w48f` 10, `project_runs_w48f` 3 passed | yes |
| L1 | the other named files and `--lib` | `canvas_layout_f977` 1, `canvas_live_f979` 2, `feed_f273` 3, `default_f1` 2, `tool_list_adr0730` 2; `rcfd-mcp --lib` 18 (1 ignored), `rcfd-workspace --lib` 14 passed | yes |
| L6 | `capabilities.spec.ts` passes | **4 failed**: the re-record as listed added the row `mcp:export_view` but not the inventory's counts (`mcp_tool` 74 to 75, `total` 1375 to 1376) or the generated report `docs/studio-coverage.md` (75 MCP tools, the list with `export_view`); the engine's list itself agrees | **no** |
| L3 | typecheck; `npm test` passes | typecheck exit 0; `npm test` 889 passed, 5 failed, 6 skipped (main's registered skips, ADRs 0807 and 0818), 6 did not run (after a failure in serial files): L6's four, and `commands.spec.ts:156` (the palette's active entry not `run.status` within the 5 s poll, at 85 to 97 % load, a file this item does not touch; not diagnosed); `run-control.spec.ts:268` passed this time | **no** |
| L4 | fmt, clippy | `cargo fmt -p rcfd-workspace -p rcfd-mcp --check` clean; clippy of both `--all-targets -D warnings` clean; `docs_site`, the proposal check and the process report after this addendum, in the report | yes |

| Mutation | Caught by |
|---|---|
| M5 again (`tokens.css` left out), on the copy of f6c8fc17 | R3f (`export_view_f980.rs` line 957: `tokens.css` not in the page, 3 of 4) |
| M6 (an integral `f64` written with `.0`) | U1 (`view.rs` line 2637: 15 of 20, `278.0` against `278` among them) |

Not run again: `npm test` (made once, as registered); L6's fix. The fix of L6 is to run `npm run capabilities` in
`apps/studio-web` (it writes the counts and `docs/studio-coverage.md` from the binaries), which is the lead's re-record.

Actual size of amendments 3 to 5: partial; 3 further registrations (amendments 3, 4, 5); 2 heavy runs (`npm test`, the
copy's build); 0 questions; about 120 000 bytes read.

Decision by the registered rule (amendments 4 and 5): **partial**: L6 and L3 missed (the inventory's counts and report
not re-recorded; one `commands.spec.ts` failure). Every check of the acceptance held: R1 to R7, R3f, U1, E1 to E6, Z1
(recorded), L1, L2, L5, L4; M1 to M6 are each caught (M5 by this run).

### For the lead: roadmap entries

> Status: partial.

> Note: ADR 0823, 2026-11-27, partial: export_view's page now writes each number as the Studio does (JavaScript's String, of two equally near texts the even), and the e2e held whole: E1 to E6 on X1 to X5 agreed on every value with no request beyond the page's own file and the Studio's theme; M5 is caught by the styles read from their files; capabilities.spec.ts failed on the inventory's counts and report, which the re-record as listed did not include, and commands.spec.ts:156 failed once under load.

> Gap: The capability inventory of the Studio has the row mcp:export_view but not its counts (mcp_tool 75, total 1376) nor docs/studio-coverage.md, so capabilities.spec.ts fails until npm run capabilities is run in apps/studio-web (the lead's re-record); commands.spec.ts:156 failed once at 85 to 97 % load, not diagnosed.

> Follow-up: The Studio's Archivo fonts embedded in an exported page. Deps F-980. Acceptance: Every limit not stated here is registered in the ADR before the run. The page of export_view carries the Archivo fonts the Studio shows (400, 500, 700) as data: fonts written by the Studio's build, with no network fetch, and the page's computed font in the e2e of ADR 0823 is Archivo with no font installed.

The same entries are `docs/roadmap/proposals/0823-F-980.json`, which keeps addendum 1's follow-up as the lead numbered
it (F-993).

## Amendment 6 (2026-11-27): E1's monitor value outside the worktree

Written after the lead's follow-up of 2026-11-27 and before any change or run it covers; nothing above is changed.
Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead. A further registration under F-980
(rule 13). The lead merged c65d8a73 (merge 2896a777 on 689fab92) and ran `scripts/push_main.py`; `npm test` failed in
each of its 3 runs on E1 only, "panel 5 monitor value: Studio 2.257184947479129, page 13.580549682488824" (894 passed),
where the same spec had passed in this worktree (addendum 2). The merge was taken off.

**What was read before this text (nothing was built or run).** The push log (`w49i-work/push-f980.log`, lines 126 to
131 and 2557 to 2575): of the 15 values of E1 only the monitor's value differs; its step agrees (278 and 278). In
`runs/r-20261028T152641-3900/residuals.jsonl` of quick-start (278 lines, one sample of `nu_hot` per step) the first line
(step 1) has `nu_hot` 13.580549682488824 and the last (step 278) 2.257184947479129, the value `metrics.json` and line 279
of `output/monitors.csv` also give. The page's value is the first sample with the last step: the output of M1, the
committed patch `crates/rcfd-mcp/tests/export_view_f980.mutation.patch`. `scripts/push_main.py` (its docstring step 3,
`verify_patch` and `do_run`) applies each mutation patch with `git apply`, runs its `expect-fail` command, removes the
patch with `git apply -R`, and then runs `npm test` with no build of the engine between; M1's command
`cargo test --release -p rcfd-mcp --test export_view_f980` builds the binaries of the package `rcfd-mcp` for its
integration test, so `target/release/rcfd-mcp.exe` of the checkout is built with M1 applied and stays so after the patch
is removed. The spec's `findMcp` (`tests/support/project.ts` line 20) takes `RCFD_MCP_BIN`, then `checkoutRelease`
(`CARGO_TARGET_DIR`'s or the checkout's `target/release`). The patches run in name order and this item's is the last of
the five (log lines 126 to 130), so its build is the one left for `npm test`. The Studio's value is read through its own
engine's `read_residuals`, which M1 does not change.

**Hypotheses, what each predicts in the control, and the remedy each points to.**

| H | Why the page's value differs from the Studio's outside the worktree | Predicts in C1, C2, C3 (below) | Remedy |
|---|---|---|---|
| H1 | the page is written by an `rcfd-mcp` built with M1 applied, left in the target folder by the push's mutation step | C1 pass, C2 pass, C3 E1 fails with the page's value 13.580549682488824 at step 278 and every other value of E1 equal | the patch's command builds in a target folder of its own (below), so the target `npm test` uses is never built with the mutation; the script's general defect to the lead |
| H2 | the page reads another file or column than the Studio (`output/monitors.csv`, `monitors.json`, `metrics.json`, another monitor) | C1 fails as the push did (this head's code of the page is the merged code's); but every file gives 2.257184947479129 at step 278 | read the Studio's series (`read_residuals`, `monitors.<m>`) |
| H3 | the page takes another sample or a reduction (a decimation to 800 points, a sample at a time, a mean) | C1 fails as the push did; but 278 samples are fewer than 800 | the Studio's sample: the last of `read_residuals {max_points: 800}` |
| H4 | state left in the project folder by an earlier spec, or the order of the specs | C1, C2 and C3 pass (the spec alone, in its own fresh copy of quick-start, which the Studio and the page both read) | a project folder the spec owns alone |
| H5 | the merge with main (689fab92) changed the code the page or the Studio reads | C1, C2 pass and C3 passes | merge main and diagnose there |

**The control.** Each run once, in this order, at the commit of this amendment (no code change), with
`CARGO_BUILD_JOBS=8` and the CPU load recorded. A run of the spec is `npm run build && npm run package:dir && npx
playwright test tests/e2e/export-view-f980.spec.ts` in `apps/studio-web` with `RCFD_BIN` and `RCFD_MCP_BIN` set to that
place's target, `rcfd`, `rcfd-mcp`, `rcfd-runner` and the example `f447_bench` built into it first.

- C1: a fresh copy of the repository at this head, outside the worktree (`git clone --shared` into the new folder
  `w49i-work/f980-export-view-html-scratch/copy6`, `npm ci` in its `apps/studio-web`), its target
  `f980-export-view-html-scratch/target-copy`: the spec alone.
- C2: this worktree, its target `f980-export-view-html-scratch/target`: the spec alone.
- C3: in the copy, the push's sequence with the patch as committed: `git apply` of the patch, its `expect-fail` command
  with the copy's target (it must fail), `git apply -R`, then the spec alone again with no build of the engine between
  (the build of the Studio and the package only, as `npm test` does).

H1 is taken if C1 and C2 pass and C3 fails as H1 predicts; H2 or H3 if C1 fails at the same value; H4 or H5 if C3
passes. Only under H1 is the remedy below made; under any other outcome the work stops and reports.

**The remedy under H1.** The first line of `crates/rcfd-mcp/tests/export_view_f980.mutation.patch` becomes
`# expect-fail: cargo test --release -p rcfd-mcp --test export_view_f980 --target-dir target/mutation` (the command line's
`--target-dir` takes precedence over `CARGO_TARGET_DIR`; `target/` is ignored by git, so the folder persists between
pushes and its build is incremental). The patch's diff, the test, the page and the spec are not changed. The other
four patches the push applies have the same defect, hidden because a later patch's build replaces their binary; their
files are not this item's, so that and a fix of `verify_patch` (a build after `git apply -R`, or one target folder for
every mutation) go to the lead.

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| C1 | the spec alone in the fresh copy | 6 passed, 0 failed | E1 to E6 as registered (D3) | pass | exact | code verification |
| C2 | the spec alone in this worktree | 6 passed, 0 failed | as C1 | pass | exact | code verification |
| C3 | the push's sequence in the copy, then the spec | the command fails; E1 fails with the page's value 13.580549682488824, the Studio's 2.257184947479129, the step 278 and 278, and no other value differs | H1; the push log lines 2570, 2571 | as stated | exact | code verification |
| C4 | at the remedy's commit, in the copy (fetched, checked out): the four binaries built clean into `target-copy` again, then the push's sequence with the remedied patch, then the spec alone with no build of the engine between | the command fails at R2 (M1 caught); then 6 passed, 0 failed; the build minutes of `target/mutation` recorded | rule 16; E1 to E6 | pass | exact | code verification |
| M1 to M6 | M1 by C4; M2 to M6 as amendments 4 and 5 stand, on the copy at the remedy's commit, each with its named test, each restored | each caught: M2 by R3, M3 by R6, M4 by R2, M5 by R3f, M6 by U1 | rule 16 | 6 of 6 | exact | code verification |
| L7 | `npm test` in `apps/studio-web` of this worktree, once, after the remedy, CPU load at its start and end | pass; a failure of a spec this item does not touch on a timing limit is recorded as a miss with its number, not remedied | rule 7; L3 | pass | exact | unchanged default output |
| L8 | in this worktree: `cargo fmt -p rcfd-workspace -p rcfd-mcp --check`; `cargo clippy -p rcfd-workspace -p rcfd-mcp --all-targets -- -D warnings`; `cargo test --release -p rcfd-mcp --test export_view_f980`; `cargo test --release -p rcfd-workspace --lib`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0823-F-980.json`; `python scripts/process_report.py --check` | clean; 4 passed and 2 ignored; 14 passed; pass; pass; only a generated file not current | rule 7; rule 9 | pass | exact | code verification |

Decision rule: **done** if C1 to C4, M1 to M6, L7 and L8 hold; **partial** otherwise, naming the check that missed or
did not run. An addendum (append-only) records the results and the proposal is updated, its follow-ups F-993 and F-994
kept.

## Addendum 3: results of amendment 6 (2026-11-27)

Written after the runs; nothing above is changed. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent
of the lead. Release profile, `CARGO_BUILD_JOBS=8`; the worktree's target `w49i-work/f980-export-view-html-scratch/target`,
the copy's `target-copy`; `rcfd`, `rcfd-mcp`, `rcfd-runner` and `f447_bench` built into each before its first spec run;
`RCFD_BIN`, `RCFD_MCP_BIN` and `CARGO_TARGET_DIR` set to that target. Commits: 3f5b4acc (amendment 6, alone), 2632f9b7
(the remedy: the first line of the committed patch), then this addendum and the proposal. The copy:
`w49i-work/f980-export-view-html-scratch/copy6`, `git clone --shared` of the repository, checked out at 3f5b4acc for C1
to C3 and at 2632f9b7 for C4 and the mutations, `npm ci` in its `apps/studio-web`. Scripts and logs: `ctl.py`,
`mut6.py`, `ctl-*.log`, `npm-test6.log`, `rust6.log` in the scratch folder.

| Id | Limit | Measured | Met |
|---|---|---|---|
| C1 | the spec alone in the fresh copy: 6 passed | 6 passed in 18.4 s; E1 to E5 0 values differ; CPU 80 % | yes |
| C2 | the spec alone in this worktree: 6 passed | 6 passed in 17.0 s; E1 to E5 0 values differ; CPU 76 % | yes |
| C3 | the push's sequence, then the spec: the command fails; E1 fails with page 13.580549682488824, Studio 2.257184947479129, steps 278 and 278, no other value different | `git apply`; the command exit 101 (`r1_r2_r3_r4_r7_a2_...` at `export_view_f980.rs:698`); `git apply -R`, the copy clean; the spec: E1 failed, "panel 5 monitor value: Studio 2.257184947479129, page 13.580549682488824", monitor step 278 and 278, the other 14 values equal, 5 did not run (serial); CPU 65 to 90 % | yes |
| C4 | at 2632f9b7: binaries rebuilt clean, the push's sequence with the remedied patch fails at R2, then 6 passed | `target-copy`'s `rcfd-mcp.exe` rebuilt from the clean source (04:01); the command `... --target-dir target/mutation` exit 101 at `export_view_f980.rs:698` (R2); the sequence took 2.2 min with the first build of `copy6/target/mutation`; `git apply -R`, the copy clean; the spec 6 passed in 16.2 s, 0 values differ | yes |
| L7 | `npm test` once in this worktree: pass | 900 passed, 6 skipped (main's registered skips, ADRs 0807 and 0818), 0 failed, 14.6 min; CPU 80 % at the start, 71 % at the end; its build ran `npm run typecheck` (exit 0); `capabilities.spec.ts` passed; no timing miss this run (`run-control.spec.ts:268` and `commands.spec.ts:156` passed) | yes |
| L8 | fmt, clippy clean; `export_view_f980` 4 passed 2 ignored; `rcfd-workspace --lib` 14 passed; `docs_site` pass; proposal check pass; process report | `cargo fmt -p rcfd-workspace -p rcfd-mcp --check` exit 0; clippy of both `--all-targets -D warnings` exit 0 (CPU 71 %); `export_view_f980` 4 passed, 2 ignored (the records of amendments 1 and 2; CPU 68 %); `rcfd-workspace --lib` 14 passed (CPU 59 %); `docs_site` 2 passed (CPU 66 %); the proposal check and the process report after this addendum, in the report | yes (see the report for the last two) |

| Mutation | Caught by |
|---|---|
| M1 (the committed patch, remedied command) | R2 (`export_view_f980.rs` line 698), in C4 |
| M2 (a link to https in the head) | R3 (line 815) |
| M3 (a file written before the check) | R6 (`r6_refusals_write_nothing`, line 929) |
| M4 (the first snapshot by default) | R2 (line 662) |
| M5 (`tokens.css` left out) | R3f (`r3_a5_...`, line 957) |
| M6 (an integral `f64` written with `.0`) | U1 (`view.rs` line 2637: `278.0` against `278`, `1.0` against `1`) |

Diagnosis: H1. The page of the push's `npm test` was written by an `rcfd-mcp` built with M1 applied: the push's
mutation step builds the package's binaries into the checkout's target with the patch applied and removes the patch
without a build, and `npm test` builds only the Studio. C1 and C2 pass, so the code at this head is not the cause in
either place (H2, H3 are refuted); C3 reproduces the push's failure to the digit without any other spec before it (H4
refuted) and without main (H5 refuted). The remedy (the patch's command builds in `target/mutation`) leaves the target
`npm test` uses as the clean build left it (C4).

For the lead: the four other committed patches the push applied to this merge (`canvas_layout_f977`, `canvas_live_f979`,
`canvas_view_f976`, `design_uq_sobol_f339`) also build `rcfd-mcp` into the checkout's target with their mutation; only
the last patch's build survives to `npm test`, so any merge whose last patch mutates code a spec reads will fail the
same way. A fix in `scripts/push_main.py` (a build of the touched packages after `git apply -R`, or every patch's
command run with its own target folder) is the lead's file and is not made here. The first push after this merge builds
`target/mutation` from nothing (2.2 min on the copy, at 65 to 90 % load); later pushes build it incrementally.

Deviations: none in the registered runs; each was made once. The loop of L8 wrote a temporary file of its output in the
worktree's root and deleted it at the end (`git status --short` empty after).

Actual size of amendment 6: done; 1 further registration (amendment 6); 2 heavy runs (`npm test`; the copy's builds and
four spec runs); 0 questions; about 90 000 bytes read.

Decision by the registered rule (amendment 6): **done**. C1 to C4, M1 to M6, L7 and L8 hold. With the lead's re-record
of c65d8a73, every check of the registration as amended holds (R1 to R7, R3f, U1, E1 to E6, Z1 recorded, L1, L2, L4 to
L8, M1 to M6), and L3 and L6, which missed in addendum 2, passed in L7's run.

### For the lead: roadmap entries

> Status: done.

> Note: ADR 0823, 2026-11-27, done: export_view writes a view as one self-contained HTML page (no script, a CSP of default-src 'none', the Studio's stylesheets inline, fields and a case's mesh as inline SVG, provenance per panel) whose every value equals the open Studio's on X1 to X5 with no request beyond its own file; the push's E1 miss was the mutated rcfd-mcp left by M1's expect-fail build, reproduced to the digit in a fresh copy, and M1's command now builds in target/mutation.

> Follow-up: The Studio's Archivo fonts embedded in an exported page. Deps F-980. Acceptance: Every limit not stated here is registered in the ADR before the run. The page of export_view carries the Archivo fonts the Studio shows (400, 500, 700) as data: fonts written by the Studio's build, with no network fetch, and the page's computed font in the e2e of ADR 0823 is Archivo with no font installed.

The same entries are `docs/roadmap/proposals/0823-F-980.json`, which keeps both follow-ups as the lead numbered them
(F-993, F-994) and drops the gap (the status is done).
