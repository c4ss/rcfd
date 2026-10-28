# ADR 0146: Job 48L, the started runner's output handled explicitly, early exits reported at once, and what still holds a capturing caller on Windows

Status: **registered** 2026-10-28, before any run of the checks below (set-ups, limits and predictions are fixed here; nothing
of this job has been built or run); results in the addendum; not yet accepted by the maintainer. House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed before the registered runs; a missed limit
becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before the
corrected run; defaults of existing code stay bit-identical outside the runner start; `crates/rcfd-api/tests/baseline_hashes.rs`
is not re-recorded. The workspace forbids unsafe code and none is added; no new crate. Parents:
[ADR 0143](0143-job48i-native-runner-start-unix-zombies-linux-known-misses.md) (questions 1 and 6),
[ADR 0123](0123-wave47b-run-service.md) (decision 2, S5, amendments 2 and 3), [ADR 0141](0141-wave48g4-one-click-run-and-follow.md)
(the Studio's budgets check and log tail).

The maintainer's decisions of 2026-10-28 that this job implements:

1. The runner start path handles the started runner's standard output, error and input explicitly, so that they do not tie
   to a caller's captured pipes: `--log-file <path>` (append the runner's output to a file; default
   `<project>/.rcfd/runner.log`), `--stdout`/`--foreground` (keep the output on the caller's standard output; explicit opt-in,
   documented that the caller then waits), `--quiet` (null). Default for a detached start: output to the log file or the null
   device, input from the null device, so that a capturing caller returns promptly. Investigate whether, with the standard
   handles redirected, the remaining inherited handles still hold the caller's pipes open on Windows (ADR 0143's finding),
   and find a safe way (no unsafe code) to make the caller return. Limit to register: `rcfd-runner start` run from a
   capturing caller returns within 3 s while the runner stays alive; control: the 48I code fails it. Also check that a runner
   started by `rcfd-mcp` does not keep the server's stdout pipe open after the server exits.
2. Early exit: `ensure_runner` reports a runner that exits before it takes the lock (for example on a bad budgets file) at
   once, with its exit code and the end of its log, instead of after 10 s. Limit to register: reported within 2 s, the
   message includes the exit code and the log tail.
3. Unix keeps `process_group(0)`.
4. Docs, and short notes in ADRs 0123 and 0143.

## Context (read at main 9052841)

* **Start path** (ADR 0143). `Client::ensure_runner_pid` (`crates/rcfd-runner/src/client.rs`) starts
  `rcfd-runner serve --project <absolute>` through `sys::spawn_detached`, which spawns `sys::detached_command`:
  * creation flags `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW` on Windows, `process_group(0)` on Unix;
  * stdin, stdout and stderr all the null device.

  It hands the `Child` to `sys::keep_started` at once, then polls `runner_running()` every 20 ms for 10 s. Callers: the CLI's
  `start` and `ensure` (`main.rs`); the Studio (`quick_run`, and in `poll.rs` "Start runner" and the Start dialog);
  `rcfd-mcp`'s `start_run`.
* **No `serve` detach path.** `serve` runs in the foreground (`main.rs`). The only detached `serve` is the one the client starts.
  This ADR reads decision 1's "`serve` detach path" as that start (a question in the addendum).
* **Bad budgets file.** `serve` loads `.rcfd/runner-config.json` (`Config::load`) before it takes the lock and before it sets
  its log (`LOG`).
  * A bad file ends it with `rcfd-runner: <path>: <error>` on stderr and exit 1 (`main.rs`).
  * With null stderr that line is lost, and `ensure_runner` says "the runner did not take the lock in time" after 10 s.
  * The Studio's Run button checks the budgets first (`quick_run.rs`) and appends the last 3 lines of `runner.log` to an
    `ensure_runner` error. `rcfd-mcp` and the CLI do not check.
* **Start race.** A losing runner exits 3 (`EXIT_ALREADY_RUNNING`) after 10 tries 50 ms apart and prints nothing. ADR 0143's
  N3 requires both racing `ensure_runner_pid` calls to return `Ok`.
* **Reaping.** `runner_running()` calls `sys::lock_held`, which calls `sys::reap_started()`. That `try_wait`s every kept child
  and drops the ones that have exited, so a child kept before the wait would lose its exit status to the first probe.
* **Windows process creation in std 1.99.** Read from `library/std/src/sys/process/windows.rs` and
  `library/std/src/os/windows/process.rs` of the installed toolchain (rustc 1.99.0).
  * `CreateProcessW` gets `bInheritHandles = TRUE` unless the unstable `CommandExt::inherit_handles(false)` (issue 146407) is
    used.
  * No handle list is passed unless the unstable `spawn_with_attributes` with a `ProcThreadAttributeList` (issue 114854) is
    used.
  * The stable `CommandExt` methods are `creation_flags` and `raw_arg`. No creation flag limits inheritance.
  * For each standard handle given to the child (`Stdio::null()`, `Stdio::from(File)`, `Stdio::inherit()`), std makes an
    inheritable duplicate for the spawn and puts it in STARTUPINFO. Nothing in std touches the caller's other inheritable
    handles.
  * No public std API clears a handle's inherit flag. `SetHandleInformation` is used only crate-privately, for sockets.
    `try_clone` makes a new non-inheritable duplicate and leaves the original as it is.
  * Closing a handle by its value needs `from_raw_handle`, which is unsafe.
  * The crates in `Cargo.lock` offer nothing safe for this. `windows-sys`, `windows` and `winapi` are FFI bindings, unsafe to
    call. `winapi-util` 0.1.11 wraps the console, file information and the computer name only.
* **Consequence**, from the `CreateProcessW` documentation and ADR 0123's S5, which failed with null stdio. A handle that a
  process inherits stays inheritable in that process. So every process the client starts through std, and every process those
  start, receives the caller's pipe handles that the client inherited.
  * Redirecting the three standard handles changes what the runner reads and writes, not which handles it holds.
  * An intermediate stage started through std inherits the same handles and passes them on.
* **Unix.** std's spawn puts the given stdio on fds 0-2 and opens every other descriptor close-on-exec. The client's piped
  stdout, its fd 1, is therefore replaced in the runner, so ADR 0143's start with null stdio should already release a capturing
  caller on Unix. Read from code only: S5 is ignored and has not run on Linux.
* **Docs.** Outside the ADRs, `rcfd-runner start` and `ensure` are described only by the binary's help text (`main.rs`
  `USAGE`); `docs/mcp-tools.md` describes `start_run`.

## Decisions

1. **Output of a started runner, in the library and the CLI.**
   * `client::RunnerOutput` has four modes. In every mode the runner's stdin is the null device.

     | Mode | stdout and stderr go to |
     |---|---|
     | `ProjectLog` (the default) | appended to `<project>/.rcfd/runner.log` |
     | `File(path)` | appended to that file. It is created when missing; its folder must exist. A relative path is taken from the caller's working folder |
     | `Caller` | the caller's own stdout and stderr |
     | `Null` | the null device (ADR 0143's start) |
   * `Client::with_output(RunnerOutput)` sets the mode and `Client::output()` reads it. `Client::new` and `with_default_runner`
     use `ProjectLog`, so the Studio and `rcfd-mcp` get it without a change of their own.
   * The client opens the file (append, create) before the start. It passes the file as the runner's stdout and stderr: two
     handles, `Stdio::from`. A file that cannot be opened is a located error, and nothing is started.
   * `File` changes only where stdout and stderr go. The runner's own log lines (`runner <pid> started` and so on) stay in
     `.rcfd/runner.log`, the folder contract of ADR 0123 decision 9.
   * **CLI.** `rcfd-runner ensure` and `start` take `--log-file <file>`, `--stdout` (alias `--foreground`) or `--quiet`.
     * More than one of them is an error: exit 1, nothing started. `--log-file` without a value is an error.
     * Only `--log-file` takes a value, so `start --quiet <case>` reads `<case>`.
     * Other commands ignore the three options.
   * `sys::spawn_detached` gains the stdout and stderr to pass. `sys::detached_command(exe, args)` is unchanged, with null
     stdio; F15, F17 and N4 (b) read it.
2. **Early exit reported at once.**
   * While it waits, the client keeps the started `Child` to itself. It hands the child to `keep_started` afterwards, unless the
     child has already been reaped.
   * Every 20 ms the client first checks the lock, then calls `try_wait` on the child.
   * Exit 3 (`EXIT_ALREADY_RUNNING`: another runner holds the lock) is not an error. The wait goes on until the lock is seen or
     the 10 s deadline passes.
   * Any other exit while the lock is not held ends the wait at once. The error message is
     `the runner (pid N) exited with exit code C before it held the project's lock; ` followed by a part that depends on the
     mode:

     | Mode | The message goes on with |
     |---|---|
     | `ProjectLog`, `File` | `its output (<file>): <tail>`, or `it wrote nothing to <file>`. The tail is the lines the runner appended to that file since this start: at most the last 8 lines and 2000 bytes, joined with ` \| ` |
     | `Caller` | `its output went to this program's standard output and error` |
     | `Null` | `its output was discarded` |

     On Unix, a signal is named as std prints it, in place of the exit code.
   * The 10 s timeout message is unchanged, except that it names the mode's output file (`runner.log` for `Caller` and
     `Null`).
3. **Unix keeps `process_group(0)`.** U1 checks it.
4. **Windows inheritance: no safe route in product code.**
   * With stable Rust and no unsafe code, nothing found above stops the runner from inheriting the caller's handles.
   * The investigation (I1-I6) measures this.
   * No `detach-child` stage is added unless I3 shows that it releases the caller. Predicted: it does not.
   * If C1 and C2 miss on Windows as predicted, they become Windows known misses (rule below), and question 1 lists the
     options that remain.
5. **Docs.**
   * The binary's help text.
   * `docs/run-control.md`: a short section on how the run service is started and where its output goes.
   * `docs/mcp-tools.md`, at `start_run`: where the runner's output goes, and the Windows caveat.
   * Notes in ADRs 0123 and 0143.

**Platform known-miss rule (registered here).** When a limit is missed on one platform only, the test keeps running on the
other. It is renamed `<name>_known_miss_on_<platform>` and gets `#[cfg_attr(<platform cfg>, ignore = "known miss on
<platform>, ADR 0146: ...")]`, with the limit intact. This is ADR 0143's C3/P1 pattern with the house rule's suffix.

## Registered checks

* All runs are release builds with `--test-threads 1`. The test processes' own output goes to files: on Windows a started
  runner also inherits the test process's handles.
* `rcfd`, `rcfd-runner` and `rcfd-mcp` come from the same build as the test.
* "Before" runs use the commit that adds this job's tests and changes no non-test code. Its non-test code is main 9052841, the
  48I code. "After" runs use the commit with the change.
* Windows runs are on this host.
* Linux runs use Docker `rust:1.99-bookworm` with `--init --cpuset-cpus 0-3 --memory 16g`, on a `git archive` of the commit,
  extracted with `tar -m` inside the container before its build.

**Capturing-caller harness** (C1, C2, I1-I4).
* The measured process is started with stdin set explicitly (null for C1 and I1-I4, piped for C2) and with stdout and stderr
  piped.
* One reader thread per pipe reads to EOF and sends the instant of the EOF. The main thread polls `try_wait` every 10 ms for the
  instant of the exit.
* **Returned:** the process has exited and both pipes have reached EOF, which is what `wait_with_output` waits for.
* **Held at T:** at time T after the exit, at least one EOF is still pending.
* No bare `wait_with_output` is used. A guard shuts the runner down (or stops the daemon) so that a held pipe ends.

| Id | Check | Limit | Prediction | Negative control |
|---|---|---|---|---|
| E1 | Early exit through the library default. The project's `.rcfd/runner-config.json` is `{"max_runs": 0}`; `Client::new(dir, rcfd-runner).ensure_runner()` is timed | `Err` within **2.0 s** of the call. The message contains `exit code 1`, `must be positive` and `runner-config.json` (the runner's own line, from the tail). `.rcfd/runner.log` holds the line `rcfd-runner: <config path>: ...must be positive`. The lock probe reads false afterwards | pass, Windows and Linux | **E1c** (before): the same test must fail: about 10 s, no `exit code` in the message, no such line in `runner.log`. **E1 control** (after): with a valid config, `ensure_runner()` returns `Ok`, so the early-exit path does not fire on a healthy start; N3 is re-run (a losing racer's exit 3 is not an error) |
| O1 | CLI default: `rcfd-runner ensure --project <p>` on the bad-budgets project, with stdin null and stdout, stderr to files | exit 1 within **2.0 s**; its stderr contains `exit code 1` and `must be positive`; `.rcfd/runner.log` contains `must be positive` | pass | (E1c) |
| O2 | The same with `--log-file <p>/out/x.log` (the folder exists) | exit 1 within 2.0 s. `x.log` contains the runner's `must be positive` line. `.rcfd/runner.log` does not contain `must be positive` (absent, or without it). Stderr contains `exit code 1`, `must be positive` and `x.log` | pass | O1: the default puts the line in `runner.log` |
| O3 | The same with `--stdout`, and once more with `--foreground` | exit 1 within 2.0 s. The CLI's stderr file contains both the runner's own line (`rcfd-runner: `, the config path, `must be positive`) and the client's `exit code 1`. `.rcfd/runner.log` has no `must be positive` | pass | as O2 |
| O4 | The same with `--quiet` | exit 1 within 2.0 s. Stderr contains `exit code 1` and `discarded`. `must be positive` is in neither the CLI's stderr nor `.rcfd/runner.log` | pass | as O2 |
| O5 | Option errors and parsing. (a) `ensure --stdout --quiet`. (b) `ensure --log-file` with no value. (c) `start --quiet <case> --set solver.max_iterations=30` on a healthy project, stdout and stderr to files | (a) and (b): exit 1; stderr names the options (a: `--log-file`, `--stdout` and `--quiet`; b: `--log-file`); no runner started (lock probe false, no `runner.json`). (c): exit 0, stdout is a run id, and the run finishes | pass | (c) shows that an option without a value does not take the case as its value |
| C1 | Capturing caller, CLI. A healthy project (`max_runs` 1); `rcfd-runner start --project <p> <case> --set solver.max_iterations=60`, stdin null, stdout and stderr piped | **returned within 3.0 s** of the spawn; exit 0; stdout is one run id. At the return, the lock probe reads true and `runner.json`'s pid is alive as `rcfd-runner`. The run then finishes, and the lock probe still reads true | **Windows: miss** (the runner inherits the client's pipes; Context). Linux: pass | **C1 control** (after): the same with `--stdout` appended must be held at 3.0 s after the client's exit, on both platforms (the runner holds the pipes by design); both EOFs arrive within 5 s after the shutdown. **C1c** (before): predicted fail on Windows (held), pass on Linux; on Linux C1c cannot discriminate, the control does. Set-up: the lock probe reads false before the start |
| C2 | Capturing caller, `rcfd-mcp`. `rcfd-mcp --project <d> --root <d>` with stdin, stdout and stderr piped; `initialize`; `tools/call start_run {case: cases/cavity.toml}` (30 steps) answered without error; then stdin is closed | the server exits 0 within 10 s of the close (set-up). Its stdout reaches EOF within **3.0 s** of its exit. At that EOF, or at exit + 3 s, the lock probe reads true (the runner stays alive) | **Windows: miss**. Linux: pass | **C2 control:** a child entry of the same test binary (gated by its environment) starts `rcfd-runner serve` through `sys::detached_command`, with stdout and stderr changed to `Stdio::inherit()`. It waits for the lock, prints a line and exits 0. Its stdout must be held at 3.0 s after its exit, on both platforms; EOF arrives within 5 s after the shutdown. **C2c** (before): predicted fail on Windows, pass on Linux |
| U1 | Unix: the runner started by `ensure_runner_pid` leads its own process group | `ps -o pgid= -p <pid>` equals the pid | Linux: pass (not compiled on Windows) | the test process's own pgid differs from that pid |
| I1-I4 | Investigation, pure std with no rcfd code, `#[ignore]`d and run by hand. The test binary plays three roles through a child entry gated by its environment: the client, an intermediate stage, and a long-lived daemon that lives until a stop file appears (at most 30 s). The client is started with stdin null and stdout, stderr piped. **I1:** the client starts the daemon with null stdio (ADR 0143's start) and exits. **I2:** the client starts the daemon with stdin from an empty file and stdout, stderr appended to a file (`Stdio::from(File)` for all three). **I3:** the client starts an intermediate stage (null stdio) and waits for it; the intermediate starts the daemon (null stdio) and exits. **I4:** the client starts nothing | reported: held or released at 3.0 s after the client's exit. Asserted only: I4 is released (the harness sees a prompt EOF), and every daemon is gone at the end | **Windows: I1, I2 and I3 held, I4 released. Linux: all released** | I4 |
| I5, I6 | Root-cause probes in a scratch crate outside the repository (not committed). **I5:** the I1 set-up with `CommandExt::inherit_handles(false)`. That method is unstable and is enabled for the probe only, with `RUSTC_BOOTSTRAP=1`; it involves no unsafe code. **I6:** as I5, but the daemon gets a log file as stderr and writes one line | reported | Windows: I5 released. I6 released, and the line does not reach the file, because standard handles need inheritance | I1 |
| RR | Re-runs: the whole `rcfd-runner` crate (S1-S11, S5a, the F-tests, N3, N4, Z1, the contract and unit tests); the whole `rcfd-studio` crate (ADR 0143 recorded 3 failures there that were already on main, G1b and the G1 and G4 controls); `rcfd-mcp` `project_runs_w48f` and `project_w48f`; S5 with `--ignored` (reported only); fmt; clippy on the workspace (Windows) and on `-p rcfd-runner` (Linux); `workspace_graph`; the N4 (c) and F17 source scans | pass as before; known misses stay ignored | | (their own) |

### Set-up details

* E1, O1-O5, C1, the C1 control and U1 are in `crates/rcfd-runner/tests/output_w48l.rs`.
  * Each test has its own project, `<temp>/rcfd_runner_w48l_<pid>_<name>`.
  * A healthy project gets `{"max_runs": 1, "cpu_threads": 8, "mem_budget_mb": 100000, "poll_ms": 100, "adopt_probe_ms": 500}`
    and the 12 x 12 cavity of `runner_w47b`.
  * The file uses only APIs that exist before the change (the CLI and `ensure_runner`), so the before runs use the same file.
  * The O checks' CLI processes are polled with `try_wait`, at most 30 s.
* C2 and its control are in `crates/rcfd-mcp/tests/runner_stdio_w48l.rs`, with the shared `tests/common` (`project`, `binaries`).
  The control's child entry is `c2_control_child_entry`, which does nothing without its environment.
* I1-I4 are in `crates/rcfd-runner/tests/inherit_w48l.rs`. The child entry is `i_child_entry`, which does nothing without its
  environment.
* Every started runner is shut down with `Client::shutdown(10 s)`; a recorded runner pid still alive after that is killed.
  Only the run's own solver is killed.
* Before runs (Windows): E1c, C1c and C2c, with `--exact`. I1-I4 do not depend on the change and run once, after the
  registration.
* After runs (Windows): every check in the table.
* Linux after run: `output_w48l`, `inherit_w48l` with `--ignored`, `runner_stdio_w48l`, `native_start_w48i` and the whole
  `rcfd-runner` crate. Linux before run: E1c, C1c and C2c.

## What I already knew when registering (rule d)

* The code reading in Context, the std 1.99 source, ADR 0123's S5 failure with null stdio on Windows (amendment 3), and
  ADR 0143's results.
* Nothing of this job was built or run before this registration. The tests are written after it, and the code change after the
  before runs.
* The predictions in the table are mine, from the reading above, not from measurements.

## Addendum: results against the registered limits (2026-10-28)

Commits:
* 6f1d109: the registration.
* 7b5b309: the tests, with no non-test code changed. The "before" runs used it.
* fa69632: the change.
* 65f9228: the known-miss attributes, the docs and the notes.
* 959ba18 and the commit after it: this addendum, the known-miss notes in the two new test files' module docs, and the
  wording of `start_run` in `docs/mcp-tools.md`. No code.

Set-up:
* Windows: this host (16 threads, 93.6 GB, 32 GB free at the start, rustc 1.99.0). Release builds in the worktree's own target
  folder, `--test-threads 1`, output to files.
* Linux: Docker `rust:1.99-bookworm` (glibc 2.36, rustc 1.99.1), `--init --cpuset-cpus 0-3 --memory 16g`, on archives of
  7b5b309 and fa69632 extracted with `tar -m`.
* No new crate, `Cargo.lock` unchanged, no unsafe code, no numerics changed.

| Id | Limit | Windows | Linux | Result |
|---|---|---|---|---|
| E1 | `Err` within 2.0 s, naming `exit code 1`, `must be positive` and `runner-config.json`; the runner's line in `runner.log`; lock probe false | **0.042 s**: "the runner (pid 67652) exited with exit code 1 before it held the project's lock; its output (`...\.rcfd\runner.log`): rcfd-runner: `...\.rcfd\runner-config.json`: max_runs, cpu_threads and poll_ms must be positive". `runner.log` holds that line; lock probe false | 0.021 s, the same message | pass |
| E1c | before: fails (about 10 s, no exit code, no line) | 10.072 s, "the runner did not take the lock in time (see `...runner.log`)", `runner.log` empty | 10.015 s, the same | fails as required |
| E1 control | a valid config: `Ok`, lock held | `Ok`; lock held | `Ok`; lock held | as required |
| N3 (re-run as E1's control) | both `Ok`, both started; one alive, `runner.json`'s; one "started" line; lock probe true | `Ok(Some(66008))` and `Ok(Some(67180))`; alive [67180], which is `runner.json`'s; 1 line; probe true. Control: two projects, both alive | pass | pass |
| O1 | exit 1 within 2.0 s; `exit code 1` and `must be positive` on stderr; the line in `runner.log` | 0.061 s | 0.031 s | pass |
| O2 | `--log-file`: the line in `x.log`, not in `runner.log`; stderr names `x.log` | 0.060 s; `x.log` has the line; no `runner.log` | 0.031 s | pass |
| O3 | `--stdout`, `--foreground`: the runner's own line and the client's report on the caller's stderr; nothing in `runner.log` | 0.060 s and 0.049 s; the runner's line, then "...; its output went to this program's standard output and error" | 0.031 s each | pass |
| O4 | `--quiet`: `exit code 1` and `discarded`; the line nowhere | 0.061 s; "...; its output was discarded" | 0.031 s | pass |
| O5 | (a), (b) refused with nothing started; (c) `start --quiet <case>` | (a) "choose one of --log-file, --stdout (or --foreground) and --quiet"; (b) "--log-file needs a file"; nothing started in either. (c) exit 0 after 0.092 s, a run id, the run finished at step 30 | pass | pass |
| C1 | returned within 3.0 s; the runner alive at the return; the run finishes | **held**. The client exited 0 after 0.102 s; both EOFs came only after the runner's shutdown request (0.014 s after it, 3.53 s after the start) | returned after 0.051 s; lock probe true, `runner.json`'s pid alive; the run finished; probe still true | **Windows: miss, as predicted.** Now `c1_capturing_caller_returns_within_3_s_known_miss_on_windows`, ignored on Windows, limit intact. Linux: pass |
| C1 control | `--stdout`: held 3.0 s after the client's exit; EOFs within 5 s of the shutdown | held; EOFs 0.013 s after the shutdown request | held; 0.083 s | as required, both platforms |
| C1c | before: Windows fails, Linux passes | held: client exit 0.067 s, EOFs 0.015 s after the shutdown request | returned after 0.061 s | as predicted |
| C2 | `rcfd-mcp` exits 0 within 10 s of the close; its stdout reaches EOF within 3.0 s of its exit; the runner alive | **held**. The server exited 0, 0.010 s after the close; its stdout reached EOF only 0.082 s after the runner's shutdown request | exit 0.010 s after the close; stdout EOF 0.0 s after the exit; lock probe true; the run finished | **Windows: miss, as predicted.** Now `c2_mcp_stdout_reaches_eof_after_the_server_exits_known_miss_on_windows`, ignored on Windows, limit intact. Linux: pass |
| C2 control | the child whose runner got its stdout is held 3.0 s after its exit; EOF within 5 s of the shutdown | held; EOF 0.007 s after the shutdown request | held; 0.070 s | as required, both platforms |
| C2c | before: Windows fails, Linux passes | held; EOF 0.030 s after the shutdown request | EOF 0.0 s after the exit | as predicted |
| U1 | Linux: the runner's process group is its pid | (not compiled) | runner 918 in group 918; the test process in group 7 | pass |
| I1-I4 | reported against the predictions; I4 released; no daemon left | **I1 held, I2 held, I3 held, I4 released**. In I1-I3 the EOFs came 5-12 ms after the stop file, that is when the daemon exited; no daemon left | all four released (EOF within 3 ms of the client's exit); no daemon left | as predicted |
| I5, I6 | scratch probe (not in the repository), reported | I1 rerun as the control: held. **I5** (`inherit_handles(false)`): released. **I6**: released, but the daemon's line never reached its log file; its write returned `Ok(())` and the line was discarded | not run | as predicted |

### Re-runs (RR)

* **`rcfd-runner`, Windows (fa69632).** Unit tests 8; `contract_w48a` 2; `inherit_w48l` 1 (the child entry), 1 ignored;
  `native_start_w48i` 3; `no_window_scan` 1 (F17 scan); `output_w48l` 8; `runner_w47b` 13, 1 ignored (S5); `runner_wfix` 6;
  `zombie_w48i` 1.
  * C1 also ran in this re-run and failed: the binary predates its known-miss attribute (deviation 1).
  * N4 (c) found no shell name in 35 files.
* **`rcfd-runner`, Linux.** Unit tests 7; contract 2; inherit 1, 1 ignored; native 3; scan 1; `output_w48l` 10 (C1 and U1
  included); `runner_w47b` 13, 1 ignored; `runner_wfix` 4 (F15 is Windows only); zombie 1.
* **S5 with `--ignored`** (reported only).
  * Windows: fails at its 10 s EOF wait ("the client's stdout did not reach EOF: the runner inherited the pipe").
  * Linux: **passes** (3.29 s). See question 2.
* **`rcfd-mcp`** `project_runs_w48f` 3 and `project_w48f` 10, on Windows and on Linux.
* **`rcfd-studio`, Windows, the whole crate:** 154 passed, 3 failed, 3 ignored, counting the re-run of deviation 7.
  * The runner-start tests all pass: N1 and N2 (ADR 0143), R1-R3, R9, G4-G7, `bins_g4`, SF2, SF4, SF5 and SF8.
  * The 3 failures are the ones ADR 0143 recorded on main (its question 3), with the same assertions: G1b ("renders 17
    uploads 2"), the G1 control ("a frame without the auto-select") and the G4 control (`!vp.contains(&reason)`).
  * The 3 ignored tests are the known misses G1, B9 tutorials and R6.
* **`rcfd-cli` `workspace_graph`:** 3 passed; no edge changed.
* **Lint.**
  * `cargo clippy -p rcfd-runner -p rcfd-mcp -p rcfd-studio --all-targets -- -D warnings` (Windows): clean.
  * `cargo clippy --workspace --all-targets -- -D warnings` (Windows): clean.
  * Linux, on 65f9228: `cargo clippy -p rcfd-runner -p rcfd-mcp --all-targets -- -D warnings` and
    `cargo fmt -p rcfd-runner -p rcfd-mcp -- --check` are clean. In the same container, `output_w48l` passed 10 (the renamed C1
    runs there) and `runner_stdio_w48l` passed 3 (the renamed C2 runs there).
  * `cargo fmt -p rcfd-runner -p rcfd-mcp -- --check` (Windows): clean.
* **Leftover processes:** none on Linux (`ps` at the end of each container run). None on Windows (`rcfd`, `rcfd-runner`,
  `rcfd-mcp`, `fake-claude` and the probe, checked after all runs).

### Deviations and notes

1. **C1 in the Windows crate re-run.** That re-run used the build of fa69632, so it ran C1 before C1 had its known-miss
   attribute. C1 failed there in the same way as in the registered run.
2. **Overlapping runs.** The Linux after run overlapped the Windows `rcfd-runner` crate re-run, and then the Studio build.
   Both have generous time limits. The registered Windows runs (E1, O1-O5, C1, C2, N3) ran with nothing else of this job
   running. This follows ADR 0143 deviation 5.
3. **Toolchains.** Linux used rustc 1.99.1 and Windows 1.99.0, as in ADR 0143. I1-I4 ran once on Windows, with the before
   runs; they do not depend on the change, as registered.
4. **Linux lint.** The registered Linux clippy step first failed to start, because `rust:1.99-bookworm` has no clippy or
   rustfmt component. It ran again on 65f9228 after `rustup component add clippy rustfmt` in the container (results above).
5. **C1's early-exit path is not involved in its miss.** On Windows the client returned its run id in about 0.1 s. Only the
   EOF waited. The runner's own output went to `runner.log`, as decided.
6. **Not changed, read from code: the Studio repeats the log tail.** The Studio's Run button (`quick_run`) appends the last 3
   lines of `runner.log` to an `ensure_runner` error. For an early exit, the client's message now carries the tail already, so
   the same lines can appear twice. The Run button checks the budgets first, so this happens only for other early exits
   (question 5).
7. **Studio re-run set-up.** The first whole-crate Studio re-run had 15 more failures: all 14 ADR 0134 chat tests in
   `chat_e`, and `parity_assistant_access`. All of them stopped at their set-up check, "`target\release\fake-claude.exe`
   missing": this job's fresh target folder had never built rcfd-claude's replay double. After `cargo build --release -p rcfd-claude --bin fake-claude`, `chat_e` passed 14 and `parity` passed 5
   (1 ignored, as before). No code changed in between.

### Not run

* A real window, by hand: the Studio's first Run of a session with the runner's output in `runner.log`; closing the Studio
  while a run runs.
* Claude Code's own Bash and PowerShell tools as the capturing caller. The registered caller is std's `Command` with piped
  output.
* What Claude Code does when `rcfd-mcp`'s stdout stays open after the server exits (C2's Windows miss).
* `--stdout` from an interactive Windows console. It is not checked whether a detached runner (`DETACHED_PROCESS`) can write
  to an inherited console handle; the checks used files and pipes.
* Linux CI (`ubuntu-latest`); Docker bookworm stood in.
* The `rcfd-studio` tests on Linux; macOS; debug builds of the new tests; the full gate (`scripts/gate_test.py`);
  `cargo deny`.
* I5 and I6 on Linux. They are Windows-only probes, and Unix has no such switch.

### Questions for the maintainer

1. **What remains for a capturing caller on Windows (C1 and C2 miss).**

   Measured: a std-spawned runner holds the caller's pipes whatever its standard handles are (I1, I2), and so does an
   intermediate stage (I3). Only turning inheritance off releases the caller (I5), and that also drops the standard handles
   given to the runner (I6). Options:
   * (a) **Accept and document**, as done. On Windows, start the runner where nobody reads the output to the end: the
     Studio, an interactive terminal, or a background job of the calling tool.
   * (b) **Allow one reviewed unsafe island**: `CreateProcessW` with `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` holding only the
     three standard handles. This keeps the log file and drops everything else.
     * It conflicts with `unsafe_code = "forbid"`: it needs a lint exception for one module or one small crate.
     * Clearing the inherit flag of the client's own standard handles (`SetHandleInformation`) would be smaller, but it
       leaves the other handles the caller leaked to the client. For example, a shell's own pipe that the shell passed on
       as an extra handle.
   * (c) **Wait for std.** `CommandExt::inherit_handles(false)` (issue 146407) releases the caller. But the runner then gets
     no usable standard handles (I6), so it would have to open its own log file:
     * a `serve --log-file`, opened by the runner itself, plus a panic hook;
     * the early-exit report would then read that file.

     A handle list through `spawn_with_attributes` (issue 114854) would need std to put its own duplicates of the standard
     handles into the list. A caller cannot name those handles.
   * (d) **A launcher that does not inherit.** For example ShellExecute through `rundll32 shell32.dll,ShellExec_RunDLL`;
     ADR 0123 measured that ShellExecute passes no handles. Not verified here. It puts a launcher back into the start path
     (against ADR 0143 decision 1). The client loses the `Child`, so there is no exit code for decision 2 and no reaping.
     It may open a console window, and it can hang as PowerShell did.

   **Recommendation:** (a) now. Then (c) when `inherit_handles` is stable, together with the runner opening its own log. (b)
   only if the fix is wanted now and one reviewed unsafe module is acceptable.
2. **S5 passes on Linux** (`--ignored`, 3.29 s), as C1 does. Options:
   * (a) keep S5 ignored everywhere: C1 runs the same set-up on Linux with a tighter limit and the same survival checks;
   * (b) ignore S5 on Windows only, so that the Linux gate runs it too.

   **Recommendation:** (a). It avoids two copies of one check in the gate.
3. **`--foreground` is an alias of `--stdout`**, as decision 1 names both. It does not keep the runner in the foreground;
   that is `serve`. Options:
   * (a) keep the alias, documented as the same option;
   * (b) drop it, to avoid the confusion with `serve`.

   **Recommendation:** (a), since it is the decision's own word. Revisit if users read it as "run in the foreground".
4. **"`serve` detach path".** It was read as the detached `serve` that `start`, `ensure` and the library start: there is no
   other detach path. `serve` itself, run in the foreground, takes no output option. Options:
   * (a) leave `serve` as it is: whoever starts it in the foreground redirects its output;
   * (b) give `serve` its own `--log-file`, which the runner opens itself for its pre-lock errors and panics.

   **Recommendation:** (a) now. (b) only together with question 1 (c), which needs it.
5. **The Studio's duplicate log tail** (deviation 6). Options:
   * (a) leave it;
   * (b) a small Studio change: `quick_run` appends its 3 lines only when the error does not already carry the runner's
     output.

   **Recommendation:** (b), as a separate Studio change. It was not done here because it is outside the runner start.
6. **Linux CI.** Run the CI unit tests and the Full gate on `ubuntu-latest` after the merge. **Recommendation:** yes.

## Files

* This ADR.
* `crates/rcfd-runner/src/{client,sys,main}.rs`.
* New tests: `crates/rcfd-runner/tests/{output_w48l,inherit_w48l}.rs`, `crates/rcfd-runner/tests/capture/mod.rs` and
  `crates/rcfd-mcp/tests/runner_stdio_w48l.rs`.
* S5's text in `crates/rcfd-runner/tests/runner_w47b.rs`.
* `docs/run-control.md` (section "Run service") and `docs/mcp-tools.md` (`start_run`, the Windows note).
* Notes in ADRs 0123 and 0143.
