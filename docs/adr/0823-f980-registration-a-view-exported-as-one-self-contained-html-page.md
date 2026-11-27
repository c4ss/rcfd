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
