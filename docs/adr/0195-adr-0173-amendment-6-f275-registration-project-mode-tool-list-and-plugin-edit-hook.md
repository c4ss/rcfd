# ADR 0195: ADR 0173 amendment 6, F-275 registration: the project-mode tool list and the plugin edit hook

Status: **REGISTERED 2026-11-06**, written before any code of F-275. Not yet accepted by the maintainer. Addendum 1 reports P1
to P11 built as written, with amendment 1 (one more Studio test edited by design, and Q8), on branch `f275-project-mode-tools`,
not merged. With `--project` the 16 session-only tools are neither listed nor callable (40 tools); the plugin's hook refused a
Write and an Edit under `cases/` in a real Claude Code 2.1.319 session. The gate at `00fbd30` gave 2918 passed, 0 failed,
821 ignored. Eleven answers wait for the maintainer under [Decisions needed](#decisions-needed).
Parent: [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) (proposed, not accepted),
work item W8, check C5, decisions D2 and D5, roadmap item F-275; it builds on
[ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) (F-270),
[ADR 0186](0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md) (F-271),
[ADR 0187](0187-adr-0173-amendment-3-f272-registration-mesh-set-up-from-the-mcp-server.md) (F-272),
[ADR 0188](0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) (F-273) and
[ADR 0189](0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md) (F-274), all merged on main.
ADR 0173 says that every limit of its checks is fixed by an amendment before any test or run; this is that amendment for F-275.
Results go in the addendum at the end.

Read at main f0d7983. Number: 0194 is the highest on main; no branch and no worktree holds a 0195.

ADR 0173 is not accepted. Where it or its amendments give a recommendation, this text uses it and says so; that is a
recommendation taken, not a maintainer decision. Where the code read raises a question no ADR answers, the options and a
recommendation are listed (Q1 to Q7), and work proceeds only on a choice that is reversible. Every answer taken here, from a
written recommendation or not, is listed under [Decisions needed](#decisions-needed) so the maintainer can overturn it.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)): defaults stay bit-identical; no solver result, case
hash or byte of an existing case changes; no case-schema key and no run-record field is added;
`crates/rcfd-api/tests/baseline_hashes.rs` and `case_hashes_w48u.rs` are not edited and nothing is re-recorded; no new
third-party crate; `Cargo.lock` unchanged; no commit trailers. Without `--project` the MCP server's `tools/list` and
`initialize` stay byte-identical (`default_f1.rs`).

## What F-275 asks

The roadmap entry: with `--project`, each session tool is hidden once its project equivalent has passed its check, until
`tools/list` holds no tool whose state lives only in the server's memory; without `--project` it is byte-identical to before; the
plugin refuses Claude Code's own Edit and Write under `cases/`, `runs/` and `designs/` with a message naming `edit_case` and
`create_case`, checked in a real session where possible (ADR 0173 W8, C5, D2, D5). Its note: the hook contract is to be checked
on an installed Claude Code build first.

ADR 0173 D2 (b) and D5 (b), its recommendations, are used.

## What the code read shows (main f0d7983)

1. **Two lists in one server.** `rcfd-mcp`'s `tools()` (`src/lib.rs` line 48) holds 33 session tools; `project::tools()`
   (`src/project.rs` line 76) the 23 project tools; `all_tools` (line 600) appends the second when the server has a project, so
   `--project` lists 56. `call_tool` (line 669) looks the name up in `all_tools`: a name not listed is the JSON-RPC error -32602
   `unknown tool '<name>'`, before any policy or handler. `dispatch` (line 699) asks the project tools first.
2. **What needs a session.** Only `create_case` makes a session (`session.create`, line 728). A handler that calls `with(..)`
   or `session_id` (line 653) answers 1001 `no session yet; call create_case first` when there is none. By handler:
   * every use needs a session, or makes one (16): `create_case`, `set_setting`, `get_setting`, `load_mesh`, `check_mesh`, `run`,
     `status`, `get_residuals`, `get_field_stats`, `export_vtk`, `list_workflow_tasks`, `create_workflow`, `edit_workflow`,
     `run_workflow_task`, `run_headless`, `render_image`;
   * a use needs no session (17): `list_cases` (a cases folder), `check_case_headless` with `path` (`run.check` reads the file,
     `rcfd-api/src/service.rs` line 1030), `read_status_stream` (a file), and the 14 design and ledger tools (a study folder,
     a base case file, a ledger file). `design_create_study` and `check_case_headless` also take an optional `session`.
3. **The design tools in project mode.** `design_run_study` and `design_evaluate` evaluate on the project's runner unless the
   study is chained or sets `max_steps` (ADR 0189 S3); `design_run_operating_map` and those studies solve in the server process
   (ADR 0189 Q2, Q3). Their results are files under the study or map folder, so their state is not only in memory, but those
   solves are not counted by the runner's budget.
4. **The project equivalents and their checks.** `new_case` (ADR 0186 C-7, C-N); `edit_case` (ADR 0135 F5, ADR 0185 C2) and its
   mesh build (ADR 0187 C8-E); `read_project_file` (ADR 0135 F3, F8); `import_mesh` (C8-I); `check_case_mesh`, whose report and
   patches equal the session `check_mesh`'s (C8-C); `start_run` and `wait_run` (ADR 0186 C-7, ADR 0135 F11); `run_status`
   (F3, F11); `read_residuals` (F3, F8); `field_summary` (F10); `render_field`, whose PNG equals `render.png`'s bytes (F9).
   Every runner run writes `runs/<id>/output/solution.vtu` (`rcfd-runner/tests/runner_w47b.rs` line 520). No project tool runs
   a workflow, continues from a checkpoint or interpolates from another case.
5. **Session mesh tools under the project root.** ADR 0187 addendum, difference (1): the session tools cannot load a project case
   that names its mesh as `../meshes/x.msh` when the server's sandbox root is the project, as it is under `--project`.
6. **Tests that hold the project-mode list or call a hidden tool on a project server** (searched: `with_project` and
   `--project` in `crates/`): `project_list_f270.rs` (56 tools at line 88, the golden of 50, 55 at line 119);
   `project_runs_w48f.rs` (56 at lines 100 and 326; R1, lines 283 to 299, calls `create_case` under `--project`);
   `project_w48f.rs` (F3 `33 + 23` at line 99; F2, lines 84 and 85, compares the read tools of ADR 0135's 17-row table with
   `PROJECT_READ_TOOLS`); `mesh_setup_f272.rs` lines 123 to 132 (an observation: `create_case` on a project server is refused
   with "leaves the sandbox"; `call` asserts there is no JSON-RPC error). `design_runner_f274.rs`, `feed_f273.rs`,
   `new_simulations_f271.rs` and `runner_stdio_w48l.rs` call only kept or project tools. No Studio test and no other crate
   starts a project server.
7. **`initialize`.** Its first sentence (`lib.rs` line 965) names `create_case`, `check_mesh` and `run`; `default_f1.rs` pins the
   whole text without `--project`. Under `--project` tests check only that the text contains given words (`project folder`, the
   mesh tools' names).
8. **The Studio bridge's allow-list.** `rcfd_claude::allow::PROJECT_READ_TOOLS` (`crates/rcfd-claude/src/allow.rs` line 301)
   holds 11 names; `AllowList::project_initial` makes them the rules of a new project's `.rcfd/claude-allow.json`, which the
   Studio writes only when the file is missing (`crates/rcfd-studio/src/chat.rs` lines 1200, 1262, 1430). An existing file is read
   as written. The read-class project tools of the live list are those 11 and `read_feed`, `check_case_mesh`, `preview_mesh` and
   `list_meshes`.
9. **The plugin.** `clients/claude-plugin` has `.claude-plugin/plugin.json` (its version pinned to the workspace's by F12),
   `.mcp.json` (`rcfd-mcp --project ${CLAUDE_PROJECT_DIR} --root ${CLAUDE_PROJECT_DIR}`) and three skills; no hooks. F12
   (`project_w48f.rs`) checks the skills' `allowed-tools` against ADR 0135's 17 tools.
10. **The feed starts at its first call.** `read_feed` (`crates/rcfd-workspace/src/queries.rs` line 523) opens a `Feed` from the
    present; `journal::Reader::at_start` exists (`journal.rs` line 249); the entry made from one journal line is built inline in
    `Feed::poll` (`feed.rs` lines 361 to 387).

## The hook contract, read and checked (2026-11-06)

**Read** ([Claude Code hooks reference](https://code.claude.com/docs/en/hooks), read 2026-11-06): plugin hooks live in
`hooks/hooks.json` at the plugin root and merge with the user's and the project's hooks. A `PreToolUse` command hook gets one JSON
object on stdin with the common fields (`session_id`, `transcript_path`, `cwd`, `permission_mode`, `hook_event_name`, ...) and
`tool_name`, `tool_input`, `tool_use_id`; Write's input is `{file_path, content}`, Edit's `{file_path, old_string, new_string,
replace_all}`, file paths are absolute and on Windows use backslashes. Exit 2 blocks the call and Claude sees the hook's stderr
as the reason; exit 0 lets it through (with any JSON decision applied); any other exit, a failure to start or a timeout is a
non-blocking error (the call goes ahead, a "hook error" notice is shown) unless `onFailure` is `block` (v2.1.295 or later). A
command hook with `args` runs in exec form: `command` is found on `PATH` and spawned with `args`, no shell, and
`${CLAUDE_PROJECT_DIR}` and `${CLAUDE_PLUGIN_ROOT}` are substituted in `command` and each argument; on Windows `command` must be a
real executable. `CLAUDE_PROJECT_DIR`, `CLAUDE_PLUGIN_ROOT` and `CLAUDE_PLUGIN_DATA` are in the hook's environment. The default
timeout of a command hook is 600 s. The matcher `Edit|Write` (letters and `|` only) is a list of exact tool names.

**Checked on the installed build**, Claude Code 2.1.319 (`claude.exe` on PATH), with a probe: a plugin whose `hooks/hooks.json`
had one `PreToolUse` hook, matcher `Edit|Write|MultiEdit|NotebookEdit`, exec form `python <probe.py> ${CLAUDE_PROJECT_DIR}
exec-form`; the script logged its argv, environment and input and exited 2 with a message for a path under `cases/`. One
`claude -p` session in a scratch folder with `cases/` and `notes/`, `--plugin-dir <probe>`, `--permission-mode acceptEdits`,
asked for a Write of `cases/a.toml` and one of `notes/b.md` (cost reported: 0.17 USD). Seen:

* the plugin's hooks were loaded (debug log: "Read hooks.json for plugin f275probe (enabled=true)", "Registered 1 hooks");
* exec form: `python` was found on PATH; the arguments arrived one by one, `${CLAUDE_PROJECT_DIR}` replaced by the session's
  folder; `CLAUDE_PROJECT_DIR`, `CLAUDE_PLUGIN_ROOT` and `CLAUDE_PLUGIN_DATA` were set; the working folder was the session's;
* the input: `session_id`, `transcript_path`, `cwd`, `prompt_id`, `permission_mode` (`acceptEdits`), `effort` {`level`},
  `hook_event_name` `PreToolUse`, `tool_name` `Write`, `tool_input` {`file_path` (absolute, backslashes), `content`},
  `tool_use_id`;
* exit 2: the file was not written; the tool result had `is_error` true and the text `PreToolUse:Write hook error: [<command and
  arguments as written>]: <stderr>` followed by "This hook comes from the f275probe@inline plugin."; the debug log said
  `Hooks: PreToolUse:Write [...] (plugin f275probe@inline) finished with status 2 (173ms)`;
* exit 0: `notes/b.md` was written;
* the session's tools include `Edit`, `Write` and `NotebookEdit`, and no `MultiEdit`.

**Not checked**: Edit's input on this build (the documented shape is used) and NotebookEdit's (its path field `notebook_path` is
the name the Studio bridge already reads, `allow.rs` `suggest`); a hook under `bypassPermissions` or
`--dangerously-skip-permissions` (the page read does not say, and it is not run here: NOT VERIFIED); a plugin installed from the
marketplace rather than `--plugin-dir`; the JSON decision form (`permissionDecision: deny`), which is documented and not used.

## Choices of this registration

**P1. The hidden session tools (ADR 0173 D2 (b), used; C5).** With a project (`McpServer::with_project`, so `rcfd-mcp
--project`), the 16 tools of item 2 whose every use needs or makes a session are not listed, and `call_tool` treats them as any
unlisted name: -32602 `unknown tool '<name>'`, nothing run. Their equivalents:

| Hidden tool | Project equivalent | Its check |
|---|---|---|
| `create_case` | `new_case`, `start_run {new_case}` | ADR 0186 C-7, C-N |
| `set_setting` | `edit_case` | ADR 0135 F5, ADR 0185 C2 |
| `get_setting` | `read_project_file`; `edit_case` `dry_run` gives `values` | ADR 0135 F3, F5 |
| `load_mesh` | `import_mesh`, then `edit_case` of `mesh.source` and `mesh.file` | ADR 0187 C8-I, C8-E |
| `check_mesh` | `check_case_mesh` | ADR 0187 C8-C |
| `run` | `start_run`, `wait_run` | ADR 0186 C-7, ADR 0135 F11 |
| `status` | `run_status`, `wait_run` | ADR 0135 F3, F11 |
| `get_residuals` | `read_residuals` | ADR 0135 F3, F8 |
| `get_field_stats` | `field_summary` | ADR 0135 F10 |
| `render_image` | `render_field` | ADR 0135 F9 |
| `export_vtk` | `runs/<id>/output/solution.vtu` of every runner run | `runner_w47b.rs` line 520 |
| `run_headless` | `start_run` (the runner's run has a status stream, stop and pause files) | ADR 0135 F11; none for `continue_from`, `interpolate_*`, `checkpoint`, `checkpoint_every`, `out_dir`, `max_elapsed_s` (Q2) |
| `list_workflow_tasks`, `create_workflow`, `edit_workflow`, `run_workflow_task` | none (Q2) | none |

The list under `--project` is the 17 kept session tools in the order of `tools()`, then the 23 project tools: 40 tools. Every
kept descriptor is byte-identical to its entry without `--project`. Without a project nothing changes.

**P2. The kept session tools.** `list_cases`, `check_case_headless`, `read_status_stream`, `design_create_study`,
`design_create_variant`, `design_run_operating_map`, `design_run_study`, `design_evaluate`, `design_record`,
`design_get_metrics`, `design_compare_variants`, `design_propose`, `design_list_study`, `design_ledger_append`,
`design_ledger_query`, `design_ledger_export` and `design_ledger_rerun_check`: each has a use that reads or writes files only
(item 2). Their texts are kept as they are, with what no longer applies in project mode (Q3): the `session` arguments of
`design_create_study` and `check_case_headless` (no session can be made, so a call with one answers that the session does not
exist, and `check_case_headless` without `path` answers 1001 naming `create_case`); `design_record`'s "running the case through
the other tools"; `list_cases`, whose default folder `cases` is the project's `cases/` (no registry there) and whose templates are
`create_case`'s (the same names as `new_case`'s).

**P3. `initialize` under a project.** The first sentence becomes "Clean-room open source CFD solver." (it named `create_case`,
`check_mesh` and `run`), followed by the project sentence as it is. Without a project the text is unchanged.

**P4. Names (ADR 0186 Q1 (a), ADR 0187 Q1 (a), ADR 0188 Q1 (a), used).** `new_case`, `check_case_mesh`, `read_feed` and
`list_changes` keep their names; no project tool takes the name of a hidden one. The hook's message therefore names `edit_case`
and `new_case`, the project-mode name of ADR 0173's `create_case` command (Q1).

**P5. The Studio bridge's allow-list (ADR 0187 Q6 (b), ADR 0188 Q6 (b), used).** `PROJECT_READ_TOOLS` gains `read_feed`,
`check_case_mesh`, `preview_mesh` and `list_meshes`, appended after the 11 in the live list's order: 15 names, the read-class
project tools of the live list. `AllowList::project_initial` then makes 15 rules for a new project. An existing allow-list file is
not changed (Q4). The plugin's skills are unchanged.

**P6. The plugin hook (ADR 0173 D5 (b), used).** `clients/claude-plugin/hooks/hooks.json`:
`{"description": ..., "hooks": {"PreToolUse": [{"matcher": "Edit|Write|MultiEdit|NotebookEdit", "hooks": [{"type": "command",
"command": "rcfd-mcp", "args": ["--edit-hook", "${CLAUDE_PROJECT_DIR}"], "timeout": 30}]}]}}`. Exec form, as the probe ran:
`rcfd-mcp` is found on PATH as the plugin's server is, and the project folder arrives as one argument. `MultiEdit` (not in
2.1.319, `file_path` in builds that had it) and `NotebookEdit` (`notebook_path`) also write a file by path, so they are matched
too. `plugin.json`, `.mcp.json` and the skills do not change.

`rcfd-mcp --edit-hook [<folder>]` is handled in `main.rs` before anything else (no service, no project tools), by the new module
`crates/rcfd-mcp/src/edit_hook.rs`, std and `rcfd-project` only. In order:

1. Stdin is read whole. Not a JSON object with a string `tool_name`: exit 1, a message on stderr (a non-blocking error: Claude
   Code goes ahead and shows a hook error notice).
2. `tool_name` not one of `Edit`, `Write`, `MultiEdit`, `NotebookEdit`: exit 0, nothing printed.
3. The project folder: the argument when given and not empty, else the environment's `CLAUDE_PROJECT_DIR`, else the input's
   `cwd`. When it holds no `rcfd-project.json`: exit 0, nothing printed (the plugin may be on in any folder).
4. The file: `tool_input.file_path`, or `tool_input.notebook_path` for `NotebookEdit`. Missing or not a string: exit 1 with a
   message. A relative path is joined to the input's `cwd`, else to the project folder.
5. Two forms of the file and of the folder are compared, by whole components: the lexical form (`/` and `\` both separators,
   `.` dropped, `..` removing one component) and the resolved form (the deepest existing ancestor canonicalised, the rest
   appended). On Windows components compare without ASCII case and without trailing dots and spaces. The file is refused when,
   in either form, it lies under the folder and its first component below it is `cases`, `runs` or `designs`.
6. Refused: exit 2, nothing on stdout, on stderr one message: `rcfd: <tool> of <project-relative path> refused: in an rcfd
   project, cases/, runs/ and designs/ change only through the project tools, so that every change is validated, checked
   against the file's hash and kept in the project journal. Use edit_case to change a case's settings (read_project_file gives
   its base_hash) and new_case to create a case (from an example, a template, TOML, settings or a copy); start_run writes runs/
   and the design tools write designs/. meshes/, notes/ and other files can still be written.`
7. Otherwise exit 0, nothing printed.

**P7. `read_feed {from_seq}` (ADR 0188 Q7 (b), used: "later, with F-275's list").** A new optional property `from_seq`
(integer, minimum 1). With it, the feed is polled as without `since`, then the journal is read from its start, and the result is
{`from_seq`, `cursor`, `entries`, `total`, `next_seq`, `skipped_lines`}: `entries` are the journal lines whose `seq` is at least
`from_seq`, in file order, each the object a live feed gives for that line without `n` (a replayed line has no number in this
feed), at most `limit` and within the size budget under the same rule as with `since` (an entry over half the budget loses its
settings and other fields, with `omitted`); `total` counts those lines; `next_seq` is one more than the last `seq` returned when
lines are left, else null; `cursor` is the feed's latest number, from which `since` continues live (the poll comes first, so a
line written between the two reads appears in both, never in neither); `skipped_lines` counts the journal's lines that are not
JSON objects. Lines without `seq` (before ADR 0188, or appended without the journal lock) are not replayed. `since` and
`from_seq` together: -32602. Without `from_seq` every result of `read_feed` is as before. The descriptor's description is kept;
the property carries its own text. The entry made from a journal line moves into one function that the poll and the replay
both call, with the poll's output unchanged.

**P8. Access classes unchanged (Q6).** `start_run` stays `execute` though it can make a case (ADR 0186 addendum);
`design_run_study` and `design_evaluate` stay `write_files` though in project mode they start runs (ADR 0189 Q7).

**P9. ADR 0189 Q4 (a), as built.** The three flat run-record fields `study`, `variant` and `condition` stay as they are; nothing
changes in F-275; ADR 0130 still has the lead approve them.

**P10. Documents.** `docs/mcp-tools.md` (the project-mode list with the hidden tools and their equivalents, the hook, `from_seq`,
the allow-list), the F-275 entries of `docs/ROADMAP.md` and `docs/roadmap.json`.

**P11. Separable commits.** (1) this text; (2) the tool list and `initialize`, with the tests of C5-L, C5-H, C5-S and C5-I and
the edits of C-1; (3) the allow-list and C5-A; (4) `from_seq` and C5-F; (5) the hook, its plugin file and C5-K, C5-R; (6)
documents; (7) the addendum. Each builds and passes on its own.

## Open questions

**Q1. Names in project mode and the hook's message (P4).** ADR 0173 D5 says the message names `edit_case` and `create_case`;
under `--project` `create_case` is now hidden. (a) Keep `new_case` and `check_case_mesh` (the written recommendations of ADR 0186
Q1 and ADR 0187 Q1); the message names `edit_case` and `new_case`; (b) rename `new_case` to `create_case` and `check_case_mesh` to
`check_mesh` under `--project`, now that those names are free there (journal `who` values, feed texts, `start_run`'s `new_case`
argument, the C-L checks and the Studio's journal readers change); (c) both names, as aliases. Recommendation (a); reversible (a
rename). Proceeding on (a).

**Q2. Session tools with no project equivalent.** The four workflow tools, and `run_headless`'s `continue_from`, interpolation,
checkpoints, `out_dir` and `max_elapsed_s`. (a) Hidden with the rest: once `create_case` is hidden (D2 (b)), no session can be
made, so each of them could only answer 1001 naming a hidden tool; these capabilities are not offered in project mode until
project equivalents exist (a workflow file in the project; a runner run that continues from another, ADR 0189 Q2 (d)); (b) keep
`create_case` and every session tool listed until those equivalents exist, so C5 is not met; (c) build the equivalents first.
Recommendation (a), which follows from D2 (b) and C5; it removes capabilities, so it is the maintainer's to overturn. Proceeding
on (a).

**Q3. Kept descriptors that still speak of sessions (P2).** (a) Byte-identical to the list without `--project`; (b) project-mode
variants without the `session` arguments and texts. Recommendation (a): one descriptor per tool, and the list without the 16 is
checked byte for byte; reversible. Proceeding on (a).

**Q4. Existing allow-list files (P5).** (a) Not changed: in a project made before F-275 the four read tools still ask before they
run, until the user allows them; (b) the Studio adds the missing names of `PROJECT_READ_TOOLS` to an existing file when it opens
the project (a write to a control file, which ADR 0134 amendment 2 has the user confirm). Recommendation (a). Proceeding on (a).

**Q5. What the hook covers (P6).** (a) D5 (b) as written: Claude Code's file tools under `cases/`, `runs/` and `designs/`;
writes by Bash or PowerShell and files under `.rcfd/` are not covered; (b) also `.rcfd/`; (c) also shell commands (their targets
cannot be read reliably from a command line). Recommendation (a). Proceeding on (a).

**Q6. Access classes (ADR 0186 addendum, ADR 0189 Q7).** Neither ADR writes a recommendation; both leave the question to F-275's
list. (a) Unchanged (P8); (b) `design_run_study` and `design_evaluate` class `execute` under `--project`, and `start_run` with
`new_case` also checked as `write_files` (a class per call, which the policy does not have). Recommendation (a): the class feeds
`_meta` and a hook policy only; the read and not-read split and `needsApproval` do not change; a class that depends on
`--project` would change descriptors that P1 keeps byte-identical. Proceeding on (a).

**Q7. When the hook cannot decide.** (a) Fail open: a missing `rcfd-mcp`, an `rcfd-mcp` from before F-275 (it would ignore
`--edit-hook`, serve stdio, read the input as a malformed request, reach the end of stdin and exit 0), unreadable input or a
timeout let the edit go ahead, by Claude Code's rule for those outcomes; (b) `onFailure: "block"` (v2.1.295 or later): a missing
`rcfd-mcp` would block every Edit and Write wherever the plugin is on. Recommendation (a). Proceeding on (a).

## Decisions needed

Each item is an answer this text takes, for the maintainer to confirm or overturn.

1. ADR 0173 D2 (b) (its recommendation): with `--project` the 16 session tools of P1 are hidden and uncallable; 40 tools remain.
2. Q2 (a): the workflow tools and `run_headless`'s restart, interpolation and checkpoint options are not offered in project mode.
3. ADR 0173 D5 (b) (its recommendation): the plugin hook of P6.
4. Q1 (a), from ADR 0186 Q1 (a) and ADR 0187 Q1 (a): `new_case` and `check_case_mesh` keep their names; the hook names
   `edit_case` and `new_case`, not `create_case` as D5 and the roadmap say.
5. ADR 0188 Q1 (a): `read_feed` and `list_changes` both stay.
6. ADR 0187 Q6 (b) and ADR 0188 Q6 (b): the four read tools join `PROJECT_READ_TOOLS`, so in a new project the Studio's sidebar
   runs them without asking (P5); Q4 (a): existing allow-list files are not changed.
7. ADR 0188 Q7 (b): `read_feed {from_seq}` replays the journal (P7).
8. ADR 0189 Q4 (a): the three flat run-record fields, as built; they still need the lead's approval (ADR 0130).
9. Q6 (a): access classes unchanged; no recommendation was written in ADR 0186 or ADR 0189.
10. Q3 (a), Q5 (a), Q7 (a).
11. Q8 (amendment 1): with 15 first rules, a rule the user adds is below the fold of the Studio's confirmation dialog until
    scrolled; not acted on.

## Acceptance checks and limits (fixed here, before any code)

* **C5-L (the list, `rcfd-mcp` binary).** Under `--project`: 40 tools; their names are the names of
  `golden/tools_list_default.json` without the 16 of P1, in that order, then the 23 project tools in the order of the F-273
  list; each kept session tool is byte-identical to its entry in that golden; none of the 16 is listed. `project_list_f270.rs`,
  restated: the live list with the 16 put back from the default golden at their places and `read_feed`'s `from_seq` property
  removed is the F-273 list that file requires, byte for byte, with every check it makes (the goldens are not re-recorded).
  Without `--project`, `default_f1.rs` passes unedited.
* **C5-H (hidden means uncallable).** Through the binary under `--project`, each of the 16 called with arguments its schema
  accepts: a JSON-RPC error -32602 whose message names the tool as unknown; the project folder's files and bytes are the same
  before and after. Control: the same calls on a server without `--project` reach their handlers (no -32602).
* **C5-S (what needs a session).** On a server without a project and without a session: each of the 15 hidden tools other than
  `create_case`, called with arguments of a table, answers 1001; `create_case` makes a session. On another such server each of the
  17 kept tools, called with arguments of a table that hold no `session` (a case file, a status stream file and a study folder
  made by the test), gives no 1001 (any other result is allowed), and `session.list` is empty after the 17 calls.
* **C5-I (`initialize`).** Under `--project` the instructions start with "Clean-room open source CFD solver. This server also
  has a project folder", none of the 14 hidden names that hold `_` is a word of them (words split at every character other than
  a letter, a digit or `_`), and the old sentence "create_case, adjust settings, check_mesh, run" is not in them. Without `--project` the text is unchanged (`default_f1.rs`).
* **C5-A (allow-list).** `PROJECT_READ_TOOLS` is the 11 of main in their order, then `read_feed`, `check_case_mesh`,
  `preview_mesh`, `list_meshes`; as a set it equals the read-class project tools of the live `--project` list (F2 restated);
  `AllowList::project_initial()` has 15 rules. A file written from main's 11 names (the test writes it as main's
  `project_initial` did) reads back as those 11 rules and is written back byte for byte.
* **C5-F (`from_seq`).** Workspace: a project whose journal holds a line as main 5fdde2d wrote it (no `seq`), then five lines
  from workspace commands with an actor (seq 1 to 5: `edit_case` with a reason, `write_note`, `new_case`, `import_mesh` of a Gmsh
  sample, `edit_case`); a feed opened before those commands gives five journal entries. A new workspace's `read_feed
  {from_seq: 1}`: five entries with `seq` 1 to 5 in file order, none with `n`, each equal to the live feed's entry for the same
  line without `n`; `total` 5, `next_seq` null, `cursor` equal to the cursor `read_feed {}` gives. `{from_seq: 3, limit: 2}`: `seq`
  3 and 4, `total` 3, `next_seq` 5. `{from_seq: 6}`: no entry, `total` 0, `next_seq` null. `{since: 0, from_seq: 1}` and
  `{from_seq: 0}`: -32602. Through the binary: an `edit_case` with a reason made by one `rcfd-mcp --project` process is in a
  second process's `read_feed {from_seq: 1}` with that reason and the first process's actor. `feed_f273.rs` and the workspace's
  `journal_f273.rs` pass unedited.
* **C5-K (the hook, through the binary).** Inputs shaped as the probe's (every key it saw, `tool_input` as documented), files in
  a project made by the test, the folder as the argument unless said otherwise.
  * Refused (exit 2, stdout empty, stderr naming `edit_case`, `new_case` and the project-relative path), 12 inputs: Write
    `cases/new.toml` (absent); Edit `cases/cavity.toml` (present); Write `runs/r-20260101T000000-0000/run.json`; Write
    `designs/s/study.json`; NotebookEdit `cases/n.ipynb` (`notebook_path`); MultiEdit `cases/cavity.toml`; Write with `/`
    separators; Write `notes/../cases/a.toml`; Write `cases/a.toml` given relative with `cwd` the project; Write `cases/new.toml`
    with no argument and the folder in `CLAUDE_PROJECT_DIR`; the same with neither and the folder as `cwd`; Write
    `notes/link/a.toml` where `notes/link` is a directory link to `cases` (a junction on Windows, a symlink elsewhere; skipped
    with a message when the link cannot be made). On Windows also `CASES\New.Toml` and `cases.\a.toml` (13 and 14).
  * Allowed (exit 0, stdout and stderr empty), 10 inputs: Write `notes/a.md`; Write `meshes/a.msh`; Write `cases.md` at the root;
    Write `casesx/a.toml`; Write `README.md`; Write `cases/../notes/a.md`; Write a file outside the project; `Read` of
    `cases/cavity.toml`; `Bash`; Write `cases/new.toml` in a folder without `rcfd-project.json`.
  * Non-blocking error (exit 1, stdout empty, stderr not empty), 2 inputs: text that is not JSON; a Write without `file_path`.
  * The plugin file: `hooks/hooks.json` parses; one `PreToolUse` entry; matcher `Edit|Write|MultiEdit|NotebookEdit`; one hook,
    `type` `command`, `command` `rcfd-mcp`, `args` `["--edit-hook", "${CLAUDE_PROJECT_DIR}"]`. F12 passes unedited.
* **C5-R (a real session; an ignored test, run by hand once, numbers in the addendum).** `claude` on PATH (2.1.319 at this
  registration), the branch's `rcfd-mcp` first on PATH, the plugin by `--plugin-dir clients/claude-plugin`, a new project with
  `cases/cavity.toml`, `claude -p` with `--permission-mode acceptEdits`, `--output-format stream-json` and `--debug-file`,
  asking for (1) a Write of `cases/new.toml`, (2) an Edit of `physics.nu` in `cases/cavity.toml`, (3) a Write of `notes/ok.md`.
  Pass: (1) and (2) each give a tool result with `is_error` true whose text holds `edit_case` and `new_case`, `cases/new.toml`
  does not exist and `cases/cavity.toml` keeps its bytes; (3) is written; the debug log has two `Hooks: PreToolUse:` lines of the
  rcfd plugin ending `finished with status 2`. If the model does not make the three calls the test fails and says so; if
  `claude` cannot run (not logged in, not found) it fails with that message and the addendum reports C5-R as not run.
* **C-1 (parity).** With no edit to them: `default_f1.rs`, `feed_f273.rs`, `new_simulations_f271.rs`, `design_runner_f274.rs`,
  `runner_stdio_w48l.rs`, `sv301_mcp_w48z.rs`, `correctors_w49j.rs`, the unit tests of `rcfd-mcp`, and every test of
  `rcfd-workspace`, `rcfd-studio`, `rcfd-claude` and `rcfd-project`. Edited by design, and only there: `project_list_f270.rs`
  (C5-L); `project_runs_w48f.rs` (the two counts 56 to 40; R1's probe uses `check_case_headless {path}` in place of
  `create_case {path}`, with the same three outcomes); `project_w48f.rs` (F3's count `33 + 23` to `17 + 23`; F2's read set taken
  from the project tools of the live list); `mesh_setup_f272.rs` (the observation of lines 123 to 132 becomes: under the project,
  `create_case` is an unknown tool, -32602). `workspace_graph.rs` passes.
* **Can fail.** Four mutations, each restored and the file compared byte for byte with the kept one afterwards: (1) P1's filter
  removed: C5-L and C5-H fail; (2) the hook's component comparison made a text prefix: the `casesx/a.toml` input of C5-K is
  refused; (3) the hook's resolved form removed: the link input of C5-K is allowed (where the link can be made); (4) `from_seq`
  taken as exclusive (`seq > from_seq`): C5-F fails. `scripts/limit_audit.py` is run on the new tests.
* **Defaults.** No solver result, case hash or existing case byte changes; `baseline_hashes.rs`, `case_hashes_w48u.rs` and the
  H1 hashes untouched; no case-schema key or run-record field added; `Cargo.lock` unchanged; without `--project` `tools/list` and
  `initialize` byte-identical. The full gate (fmt per package, clippy with `-D warnings`, every test in release on a
  `git archive` copy, one process per test, three jobs) reports no failure; a flaky failure is run again alone and reported with
  its name.

## Not in F-275

Project equivalents of the workflow tools and of `run_headless`'s restart options (Q2); a runner run that continues from another
(ADR 0189 Q2 (d), Q3 (d)); journal lines for design runs and records (ADR 0189 Q6); adopting runs that outlive a call (ADR 0189
Q5); renaming project tools (Q1 (b)); a hook for `.rcfd/` or shell writes (Q5); migrating existing allow-list files (Q4); a link
between an `mcp` actor and a Claude Code session (ADR 0188 Q4).

## Where to go next

* [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) for the layer and its decisions.
* [ADR 0135](0135-wave48f-plugin-and-project-tools.md) for the plugin and the project tools; [ADR 0134](0134-wave48e-claude-code-sidebar.md)
  for the Studio's allow-list.
* [MCP tools](../mcp-tools.md) for both tool families.

## Amendment 1 (2026-11-06, before the change it covers)

The first run of `rcfd-studio`'s tests after P5 found an existing test that C-1 did not name: `crates/rcfd-studio/tests/chat_e.rs`
`e11_confirmation_dialog_gates_the_first_chat` (lines 550 to 584) writes `AllowList::project_initial()` plus `Bash(echo hi)` as
the project's allow-list file and checks that the confirmation dialog's first frame shows `Bash(echo hi)`. The dialog lists the
file's rules inside a scroll area 440 pixels high (`crates/rcfd-studio/src/chat_ui.rs` lines 414 to 461). With 15 first rules the
file has 16, and the first frame shows the rules down to `preview_mesh` (the 14th); `list_meshes` and `Bash(echo hi)` lie below
the fold and are not painted, so the check fails, the same way alone as in the run (deterministic, not a flake).

C-1 therefore covers one more edit by design: e11's file is main's 11 read-tool rules, built from the names as C5-A builds them,
plus `Bash(echo hi)`, so the test keeps checking what it checked (a 12-rule file shown whole, the extra rule visible). Nothing
else of `chat_e.rs` changes, and no Studio source file changes.

What the test found stays a question for the maintainer, **Q8. The confirmation dialog's fold.** In a project made after F-275 the
allow-list file starts with 15 rules, so a rule the user adds later (for example a `Bash(..)` or `Edit(..)` rule, the ones worth
reading) is the 16th or later and is below the fold of the confirmation dialog until the user scrolls. (a) Leave it: the list
scrolls and its count is in the heading; (b) list the rules that are not the project's read tools first, or show the read tools
as one line; (c) a taller area. Recommendation (b), a Studio change after the maintainer's Studio worktree lands; not acted on in
F-275. It joins [Decisions needed](#decisions-needed) as item 11.

## Addendum 1: what was built, and the results (2026-11-06)

Branch `f275-project-mode-tools`, after this text (`9bfb97f`):

* `9e23b2c`: the project-mode tool list and `initialize` (P1 to P3), with C5-L, C5-H, C5-S and C5-I and the C-1 edits;
* `a2a2f82`: the allow-list (P5) and C5-A, F2 restated;
* `fb802bf`: `read_feed {from_seq}` (P7) and C5-F;
* `4525e4c`: the edit hook and its plugin file (P6), C5-K and C5-R;
* `584b306`: C5-A through `project_initial` (below, difference 5);
* `28f9de0`, `00fbd30`: the documents;
* `2fa8742`, `92953fd`: amendment 1 and the `chat_e.rs` e11 edit it covers.

The gate ran on `00fbd30`. The branch is not merged.

**Built as registered.** P1 to P11 as written, with amendment 1.

* `rcfd-mcp`: `SESSION_ONLY` (the 16 names) and the filter in `all_tools`; `initialize`'s first sentence under a project;
  `read_feed`'s `from_seq` property; the new module `src/edit_hook.rs` and the `--edit-hook` branch of `main.rs` (one help line).
* `rcfd-workspace`: `feed::journal_entry` (the entry of one journal line, which `Feed::poll` now calls) and `from_seq` in
  `read_feed`, whose page rule moved into one function both modes use.
* `rcfd-claude`: `PROJECT_READ_TOOLS` 15 names.
* `clients/claude-plugin/hooks/hooks.json` (new); `plugin.json`, `.mcp.json` and the skills unchanged.
* Existing tests edited, and only these: `project_list_f270.rs` (C5-L), `project_runs_w48f.rs` (two counts, R1's probe),
  `project_w48f.rs` (F3's count, F2's read set), `mesh_setup_f272.rs` (the observation), `chat_e.rs` e11 (amendment 1). New tests:
  `rcfd-mcp/tests/tool_list_f275.rs` (6), `rcfd-mcp/tests/edit_hook_f275.rs` (1, and the ignored C5-R),
  `rcfd-workspace/tests/feed_replay_f275.rs` (1), one unit test in `edit_hook.rs`.

**Differences found while building.**

1. Amendment 1: `chat_e.rs` e11 depended on the length of the first allow-list (Q8).
2. C5-F refused a third input besides the two registered: `from_seq` given as a string (-32602).
3. C5-S: two kept tools answered with an error other than 1001 on the test's arguments, as the registration allows:
   `read_status_stream` (-32602; the test's one-line file is not a status stream) and `design_compare_variants` (-32602).
4. Public names the registration did not list: `rcfd_mcp::SESSION_ONLY`, the module `rcfd_mcp::edit_hook` and
   `rcfd_workspace::feed::journal_entry`. The tests keep their own tables and do not read `SESSION_ONLY`.
5. C5-A first compared `PROJECT_READ_TOOLS` with a constant; `scripts/limit_audit.py` reported those two assertions as class (i),
   and the check now reads the rules `project_initial` makes.
6. C5-R confirmed Edit's input on 2.1.319 (`replace_all`, `file_path`, `old_string`, `new_string`) and that the plugin's MCP server
   started in the same session (`plugin:rcfd:project` connected). `MultiEdit` and `NotebookEdit` are checked through the binary
   only (C5-K): 2.1.319 has no `MultiEdit` and the session did not use `NotebookEdit`.
7. The probe of the registration and C5-R each ran one `claude -p` session: 0.17 USD and 0.22 USD as the CLI reported.

**Results.**

| Check | Result |
|---|---|
| C5-L | Through the binary under `--project`: 40 tools, 36 970 bytes; the 17 kept session tools in the default list's order, each byte-identical to its entry in `tools_list_default.json`; then the 23 project tools in the F-273 order; none of the 16 listed. `project_list_f270.rs`: with the 16 put back from the default golden and `from_seq` removed (an integer, minimum 1), the 56-tool F-273 list passes every check of that file, byte for byte; neither golden re-recorded; `default_f1.rs` passes unedited |
| C5-H | Each of the 16 under `--project`: JSON-RPC -32602 `unknown tool '<name>'`; the project's files and bytes the same before and after. Control (a server without `--project`): all 16 reached their handlers |
| C5-S | Without a project or a session: the 15 hidden tools other than `create_case` answered 1001, `create_case` made a session. The 17 kept tools without `session`: no JSON-RPC error and no 1001 (15 answered without error, two with -32602, difference 3); `session.list` empty after them |
| C5-I | Under `--project`: 852 characters starting "Clean-room open source CFD solver. This server also has a project folder"; none of the 14 hidden names with `_` is a word of them; the old sentence absent. Without `--project` unchanged (`default_f1.rs`) |
| C5-A | A new project's first list: 15 rules, main's 11 in their order, then `read_feed`, `check_case_mesh`, `preview_mesh`, `list_meshes`. F2: the read-class project tools of the live list are those 15. A file written from main's 11 names (464 bytes) read back as the 11 rules and written back byte for byte |
| C5-F | Workspace: after an old line without `seq` and five commands (seq 1 to 5), a workspace made afterwards replayed exactly the five lines, in file order, without `n`, each equal to the live feed's entry for the line; `total` 5, `next_seq` null, `skipped_lines` 0, `cursor` equal to `read_feed {}`'s; `{from_seq: 3, limit: 2}` gave seq 3 and 4, `total` 3, `next_seq` 5; `{from_seq: 6}` nothing; three refusals -32602 (difference 2). Binary: a second server's `read_feed {from_seq: 1}` gave the first server's edit (seq 1, `edit_case`, its reason, actor `mcp` with the first process's id, `changes` [{`physics.nu`, 0.01, 0.02}]) while its live feed from `since: 0` was empty |
| C5-K | 14 refused with exit 2, an empty stdout and a message naming `edit_case`, `new_case` and the path (the 12 registered, the link input as a directory junction, and the two Windows inputs); 10 allowed with exit 0 and nothing printed; 2 not read with exit 1 and a message. `hooks/hooks.json` as registered; F12 passes unedited |
| C5-R | Claude Code 2.1.319, the plugin loaded as `rcfd@inline` with its MCP server connected, the branch's `rcfd-mcp` first on PATH (no other on PATH). The Write of `cases/new.toml` and, after a Read, the Edit of `cases/cavity.toml` came back with `is_error` true and the text `PreToolUse:<tool> hook error: [rcfd-mcp --edit-hook ${CLAUDE_PROJECT_DIR}]: rcfd: <tool> of cases/<file> refused: ...` (naming `edit_case` and `new_case`) and "This hook comes from the rcfd@inline plugin."; `cases/new.toml` absent, `cases/cavity.toml` byte for byte as before; `notes/ok.md` written. Debug log: `Hooks: PreToolUse:Write [...] (plugin rcfd@inline) finished with status 2 (30ms)` and the same for Edit (22 ms). 14.9 s, 0.22 USD |
| C-1 | The registered edits only (C5-L, two counts and R1, F3 and F2, the `mesh_setup_f272.rs` observation) and amendment 1's. Release run of `rcfd-mcp`, `rcfd-workspace`, `rcfd-claude`, `rcfd-project` and `rcfd-studio` before amendment 1: 492 passed, 1 failed (e11, amendment 1), 34 ignored; after it `chat_e.rs` 14 of 14. `workspace_graph.rs` passed in the gate |
| Can fail | Four mutations, each file restored and compared with the committed one afterwards (equal). (1) P1's filter removed: C5-L and C5-H failed, the other four passed. (2) The hook's component comparison made a text prefix: C5-K failed at the allowed `casesx/a.toml` (refused). (3) The hook's resolved form dropped: C5-K failed at the link input (exit 0, not 2). (4) `from_seq` exclusive: C5-F failed in the workspace (seq 2 to 5 for 1 to 5) and through the binary |
| Defaults | Against f0d7983: `Cargo.lock`, `baseline_hashes.rs`, `case_hashes_w48u.rs` and both MCP goldens unchanged (`git diff` empty); no case-schema key or run-record field added; without `--project` `tools/list` and `initialize` byte-identical (`default_f1.rs`). `scripts/limit_audit.py`: 154 hits, as on main f0d7983, none in the new tests |
| Gate | `00fbd30` (the code of this branch and the documents before this addendum), `git archive` copy, three jobs (`scripts/gate_test.py`, one process per test): fmt 0, clippy `-D warnings` 0, 2918 passed, 0 failed, 821 ignored, 2918 test processes in 564 binaries, 82 min wall (69 min of tests); lowest free commit 53 GB. The 9 tests of F-275 (and the ignored C5-R) in 3 new binaries; no failure, so nothing was run again |

**Questions for the maintainer.** The answers of [Decisions needed](#decisions-needed), items 1 to 11, stand as registered.
In order of weight: Q2 (the workflow tools and `run_headless`'s restart options are not offered in project mode); item 6 with Q8
(the Studio's sidebar now runs four more read tools without asking in a new project, and a rule the user adds falls below the
fold of the confirmation dialog); Q1 (the hook names `new_case`, not `create_case`); Q6 (access classes); item 8 (the lead's
approval of the three run-record fields of ADR 0189). The hook under `bypassPermissions` stays NOT VERIFIED.