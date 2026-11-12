# ADR 0229: Runner: solver identity by a held file, retried status and stream writes, and the shutdown-start race

Status: **REGISTERED 2026-11-11**, written before any runner code of this ADR and before any run of its checks.
**ACCEPTED 2026-11-11 by the maintainer**, in the session that wrote it ("create an ADR for the runner fixes then auto
accept and fix them"), so the code follows in the same branch without a separate review of this text. Results go in an
addendum; nothing above the addendum is changed after a run.

Number: 0229 (0225 and 0227 are taken on the branches `f300-transient-rans` and `f302-pair-diffusivity`). Branch
`studio-runner-semantic-fixes` from main 0d250c75. Worktree `E:/projects/cfd/w49i-work/semfix`.

The same branch also holds Studio changes that the maintainer asked for directly, without a registration (the case
editor's save, the Start run dialog's plan, the chat feed, the preview's "anyway" answer). They are listed at the end
for the reader of the branch and are not part of this registration.

## Why

A read of the runner, its client and the project store on 2026-11-11 (no code run) found four ways in which what the
runner writes can disagree with what happened, each with a concrete sequence that reaches it:

1. **The lock probe collides with itself.** `sys::lock_held` opens `.rcfd/runner.lock` without sharing
   (`share_mode(0)`) and reads error 32 as "a runner holds it". Two probes at the same instant are enough: the second
   gets error 32 from the first one's handle. The Studio's run worker probes every 250 ms, so a start that probes in
   that instant sees "runner alive", starts none, and the run stays queued with no error.
2. **A runner that is shutting down still looks alive.** The loop of `runner::serve` breaks on `.rcfd/shutdown` without
   a last scan and drops the lock only at the end. A run submitted in between is seen by no runner, while
   `Client::ensure_runner` returns at once because the lock is still held.
3. **An adopted solver is known by its pid and its executable name only.** After a runner crash, a pid that the
   system gave to another `rcfd` process (another project's solver, a command-line run) reads as alive for ever, and a
   cancel force-kills that process after `cancel_grace_ms` (`sys::kill(a.pid)`, no check). A failed `tasklist` call
   reads as "dead", and two in a row end a live run as failed.
4. **Writes that fail are only logged.** `set_state` logs a failed `status.json` write and goes on, so a finished run
   can show `running` until a runner restarts. `drain` moves its cursors (`offset`, `step`, `max_seq`) before the
   appends to `residuals.jsonl` and `events.jsonl`, so one failed append loses those lines for good.

## What was read before this text (disclosed)

- `crates/rcfd-runner/src/{runner,client,sys,store}.rs`, `crates/rcfd-project/src/{atomic,watch}.rs` and
  `crates/rcfd-workspace/src/runs.rs` at 0d250c75, in full or in the parts named above.
- Nothing was built or run. The sequences above are from reading; none was reproduced.
- The workspace forbids unsafe code (`unsafe_code = "forbid"`) and the runner uses std only, so a process handle or a
  process start time is not available to it. `rust-version` is 1.75, so `File::lock` (1.89) is not used.

## Decisions

**D1. The lock probe shares.** On Windows `sys::lock_held` opens the lock for reading with every sharing mode
(read, write, delete). The runner's own handle shares nothing, so the probe still fails with error 32 exactly while a
runner holds the lock, and two probes no longer exclude each other. Unix is unchanged (a pid in the file).

**D2. A start waits out a shutdown.** `Client::ensure_runner_pid`: when the lock is held and `.rcfd/shutdown` exists,
it waits (20 ms steps, at most 10 s) until the lock is free or the request file is gone; then it starts a runner if
the lock is free, returns as before if a runner holds the lock with no request pending, and returns a located error
if the lock is still held with the request pending after 10 s. `runner::serve` removes `.rcfd/shutdown` after it
has released the lock, not before, so that the request file is there for as long as the exiting runner holds the lock.

**D3. A launched solver holds a file for its lifetime (Windows).** At launch the runner opens
`<project>/.rcfd/alive/<run id>` without sharing and hands that handle to the solver as its standard input (it was the
null device; the solver does not read it). The runner keeps no copy. The file is then open, unshared, exactly as long as
the solver process lives, whatever its pid, and the system releases it when the process ends in any way.

- An adopted run whose file exists is alive when an open of that file fails with error 32, and gone when the open
  succeeds. `tasklist` is not called for it, so a failed process listing no longer ends a live run.
- The kill after the cancel grace is made only when the file is held: a live process keeps its pid, so the pid is
  then the solver's. When the file is not held, nothing is killed and the run is ended as "found not running".
- A run without the file (launched by an earlier runner, or on Unix) is probed and killed as before.
- The file lies under `.rcfd/` because the project watcher skips that folder: it reads small files to hash them, and
  an unshared file inside `runs/<id>/` would cost every poll a retried read. The runner removes the file when the run
  ends (best effort).

**D4. A failed status write is retried.** Every `status.json` write but the one of the launch goes through one method.
A write that fails is kept (the latest status per run) and written again on each tick until it succeeds; a later
write for the same run replaces the kept one. The failure is logged once, and so is the success after it. The launch
keeps its own rule (ADR 0128 finding 3: a failed `running` write stops the solver and fails the run).

**D5. `drain` moves a cursor only after its append.** The step and the time move after the residual lines of the
chunk were appended; `max_seq` moves after each solver event was appended, and the first failed event append stops
the forwarding of the chunk's later events; the offset moves only when every append of the chunk succeeded. A chunk
read again skips what was written, by step and by `seq`, as an adoption's re-read does today.

Unchanged: the run-folder contract (no file is added to `runs/<id>/`), every record format, the solver's command line
but for its standard input on Windows, the solver, the case schema, `baseline_hashes.rs` and the goldens.

## Checks, with their limits (fixed here)

Unit tests in `crates/rcfd-runner/src/` (`sys.rs`, `client.rs`, `runner.rs`); the runner tests use the existing
wrapper-store and scripted-probe set-up of F3, F4 and F6. Each is run once.

| Id | Set-up | Limit |
|---|---|---|
| P1 | Windows. A lock file nobody holds, with one shared read handle open on it (a probe in flight). | `lock_held` is false. Control: an unshared open of the same file at that moment fails with error 32 (the verdict before D1). |
| P2 | Windows. The same file held unshared (a runner). | `lock_held` is true. |
| S1 | The wait of D2 as a function of two scripted answers (lock held, request pending). | Held and no request: "alive" at the first look. Held with a request for 3 looks, then free: "start". Held, the request gone after 2 looks: "alive". Held with a request throughout, limit 100 ms: "stuck", after at least 100 ms. |
| S2 | Windows (the test holds the lock as a runner does; on Unix the lock names a live `rcfd-runner` pid, which a test is not). A project whose lock the test holds, `.rcfd/shutdown` present, a client whose runner program does not exist; the test frees the lock after 300 ms. | `ensure_runner` returns the error that names the missing runner program (it went on to start one), after at least 300 ms. Control: with no request file, the same call returns `Ok` at once (under 200 ms) while the lock is held. |
| I1 | Windows. A child process started with the liveness file as its standard input, living about 1 s. | The identity probe says "held" while it lives and "not held" within 5 s of its exit; "no file" for a path that does not exist. |
| I2 | An adopted run, cancel requested, `cancel_grace_ms = 0`, the pid probe scripted "alive" (a recycled pid), the liveness file present and not held. | The kill function is called 0 times; the run ends `failed` with "found not running"; no `killed` event. Control: the same without the liveness file calls the kill function once with the run's pid. |
| I3 | An adopted run, the liveness file held by the test, the pid probe scripted "dead" for 4 ticks (a failing process listing). | The run is `running` after the 4 ticks; the pid probe was not consulted (its script is untouched). |
| W1 | An adopted run that ends (two dead probes), a store whose terminal status write fails twice. | After the finalizing tick `status.json` still says `running` (the injected failure took effect); after 2 more ticks it says `failed`; exactly one terminal `state` event in `events.jsonl`. |
| W2 | An adopted run with 5 iteration lines and 2 solver events in its stream, a store whose first `append_residuals` fails. | After tick 1: 0 residual lines, offset 0. After tick 2: exactly 5 residual lines (steps 1 to 5, once each) and exactly 2 solver events. |
| W3 | As W2, but the first solver-event append fails and the residual append succeeds. | After tick 1: 5 residual lines, offset 0. After tick 2: still exactly 5 residual lines, exactly 2 solver events in stream order. |
| R | The runner crate's existing tests (`cargo test -p rcfd-runner`), and the Studio's run tests that start a real runner. | No test that passed at 0d250c75 fails. |

A missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded. A
defective set-up gets a corrected test by an amendment written before the corrected run.

## Limits (what this does not cover)

- **Unix keeps the pid-and-name probe and the unchecked kill.** D3 rests on Windows sharing modes; std offers nothing
  comparable under `rust-version` 1.75. P1, P2, S2 and I1 are Windows tests and are not built elsewhere.
- **A runner that dies between the solver's spawn and its `launched` event** still lets the next runner launch a
  second solver into the same run folder. Not changed here.
- **A hung pre-launch check** still blocks the queue. Not changed here.
- **D2's window is narrowed, not closed**: between the runner's release of the lock and its removal of the request
  file a new shutdown request can be removed with the old one (the caller's `Client::shutdown` then times out with its
  error). A run submitted before a shutdown request and not yet scanned waits for the next runner, as before; that is
  what a shutdown means.
- **A copy or backup of the project made while a run is live** cannot read `.rcfd/alive/<run id>`; the file is empty.
- **A failure that never clears** (a status file or a JSONL file that stays unwritable) is retried on every tick and
  logged once; a run whose stream cannot be forwarded is still finalized when its solver exits, with the lines lost as
  before.

## Studio changes in the same branch (not registered here)

- The case editor takes as saved the settings whose bytes the save carried, not the settings at the time the answer
  arrives; a failure of another operation no longer ends a save in flight.
- The Start run dialog's plan carries the hash of the case text it was made from: a cell count of another text is not
  applied, the plan is asked for again when the case changes on disk, and Start refuses when the file no longer has
  those bytes.
- The chat feed places transcript lines read before a turn's pause in front of that turn's items; a turn's messages
  carry its number, so a replaced turn's exit does not reach the next turn.
- "Build anyway" and "Show anyway" hold for the text they were given for, not for every later edit of the case.

## Addendum 1 (2026-11-11): results

Code and tests on the branch `studio-runner-semantic-fixes` (committed after this addendum; the registration above was written before the runner code, in the same session). One run of
`cargo test -p rcfd-runner --no-fail-fast` (debug profile, Windows 11, rustc 1.99.0), each check run once.

| Id | Measured | Limit met |
|---|---|---|
| P1 | `lock_held` false with a probe in flight; the unshared open failed with error 32. | yes |
| P2 | `lock_held` true under a runner's handle, false after it. | yes |
| S1 | "alive" after 1 look; "start" after 4 looks; "alive" after 3 looks; "stuck" after 101.8 ms. | yes |
| S2 | Control `Ok(None)` in 0.16 ms. With the request pending: the error "cannot start ...no-such-runner.exe" after 483 ms. | yes |
| I1 | No file: `None`. While the child lived (1.035 s): held. Free 0.1 ms after its exit. | yes |
| I2 | 0 kill calls, the run `failed` with "found not running by the liveness probe", 0 `killed` events. Control: one kill call with the run's pid. | yes |
| I3 | `running` after 4 ticks; 4 of 4 scripted pid-probe answers untouched; the file was held for real. | yes |
| W1 | `running` at the finalizing tick and one tick later, `failed` two ticks later; 1 terminal state event. | yes |
| W2 | Tick 1: 0 residual lines, offset 0. Tick 2: steps 1 to 5 once each, solver events 1 and 2. | yes |
| W3 | Tick 1: steps 1 to 5, offset 0, no solver event. Tick 2: still steps 1 to 5, solver events 1 and 2 in order. | yes |
| R (runner) | 56 passed, 0 failed, 5 ignored across the crate's 14 test binaries (the 10 new tests included). | yes |

Deviations and notes:

- **S2 was narrowed to Windows before any runner code was written** (the table above says so): the test holds the lock
  as a Windows runner does.
- **I2 and I3 take the liveness file's answer from the real file on Windows and from a scripted answer elsewhere**
  (`sys::held` answers `None` off Windows), so both are built on every platform; only the Windows run was made.
- **The checks were run in the debug profile**, not in a release gate, and only on Windows. The Ubuntu gate has not
  run on this branch.
- **No mutation runs were made.** I2's control (the run without a file is killed) and P1's control (the unshared open
  collides) are the only negative controls.
- **R (Studio and the other users of the runner client)**, release profile, after `cargo fmt` of the two crates:
  `cargo test -p rcfd-studio -p rcfd-runner --release`: 359 passed, 0 failed, 26 ignored. `cargo test -p rcfd-runner
  -p rcfd-workspace -p rcfd-mcp -p rcfd-project -p rcfd-claude --release`: 263 passed, 1 failed, 18 ignored. The
  failure is ADR 0123's S9 (`runner_w47b`): its sampler saw 0 runs running at once where it expects 1, in a run made
  while three other builds loaded the machine (31 s for the test). It passed in the two suite runs before and in three
  runs alone afterwards (3.4 to 4.1 s each, 1 each time). Read as a miss of the 20 ms sampler under load, not
  established further.
- **The Studio suite in the debug profile fails 91 tests on this branch**, each with egui's debug-only assertion
  "Dropped TexturesDelta with 1 unapplied deltas". Not compared with main in the debug profile; the gate runs the
  release profile.
- `cargo clippy -p rcfd-runner -p rcfd-studio --all-targets -- -D warnings`: clean. `cargo fmt --check` for the whole
  workspace cannot run on this machine (os error 206); the two packages were checked by name.

## Gate (2026-11-12)

Gate of main f88a18cd (this ADR, F-323, F-324, F-243, F-328, F-329 and F-327 merged) on a git-archive copy, Windows, while three
other builds loaded the machine: fmt per package ok, clippy `-D warnings` clean, release tests 3416 passed, 2 failed, 943
ignored. The two failures are timing assertions: `vof_dambreak_amg::a4_dam_break_with_amg` ("A4 wall time") and
`live_preview_j::j3c_the_frame_asked_for_comes_when_the_debounce_ends` (its control on the first frame). Each passed when run
alone on main afterwards (A4 once, j3c twice). Not a clean gate; the Ubuntu gate has not run.

Second gate, of main 49fbf1a3 (everything merged up to F-352), on a git-archive copy, Windows, with at most three other
builds at its start and none at its end: fmt per package ok, clippy `-D warnings` clean, release tests 3548 passed, 0 failed,
1023 ignored (2 h 28 min). The two timing tests of the first gate passed. Merged after it and not covered: F-351, F-340,
F-357, F-139, F-355. The Ubuntu gate has not run.
