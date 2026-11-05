# ADR 0186: ADR 0173 amendment 2, F-271 registration: new simulations from the MCP server in project mode

Status: **REGISTERED 2026-11-05**, written before any code of F-271. Not yet accepted by the maintainer.
Parent: [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) (proposed, not accepted),
requirement R1, work item W3, check C7, decisions D2 and D7, roadmap item F-271; it builds on
[ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) (amendment 1, F-270, merged at a662c4a).
ADR 0173 says that every limit of its checks is fixed by an amendment before any test or run; this is that amendment for F-271.
Results go in the addendum at the end.

Read at main 4e84c11. Number: 0185 is the highest on main; no branch and no worktree holds a 0186.

ADR 0173 is not accepted. Where it gives a recommendation, this text uses it under the maintainer's loop instruction and says so;
that is a recommendation taken, not a maintainer decision. Where the code read below raises a question ADR 0173 does not answer,
the options and a recommendation are listed (Q1, Q2), and work proceeds only on a choice that is reversible.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)): defaults stay bit-identical; no solver result, case
hash or byte of an existing case changes; no case-schema key and no run-record field is added;
`crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new third-party crate; no commit trailers.

## What F-271 asks

The roadmap entry: in project mode the MCP server creates a case in `cases/` (from a shipped example, a built-in template, TOML,
settings or a copy of a project case, never replacing a file) and starts a run of it through the runner, in one call if asked,
with the same start plan as the Studio, so the run's fields are stored and the Studio lists the run and can read its snapshots
(ADR 0173 R1, W3, C7, D7 and D2).

## What the code read shows (main 4e84c11, nothing run)

1. **The name `create_case` is taken.** `rcfd-mcp`'s session tool `create_case` (`src/lib.rs`) makes an in-memory session; it is
   the only tool that makes one, so `set_setting`, `check_mesh`, `run`, `status` and the other session tools need it first. With
   `--project` the list is the 33 session tools followed by the 17 project tools; `call_tool` takes the first tool of a name for
   the schema check, while `dispatch` asks the project tools first. Two tools of one name cannot both work.
2. **A C1 file uses the session `create_case` in project mode.** `crates/rcfd-mcp/tests/project_runs_w48f.rs` R1 (ADR 0135
   amendment 3) calls it with `{"path": "cases/cavity.toml"}` under `--project` to show that `--project` confines the session
   tools' paths. The same file asserts 50 tools under `--project` twice (F3/F11 and F13).
3. **The F-270 tool-list golden is pinned byte for byte.** `crates/rcfd-mcp/tests/project_list_f270.rs` (ADR 0185 A2) compares
   `tools/list` under `--project` with `golden/tools_list_project_f270.json` and asserts 50 tools. Any new project tool or
   argument changes it.
4. **The Studio's start plan.** `crates/rcfd-studio/src/runs.rs`: `case_facts` reads the case as `rcfd run` does
   (`preview::parse_case`: JSON when the text starts with `{`, else TOML); `propose_store` gives no store when the case sets
   `output.store_interval` or `output.store_time_interval` to a value other than its schema default (ADR 0152) or sets
   `dynamic_mesh.enabled`; a transient run (`piso`, `pimple`, `compressible` unless `compressible.mode = "steady"`) with an
   `end_time` gets `store_time_interval = end_time / 100`, every other run `store_interval = max(1, ceil(max_iterations / 100))`;
   precision f32 unless the case sets `output.store_precision`. `StartPlan::set()` puts these entries before the user's own;
   `snapshots()` and `estimated_bytes()` (cells x 4 fields x snapshots x 4 or 8 bytes) are the dialog's report. Run's time
   settings (ADR 0153) and pressure correctors (ADR 0158) are added by `quick_run` only, not by the start plan.
5. **The maintainer's uncommitted work** (worktree `studio-case-lineage`) modifies the Studio's `runs.rs`, `store_plan.rs`,
   `poll.rs`, `quick_run.rs` and several tests, and in `rcfd-mcp/src/project.rs` (the pre-F-270 file) the `threads` and `device`
   lines of `start_run`'s `RunSettings`, which now live in `rcfd-workspace/src/runs.rs`. It does not modify
   `case_starter.rs`.
6. **The examples are compiled into the Studio** (`case_starter.rs`: `Example`, `Companion`, `EXAMPLES`, `EXCLUDED`,
   `is_reference`, `Description` and the methods `stem`, `ext`, `bytes`, `parse`, `describe`, `describe_with`), with
   `include_str!("../../../cases/...")`. `Example::parse` reads TOML into a tree with the `toml` crate. Studio tests call them as
   `rcfd_studio::case_starter::...` and with method syntax (`e.describe_with(true)`).
7. **The built-in template** `lid_driven_cavity` is `rcfd_mcp::TEMPLATES` / `rcfd_mcp::LID_DRIVEN_CAVITY`; `rcfd-gui` uses the
   second.
8. **No example of `cases/` sets a store key**; `sod/sod.toml` (100 cells, `compressible`, `end_time = 0.2`) is the cheapest
   transient one.

## Choices of this registration

**G1. The examples move into the workspace (ADR 0173 W3, used).** `rcfd_workspace::examples` holds `Example`, `Companion`,
`EXAMPLES`, `EXCLUDED`, `is_reference`, `Description` and their methods, moved unchanged (text, order, messages). The Studio's
`case_starter.rs` re-exports them under the same names; nothing else of the Studio changes for this. `rcfd-workspace` gains the
dependency `toml` (`toml.workspace = true`, already in `Cargo.lock` through `rcfd-api` and `rcfd-studio`), which `Example::parse`
needs: the only change to `Cargo.lock` is that name in `rcfd-workspace`'s dependency list.

**G2. The built-in template moves into the workspace.** `rcfd_workspace::templates::{TEMPLATES, LID_DRIVEN_CAVITY}`, the same
text; `rcfd-mcp` re-exports both at `rcfd_mcp::TEMPLATES` and `rcfd_mcp::LID_DRIVEN_CAVITY`, so the session tools and
`rcfd-gui` are unchanged.

**G3. A new workspace command and project tool `new_case` (Q1 (a)).** Arguments: `name` (required) and exactly one source:
`example` (a source path of `EXAMPLES`, e.g. `sod/sod.toml`), `template` (a name of `TEMPLATES`), `toml` (case text),
`settings` (a settings object) or `copy` (a project-relative case file). The new file is `cases/<name>.<ext>`: the example's or
the copied file's extension, `.toml` for a template or TOML, `.json` for settings. Rules, in this order, nothing written before
the last step:
* exactly one source, else -32602; an unknown example or template -32602 with the list;
* the name by `rcfd_workspace::cases::check_name` against the files directly under `cases/` (letter case ignored, `.toml` and
  `.json` both), refused with 1004 and the rule's message;
* the bytes: an example's and a template's text unchanged, the TOML text as given, the settings object as pretty JSON with a
  final newline (not `Case::to_toml`, which spells out every default; ADR 0135 amendment 3 item 3), a copy's bytes unchanged
  (missing source: 1202);
* validation through `rcfd-api` with `cases/` as the base folder and the project as root (`Case::parse_toml_with` /
  `parse_json_with`, JSON when the text starts with `{` as `rcfd run` reads it, then `semantic_errors`): a refusal is 1003 with
  the located diagnostics. An example is validated by `Example::parse` (its data files inline), as the case starter offers only
  examples that parse;
* the write: `rcfd_workspace::cases::create_with` (never replaces a file; an example's data files next to the case first, an
  equal one reused, one with other bytes refusing the creation).
Result: `path`, `hash` (of the new file: the `base_hash` for `edit_case`), `bytes`, `from` (`example <source>`,
`template <name>`, `toml`, `settings` or `copy <path>`), `data_files` [{`path`, `written`}]. No journal line (a `case_created`
line is ADR 0173 W6, F-273), as for the Studio's creations (ADR 0185 E6). Access class `write_files`.

**G4. The start plan in the workspace (ADR 0173 D7 (a), used as far as the house rules allow).** `rcfd_workspace::plan` gives
the field-store proposal of item 4 for a parsed case: the same rule, constants (100 snapshots, 4 fields), entry text and order,
`snapshots` and `estimated_bytes`, and the same notes when no store is proposed. Only ADR 0140's store proposal (`StartPlan::new`
followed by `set()`), not Run's time settings or correctors. The case is read as `rcfd run` reads it, and the caller's `set`
entries are applied to it first (`rcfd_api::chain::apply_set`, as `rcfd run --set` does), so a `solver.max_iterations` or
`solver.end_time` override is planned for; without entries the plan is the Studio's plan of the file. The Studio keeps its own
`propose_store` (its `runs.rs` and `store_plan.rs` are being changed in the maintainer's worktree, item 5) until it switches to the
workspace's; a parity test (C-P below) pins the two equal meanwhile.

**G5. `start_run` grows two arguments.** `new_case` (an object with `new_case`'s arguments): the case is made and then run, in one
call; `case` and `new_case` are exclusive and one is needed (-32602). `store`: `"plan"` applies the start plan, `"none"` does
not. Omitted, it is `"plan"` with `new_case` and `"none"` with `case` (Q2; one constant, `PLAN_FOR_PROJECT_CASES = false`).
The caller sets the store explicitly, so the plan is not applied (ADR 0173 D7's wording), when a `set` entry's path is
`output.store_interval` or `output.store_time_interval`. Order: the case text is made and validated (G3), the `set` entries are
checked against the case and the plan is made (a malformed or refused entry: 1003, nothing written or queued), the case is
written, then the run is submitted with the plan's entries before the caller's (the Studio's order) and the runner started. A
`case` with the plan is read and planned the same way; a case that does not parse is refused with 1003 before anything is
queued. The `RunSettings` construction of F-270 is kept line for line (only the `set` value it receives is longer), so the
maintainer's `threads`/`device` hunk still applies there. Result: `run`, `case`, `state` as before; plus `created` (the
`new_case` result) when a case was made; plus `store` {`applied`, `set` (the entries added), `snapshots`, `estimated_bytes`,
`note`} when the plan was asked for. A `start_run {case}` without `store` gives exactly `{run, case, state}` and the run record
it gave before F-271.

**G6. The MCP descriptors.** One new project tool `new_case` after `write_note`; `start_run`'s descriptor gains the properties
`new_case` and `store` and its `required` list becomes empty; its description and its other properties are unchanged. The
project sentence of `initialize`'s instructions names `new_case`. Without `--project` nothing changes (F1's golden).

**G7. The tool-list checks restated.** ADR 0185 A2's golden cannot hold after G6 by construction. It is not re-recorded; its
test (`project_list_f270.rs`) is changed to C-L below. The two "50 tools" assertions of `project_runs_w48f.rs` become 51. These
are the only existing tests F-271 edits.

**G8. Separable commits.** (1) this text; (2) the examples and template moved, `new_case`, the plan and `start_run`, the MCP
descriptors and their tests; (3) the Studio's re-export and the Studio test file; (4) documents and the addendum. Each builds
and passes on its own.

## Open questions

**Q1. The tool name (item 1, D2).** ADR 0173 D2 (b) hides the session `create_case` once the workspace's `create_case` passes C7.
Taken literally in F-271 that (i) breaks R1's probe in `project_runs_w48f.rs` (item 2) and (ii) leaves every session tool in
project mode without a way to make a session, so `check_mesh` and `run` of a session stop working before their project
equivalents exist (F-272, F-275). Options: (a) the project command is `new_case`; the session tools stay as they are until F-275
(W8) decides the project-mode list, which may rename it; (b) the project `create_case` replaces the session one under
`--project` now, with R1's probe rewritten; (c) one `create_case` with both meanings, chosen by its arguments.
Recommendation: (a), reversible (a rename). Proceeding on (a). D2 (b) is otherwise not applied in F-271 (`run` and `status`
stay; W8).

**Q2. D7's default for a project case.** ADR 0173 D7 (a) and the roadmap note say that applying the start plan to `start_run`
with no store argument changes what it writes for cases with no store setting, so it needs the maintainer's explicit approval.
Options: (a) the plan is on by default for every `start_run`; (b) on by default only where there was no behaviour before
(`new_case`), off for `case` unless `store: "plan"`; (c) off everywhere unless asked. Recommendation: (a) once the maintainer
approves; proceeding on (b), which leaves every existing call bit-identical. Reversible: one constant.

## Acceptance checks and limits (fixed here, before any code)

* **C-7 (C7, MCP binary).** A new project; through the `rcfd-mcp` binary, one call `start_run {new_case: {name: "sod", example:
  "sod/sod.toml"}}`. Pass: `cases/sod.toml` is byte-equal to the repository's `cases/sod/sod.toml`; the result has
  `created.path` `cases/sod.toml`, `store.applied` true, `store.set` `["output.store_time_interval=0.002",
  "output.store_precision=f32"]` and `store.snapshots` 100; `run.json`'s `settings.set` equals `store.set`; `wait_run` reaches
  `finished` within 300 s; `runs/<id>/fields/store.rts` holds n snapshots with 50 <= n <= 101; `field_summary {run, field: "p"}`
  returns a finite `min` and `max`. Controls in the same test: (i) `new_case {name: "sod-none", ...}` with `store: "none"`
  finishes with no `store.rts` and an empty `settings.set`; (ii) `start_run {case: "cases/sod.toml"}` returns exactly the keys
  `run`, `case`, `state` and its `settings.set` is empty (today's behaviour); (iii) `new_case {name: "SOD", example: ...}` again
  is refused with 1004, `cases/sod.toml` keeps its bytes and the run folders are the same as before the call.
* **C-S (C7, the Studio).** A project served by `rcfd-runner`; `Workspace::start_run` (the function the MCP tool calls) with a
  new case from `sod/sod.toml`. Pass: the first `RunPoller::tick` after the call lists the run with case `cases/<name>.toml`;
  after the runner client reports the run finished (cap 300 s), the next tick shows it `finished`; the Studio's store reader
  (`viewport::StoreView::open` on `runs/<id>/fields`) opens it and samples the last snapshot's `p` into a frame of finite values.
* **C-P (parity of the start plan).** For every entry of `EXAMPLES` (read from the repository's `cases/` folder) and nine
  variants (a steady case, a steady case with 250 iterations, `piso` with `end_time`, `piso` without, `dynamic_mesh.enabled`,
  a case setting `store_interval = 5`, one setting `store_time_interval`, one setting `store_precision = "f32"`, and the steady
  case written as JSON): the workspace's plan of the text gives the same entries, `snapshots`,
  `estimated_bytes` and "no store" verdict as `rcfd_studio::runs::StartPlan::new(...)` (`set()`, `snapshots()`,
  `estimated_bytes()`, `store.is_none()`). With an override `solver.max_iterations=40` on a 600-iteration case, the workspace's
  plan equals the Studio's plan of the text edited to 40; control: the workspace's plan without the override differs from it.
* **C-N (`new_case`, workspace).** Each source creates its file: an example byte-equal (and `channel/profile_inlet.toml` with
  `cases/profile_inlet.csv` byte-equal); the template byte-equal to `LID_DRIVEN_CAVITY`; TOML byte-equal to the text; settings
  as `.json` whose tree equals the object; a copy byte-equal to its source. Refused with nothing written (the listing of `cases/`
  is the same before and after): invalid TOML (1003, a diagnostic with a path or line), an invalid setting value (1003), an
  existing name in other letter case (1004), a name with a space (1004), two sources (-32602), an unknown example (-32602), a
  copy of a missing file (1202). A `start_run` with `new_case` and a malformed `set` entry writes no case and queues no run.
* **C-L (tool list).** Under `--project`: 51 tools; the list without `new_case` has the golden's names in the golden's order;
  every tool but `start_run` is byte-identical to its golden entry; `start_run`'s name, description, annotations, `_meta` and
  each golden property are byte-identical, its `required` is `[]` and its only added properties are `new_case` and `store`;
  `new_case` comes right after `write_note`, with access `write_files`. Without `--project`, `default_f1.rs` passes unchanged.
* **C-1 (parity).** With no edit to them: `project_w48f.rs`, `runner_stdio_w48l.rs`, `sv301_mcp_w48z.rs`, `correctors_w49j.rs`
  of `rcfd-mcp`, and `case_starter_g2.rs`, `editor_b5.rs`, `watch_b6.rs`, `run_control_r1_r3.rs`, `starter_run_m.rs`,
  `live_preview_j.rs`, `workspace_f270.rs` of `rcfd-studio` pass; `project_runs_w48f.rs` passes with only its two tool counts
  changed (G7); every test of `rcfd-workspace` and `rcfd-mcp` passes.
* **Defaults.** No solver result, case hash or existing case byte changes; `baseline_hashes.rs` untouched; `Cargo.lock` differs
  from main only in `rcfd-workspace`'s dependency list. The full gate (fmt per package, clippy with `-D warnings`, every test in
  release on a `git archive` copy, three jobs) reports no failure; a flaky failure is reported as a failure with its name.

## Not in F-271

The Studio switching to the workspace's start plan (after the maintainer's worktree lands), hiding session tools (W8, F-275),
journal lines and the feed (W6, F-273), mesh commands (W4, F-272), design studies on the runner (W7, F-274), and ADR 0185 Q1.

## Where to go next

* [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) for the layer and its decisions.
* [ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) for the crate this extends.
* [ADR 0140](0140-wave48g3-store-fields-by-default.md) for the store proposal the start plan follows.
* [MCP tools](../mcp-tools.md) for the project tools.

## Amendment 1 (2026-11-05, before the change it covers)

The first run of `rcfd-mcp`'s tests found a third count of the project-mode tool list that item 2 did not name:
`crates/rcfd-mcp/tests/project_w48f.rs` F3 asserts `33 + 17` tools for a library server with a project. G7 therefore covers a
third edit, that count to `33 + 18`, and C-1 moves `project_w48f.rs` from "with no edit" to "with only its tool count
changed". Nothing else of that file changes. Because F3's per-tool loop covers the 17 tools of its own list, the
read-only refusal of `new_case` is checked in `new_simulations_f271.rs` instead: under `Policy::read_only()` a `new_case`
call is refused with 1100 and the project folder is unchanged.

## Addendum 1: what was built, and the results (2026-11-05)

Branch `f271-new-simulations`, after this text and amendment 1: `85440ca` (the workspace and `rcfd-mcp`), `4afe4cf` (the Studio's
re-export and its test file), then the documents. Not merged.

**Built as registered.** G1 to G8 as written, with amendment 1. `rcfd-workspace` gains `examples.rs` (moved from the Studio,
text unchanged but two doc lines), `templates.rs` (moved from `rcfd-mcp`), `plan.rs` (the rule, `Plan::with_set` and
`parse_case`, which `start_run` calls and the parity test calls), `create.rs` (`new_case`) and the `start_run` changes in
`runs.rs`; the `RunSettings` lines of F-270 are unchanged. `rcfd-mcp` gains the `new_case` descriptor, `start_run`'s two
properties (written above `case`, so the `threads`/`mem_mb` lines of the maintainer's hunk are untouched) and the re-export of
the template. The Studio changes are `src/case_starter.rs` (the moved block replaced by one `pub use`, 3 lines of comment) and
the new `tests/new_simulations_f271.rs`. Existing tests edited: `project_list_f270.rs` (C-L), and the tool counts of
`project_runs_w48f.rs` (two) and `project_w48f.rs` (one, amendment 1).

**Results.**

| Check | Result |
|---|---|
| C-7 | one `start_run` call wrote `cases/sod.toml` byte-equal to the example and queued the run with `store.set` `["output.store_time_interval=0.002", "output.store_precision=f32"]`, `snapshots` 100, `estimated_bytes` 160 000; `run.json` holds the same entries; finished in 0.25 s with 100 snapshots; `field_summary` p min 0.1, max 1.0. Controls: `store: "none"` finished with no store and no entry; `start_run {case}` gave exactly `case`, `run`, `state` and no entry; `SOD` refused with 1004 through `start_run` and `new_case`, bytes and run folders unchanged; a `set` entry `output.store_interval=0` with `store: "plan"` gave `applied: false` and only that entry |
| C-S | the poller's first tick after the call listed the run (`cases/sod.toml`, queued); after it finished the next tick showed `finished`; `StoreView` read 100 snapshots and the last one's p (t = 0.2) as 100 finite face values in [0.1, 1.0] |
| C-P | all 10 examples and the 9 variants: the workspace's entries, snapshots, estimate, verdict and note equal the Studio's (3 variants without a store: the moving mesh and the two own-interval cases); `solver.max_iterations=40` on the 600-iteration case planned as the edited text (`store_interval=1`); control: without it the plans differ |
| C-N | five sources byte-equal (the settings tree equal), `profile_inlet.csv` written with its case; ten refusals with nothing written: invalid TOML 1003 (line 2), invalid `physics.nu` 1003 (located at the setting), `SOD` 1004, a space 1004, two sources and no source -32602, unknown example -32602, missing copy 1202, invalid copy 1003, `../x.toml` 1004; a malformed or unknown `set` entry with `new_case` 1003, no case and no run; no `edits.jsonl` line. Read-only server: `new_case` 1100, nothing written; control written |
| C-L | 51 tools, 42 103 bytes; the golden (38 978 bytes, not re-recorded) with exactly the registered changes is the live text byte for byte; without `--project`, `default_f1.rs` passes |
| C-1 | `project_w48f.rs` (count only), `project_runs_w48f.rs` (counts only), `runner_stdio_w48l.rs`, `sv301_mcp_w48z.rs`, `correctors_w49j.rs`, and the Studio files `case_starter_g2.rs`, `editor_b5.rs`, `watch_b6.rs`, `run_control_r1_r3.rs`, `starter_run_m.rs`, `live_preview_j.rs`, `workspace_f270.rs`, `start_plan_r10.rs` pass; every test of `rcfd-workspace` and `rcfd-mcp` passes (release); `workspace_graph.rs` 4 of 4 |
| Can fail | with the time rule's 100 changed to 50 and `PLAN_FOR_PROJECT_CASES` set to true, `plan_values`, C-7 and C-P failed (C-S, which reads whatever store is written, passed); restored |
| Defaults | `Cargo.lock`: only `toml` in `rcfd-workspace`'s list; `baseline_hashes.rs` untouched; no case-schema key or run-record field added; `scripts/limit_audit.py` reports none of the new tests |
