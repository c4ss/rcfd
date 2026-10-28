# ADR 0135: Wave 48F, the Claude Code plugin: project tools in `rcfd-mcp`, skills, slash commands, file watching and edit conflicts

Status: **ACCEPTED 2026-10-28** (the maintainer accepted the ADR and took the recommended option of every open decision W48-1 to W48-16, scope cut (a)) (scoping only; nothing implemented or run). The server part needs ADR 0130 (48A) only and can run in
parallel with ADR 0131 (48B); the conflict part needs the 48B case editor. House rules as in ADR 0130. Parent: plan sections 2 (tools
in Claude Code outside the app; edits there appear live in the app), 3 (L7), 7 (plugins, MCP). Roadmap: F-267.

## Context (read at main 1248a76)

* `rcfd-mcp` (`crates/rcfd-mcp/src/lib.rs`, `main.rs`): a stdio JSON-RPC MCP server over `rcfd_api::service::Service`, std threads and
  `serde_json` only. Tools are a static list (`fn tools()`, line 69) with access classes `read`, `mutate`, `execute`, `write_files`
  (`src/policy.rs`); `tools/list` marks `readOnlyHint` and `_meta."rcfd/access"`; `initialize` declares `tools.listChanged: false`
  (line 957); file paths are sandboxed by `--root` (error 1004). The 33 tools ([`docs/mcp-tools.md`](../mcp-tools.md)) work on
  in-memory sessions; none knows project folders, the runner or the time-series store.
* `rcfd-mcp` is a dependency of `rcfd-assistant` and of the frozen `rcfd-gui` ([`docs/architecture.md`](../architecture.md)). Whatever
  `rcfd-mcp` depends on, both of them build.
* `render_image` calls the service's `render.png`, which uses the CPU renderer `rcfd_post::render::render_scalar`
  (`crates/rcfd-api/src/service.rs` line 728).
* `clients/claude-panel/` is an existing Claude Code plugin named `rcfd-panel` (a mod; F-092, demoted, not extended). Its
  `plugin.json` version is checked against the release tag by `scripts/release_notes.py` (line 17). There is no plugin with an MCP
  server, skills or commands.
* Releases ship no binaries ([`docs/releasing.md`](../releasing.md), "What a release is").
* Read in the Claude Code documentation on 2026-10-28 (code.claude.com/docs/en/plugins/manifest-reference, /plugins/loading,
  /plugins/cli-reference, /plugins/create-marketplace, /mcp, /skills, /headless; not observed on an installed build):
  * layout: `.claude-plugin/plugin.json` (only `name` required, kebab-case), `skills/<name>/SKILL.md`, `commands/*.md`,
    `hooks/hooks.json`, `.mcp.json` at the plugin root or `mcpServers` in `plugin.json`, `bin/` on the Bash PATH;
  * `${CLAUDE_PLUGIN_ROOT}`, `${CLAUDE_PLUGIN_DATA}` and `${CLAUDE_PROJECT_DIR}` are substituted in a stdio server's `command`,
    `args` and `env`; the working directory of a plugin's stdio server is NOT DOCUMENTED;
  * plugin tool names are `mcp__plugin_<plugin>_<server>__<tool>`, hyphens kept, names over 128 characters dropped;
  * `--plugin-dir <path>` loads a plugin for one session; marketplaces are `.claude-plugin/marketplace.json`;
    `claude plugin marketplace add`, `claude plugin install`, `claude plugin validate` (exit 0 pass, 1 fail);
  * MCP results: a warning at 10 000 tokens, a default limit of 25 000 (`MAX_MCP_OUTPUT_TOKENS`); images (PNG and others) are shown
    inline; `list_changed` is supported; resource subscriptions are not documented in the pages read;
  * skills and commands are namespaced `/<plugin>:<name>`; frontmatter has `name`, `description`, `allowed-tools`, `argument-hint`,
    `$ARGUMENTS`;
  * hooks: `PostToolUse` with matcher `Edit|Write` gets `tool_input.file_path` (absolute, native separators).
* `rcfd-project` has the path check (`paths::resolve_in`), atomic replace, the poll watcher and run records; `rcfd-runner`'s client
  starts and controls runs; `rcfd-tsstore`'s reader reads fields at a time.

## Decisions needed

### W48-12: where the project tools live

* (a) **In `rcfd-mcp`, behind `--project <dir>`**: a `project` module adds the project tools to `tools/list` only when the flag is
  given; without it the list is unchanged. New edges `rcfd-mcp -> rcfd-project, rcfd-runner, rcfd-tsstore` (all lower layers). Images
  through the CPU renderer of `rcfd-post`, which `rcfd-mcp` already reaches through `rcfd-api`. No wgpu.
* (b) **New crate `rcfd-mcp-project`** (binary `rcfd-project-mcp`) that reuses `McpServer` through a small extension hook in
  `rcfd-mcp`; only the new crate depends on the project crates.
* (c) **As (a), plus a cargo feature `gpu-render`** that adds a 3D render tool through `rcfd-render`.

Trade-offs: (a) keeps one server binary, which is what the plan says ("extending `rcfd-mcp`"), and adds three small std-only crates to
the build of `rcfd-assistant` and `rcfd-gui` (no third-party dependency beyond `miniz_oxide`, already compiled); (b) keeps those builds
as they are but adds a crate and an extension API; (c) would put wgpu into the build of everything that enables the feature and makes
the MCP server need a GPU for one tool. **Recommendation: (a)**; a 3D render tool waits for wave 49 and goes into a separate binary if
it needs wgpu.

### W48-13: how the plugin finds and starts the server

* (a) `.mcp.json` with `"command": "rcfd-mcp"` (found on PATH) and `"args": ["--project", "${CLAUDE_PROJECT_DIR}", "--root",
  "${CLAUDE_PROJECT_DIR}"]`; an optional `RCFD_MCP` path in `env` documented for users without PATH entries.
* (b) Ship built binaries in the plugin's folder and call `${CLAUDE_PLUGIN_ROOT}/bin/rcfd-mcp`.
* (c) The Studio writes a project-scoped `.mcp.json` into each project folder.

Trade-offs: (b) contradicts "a release has no binaries" and needs per-platform builds in the plugin; (c) puts a control file into the
project (the allow-list refuses `.mcp.json`, and ADR 0134 W48-10 treats such files as untrusted), and project servers load without a
prompt in `-p`. (a) needs `rcfd-mcp` installed (built from source, as the README says for everything), and for the run tools also
`rcfd-runner` next to it (`Client::with_default_runner`) and `rcfd` next to the runner or named by `RCFD_BIN` (ADR 0128 finding 1):
the install layout is the three binaries in one folder on PATH, as `cargo install` or a release build directory gives. The server takes its project
from the argument, never from its working directory, which is not documented. **Recommendation: (a)**; the server refuses to start when
`--project` is not a project (located error naming the path), so a session outside a project folder simply has no project tools.

### W48-14: plugin packaging and distribution

* (a) A new folder `clients/claude-plugin/` with plugin name `rcfd` and server name `project` (tool names
  `mcp__plugin_rcfd_project__<tool>`, accepted by `AllowRule::Tool` without change), loaded with `--plugin-dir` for development, plus
  a `.claude-plugin/marketplace.json` in the repository root so `claude plugin marketplace add <repo>` and `claude plugin install
  rcfd@<marketplace>` work once the repository is reachable. Its version joins the release check in `scripts/release_notes.py`.
* (b) Extend `clients/claude-panel/` (`rcfd-panel`): one plugin, but it carries the demoted mod, and its hyphenated name gives tool
  names with `-` that `AllowRule::Tool` refuses.
* (c) A separate public repository for the plugin.

Trade-offs: (c) is the usual route for a public plugin, but the repository is private now and a second repository splits versions;
(b) revives a demoted item. **Recommendation: (a)**; public distribution is decided when the repository's visibility is (the history
rewrite and visibility are open items of their own).

### W48-15: edit conflict policy

* (a) **Base hash and cooperative lock.** Every writer that cooperates (the Studio editor, the MCP edit tools) records the FNV-1a hash
  of the file it read (`rcfd_project::records::hash_bytes`) and writes with "replace only if the file still has this hash", checked and
  replaced while holding `.rcfd/locks/<path hash>.lock` (created with `create_new`, removed after; a stale lock older than 30 s is
  broken with an event). A mismatch is a conflict: the MCP tool returns an error naming the current hash; the Studio shows both versions
  and the user picks or merges by hand. Edits by non-cooperating writers (Claude Code's own Edit/Write tools, an editor) are seen by the
  watcher after the fact; a dirty Studio editor then enters the conflict state (ADR 0131 B6).
* (b) Last writer wins, with the replaced content kept in `.rcfd/backup/`.
* (c) A lock held for the whole editing session: Claude cannot edit a file the Studio has open.

Trade-offs: (b) never blocks but silently loses an edit until someone looks at the backup; (c) blocks the assistant whenever the user
has a file open. **Recommendation: (a)**. Limits stated: the lock covers cooperating writers only; on Windows a non-cooperating writer can
still replace the file between the check and the rename (the window is the duration of one rename).

## Design (proposed, assuming the recommendations)

**Project tools** (all take paths relative to the project and resolve them with `rcfd_project::paths::resolve_in`; refusals are error
1004 as today):

| Tool | Access | Does |
|---|---|---|
| `project_info` | read | manifest, counts, `validate` issues |
| `list_runs` | read | runs with state, step, time, numeric metrics; skipped runs with reasons; filter by state or case; paged |
| `run_status` | read | one run's status, last residual line, last events |
| `read_metrics` | read | `metrics.json` of one or more runs |
| `read_residuals` | read | residual and monitor series of a run, decimated to at most N points (M3 rule) |
| `field_summary` | read | min, max, mean (cell-volume weighted where the mesh gives volumes) of a stored field at a step or time (nearest or linear) |
| `render_field` | read | PNG of a stored field at a time through `rcfd_post::render::render_scalar` (the CPU path of `render.png`); 2D cases only in wave 48 |
| `compare_runs` | read | metrics side by side with differences, and the residual history lengths |
| `list_changes` | read | project-relative paths changed since a token (the watcher's `changes_since`) |
| `read_project_file` | read | text of a case, mesh recipe or note, with its hash (base for edits) |
| `edit_case` | write_files | set dotted settings in a case file: parsed and validated through `rcfd_api` (as `settings.set`), written atomically only if valid and the base hash matches |
| `write_note` | write_files | create or replace a file under `notes/` with the base-hash rule |
| `start_run` | execute | submit a case through `rcfd_runner::client::Client::start` with settings |
| `pause_run`, `resume_run`, `cancel_run` | execute | the client calls |
| `wait_run` | read | wait for a state change with a timeout of at most 60 s (polling; MCP push only through channels, plan section 7) |

Large outputs (long series, images over a size) are written under `runs/<id>/` or `notes/` and returned as paths. `tools/list` with
`--project` sets `listChanged: false` as today; the tool set does not change while the server runs.

**Skills and commands** (in `clients/claude-plugin/skills/`): `project-status` (summarise runs and failures with `list_runs`,
`run_status`), `start-and-watch` (validate a case with `edit_case`'s dry-run mode, start it, poll), `compare` (two runs' metrics and
residuals). Slash commands come from the same skills (`/rcfd:project-status` and so on). Their `allowed-tools` name only the project
tools.

**Hooks**: none in wave 48. A `PostToolUse` hook on `Edit|Write` that validates case files after Claude Code edits them is possible
(docs above) but would run a command on every edit; it is listed as a follow-up.

**Studio side**: the watcher (ADR 0131) shows every change; files touched through the MCP tools also get an event line in
`.rcfd/edits.jsonl` (tool, path, old hash, new hash, time) so the Studio can say who changed a file.

## Placement (acyclic)

`rcfd-mcp -> rcfd-api, rcfd-io, rcfd-project, rcfd-runner, rcfd-tsstore` (layers 1, 2, 3, 4, 5 under its layer 6). No new third-party
dependency. `rcfd-project` gets the lock helper (`atomic::replace_if_hash`, std only). `clients/claude-plugin/` holds no code that is
built by cargo.

## Registered checks (proposal)

| Id | Check | Limit | Reason | Negative control |
|---|---|---|---|---|
| F1 | `tools/list` without `--project` | byte-identical to a golden recorded from main 1248a76 before any change | default unchanged | the list with `--project` differs |
| F2 | Tool names, both forms: plugin `mcp__plugin_rcfd_project__<tool>` and the Studio's app-owned server `mcp__rcfd_project__<tool>` (ADR 0134 W48-10 (a)) | every project tool's full name in both forms passes `AllowRule::Tool` validation and is <= 128 characters; the plugin, server and app-owned server names contain no `-` | ADR 0126 D3, the 128-character rule | a name with `-` is refused |
| F3 | Protocol: each tool on a fixture project (made through 48A's runner) | result shapes equal golden JSON; access classes and `_meta` as listed; `--read-only` refuses every non-read tool (1100); arguments outside the schema are -32602 | [`docs/mcp-tools.md`](../mcp-tools.md) conventions | a call with an unknown argument is -32602 |
| F4 | Path safety | the 17 escape and refusal inputs of ADR 0122 P1 given to every path argument are refused with 1004 and create nothing | ADR 0122 P1 | `notes/a.md` is accepted |
| F5 | `edit_case` validation | an invalid value (`physics.nu = -1`) is refused with the located message and the file hash unchanged; a valid edit is written and re-parses to the edited value | the service's rules | the same invalid value written by a naive writer changes the hash |
| F6 | Concurrency: the Studio model and an MCP client edit one case 200 times each, interleaved, with base hashes | 0 lost updates: every accepted edit's base hash equals the previous accepted result; every rejected edit returned a conflict naming the current hash | correctness | the same run without the lock and hash check loses >= 1 update |
| F7 | Non-cooperating edit | a file replaced by a plain atomic write while the Studio editor is dirty enters the conflict state within 2.0 s; the editor's content is kept | ADR 0131 B6 | a clean editor reloads instead |
| F8 | Output size: every read tool on a 500-run fixture with a 100 000-step residual file | each result's text <= 40 000 bytes, or a path to a file | 10 000 tokens at an assumed 4 bytes per token (the ratio is UNVERIFIED); stays under the documented warning | `read_residuals` without decimation exceeds 40 000 bytes |
| F9 | `render_field` | PNG bytes equal those of `render.png` on a service session holding the same field values | same CPU renderer | the next snapshot's field differs |
| F10 | `field_summary` | min, max equal the stored field's exactly; mean within 1e-12 relative of a compensated sum | f64 summation | a summary of another snapshot differs |
| F11 | Run tools | `start_run` then `wait_run` reaches `finished` on a 30-step cavity; `cancel_run` on a long run gives `cancelled` within 5.0 s | ADR 0123 S7 | `wait_run` with a 0 s timeout returns the current state without waiting |
| F12 | Plugin files | `plugin.json` parses, `name` is `rcfd`, `version` equals the workspace version (and `scripts/release_notes.py` checks it, with its self-test extended); `.mcp.json` args contain `${CLAUDE_PROJECT_DIR}`; each `SKILL.md` has `name` and `description` and its `allowed-tools` lists only existing project tools | release rules, F2 | a skill naming an unknown tool fails |
| F13 | Server start | `rcfd-mcp --project <not a project>` exits non-zero with a message naming the path; with a project it answers `initialize` | W48-13 (a) | (built in) |

## Not in CI

* `claude plugin validate clients/claude-plugin` (needs the CLI; run by hand, exit code recorded).
* One real session with `--plugin-dir clients/claude-plugin` in a project folder, `--model haiku`: the tools are listed under their
  `mcp__plugin_rcfd_project__` names, `list_runs` and `start_run` work, an `edit_case` shows up in an open Studio within 2 s, a slash
  command runs. Recorded with the CLI version.
* Installing from the in-repo marketplace.

## Out of scope

3D rendering through wgpu in the server (wave 49); video and side-by-side images (wave 49); design-ledger tools for projects beyond the
existing `design_*` tools (wave 50); server push of run events (channels, research preview); the demoted `rcfd-panel` mod; Windows
installers.

## Risks

* The plugin documentation was read, not observed; tool naming (F2) is the item most likely to differ on the installed build, which
  is why H5 of ADR 0134 and the real session above check it.
* Adding three crates to `rcfd-mcp` lengthens the build of `rcfd-assistant` and `rcfd-gui` slightly; measured and reported.
* `--root` and `--project` both point at the project: the existing tools then work on project files too, under the same sandbox.

## Amendment 1 (2026-10-28, 48F server part, before any registered run): set-ups and the checks not run

Written by the 48F server implementer after reading the code and before building or running anything of 48F. What I knew: the
code reading below; no 48F code existed. Limits are unchanged. Decisions W48-12 (a), W48-13 (a), W48-14 (a), W48-15 (a) as
accepted.

1. **F1 golden.** Recorded at main 15b00ef (the base of this branch), not at 1248a76: `git diff 1248a76 15b00ef -- crates/rcfd-mcp`
   is empty and `tools/list` is built only from `rcfd-mcp`'s static `tools()`, so the two commits give the same bytes. The golden
   (`crates/rcfd-mcp/tests/golden/tools_list_default.json`, the `result` of `tools/list` as serde_json writes it) and its comparing
   test are committed before any `rcfd-mcp` source changes; the golden is never re-recorded. The test also checks that `initialize`
   without `--project` is unchanged (same `capabilities`, `serverInfo` and `instructions` text).
2. **F2** uses the real `rcfd_claude::allow::AllowRule::validate` through a new dev-dependency `rcfd-mcp -> rcfd-claude` (layer 2
   under 6; `rcfd-claude` is read, not edited). Added, not limited: the read-class project tools equal
   `rcfd_claude::allow::PROJECT_READ_TOOLS` (11 names).
3. **F3 result shapes.** "Shape" is the JSON value with every leaf replaced by its type name (`string`, `number`, `bool`, `null`),
   objects with their keys, arrays by the shape of their first element (`[]` when empty). The golden shapes
   (`crates/rcfd-mcp/tests/golden/project_shapes.json`) are written by hand from the result description in
   [`docs/mcp-tools.md`](../mcp-tools.md) before the first run, not recorded from the implementation. The fixture project is made
   through 48A's runner: the `rcfd-mcp` binary with `--project` starts a 30-step cavity with the field store on
   (`output.store_interval`) through `start_run`, which starts `rcfd-runner` next to it (W48-13 (a) install layout: `rcfd`,
   `rcfd-runner` and `rcfd-mcp` in one target folder). The test runs the binaries of one `cargo build` profile (debug) and fails with
   a located message if `rcfd` or `rcfd-runner` is missing.
4. **F4 inputs.** The 17 inputs of ADR 0122 P1 (13 escapes with the junction on Windows, 4 refusal-only) go to every path argument:
   `read_project_file.path`, `edit_case.path`, `write_note.path`, `start_run.case`. "Create nothing": the sorted file list of the
   project folder and of the outside folder are equal before and after. Run arguments are not paths: a string that is not a run id
   (`runid::is_run_id`) is refused with 1004 as well (reported, not part of the 17 x 4).
5. **F5.** The edited file is written in the smallest form that re-parses to the edited case: the `key = value` line is replaced (or
   added) in the TOML text, comments kept; when the re-parsed text does not give exactly the expected settings tree (the original
   case with `Case::set` applied, which is what `settings.set` calls), the file is written in the canonical form of `Case::to_toml`
   / `Case::to_json_string` instead (as `design_create_variant` and the frozen GUI write cases). The result names the form.
6. **F6 and F7 are not run in this sub-wave.** Both name the Studio: F6 "the Studio model", F7 "the Studio editor is dirty" (ADR 0131
   B6), and the Studio crate is being built in parallel (48B). They run with the conflict part after 48B merges. Reported, not
   registered, here: a library-level run of `rcfd_project::atomic::replace_if_hash` with two writer threads, 200 interleaved edits
   each, with the same 0-lost-updates criterion, and a no-lock control (plain read, modify, write) that is expected to lose updates.
7. **F8 set-up.** The 500-run fixture and the 100 000-step `residuals.jsonl` are written directly with `rcfd-project` (records,
   status files and lines), not by solver runs. "Each result's text" is the sum of the lengths of every `content[*].text` and
   `content[*].data` (base64 image bytes) of the tool result; `structuredContent` repeats the text and is not counted (whether Claude
   Code counts it is UNVERIFIED). Every read tool is called on the fixture with its default arguments and with its largest allowed
   page or point count. `read_project_file` pages a file by byte `offset` and `limit` (at most 32 000 bytes per call), so a large
   file is read in pieces; `render_field` writes the PNG to `runs/<id>/renders/` and returns the path when the base64 image would
   pass the 40 000-byte limit. The negative control calls the residual reader with decimation off (`max_points` = the line count).
8. **F9 procedure.** One service session runs the cavity template with `output.store_interval` set and an `out_dir`
   (`run_headless`); the hook writes the final state at the run end in f64. `render.png` with `field = "p"` and width 640 on that
   session is compared with `render_field` with `field = "p"`, width 640, on the store's last snapshot (the store copied to
   `runs/<id>/fields/` of a project run). `speed`, the default of `render.png`, is not stored, so F9 uses a stored field. The
   negative control renders the snapshot before the last.
9. **F9 "2D cases only".** `render_field` refuses a mesh where the CPU renderer's view plane does not cut every cell
   (`cells_drawn != n_cells`, i.e. more than one cell layer), with a located message; this is how "2D" is tested.
10. **F10.** The test computes its reference with its own Neumaier-compensated sums of `v_i V_i` and `V_i`; min and max are over the
    finite values (the summary reports the count of non-finite values).
11. **F11 and `wait_run`.** `wait_run {run, timeout_s <= 60, from?}` returns as soon as the run's state differs from `from` (default:
    the state at the call) or the run is terminal, else at the timeout with `changed: false`. F11 calls `wait_run` in a loop (each
    call 60 s or less) until the state is terminal; the 30-step run must reach `finished`, and the `cancelled` state after
    `cancel_run` on a long run must be read within 5.0 s of the `cancel_run` call.
12. **New error codes** (tool results with `isError`, as today): 1200 edit conflict (`data.current_hash`, null when the file does not
    exist), 1201 file lock busy, 1202 not found (run, file, field, snapshot). Existing codes keep their meaning: 1002 wrong run state,
    1003 validation, 1004 path refused.
13. **Edge `rcfd-mcp -> rcfd-post`** (layer 4), not in the Placement list: `render_field` calls `rcfd_post::render::render_scalar`
    directly, because `rcfd-api` does not re-export `rcfd-post`. Acyclic; recorded in [`docs/architecture.md`](../architecture.md).
    No new third-party crate.
14. **Lock helper.** `rcfd_project::atomic::replace_if_hash(root, rel, base, bytes, who)`: `base = None` means "the file must not
    exist"; the lock is `.rcfd/locks/<fnv64 of the project-relative path>.lock` (`create_new`); a busy lock is retried for up to 2 s,
    then 1201; a lock older than 30 s is removed and a `stale_lock_broken` line is appended to `.rcfd/edits.jsonl`; every accepted
    write appends `{t, kind: "edit", who, path, old_hash, new_hash}` there (`who` is the tool name for the MCP server).
15. **Not in CI, not run here:** `claude plugin validate`, the real `--plugin-dir` session and the marketplace install (they need the
    Claude Code CLI and a person; the maintainer runs them).

## Amendment 2 (2026-10-28, 48F server part, before any registered run): F3 shape rule for passed-through and data-keyed parts

Written after the tools and `docs/mcp-tools.md` were written and before the F3 golden and any registered run. Reading the runner
while writing the golden showed that several result parts are documents of ADR 0130's contract passed through unchanged, whose
fields are the runner's (for example `metrics.json` holds the runner's run summary with the solver's `finish` event, not report
values), and that some objects are keyed by data (run ids, metric and series names). Amendment 1 item 3 is refined; the limit is
unchanged:

1. Passed-through documents are shaped as the leaf `object`: `run_status` `/settings`, `/last_residual` and each `/events/*`;
   `read_metrics` `/metrics/*`. Their shapes are checked by ADR 0130's tests, not here.
2. Data-keyed objects are shaped as `{"*": <shape of their first value>}`: `project_info` `/runs_by_state`; `list_runs`
   `/runs/*/metrics`; `read_metrics` `/metrics`; `read_residuals` `/series`; `compare_runs` `/metrics`, `/metrics/*`,
   `/differences`, `/differences/*`; `edit_case` `/values`.
3. The shape of `render_field` is that of the whole tool result (`content` with the image block, `structuredContent`), of the
   other tools that of `structuredContent`.
4. F3's fixture: two finished runs of the 12 x 12 cavity with the `lid_drag` report and `output.store_interval = 10` (30 steps and,
   for `compare_runs`, 20 steps through `start_run`'s `set`), and a third long run for the control tools (paused, resumed,
   cancelled; F11's 5.0 s limit). `run_status` is called with `events: 1`, `read_residuals` with `max_points: 10`, `render_field`
   with `width: 128` (an inline image), `field_summary` and `render_field` with `field: "p"`.

## Addendum: 48F server part results (2026-10-28, branch `worktree-agent-a3111463805750a3f`, debug builds on the gate host)

Decisions applied as accepted: W48-12 (a), W48-13 (a), W48-14 (a), W48-15 (a). Base main 15b00ef. New workspace edges
`rcfd-mcp -> rcfd-project, rcfd-runner, rcfd-tsstore, rcfd-post` and the dev-only `rcfd-mcp -> rcfd-claude` (all lower layers;
`workspace_graph` 3 passed; `docs/architecture.md` updated). No new third-party crate. `rcfd-claude` and the Studio crate were
not edited. The workspace gate was not run (the maintainer runs it); `rcfd-project` and `rcfd-mcp` were tested and linted alone,
and `rcfd-assistant`, `rcfd-gui`, `rcfd-runner`, `rcfd-claude` pass `cargo check --all-targets`.

| Id | Limit | Measured | Result |
|---|---|---|---|
| F1 | `tools/list` without `--project` byte-identical to the golden of 15b00ef | 33 tools, 27 188 bytes, identical with no flags, `--root .` and `--read-only`; `initialize` unchanged | pass |
| F1 control | the list with `--project` differs | 50 tools with `--project` (F3, F13) | fails as required |
| F2 | both name forms valid for `AllowRule::Tool`, <= 128 characters; no `-` in plugin, server, app-owned server | 17 x 2 names valid, longest 43; `rcfd`, `project`, `rcfd_project` | pass |
| F2 control | a name with `-` is refused | `mcp__plugin_rcfd-panel_project__list_runs` refused | fails as required |
| F3 | result shapes equal the golden; access classes and `_meta` as listed; read-only 1100; unknown argument -32602 | 19 of 19 shapes equal the hand-written golden (runs made by `start_run` through the runner); 17 access classes as listed; 6 non-read tools refused read-only with 1100, project unchanged; 17 unknown-argument calls -32602 | pass |
| F4 | 17 inputs x every path argument refused with 1004, nothing created | 17 x 4 = 68 calls refused with 1004 (junction made); project and outside folder trees unchanged; 5 non-run-id strings refused with 1004 | pass |
| F4 control | `notes/a.md` accepted | written | as required |
| F5 | `physics.nu = -1` refused with the located message, hash unchanged; a valid edit written and re-parsed | 1003 "cases/cavity.toml: physics.nu = -1 refused: ... -1 outside [Some(0.0), None]", hash unchanged; edit of nu 0.02 and max_iterations 40 written in the minimal form (comments kept), re-parsed to 0.02 and 40; stale base 1200 naming the current hash; a JSON case edited | pass |
| F5 control | a naive writer of the invalid value changes the hash | changed, and the file no longer parses | fails as required |
| F6 | Studio model and MCP client, 200 edits each | not run (needs the 48B Studio model, amendment 1 item 6) | not run |
| F6 stand-in (reported) | 0 lost updates | two library writers, 200 each: 400 accepted, 227 conflicts, 400 lines, 0 lost; the edit log chains `old_hash` to the previous `new_hash` | reported |
| F6 stand-in control | loses >= 1 update | plain read-modify-write: 400 writes, 200 lines, 200 lost | fails as expected |
| F7 | non-cooperating edit, dirty Studio editor | not run (needs the 48B editor, ADR 0131 B6) | not run |
| F8 | every read tool <= 40 000 bytes or a path | 20 calls on 500 runs; largest 35 897 bytes (`list_changes` over 2 001 changes, truncated), `list_runs` limit 100: 35 701 (paged); `read_residuals` max_points 2000 and `render_field` width 4096 returned paths | pass |
| F8 control | `read_residuals` without decimation > 40 000 | 100 000 lines: 12 802 924 bytes | fails as required |
| F9 | PNG bytes equal `render.png` | 5 084 base64 bytes, equal (p, width 640, last snapshot step 30) | pass |
| F9 control | the snapshot before differs | differs | fails as required |
| F10 | min, max exact; mean within 1e-12 relative | p, ux, uy on 24 x 24: min and max bit-equal, mean relative difference 0 | pass |
| F10 control | another snapshot's summary differs | differs for all three | fails as required |
| F11 | 30-step cavity reaches `finished`; `cancelled` within 5.0 s | finished at step 30 in 0.36 s; cancelled 0.152 s after `cancel_run` (step 38); pause seen in 0.116 s; control on an ended run 1002 | pass |
| F11 control | `wait_run` with 0 s returns at once | under 1 s, `changed: false` | as required |
| F12 | `plugin.json` name `rcfd`, version = workspace; `.mcp.json` args with `${CLAUDE_PROJECT_DIR}`; skills valid; release check | as listed, 3 skills; `scripts/test_release_notes.py` 6 passed (4 version places, the new plugin alone named) | pass |
| F12 control | a skill naming an unknown tool fails | refused naming `no_such_tool` | fails as required |
| F13 | not a project: non-zero, names the path; a project answers `initialize` | exit 2, message names the folder; `initialize` and 50 tools | pass |

No known miss. Not run here: F6, F7 (Studio), the "Not in CI" items (`claude plugin validate`, a real `--plugin-dir` session, the
marketplace install), Linux, and the build-cost measurement of the Risks section (only an incremental `cargo check` of
`rcfd-assistant` and `rcfd-gui`, 17.5 s, which is not a cost measurement).

Deviations and notes:
* The first run of the F8, F9 and F10 tests stopped in their set-up before measuring anything: the store helper gave the service a
  sandbox folder that did not exist (1004). A test-code defect, fixed (the folder is created), then all three ran as counted above.
* `rcfd-mcp`'s existing test `design_tests::t2_paths_are_sandboxed_and_names_are_restricted` fails on this host because
  `%TEMP%\x` exists (created 2026-10-15, not by this branch); it asserts that path is absent. The failing assertion reads only
  that folder and the test is unchanged (not re-run on 15b00ef); skipped in the runs above.
* `rcfd-mcp --help` gained one line for `--project`.
* After the counted runs, for the gate: `scripts/limit_audit.py` flagged the F6 stand-in control (class iii, no failing
  assertion); it now asserts that at least one update is lost (it lost 200 of 400). Two unregistered timing bounds of these tests
  were widened against gate load: the busy-lock wait upper bound 6 s to 30 s (the lower bound of 2 s is the check), and the
  `wait_run` 0 s control from 1 s to 5 s (the default wait is 30 s). The registered 5.0 s of F11 is unchanged. `edit_case`'s
  refusal of `runs/`, `.rcfd/` and the manifest compares case-insensitively on Windows, as the edit lock does. The affected tests
  were re-run and pass; the audit has no hit in the new tests. The gate builds `rcfd` and `rcfd-runner` with every workspace
  binary (`cargo test --workspace --release --no-run`), which F3 and F11 need.
* The edit lock, edit log and conflict codes are as amendment 1 items 12 and 14. Read tools write a file under `runs/<id>/mcp/` or
  `runs/<id>/renders/` when a result is over the budget, also with `--read-only` (as the Design section says for large outputs).

Questions for the maintainer:
1. The runner's `metrics.json` holds the run summary (`state`, `reason`, `exit_code`, `steps`, `time`, `solver_finish`,
   `last_residuals`), not the "final report values" of the plan's contract; report values are in `output/monitors.json`. So
   `list_runs` and `compare_runs` compare `exit_code`, `steps` and `time`. Should the runner also write the final report values
   into `metrics.json` (an ADR 0130 change), or should the tools read `output/monitors.json`?
2. W48-13 (a) mentions an optional `RCFD_MCP` path in `env`; Claude Code's documentation does not say that `env` can replace
   `command`, so it was not built. Keep PATH only?
3. Should `--project` imply `--root <project>` when `--root` is not given? Now the existing tools keep their sandbox as given
   (the plugin passes both).
4. When the minimal TOML edit cannot be placed (a multi-line value, a key set through an inline table), `edit_case` writes the
   canonical form, which spells out every default. Acceptable, or refuse such edits?
5. Marketplace name `rcfd` and owner name "rcfd maintainers" in `.claude-plugin/marketplace.json`: confirm or replace.
6. Error codes 1200 (conflict), 1201 (lock busy), 1202 (not found): confirm the numbers.

## Amendment 3 (2026-10-28, after the maintainer's answers to the addendum questions, before any run of the changes)

Maintainer decisions (recommendations taken): (1) the runner also writes the final report values into `metrics.json`, additively,
and `list_runs` and `compare_runs` use them; (2) no `RCFD_MCP` override, PATH only; (3) `--project` implies `--root <project>` when
`--root` is not given; (4) `edit_case` refuses an edit that would need the whole case rewritten with every default spelled out;
(5) marketplace name `rcfd` kept, the owner name "rcfd maintainers" stays a placeholder (open); (6) codes 1200, 1201, 1202
confirmed. Written at main 64732bb before any code of these changes. Set-ups and limits:

1. **Report values.** `rcfd-runner` adds one key to the `metrics.json` it writes at a run's end: `reports`, an object
   `{report name: final value}`. The names are the keys of `monitors` in the run's last `residuals.jsonl` line (the reports, 48A
   `--status-monitors`); each value is the last `y` of that name's series in `output/monitors.json`, or the residual line's value
   when the file or series is missing; a non-finite value is `null`. The existing keys (`state`, `reason`, `exit_code`, `steps`,
   `time`, `solver_finish`, `last_residuals`) are unchanged; a run that never launched gets `reports: {}`. The solver is not
   changed, so its outputs stay bit-identical. `list_runs` and `compare_runs` take a run's `metrics` from `reports` when
   `metrics.json` has that object, else (an older record) from its numeric top-level entries, as before. `rcfd-project`'s reader
   is not changed.
   * **M1 limit:** on the F3 fixture, each finished run's `reports.lid_drag` equals bit-exactly the last `y` of `lid_drag` in its
     `output/monitors.json`; every existing key is still present; `list_runs` and `compare_runs` report exactly `{lid_drag}` with
     that value. **Control:** a `metrics.json` without `reports` gives `list_runs` the numeric top-level entries (`exit_code`,
     `steps`, `time`). The `rcfd-runner` tests (`runner_w47b`, `contract_w48a`, `runner_wfix`, unit tests) pass as before (S5
     stays ignored).
2. **R1, `--project` implies `--root`:** `rcfd-mcp --project <dir>`, started in another working folder, resolves a sandboxed
   path of an existing tool against the project (`create_case {path: "cases/cavity.toml"}` succeeds; `../x` is refused with
   1004). **Control:** with an explicit `--root <other>`, that root is used (the same `create_case` fails). F1 is unaffected (no
   `--project`).
3. **E1, no whole-case rewrite:** an edit that cannot be placed in the file's own lines (a multi-line value) is refused with
   1003 and a message naming the setting and saying to edit the file by hand; the file hash is unchanged and no edit-log line is
   written. **Control:** the single-line edit of F5 is still written. The canonical form is no longer written; `form` is always
   `minimal`.

## Addendum 2: results of amendment 3 (2026-10-28, on main 64732bb, debug builds on the gate host)

| Id | Limit | Measured | Result |
|---|---|---|---|
| M1 | `reports.lid_drag` bit-equal to the last `y` of `output/monitors.json`; existing keys kept; `list_runs`, `compare_runs` give exactly `{lid_drag}` | both F3 runs: -1.025564180656281e-2 and -1.0255874488778053e-2, bit-equal; all 7 earlier keys present, `reports` has 1 entry; `list_runs` and `compare_runs` metrics exactly `{lid_drag}`, difference bit-exact | pass |
| M1 control | without `reports`: the numeric top-level entries | `{exit_code, steps, time}` | as required |
| M1 runner tests | pass as before | unit 7, `contract_w48a` 2, `runner_w47b` 13 (S5 ignored), `runner_wfix` 6, `no_window_scan` 1 | pass |
| R1 | `--project` without `--root` resolves in the project from another working folder; `../x` 1004 | `create_case {path: cases/cavity.toml}` succeeds; `../x.toml` 1004 | pass |
| R1 control | explicit `--root <other>` is used | the same `create_case` fails | as required |
| E1 | a multi-line value refused with 1003 naming the setting, hash unchanged, no log line | refused, message names `boundaries.lid.velocity_value` and says to edit by hand; hash unchanged; no `.rcfd/edits.jsonl` | pass |
| E1 control | the single-line edit is written | written, form `minimal` | as required |

F1 to F13 re-run on this commit: F1 2 passed; F2 to F5 and F8 to F10, F12 pass; F3 19 of 19 shapes; F11 finished at step 30, cancelled
0.153 s after `cancel_run`; F13 passes. The solver was not changed (no file of `rcfd-api` or `rcfd-cli`), so its outputs are
bit-identical. `docs/mcp-tools.md` describes the three changes. Still open: the marketplace owner name ("rcfd maintainers" is a
placeholder). Still not run: F6, F7 (Studio), the Not-in-CI items, the gate, Linux.

## Files

`crates/rcfd-mcp/src/{lib,main}.rs`, new `src/project.rs` and tests, `crates/rcfd-mcp/Cargo.toml`, `crates/rcfd-project/src/atomic.rs`,
`clients/claude-plugin/` (new: `.claude-plugin/plugin.json`, `.mcp.json`, `skills/*/SKILL.md`), `.claude-plugin/marketplace.json` (new),
`scripts/release_notes.py` and its test, `docs/mcp-tools.md` (new section), `docs/releasing.md` (version places), this ADR.
