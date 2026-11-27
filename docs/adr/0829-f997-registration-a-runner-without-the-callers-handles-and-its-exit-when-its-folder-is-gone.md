# ADR 0829: F-997 registration: a runner started without the caller's handles, and its exit when its project folder is gone

Status: **REGISTERED 2026-11-27, written before any code of F-997 and before any run of its checks.** Before this text no
library or test code of F-997 was written and no file of the repository was changed. What was done: reading (below),
searches of the source tree and of std's source (rustc 1.99.0, `rust-src`), and a release build of the `docs_site` test
into the scratch target (not a check). Results go in addenda at the end; nothing above them is changed afterwards.
Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f997-runner-no-inherited-handles.md`
([the file](briefs/f997-runner-no-inherited-handles.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f997-runner-no-inherited-handles` from
2cbdfa5f, scratch `w49i-work/f997-runner-no-inherited-handles-scratch`. It builds on
[ADR 0123](0123-wave47b-run-service.md) (the runner), [ADR 0143](0143-job48i-native-runner-start-unix-zombies-linux-known-misses.md)
(the native start) and [ADR 0146](0146-job48l-runner-output-early-exit-windows-inherited-handles.md) (the inherited
handles, measured).

## What is asked

F-997's acceptance, read with a script:

> Every limit not stated here is registered in the ADR before the run. A runner started detached (rcfd-runner ensure, the run tools' start) inherits no handle of its caller but its own standard streams: a parent whose standard output is a pipe starts a runner and exits, and the pipe reaches its end within a registered time while the runner lives, with the negative control on the code before the change; a runner whose project folder is removed, with no run queued, running or paused, exits by itself within a registered time as rcfd-runner shutdown does, and one whose folder is present keeps serving; every existing runner start test passes.

Clauses: **a** no inherited handle but the standard streams (pipe end within a time, runner alive; control before);
**b1** exit by itself after the folder's removal, cleanly, within a time; **b2** a runner with its folder keeps serving;
**c** every existing runner start test passes.

## What was read

AGENTS.md; the rules (version 5); the brief; `crates/rcfd-runner/src/sys.rs` (all), `runner.rs` (the service loop
`serve`, `Config`, `Runner::shutdown`, lines 1-310 and 1155-1200), `client.rs` (`shutdown`, the start's surroundings),
`site/remote.rs` (`RemoteRuns`), `tests/capture/mod.rs`, the heads of every file in `crates/rcfd-runner/tests/`,
`started_by_qrl2.rs` lines 1-80; ADR 0146's Context, checks table and question 1 (lines 1-100, 140-210, 340-420); std's
`library/std/src/sys/process/windows.rs` (lines 160-430: `CreateProcessW` gets `inherit_handles`, true unless the
unstable `CommandExt::inherit_handles`), `library/std/src/os/windows/process.rs` (`inherit_handles` unstable, issue 146407;
`raw_attribute` unstable and `unsafe`, issue 114854), `library/std/src/sys/fs/windows/remove_dir_all.rs` lines 173-206.

## Findings of the reading (before any code)

1. **(a) cannot be built under the rules on Windows.** Every route that stops the inheritance needs either unsafe code
   (`CreateProcessW` with `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`, or `SetHandleInformation`), forbidden by rule 5 and by the
   workspace lint `unsafe_code = "forbid"` (`Cargo.toml` line 60, which no crate can lower), or an unstable std switch
   (`inherit_handles`, `raw_attribute`) on a stable toolchain, or a launcher in the start path, against ADR 0143
   decision 1. ADR 0146 measured this (I1-I6) and left it as its question 1, which has no recorded answer. Changing the lint
   or rule 5 needs the maintainer's word (rule 6: not taken). So (a) is registered with its check and its control, and is
   expected to miss on Windows. The Windows fix is the open question 1 below.
2. **Unix holds (a) for the standard streams already.** std creates its own descriptors close-on-exec and replaces 0-2 in
   the child, so the caller's pipe on descriptor 1 does not reach the runner. ADR 0146 C1 passed on Linux (3 ms). A
   descriptor of 3 or more that the caller itself inherited without close-on-exec would pass on; closing it needs
   `pre_exec`, which is unsafe (not covered).
3. **What a removal leaves on Windows.** The runner holds `.rcfd/runner.lock` open without sharing. std's
   `remove_dir_all` deletes in listing order and stops at the first file it cannot delete (after its sharing-violation
   retry). In `.rcfd`, `runner-config.json` and `runner.json` come before `runner.lock`, so a removal under a live runner
   takes `runner.json` and leaves the folder, `.rcfd` and the lock. On Unix the whole folder goes. `runner.json` is written
   once at the start (atomic rename, never absent while the runner lives) and removed only at a clean shutdown. No code in
   the repository removes it (searched: `crates/`, `apps/studio-web/src`).

## Design

### A. No inherited handle (not built: finding 1)

Nothing is changed in `sys.rs` or `client.rs`. The intended Windows design, for the follow-up after the maintainer's
answer, is ADR 0146 question 1 option (b): `CreateProcessW` with an extended start-up info whose
`PROC_THREAD_ATTRIBUTE_HANDLE_LIST` holds only the three standard handles, in one reviewed module. The operating system's
page for it was not read in this work.

### B. The exit when the project is gone (`runner.rs`, the service loop)

* **The look.** Every `GONE_LOOK_MS = 1000` ms (a constant, not a config key, so `runner.json`'s `config` keeps its keys),
  the loop asks `project_gone(project, dot)`. The project is gone when any of these is absent (NotFound only; any other
  error reads as present): the project folder, `.rcfd`, `.rcfd/runner.lock`, `.rcfd/runner.json`. The last covers Windows
  (finding 3), the lock covers a partial removal on Unix.
* **The rule.** `leave_gone(gone, active, pending_left)`: leave only when gone, no local run running or paused
  (`active` empty) and no queued or paused pending run whose run folder still exists. A pending run whose folder is gone
  cannot be launched and does not hold the runner. Active runs at the removal: the runner keeps serving until they end
  (they are finalised as now), then leaves; nothing is cancelled or killed. Taken on the maintainer's standing instruction
  of 2026-11-11 (take the recommendation), question 3.
* **The exit** is the clean shutdown's path: the loop breaks, `Runner::shutdown` (pre-launch checks killed, remote runs no
  longer followed, as `rcfd-runner shutdown`), `runner.json` removed, the lock dropped, exit code 0. The log line "shut
  down cleanly" is replaced by none when the project is gone, so nothing is written into a removed folder.
* The look interval, 1 s: taken on the standing instruction, question 2. It is 10 default polls, and a removal is seen
  within one look.

## Checks (rule 14)

| Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|
| A1 (clause a): a parent `rcfd-runner ensure --project p` with stdin null and stdout, stderr piped starts a runner and exits; its stdout reaches EOF within the limit after its exit, and the runner lives at that EOF (lock held) | EOF at most 3.0 s after the parent's exit | ADR 0146 C1's limit for a released caller | Windows: held until the teardown's shutdown (miss, finding 1); Unix: 3 ms | Windows none; Unix 1000 | code verification |
| A1c (control, before): A1 on the code before the change | held at 3.0 s on Windows | ADR 0146 I1, C1c | held | | code verification |
| B1 (b1): `serve` started directly on a project; once `runner.json` is there the test removes the folder (`remove_dir_all`, its error ignored); the runner exits by itself, code 0 | exit at most 5.0 s after the removal returns | F-997 acceptance (time registered here), from one look (1.0 s) plus one poll and the shutdown | 1.1 s | 4.5 | code verification |
| B2 (b2): the same start with the folder kept; at 3.5 s after `runner.json` appears (3 looks) the runner still lives and holds the lock; then a shutdown request ends it, code 0 | alive at 3.5 s; exit at most 5.0 s after the request | F-997 acceptance | alive; 0.1 s | 50 | code verification |
| B3 (unit, `runner.rs`): `leave_gone` over every combination of gone, active (0, 1) and pending left (0, 1): true only for (gone, 0, 0); `project_gone` on a temporary folder: false when whole; true with `runner.json` removed alone, with the lock removed alone, with the folder removed | exact | the design | as stated | | code verification |
| C (clause c): `cargo test -p rcfd-runner --release --lib` and the files `native_start_w48i`, `started_by_qrl2`, `no_window_scan`, `zombie_w48i`, `output_w48l`, `runner_w47b`; their ignored tests stay ignored | all pass | F-997 acceptance | pass | | unchanged default output |

A1 is in the new file `crates/rcfd-runner/tests/gone_f997.rs` as `a1_a_capturing_parent_of_ensure_reaches_eof_within_3_s_known_miss_on_windows`,
ignored on Windows with the limit intact (rule 2), and run once here with `--ignored --exact`. Because (a) is not built,
the code of A1's run is the code before the change: A1 and A1c are one run. B1 and B2 are in the same file
(`b1_a_runner_whose_folder_is_removed_exits_within_5_s`, `b2_a_runner_with_its_folder_keeps_serving`), B3 in
`runner.rs`'s tests (`b3_leave_gone_and_project_gone`). Every test of the file ends its runner in a `Drop` guard that runs when the test fails too (a
shutdown request, then a kill of the recorded pid if it still lives after 10 s, then the folder's removal).

### Negative controls and mutations (rule 16)

Each run once on a scratch copy (`git archive` of the branch head) with `CARGO_TARGET_DIR` in the scratch folder:

* M1: `project_gone` always false. Expected caught by B1 (no exit) and B3.
* M2: `project_gone` without the `runner.json` term. Expected caught by B3 on every platform and by B1 on Windows.
* M3: `leave_gone` ignores the active runs. Expected caught by B3.
* M4: `project_gone` true with the folder whole (the `runner.json` term inverted). Expected caught by B2 and B3.

M1 is committed as `crates/rcfd-runner/tests/gone_f997.mutation.patch`, with the first line
`# expect-fail: cargo test -p rcfd-runner --release --target-dir target/mutation --test gone_f997 b1_`.

## Size

Size (agent): partial; 1; 0; 3; 110000

## Decision rule

Done if A1 holds with its control, B1, B2, B3 and C pass, and each mutation is caught. Partial otherwise. Predicted:
partial, because A1 misses on Windows (finding 1).

## Not covered

Any run on Linux or macOS (A1 and B1-B2 are not ignored there and run in the lead's CI); descriptors of 3 or more on Unix
(finding 2); the run tools' start through `rcfd-mcp` (its check C2 exists in `crates/rcfd-mcp/tests/runner_stdio_w48l.rs`,
a known miss on Windows for the same cause, not run: `rcfd-mcp` is not touched); the `.rcfd/runner.lock` file a removal
leaves on Windows after the runner's exit (released, not deleted, as before); `docs/run-control.md` (not in the owned
files).

## Open questions for the maintainer

1. **The Windows route for (a)** (scope; rule 5 and the workspace lint; not taken). Options: (a) one reviewed unsafe
   module in `rcfd-runner` (the lint lowered from `forbid` to `deny` in the workspace, `allow` in that module only) with
   the handle list of the design A; (b) wait for std's `inherit_handles` to be stable, with the runner opening its own log
   (ADR 0146 option c); (c) a launcher that does not inherit, against ADR 0143 decision 1. **Recommendation:** (a), as a
   follow-up registration under F-997 (rule 13).
2. **The look interval.** Options: (a) 1 s; (b) every poll (100 ms by default); (c) 5 s. **Recommendation:** (a), taken.
3. **Active runs at a folder's removal.** Options: (a) keep serving until they end, then leave; (b) cancel them and leave;
   (c) leave at once, the solvers keep running unadopted. **Recommendation:** (a), taken: a removal is not a cancel
   request.
