# ADR 0173: One workspace layer between the project folder, the MCP server and the Studio

Status: **proposed 2026-11-04** (scoping only; nothing implemented or run). Not yet accepted by the maintainer. Decisions D1 to D6
below are open; each carries a recommendation. Parent: [the Studio plan](../ui-restart-plan.md), section 2 ("Edits made there
appear live in the app") and layer L7; [ADR 0135](0135-wave48f-plugin-and-project-tools.md).

Read at main edb4839. Number: 0172 is the highest on main; checked just before writing.

## The request (the maintainer, 2026-11-04)

Three layers: a deep layer that the MCP server works on, a middle layer that abstracts around it, and a Studio that reacts to
the changes the MCP server makes.

## Context (read at main edb4839, nothing run)

1. **Two tool lists in one server.** `fn tools()` (`crates/rcfd-mcp/src/lib.rs` line 74) lists 33 tools over in-memory
   `rcfd_api::service::Service` sessions. With `--project`, `all_tools` (line 626) adds the 17 tools of `project::tools()`
   (`crates/rcfd-mcp/src/project.rs` line 101). A Claude Code session with the plugin sees all 50. The three skills in
   `clients/claude-plugin/skills/` allow only project tools in `allowed-tools`, but nothing stops a session calling the other 33.
2. **Session tools keep their state in the server process.** A session run is a thread of that process
   (`crates/rcfd-api/src/service.rs` line 1573, `svc.worker`). `journal.get` can rebuild a session's settings, not its results,
   so session state and session runs end with the process. A design study writes its own folder (`design.create_study {dir}`,
   `crates/rcfd-api/src/design/tools.rs` line 630) and the ledger is a file ([ADR 0121](0121-wave46d-design-ledger-and-tolerance-study-driver.md)),
   but each evaluation is a solve inside the server process: `run_case` in `CaseEvaluator::evaluate`
   (`crates/rcfd-api/src/design/case_eval.rs` line 265), behind the `Evaluator` trait (`design/study.rs` line 249). Such
   evaluations have no run record, no stored fields to scrub, and are not counted by the runner's budget (`max_runs`, threads,
   `mem_budget_mb`, `crates/rcfd-runner/src/runner.rs` line 36). The Studio does not see any of it.
3. **Project tools work on files.** They read run records ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)), drive
   runs through the runner client ([ADR 0123](0123-wave47b-run-service.md), [ADR 0133](0133-wave48d-run-control-and-live-monitoring.md))
   and read the time-series store ([ADR 0124](0124-wave47c-time-series-field-store.md)). `edit_case` and `write_note` write
   through `rcfd_project::atomic::replace_if_hash` (`crates/rcfd-project/src/atomic.rs` line 326, called at `project.rs`
   line 1145). That function takes the edit lock under `.rcfd/locks/` and appends `{t, kind: "edit", who, path, old_hash,
   new_hash}` to `.rcfd/edits.jsonl` (`EDIT_LOG`, `atomic.rs` line 216). `who` is the command name (`"edit_case"`), not the
   session. No crate reads `.rcfd/edits.jsonl`.
4. **The Studio reaches the same folder through its own code.** `crates/rcfd-studio/src/poll.rs` (the runner client, every
   250 ms, `PERIOD` at line 25), `watch.rs` (the project watcher, `changes_since` at line 303) and `case_edit.rs`. Its saves go
   through `save_if_base` (`case_edit.rs` line 54): compare the hash, then `atomic_write`, with no edit lock and no
   `edits.jsonl` entry. The module comment says the cooperative lock of ADR 0135 replaces it when 48F merges. 48F has merged and
   the MCP server uses the lock, but the Studio does not yet. A Studio save and an `edit_case` on the same file can therefore
   still race, and Studio saves leave no record.
5. **Two rules for writing a case.** `crates/rcfd-mcp/src/case_edit.rs` changes the `key = value` line in place and keeps
   comments and layout (ADR 0135 amendment 1 item 5, amendment 3 item 3). The Studio editor regenerates the whole file from its
   settings tree (`Case::to_toml` or `to_json_string`, `crates/rcfd-studio/src/case_edit.rs` line 342), so comments, which are
   not part of the tree, do not survive a Studio save.
6. **The same reading code exists twice.** `rcfd-mcp/src/project.rs` has 1,286 lines. The Studio's `poll.rs`, `watch.rs` and
   `case_edit.rs` have 940, 334 and 434. The layering rules of [the architecture page](../architecture.md) forbid
   `rcfd-studio -> rcfd-mcp`, so neither crate offers a home for code both need.

## The problem in one table

| | The 33 session tools (`Service`) | The 17 project tools |
|---|---|---|
| Where state lives | The MCP server's memory | Files in the project folder |
| Where a solve runs | A thread of the MCP server | A child process of the runner |
| After Claude Code closes | Gone (settings only, through the journal) | Kept, with crash recovery |
| Counted by the runner's budget | No | Yes |
| Seen by the Studio | No | Yes, by polling |
| Who made a change | Not recorded | `edits.jsonl` names the command, not the session; Studio saves are not recorded |

## Decisions needed

### D1: where the shared layer lives

* (a) **A new crate `rcfd-workspace`** depending on `rcfd-project`, `rcfd-runner`, `rcfd-tsstore`, `rcfd-api` and `rcfd-post`.
  `rcfd-mcp` and `rcfd-studio` both depend on it.
* (b) Grow `rcfd-project`. It sits below `runner`, `api` and `tsstore` in the layering rules, so it cannot call them.
* (c) Keep two implementations and keep them in step by review.

Trade-offs: (b) breaks the layering rules; (c) is the state today, and items 4 and 5 of the context show it has already drifted.
**Recommendation: (a)**, placed above `api` and `render` and below `mcp` and `cli`. `rcfd-studio` still does not depend on
`rcfd-mcp`.

### D2: the 33 session tools when `--project` is given

* (a) Hide all 33 in project mode.
* (b) **Move the design tools onto the runner** (see "Design studies on the runner" below) **and hide the rest** in project
  mode.
* (c) Keep all 33 and write each session's journal into the project folder.

Trade-offs: (a) loses the design loop, Claude's main way of exploring a design; (c) keeps two places where solves run and two
budgets. The interactive session tools (`run`, `status`, `get_residuals`) matter less once every run can be scrubbed in the
Studio.
**Recommendation: (b).** Without `--project` the tool list stays exactly as it is, so the command line, the TCP service,
`rcfd-assistant` and the frozen `rcfd-gui` are unchanged.

### D3: how changes reach the clients

* (a) **The files stay the truth and readers poll.** The workspace merges `edits.jsonl`, the run records and events, and the
  watcher's changes into one ordered feed.
* (b) As (a), plus the runner pushes run progress to subscribers over a local socket, with polling as the fallback. The runner
  has no socket today: clients write request files (`crates/rcfd-runner/src/client.rs`).
* (c) A project daemon owns every write and pushes every change.

Trade-offs: an MCP server starts and stops with each Claude Code session, so reads must work with no daemon running; (c) makes
every edit depend on one. (b) lowers the delay for run progress only.
**Recommendation: (a) now**, and (b) later if the delay is measured and found too long.

### D4: what identifies who made a change

* (a) The command name only (today's `who`).
* (b) **An additive `actor` field**: `{client, instance}`. `client` is `mcp`, `studio`, `cli` or, later, `python`. `instance` is
  the MCP server's process id and start time. The MCP server's parent process should be the Claude Code session, which
  `claude agents --json` lists with its `pid` and `sessionId` ([the Studio plan](../ui-restart-plan.md), section 7), so the
  Studio could map an MCP actor to a Claude Code session. NOT VERIFIED on an installed build.
* (c) Require a Claude Code session id. No documented way exists to pass one to a plugin's stdio server: ADR 0135 lists
  `${CLAUDE_PLUGIN_ROOT}`, `${CLAUDE_PLUGIN_DATA}` and `${CLAUDE_PROJECT_DIR}` as the substituted values.

**Recommendation: (b)**, keeping `who` for the command name. Changes found by the watcher with no matching journal entry are
labelled `external`.

### D5: Claude Code's own Edit and Write tools on project files

* (a) Allow them; the watcher detects the change after the fact and labels it `external` (today's behaviour).
* (b) **A plugin `PreToolUse` hook that refuses Edit and Write under `cases/`, `runs/` and `designs/`**, with a message that
  names `edit_case`. Notes and other files stay editable.
* (c) Refuse every Edit and Write in the project folder.

Trade-offs: (a) lets a change skip validation and the edit lock; (c) blocks ordinary note-taking. The hook contract (input
shape, how a refusal is returned) is to be read from the Claude Code hooks documentation and checked on an installed build
before (b) is built. NOT VERIFIED here.
**Recommendation: (b).**

### D6: which rule for writing a case survives

* (a) **The MCP rule** (change the lines of the settings that changed, keep comments and layout) for every writer.
* (b) The Studio rule (regenerate the whole file).

**Recommendation: (a).** The Studio editor keeps its schema-tree interface but saves through the workspace's `edit_case`, which
receives the changed settings, not the whole file.

## Design (proposed, assuming the recommendations)

```text
  Claude Code --stdio--> rcfd-mcp (project mode)        rcfd-studio <-- user
                              | adapter: tools are         | adapter: draws from the feed,
                              | commands, queries, feed    | sends the same commands
                              v                            v
                 +--------------------------------------------------+
  middle         | rcfd-workspace                                   |
                 |   commands: edit_case, write_note, start_run,    |
                 |     pause, resume, cancel, create_study,         |
                 |     run_study, record_design                     |
                 |   queries: project_info, list_runs, run_status,  |
                 |     metrics, residuals, field_summary, render,   |
                 |     compare_runs, read_project_file              |
                 |   feed: ordered, typed changes with an actor     |
                 |   one write rule, edit lock, limits, path safety |
                 +--------------------------------------------------+
                              | writes                     ^ reads; detects outside edits
                              v                            |
                 +--------------------------------------------------+
  deep           | project folder + runner + time-series store      |
                 |   the only state; .rcfd/edits.jsonl is the       |
                 |   project journal                                |
                 +--------------------------------------------------+
  rcfd-api Service: validation, schema and solving underneath; in project mode it holds no state a user sees
```

**Deep layer.** The project folder, the runner and the store, unchanged, become the only place where state lives in project mode.
`.rcfd/edits.jsonl` becomes the project journal through additive changes only: an `actor` field (D4) and a `seq` number given
under a short project-wide append lock, and new `kind` values for `run_submitted`, `run_control`, `design_recorded` and
`note`. Existing entries stay readable. The runner's per-run `events.jsonl` files stay where they are.

**Middle layer, `rcfd-workspace`.** `Workspace::open(root, actor)` gives the commands, the queries and
`changes(since) -> (Vec<Change>, cursor)`. Every command checks paths with `rcfd_project::paths::resolve_in`, applies the limits
and writes through `replace_if_hash`. The feed merges the journal, the run records and events, and the watcher's
`changes_since`, labelling changes with no journal entry `external`. The crate has no UI and no MCP types.

**Design studies on the runner.** A `RunnerEvaluator` implements `Evaluator` by submitting each condition as a run and reading
`metrics.json` when it finishes. The run record gets additive `study` and `variant` fields; ADR 0130 requires such a change to go
through the lead. A chained study (`chain`) needs each run to start from the previous result. Whether the runner client can pass
`continue_from` today is not established: no use of it was found in `crates/rcfd-runner/src`.

**Edges.** In project mode `rcfd-mcp` builds its tool list from the workspace's command and query descriptors (schema and access
class), and `list_changes` returns the feed. `rcfd-studio` replaces the reading parts of `poll.rs` and `watch.rs` with a feed
consumer, and saves through `edit_case`, which closes the race of context item 4. Without `--project`, `rcfd-mcp` is unchanged.

## Order of work (each step ships on its own)

1. W1: extract `rcfd-workspace` from `rcfd-mcp/src/project.rs` with no behaviour change; the existing project-tool tests are
   the parity check.
2. W2: `rcfd-mcp` calls the workspace.
3. W3: the Studio calls the workspace and saves through `edit_case`; `save_if_base` and the Studio's whole-file write go.
4. W4: journal fields (`actor`, `seq`, new kinds) and the merged feed; the Studio shows who made each change.
5. W5: `RunnerEvaluator`; the design tools use it in project mode.
6. W6: the project-mode tool list (D2) and the plugin hook (D5).

## Registered checks (proposed; every limit is fixed by an amendment before any test or run)

* C1 parity: after W1 to W3, `crates/rcfd-mcp/tests/project_w48f.rs`, `project_runs_w48f.rs`, `runner_stdio_w48l.rs`,
  `sv301_mcp_w48z.rs` and `correctors_w49j.rs`, and `crates/rcfd-studio/tests/editor_b5.rs`, `watch_b6.rs` and
  `run_control_r1_r3.rs`, pass unchanged.
* C2 one write rule: a Studio save and an `edit_case` on the same file at once give one write and one conflict naming the
  current hash. Each accepted write leaves one journal entry. A comment line in the case survives a Studio save.
* C3 feed: an `edit_case` by one process appears with its actor in a second process's feed within a number of poll periods fixed
  by the amendment (proposal: 4, so 1 s at 250 ms). A write by plain `std::fs::write` appears as `external`.
* C4 design on the runner: a three-point study through `design_run_study` in project mode produces three run folders tagged
  with study and variant, never more than `max_runs` at once. Their metrics match the in-process evaluator's for the same cases
  within a tolerance fixed by the amendment. The extra cost per evaluation is measured and recorded.
* C5 tool list: with `--project`, `tools/list` holds no tool whose state lives only in memory. Without `--project` it is
  byte-identical to main.
* C6 layering: `crates/rcfd-cli/tests/workspace_graph.rs` passes with the new crate, and `rcfd-studio` does not depend on
  `rcfd-mcp`.
* Defaults: no solver result, case hash or run output changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

## Out of scope (stated now)

* Python bindings (F-083, [ADR 0034](0034-wave20c-study-python.md) D5). The workspace is shaped so that bindings to it, not to
  the command line, can become a third client with actor `python`; that is a follow-on ADR.
* The runner push channel (D3 (b)), undo in the Studio (which the journal makes possible), MCP over HTTP, and any change to the
  session tools outside project mode.

## Risks

* Each runner evaluation starts an `rcfd` process, so a study of many small evaluations gets slower; C4 measures it.
* D4 depends on the process relationship between Claude Code and its stdio servers, which is not verified.
* The hook of D5 may surprise a user who asks Claude to edit a case by hand; its message must say what to use instead, and the
  user can turn it off.
* Feed reads grow with the number of runs. The watcher already keeps the hash of a file whose length and time are unchanged
  (`REUSE_AGE`, `crates/rcfd-project/src/watch.rs` line 22); the feed starts from that.

## Roadmap items

Added to [the roadmap](../ROADMAP.md) with this ADR. The maintainer set them as the current priority on 2026-11-04, ahead of the
FAN-01 diagnosis, in this order:

1. F-270: `rcfd-workspace` crate, with the MCP server and the Studio moved onto it (W1 to W3).
2. F-271: project journal and change feed with actor (W4).
3. F-272: design evaluations on the runner (W5).
4. F-273: project-mode tool list and plugin edit hook (W6).

## Where to go next

* [Architecture and crate graph](../architecture.md) for the layering rules the new crate joins.
* [MCP tools](../mcp-tools.md) and [the service protocol](../service-protocol.md) for the two tool families.
* [ADR 0130](0130-wave48a-overview-and-run-record-contract.md) for the run-record contract and the house rules.
