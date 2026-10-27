# ADR 0128: Sixteen audit fixes (runner, Claude bridge, project folder, time-series store, renderer, API)

Status: **ACCEPTED 2026-10-27** (acceptance registered before any code). Written at main 511a24c on branch `wfix`, worktree
`<scratch dir>`, target `<target dir>`. The findings come from an audit of the wave-47 crates and of
`rcfd-api`; the file:line references below were re-read on 511a24c.

## Rules for every finding

* A finding is fixed only when its new test **fails on the pre-fix code and passes after the fix**. Each finding is one commit
  (test plus fix). A finding whose concrete case cannot be reproduced is reported as *not reproduced* and not changed.
* **Fails-before method.** Where the test reaches the code through existing interfaces (findings 1, 2, 5, 7, 10, 11, 13, 16 and the
  scan leg of 3), the same test runs unchanged on the pre-fix commit. Where the test needs a seam that does not exist yet (findings 3,
  4, 6, 8, 9, 14, 15), the seam is first applied alone on top of the pre-fix source (behaviour unchanged), the test is shown to fail
  there, then the fix is applied and the test passes. The before/after output is recorded in the addendum.
* New tests go into new test files or new `#[cfg(test)]` functions; the registered checks S1-S11 (ADR 0123), D1-D7 and P1-P4
  (ADR 0126), C1-C12 (ADR 0124), W1, L1 (ADR 0122), U1-U8 (ADR 0121) and the render checks (ADR 0125) keep their limits. Pre-declared
  edits of existing tests are listed under the finding (U6 one line, L1 negative control).
* New runner process tests start `rcfd-runner serve` directly (as S8 does), never through `ensure_runner` (PowerShell), except the
  one start check of finding 15.
* No default behaviour of a working configuration changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not touched.

## Findings and registered acceptance

### 1. Runner config may name the solver binary (`rcfd-runner/src/runner.rs` 44-45, 93-99)

`.rcfd/runner-config.json` can set `rcfd_bin`, so a project folder can make the runner execute any program.
**Fix:** remove the field. The solver is `RCFD_BIN`, else `rcfd` next to the runner. `Config` keeps `deny_unknown_fields`, so a config
that still names `rcfd_bin` is **rejected** at `serve` start with an error that names the config file and the key.
**Accept:** test `f01_config_naming_rcfd_bin_is_rejected_and_never_run`: a project whose config names a script (`.cmd` on Windows,
`sh` script elsewhere) that writes a marker file, one queued run without `mem_mb` (so the runner would start a pre-launch check with
the solver at once); `rcfd-runner serve` started directly exits non-zero within 10 s, stderr names `runner-config.json` and `rcfd_bin`,
and the marker file does not exist 2 s after the exit. S1-S11 pass unchanged.

### 2. Allow-list accepts control files (`rcfd-claude/src/allow.rs` 75-88, 256-265)

**Fix:** `check_path` refuses the control files `.mcp.json`, `CLAUDE.md`, `.claude/**`, `.rcfd/**` and `rcfd-project.json`.
`suggest()` ends in `validate()`, so it inherits the refusal. Before comparing, each `/`-segment is normalised: empty and `.`
segments dropped, ASCII lowercase (on every platform; refusing `Claude.md` on Linux costs nothing), trailing `.` and spaces stripped
(Windows ignores them). Decisions registered here:
* a segment equal to a control name is refused at **any** depth (a nested `CLAUDE.md` is also loaded by the CLI; `cases/.claude/x`
  is refused although only the root one is read: conservative);
* a wildcard (`*`, `?`, `[`) in the **first** segment is refused when the segment could match a control name (`*.md`, `.c*`,
  `*.json`, `rcfd-*`); the test is a small glob match of that segment against the five names;
* known limit: a wildcard in a deeper segment (`cases/**`) still matches a nested `cases/CLAUDE.md`. Refusing `cases/**` would
  remove the main use of the list; recorded, not fixed;
* `~` anywhere in a path is refused (8.3 short names such as `CLAUDE~1.MD` reach a control file).
**Accept:** test `t5_control_files_are_refused`: for each of the five names, the forms `X`, `./X`, `X` upper/lower/mixed case,
`X.` (trailing dot), `cases/../X`, `a\X` (backslash), and for the two directories also `X/settings.json`, `X/**`,
`./X/sub/f`, plus `*.md`, `.c*`, `**/CLAUDE.md`, `CLAUDE~1.MD` are refused by `AllowRule::Edit` and `AllowRule::Read`, and
`suggest()` returns `None` for a `Write` and a `Read` of the absolute path of each file inside the project (also with Windows
case differences). Ordinary paths `cases/x.json`, `notes/a.md`, `./cases/**`, `runs/*/metrics.json` stay allowed. T1-T4 pass
unchanged (T3 still finds no forbidden flag and keeps its space-size floor).

### 3. A failed status write can launch a solver twice (`rcfd-runner/src/runner.rs` 546-561)

After spawn, a failed write of the `running` status is only logged; the run stays `queued` on disk, and a later runner launches a
second solver into the same folder (which deletes the first one's `output/`).
**Fix:** right after spawn the `launched` event is appended (it now comes before the `running` state event), then the status is
written. If either write fails, the child is killed and the run is failed with a reason naming the file and the error. In `scan`, a
run whose status is `queued` (or `paused` without pid) but whose `events.jsonl` has a `launched` event is **adopted** (pid from the
event, probed like any adopted run) and never put back in the queue.
**Seam:** `Runner` becomes generic over `RunStore` (the trait the crate already has for this boundary) so a unit test can inject a
store whose `write_status(running)` fails.
**Accept:**
* unit test `f03_failed_running_write_kills_the_solver_and_fails_the_run`: real `rcfd` solver (next to the test binary's
  `target/<profile>`), store failing the running write; after the first runner's ticks: exactly one spawn (`launch_seq` = 1, one
  `launched` event), the spawned pid not alive within 5 s, status `failed` with a reason containing `status.json`; a second runner
  on the same folder (plain store) launches nothing (`launch_seq` = 0).
* integration test `f03_queued_run_with_launched_event_is_not_relaunched`: hand-made run with status `queued` and a `launched` event
  (a) with a dead pid: the runner marks it `failed` (probe) and launches nothing; (b) with the pid of a live `rcfd` started by the
  test: the runner adopts it (status `running`, `adopted` event) and launches nothing.
* Not measured: deletion of `output/` by a second solver; "no second launch" is the registered proxy for it.

### 4. Liveness probe by fixed name, one miss finalises (`runner.rs` 655-657, `sys.rs` 13-28)

**Fix:** the probe uses the file stem of the solver the runner was started with; an adopted run is finalised only after **two
consecutive** missed probes (a hit resets the count). Counting is a small pure function.
**Seam:** the probe function is a field of `Runner` (default `sys::pid_alive`).
**Accept:** unit test `f04_one_missed_probe_does_not_finalise_two_do` (scripted probe: miss, hit, miss stays running; miss, miss
finalises); integration test `f04_renamed_solver_survives_a_runner_restart`: `rcfd` copied to `mysolver(.exe)`, `RCFD_BIN` set on a
directly started runner, runner killed while the solver runs, a second runner started: the run is still `running` 3 s
(>= 4 probe periods of 500 ms) after the restart and ends `finished`. S4b (dead pid marked failed within adopt_probe_ms + 2 s = 2.5 s)
passes unchanged.

### 5. Permission-mode check only on a parsed Init (`rcfd-claude/src/driver.rs` 250-257)

**Fix:** until a checked `Init` (session id present, `permissionMode` = `dontAsk`) has been seen, any event other than an unknown
`system/*` line aborts the session: an `Init` with another mode, an `Unrecognised system/init` (init without session id), a tool
call, text, denial, result or an unparseable line. After an abort the driver stops forwarding stream lines.
Deviation from the audit's wording ("the first event"), registered with its reason: unknown `system/*` lines (for example hook
output; UNVERIFIED that the CLI prints them before `init`) are tolerated before `init` because they carry no tool action; refusing
them could abort every session of a user with hooks. Tool calls, denials and results before a checked `Init` still abort.
**Accept:** test `d8_init_without_session_id_is_aborted` (fake CLI, stream built in the test from the recorded allow-list stream
with `session_id` removed from the init line): an `Aborted` event whose text names `system/init` and `session_id`, no `ToolCall`
forwarded, `Exited` last. Test `d9_tool_call_before_init_is_aborted` (tool line moved before init): aborted, the tool call not
forwarded. D1-D7 and P1-P4 (v2.1.310 fixtures) pass unchanged.

### 6. Adopted run reads the whole solver stream at once (`runner.rs` 588-597)

**Fix:** `drain` reads at most 4 MiB per call. On the exit/finalise path the runner drains until nothing more is read, so a solver
that wrote more than 4 MiB and exited still has every line and its finish event forwarded. A single line longer than 4 MiB (no
newline in a full chunk) is skipped with a `skipped_line` event naming the byte offset (it cannot be a valid status line).
**Seam:** `drain` returns the bytes it consumed (for the test).
**Accept:** unit test `f06_large_status_file_is_drained_in_bounded_chunks`: a 100 MB (>= 100 * 10^6 bytes) `solver-status.jsonl`
of iteration lines of about 1 KB with increasing index, adopted run with a scripted always-alive probe: consumed over >= 24 ticks,
no tick reads more than 4 MiB, the final residual count equals the file's line count. Peak extra memory, measured as the peak
working set of the test process (OS counter read by a Python harness, `K32GetProcessMemoryInfo`) minus that of a trivial test of the
same binary: **< 16 MiB** after the fix (the pre-fix value is reported).

### 7. Bad frame head in the tail makes `Reader::open` fail (`rcfd-tsstore/src/lib.rs` 228-230, 363)

**Fix:** `Reader::open` and `Reader::refresh` stop scanning at a frame head whose checksum does not match and report the bytes from
there on as `tail_bytes` (the offset goes into `reason`; `IndexSource` keeps its fields). `Writer::open_append` stays strict.
**Accept:** test `c13_zero_and_garbage_tails_after_complete_snapshots`: for N = 0, 1, 3 complete snapshots and both an unfinished
store and a finished one cut after snapshot N, a tail of 4096 zero bytes and one of 4096 pseudo-random bytes: `Reader::open` gives
exactly N snapshots, `tail_bytes` = 4096 plus the cut-off bytes (unfinished case: 4096), `verify()` succeeds, every field reads back
bit-exactly; `refresh()` on a reader opened before the tail was appended adds the snapshots before the tail and reports the tail;
`Writer::open_append` on the same file returns an error. C4, C5, C5b, C6 (every flipped byte of a finished store detected), C6b pass
unchanged.

### 8. Upload larger than the device's buffer limit panics (`rcfd-render/src/gpu.rs` 289-316)

**Fix:** the vertex byte count is computed from the triangle count **before** the buffer is built and checked against
`device.limits().max_buffer_size`; over the limit it returns `Error::Mesh` naming both numbers and the adapter.
**Seam:** a pure helper `vertex_bytes_check(triangles, limit)` (for the test without a GPU).
**Accept:** unit test of the helper (just under, at, just over the limit; overflowing counts); GPU test
`u1_upload_over_the_buffer_limit_is_an_error` on every adapter: a strip mesh with `limit / 156 + 1` triangles (one byte over at 156
bytes per triangle) returns `Err(Error::Mesh)`, no panic; with no adapter it prints the located SKIP line and passes. The registered
render checks of ADR 0125 pass unchanged.

### 9. Watcher hashes every small file on every poll (`rcfd-project/src/watch.rs` 110-121)

**Fix:** a file whose length and modification time equal the previous scan's, and whose modification time was more than 2 s older
than the **start of the previous scan**, reuses the previous hash (a write after that scan would have changed the time on any file
system with a timestamp granularity of 2 s or finer). The scan time is kept outside `Sig` equality.
Known limit (unchanged in kind): a same-length rewrite that restores an old modification time on purpose is not seen.
**Seam:** `Watcher::bytes_hashed()` (cumulative bytes read for hashing).
**Accept:** test `w2_unchanged_project_poll_reads_ten_times_fewer_bytes`: a project with 300 runs (`create_run`), every file's
time set 60 s back, `Watcher::new` then `poll` with nothing changed: bytes hashed by the poll <= 1/10 of the initial scan's
(pre-fix: equal); a file written during the test, polled, then rewritten with the same length and its time restored is reported;
W1 and its negative control pass unchanged.

### 10. `Project::open` keeps the `\\?\` prefix on Windows (`rcfd-project/src/project.rs` 149)

**Fix:** a helper canonicalizes and turns `\\?\X:\...` into `X:\...` when that is safe (drive-letter form, shorter than 260
characters, no component ending in `.` or space, no reserved device name), and `\\?\UNC\server\share\...` into
`\\server\share\...` under the same conditions; otherwise the verbatim form is kept. MSRV 1.75 (no `std::path::absolute`).
A dev-dependency of `rcfd-claude` on `rcfd-project` would break `layering_matches_architecture_doc` (rcfd-claude is not in its
layer table), so the two sides are tested separately.
**Accept:** Windows tests in rcfd-project: `Project::open(root).root()` has no `\\?\` prefix and names the same directory
(`same_file` by canonicalizing both); unit test of the string conversion (drive, UNC, long, trailing dot, reserved name kept
verbatim). rcfd-claude test `project_dir_name_and_same_path_of_a_plain_root`: the plain root string gives the expected dir name and
`same_path` is true against the same path with a trailing separator and other case, and false against the `\\?\` form (which is
why rcfd-project must not return it).

### 11. `wait --timeout -1` panics (`rcfd-runner/src/main.rs` 101)

**Fix:** `Duration::try_from_secs_f64` and an error naming `--timeout` and the value.
**Accept:** test `f11_bad_wait_timeouts_are_clean_errors`: `rcfd-runner wait r-x --timeout V` for V in `-1`, `NaN`, `inf`,
`1e300` exits with code 1 (not 101) and stderr contains `--timeout` and V, and no `panicked`.

### 12. L1 negative control cannot fail (`rcfd-project/tests/listing_budget.rs` 70-82)

500 sleeps of 2 ms always exceed 500 ms, whatever the listing costs. **Decision rule, registered before measuring:** run L1 once in
release and read the worst listing time `w`. If `10 w > 500 ms`, the budget itself catches a 10x slower listing: the sleep control is
replaced by a control that runs the listing path 10 times per measured listing and asserts that this exceeds the budget, and a
throwaway run with the listing made 10x slower must fail L1. If `10 w <= 500 ms`, the budget cannot catch a 10x slowdown; the dead
control is deleted and the addendum records `w`, the factor `500 ms / w` that the budget does catch, and that the remaining guard
is the budget assertion itself. The budget (500 ms) is not changed either way.

### 13. `create_run` stores the case path as given (`rcfd-project/src/project.rs` 253)

**Fix:** `run.json` records the project-relative normalised path (`paths::rel_string` of the resolved file). An absolute path inside
the project is accepted (canonicalized with the helper of finding 10, then made relative; outside is refused as before).
`latest_finished` normalises both the query and the recorded value (`\` to `/`, empty and `.` segments dropped), so older
`run.json` files with `./cases/a.json` still match.
**Accept:** test `r9_case_path_is_recorded_normalised`: runs created with `./cases/a.json`, `cases/a.json` and the absolute path
of the file are all found by `latest_finished("cases/a.json")` (and by `./cases/a.json`), and record `cases/a.json`; a hand-written
old `run.json` with `./cases/a.json` is matched; an absolute path outside the project is refused.

### 14. `ledger_range` is positional (`rcfd-api/src/design/uq.rs` 495-498, 664-666)

With another writer appending to the ledger during a study, `(first, first + n_samples)` names the wrong entries.
**Fix:** drop `UqResult::ledger_range` and the `ledger_range` key of `result_json` (the summary provenance). Samples are identified
by `study` plus the variant `name#k`, the summary by `name#summary`. Nothing in the repository reads the key back; old ledgers that
carry it stay readable (provenance is free JSON). Registered limit: a second study under the same name in the same ledger repeats
the variant names; telling them apart needs the seed or time.
**Pre-declared edit:** U6 asserts `r.ledger_range == Some((0, 2000))`; that line is replaced by the by-name check (entries 0..2000
have variants `name#0..name#1999`). U1-U8 limits unchanged.
**Accept:** test `u9_second_writer_during_a_study`: a model whose evaluation appends a foreign entry to the same ledger file through
a second `Ledger`; by name the study has exactly `n_samples` sample entries `name#0..` plus one summary, the foreign entries are not
among them, and no positional range recorded anywhere in the ledger (summary provenance `ledger_range`) points at a foreign entry
(pre-fix: it does). U1-U8 pass.

### 15. Trailing backslash breaks the PowerShell start (`rcfd-runner/src/sys.rs` 69-75)

**Amendment to the audit's fix, registered with its reason:** trimming trailing separators turns `C:\` into `C:` (the current
directory of drive C). The fix is argv quoting by the Windows rules (`CommandLineToArgvW`/MSVCRT): backslashes before a quote and
at the end of a quoted argument are doubled, embedded quotes become `\"`. The whole argument string goes to `Start-Process
-ArgumentList` as one string (PowerShell passes a single string through as the command line), single quotes doubled for PowerShell.
**Seam:** `sys::command_line(args)` builds the string.
**Accept:** Windows test `f15_command_line_round_trips_through_the_real_argv_parser`: for `C:\tmp\a b\`, `C:\`, `C:\x\\`,
`a"b`, `a\"b`, `it's`, ``, `plain`, the command line `start <arg> --no-runner --project <tmp>` built by `command_line` and passed
with `raw_arg` to the real `rcfd-runner` makes it report the case path `<arg>` exactly (its error message prints the path it parsed)
and treat the project as `<tmp>`; unit test of the PowerShell script text (single quotes doubled). Start check
`f15_runner_starts_with_a_trailing_backslash_project_path`: `ensure_runner` on a project path ending in `\` takes the lock. The
PowerShell route itself is not observable here (PowerShell hangs on this host and `ensure_runner` falls back to a direct start);
recorded as a limit.

### 16. Duplicated code

(a) The sandbox containment check is copied in `rcfd-api/src/chemistry.rs` 149, `exprs.rs` 78 and `udf.rs` 421, and each copy treats a
failed canonicalize of the **root** as inside. **Fix:** one helper `case::sandbox_contains(root, base, file) -> Result<bool, String>`
used by all three: a root that cannot be canonicalized is an error (each caller reports it at its own key: "the sandbox root ...
cannot be resolved: ..."); a missing file still counts as inside, so the later "cannot read" diagnostic is unchanged; each caller's
"outside the sandbox" message is unchanged. Behaviour change on an error path only: a configuration whose sandbox root exists is not
affected.
**Accept:** test `sandbox_helper_matches_the_old_check_on_resolvable_roots`: >= 30 paths (inside, nested, outside, `..`, absolute
inside/outside, missing file inside/outside, base dir inside/outside the root, a symlink pointing out where the OS lets the test create
one, else recorded as skipped) give the same verdict as the old expression copied into the test; test
`unresolvable_sandbox_root_is_refused_by_all_three`: a case with a missing sandbox root refuses the chemistry mechanism file, the
lookup-table file and the UDF module file with a diagnostic naming the root (pre-fix: none of the three refuses).
`rcfd-api/src/service.rs` `sandbox_path` has a related pattern (falls back to the raw root); not part of this finding, untouched.

(b) Listed, not touched (F-261, separate task): `rcfd-runner/src/records.rs` re-implements the FNV-1a hash (`fnv1a64`, also
`rcfd_core::hash::fnv64`), the civil date and ISO time (`civil`, `now_iso`; `rcfd-project/src/time.rs`), `atomic_write`
(`rcfd-project/src/atomic.rs`) and run ids (`new_run_id`; `rcfd-project/src/runid.rs`).

## Gate

Fresh `git archive` of the branch head into `<scratch dir>\src`, `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, tests with `scripts/gate_test.py` (docs/test-process.md section 4). Then rebase onto the current main
and re-run fmt, clippy and the touched crates' tests.

## Amendment 1 (finding 5, written after the run that motivated it)

What I knew: with the registered "stop forwarding after an abort", D3 failed. Its second half replays the recorded
default-mode stream (`stream_denied_default.jsonl`) and expects the two denials with their suggestions; that stream aborts at its
`init` (mode `default`), and D3 relied on the lines after the abort still being forwarded. D1-D7 must stay unchanged, so the
stop-forwarding part is withdrawn: after an abort, lines already buffered are still forwarded (as before this ADR), after the
`Aborted` event. Unchanged: the offending event itself is not forwarded (except an `Init`, which shows the mode the CLI applied, D4).
D8 and D9 therefore check "no `ToolCall` before the `Aborted` event" (and D9: the offending call is never forwarded, at most the two
later calls follow the abort) instead of "no `ToolCall` forwarded". Both still fail on the pre-fix driver (no `Aborted` event,
three tool calls forwarded).

## Amendment 2: finding 17 (added on the lead's request, registered before code)

### 17. Console windows flash on Windows

Console programs started by the console-less runner, by the Studio client or by the bridge open a visible window. Spawns found by
grep over `crates/*/src` (non-test code): rcfd-runner `sys.rs` `pid_alive` (`tasklist`), `kill` (`taskkill`), `spawn_detached`
(`powershell`), `spawn_direct` (the runner itself), `client.rs` `submit` (`rcfd --version`), `runner.rs` solver and pre-launch check
(already `no_window` since ADR 0123); rcfd-claude `discover.rs` `live_pids` (`tasklist`), `driver.rs` `kill_tree` (`taskkill`),
`agents` (`claude agents --json`) and the chat spawn (`claude -p`). rcfd-cli, rcfd-api, rcfd-mcp and rcfd-gui spawn no console
program outside tests (the `cmd` spawns in `case.rs` and `discover.rs` are inside `#[cfg(test)]`; `fake-claude` is a test double).
**Fix:** one helper per crate, no new dependency and no new crate edge: rcfd-runner `sys::console(program, extra) -> (Command, u32)`
and rcfd-claude's own equivalent create the command with creation flags `extra | CREATE_NO_WINDOW (0x0800_0000)` on Windows and
return the flags set (std's `Command` has no getter). Every spawn above is built through it, by a small builder function per site that
returns the command and its flags. Flags are combined, never replaced: `spawn_direct` keeps `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`;
the PowerShell launcher gets the same pair (it had none; registered decision: it detaches the launcher from the caller's console and
Ctrl+C like the fallback, and `CREATE_NO_WINDOW` is ignored by Windows together with `DETACHED_PROCESS`, so no window either way);
the 5 s fallback is unchanged. The claude CLI spawns get only the flag: stdio and pipes are unchanged.
**Accept:**
* (a) a Windows unit test per crate (`no_window_flags_on_every_console_spawn`) builds each site's command through its builder and
  asserts `CREATE_NO_WINDOW` in the flags, and `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` in the PowerShell launcher's and
  `spawn_direct`'s flags;
* (b) negative control in the same test: a command built without the helper (flags 0) fails the same check;
* (c) `tests/no_window_scan.rs` (rcfd-runner): scans every `.rs` file under `crates/*/src`, text before the first `#[cfg(test)]`,
  for `Command::new("tasklist" | "taskkill" | "cmd" | "powershell" | "typeperf" | "where")` (with or without `.exe`); none may remain;
  negative control: the scanner finds such a line in a synthetic text and ignores it after `#[cfg(test)]`;
* (d) S1-S11, D1-D9, P1-P4 unchanged and passing (gate);
* (e) whether a child started with the flag has no console window: checked once outside the test suite (Python, standard library:
  `GetConsoleWindow()` in a child started with and without `0x0800_0000`); reported in the addendum, or reported as not verified.
Fails-before: (a) and (c) fail on the pre-fix code (the builders are introduced as a seam returning the old flags first).

## Addendum: measured values (2026-10-27)

Every finding was reproduced: each new test failed on the pre-fix code (the same test, or the test plus its seam alone) and passes
after the fix. Debug runs unless noted; one commit per finding on branch `wfix`.

| # | Test(s) | Before the fix | After the fix |
|---|---|---|---|
| 1 | `runner_wfix::f01_config_naming_rcfd_bin_is_rejected_and_never_run` | the named `.cmd` ran; runner still serving after 10 s | `serve` exits 1: `...\.rcfd\runner-config.json: unknown field 'rcfd_bin' ... at line 1 column 76`; script never ran |
| 2 | `allow::tests::t5_control_files_are_refused` (113 forms) | plain, `./`, upper/lower case, trailing-dot and nested forms of all five names accepted | all 113 refused; ordinary paths allowed; T1-T4 pass |
| 3 | unit `f03_failed_running_write_kills_the_solver_and_fails_the_run`; `runner_wfix::f03_queued_run_with_launched_event_is_not_relaunched` | 2 spawns, first solver alive, run left `queued`; a queued run with a dead-pid `launched` event was launched again (still running after 10.2 s) | 1 spawn, solver killed, `failed`: "cannot record the launched solver N in ...status.json: injected write failure; ..."; dead pid: `failed` 0.25 s after start, no relaunch; live pid: adopted, `running` with that pid |
| 4 | unit `f04_one_missed_probe_does_not_finalise_two_do`; `runner_wfix::f04_renamed_solver_survives_a_runner_restart` | every probe script finalised; renamed solver (alive) marked failed at step 155 | [miss], [miss, hit, miss] stay running, [miss, miss] and [hit, miss, miss] fail; renamed solver running 3 s after the restart (step 2131), finished at 10000 steps |
| 5 | `driver_init_wfix::d8_init_without_session_id_is_aborted`, `d9_tool_call_before_init_is_aborted` | no `Aborted`, 3 tool calls forwarded | D8 aborted "the CLI's system/init line has no session_id, so its permission mode cannot be checked", D9 aborted "a tool call (Write) arrived before a checked system/init ...", 0 tool calls before the abort |
| 6 | unit `f06_large_status_file_is_drained_in_bounded_chunks` (100,000,284 bytes, 99,624 lines) | 1 tick read the whole file; peak working set 100.6 MB (trivial test of the same binary: 5.2 MB) | 24 ticks, largest 4,194,162 bytes, 99,624 residual lines; peak working set 14.2 MB, **+9.0 MB** (limit 16 MiB) |
| 7 | `tail_wfix::c13_zero_and_garbage_tails_after_complete_snapshots` | `Reader::open` and `refresh` failed ("frame head checksum mismatch at offset 4239/4920/6282/6392") | 0/1/3 snapshots bit-exact, `tail_bytes` 4096 (finished store plus tail: 3 snapshots, 4226); `refresh` adds 2; `open_append` still refuses |
| 8 | `upload_limit_wfix::u1_upload_over_the_buffer_limit_is_an_error`, `vertex_bytes_check_bounds` | wgpu panic "Buffer size 268435752 is greater than the maximum buffer size (268435456)" | `Error::Mesh` naming both sizes on all 5 adapters (RTX 5080 DX12/Vulkan, Radeon DX12/Vulkan, WARP); no panic |
| 9 | `watch_wfix::w2_unchanged_project_poll_reads_ten_times_fewer_bytes` (300 runs, 1202 files) | unchanged poll hashed 121,817 of 121,817 bytes | 0 bytes; new file, same-length rewrite with restored time inside the window, and an old file rewritten are all reported |
| 10 | `root_wfix::root_has_no_verbatim_prefix`, `verbatim_strings_are_stripped_only_when_safe`; rcfd-claude `project_dir_name_and_same_path_of_a_plain_root` | `root()` = `\\?\<scratch dir>\...\proj` | `<scratch dir>\...\proj`, same folder; 9 string cases and a 300-character path as registered |
| 11 | `runner_wfix::f11_bad_wait_timeouts_are_clean_errors` | `-1`, `NaN`, `inf`, `1e300`: exit 101, panic in `core::time` | exit 1, "--timeout needs a non-negative number of seconds, got '<v>' (...)" |
| 12 | L1 (release) | decision run: worst listing 196.4 ms, so `10 w` = 1.96 s > 500 ms; the old control (500 x 2 ms sleep) read 1.44 s whatever the listing cost | control replaced by the listing done 10 times: 1.67 s > 500 ms (worst 159.7 ms); throwaway with the listing made 10x slower: worst 1.45 s, **L1 fails** on the 500 ms budget |
| 13 | `case_path_wfix::r9_case_path_is_recorded_normalised` | `./cases/a.json`, `cases/./a.json` recorded as given and not found; the absolute path inside refused; old records not matched | all record `cases/a.json` and are found (also by `./cases/a.json`); old records `./cases/a.json`, `cases\a.json`, `cases//a.json` matched; outside still refused |
| 14 | `uq::tests::u9_second_writer_during_a_study` | summary at index 2000 claimed `ledger_range` (0, 1000) with 1000 foreign entries interleaved | no positional claim; samples `u#0..u#999` and `u#summary` found by name; U1-U8 pass |
| 15 | `runner_wfix::f15_command_line_round_trips_through_the_real_argv_parser` (9 arguments), `f15_runner_starts_with_a_trailing_backslash_project_path` | 6 of 9 parsed wrong (e.g. `C:\` became `C:" --no-runner --project ...`) | 9 of 9 exact; script text exact |
| 16a | `case::sandbox_tests::unresolvable_sandbox_root_is_refused_by_all_three`, `sandbox_helper_matches_the_old_check_on_resolvable_roots` | none of the three refused (the files were read) | all three: "the sandbox root ... cannot be resolved: ..." at `chemistry.mechanism`, `tables.<n>.file`, `udf.<n>.file`; 84 (base, file) cases (60 inside; junctions in/out and a file symlink created) identical to the old check |
| 17 | `sys::tests::no_window_flags_on_every_console_spawn` (runner), `tests::no_window_flags_on_every_console_spawn` (bridge), `no_window_scan::no_console_program_is_started_without_the_no_window_helper` | old flags (seam): tasklist, taskkill, powershell, `rcfd --version` 0x0, direct start 0x208, every bridge spawn 0x0; scan found 5 direct spawns (`sys.rs` 16, 44, 71; `discover.rs` 124; `driver.rs` 331) | runner 0x8000000, launcher and direct start 0x8000208; bridge 0x8000000; scan: none in 316 files; negative controls hold |

### Deviations and notes

* **Finding 5:** amendment 1 (forwarding after an abort kept for D3).
* **Finding 4:** the first version of the F4 unit test put every probe script into one project; the runner of a later script
  adopted the earlier scripts' runs too, which shared the scripted answers (2 of 3 reruns failed). Fixed in the finding-6 commit
  (one project per script); the pre-fix evidence (all four scripts finalised) is unaffected.
* **Finding 6:** the pre-fix peak (100.6 MB) was measured with the first test version, which counted the residual lines through one
  `read_to_string` (about 10 MB); the final test counts them streaming. With the fix but an unbounded chunk (`take(u64::MAX)`, no
  size hint) the peak was 134.2 MB. 24 ticks is exactly the registered floor (100,000,284 / 4 MiB = 23.8).
* **Finding 7:** C6 still has 0 undetected flips, but 56 flips per codec (the 28-byte footer frame head, two masks) are now
  flagged (index rebuilt by scan, data exact) instead of rejected: 260 flagged per codec, ADR 0124 recorded 204.
* **Finding 8:** the test mesh is one *triangle* over the limit (296 bytes), not one byte as the registration worded it.
* **Finding 10:** `tests/paths.rs` P1's link control now compares the canonicalized link target with the canonicalized root; with the
  plain root it would have passed vacuously on Windows. Not pre-declared; P1's refusals are unchanged.
* **Finding 13:** the mesh path of `create_run` is normalised the same way (same code path).
* **Finding 15:** the start check passes before and after the fix: on this host `powershell.exe` does not start the runner within
  5 s and `ensure_runner` falls back to the direct start. The PowerShell route itself is not observed; the round trip through the
  real argv parser is the evidence.
* **Finding 16a:** `service.rs::sandbox_path` (falls back to the raw root) is untouched.
* **Finding 17, limits of the checks:** the negative control inside the two flag tests is a constant (flags 0 of a command built
  without the helper); the discriminating evidence is the seam-only run, where every site read 0x0. The source scan (c) reads each
  file only up to its first `#[cfg(test)]`; `case::sandbox_contains` (finding 16a) sits after `case.rs`'s existing test module, so a
  direct spawn added below that point would not be seen (known limit, code not moved after the gate).
* **`scripts/limit_audit.py` (from main, ADR 0129) on the rebased tree:** three hits in this ADR's tests, none makes a test unable to
  fail: `driver_init_wfix.rs:75` (a `return` inside a `map` closure), `upload_limit_wfix.rs:44` (the registered no-adapter SKIP),
  `runner_wfix.rs:284` (a set-up timeout guard in F4).
* **Finding 17 (e):** outside the suite (Python, standard library): from a parent started with `DETACHED_PROCESS` (no console, like the
  runner), a child console program without flags got a console window (`GetConsoleWindow` = 31656568) and with `CREATE_NO_WINDOW`
  none (0). From this agent's own shell the check does not discriminate (no flag also read 0). This verifies the flag's effect, not
  `tasklist` itself; that our spawns carry the flag is what the unit tests check. The runner's solver and pre-launch check already used
  `no_window` and keep it (now through the same helper); `fake-claude` and test-only spawns are unchanged. The claude CLI spawns get
  only the flag (pipes unchanged); the real CLI was not run (R1-R3 are ignored tests).

### Gate

Fresh `git archive` in `<scratch dir>`, `scripts/gate_test.py` (docs/test-process.md section 4):

| Run | HEAD | fmt | clippy | tests (passed / failed / ignored) |
|---|---|---|---|---|
| 1 | 652e598 (findings 1-16) | 0 | **failed**: `clippy::double_ended_iterator_last` in `launched_pid` (finding 3) | 2213 / 0 / 666 |
| 2 | e646d2d (clippy fixed) | 0 | 0 | 2213 / 0 / 666 |
| 3 | b8cf6eb (finding 17 added) | 0 | 0 | **2216 / 0 / 666** (61 min) |

Run 3 is the final gate. ADR 0129's final gate on 5ec4423 recorded 2190 / 0 / 666; `511a24c..5ec4423` changes no file under
`crates/`, so that is the same crate code as this branch's base: 2216 - 2190 = 26, the tests this ADR adds (none ignored).
After the gate the branch was rebased onto main 54e968e (ADR 0129: scripts and docs only, no crate code; no conflict). On the
rebased head: `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, debug tests of the touched
crates: rcfd-runner, rcfd-claude, rcfd-project, rcfd-render 101 passed / 0 failed / 7 ignored; rcfd-tsstore (lib, store, tail_wfix)
16 / 0 / 0; rcfd-api lib `design::` and `case::` 56 / 0 / 2, `tables` and `udf` 13 / 0 / 0. The full release gate was not repeated
on the rebased head.
