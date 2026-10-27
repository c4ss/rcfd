# ADR 0128: Sixteen audit fixes (runner, Claude bridge, project folder, time-series store, renderer, API)

Status: **ACCEPTED 2026-10-27** (acceptance registered before any code). Written at main cb61148 on branch `wfix`, worktree
`<scratch dir>`, target `<target dir>`. The findings come from an audit of the wave-47 crates and of
`rcfd-api`; the file:line references below were re-read on cb61148.

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
