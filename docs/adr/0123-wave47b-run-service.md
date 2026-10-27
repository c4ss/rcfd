# ADR 0123: Wave 47B, the run service (`rcfd-runner`, Studio layer L2)

Status: **ACCEPTED 2026-10-26** (limits registered before the registered runs). Written at main 3a379a1 on branch `w47b`, target
`<target dir>`. House rules of ADRs 0110-0117: limits fixed before the registered runs; a missed limit becomes an
ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment before the corrected run; defaults
are bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded. Parents: [the Studio plan](../ui-restart-plan.md)
(layer L2, the section-8 folder contract), [run control](../run-control.md) (F-120, F-121). ADR 0122 (`rcfd-project`, wave 47A) is
built in parallel; this ADR codes against the section-8 file contract with local types (see "Switching to rcfd-project").

## Context (checked against the code at 3a379a1)

* Runs start today through `rcfd run` (`crates/rcfd-cli/src/main.rs` `cmd_run`) and `runctl::drive` (`crates/rcfd-api/src/runctl.rs`):
  stop file (exit 7, clean stop, results and checkpoint written), wall-clock limit, the `rcfd-status` JSON-lines stream
  (`--status-file`, `--status-every`), checkpoints and `--continue-from`. The service (`service.rs`) pauses in process with an atomic
  flag polled between steps (`run.pause`). The CLI has no pause.
* Checkpoint and continue covers steady SIMPLE/SIMPLEC without turbulence and transient laminar PISO/PIMPLE without energy, scalars or
  statistics (`Simulation::supports_restart`). A checkpoint-and-stop pause would therefore not work for turbulent or MRF runs, which
  include the fan runs.
* `Case::load` picks the format from the file extension (`.json`, else TOML) and resolves relative paths against the case file's
  folder. A snapshot named `case.snapshot` inside `runs/<id>/` would lose both.
* Windows finding from the smoke: a std spawn passes the child every inheritable handle of the parent. A client started by a shell
  that reads its output (`$id = rcfd-runner start ...`) hands the shell's pipe to the runner, and the shell then waits until the runner
  exits. `Stdio::null()` does not prevent it, and neither does `cmd /c start` (both tried). PowerShell `Start-Process` without
  redirection uses ShellExecute, which passes no handles (tried: returned in 0.64 s).
* `crates/rcfd-cli/tests/workspace_graph.rs` `layer()` panics on an unknown crate only when that crate has an rcfd dependency.
  `rcfd-runner` has none (std, serde, serde_json), so no existing test changes.

## What I already knew when registering (rules c, d; disclosed)

* The code and the tests below were written before this registration so that the rule-d smoke could run. They are committed after
  this ADR, unchanged except as stated in amendments.
* Smoke, debug build, one run of every test: all passed except S4b, whose helper counted the runs of a two-run project as one (a
  test-code defect). It was fixed and re-run (pass) before registration. Debug smoke values: S3 detection 0.156 s; S4b 0.277 s;
  S4c shutdown 0.125 s; S6 pause acknowledged in 0.144 s, 0 steps in 1 s paused, bitwise equal; S7 0.128 s; S8 one runner, loser exit 3;
  S1 max 2 by status and by OS, negative control 3. No release-build test values were seen before registration.
* Solver timings (12 x 12 cavity, SIMPLEC, tolerance 1e-14 so the iteration cap ends it): debug about 2.2 ms per step with
  `--status-every 1`; release 0.42 ms per step with it (5000 steps 2.49 s). Test run lengths are set from these:
  `long_steps()` = 2500 (debug) or 8000 (release), short runs 60 steps.
* Memory (release, peak working set): 144 cells 9.7-19.1 MB (growing with the step count); 10 000 cells 33.9 MB; 40 000 cells 110.2 MB;
  160 000 cells 422.3 MB, about 2.6 KB per cell. Only the 2-D laminar cavity was measured.

## Decisions

1. **Transport: plain files, no socket and no network port.**
   * The project folder is the queue. A client builds `runs/.tmp-<id>/` (run.json, case.snapshot, status `queued`, events) and renames
     it to `runs/<id>/`, so a run appears whole or not at all. Requests are files in the run folder: `pause` and `cancel`, plus
     `.rcfd/shutdown` for the runner.
   * Reasons:
     * std only. Named pipes need FFI on Windows and the workspace forbids unsafe code; Unix sockets are Unix-only.
     * There is no inbox to lose: a runner crash cannot lose or duplicate a submission.
     * Requests work while no runner is alive, because the solver watches them itself.
     * The same files are what the GUI and Claude Code read.
   * Cost: a 100 ms poll (`poll_ms`).
2. **Process model.**
   * `rcfd-runner serve --project <dir>`, one per project, holding `.rcfd/runner.lock`.
     * Windows: the lock file is opened without sharing; the OS releases it when the holder dies.
     * Unix: `create_new` with the pid inside; a stale lock, whose pid is not a live `rcfd-runner`, is taken over under a second
       `create_new` guard.
   * Each run is the child `rcfd run <runs/id/case.snapshot> --case-dir <case folder> --out runs/<id>/output --status-file
     runs/<id>/solver-status.jsonl --status-every 1 --quiet --threads n --stop-file runs/<id>/cancel --pause-file runs/<id>/pause`.
     Its stdout and stderr go to `log.txt`, never to a pipe, so it outlives the runner.
   * The runner turns the stream into `residuals.jsonl`, `events.jsonl`, `status.json` and `metrics.json`.
   * Clients start the runner detached. Windows uses `Start-Process` through `powershell -EncodedCommand`, for the handle reason in
     Context. Unix uses `process_group(0)` with null stdio.
3. **Pause and resume: a pause file, held at a step boundary in memory, on both platforms.** This deviates from the task's suggestion
   of a signal on Unix. Reasons:
   * One mechanism, and it is tested on the gate host.
   * It stops between steps, never mid-write.
   * It needs no FFI.
   * It works for every case type (checkpoint-and-stop does not; see Context).

   **CLI change (opt-in):** `rcfd run --pause-file f`. The `should_stop` closure of `cmd_run`, which `drive` polls between steps
   without holding the simulation lock, writes `f.ack` (the iteration count) and sleeps in 20 ms ticks while `f` exists. The stop file
   ends the hold. Without the flag the closure is the old `|| false`.

   The runner reports `paused` only after the ack exists, and drains the stream again first. A paused run keeps its slot, threads and
   memory.
4. **Cancel.** A queued run is cancelled at once. A running or held one gets the stop file, so the solver exits 7 after writing its
   results. If it has not stopped after `cancel_grace_ms` (10 s), it is killed.
5. **Crash recovery.** The first scan of a new runner reads every run:
   * `queued`, or `paused` without a pid: pending.
   * `running` or `paused` with a pid: adopted and never relaunched. Its stream is read from the start, skipping residual steps and
     solver events (`solver_seq`) already forwarded.
   * Adopted processes are probed every `adopt_probe_ms` (500 ms in the tests, 1 s by default) by pid and image name (`tasklist` or
     `ps`).
   * A dead process without a finish event becomes `failed`, with a reason giving the pid, the probe, the last step, the last log line
     and the log path. A finish event read after the death decides the state as usual.
6. **Budgets and their defaults (conservative, stated).**

   | key (`.rcfd/runner-config.json`) | default | reason |
   | --- | --- | --- |
   | `max_runs` | 2 | parallel runs on a workstation |
   | `cpu_threads` | half the logical cores, at least 1 | leaves the GUI and the OS responsive |
   | `mem_budget_mb` | 4096 | std cannot read the installed memory, so the default is small |
   | `mem_base_mb` | 64 | covers the 9.7-19 MB measured on small meshes |
   | `mem_kb_per_cell` | 4 | 1.5 times the 2.6 KB per cell measured |
   | `poll_ms` | 100 | — |
   | `cancel_grace_ms` | 10 000 | — |
   | `adopt_probe_ms` | 1000 | — |

   * A run's memory is its declared `settings.mem_mb`, or else `mem_base_mb + cells x mem_kb_per_cell`. The cell count comes from a
     pre-launch `rcfd run --check-only --status-json` (one check at a time, outside the budget), which also turns an invalid case into a
     located failure.
   * A run that cannot fit even alone (threads > `cpu_threads`, memory > `mem_budget_mb`) fails with a reason naming the value, the
     key and the config file.
   * A run that fits alone waits.
7. **Queue order.** Priority descending, then `created` (UTC with nanoseconds), then id. The queue is strict: a waiting head is not
   overtaken, and nothing launches while a queued run still lacks its estimate.
8. **Clean shutdown** (`.rcfd/shutdown`). The runner:
   * stops launching;
   * kills only its pre-launch checks;
   * writes a `runner_shutdown` event for each running solver, which keeps running;
   * releases the lock and exits 0.

   The next runner adopts the running solvers. Closing the app therefore never stops a run.
9. **Files added to the section-8 contract (additive, for the lead).** In `runs/<id>/`: `pause`, `pause.ack`, `cancel`,
   `solver-status.jsonl`, `output/` (the solver's `--out`) and `check.json`. In `status.json`: `reason` and `exit_code`. In `run.json`:
   `case_source`, `case_dir` and `settings` (`priority`, `threads`, `mem_mb`, `set`). In `.rcfd/`: `runner.lock`, `runner.json`,
   `runner.log`, `runner-config.json` and `shutdown`. The `residuals.jsonl` lines carry `monitors: {}`, because the status stream has no
   monitors. `mesh_hash` is null, because the runner does not resolve meshes.
10. **Second CLI flag (opt-in):** `rcfd run --case-dir d` reads the case from its text (JSON when it starts with `{`, else TOML) and
    resolves relative paths against `d`. Both flags are in `RUN_VALUE_OPTS` and in [run control](../run-control.md).
11. **Client library** (`client::Client`): `start` (submit, then `ensure_runner`), `submit`, `list`, `status`, `pause`, `resume`,
    `cancel`, `wait`, `shutdown`, `ensure_runner`, `runner_running`. The binary exposes the same as subcommands. Run ids from users are
    validated (`r-` plus `[A-Za-z0-9-]`) before they become path components.

## Switching to rcfd-project (what must change at merge)

The runner and the client reach records only through the trait `store::RunStore` (implemented by `store::FsStore`) and the types of
`records`. To switch:

1. Replace `crates/rcfd-runner/src/records.rs` with re-exports from `rcfd-project`: `RunState` (six states), `RunStatus` (`state`,
   `step`, `time`, `pid`, `updated`, plus `reason` and `exit_code`), `RunRecord` (`id`, `created`, `case_dir`, `settings`, plus
   `case_source`, `case_snapshot_hash`, `mesh_hash`, `solver_version`), `RunSettings`, `new_run_id`, `now_iso`, `atomic_write`,
   `append_line`, `retry`. Any field missing in `rcfd-project` goes to the lead as an additive change (list in decision 9).
2. Re-implement the nine required `RunStore` methods of `FsStore` (`project`, `list_runs`, `create_run`, `read_record`, `read_status`, `write_status`, `append_event`, `append_residual`, `write_metrics`; `run_dir` has a default) on `rcfd-project`'s helpers. `create_run` takes `rcfd-project`'s snapshot
   hash instead of `fnv1a64`.
3. Replace `client::check_id` with `rcfd-project`'s run-id validator.
4. Add `rcfd-project` to `crates/rcfd-runner/Cargo.toml`, both crates to `layer()` in `crates/rcfd-cli/tests/workspace_graph.rs` (it
   panics on a crate with an rcfd dependency that it does not know), and the edge to `docs/architecture.md`.
5. `runner.rs` reads two contract files directly when it adopts a run: the last line of `residuals.jsonl` and the `solver_seq` and
   `launched` lines of `events.jsonl`. If `rcfd-project` offers readers, use them.

Nothing else touches the run records.

## Registered checks (`crates/rcfd-runner/tests/runner_w47b.rs`; release gate)

Each test has its own temp project `rcfd_runner_w47b_<pid>_<name>`, its own runner and its own solvers. It kills only pids it recorded.

| id | check | limit | reason for the limit | negative control |
| --- | --- | --- | --- | --- |
| S1 | 5 runs of `long_steps()/4` steps, `max_runs` 2 | `status.json`, sampled every 20 ms over the whole queue: at most 2 in running or paused. OS process list (pids launched by this project), every ~200 ms: at most 2 live solvers. Saturation exactly 2 | the contract | `max_runs` 3 on 3 runs: the samplers see 3 by status and more than 2 by OS |
| S2 | 4 short runs with priorities [0, 0, 5, 0], `max_runs` 1 | `launch_seq` order [2, 0, 1, 3] | the contract | all priorities 0 give [0, 1, 2, 3] |
| S3 | kill a running solver (pid from status) | failed within 2.0 s of the kill. The reason names the pid, "without a finish event", the last step and `log.txt` | 20 poll periods: margin for a loaded host running two release test suites | a short run of the same project ends `finished` |
| S4a | kill the runner while run A runs (`max_runs` 1, A plus 2 queued); start a new one | A's solver survives the kill and is adopted. All 3 finish. Each run has exactly one `launched` event. Residual steps are exactly 1..n with no gap or duplicate. 3 run folders | the contract | the counts would fail on a duplicate or a loss |
| S4b | kill the runner, then A's solver; start a runner | A failed within `adopt_probe_ms` + 2.0 s = 2.5 s after `ensure_runner` returns. The reason names the probe and the last step. B finishes; one launch each | first probe at once, plus margin | the reason distinguishes it from S3 (exit code) |
| S4c | clean shutdown with A running and B queued | runner gone (lock released) within 2.0 s. A's solver alive; B still queued; `runner_shutdown` event. After a restart both finish, one launch each | 20 poll periods | — |
| S5 | the client is a separate process (`rcfd-runner start`, stdout piped) that exits | the client's stdout reaches EOF within 10 s of its exit (no inherited pipe). The runner is alive after the client exits and after the run finishes | the Windows handle finding | the lock probe reads false before the first start and after the shutdown |
| S6 | pause a run at step >= m/5 of m = `long_steps()`; resume after 1.0 s; an uninterrupted reference run of the same case | `paused` within 2.0 s. 0 new residual steps and the same status step during the 1.0 s held. Residual steps exactly 1..m. `output/solution.vtu` byte-equal to the reference | the solver is deterministic: `crates/rcfd-cli/tests/run_control.rs` shows byte-equal solutions for 1 and 4 threads and for continue-from, so bitwise is registered | the run moved more than 0 steps in 300 ms before the pause; a 60-step run's solution differs from the reference |
| S7 | cancel a running run, a held one and a queued held one | all three `cancelled` within 5.0 s. The folder check passes: status, events, metrics and residuals parse and agree, no `.tmp-` or request files remain, pid null. The solvers are dead; the two that ran wrote `output/solution.vtu`; the queued one never launched | grace not reached: a clean stop takes one step | the folder check reports a left-over temp file (exactly 1 problem) and a truncated `status.json` |
| S8 | two `serve` on one project at once | after 1.5 s exactly one alive; the other exited 3 | the contract | a `serve` on a second project is alive too |
| S9 | budgets: `cpu_threads` 2, `mem_budget_mb` 100 | declared 200 MB fails, naming "memory budget", "200 MB", `mem_budget_mb = 100`, `settings.mem_mb` and the config path. The 200 x 200 mesh is estimated (40 000 cells) and fails, pointing to the estimate event. 4 threads fail with "CPU budget". `physics.nu = -1` fails "pre-launch check failed" naming `physics.nu`. None launched. Two 60 MB runs never run together (max 1 by status) | the contract | a run under the budget (and the pair) finishes |
| S10 | a JSON case and an invalid TOML case | JSON runs to 30 steps through `--case-dir`; `run.json` holds the snapshot's fnv1a64. The invalid one fails "pre-launch check failed" naming `mesh.generator` | — | the invalid case |

Unit tests: the civil date of day 0, 2000-02-29 and 2026-10-26; the run id format; an atomic write leaves no temporary; base64 RFC
4648 vectors.

## Limits

* The Unix code paths (lock takeover, `ps` probe, `process_group`, `kill -9`) compile only on Unix and are not run on the Windows gate
  host.
* Liveness by pid and image name: a recycled pid of another `rcfd` process reads as alive, and the run then stays `running` until that
  process ends.
* The memory estimate is calibrated on the 2-D laminar cavity only. 3-D and turbulence use more per cell. Measured memory is not
  enforced: the budget is an admission rule, not a cap.
* A held run keeps memory, its slot and its threads. `--max-elapsed` and the solver's `wall_s` count held time. Chains
  (`chain.sequence`) ignore the pause file.
* No fsync: a power loss can lose the last status write, a killed process cannot. The runner does not exit when idle, and polls every
  100 ms.
* Windows detached start needs `powershell.exe`, which is present on every supported Windows; without it `ensure_runner` reports the
  spawn error.

## Addendum 2026-10-26: measured values (registered run, release, commit 8b83bb1)

`cargo test --release -p rcfd-runner` with `RUST_TEST_THREADS=3`, on the gate host, while two other agents' release workspace test
suites were running: 12 passed, 0 failed, 19.6 s for the process tests. All checks were met on the first registered run. There are no
known misses, no amendments, and no deviation from the registered set-ups.

| id | limit | measured |
| --- | --- | --- |
| S1 | at most 2 by status and by OS; saturation 2 | max 2 by status (111 samples), 2 by OS; negative control (`max_runs` 3): 3 by status, 3 by OS |
| S2 | [2, 0, 1, 3]; control [0, 1, 2, 3] | [2, 0, 1, 3]; [0, 1, 2, 3] |
| S3 | at most 2.0 s, located | 0.077 s; "solver process 38580 ended without a finish event (exit code 1); last step 489 ... (log: ...\log.txt)"; the short run finished |
| S4a | adopted, one launch each, steps 1..n, 3 folders | met (solver alive after the runner kill, `adopted` event, all 3 finished) |
| S4b | at most 2.5 s, located | 0.418 s; "... (found not running by the liveness probe); last step 891 ..." |
| S4c | at most 2.0 s; solver alive; queue intact | 0.102 s; met; both finished after the restart, one launch each |
| S5 | EOF within 10 s; runner outlives the client | met; the lock probe read false before the start and after the shutdown |
| S6 | paused within 2.0 s; 0 steps held; bitwise | 0.307 s at step 2504 of 8000; 0 steps in 1.0 s; `solution.vtu` byte-equal; 508 steps in the 300 ms before the pause (control); the 60-step run differs |
| S7 | at most 5.0 s, consistent folders | 0.122 s for all three; folder check clean; both solvers dead; the queued run never launched; the checker found the planted temp file (1 problem) and the truncated status |
| S8 | one runner, loser exit 3 | 1 alive, exit 3; second project alive |
| S9 | located refusals; pair max 1 | as registered: "memory budget: the run needs 200 MB (declared as settings.mem_mb in run.json) ... mem_budget_mb = 100 MB (the config path)"; estimated 221 MB from 40 000 cells; "CPU budget: ... threads = 4 ... cpu_threads = 2"; "pre-launch check failed (exit 3): ... --set physics.nu: invalid value at 'physics.nu': -1 outside [Some(0.0), None]"; pair max 1; the in-budget run finished |
| S10 | JSON case runs; invalid case located | 30 steps finished, hash in `run.json`; "pre-launch check failed (exit 3): ...case.snapshot: line 2: mesh.generator: expected one of [...]" |

Determinism: S6 shows a paused-and-resumed run byte-equal to the uninterrupted one, so the solver is deterministic for this case, as
the run-control tests showed for thread counts and continue-from.

Not done here, for the lead:
* the switch to `rcfd-project` (section "Switching to rcfd-project");
* an MCP tool front end (wave 48, L7);
* Unix runs of the Unix code paths;
* runtime memory measurement and enforcement;
* pause by checkpoint for runs that should give their memory back.

## Amendment 1 (2026-10-26, before its run): S11, relative paths through `--case-dir`

Written after the addendum above and before S11 was run as a registered test. It was prompted by a review: decision 10 claims that
relative paths resolve against the case folder, and no check covered it. What I knew: a manual smoke, debug build, on a converted
OpenFOAM mesh.
* `rcfd convert case.toml meshes/cavity` writes `meshes/cavity/polyMesh`.
* `cases/cavity.toml` with `mesh.source = "file"` and `mesh.file = "../meshes/cavity/polyMesh"` ran from `cases/`, and ran from
  `runs/x/case.snapshot` with `--case-dir cases`; both exited 5 after the 20-step cap.
* Without `--case-dir` it exited 3, naming `mesh.file` and the path resolved against `runs/x`. The first smoke attempt pointed at
  `meshes/cavity` (without `polyMesh`) and failed with a missing-file error. That was a set-up error of the smoke, not of the flag.

| id | check | limit | negative control |
| --- | --- | --- | --- |
| S11 | the project holds `meshes/cavity/polyMesh` (from `rcfd convert`) and `cases/file.toml` referencing it as `../meshes/cavity/polyMesh`; submitted through the client with 30 steps | `finished` with step 30 | the same `case.snapshot` run directly by `rcfd run` without `--case-dir` exits 3, and stderr names `mesh.file` |

S11 runs in the debug build now; the release gate runs it again.
Result of S11 (debug, 2026-10-26): the run finished at step 30. The negative control (`rcfd run <snapshot> --check-only` without
`--case-dir`) exited 3 with "mesh.file: <run folder>\../meshes/cavity/polyMesh: io error: The system cannot find the path specified".
Met.
## Note: differences from `rcfd-project` as committed on branch `w47a` (95e1e3c), read only, for the merge

* `RunRecord`: it has `case`, where this ADR has `case_source`, and `mesh`. It has no `case_dir`. Its `solver_version` is a `String`,
  not optional, and its `settings` is free JSON, which holds `RunSettings` as an object. `case_dir`, or an equivalent, must be added.
* `RunStatus` has no `reason` and no `exit_code`. They need adding, since every failure reason lives there.
* `events.jsonl`: `RunEvent` is `{t, kind}` with a closed `EventKind` (`state {from, to}`, `warning`, `stop`). This runner writes
  `{t, event, ...}` with the kinds `state`, `launched`, `adopted`, `estimate`, `solver`, `cancel_requested`, `killed` and
  `runner_shutdown`. At the merge, either `EventKind` gains an open variant (for example `other` with a JSON body) or the runner maps
  its events onto the three kinds and drops the rest. Until then, a strict `rcfd-project` reader fails on this runner's lines.
* `ResidualLine.residuals` is `BTreeMap<String, f64>`. The runner copies the solver's values, which are `null` when non-finite (a
  diverged step). A strict reader would reject that line. The runner should drop non-finite entries, or `rcfd-project` should accept
  `null`.
* Hash: `rcfd-project` has `hash_bytes`; this runner writes `fnv1a64:<hex>`. Run ids: `is_run_id` and `claim` replace `check_id` and
  `new_run_id`.
## Amendment 2 (2026-10-26, before the second gate run): the gate failure and the direct-start fallback

**Gate run 1 failed.** It ran on a fresh archive of 61d986d: fmt 0, clippy 0, then the test stage, 2077 passed / 12 failed / 656
ignored.
* The 12 failures are all of this crate's process tests except S8, which starts `serve` directly. Every one failed in
  `Client::ensure_runner`: "the runner did not take the lock within 10 s".
* Cause, found after the run: `powershell.exe` hung on the gate host. 15 instances were left, all waiting. A plain
  `powershell -NoProfile -NonInteractive -Command "Write-Output hi"` from Git Bash timed out after 30 s, both with and without the
  gate's `TEMP`. The harness's own PowerShell shells and WMI queries (`Get-CimInstance`) hung at the same time.
* During the registered run (addendum above) the same route started the runner in about 1 s. So the cause is a host fault that
  decision 2 depended on. It is a defect of the product, not of the test set-up: `ensure_runner` must not depend on PowerShell
  answering.

**Fix.**
* `ensure_runner` (Windows) waits 5 s for the runner started through PowerShell.
* If the lock is not held by then, it kills the PowerShell launcher and starts the runner directly (`sys::spawn_direct`:
  `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`, null stdio), then waits 10 s more.
* The direct route hands the runner the caller's inheritable handles, the limitation recorded in Context. So while PowerShell is
  unavailable, a shell that captures the client's output waits for the runner. S5 tests exactly that EOF, so S5 is expected to fail
  for as long as the host's PowerShell is hung.
* Limits, set-ups and every other check are unchanged. No check was tuned.

What I knew when writing this: the gate log, the hung `powershell.exe` processes and the failed manual `powershell` calls. No run of
the fixed code had been made. Gate run 2 runs on a fresh archive of the commit that carries this amendment and the fix. The
15 hung `powershell.exe` processes were not killed, because without WMI I could not confirm that all of them were mine.

## Amendment 3 (2026-10-26, before S5a's first run): S5 split

After the fallback, the debug run of every test passed except S5. S5 failed on its EOF part: "the client's stdout did not reach EOF:
the runner inherited the pipe". `powershell.exe` was still hung, so the fallback was taken, and amendment 2 predicted this.
* S5 checks two things: survival and EOF. The EOF part depends on the host's PowerShell.
* Following the house rule, S5 stays as the ignored `s5_runner_survives_the_client_known_miss`, its limit intact. It passed in the
  registered run while PowerShell worked.
* New S5a: the same survival check and negative control, with the client's stdout going to a file instead of a pipe.
  Limit: the client exits 0; the runner is alive after the client exits and after the run finishes; the lock probe reads false
  before the start and after the shutdown.

## Final gate (2026-10-26, fresh archive of 43618e3, rebased on main f0a8814)

| stage | result |
| --- | --- |
| fmt | 0 |
| workspace tests (release) | 2126 passed, 0 failed, 662 ignored |
| this crate (release) | 13 passed, 1 ignored (`s5_runner_survives_the_client_known_miss`) |
| `cargo clippy -p rcfd-runner -p rcfd-cli --all-targets -D warnings` | clean |
| `cargo clippy --workspace --all-targets -D warnings` | exit 101 |

The workspace clippy failure is not in this branch's files:
* `crates/rcfd-api/src/design/ledger_tests.rs:134` (`cloned_ref_to_slice_refs`) and `uq_tests.rs:295` (`type_complexity`).
* Both files came to main with the wave-46D merge (last commit 6bb6f51, ADR 0121), and the toolchain is now rustc 1.99.0.

Commit ids: the registered run of the addendum was on 8b83bb1, which became 7a561b7 after the rebases; the tree is unchanged.

After the gate: rebased on main df20467, which merges `rcfd-project` and `rcfd-claude` and fixes the two lints. Results there:
* `cargo fmt --check` passes.
* `cargo clippy --workspace --all-targets -D warnings` is clean.
* In debug, this crate (13 passed, 1 ignored) and the rcfd-cli tests `run_control`, `cli`, `docs_site` and `workspace_graph` pass.

`rcfd-runner` still does not depend on `rcfd-project`; the switch is the lead's merge step.
