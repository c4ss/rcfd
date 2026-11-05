# ADR 0173: One workspace layer between the project folder, the MCP server and the Studio

Status: **proposed 2026-11-04, revised the same day** for the maintainer's requirements R1 to R3 (scoping only; nothing
implemented or run). Not yet accepted by the maintainer. Decisions D1 to D8 below are open; each carries a recommendation.

In project mode today the MCP server and the Studio reach the same project folder through separate code, with separate rules
for writing a case, and the 33 session tools keep their state in the server process, where the Studio does not see it. The
maintainer requires that the MCP server can run new simulations directly (R1) and can edit and set up meshes (R2), and that the
Studio updates from the MCP server's changes (R3). This ADR proposes one new crate, `rcfd-workspace`, that both clients depend
on, so that in project mode a capability exists only as a workspace command that writes the project folder. The design below
assumes the recommendations. The maintainer set the ADR's roadmap items, F-270 to F-275, as the current priority on 2026-11-04,
ahead of the FAN-01 diagnosis.

Parent: [the Studio plan](../ui-restart-plan.md), section 2 ("Edits made there appear live in the app"), section 8 (the
project folder contract) and layer L7; [ADR 0135](0135-wave48f-plugin-and-project-tools.md).

Read at main edb4839. Number: 0172 is the highest on main; checked just before writing.

## The request (the maintainer, 2026-11-04)

The maintainer asked for three layers: a deep layer that the MCP server works on, a middle layer that abstracts around it, and
a Studio that reacts to the changes the MCP server makes. The maintainer then set three requirements:

* **R1** The MCP server can run new simulations directly.
* **R2** The MCP server can edit and set up meshes.
* **R3** The Studio updates from the MCP server's changes.

All of it follows from the project structure, the folder contract of [the Studio plan](../ui-restart-plan.md) section 8.

This ADR draws one principle from that: in project mode, a capability exists only as a workspace command that writes the
project folder, and the MCP server and the Studio offer the same commands. So whatever Claude changes, the Studio shows, and
whatever the Studio can change, Claude can change.

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
4. **The Studio reaches the same folder through its own code.** It uses `crates/rcfd-studio/src/poll.rs` (the runner client,
   every 250 ms, `PERIOD` at line 25), `watch.rs` (the project watcher, `changes_since` at line 303) and `case_edit.rs`.
   Its saves go through `save_if_base` (`case_edit.rs` line 54): compare the hash, then `atomic_write`, with no edit lock and
   no `edits.jsonl` entry. The module comment says the cooperative lock of ADR 0135 replaces it when 48F merges. 48F has
   merged and the MCP server uses the lock, but the Studio does not yet. A Studio save and an `edit_case` on the same file can
   therefore still race, and Studio saves leave no record.
5. **Two rules for writing a case.** `crates/rcfd-mcp/src/case_edit.rs` changes the `key = value` line in place and keeps
   comments and layout (ADR 0135 amendment 1 item 5, amendment 3 item 3). The Studio editor regenerates the whole file from its
   settings tree (`Case::to_toml` or `to_json_string`, `crates/rcfd-studio/src/case_edit.rs` line 342), so comments, which are
   not part of the tree, do not survive a Studio save.
6. **The same reading code exists twice.** `rcfd-mcp/src/project.rs` has 1,286 lines. The Studio's `poll.rs`, `watch.rs` and
   `case_edit.rs` have 940, 334 and 434. The layering rules of [the architecture page](../architecture.md) forbid
   `rcfd-studio -> rcfd-mcp`, so neither crate offers a home for code both need.
7. **In project mode, only the Studio can create a case (R1).** The case starter writes `cases/<name>.toml` from the shipped
   examples compiled into the Studio, never replacing a file ([ADR 0139](0139-wave48g2-case-starter.md),
   `crates/rcfd-studio/src/case_starter.rs`). No project tool creates a case; `create_case` (`lib.rs` line 83) is a session tool
   and writes nothing to the folder.
8. **Runs started by Claude may not be scrubbable (R1, R3).** The Studio starts a run with a start plan that proposes a field
   store ([ADR 0140](0140-wave48g3-store-fields-by-default.md), `propose_store` in `crates/rcfd-studio/src/runs.rs` line 342)
   and can follow it in the viewport ([ADR 0141](0141-wave48g4-one-click-run-and-follow.md)). The MCP `start_run` has no such
   plan, so a run Claude starts stores fields only when the case sets `output.store_interval` or `output.store_time_interval`
   (the message at `project.rs` line 730). Without stored fields the Studio cannot scrub the run.
9. **In project mode, only the Studio can look at a case's mesh, and neither client can check or import one (R2).** A case
   defines its mesh through settings: `mesh.generator` and its parameters, `mesh.source = "file"` with `mesh.file`,
   `mesh.scale`, `[mesh.transform]` ([ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md)) and `mesh.wall_layers`
   ([the case schema](../case-schema.md)). `edit_case` can already change those settings. The mesh preview of a case is a
   Studio feature drawn with wgpu ([ADR 0138](0138-wave48g1-mesh-preview-from-case.md)). `check_mesh` and `load_mesh`
   (`lib.rs` lines 122 and 111) are session tools. Files are read by `rcfd-io`: Fluent `.msh`/`.cas` ([mesh import](../mesh-import.md)),
   Gmsh and OpenFOAM. The surface-to-volume pipeline (`rcfd_mesh::tetmesh`, `surface_remesh`, [meshing](../meshing.md)) is not
   used by `rcfd-api` or `rcfd-cli`, so no case or command reaches it. The folder contract names "mesh files or recipes" under
   `meshes/`, but `rcfd-project` defines no recipe format.

## The problem in two tables

| | The 33 session tools (`Service`) | The 17 project tools |
|---|---|---|
| Where state lives | The MCP server's memory | Files in the project folder |
| Where a solve runs | A thread of the MCP server | A child process of the runner |
| After Claude Code closes | Gone (settings only, through the journal) | Kept, with crash recovery |
| Counted by the runner's budget | No | Yes |
| Seen by the Studio | No | Yes, by polling |

| In a project | Studio | MCP project tools |
|---|---|---|
| Create a case | Case starter, from the shipped examples | No tool |
| Edit case settings | Schema-tree editor, whole-file write, no lock | `edit_case`, line-level write, edit lock |
| Look at or check a case's mesh | Mesh preview (wgpu) | No tool |
| Bring in a mesh file | Lists `meshes/`, no import | No tool |
| Start a run | Start plan with a proposed field store | `start_run`, fields only if the case asks |
| Record who changed what, and why | Not recorded | The command name only |

## Decisions needed

### D1: where the shared layer lives

* (a) **A new crate `rcfd-workspace`** depending on `rcfd-project`, `rcfd-runner`, `rcfd-tsstore`, `rcfd-api`, `rcfd-io` and
  `rcfd-post`. `rcfd-mcp` and `rcfd-studio` both depend on it.
* (b) Grow `rcfd-project`. It sits below `runner`, `api` and `tsstore` in the layering rules, so it cannot call them.
* (c) Keep two implementations and keep them in step by review.

Trade-offs: (b) breaks the layering rules; (c) is the state today, and context items 4, 5, 7 and 8 show it has already drifted.
**Recommendation: (a)**, placed above `api` and `render` and below `mcp` and `cli`. `rcfd-studio` still does not depend on
`rcfd-mcp`.

### D2: the 33 session tools when `--project` is given

* (a) Hide all 33 in project mode now.
* (b) **Hide each session tool once the workspace offers its project equivalent and that equivalent passes its check**:
  `create_case` after the workspace's `create_case` (C7), `check_mesh` and `load_mesh` after the mesh commands (C8), `run` and
  `status` after `start_run` with the start plan (C7), and the design tools once they run on the runner (C4).
* (c) Keep all 33 and write each session's journal into the project folder.

Trade-offs: (a) removes now what R1 and R2 need, before the project equivalents exist; (c) keeps two places where solves run
and two budgets. **Recommendation: (b).** Without `--project` the tool list stays exactly as it is, so the command line, the
TCP service, `rcfd-assistant` and the frozen `rcfd-gui` are unchanged.

### D3: how changes reach the clients

* (a) **The files stay the truth and readers poll.** The workspace merges `edits.jsonl`, the run records and events, and the
  watcher's changes into one ordered feed.
* (b) As (a), plus the runner pushes run progress to subscribers over a local socket, with polling as the fallback. The runner
  has no socket today: clients write request files (`crates/rcfd-runner/src/client.rs`).
* (c) A project daemon owns every write and pushes every change.

Trade-offs: an MCP server starts and stops with each Claude Code session, so reads must work with no daemon running; (c) makes
every edit depend on one. (b) lowers the delay for run progress only.
**Recommendation: (a) now**, and (b) later if the delay is measured and found too long.

### D4: what a journal entry records

* (a) Today's fields: time, command (`who`), path, hashes before and after.
* (b) (a) plus an **actor**: `{client, instance}`. `client` is `mcp`, `studio`, `cli` or, later, `python`. `instance` is the MCP
  server's process id and start time. The MCP server's parent process should be the Claude Code session, which
  `claude agents --json` lists with its `pid` and `sessionId` ([the Studio plan](../ui-restart-plan.md), section 7), so the
  Studio could map an MCP actor to a Claude Code session. NOT VERIFIED on an installed build.
* (c) **(b) plus what changed and why.** `changes` lists each setting with its path, old value and new value (`edit_case`
  receives them, so it can write them; a hash cannot be turned back into a value). A mesh import records the file's hash, cell,
  face and patch counts. `reason` is an optional short text that the caller gives with any command; the MCP tools take it as an
  argument, so Claude can say why it made a change.

A Claude Code session id cannot be required: no documented way exists to pass one to a plugin's stdio server. ADR 0135 lists
`${CLAUDE_PLUGIN_ROOT}`, `${CLAUDE_PLUGIN_DATA}` and `${CLAUDE_PROJECT_DIR}` as the substituted values.
**Recommendation: (c)**, keeping `who` for the command name, so the Studio can say "Claude set `boundaries.inlet.velocity`
from 8 to 12 m/s: to reach the target Reynolds number". Changes found by the watcher with no matching journal entry are
labelled `external` and carry the path only.

### D5: Claude Code's own Edit and Write tools on project files

* (a) Allow them; the watcher detects the change after the fact and labels it `external` (today's behaviour).
* (b) **A plugin `PreToolUse` hook that refuses Edit and Write under `cases/`, `runs/` and `designs/`**, with a message that
  names `edit_case` and `create_case`. `meshes/`, `notes/` and other files stay writable, so Claude can still place a mesh file
  in the folder for `import_mesh`.
* (c) Refuse every Edit and Write in the project folder.

Trade-offs: (a) lets a change skip validation, the edit lock and the journal; (c) blocks ordinary note-taking and placing mesh
files. The hook contract (input shape, how a refusal is returned) is to be read from the Claude Code hooks documentation and
checked on an installed build before (b) is built. NOT VERIFIED here.
**Recommendation: (b).**

### D6: which rule for writing a case survives

* (a) **The MCP rule** (change the lines of the settings that changed, keep comments and layout) for every writer.
* (b) The Studio rule (regenerate the whole file).

**Recommendation: (a).** The Studio editor keeps its schema-tree interface but saves through the workspace's `edit_case`, which
receives the changed settings, not the whole file.

### D7: the start plan of a run Claude starts

* (a) **One start plan for every client.** The field-store proposal of ADR 0140 moves into the workspace, and `start_run` applies
  it unless the caller sets the store explicitly. The tool result reports the planned snapshot count and size.
* (b) Keep today's `start_run`: the case alone decides whether fields are stored.

Trade-offs: (b) leaves runs Claude starts unscrubbable unless the case asks for a store (context item 8), which defeats R3 for
the runs that matter most. (a) changes what `start_run` writes for cases with no store setting, so it needs the maintainer's
explicit approval under the house rules of ADR 0130 (defaults stay bit-identical unless a decision says otherwise).
**Recommendation: (a)**, with `store: "none"` as the caller's way back to today's behaviour.

### D8: what "set up meshes" covers now

* (a) **The case's mesh settings and mesh files, with commands to check, preview and import.** A generator's parameters, the
  mesh file, scale, transform and wall layers are case settings changed through `edit_case`. New commands check a case's mesh,
  draw it, and bring a mesh file into `meshes/` (design below).
* (b) (a) plus a mesh recipe format under `meshes/` that runs the surface-to-volume pipeline and caches the result.
* (c) Only `edit_case` on the mesh settings (today).

Trade-offs: (c) lets Claude change a mesh without seeing or checking it. (b) would expose meshing that no case or command
reaches today (context item 9), and needs a recipe format of its own.
**Recommendation: (a) now**, and (b) in a later ADR that defines the recipe format the folder contract names.

## Design (proposed, assuming the recommendations)

```text
  Claude Code --stdio--> rcfd-mcp (project mode)        rcfd-studio <-- user
                              | adapter: tools are         | adapter: draws from the feed,
                              | commands, queries, feed    | sends the same commands
                              v                            v
                 +--------------------------------------------------+
  middle         | rcfd-workspace                                   |
                 |   cases:  create_case, edit_case                 |
                 |   meshes: check_mesh, preview_mesh, import_mesh, |
                 |           list_meshes (settings via edit_case)   |
                 |   runs:   start_run (one start plan), pause,     |
                 |           resume, cancel; create_study,          |
                 |           run_study, record_design; write_note   |
                 |   queries: project_info, list_runs, run_status,  |
                 |     metrics, residuals, field_summary, render,   |
                 |     compare_runs, read_project_file              |
                 |   feed: ordered changes with actor, what, why    |
                 |   one write rule, edit lock, limits, path safety |
                 +--------------------------------------------------+
                              | writes                     ^ reads; detects outside edits
                              v                            |
                 +--------------------------------------------------+
  deep           | project folder + runner + time-series store      |
                 |   cases/ meshes/ runs/ designs/ notes/           |
                 |   .rcfd/edits.jsonl is the project journal       |
                 +--------------------------------------------------+
  rcfd-api Service: validation, schema, mesh building and solving underneath; in project mode it holds no state a user sees
```

**Deep layer.** The project folder, the runner and the store, unchanged, become the only place where state lives in project mode.
`.rcfd/edits.jsonl` becomes the project journal through additive changes only: the D4 fields (`actor`, `changes`, `reason`), a
`seq` number given under a short project-wide append lock, and new `kind` values for `case_created`, `mesh_imported`,
`run_submitted`, `run_control`, `design_recorded` and `note`. Existing entries stay readable. The runner's per-run
`events.jsonl` files stay where they are.

**Middle layer, `rcfd-workspace`.** `Workspace::open(root, actor)` gives the commands, the queries and
`changes(since) -> (Vec<Change>, cursor)`. Every command checks paths with `rcfd_project::paths::resolve_in`, validates through
`rcfd-api`, applies the limits, writes through `replace_if_hash` (or a write that never replaces, for new files) and appends to
the journal. Every command takes an optional `reason`. The crate has no UI and no MCP types.

**R1, new simulations.** `create_case` makes `cases/<name>.toml` from a shipped example (the case starter's examples and
companions move from the Studio into the workspace), from a built-in template, from TOML or a settings object, or as a copy of a
project case; it never replaces a file. `start_run` takes a project case and settings overrides, or a new case in one call
(created first, then submitted), and applies the start plan of D7. Every run goes through the runner, so it is counted by the
budget, survives Claude closing, and shows in the Studio.

**R2, meshes.** A case's settings set up its mesh, and `edit_case` changes them; when a `mesh.*` setting changes, the result
includes the mesh check of the new mesh. `check_mesh {case}` returns the quality report and the patch list (the patches are what
`boundaries.*` must name), the same report as `rcfd check-mesh`. `preview_mesh {case}` returns a PNG drawn by the CPU renderer
(`rcfd_post::render::render_scalar`, `crates/rcfd-post/src/render.rs` line 460) with a cell-quality measure as the colour; wgpu
stays out of the MCP server (ADR 0135 W48-12). `import_mesh {path}` reads a file already in the project folder with `rcfd-io`,
reports its cells, faces, patches and zones, and copies it into `meshes/` without replacing anything. `list_meshes` lists
`meshes/` with the cases that use each file. Large results stay within ADR 0135's result limit by returning summaries.

**R3, the Studio updates from every change.** Because every change lands in the folder and the journal, the Studio needs only
the feed:

| Change | What the Studio does |
|---|---|
| Case created | It appears in the project browser |
| Case edited | An open editor reloads it, or shows the conflict if it has unsaved edits (ADR 0131 B6); the mesh preview redraws when a `mesh.*` setting changed |
| Mesh imported | It appears under `meshes/` |
| Run submitted, state or progress | The run list and status strip update (ADR 0133); the viewport can follow it, as a user preference |
| Design recorded | The design ledger updates |
| Note written | It appears under `notes/` |

Each change shows its actor and, from D4 (c), what changed and why.

**Design studies on the runner.** A `RunnerEvaluator` implements `Evaluator` by submitting each condition as a run and reading
`metrics.json` when it finishes. The run record gets additive `study` and `variant` fields; ADR 0130 requires such a change to go
through the lead. A chained study (`chain`) needs each run to start from the previous result. Whether the runner client can pass
`continue_from` today is not established: no use of it was found in `crates/rcfd-runner/src`.

**Edges.** In project mode `rcfd-mcp` builds its tool list from the workspace's command and query descriptors (schema and access
class), and `list_changes` returns the feed. `rcfd-studio` replaces the reading parts of `poll.rs` and `watch.rs` with a feed
consumer, and creates, saves and starts through the workspace, which closes the race of context item 4. Without `--project`,
`rcfd-mcp` is unchanged.

## Order of work (each step ships on its own)

1. W1: extract `rcfd-workspace` from `rcfd-mcp/src/project.rs` with no behaviour change; the existing project-tool tests are
   the parity check.
2. W2: `rcfd-mcp` calls the workspace.
3. W3 (R1): `create_case` in the workspace (examples moved from the Studio), the shared start plan, and `start_run` with a new
   case.
4. W4 (R2): `check_mesh`, `preview_mesh`, `import_mesh` and `list_meshes`, and the mesh check in `edit_case` results.
5. W5: the Studio creates, saves and starts through the workspace; `save_if_base` and the Studio's whole-file write go.
6. W6 (R3): the journal fields of D4, the merged feed, and the Studio's reactions in the table above.
7. W7: `RunnerEvaluator`; the design tools use it in project mode.
8. W8: hide the session tools whose equivalents have passed (D2), and the plugin hook (D5).

## Registered checks (proposed; every limit is fixed by an amendment before any test or run)

* C1 parity: after W1, W2 and W5, `crates/rcfd-mcp/tests/project_w48f.rs`, `project_runs_w48f.rs`, `runner_stdio_w48l.rs`,
  `sv301_mcp_w48z.rs` and `correctors_w49j.rs`, and `crates/rcfd-studio/tests/editor_b5.rs`, `watch_b6.rs`,
  `run_control_r1_r3.rs` and `case_starter_g2.rs`, pass unchanged.
* C2 one write rule: a Studio save and an `edit_case` on the same file at once give one write and one conflict naming the
  current hash. Each accepted write leaves one journal entry. A comment line in the case survives a Studio save.
* C3 feed: an `edit_case` with a `reason`, made by one process, appears in a second process's feed with its actor, each changed
  setting's path, old and new value, and the reason, within a number of poll periods fixed by the amendment (proposal: 4, so
  1 s at 250 ms). A write by plain `std::fs::write` appears as `external`, with its path only.
* C4 design on the runner: a three-point study through `design_run_study` in project mode produces three run folders tagged
  with study and variant, never more than `max_runs` at once. Their metrics match the in-process evaluator's for the same cases
  within a tolerance fixed by the amendment. The extra cost per evaluation is measured and recorded.
* C5 tool list: after W8, with `--project`, `tools/list` holds no tool whose state lives only in memory. Without `--project` it is
  byte-identical to main.
* C6 layering: `crates/rcfd-cli/tests/workspace_graph.rs` passes with the new crate, and `rcfd-studio` does not depend on
  `rcfd-mcp`.
* C7 (R1): in a new project, one `start_run` call with a new case from a shipped example writes `cases/<name>.toml` byte-equal
  to the example, as the case starter does, and submits a run whose field store follows the start plan. The Studio's run
  poller, driven headless as its tests do, lists the run, and a snapshot can be read from its store.
* C8 (R2): importing the Gmsh and Fluent samples used by `crates/rcfd-io/tests/gmsh_samples.rs` and `fluent_public.rs` reports
  the cell, face and patch counts the readers give. `check_mesh` on a project case gives the same report as `rcfd check-mesh`
  on that case. `preview_mesh` returns a PNG whose statistics match a CPU reference within limits fixed by the amendment. An
  invalid `mesh.*` edit is refused with a located diagnostic and writes nothing.
* Defaults: no solver result or case hash changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded. Run output
  of `start_run` changes only through D7, once the maintainer approves it.

## Out of scope (stated now)

* Python bindings (F-083, [ADR 0034](0034-wave20c-study-python.md) D5). The workspace is shaped so that bindings to it, not to
  the command line, can become a third client with actor `python`; that is a follow-on ADR.
* A mesh recipe format and the surface-to-volume pipeline from the MCP server (D8 (b)).
* wgpu rendering in the MCP server, the runner push channel (D3 (b)), undo in the Studio (which the journal makes possible),
  MCP over HTTP, and any change to the session tools outside project mode.

## Risks

* Each runner evaluation starts an `rcfd` process, so a study of many small evaluations gets slower; C4 measures it.
* D7 makes Claude's runs larger on disk; the start plan's size report (ADR 0140) is in every `start_run` result.
* D4's link to a Claude Code session depends on the process relationship between Claude Code and its stdio servers, which is
  not verified.
* The hook of D5 may surprise a user who asks Claude to edit a case by hand; its message must say what to use instead, and the
  user can turn it off.
* Feed reads grow with the number of runs. The watcher already keeps the hash of a file whose length and time are unchanged
  (`REUSE_AGE`, `crates/rcfd-project/src/watch.rs` line 22); the feed starts from that.

## Roadmap items

Added to [the roadmap](../ROADMAP.md) with this ADR. The maintainer set them as the current priority on 2026-11-04, ahead of the
FAN-01 diagnosis, in this order:

1. F-270: `rcfd-workspace` crate, with the MCP server and the Studio moved onto it (W1, W2, W5).
2. F-271: new simulations from the MCP server in project mode (R1, W3).
3. F-272: mesh set-up from the MCP server in project mode (R2, W4).
4. F-273: project journal and change feed with who, what and why, and the Studio's reactions (R3, W6).
5. F-274: design evaluations on the runner (W7).
6. F-275: project-mode tool list and plugin edit hook (W8).

## Where to go next

* [Architecture and crate graph](../architecture.md) for the layering rules the new crate joins.
* [MCP tools](../mcp-tools.md) and [the service protocol](../service-protocol.md) for the two tool families.
* [ADR 0130](0130-wave48a-overview-and-run-record-contract.md) for the run-record contract and the house rules.
