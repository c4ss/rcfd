# ADR 0185: ADR 0173 amendment 1, F-270 registration: the rcfd-workspace crate, with the MCP server and the Studio moved onto it

Status: **REGISTERED 2026-11-05**, written before any code of F-270. Not yet accepted by the maintainer.
Parent: [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) (proposed, not accepted),
work items W1, W2 and W5, checks C1, C2 and C6, roadmap item F-270. ADR 0173 says that every limit of its checks is fixed by an
amendment before any test or run; this is that amendment for F-270. Amendments of this text follow it, each written before what it
covers. Results go in the addendum at the end.

Read at main 7c960bb. Number: 0184 is the highest on main; 0182 is used on the unmerged branch `m0-gate-roadmap`; no worktree
holds a 0185.

ADR 0173 is not accepted. Where it gives a recommendation, this text uses it and says so; that is a recommendation used, not a
maintainer decision. Where the code read below raises a question ADR 0173 does not answer, the options and a recommendation are
listed (Q1 to Q3), and work proceeds only on a choice that is reversible.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)): defaults stay bit-identical; no solver result, case
hash or file written by a client changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new third-party crate;
no commit trailers.

## What F-270 asks

The roadmap entry: one crate holds the commands, queries and write rule for a project folder, and both the MCP project tools and
the Studio use it. The existing project-tool and Studio tests pass unchanged; a Studio save and an `edit_case` on the same file
give one write and one conflict naming the current hash; a comment line in a case survives a Studio save; the crate graph stays
acyclic with `rcfd-studio` not depending on `rcfd-mcp`.

ADR 0173's work items: W1 extract `rcfd-workspace` from `rcfd-mcp/src/project.rs` with no behaviour change; W2 `rcfd-mcp` calls
the workspace; W5 the Studio creates, saves and starts through the workspace, and `save_if_base` and the Studio's whole-file
write go.

## What the code read shows (main 7c960bb, nothing run)

1. `crates/rcfd-mcp/src/project.rs` (1,286 lines) holds the 17 project tools: the MCP descriptors (`fn tools`, names,
   descriptions, input schemas, access classes) and `ProjectTools`, whose methods do the work and return MCP result values
   (`ok_result`, an image content block for `render_field`). `crates/rcfd-mcp/src/case_edit.rs` (262 lines) is the line-level
   write rule of `edit_case` (ADR 0135 amendment 1 item 5, amendment 3 item 3).
2. Public items of `rcfd_mcp::project` used outside the module: `ProjectTools::open`, `with_runner_exe` and `root`
   (`src/main.rs`, `tests/common/mod.rs`) and `residual_series` (`tests/project_w48f.rs` line 512). The golden
   `crates/rcfd-mcp/tests/golden/tools_list_default.json` pins `tools/list` without `--project`; nothing pins it with
   `--project`.
3. The Studio saves through its project worker (`crates/rcfd-studio/src/watch.rs`, `Command::Save {rel, base, bytes}`), which
   calls `save_if_base` (`case_edit.rs` line 54): read, compare the hash, `atomic_write`. No edit lock, no `edits.jsonl` entry.
   The bytes come from `CaseEditor::to_bytes` (`Case::to_toml` or `to_json_string` of the whole settings tree).
4. **The Studio's saved bytes are pinned by the preview and by tests.** `preview::editor_bytes` makes the same whole-file text
   to build the mesh of an unsaved edit; after a save, the preview compares the file's hash with that text's hash and skips the
   rebuild ([ADR 0147](0147-job48m-run-gate-camera-skip-starter-data-files-run-order.md) Q2 (b), the maintainer's answer
   recorded in [ADR 0148](0148-job48n-selected-case-middle-drag-units-scale-refine.md) decision 4).
   Tests that depend on the saved file being exactly `to_bytes()`: `crates/rcfd-studio/tests/live_preview_j.rs` J7 ("saved
   bytes" equal to `to_bytes()`, and the preview's case hash equal to their hash), `followups_n.rs` N7 ("a save of the shown
   text is not built again"), and `case_starter_g2.rs` `cs2_control_the_editors_save_bytes_are_not_byte_equal`, which asserts
   that the editor's save bytes of an unedited example differ from the example; `case_starter_g2.rs` is one of the C1 files.
   A save that changes only the lines of the changed settings (ADR 0173 D6 (a)) writes other bytes, so J7, N7 and CS2 cannot
   all pass unchanged with it. ADR 0173 did not list this; it is Q1 below.
5. The Studio creates a case through the worker (`Command::Create`) with `case_starter::create_with`: names checked, the case
   and its data files created with `rcfd_project::atomic::write_new` (never replaces), data files equal to the new ones reused,
   written ones removed again when the case cannot be written. Tests call `case_starter::create` and `create_with` with a
   `&Project`.
6. The Studio starts, holds, resumes and cancels runs with `rcfd_runner::client::Client`, built in `poll.rs`, `quick_run.rs`,
   `store_plan.rs` (`rerun`) and `stop.rs`. The MCP server uses the same client.
7. `crates/rcfd-cli/tests/workspace_graph.rs` panics on a crate name its `layer` function does not know, and counts every
   dependency `cargo metadata` lists, development dependencies included. The layering rule of the
   [architecture page](../architecture.md) puts `api`/`render` at one layer and `mcp`/`cli` at the next, so a crate between them
   needs the layers above renumbered.
8. The maintainer's uncommitted work in the worktree `studio-case-lineage` edits `start_run` in `rcfd-mcp/src/project.rs`
   (`threads` optional, a `device` argument) and `RunSettings` in `rcfd-runner`. Moving `start_run`'s body into the new crate means
   that hunk is applied there instead. Its other Studio edits do not touch `case_edit.rs`, `watch.rs`, `case_starter.rs` or
   `preview.rs`; they touch `poll.rs`, `quick_run.rs`, `stop.rs`, `store_plan.rs` and `studio.rs`, not on the import lines.

## Choices of this registration

**E1. The crate (ADR 0173 D1 (a), its recommendation, used).** A new crate `crates/rcfd-workspace` at a new layer between
`api`/`render` and `mcp`/`cli`. Dependencies: `rcfd-api`, `rcfd-project`, `rcfd-runner`, `rcfd-tsstore`, `rcfd-post`, `serde_json`;
all in the workspace already, no third-party crate added. `rcfd-io` (named by D1) is added when W4 uses it. `rust-version` 1.85
as `rcfd-mcp` (the `toml` closure of `rcfd-api`). The new package changes `Cargo.lock` (its own entry and the dependency lists of
`rcfd-mcp` and `rcfd-studio`, nothing else); with D1 (a) that is unavoidable, and the fast tier then selects every package.

**E2. What moves (W1).** Everything `ProjectTools` does moves into `rcfd_workspace::Workspace`: opening the project, the runner
client, the watcher, the 11 queries, `edit_case`, `write_note`, `start_run`, the run controls and `wait_run`, with their limits
(`TEXT_BUDGET`, `FILE_PAGE`, `MAX_WAIT_S`), the series decimation, and the error codes (1002, 1003, 1004, 1200, 1201, 1202,
-32000, -32602). `case_edit.rs` moves whole (`rcfd_workspace::case_text`). The workspace returns the result object of each call
(the `structuredContent` the MCP tool gave) or, for `render_field`, the PNG with its metadata; errors are
`rcfd_api::service::RpcError` with the same codes, messages and data. The text of every query and command is kept as it is.

**E3. What stays in rcfd-mcp (W2), reversible.** The 17 tool descriptors stay byte-for-byte in `rcfd-mcp/src/project.rs`, and
`ProjectTools` becomes the adapter: it holds a `Workspace`, sends each call to it and wraps the answer as an MCP result.
`ProjectTools::open`, `with_runner_exe`, `root`, and `project::residual_series`, `decimate_minmax`, `CONFLICT`, `LOCK_BUSY`,
`NOT_FOUND`, `TEXT_BUDGET`, `FILE_PAGE` and `MAX_WAIT_S` stay reachable (re-exported). Building the tool list from descriptors in
the workspace (ADR 0173 "Edges") is left to W8 (F-275).

**E4. Studio saves (W5).** The project worker's `Command::Save` calls `Workspace::save_case(rel, base, bytes, form)`, which writes
through `rcfd_project::atomic::replace_if_hash`: under the file's edit lock, only if the file still has `base` (an empty base:
only if there is no file), one `.rcfd/edits.jsonl` line per accepted write. A mismatch is a conflict naming the current hash; the
worker then reads the current bytes for the editor's conflict state, as `save_if_base` gave them. A busy lock (2 s) is a failed
save with its located message. `save_if_base` and `SaveOutcome` are removed. The bytes are the ones the Studio writes today
(`form` = `Whole`, the text of `CaseEditor::to_bytes`), so the preview's rule of item 4 and every test that reads saved bytes see
what they see today.

**E5. The line-level save (ADR 0173 D6 (a), its recommendation, built but not switched on in the Studio).**
`save_case` with `form` = `Lines` reads the base file, finds the settings whose values differ between the base file and the
saved text (a setting under an optional table the base file does not hold is compared with the default it reads as), changes
those lines with the rule of `edit_case`, re-parses the result and compares it with the saved text's settings tree. When they are
equal it writes that text (comments and layout kept, `form: "lines"`); when they are not, or a change is a removal, it writes the
saved text (Q2). The Studio keeps `Whole` until Q1 is answered.

**E6. Studio creates (W5).** `case_starter::create_with`'s body moves to `rcfd_workspace::cases::create_with(root, rel, bytes,
companions)` with the same checks, order, messages and never-replace writes; `case_starter::create` and `create_with` keep their
signatures and call it. No journal line for a creation (that is W6's `case_created`). The shipped examples stay in the Studio
(W3).

**E7. Studio starts (W5).** `rcfd_workspace::runs::RunService` is a cloneable handle with the runner client's method names
(`new`, `with_default_runner`, `project`, `runner_running`, `ensure_runner`, `submit`, `start`, `list`, `status`, `pause`, `resume`,
`cancel`, `wait`), each passing the call through. The four Studio files of item 6 import it in place of
`rcfd_runner::client::Client` (one import line each; the rest of each file is unchanged). The Studio's start plan stays where it
is (D7 is F-271); a start writes no journal line (W6).

**E8. The crate graph (C6).** `workspace_graph.rs` gets `rcfd-workspace` at layer 6 and the layers above it renumbered
(`mcp`/`cli` 7, `assistant` 8, `gui` 9, `studio` 10), plus one new test: `rcfd-studio` does not depend on `rcfd-mcp` (the layer
order alone allows it). This is the one existing test file F-270 edits. The architecture page gets the new crate and rule.

**E9. Separable commits.** (1) this text; (2) W1 and W2: the crate, `rcfd-mcp` on it, the graph test and the architecture page;
(3) Studio saves and creates; (4) Studio starts. Each builds and passes on its own, so the maintainer can take (2) and leave (3) or
(4) while the Studio files are being changed elsewhere.

## Open questions

**Q1. D6 against the preview's save rule (item 4).** (a) The Studio writes the line-level form: `CaseEditor::to_bytes` and
`preview::editor_bytes` both make it (the editor keeps its base bytes), so J7 and N7 hold, but the CS2 control's premise turns
(an unedited editor would write the file's own bytes) and tests that read the editor's text may change; the maintainer accepts
those test changes. (b) The Studio keeps the whole-file text; "a comment line survives a Studio save" leaves F-270. (c) The Studio
saves the line-level form and the preview builds again after every save; N7 and J7 fail. Recommendation: (a), as its own change
after the maintainer's answer, since it changes the bytes the Studio writes. Proceeding now on neither: the Studio writes what it
writes today (E4), and the line-level save exists and is checked in the workspace (E5). Reversible: one constant.

**Q2. A change the line rule cannot place** (a removed patch or report, a value over several lines), in the `Lines` form.
(a) Write the whole saved text under the same lock and journal line, reported as `form: "whole"`. (b) Refuse it, as `edit_case`
does. (c) Teach the line rule removals. Recommendation: (a), so no Studio edit becomes unsaveable; built that way, reversible.

**Q3. `who` of a Studio save in `edits.jsonl`.** `"save_case"`, the workspace command's name, as `"edit_case"` and `"write_note"`
name theirs, until D4's actor (F-273) says which client. Reversible.

## Acceptance checks and limits (fixed here, before any code)

* **A1, C1 parity.** With no edit to them: the MCP files `project_w48f.rs`, `project_runs_w48f.rs`, `runner_stdio_w48l.rs`,
  `sv301_mcp_w48z.rs`, `correctors_w49j.rs` and the Studio files `editor_b5.rs`, `watch_b6.rs`, `run_control_r1_r3.rs`,
  `case_starter_g2.rs` pass, and so does every other test of `rcfd-mcp`, `rcfd-studio`, `rcfd-project` and `rcfd-runner`.
* **A2, tool list.** With `--project`, `tools/list` of the `rcfd-mcp` binary is byte-identical to a golden recorded from the binary
  of main 7c960bb before any change (`crates/rcfd-mcp/tests/golden/tools_list_project_f270.json`, never re-recorded); without
  `--project`, the existing golden holds (`default_f1.rs`).
* **A3, C2 race, Studio route.** The file's edit lock is held by another writer; the Studio save is sent; that writer replaces the
  file and releases the lock within 300 ms (the wait limit of the lock is 2 s). Pass: the Studio editor ends in the conflict state
  holding the other writer's bytes, the file holds them, and the Studio's save added no journal line. Control (the route before
  F-270: `save_if_base`'s compare-then-write, copied into the test), same schedule: the save is written and then replaced, two
  writes and no conflict.
* **A4, C2 in both orders.** Through the Studio worker and the workspace's `edit_case` (the function the MCP tool calls), each order
  from the same base: the second gives a conflict naming the first's new hash (`data.current_hash` 1200 for `edit_case`, the
  conflict state for the Studio), the file holds the first's bytes, and `edits.jsonl` gained exactly one line, whose `who` names
  the first writer.
* **A5, a comment survives (workspace).** `save_case` with `Lines` on a case with a comment line and one changed setting keeps the
  comment and every other line, and the file re-parses to the saved settings. Control: `Whole` loses the comment. A removed report
  in `Lines` is written whole (Q2 (a)) and re-parses to the saved settings.
* **A6, C6.** `workspace_graph.rs` passes with the new crate; `rcfd-studio` has no dependency on `rcfd-mcp`.
* **A7, no new crate.** `Cargo.lock` differs from main only in `rcfd-*` entries.
* **Defaults.** No solver result, case hash or saved byte changes; `baseline_hashes.rs` untouched. The full gate (fmt, clippy with
  `-D warnings`, every test in release on a `git archive` copy, three jobs) reports no failure; a flaky failure is reported as a
  failure with its name.

## Not in F-270

W3 (new cases from the MCP server, the shared start plan, D7), W4 (mesh commands), W6 (journal fields, feed, the Studio's
reactions), W7 (design evaluations on the runner), W8 (the project-mode tool list and the plugin hook). The Studio's watcher and
run poller stay as they are.

## Where to go next

* [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) for the layer this begins.
* [MCP tools](../mcp-tools.md) for the project tools the crate now serves.
* [Architecture and crate graph](../architecture.md) for the layering rule.

## Addendum 1: what was built, and the results (2026-11-05)

Branch `f270-workspace`, four commits after this text, as E9 planned: the crate and `rcfd-mcp` on it (W1, W2); Studio saves
and creates; Studio starts. Not merged; the Studio files are being changed in another worktree.

**Built as registered.** E1 to E8 as written. `rcfd-mcp/src/project.rs` keeps the 17 descriptors byte-for-byte and is now an
adapter around them (272 lines, from 1,286); `case_edit.rs` moved whole to `rcfd_workspace::case_text` (its 4 unit tests with it), the
setting loop of `edit_case` became `case_text::apply_sets` (the same calls in the same order). The Studio changes are: the
worker's `Save` arm (`watch.rs`), the removal of `save_if_base` and `SaveOutcome` (`case_edit.rs`), the name rule and
`create_with` re-exported or wrapped (`case_starter.rs`), one dependency line, and one import line in each of `poll.rs`,
`quick_run.rs`, `store_plan.rs` and `stop.rs` (`rcfd_workspace::runs::RunService as Client`).

**One difference found while building.** `replace_if_hash` creates a missing parent folder; `save_if_base` did not. A Studio save
of a case whose folder was deleted meanwhile now recreates the folder instead of failing. No test covers that case before or
after.

**Results.**

| Check | Result |
|---|---|
| A1 parity | the five MCP files and the four Studio files pass unedited; every test of `rcfd-mcp` passes (debug, with `rcfd` and `rcfd-runner` built); `cargo test --release -p rcfd-studio`: 280 passed, 21 ignored, 15 failed, all 15 because the `fake-claude` test binary of `rcfd-claude` is not built by a one-package run (the workspace gate builds it). A debug run of `case_starter_g2.rs` stopped at a `debug_assert!` inside epaint (a dropped `TexturesDelta` in headless frames), which release builds do not compile; the gate runs release |
| A2 tool list | `project_list_f270.rs`: 50 tools, 38 978 bytes, byte-identical to the golden of main 7c960bb; `default_f1.rs` passes |
| A3 race | the Studio's save waited for the held lock and ended 6 ms after its release, in the conflict state with the other writer's bytes; no journal line. Control: the route before wrote under the held lock, and its bytes were then lost |
| A4 both orders | workspace and Studio: the second writer gets a conflict naming the first one's hash (1200 with `data.current_hash`, or the editor's conflict state); one journal line, `who` `edit_case` or `save_case` |
| A5 comment | `Lines` keeps the comment line and every other line (one line changed; an absent optional table gets only its changed setting); control `Whole` loses it; a removed report is written whole |
| A6 graph | `workspace_graph.rs` 4 of 4, including the new rule that `rcfd-studio` does not depend on `rcfd-mcp`, `rcfd-gui` or `rcfd-assistant` |
| A7 lock file | `Cargo.lock` gains the `rcfd-workspace` entry and that name in the dependency lists of `rcfd-mcp` and `rcfd-studio`; nothing else |
| Scanner | `scripts/limit_audit.py` reports none of the new tests |
