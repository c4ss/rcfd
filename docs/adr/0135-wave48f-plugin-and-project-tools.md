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

## Files

`crates/rcfd-mcp/src/{lib,main}.rs`, new `src/project.rs` and tests, `crates/rcfd-mcp/Cargo.toml`, `crates/rcfd-project/src/atomic.rs`,
`clients/claude-plugin/` (new: `.claude-plugin/plugin.json`, `.mcp.json`, `skills/*/SKILL.md`), `.claude-plugin/marketplace.json` (new),
`scripts/release_notes.py` and its test, `docs/mcp-tools.md` (new section), `docs/releasing.md` (version places), this ADR.
