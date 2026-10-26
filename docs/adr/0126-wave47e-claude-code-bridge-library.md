# ADR 0126: Wave 47E, the Claude Code bridge library `rcfd-claude` (Studio layer L6)

Status: **ACCEPTED 2026-10-26** (limits registered after the rule-d smoke and before the registered runs). Written at main f307f59 on
branch `w47e`, target `<target dir>`. House rules of ADRs 0110-0117: limits fixed before the registered runs; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment before the corrected
run; defaults of existing code are bit-identical (this wave adds a new crate and touches no existing crate). Parents: the Studio plan
[`docs/ui-restart-plan.md`](../ui-restart-plan.md) sections 3 (layer L6), 7 (what the Claude Code documentation says) and 8 (user
decisions of 2026-10-26).

## Context

* User decisions (plan section 8): (1) the Studio sidebar may type into a Claude Code session that is CLOSED, by resuming it with
  `claude -p --resume`, and only mirrors read-only a session that another process has open: one writer per session; (2) tool approvals
  use a conservative allow-list first. Never `bypassPermissions`, never auto-approval of everything. An Agent SDK sidecar is not in scope.
* Plan section 7: the transcript format is declared internal and version-dependent; the stream-json events have no published schema;
  the `--permission-prompt-tool` contract is not documented. Nothing may depend on a shape that was not observed.
* No Rust SDK exists; the library spawns the CLI.

## Observed on the installed build (Claude Code 2.1.310, 2026-10-26)

Method: `claude --version`, `claude --help`, `claude -p --help` (the same options without the subcommand list), `claude agents --help`,
`claude agents --json` (no model calls), then **five** real headless calls with `--model haiku` (resolved to `claude-haiku-5-5`) in the
scratch folder `<scratch dir>`. Total reported cost 0.0241 USD. Conditions: the calls ran from inside a Claude Code
session, so they inherited `CLAUDECODE` and several `CLAUDE_CODE_*` session and messaging variables; the driver passes the environment
through unchanged, so a Studio started from a Claude Code terminal behaves as observed. The user settings held no permission rules.

| Call | Flags (besides `-p --model haiku --output-format stream-json --verbose`) | What it showed |
|---|---|---|
| 1 | `--include-partial-messages`, prompt as argument | event sequence of a text turn; `result` with `session_id`, `total_cost_usd` |
| 2 | `--include-partial-messages --resume <id of 1>` | resume keeps the session id (init and result); the model saw turn 1 |
| 3 | `--include-partial-messages --resume <id> --permission-mode default` | Bash with a redirection and Write both denied: `system/permission_denied` events, `permission_denials` in `result` |
| 4 | `--include-partial-messages --input-format stream-json --session-id <new uuid> --permission-mode dontAsk --allowedTools "Edit(./note.txt)" "Bash(echo hello)"`, prompt on stdin | `Edit(./note.txt)` allowed a **Write** of `note.txt`; `echo hello` allowed by rule; `echo bye` **also accepted** although no rule named it (`permission_decision.reason_type: "subcommandResults"`: the CLI's own read-only command classification) |
| 5 | `--resume <id of 4> --permission-mode dontAsk --allowedTools "Edit(./note.txt)"`, no partial messages; `claude agents --json` polled six times during the run | Write of `other.txt` denied with `reason_type: "mode"`; while running, the `-p` process was listed with `kind: "interactive"`, `status: "busy"` and its pid; after exit it was gone |

No schema is published. The shapes below are what these calls produced; the library takes only the fields named here.

| Source | Shape (fields relied on in **bold**) |
|---|---|
| stream, first line | `type: "system"`, `subtype: "init"`, **`session_id`**, **`permissionMode`** (`default` / `dontAsk`), **`claude_code_version`** (`2.1.310`), **`model`**, plus cwd, tools, mcp_servers, plugins, memory paths, a messaging pipe path (not used, not stored) |
| stream, text | `type: "stream_event"`, `event.type: "content_block_delta"`, `event.delta.type: "text_delta"`, **`event.delta.text`** (only with `--include-partial-messages`) |
| stream, tool call | `type: "assistant"`, `message.content[]` with `type: "tool_use"`, **`id`**, **`name`**, **`input`** (each observed `assistant` line carried one content block) |
| stream, denial | `type: "system"`, `subtype: "permission_denied"`, **`tool_name`**, **`tool_use_id`**, **`message`**; no tool input (it is taken from the matching tool call) |
| stream, end | `type: "result"`, **`subtype`** (`success`), **`is_error`**, **`result`** (text), **`session_id`**, **`total_cost_usd`**, **`permission_denials[]`** (`tool_name`, `tool_use_id`, `tool_input`) |
| stream, ignored | `system/status`, `system/thinking_tokens`, `rate_limit_event`, `user` (tool results, with `tool_result_meta[].permission_decision`), other `stream_event` types |
| transcript | `~/.claude/projects/<encoded scratch dir>/<session-id>.jsonl`: lines with **`type`** `user` / `assistant` carry **`message.content`** (string, or blocks `text` / `tool_use` with **`name`** / `tool_result` / `thinking`) and **`timestamp`**, plus `cwd` (used to confirm the folder); bookkeeping types `queue-operation`, `attachment`, `atis-latch`, `api-request-shape`, `api-request-blob`, `api-request`, `last-prompt`, `cost-state`, `mode` (the `api-request-blob` lines also carry `message.role` and hold full prompts) |
| `claude agents --json` | JSON array of **`sessionId`**, **`pid`**, **`cwd`**, `kind`, `startedAt`, **`name`**, `status` (the `id` and `state` fields named in the docs did not appear for these sessions) |
| `--help` | `--permission-mode` choices `acceptEdits`, `auto`, `bypassPermissions`, `manual`, `dontAsk`, `plan` (`default` accepted although not listed); `--permission-prompts` `host` or `none`; `--permission-prompt-tool` is **not listed** |

Fixtures: the five streams, the two transcripts and one listing, scrubbed (paths replaced by `C:\proj\demo`, plugin, MCP, memory,
account and environment fields emptied, thinking signatures and `api-request-blob` contents replaced) and kept under
`crates/rcfd-claude/tests/fixtures/v2.1.310/`, which pins the version.

## Decisions

1. **D1 crate** `rcfd-claude` (library, std + `serde_json` only; no new third-party dependency). Modules: `discover`, `transcript`,
   `stream`, `allow`, `driver`. No UI.
2. **D2 permission flags.** Every chat process gets `--permission-mode dontAsk --permission-prompts none` and, when the list is not empty,
   `--allowedTools` with one rule per argument. Nothing else about permissions is ever passed. `check_args` is a separate function
   from the mapping. It runs before every spawn and refuses:
   * any argument containing `dangerously` or `bypassPermissions`;
   * `--settings`, `--permission-prompt-tool` and `--add-dir`;
   * `--allowedTools=` in its comma form;
   * a permission mode other than `dontAsk` and prompts other than `none`;
   * any `--allowedTools` value that does not parse back into a valid rule.

   The driver also checks the `permissionMode` of the init event and kills the process (`Aborted`) unless it is `dontAsk`: settings in
   the project folder must not change the mode silently.
3. **D3 typed allow-list** `AllowRule`: `Tool(name)`, `Bash(cmd)`, `PowerShell(cmd)`, `Edit(path)` and `Read(path)`, mapped to `name`,
   `Bash(cmd)`, `PowerShell(cmd)`, `Edit(./path)` and `Read(./path)`. Rejected:
   * a bare `Bash`, `PowerShell`, `Edit`, `Write`, `MultiEdit`, `Read`, `NotebookEdit` or `WebFetch`;
   * a wildcard command name;
   * a file pattern that is absolute, uses `~`, `\` or `:`, contains `..`, or has no literal character;
   * `(`, `)` or `,`, control characters, leading or trailing blanks, a leading `-`, and bypass tokens.

   The rules are a floor that the app controls, not the whole gate. User and project settings add rules, and the CLI approves some
   read-only commands itself (call 4).
4. **D4 denial suggestion.** A denied call becomes a `PermissionDenied` event with `suggestion`, the narrowest exact rule:
   * Bash: the exact command;
   * Write or Edit: `Edit(<project-relative path>)`;
   * Read: `Read(<path>)`.

   There is no suggestion when the path is outside the project or the command contains wildcards. Accepting a suggestion only adds that
   rule to the list.
5. **D5 one writer.** `resume` is refused with `Error::SessionOpen { session_id, pid }` when `claude agents --json` lists the session
   (in any cwd) with a live pid. It is refused with `Error::OpenStateUnknown` when the listing or the pid check fails: if the state cannot
   be known, nothing is written. This refines the plan's "type only when closed". `new_session` generates the UUID itself and passes
   `--session-id`. Session ids are validated as UUIDs before they reach a command line. The prompt goes to stdin as one stream-json line,
   never as an argument, and stdin is closed (one process per message).
6. **D6 discovery.** Two sources, each allowed to fail with a warning:
   * the transcript directory `<home>/projects/<cwd with non-alphanumerics replaced by '-'>`. Each transcript is confirmed by the `cwd`
     on its first lines, because the naming is not injective;
   * `claude agents --json` filtered by cwd (separators ignored, ASCII case ignored on Windows).

   The state is OPEN (listed with a live pid), CLOSED or UNKNOWN (listing failed). Pid liveness comes from `tasklist /FI "PID eq n"`
   (Windows) or `ps -p` (elsewhere), with no `unsafe`. An unfiltered `tasklist` took 2.5 s on this machine; a filtered one takes 0.3 s.
7. **D7 cancel** kills the process tree with `taskkill /PID <child> /T /F`, using the pid of our own child while its handle is held
   (Unix: a process group and `kill -KILL -<pgid>`). Then it reaps the child. Dropping a `Chat` cancels it.
8. **D8 tolerant readers.** The stream parser and the transcript reader return `Unrecognised { kind }` for any line of unknown or changed
   shape. They never return an error or panic for content. The transcript tail reads from the end in a window that doubles from 256 KiB.
   It drops an unterminated last line, and `read_new` follows a file that another process is writing.

## Registered checks

Unit and fixture tests never call the real CLI. The driver tests use `fake-claude`, a `[[bin]]` of the crate (`test = false`) that
replays a fixture stream. Its path is passed as `Claude::program`; the user's PATH is never changed.

| Id | Test | Set-up | Limit and reason | Negative control |
|---|---|---|---|---|
| T1 | `allow::tests::t1_*` | table of 9 rules -> flag values, the full argument list, round trip through `AllowRule::parse` | 0 mismatches (exact mapping) | a deliberately wrong mapper (Edit without `./`) gives exactly 1 mismatch |
| T2 | `allow::tests::t2_*` | 22 broad or injecting rules | all rejected by `validate` and `add` | T1 rules are accepted |
| T3 | `allow::tests::t3_*` | generated space: 70 candidate rules (14 adversarial fragments x 5 kinds), stepped 4-subsets | no generated argument list fails `check_args` or contains `dangerously` / `bypassPermissions`; first four arguments are exactly `--permission-mode dontAsk --permission-prompts none`; space >= 5000 lists and >= 10 000 rule insertions (smoke: 7469 / 13 294) | 15 hand-built bad lists (bypass, auto, acceptEdits, `=` forms, prompts host, settings, prompt tool, add-dir, bare Bash, wildcard Edit, comma form) are all refused |
| T4 | `allow::tests::t4_*` | suggestions from tool inputs | exact rules inside the project; `None` outside, with wildcards, multi-line, missing input, bare WebFetch | the outside path and the wildcard command |
| P1 | `fixtures_v2_1_310::p1_stream_*`, `p1_transcript_*` | every fixture | version `2.1.310`; zero unrecognised lines; init and result ids equal; deltas rebuild the result text (calls 1-4); tool calls and denials per call as in the table above; the transcript message sequence (role, first 24 characters, tools) equal to the recorded one; every message has a timestamp | P2 |
| P2 | `p2_mutated_*` | each of the 24 transcript message lines x 6 targeted mutations (truncated, `type` removed, `message` retyped, content retyped, `message` renamed, new type) and 200 random byte mutations (seed 0x47E); stream lines with a recognised event x 3 (truncated, `type` removed, the relied field removed) and 200 byte mutations | no panic; 144 / 144 targeted transcript and >= 60 targeted stream mutations reported as unrecognised | P1 shows the unmutated lines are recognised |
| P3 | `p3_tail_and_incremental_read` | a 2 MB file (messages then 2000 skipped 1 KB lines); a file with an unterminated last line, then completed | tail(3) equals the last 3 messages; the partial line is not returned until completed, then `read_new` returns exactly it | the same file read whole |
| P4 | `p4_stream_parser_throughput` | 1000 copies of the call-3 stream (30.5 MB, 34 000 events) | >= 10 MB/s in release builds (timing not asserted in debug). Reason: a turn streams far under 1 MB/s (call 3: 30 KB in about 4 s), so 10 MB/s is more than 100x headroom, and the gate runs under load; smoke 260 MB/s in release, 26.6 MB/s in debug | event count equals 1000 x the single-stream count |
| D1 | `driver_fake::d1_*` | new session, two rules, a prompt that looks like flags | argv recorded by the fake equals `Chat::args`; `--session-id` equals `Chat::session_id`, no `--resume`; prompt only on stdin; Init dontAsk, 3 tool calls, 1 result, `Exited` code 0 last | D4 |
| D2 | `d2_*` | listings with our own live pid / a dead pid / no entry / listing failure / a non-UUID id | open -> `SessionOpen` with that pid and no chat process started; dead or absent -> resumed with `--resume <id>`; failure -> `OpenStateUnknown`; non-UUID -> `InvalidSessionId` and no process started | the dead-pid and absent cases |
| D3 | `d3_*` | the call-5 and call-3 streams | suggestions `Edit(other.txt)`, `Bash(echo hello > made_by_bash.txt)`, `Edit(note.txt)`; adding one grows the argv by exactly one rule and keeps `dontAsk` | T4 outside path |
| D4 | `d4_*` | the call-3 stream (init says `default`) on a hanging fake | `Aborted` right after Init, process killed, `Exited` not code 0 | D1 (init says `dontAsk`, not aborted) |
| D5 | `d5_cancel_leaves_no_child_process` | hanging fake that starts a sleeping grandchild | both pids alive before cancel and 300 ms later; after `cancel()` both gone within 5 s (reason: smoke 0.75-0.94 s including one 0.3-0.5 s pid query; 5x headroom for the loaded gate) | D6 |
| D6 | `d6_*` | the same fake killed with `Child::kill` only | the grandchild is still alive 1 s later (so D5 discriminates); it is then killed by its pid | - |
| D7 | `d7_*` | a home with 3 transcripts (one of `my_proj`, which shares the directory name of `my-proj`) and a listing (our pid, a dead pid, an open session without a transcript, a non-UUID row) | sessions exactly [S2 open, S1 closed, S3 open without transcript]; listing failure -> both transcripts UNKNOWN plus a warning; transcript directory missing -> the two open sessions plus one warning | the colliding transcript and the non-UUID row are excluded |
| R1-R3 | `real_cli::r1_real_list`, `r2_real_new_session`, `r3_real_resume_with_denial` (`#[ignore]`) | the installed `claude` in `RCFD_CLAUDE_REAL_DIR`, `--model haiku` | R1: no warnings, every scratch session CLOSED (and an open session seen OPEN in its own folder when named by env); R2: init id = chosen id, mode dontAsk, text deltas, result contains `pong`, exit 0; R3: a closed session resumed, the Write denied with suggestion `Edit(r3.txt)`, the file not created, exit 0 | R2 is a positive control for the R3 denial |

R1-R3 are run once each by the author after the implementation commit (two more billable calls, seven in total) and their output is
recorded in the addendum.

## Smoke results before registration (rule d)

* Test set-up errors found and fixed before registration:
  * an expected prefix typo ("did yo" for "did you");
  * the transcript message count is 24, not the 20 I had guessed;
  * call 5 was recorded without `--include-partial-messages`, so its text is not in deltas, and the delta check excludes it;
  * T3's space size gave 7469 lists, and its floor is set below that;
  * the wrong mapper in T1 gives 1 mismatch, not 2 (`./cases/**` already carries the anchor);
  * rule validation did not refuse bypass tokens that `check_args` refuses (T3 found `PowerShell(bypassPermissions)`), so validation now
    refuses them too.
* Liveness: the full `tasklist` took 2.5 s (610 processes), which made D5 measure the query instead of the cancel. The query was
  replaced by a filtered per-pid query of 0.3 s. Then D5 measured 0.75 s, 0.94 s and 0.78 s, with `cancel()` itself taking 0.27-0.34 s.
* P4: 26.6-26.9 MB/s in debug and 260 MB/s in release (one release smoke run of the whole crate, which also gave D5 0.63 s). Every
  check passed in the debug and release smoke runs.

## What I already knew when registering (rules c, d; disclosed)

The code and every test were written before this ADR so that the smoke could run. The only results seen before registration are the
smoke results above and the five recorded calls. No registered limit was chosen after seeing a registered run.

## Not observed (the lead must test by hand)

* The `--permission-prompt-tool` contract and the `--permission-prompts host` path with an SDK host: not used, not observed.
* Whether a `-p` process with `--input-format stream-json` exits when stdin stays open: the driver always closes stdin, and multi-turn
  in one process was not tried.
* Whether a Bash rule with a redirection (`Bash(echo hello > x.txt)`, the D4 suggestion for call 3) actually allows that command. The CLI
  treats redirections separately ("Output redirection ... needs approval"). R3 tests the Write path only.
* Pattern semantics of `Edit(./path)` beyond the exact file, and of `Read(...)`: not observed.
* How transcript directory names are formed for very long paths: one directory under `~/.claude/projects` looked truncated with a
  hash; not verified.
* Background (`--bg`) sessions in `claude agents --json`, and their `id` / `state` fields: not observed.
* The gap between the open check and the spawn: a session opened by another process in that gap is not detected.
* The non-Windows paths (`ps`, process group kill) were not run on this machine.
* By hand: open a session in a terminal in a project folder and check that the Studio mirrors it and refuses to type. Close the terminal,
  type a message and check the reply. Press "allow this" on a denied Write and repeat. Cancel a long turn and check that no `claude.exe`
  or tool shell is left in Task Manager.

## Out of scope

UI, the Agent SDK sidecar, cross-session messaging, Remote Control, `claude attach`, the IDE protocol, the plugin (wave 48).

## Files

`Cargo.toml` (workspace member), `crates/rcfd-claude/` (`src/{lib,allow,discover,driver,stream,transcript}.rs`,
`src/bin/fake-claude.rs`, `tests/{fixtures_v2_1_310,driver_fake,real_cli}.rs`, `tests/fixtures/v2.1.310/*`), this ADR.

## Addendum: measured values (registered runs, 2026-10-26)

The registered run was one release run of the crate (`cargo test --release -p rcfd-claude -- --nocapture`, `target-w47e`) at a45ffa7.
After the rebase onto main a16cda2 that commit is c2fa881.
The real-CLI tests R1-R3 were each run once in debug. Every registered check passed; there are no known misses.

| Check | Limit | Measured |
|---|---|---|
| T1 | 0 mismatches; wrong mapper exactly 1 | 0; 1 |
| T2 | 22 / 22 rejected | 22 / 22 |
| T3 | no forbidden argument in the generated space; >= 5000 lists, >= 10 000 insertions; 15 / 15 bad lists refused | none; 7469 lists, 13 294 insertions; 15 / 15 |
| T4 | exact suggestions; `None` cases | as registered |
| P1 | 0 unrecognised lines; fields and sequences as recorded | 0 in 5 streams and 2 transcripts; all equal |
| P2 | no panic; 144 / 144 transcript and >= 60 stream targeted mutations unrecognised | no panic in 4800 + 200-per-line random mutations; 144 / 144; 126 / 126 stream (42 recognised lines x 3; the test asserts >= 60, and the number was counted from the fixtures by a script that makes the same selection) |
| P3 | tail and incremental reads exact | exact |
| P4 | >= 10 MB/s (release) | 259.4 MB/s (30.5 MB, 34 000 events, 0.118 s) |
| D1-D4, D6, D7 | as registered | passed |
| D5 | both pids gone within 5 s after `cancel()` | `cancel()` returned after 0.196 s; both pids gone at the first check, 0.528 s after cancel started |
| R1 | no warnings, scratch sessions CLOSED, a live session OPEN | 2 scratch sessions CLOSED; the lead's live session in `<project dir>` classified OPEN |
| R2 | id chosen here, dontAsk, `pong`, exit 0 | session 75ebe38e-…, `permissionMode` dontAsk, `pong`, exit 0, 0.0026 USD |
| R3 | closed session resumed, Write denied with `Edit(r3.txt)`, no file, exit 0 | resumed 75ebe38e-… (the R2 session, the newest closed one), denied with suggestion `Edit(r3.txt)`, no file, exit 0, 0.0058 USD |

New observations from R2 and R3, the first runs with the driver's exact arguments:
* `--permission-prompts none` together with `dontAsk` was accepted, and init reported `dontAsk`.
* The prompt on stdin with `--resume` worked.
* `--allowedTools <rule>` followed by `--resume <id>` parsed as intended, so the variadic list ended at the next option.
* The resumed session kept its id.

Real CLI usage in total: **7** billable headless calls (5 observation calls and R2, R3), all with `--model haiku`, about 0.033 USD.
`claude agents --json` ran about 12 times and makes no model call.

Deviations:
* Clippy ran after the registered run, not before it. It found two style lints in `discover.rs` (`io::Error::other` and
  `sort_by_key`) and the intended never-waited grandchild in `fake-claude`, which now has an explicit `allow` saying why. None of these
  changes behaviour. The final gate re-ran every test on the fixed code.
* No other deviation from the registered plan.

## Final gate

The final gate ran on a fresh `git archive` of 21557f6 (the branch rebased onto main a16cda2) in `<scratch dir>`:
* `cargo fmt --check`: exit 0.
* `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
* `cargo test --workspace --release --no-fail-fast`: **2093 passed, 0 failed, 659 ignored**.

`rcfd-claude` contributes 20 passed (lib 7, `driver_fake` 7, `fixtures_v2_1_310` 6) and 3 ignored (`real_cli`). After the gate, only
this ADR changed (the addendum corrections and this section).
