# ADR 0838: F-987 registration: show_view's description names every panel kind, the three forms of ADR 0807 and the run form

Status: **REGISTERED 2026-11-27, written before any code of F-987 and before any run of its checks.** Before this text
nothing was changed, built or run but reading and searching. Results go in addenda at the end; nothing above them is
changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f987-show-view-description.md` ([the file](briefs/f987-show-view-description.md))
and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f987-show-view-description`
from 476de79b (origin/main at the start of this work, also the merge base), scratch
`w49i-work/f987-show-view-description-scratch`.

## What was read

`AGENTS.md`, the rules (version 5) and the brief, whole. F-987's acceptance by a script. Of
[ADR 0807](0807-f979-registration-live-meshes-settings-and-monitors-on-the-canvas.md): "what was read" item 8, D1 (the
three forms), D2, the checks V3 and V4, and Q1. Of [ADR 0817](0817-f977-registration-layouts-chosen-by-the-case-category.md):
items 6 and 7 of what was read, D3 (the run form), V5, V6 and Q1. `crates/rcfd-mcp/tests/tool_list_f275.rs` lines 1 to 175
(its golden use and `PROJECT_TOOLS`), `crates/rcfd-mcp/tests/project_list_f270.rs` lines 30 to 175, the description check
B3 of `crates/rcfd-mcp/tests/canvas_process_f978.rs` (lines 205 to 235), the tool counts of `project_runs_w48f.rs`,
`crates/rcfd-mcp/src/project.rs` lines 505 to 560 (show_view, read_view, export_view), `crates/rcfd-workspace/src/view.rs`
lines 26 to 52 (`KINDS`, `KINDS_F979`, `KINDS_F978`) and 120 to 330 (the checks of a specification), `setting_ref` and
`report_ref`, the head of `crates/rcfd-mcp/tests/common/mod.rs`. ADR 0835's registration for the form. Bytes read: about
150 000.

Found by reading, before any code:

1. **No golden holds show_view.** `tools_list_project_f270.json` holds 52 tools and ends with `wait_run`;
   `tools_list_default.json` holds 35 and ends with `render_image`; neither names `show_view`, `read_view` or
   `export_view` (a search of the three files for `show_view` finds nothing). `project_list_f270.rs` takes `export_view`,
   then `show_view` and `read_view` off the end of the live list before it compares the rest with the golden byte for
   byte, and checks only their names and classes; `tool_list_f275.rs` C5-L compares the session tools with the default
   golden and the project tools by name only. So changing show_view's description changes **no golden byte**, and there
   is nothing for the lead to re-record. ADR 0807 item 8 and ADR 0817 item 6 took the change to be one of the tool-list
   goldens; it is one of the live list only. This registration therefore registers that both goldens stay byte for byte
   as at 476de79b (G1), which is stronger than "differ in show_view's entry only", and leaves the reading of the
   acceptance's re-record clause to the lead (Q1).
2. The only test that reads show_view's description is B3 of `canvas_process_f978.rs`: it looks for `process {case}`.
3. The kinds the engine takes are `KINDS` (case, run, field, mesh, plot, monitor, table; F-976), `KINDS_F979` (setting;
   ADR 0807) and `KINDS_F978` (process; ADR 0826), all public in `rcfd_workspace::view`. The forms of ADR 0807 D1 are a
   mesh panel with `case` instead of `mesh`, a setting panel `{case, setting}` (a dotted setting path as `edit_case` takes
   it; `read_view` gives its value under `settings`, D2), and a monitor panel with `case` instead of `run`, whose `monitor`
   is a report of the case (`reports.<name>`) followed in the case's newest run. The run form of ADR 0817 D3 is
   `{version: 1, run, title?, layout?}` without `panels`, a view of the run that takes the layout of its case's category.

## Design

### D1. The description (the one changed line of `crates/rcfd-mcp/src/project.rs`)

Only the `description` string of the `show_view` entry changes; its name, class and schema do not, and no other tool's
text changes. The new text, registered word for word:

> Show a view in the Studio that has this project open (Claude's canvas). view is a view specification, in one of two forms: {version: 1, title?, layout?: {columns: 1-4}, panels: [1 to 8 panels]}, or {version: 1, run, title?, layout?} without panels, a view of a run that takes the layout of its case's category (the panels chosen for the category of the run's case). Each panel is {kind, title?} with, by kind: case {case: a file under cases/}, run {run}, field {run, field: a stored field, snapshot?: index, default the last}, mesh {mesh: an entry of list_meshes} or mesh {case} (the mesh of a case under cases/), plot {run} (the residuals), monitor {run, monitor: a name of the run's monitors.<name>} or monitor {case, monitor: a report of the case, reports.<name>} (followed in the case's newest run), table {runs: 1 to 10 run ids}, setting {case, setting: a dotted setting path of the case, as edit_case takes it} (its value, which read_view gives under settings), process {case} (the case's steps from its creation to its last result, which read_view gives as process; each step names the view of its state). Checked against the project first: an invalid view is refused (error 1003) with every key and reason in data.errors, and nothing changes. With no Studio open, error 1002 says so and nothing is written. Otherwise the view is written to .rcfd/views/current.json with one journal line (view_shown), which every open Studio reads from the change feed and shows with its own views; returns its id and how many Studios have the project open.

The text from "Checked against the project first" to the end is origin/main's, unchanged.

### D2. The test: `crates/rcfd-mcp/tests/show_view_description_f987.rs`

A function `missing(desc)` gives, in a fixed order, every registered phrase that `desc` does not contain. The phrases
(22), each with its source:

| group | phrases | source |
|---|---|---|
| kinds (9) | `<kind> {` for each kind of `KINDS`, `KINDS_F979` and `KINDS_F978`, read from `rcfd_workspace::view` | F-976 (ADR 0805), ADR 0807 D1, ADR 0826 D1 |
| F-976's forms (7) | `case {case: a file under cases/}`, `run {run}`, `field {run, field`, `mesh {mesh: an entry of list_meshes}`, `plot {run}`, `monitor {run, monitor`, `table {runs` | ADR 0805 |
| F-978 (1) | `process {case}` | ADR 0826 |
| ADR 0807 D1 (3) | `mesh {case}`, `setting {case, setting`, `monitor {case, monitor` | ADR 0807 D1 |
| ADR 0817 D3 (2) | `{version: 1, run, title?, layout?}`, `layout of its case's category` | ADR 0817 D3 |

That is 9 + 7 + 1 + 3 + 2 = 22 phrases; the test asserts that the list has 22.

Tests, each through the `rcfd-mcp` binary started with `--project` on a fresh project (`common::project`), `tools/list`:

- `d1_show_view_description_names_every_form`: `missing(show_view's description)` is empty.
- `d2_show_view_entry_otherwise_as_before`: the entry's `_meta.rcfd/access` is `write_files`, `inputSchema.required` is
  `["view"]`, `inputSchema.properties` has exactly the keys `reason` and `view`, and `view`'s description is
  `the view specification (version 1)`.
- `n1_old_description_misses_the_new_forms` (negative control, no binary): `missing` of origin/main's description,
  held in the test as a constant copied from 476de79b, is exactly, in order: `setting {`, `mesh {case}`,
  `setting {case, setting`, `monitor {case, monitor`, `{version: 1, run, title?, layout?}`,
  `layout of its case's category` (6).

The test file's header names ADR 0838 and F-987 (rule 12). No golden is written: neither by a record mode nor by hand.

## Checks (rule 14)

Release profile, `CARGO_TARGET_DIR` under the scratch folder, `CARGO_BUILD_JOBS=8`. `canvas_process_f978` needs `rcfd`
and `rcfd-runner` next to `rcfd-mcp`, so `cargo build --release -p rcfd-cli -p rcfd-runner` comes first. Base for the
diffs: 476de79b.

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| D1 | `d1_show_view_description_names_every_form` | 0 of 22 phrases missing | F-987 acceptance (names the forms of ADR 0807 and the run form of ADR 0817, and F-976's kinds) | 0 | exact | code verification |
| D2 | `d2_show_view_entry_otherwise_as_before` | as stated in D2 | F-987 acceptance ("every other entry unchanged"); ADR 0805 | holds | exact | code verification |
| N1 | `n1_old_description_misses_the_new_forms` | missing is exactly the 6 phrases of D2, in order | ADR 0807 Q1 and ADR 0817 Q1 (what the old text lacks) | 6 | exact | code verification |
| G1 | `git diff --exit-code 476de79b HEAD -- crates/rcfd-mcp/tests/golden/` | exit 0, no output | rule 3; F-987 acceptance ("every other entry unchanged") | 0 bytes | exact | unchanged default output |
| G2 | `git diff -U0 476de79b HEAD -- crates/rcfd-mcp/src/project.rs` and `--numstat` over `crates/*/src` | one file, 1 line removed and 1 added, both the `description:` line of `show_view` (starting `description: "Show a view in the Studio`) | F-987 acceptance; the brief ("no other tool's text changes") | 1 and 1 | exact | unchanged default output |
| E1 | `tool_list_f275`, `project_list_f270`, `tool_list_adr0730`, `default_f1` | every test passes | the brief's decision rule ("every tool-list test passes") | pass | exact | unchanged default output |
| E2 | `canvas_process_f978` (B3 reads the description) | passes | ADR 0826 B3 | pass | exact | code verification |
| L1 | `cargo test -p rcfd-mcp --release --lib` | passes | rule 7 | pass | exact | code verification |
| L2 | `cargo fmt -p rcfd-mcp --check` (or `rustfmt --check` of the touched files on os error 206); `cargo clippy -p rcfd-mcp --release --all-targets -- -D warnings` | clean | rule 7 | clean | exact | unchanged default output |
| L3 | `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0838-F-987.json`; `python scripts/process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

No row is of kind validation: the acceptance claims no agreement with a measured case.

### Negative controls and mutations (rule 16)

N1 is the negative control. Mutations, each on a scratch copy of the tree made by `git archive` of the code commit into
the scratch folder, applied there, with the test named and its own target folder under the scratch folder:

| Id | Mutation of `project.rs` (show_view's description unless said) | Expected to be caught by |
|---|---|---|
| M1 | delete ` or mesh {case} (the mesh of a case under cases/)` | D1 |
| M2 | delete the run form, from `, or {version: 1, run` to `of the run's case)` | D1 |
| M3 | `setting {case, setting:` becomes `setting {setting:` | D1 |
| M4 | delete ` or monitor {case, monitor: a report of the case, reports.<name>} (followed in the case's newest run)` | D1 |
| M5 | delete `process {case} ` | D1 |
| M6 | in `read_view`'s description, `exists false` becomes `exists: false` (another tool's text) | not caught by a test (no golden holds `read_view`, finding 1); caught by G2 |
| M7 | in `list_runs`'s description, one word changed (another tool's text, held in the project golden) | `project_list_f270` |

M1 is committed as `crates/rcfd-mcp/tests/show_view_description_f987.mutation.patch` (made with `git diff`), first line
`# expect-fail: cargo test -p rcfd-mcp --release --target-dir target/mutation --test show_view_description_f987`.

## Size

Size (agent): done; 0; 0; 1; 150000

(Expected outcome done; no further registration; no heavy run; one question for the maintainer, Q1; bytes to read.)

## Decision rule

Done for F-987 if D1, D2, N1, G1, G2, E1, E2, L1, L2 and L3 hold, M1 to M5 and M7 are caught by the test named, and M6
is caught by G2. Partial otherwise. G1 stands for the brief's "the goldens differ from origin/main in that entry only":
no golden holds the entry (finding 1), so they must not differ at all.

## Not covered

- That each form the description names is accepted by `show_view`: the checks V3 to V6 of ADRs 0807 and 0817 hold that
  already; this item changes text only.
- `docs/mcp-tools.md` and the Studio are not changed.
- The other forms' prose in `export_view`'s description is not changed (it is another tool's text).

## Open questions for the maintainer

- **Q1. The re-record clause of the acceptance (scope).** The acceptance has the lead re-record the tool-list goldens for
  show_view's entry, but no golden holds that entry (finding 1). (a) the clause is met as there is nothing to re-record,
  and F-987 is done by the rule above; (b) a follow-up item adds `show_view`, `read_view` and `export_view` to
  `tools_list_project_f270.json` (an additive golden edit the lead re-records) and has `project_list_f270.rs` compare
  them instead of taking them out, so that M6 is caught by a test. Recommendation: (a) for F-987, and (b) as a new item
  for the lead to decide; not taken here, as it touches an existing test and a golden.

## Addendum 1: results (2026-11-27)

Code commit 2c5009cf (the description line and the test), mutation patch fe644394; the run was made once, in the release
profile, by `run.sh` in the scratch folder (`CARGO_BUILD_JOBS=8`, the scratch target folder). Logs: `build.log`,
`tests.log`, `lib.log`, `fmt.log`, `clippy.log`, `mut_M1.log` to `mut_M7.log` in the scratch folder.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| D1 | 0 of 22 missing | 0 of 22 missing | yes |
| D2 | as stated in D2 | `write_files`, required `["view"]`, properties `reason` and `view`, `view`'s text as before | yes |
| N1 | exactly the 6 phrases, in order | 6, in the registered order | yes |
| G1 | exit 0, no output | exit 0, no output | yes |
| G2 | one file, 1 and 1, the show_view `description:` line | `crates/rcfd-mcp/src/project.rs` only, 1 removed and 1 added, line 529, show_view's description | yes |
| E1 | every test passes | `tool_list_f275` 8, `project_list_f270` 1, `tool_list_adr0730` 2, `default_f1` 2: 13 passed, 0 failed | yes |
| E2 | passes | `canvas_process_f978` 1 passed in 33.5 s ("B3: description names process true") | yes |
| L1 | passes | `rcfd-mcp --lib` 18 passed, 0 failed, 1 ignored (an existing record, not touched) | yes |
| L2 | clean | `cargo fmt -p rcfd-mcp --check` exit 0, no output; clippy `--release --all-targets -D warnings` exit 0 | yes |
| L3 | pass | in Addendum 2, run after this addendum is committed | Addendum 2 |

### Mutations

| Id | Caught by | What failed |
|---|---|---|
| M1 | D1 | exit 101, missing `mesh {case}` |
| M2 | D1 | exit 101, missing `{version: 1, run, title?, layout?}` and `layout of its case's category` |
| M3 | D1 | exit 101, missing `setting {case, setting` |
| M4 | D1 | exit 101, missing `monitor {case, monitor` |
| M5 | D1 | exit 101, missing `process {` and `process {case}` |
| M6 | G2 (not by a test, as registered) | the three tests exit 0; against 476de79b the mutated `project.rs` has 2 removed and 2 added lines (529 and 543), over G2's limit of 1 |
| M7 | `project_list_f270` | exit 101, "list_runs differs from the golden" |

### Deviations and what was not run

None of the registered checks deviated. The goldens are not touched (finding 1), so there is no golden diff to report and
no re-record for the lead. Not run: the whole `rcfd-mcp` integration suite and `project_runs_w48f` (they count the tools
only), `baseline_hashes` (no solver file touched), `npm test` (no Studio file touched).

Size (actual): done; 0; 0; 1; 160000

### Decision

D1, D2, N1, G1, G2, E1, E2, L1 and L2 hold, M1 to M5 and M7 are caught by the tests named and M6 by G2; with L3 passing
(Addendum 2), **done** by the registered rule. Q1 above stays open for the lead.

### For the lead: roadmap entries

> Status: done.

> Note: ADR 0838, 2026-11-27, done: show_view's description names F-976's seven kinds, the setting kind, ADR 0807's mesh-of-a-case and monitor-of-a-case forms, the process panel and ADR 0817's run form (22 of 22 phrases found, the old text missing exactly 6); no golden holds show_view, so no golden byte changed and nothing is re-recorded.
