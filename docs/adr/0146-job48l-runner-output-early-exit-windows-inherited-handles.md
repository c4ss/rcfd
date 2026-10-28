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
