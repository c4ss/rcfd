# ADR 0143: Job 48I, the runner started natively (no PowerShell), dead Unix runners read as dead, two Linux known misses

Status: **registered** 2026-10-28, before any run of the checks below (set-ups and limits are fixed here; nothing of this job
had run); results in the addendum; not yet accepted by the maintainer. House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed before the registered
runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written
before the corrected run; defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not
re-recorded. Parents: [ADR 0123](0123-wave47b-run-service.md) (decision 2, amendments 2 and 3), [ADR 0133](0133-wave48d-run-control-and-live-monitoring.md)
R9, [ADR 0141](0141-wave48g4-one-click-run-and-follow.md) question 11, [ADR 0137](0137-linux-gate-platform-values.md) ("Reported,
not changed": S4a and S4b, C3, P1).

The maintainer's decisions of 2026-10-28 that this job implements:

1. PowerShell out of the loop: the Studio starts the runner natively, by spawning the `rcfd-runner` binary directly (detached on
   Windows by creation flags, its own process group on Unix), with no PowerShell and no `sh` in the start path; the runner still
   survives the Studio's exit, and a double start does not leave two runners. The PowerShell route is removed or demoted.
2. Linux zombie fix (gate failures S4a and S4b): reap with `try_wait` where the parent owns the child, and treat process state Z
   as dead; a test that fails before the fix on Linux and passes after.
3. C3 `c3_boundedness` and P1 `p1_pipeline_cylinder_in_box` become Linux known misses, limits intact, Windows behaviour
   unchanged, no numerics changed.

## Context (read at main e11c656)

* `Client::ensure_runner` (`crates/rcfd-runner/src/client.rs`):
  * Windows: `sys::spawn_detached` runs `powershell -NoProfile -NonInteractive -EncodedCommand <Start-Process ...>` (flags
    `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`) and waits 5 s for the lock. Then it kills the launcher,
    starts the runner directly (`sys::spawn_direct`: the same flags, null stdio) and waits 10 s more (ADR 0123 amendment 2).
  * Unix: the direct start (`process_group(0)`, null stdio), then a 10 s wait.
  * The `Child` of the direct start is dropped and never waited for.
* On this host the PowerShell route does not take the lock within 5 s, so the first start of a session waits about 5.3 s and
  then takes the direct start: ADR 0141 G1 5.339 s and G1b 5.353 s from the click to the run shown running, ADR 0133 R9 5.166 s.
* Every client goes through `ensure_runner`:
  * the Studio's Run button (`quick_run::start_job`), its "Start runner" (`poll.rs`, `Command::EnsureRunner`) and its Start
    dialog (`Client::start` on the run worker);
  * `rcfd-mcp`'s `start_run` tool;
  * the CLI's `rcfd-runner start` and `rcfd-runner ensure`.
* Windows handles (read from the std source of rustc 1.99):
  * std's spawn calls `CreateProcessW` with handle inheritance on. The switch `CommandExt::inherit_handles` is unstable
    (feature `windows_process_extensions_inherit_handles`, issue 146407).
  * The workspace forbids unsafe code (`[workspace.lints.rust] unsafe_code = "forbid"`), so `SetHandleInformation` and a
    handle list (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`) are out of reach.
  * So a direct start hands the runner every inheritable handle of its starter. A starter whose stdout is a pipe that another
    process reads to the end keeps that reader waiting until the runner exits; `Stdio::null()` does not prevent it (ADR 0123
    Context). This is the S5 known miss (ADR 0123 amendment 3), and it is what already happens on this host for every start,
    because the fallback always runs here.
* Unix detachment: std's `CommandExt::setsid` is unstable (feature `process_setsid`, issue 105376), and `pre_exec` with
  `setsid(2)` needs unsafe code. `process_group(0)` (stable since 1.64) is what the code uses.
* Unix liveness:
  * `sys::pid_alive` runs `ps -p <pid> -o comm=` and checks the name. `lock_held` applies it to the pid in `runner.lock`,
    and the runner's `unix_takeover` applies it before it takes over a stale lock.
  * ADR 0137's diagnosis: a killed runner whose starter is still alive stays a zombie, because the dropped `Child` is never
    waited for. `ps` still prints the zombie's name, so `runner_running()` stays true and no new runner can take over the lock.
    S4a and S4b fail on Linux for that reason.
* C3 and P1 (ADR 0137): C3 misses by 1 ulp on Linux (the glibc `cbrt` round trip in `Cavitation::advance`). P1's default tet
  mesh on Linux has one face at 86.06 deg non-orthogonality, above `check_mesh`'s 85 deg error threshold. Both pass on Windows.
  The CI platforms are `ubuntu-latest` and `windows-latest` (`.github/workflows/ci.yml`, `gate.yml`); macOS is not built.
* Tests built on the PowerShell route's code:
  * `sys::tests::no_window_flags_on_every_console_spawn` (ADR 0128 finding 17) lists the PowerShell launcher.
  * `runner_wfix::f15_command_line_round_trips_through_the_real_argv_parser` (ADR 0128 finding 15) uses `sys::command_line`
    and `sys::start_script`.
  * `sys::tests::base64_matches_rfc4648_vectors` tests `sys::base64`, which only the `-EncodedCommand` argument uses.
* `scripts/studio.ps1` and `scripts/studio.sh` build and start the Studio in the foreground; they do not start the runner.
  Outside the ADRs no document describes the start route (grep for PowerShell, `Start-Process`, `ensure_runner`, detached).

## Decisions

1. **One start route, direct, on every platform.**
   * `sys::spawn_detached(exe, args)` starts `exe` itself, with null stdio:
     * Windows: creation flags `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW` (the command `spawn_direct` used,
       ADR 0123 amendment 2, ADR 0128 finding 17);
     * Unix: `process_group(0)`.
   * No PowerShell, no `cmd`, no `sh` anywhere in the start path.
   * `ensure_runner` starts the runner once and waits up to 10 s for the lock. Before, Windows waited 5 s and then 10 s more.
   * The PowerShell launcher is **removed**, not demoted: `launcher_cmd`, `start_script`, `command_line`, `base64` and
     `spawn_direct` go; `sys::detached_command(exe, args)` returns the command and its flags (the tests read it).
   * New `Client::ensure_runner_pid() -> Result<Option<u32>, String>`: the pid of the process the call started (`None` when a
     runner was already alive). That process is the runner itself. It may have lost a start race and exited (exit 3) while the
     other runner holds the lock. `ensure_runner()` keeps its signature and returns `ensure_runner_pid().map(|_| ())`.
   * **Survival of the starter's exit:** the mechanism is unchanged. Windows: detached, no console, its own process group.
     Unix: its own process group, null stdio.
   * **Double start:** the mechanism is unchanged. The lock decides; the loser exits 3 after 10 tries 50 ms apart (ADR 0123 S8).
   * **Cost on Windows (stated, not fixed):** every start hands the runner its starter's inheritable handles, as every start on
     this host already did.
     * The Studio, started from Explorer, a shortcut or the scripts (foreground, output not captured), holds no handle that
       anyone reads to the end, so for the Studio nothing changes.
     * `rcfd-runner start` or `ensure` run by a shell that captures their output: the capture waits until the runner exits
       (S5 stays an ignored known miss; its reason text is updated).
     * `rcfd-mcp`: its stdio pipes stay open in the runner and the runner's solvers until those exit.
     * A test process whose output is piped (`scripts/gate_test.py`): its output ends when the test's drop guard has shut the
       runner down.
     * Question 1 lists the ways out.
   * **Unix ("setsid-style"):** a new session needs an unstable API or unsafe code, so `process_group(0)` with null stdio stays.
     Ctrl+C in the Studio's terminal does not reach the runner. On a hang-up the shell signals its jobs and the kernel signals
     the foreground process group; the runner's own group is neither. Stated as a deviation from "setsid-style".
2. **Reaping where the parent owns the child.**
   * `ensure_runner_pid` hands the started `Child` to a process-wide list (`sys::keep_started`).
   * `sys::reap_started` calls `try_wait` on each kept child and drops those that have exited.
   * `lock_held` and `pid_alive` call `reap_started` first, on every platform. Unix: a runner this process started and that
     died is reaped before any probe can see it as a zombie. Windows: its process handle is released.
3. **A zombie is dead.**
   * On Unix, `pid_alive` asks `ps -p <pid> -o stat= -o comm=`. The process counts as alive only when `ps` succeeds, its state
     does not start with `Z` (zombie) or `X` (dead), and its name ends with the name asked for (the old name rule).
   * The parse is a public pure function, `sys::ps_alive(output, name)`, compiled and unit-tested on every platform.
   * This covers processes that the prober did not start: an adopted solver whose parent was a killed runner, or a runner whose
     starting client is alive and has not probed yet.
4. **C3 and P1 are Linux known misses.**
   * `#[cfg_attr(not(windows), ignore = "known miss on Linux, ADR 0143: ...")]` on `c3_boundedness`
     (`crates/rcfd-fv/tests/cavitation.rs`) and `p1_pipeline_cylinder_in_box` (`crates/rcfd-mesh/tests/tetmesh.rs`).
   * The test bodies, and so the limits, are unchanged. On Windows both run as before, under the same names.
   * `not(windows)` is the maintainer's spelling. Linux is the only non-Windows platform that is built (CI), and the misses were
     measured on x86_64 Linux only, so on any other platform the tests are ignored without evidence there either way.
5. **Tests that change with the removed code** (limits unchanged):
   * F17 (`no_window_flags_on_every_console_spawn`) drops the PowerShell launcher from its list of console spawns.
     `tasklist`, `taskkill`, `rcfd --version` and the runner start (`detached_command`) stay, with the same assertions.
   * F15's round trip runs the runner through the direct start's command (`detached_command`, std's quoting) instead of
     `sys::command_line`. It keeps the same nine arguments and the same pass rule: exit 1, and stderr starts with
     `rcfd-runner: <arg>: `. The `start_script` text assertion goes with the script.
   * `base64_matches_rfc4648_vectors` goes with `base64`.
   * S5 keeps its body and limit; its ignore reason and doc comment now say that the direct start is the only route.
   * The doc comments of R9 (`runner_start_r9.rs`) and G1 (`oneclick_e2e_g4.rs`) that say "either route" or "the direct-start
     fallback" are reworded; their code and limits are unchanged.

## Registered checks

All runs are release builds with `--test-threads 1`, with output going to files; `rcfd` and `rcfd-runner` come from the same
build as the test. Windows runs are on this host. Linux runs use Docker `rust:1.99-bookworm` (ADR 0137's image),
`--init --cpuset-cpus 0-3 --memory 16g`, on a `git archive` of the commit named, extracted with fresh file times (`tar -m`) into
the container before its build.

| Id | Check | Limit | Reason | Negative control |
|---|---|---|---|---|
| N1 | First Run of a session, through the drawn app: G1's set-up without the GPU part, on the 12 x 12 lid cavity with `max_iterations = 2000` | the run's `status.json` says `running`, or a terminal state, with a solver pid within **1.5 s** of the click's release frame; the run then finishes with step 2000 | the maintainer's limit ("e.g. 1.5 s on this host") | **N1c:** the same test, unchanged, on the code before the change (the commit that adds this job's first tests, whose non-test code is main e11c656's: the PowerShell route, then the direct start), run once on this host before the change. It must exceed 1.5 s (the timings in Context suggest about 5.3 s) |
| N2 | The runner survives closing the Studio. A child process (this test binary) opens the project in a Studio model, starts the cavity (`max_iterations = 15000`) with the Run path (`quick_run::run_selected_case`), waits until `status.json` says `running` and the start has reported, then ends with `std::process::exit(0)` | the child exits 0. After its exit the lock probe reads true and the run is not terminal (a set-up check: closed while running). The run then finishes, and the lock probe still reads true after that | ADR 0123 S5a and ADR 0133 R2, with the Studio's own start path and a real process exit | the lock probe reads false before the child starts and after `shutdown` |
| N3 | Double start. Two threads call `ensure_runner_pid` on one fresh project at the same moment (a `Barrier`) | both return `Ok`. Both started a process (a set-up check: the race happened). 1.5 s after both returned, exactly one of the two pids is alive as `rcfd-runner`, and it is `runner.json`'s pid. `runner.log` has exactly one "started" line, and the lock probe reads true | the lock contract (ADR 0123 decision 2, S8) through the client | the same two starts on two different projects: both pids alive, each its own project's `runner.json` pid |
| N4 | The start path. (a) On a fresh project, the pid that `ensure_runner_pid` returns is `runner.json`'s pid and is alive as `rcfd-runner`: no launcher process sits between. (b) Unit: `detached_command`'s program is the runner executable itself; on Windows its flags contain `DETACHED_PROCESS`, `CREATE_NEW_PROCESS_GROUP` and `CREATE_NO_WINDOW`. (c) Scan: the non-test sources of `rcfd-runner` and `rcfd-studio` (text before the first `#[cfg(test)]` of each file) hold none of the string literals `"powershell"`, `"powershell.exe"`, `"pwsh"`, `"pwsh.exe"`, `"cmd"`, `"cmd.exe"`, `"sh"`, `"/bin/sh"`, `"bash"`, nor the text `Start-Process` | exact | decision 1 | (c): the scanner finds the pre-change launcher line `console("powershell", DETACHED_PROCESS \| CREATE_NEW_PROCESS_GROUP)`, kept as a copy in the test, and ignores it after a `#[cfg(test)]`. (a): before the change the started process was PowerShell (read from code; not runnable after the change) |
| Z0 | `sys::ps_alive` on simulated `ps -o stat= -o comm=` output | alive exactly for `S    rcfd-runner`, `Ssl  rcfd-runner`, `R+   rcfd`, `D    rcfd`, `T    rcfd`, `I    rcfd-runner` and `  S rcfd-runner\n` (asking for the name shown); dead for `Z    rcfd-runner`, `Z+   rcfd-runner`, `X    rcfd-runner`, `Z    rcfd-runner <defunct>`, `S    other`, `S`, `` and a blank line | decision 3 | the pre-change rule (the trimmed output ends with the name) reads `Z    rcfd-runner` as alive |
| Z1 | A runner started by this test process with `ensure_runner` is killed (`sys::kill`, pid from `runner.json`) | (a) within **2.0 s** of the kill, `runner_running()` and `pid_alive(pid, "rcfd-runner")` both read false; (b) then `ensure_runner()` returns `Ok`, and `runner.json` names another pid | ADR 0137's S4 diagnosis, reduced to the probe | before the kill both read true. **Z1c:** Z1 on the code before the change, on Linux (Docker), must fail at (a); on Windows it passes before and after (no zombies) |
| S4L | `runner_w47b` S4a and S4b on Linux (Docker) after the change | their ADR 0123 limits, unchanged | ADR 0137 | **S4Lc:** the same two on the code before the change fail, as ADR 0137 recorded |
| K1 | C3 after the change | Linux (Docker): `cargo test` lists `c3_boundedness` as ignored with the ADR 0143 reason; with `--ignored` it still fails, at the bound ADR 0137 recorded (limit intact). Windows: it runs and passes, as before | decision 4 | the Linux `--ignored` run is the control: the limit still catches the 1 ulp |
| K2 | P1 after the change | the same as K1 for `p1_pipeline_cylinder_in_box`; the Linux `--ignored` run fails at `check_mesh` | decision 4 | as K1 |
| F15' | F15's round trip through `detached_command` (decision 5) | 9 of 9 arguments parse back exactly (exit 1, stderr starts with `rcfd-runner: <arg>: `) | ADR 0128 finding 15, for the route now used | (the nine arguments include the six that the pre-fix hand-built line broke) |
| F17' | F17 without the PowerShell site (decision 5) | as registered in ADR 0128 | | as registered |
| RR | Re-runs on Windows of the earlier tests that start runners or test this code: the whole `rcfd-runner` crate (S1-S11, F-tests, contract tests, unit tests); the whole `rcfd-studio` crate (R9, G1b and the rest; known misses stay ignored); `rcfd-mcp` `project_runs_w48f` and `project_w48f`; `rcfd-fv` `cavitation`; `rcfd-mesh` `tetmesh`. On Linux (Docker), the whole `rcfd-runner` crate | pass unchanged; the known-miss tests stay ignored | regression | (their own) |

### Set-up details

* **N1** (`crates/rcfd-studio/tests/native_start_w48i.rs`):
  * Set-up: `g4::project(tag, "live.toml", &g4::cavity(2000), false)` (no runner started), `g4::app_on` (the Run button
    searches only the test build's runner, as `RCFD_RUNNER_BIN` would), and the assertion that the lock probe reads false
    before the click. Then `g4::click(.., "Run")`.
  * Each loop: one headless frame, then a read of the run's `status.json` (the run id is the only folder under `runs/`),
    then a sleep of 2 ms.
  * Reported, not limited: the times from the click to the run folder existing, to `.rcfd/runner.json` existing (the
    runner holds the lock), to `check.json` existing (the pre-launch check started), and to the model showing the run running.
  * Release only: in a debug build the test is ignored (`cfg_attr(debug_assertions, ignore)`, as `render_cli_w39a`), because
    the limit is a release timing.
  * The drop guard of `RunnerProject` shuts the runner down and kills only the run's solver.
  * N1c runs the same file, with `--exact` for N1, at the commit that adds N1, N2 and Z1. That commit follows this
    registration and changes no non-test code; the code change comes in a later commit, after N1c, Z1c and S4Lc have run.
* **N2** (same file):
  * The parent writes the project (`g4::project`, no runner) and asserts the lock probe reads false.
  * It starts the test binary with `--exact n2_child_entry --nocapture --test-threads 1`, environment `RCFD_W48I_N2_DIR` and
    `RCFD_W48I_N2_RUNNER`, stdout and stderr to files, stdin null, and waits up to 120 s for the child's exit (killed after).
  * The child: `Studio::in_memory()`, `runner_exe` and `quick.search = Search::explicit(runner)`, `open_project`, drained
    until the run worker ticked, `open_case("cases/live.toml")`, drained until the editor holds it, then
    `quick_run::run_selected_case`. It drains until `quick.run` holds the id, `quick.starting` is `None`, `quick.runner_error`
    is `None` and `status.json` says `running`. It prints `N2 RUN <id>`, flushes, and calls `std::process::exit(0)`.
  * `n2_child_entry` does nothing without its environment, a documented child entry point (test-process.md section 6.3 item 4).
  * The parent then reads the id from the child's stdout file and checks the limits. It waits for the run with
    `Client::wait` (300 s) and shuts the runner down; its drop guard kills only that run's solver.
* **N3, N4 (a), (c)** (`crates/rcfd-runner/tests/native_start_w48i.rs`):
  * Projects as in `runner_w47b` (`base_config(1)`: `max_runs` 1, `poll_ms` 100, `adopt_probe_ms` 500).
  * N3's threads clone one `Client` and wait at a `Barrier(2)` right before `ensure_runner_pid`.
  * "Alive as `rcfd-runner`" is `sys::pid_alive(pid, "rcfd-runner")`.
  * Every runner started is shut down with `Client::shutdown(10 s)`; a recorded pid still alive after that is killed.
* **N4 (b), Z0** are unit tests in `crates/rcfd-runner/src/sys.rs`.
* **Z1** (`crates/rcfd-runner/tests/zombie_w48i.rs`):
  * Its own project; it uses only APIs that exist before the change, so Z1c runs the same file at the commit that adds it
    (before the code change).
  * (a) polls every 10 ms. The kill time is taken after `sys::kill` returns.
* **S4L, Z1c, S4Lc, K1, K2 on Linux:** in the container, `cargo build --release --locked -p rcfd-cli`, then
  `cargo test --release --locked -p rcfd-runner --test zombie_w48i --test runner_w47b -- --test-threads 1 z1 s4a s4b`.
  After the change also: the whole `rcfd-runner` crate, `cargo test --release --locked -p rcfd-fv --test cavitation` and
  `-p rcfd-mesh --test tetmesh` (listing the ignored tests), and each of `c3_boundedness`, `p1_pipeline_cylinder_in_box`
  with `--ignored --exact`.

## What I already knew when registering (rule d)

* The code reading in Context, and the std 1.99 source for `inherit_handles`, `setsid` and `process_group`.
* The old route's timings are ADR 0141's and ADR 0133's (Context). No timing of the direct start alone has been taken on this
  host, by me or by an ADR I read; that ADR 0141 says which route started the runner "is not recorded".
* ADR 0137's Linux results (S4a, S4b, C3 and P1 fail; the S4 probe shows a zombie with the test process as parent).
* Docker and the `rust:1.99-bookworm` image are present on this host.
* Nothing of this job was built or run before this registration. The tests are written after it, and the code change after the
  N1c, Z1c and S4Lc runs.

## Addendum: results against the registered limits (2026-10-28)

Commits: registration 43f1f89; tests N1, N2 and Z1 48ef903 (no non-test code changed: the "before" runs); the change
87605aa (code, the other tests, the notes). Windows: this host (16 threads, 93.6 GB, other agents building at the same
time), release, `--test-threads 1`, output to files. Linux: Docker `rust:1.99-bookworm` (glibc 2.36, rustc 1.99.1),
`--init --cpuset-cpus 0-3 --memory 16g`, `git archive` of 48ef903 (before) and 87605aa (after), extracted with `tar -m`.
No new third-party crate, no new workspace edge, `Cargo.lock` unchanged, no numerics changed.

| Id | Limit | Measured | Result |
|---|---|---|---|
| N1 | solver started within 1.5 s of the click; finished, step 2000 | **0.389 s** (run folder 0.152 s, `runner.json` 0.272 s, `check.json` 0.272 s, `status.json` running with pid 0.389 s, model shows it running 0.501 s); finished, step 2000. Again in the crate re-run: 0.247 s | pass |
| N1c | the code before the change: above 1.5 s | **5.341 s** (run folder 0.082 s, `runner.json` 5.223 s: the PowerShell route did not take the lock in its 5 s, the direct start followed; model 5.509 s); finished, step 2000 | fails as required |
| N2 | child exits 0; runner alive after its exit, the run not terminal; finished; runner alive after the run; probe false before and after the shutdown | child exit 0 after 0.557 s; after it: runner true, run `running`; finished, step 15 000; runner true after the run; probe false before the start and after the shutdown. Crate re-run: the same, child exit after 0.311 s | pass |
| N3 | both `Ok`, both started; 1.5 s later one alive, `runner.json`'s; one "started" line; lock probe true | Windows: `Ok(Some(41664))`, `Ok(Some(53568))`; alive [41664] = `runner.json`; 1 line; probe true. Linux: pass | pass |
| N3 control | two projects: both alive, each its `runner.json` | Windows: [10556, 48252] both alive, each its project's `runner.json`. Linux: pass | as required |
| N4 | (a) started pid = `runner.json`'s, alive; (b) program = the runner, flags; (c) no shell names | Windows: (a) 62276 = `runner.json`, alive (also asserted, not registered: a second call returns `Ok(None)`); (b) `C:\x y\rcfd-runner.exe`, arguments `serve --project p`, flags 0x8000208; (c) 35 source files, none. Linux: (a), (b) (flags 0), (c) pass | pass |
| N4 (c) control | the pre-change launcher line is found, not after `#[cfg(test)]` | found at its line 2; ignored after `#[cfg(test)]` | as required |
| Z0 | the registered verdicts | 7 outputs read alive, 8 dead, 0 wrong (Windows and Linux) | pass |
| Z0 control | the old rule reads the zombie alive | it does | as required |
| Z1 | (a) both probes false within 2.0 s; (b) `Ok`, another pid | Windows before: 0.277 s, (b) met; Windows after: 0.199 s, (b) met; Linux after: **0.0025 s**, (b) met | pass |
| Z1c | Linux, before the change: fails at (a) | (true, true) still 10.011 s after the kill | fails as required |
| S4L | ADR 0123's S4a and S4b limits, Linux, after | S4a met; S4b failed 0.504 s after the start (limit 2.5 s) | pass |
| S4Lc | Linux, before the change: S4a and S4b fail | both fail at their `wait_until(5 s, !runner_running())` (`runner_w47b.rs` 351 and 379), as ADR 0137 recorded; the same in both "before" runs (deviation 2) | fail as required |
| K1 | Linux: listed ignored; `--ignored` fails at the bound; Windows: runs and passes | Linux: `ignored, known miss on Linux, ADR 0143 ...`; with `--ignored`: `0.000000000001 0.0001 1: 0.00009999999999999998 < 0.0001` (`cavitation.rs` 87, ADR 0137's value). Windows: `cavitation` 4 passed | as registered |
| K2 | the same for P1 | Linux: listed ignored; with `--ignored` it fails at `assert!(check_mesh(&m).is_ok())` (`tetmesh.rs` 233). Windows: `tetmesh` 10 passed, 1 ignored (`t5_torus_known_miss`, as before) | as registered |
| F15' | 9 of 9 parse back | 9 of 9 (Windows) | pass |
| F17' | as ADR 0128 | `tasklist` 0x8000000, `taskkill` 0x8000000, runner start 0x8000208, `rcfd --version` 0x8000000; control holds | pass |
| RR | earlier tests pass unchanged | see below | pass, except three failures that are on main already (below) |

RR in detail:
* `rcfd-runner`, Windows: 34 passed, 1 ignored (S5). S1 max 2 by status (125 samples) and by OS, control 3 and 3; S2
  [2, 0, 1, 3] and [0, 1, 2, 3]; S3 0.068 s; S4b 0.866 s; S4c 0.103 s; S6 paused after 0.049 s, 0 steps held, bitwise;
  S7 0.137 s; S8 one runner, loser exit 3; S9, S10, S11, S5a, the F-tests and the contract tests pass. Linux: 31 passed,
  1 ignored (S5; the F15 tests and F17 are Windows-only).
* `rcfd-mcp` `project_runs_w48f` 3 passed, `project_w48f` 10 passed (they start runs through `ensure_runner`).
* `rcfd-studio`, Windows, the whole crate: 153 passed, **3 failed**, 3 ignored (the known misses G1, B9 tutorials, R6).
  R9 passed (0.92 s for its binary). The three failures are G1b, the G1 control and the G4 control of ADR 0141. They are
  not from this change: run on the code before the change (a separate `git archive` of 48ef903 with its own target folder),
  all three fail in the same way:
  * G1b, limit (g): `renders 10 uploads 2` before (running 5.330 s after the click), `renders 9 uploads 2` after.
  * G1 control: "frame within 5 s of the first snapshot true (0.000 s)" before, 0.001 s after.
  * G4 control: `assertion failed: !vp.contains(&reason)` before and after.
  * Read from the code, not run separately: ADR 0138's case preview (`Viewport::show_surface`, merged in e11c656) shows the
    selected case's mesh in the viewport. That is a frame without a run (the G1 control) and a first mesh upload before the
    run's (G1b (g)). ADR 0140's empty states explain a failed run in the viewport ("failed before it stored its first
    snapshot"), which 48G4's `no_explain` switch does not turn off (the G4 control). ADR 0141's checks were registered and
    run before those merges. Question 3.
* `cargo fmt -p rcfd-runner -p rcfd-studio -p rcfd-fv -p rcfd-mesh -- --check` clean (Windows; Linux for runner, fv,
  mesh). `cargo clippy --workspace --all-targets -- -D warnings` clean on Windows; `cargo clippy -p rcfd-runner
  --all-targets -- -D warnings` clean on Linux (the Unix code paths).
* `rcfd-cli` `workspace_graph`: 3 passed (no edge changed).
* Leftover processes after all runs: none (`ps` in the containers; `tasklist` for `rcfd-runner.exe` and `rcfd.exe` on
  Windows).

### Deviations and notes

1. The first Windows build of the "before" code failed at link (`LNK1181: cannot open input file 'dbghelp.lib'`). The
   Visual Studio Installer was modifying the installation from 15:13 to 15:27, and the Windows SDK's `Lib` folder was
   missing in that time. Nothing ran on that build; it was built again after the installer had finished.
2. The first Linux "before" run stopped after `runner_w47b` failed (cargo stops at the first failing test binary without
   `--no-fail-fast`), so Z1c did not run in it. The script got `--no-fail-fast`, and the same archive ran again: S4a and S4b
   failed in the same way both times, and Z1c failed at (a).
3. Z1 and N3/N4 read `runner.json` until it exists or changes (at most 5 s): the runner writes it just after it takes the
   lock, which is when `ensure_runner` may return. Written into the tests before any run; the limits are unchanged.
4. After their "before" runs, N1, N2 and Z1 were reformatted by `cargo fmt` (line breaks only, `git diff -w` shows none
   else).
5. The coordinator's correction during the job: memory is not short (36 GB free at the time), so jobs ran side by side
   where they did not touch a timing check: N1's registered run ran with no other job of mine; the Linux "after" run
   overlapped the Windows `rcfd-runner` crate run and the `rcfd-studio` crate re-run; both have generous time limits
   (above).
6. Not in the table, from N1's breakdown: of the 0.389 s, 0.15 s pass before the run folder exists (the program search,
   `rcfd --help` and `rcfd --version` on the Studio's control thread), 0.12 s until the runner holds the lock, 0.12 s for the
   pre-launch check and the launch.

### Not run

* A real window by hand: the first Run of a fresh Studio session, closing the Studio window while a run runs (Task
  Manager), F5 pressed twice quickly.
* Linux CI (`ci.yml` unit tests and `gate.yml` on `ubuntu-latest`, Ubuntu 24.04): Docker bookworm stood in; no push.
* The `rcfd-studio` tests on Linux (N1 and N2 have run only on Windows), macOS, debug builds of the new tests, the full gate
  (`scripts/gate_test.py`) and `cargo deny`.

### Questions for the maintainer

1. **Inherited handles on Windows (S5).** The direct start hands the runner every inheritable handle of its starter.
   Options:
   * (a) Accept and document it. On Windows, do not capture the output of `rcfd-runner start` or `ensure` through a pipe:
     redirect it to a file, or use `start --no-runner` and then `ensure`.
   * (b) Keep an opt-in ShellExecute route for the CLI client only. It would be PowerShell again, which fails on this host.
   * (c) `CommandExt::inherit_handles(false)` once it is stable (issue 146407): no unsafe code.
   * (d) Allow one reviewed unsafe function (clear the inherit flag of the std handles, or a handle list); this conflicts
     with `unsafe_code = "forbid"`.

   For `rcfd-mcp`, the effect on Claude Code of its stdio pipes staying open in the runner after the server exits is not
   verified. **Recommendation:** (a) now, (c) when it is stable.
2. **Unix "setsid-style".** Options: (a) keep `process_group(0)` with null stdio; (b) `setsid` once
   `CommandExt::setsid` is stable (issue 105376); (c) start through the `setsid` program (util-linux; an extra program in the
   start path, not on macOS). **Recommendation:** (a), then (b).
3. **Three ADR 0141 checks fail on main e11c656** (G1b (g), the G1 control, the G4 control; RR above). Options:
   * (a) Their owners register amendments: count uploads and frames from the run's selection on, and exclude ADR 0140's
     empty-state text from the G4 control.
   * (b) Make them known misses with their limits intact (the house rule).
   * (c) Change the code so that the preview and ADR 0140's text follow 48G4's switches.

   **Recommendation:** (a). The failures come from newer features that the older checks count, not from a regression of
   the run display. Not done here, because these are another ADR's checks.
4. **C3 and P1 are ignored on `not(windows)`.** The other option is `target_os = "linux"`, under which other platforms
   would run them without evidence. **Recommendation:** keep `not(windows)` (no other platform is built).
5. **Tests removed with the PowerShell code:** the base64 vectors and F15's `start_script` text assertion. F15's round trip
   now runs through `detached_command`, and F17 lost its PowerShell site. **Recommendation:** accept.
6. **Early exit.** Now that the client owns the started process, `ensure_runner` could report a runner that exits before it
   takes the lock (for example on a bad budgets file) at once, with its exit code, instead of after 10 s. The Studio
   checks the budgets first; `rcfd-mcp` and the CLI do not. **Recommendation:** yes, as a small follow-up with a registered
   check.
7. **Linux CI.** Run the Full gate on `ubuntu-latest` and the CI unit tests after the merge, since Docker bookworm stood in
   for Ubuntu 24.04. **Recommendation:** yes.

## Files

This ADR; `crates/rcfd-runner/src/{sys,client}.rs`; tests `crates/rcfd-runner/tests/{native_start_w48i,zombie_w48i}.rs` (new),
`crates/rcfd-runner/tests/{runner_wfix,runner_w47b}.rs` (F15, S5 text), `crates/rcfd-studio/tests/native_start_w48i.rs` (new),
the doc comments of `crates/rcfd-studio/tests/{runner_start_r9,oneclick_e2e_g4}.rs`, `crates/rcfd-fv/tests/cavitation.rs`,
`crates/rcfd-mesh/tests/tetmesh.rs`; notes in ADRs 0123, 0137 and 0141.
