# ADR 0676: F-755 registration: a forced control of the edit lock's starvation and of a lock left behind

Status: **REGISTERED 2026-11-17, written before any code of F-755 and before any run of its checks.** Not yet accepted by the
maintainer. Written by the implementing agent on branch `f755-work` (from 9bfc53e4, F-751 / ADR 0669 merged), worktree
`w49i-work/f755`, scratch `w49i-work/f755-scratch`. Before this text no library or test code of F-755 was written and no run of
any kind was made; the numbers quoted are those printed in ADR 0669. Every choice the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and marked "(standing instruction)".
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-755 (deps F-751), proposed by [ADR 0669](0669-f751-registration-the-two-failures-of-the-first-daily-gate.md)
remaining questions 1 and 2. The binding acceptance:

> Every limit not stated here is registered in the ADR before the run. Test-only hooks (feature-gated, off in every default build) that delay a waiter's attempt and that hold a handle on the lock file without delete sharing; with them the old acquire starves to LockBusy in a registered share of trials and the claim of ADR 0669 never does; the removal of the lock in EditLock::drop retried on a transient error with a logged failure, a lock left behind shown before and removed after; the default bits and every ADR 0135 and ADR 0669 check keep their results.

Clauses: **a1** every limit registered here; **a2** test-only hooks, feature-gated and off in every default build: (i) a delay of a
waiter's attempt, (ii) a handle on the lock file without delete sharing; **a3** with them the old acquire starves to `LockBusy` in a
registered share of trials; **a4** the claim of ADR 0669 never does; **a5** the removal in `EditLock::drop` retried on a transient
error, a failure logged; **a6** a lock left behind shown before (old drop) and removed after (new drop); **a7** the default bits and
every ADR 0135 and ADR 0669 check keep their results.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; each registered run made once; existing
tests and ignored records not edited; defaults bit-identical; no golden, hash table or `baseline_hashes.rs` edit; no new
dependency, no unsafe code; clean-room; roadmap files not edited.

## What was read (no run of a check)

`AGENT_RULES.md`; the brief; ADR 0669 (whole: D1, D2, the checks, amendment 1, addendum 1 and its remaining questions 1 and 2);
ADR 0135 (the edit lock, W48-15 (a)); `crates/rcfd-project/src/atomic.rs` (`acquire`, `EditLock`, `Claim`, `claimed_by_other`,
`with_retry`, `is_transient`, `DEFAULT_RETRY`, `replace_if_hash_with`), `src/journal.rs` (`append`), `src/lib.rs`;
`crates/rcfd-project/tests/edit_lock_f751.rs` (F1, F1c, F2, F3a, F3b) and `edit_lock_w48f.rs`; `crates/rcfd-workspace/src/feed.rs`
(`journal_text`: an unknown kind is shown by its path); `docs/mcp-tools.md` (the journal's kinds); `scripts/gate_test.py` (the gate
builds `cargo test --workspace --release --no-run` with no `--features`); the workspace `Cargo.toml` and the `Cargo.toml` of every
crate that depends on `rcfd-project` (`rcfd-claude`, `rcfd-mcp`, `rcfd-runner`, `rcfd-studio`, `rcfd-workspace`: none has a
`[features]` table naming it, and none is a dev-dependency with features).

## Design (fixed now)

### H. The test hooks (clause a2)

A cargo feature `test-hooks` of `rcfd-project`, **not** in its `default` (the crate has no `[features]` table today; `default = []`
is added). No other `Cargo.toml` names it, and no crate gets it through a dev-dependency (a dev-dependency of another crate with
`features = ["test-hooks"]` would unify into `cargo test --workspace`; not done). The hooks are used only by the crate's own new
test file, which is `#![cfg(feature = "test-hooks")]` and so has no tests in a default build; its tests are run with
`cargo test -p rcfd-project --release --features test-hooks --test edit_lock_f755`.

Module `rcfd_project::atomic::test_hooks`, compiled only with the feature. The hooks are **thread-local** (they act on the thread
that calls `replace_if_hash`, which is the thread that runs `acquire` and `EditLock::drop`), so a test's other threads and other
tests running at the same time are not affected. `test_hooks::set(Hooks)` and `test_hooks::clear()`; `Hooks` has:

* `before_attempt: Option<Box<dyn FnMut(&Path)>>`: called with the lock path at the top of every pass of `acquire`'s loop, before
  the claim check and the `create_new` (a2 (i): the delay of a waiter's attempt; what the delay waits for is the test's).
* `claim_off: bool`: **the old acquire** is reached by this knob: with it `acquire` neither respects another writer's claim nor
  makes one, which is exactly ADR 0669's mutation M1 ("no claim is ever created") exposed under the feature (standing
  instruction: a knob rather than a second copy of the old function, so that the old acquire differs from the new one by the two
  claim branches only).
* `old_drop: bool`: `EditLock::drop` as before F-755 (one `remove_file`, its error ignored).
* `after_lock: Option<Box<dyn FnMut(&Path)>>`: called with the lock path in `replace_if_hash_with` right after the lock is held
  (before the read, the write and the log line).
* `fail_removals: u32`: the next `n` removal attempts of the lock in `EditLock::drop` return a synthetic
  `io::ErrorKind::PermissionDenied` without touching the file (the portable form of a transient removal error; see W below).
* `test_hooks::hold_without_delete_sharing(path, hold) -> io::Result<JoinHandle<()>>`, `#[cfg(windows)]` only: opens `path` for
  reading with `std::os::windows::fs::OpenOptionsExt::share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)` (1 | 2, no `FILE_SHARE_DELETE`
  = 4; std, no unsafe code), and a thread keeps the handle open for `hold` and closes it (a2 (ii)). While it is open, a delete of
  the file needs `DELETE` access that the handle does not share, so `remove_file` fails with `ERROR_SHARING_VIOLATION` (32), which
  `is_transient` already counts.

Without the feature none of this is compiled; the call sites go through private functions that are constant (`false`, `0`, no
call) in a default build.

### R. The retried removal in `EditLock::drop` (clauses a5, a6)

`EditLock` keeps the root, the normalised relative path and `who` beside the lock path. `drop` removes the lock with
`with_retry("remove edit lock", lock, &DEFAULT_RETRY, ..)`: a transient error (`is_transient`: access denied, sharing or lock
violation) is retried, 12 attempts, sleeps 1, 2, 4, 8, 16, 32, 64, 100, 100, 100, 100 ms (sum 527 ms, the existing policy, not
tuned). Outcomes:

* removed (at any attempt): nothing logged (an ordinary edit, removed at the first attempt, does exactly what it did before);
* `NotFound` (the lock was removed by another writer as stale, or by hand): nothing logged, as before (not a failure: the lock is
  gone);
* any other error, or a transient one on the 12th attempt: one journal line through `journal::append` (numbered like every other
  line) `{t, kind: "edit_lock_remove_failed", who, path, error, attempts}` (`attempts` 12 for a retried transient error, 1
  otherwise; `error` the error's text). The lock is then left behind; a cooperating writer breaks it after `EDIT_LOCK_STALE` (30 s)
  as before. The edit itself was already written and logged, so `replace_if_hash` still returns `Ok`.

Effect on default results: an edit whose lock removal succeeds at once makes the same file operations, the same journal lines and
the same return value as before. Only a removal that fails changes: it now waits up to 527 ms (the caller's call returns that much
later) and logs one line instead of leaving the lock silently. No error kind, code, message or timeout changes. The feed
(`rcfd-workspace`) shows the new kind by its path (its default arm); `docs/mcp-tools.md` gets the kind's line.

### Why the old acquire starves under the delay (arithmetic before any run, from ADR 0669's numbers)

The adversarial delay is the test's `before_attempt`: it returns as soon as the holder (a cooperating writer thread of the test,
holding 100 ms and re-taking the lock within one remove and one create as F1c's) is **in the first 50 ms of a hold**, or as soon
as the holder has **yielded** to a claim (released and waiting for the claim to go), or after 150 ms in any case (a bound longer
than one hold, so the hook never blocks forever). The holder publishes the start of its current hold and its yield in atomics that
the hook reads; no file is polled by the hook. This models a scheduler that lets the waiter run only while the lock is surely held
unless the holder has stepped aside. The old waiter's `create_new` then follows the hook's return by microseconds, while at least
50 ms of the hold remain, so it fails unless the waiter thread is preempted for more than 50 ms between the two; that is rare on
this machine (ADR 0669 measured the longest whole critical sections at 31 to 375 ms on a loaded machine, but a 50 ms preemption of
a running thread between two adjacent calls is far rarer). Expected: the old acquire ends in `LockBusy` after `EDIT_LOCK_WAIT`
(2 s) in 10 of 10 trials; the registered share is **at least 9 of 10**.

With the claim (the same hook, the same holder): the waiter's attempts fail until 50 ms; at the first failed attempt after 50 ms
it creates the claim; the holder sees it at the end of its current hold (at most 100 ms later), releases and yields; the hook
returns at once on the yield, the attempt succeeds. Expected 50 + up to 100 + a few ms, about 100 to 200 ms; limit 1.0 s, as F1.

### Why a lock is left behind before and removed after (arithmetic)

`after_lock` opens a handle without delete sharing on the lock for `hold`. The drop runs after the read, the write (with fsync)
and the journal line, about 5 to 40 ms after the open (ADR 0669's critical sections, bulk 4 to 32 ms). With `hold` = 300 ms:
the old drop's one removal fails (the handle is open), the lock is left; the new drop's attempts at 0, 1, 3, 7, 15, 31, 63, 127,
227, 327, 427, 527 ms find it free from about 260 to 295 ms on, so it is removed at the 327 ms attempt (budget 527 ms). With
`hold` = 2.0 s every attempt fails: the lock is left and the failure is logged. Removal can only succeed after the handle closes,
so a lock absent when `replace_if_hash` returns proves the drop waited for the handle.

### W. Windows only, and the non-Windows path

`hold_without_delete_sharing` and the checks that use it (D1 to D3) are `#[cfg(windows)]`: on Linux and macOS `unlink` of an open
file succeeds, so a handle cannot make the removal fail, and those tests are not compiled. The retry and its log are checked on
every platform by D4 with the synthetic error of `fail_removals`, and D5 (a lock already gone). S1 and S2 use no Windows behaviour
and run everywhere. Only Windows 11 is measured here.

## Checks (limits fixed now)

All runs release, `CARGO_TARGET_DIR=f755-scratch/target`, each test by the gate's per-test command `<exe> <name> --exact
--test-threads=1` from the package directory with `CARGO_MANIFEST_DIR` set, counted by the gate's rule (`test result: ok. 1
passed; 0 failed; 0 ignored`, exit 0). New tests in `crates/rcfd-project/tests/edit_lock_f755.rs` (built with `--features
test-hooks`), each run once (one registered run).

| Check | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| S1 | a3 | old acquire (`claim_off`) under the adversarial delay, 10 trials, each on its own file, the call 150 ms after the holder starts | `LockBusy` in **at least 9 of 10** trials, each after at least 2.0 s and under 30 s; no claim file at any time it is checked (after each trial) | 10 of 10 |
| S2 | a4 | the claim (no `claim_off`), the same hook and holder, 10 trials | **10 of 10** accepted, each in under 1.0 s; no claim file left | 100 to 200 ms |
| D1 | a6 before | `old_drop`; `after_lock` holds a handle without delete sharing 300 ms; one edit; then the handle closed (thread joined) | the edit `Ok`; the lock file **present** after the handle is closed; a second edit of the same file (hooks cleared) `LockBusy` after at least 2.0 s; no `edit_lock_remove_failed` line | as stated |
| D2 | a5, a6 after | the new drop, the same 300 ms handle | the edit `Ok`; the lock **absent** when it returns, with the call's time at least 300 ms after the handle opened; no `edit_lock_remove_failed` line; a second edit accepted in under 1.0 s | removed at about 327 ms |
| D3 | a5 | the new drop, a 2.0 s handle | the edit `Ok`; exactly one `edit_lock_remove_failed` line with `path` the file's, `who` the writer's, `attempts` 12, `seq` present; the lock present after the handle is closed | budget 527 ms < 2 s |
| D4 | a5 (all platforms) | `fail_removals` 3: then 12: then `old_drop` with `fail_removals` 1 | 3: lock absent, no failure line; 12: one failure line, `attempts` 12, lock present; old drop: lock present, no failure line | as stated |
| D5 | a5 | `after_lock` removes the lock itself (another writer broke it) | the edit `Ok`; no failure line; no lock | `NotFound` not a failure |
| G1 | a2 | `cargo tree --workspace -e features -i rcfd-project` (default features, every member, dev-dependencies included) | the text has no `test-hooks` | no crate names it |
| G1p | a2 (positive control) | `cargo tree -p rcfd-project --features test-hooks -e features -i rcfd-project` | the text has `test-hooks` | |
| G2 | a2 | `git grep -n test-hooks -- '*Cargo.toml'` at the tip | only `crates/rcfd-project/Cargo.toml` | |
| G3 | a2 | the default test binary of `edit_lock_f755` (`cargo test -p rcfd-project --release --test edit_lock_f755`, no features) | `0 passed`, 0 tests listed | the file is `cfg`'d out |
| E1 | a7 | default build (no features), each test once by the gate's command: `rcfd-project` `edit_lock_f751` (all 5: F1, F1c, F2, F3a, F3b), `edit_lock_w48f` (all 4), `journal_f273` (all); `rcfd-workspace` `save_f270`, `journal_f273`, `mesh_f272`, `feed_replay_f275`, `feed_f273`; `rcfd-mcp` `edit_hook_f275`, `feed_f273` (with `rcfd.exe` built, ADR 0669 deviation 2); `rcfd-studio` `workspace_f270`; `cargo test -p rcfd-project --release --lib` | every test passes with ADR 0669's / ADR 0135's limits (the tests are unchanged) | no result changes |
| E1f | a7 | with `--features test-hooks` (hooks unset): `edit_lock_w48f` (all 4), `edit_lock_f751` F1c, F3a, F3b | all pass | the hooks are inert when unset |
| L1r | a7 | ADR 0669 L1 again on the new code: `edit_lock_w48f two_writers_with_base_hashes_lose_no_update`, 200 runs (script in the scratch folder logging every run), default build | 200 of 200 pass; median test time at most 15.14 s (ADR 0669 L1t's limit) | ADR 0669: 200 of 200, median 6.02 s |
| E2 | rule 7 | `cargo fmt -p rcfd-project --check`; `cargo clippy -p rcfd-project --all-targets -- -D warnings`, and the same with `--features test-hooks` | clean | |
| E3 | docs | `rcfd-cli` `docs_site` | passes | |

NC0 of ADR 0669 (the code of 6600f7c9) is not re-run: it is a run of code that F-755 does not change.

## Negative controls and mutations (registered now; on a scratch copy, `f755-scratch/mut`)

S1 and D1 are this item's negative controls (the old acquire, the old drop), run inside the registered tests. Mutations of the
library, each run against the named tests:

| | Mutation | Expected to be caught by |
|---|---|---|
| M1 | ADR 0669's M1 in the source: no claim ever respected or made, whatever the knob | S2 (the claim's trials end in `LockBusy`) |
| M2 | the old drop in the source (one removal, error ignored), whatever the knob | D2 (lock present), D3 (no failure line), D4 (3: lock present) |
| M3 | the retry kept, the failure not logged | D3, D4 (12: no line) |
| M4 | `NotFound` treated as a failure (logged) | D5 |
| M5 | the `claim_off` knob ignored | S1 (the claim makes the waiter succeed: fewer than 9 `LockBusy`) |
| M6 | `before_attempt` never called | S1 (without the delay the old waiter got the lock in all of ADR 0669's F1c trials) |
| M7 | `fail_removals` ignored | D4 (12: lock absent, no line) |

## Decision rule

* **done:** S1, S2, D1 to D5, G1 to G3, E1, E1f, L1r, E2, E3 met; M1 to M7 caught.
* **partial:** a missed S, D, G or L1r limit (an ignored `*_known_miss` record with the limit intact), or a mutation survives,
  with E1 met.
* **todo** (the lead does not take the change): an E1 test fails because of the change, or the feature reaches a default build
  (G1 or G2 missed).

## Limits (not covered)

* The forced starvation is forced: S1 shows that the old acquire can starve and the claim cannot under the hook's scheduling; it
  does not measure how often the old acquire starved on the gate runner. ADR 0669's L1 / NC0 statistics remain the field evidence.
* The daily gate builds without `--features`, so `edit_lock_f755` has no tests there; it is run by the command above (remaining
  question 1 of the addendum).
* A lock left behind after the retries (D3) is still broken only after `EDIT_LOCK_STALE` (30 s); F-755 logs it and does not
  shorten that.
* Linux and macOS: D1 to D3 are not compiled there (W); D4, D5, S1, S2 run there but are measured here on Windows 11 only.
* The journal's own lock (`journal::lock`) is not changed (ADR 0669 remaining question 3).

## Addendum 1: results (2026-11-17)

Code at 2782c8a3 (committed before any run). Runs: release, `CARGO_TARGET_DIR=f755-scratch/target`, the gate's per-test command
from the package directory with `CARGO_MANIFEST_DIR` set, counted by the gate's rule. The machine was shared with other building
agents. Logs in `f755-scratch` (`run.log`, `e1.log`, `l1r.log`, `g1.txt`, `g1p.txt`, `mut_*.log`, `mut_summary.log`; scripts
`mutate.py`, `e1.py`, `repeat.py`).

| Check | Limit | Measured | Met |
|---|---|---|---|
| S1 | `LockBusy` in at least 9 of 10, each 2.0 to 30 s; no claim file | **10 of 10 `LockBusy`**, 2.0008 to 2.0144 s; 0 claim files | yes |
| S2 | 10 of 10 accepted, each under 1.0 s; no claim left | 10 of 10, 158.8 to 176.4 ms; 0 claims left | yes |
| D1 | edit `Ok`; lock present after the handle closed; second edit `LockBusy` after 2.0 to 30 s; no failure line | `Ok`; lock present; second edit `LockBusy` after 2.0014 s; 0 lines | yes |
| D2 | edit `Ok`; lock absent at return, at least 300 ms after the handle opened; no failure line; second edit under 1.0 s | `Ok`; absent, returned 348.3 ms after the open; 0 lines; second edit accepted in 5.2 ms | yes |
| D3 | edit `Ok`; one failure line (path, who, attempts 12, seq); lock present after the handle closed | `Ok`; one line, `attempts` 12, error `os error 32` (sharing violation), `seq` present; lock present | yes |
| D4 | 3: removed, no line; 12: one line, attempts 12, lock present; old drop: lock present, no new line | as limited | yes |
| D5 | edit `Ok`; no failure line; no lock | as limited (`.rcfd/locks` empty) | yes |
| G1 | `cargo tree --workspace -e features -i rcfd-project` has no `test-hooks` | 25 lines, every edge `feature "default"`, 0 `test-hooks` | yes |
| G1p | with `-p rcfd-project --features test-hooks` the text has `test-hooks` | `rcfd-project feature "test-hooks" (command-line)` | yes |
| G2 | `test-hooks` only in `crates/rcfd-project/Cargo.toml` | only there (line 22) | yes |
| G3 | the default `edit_lock_f755` binary lists 0 tests | 0 tests | yes |
| E1 | every listed existing test passes, default build | **51 of 51 passed**: rcfd-project `edit_lock_f751` 5 (F2 68.4 s), `edit_lock_w48f` 4, `journal_f273` 4; rcfd-workspace `save_f270` 8, `journal_f273` 9, `mesh_f272` 7, `feed_replay_f275` 1; rcfd-mcp `edit_hook_f275` 1 (1 ignored record not run), `feed_f273` 3; rcfd-studio `workspace_f270` 4, `feed_f273` 3 (deviation 2); `rcfd-cli` `docs_site` 2; `rcfd-project --lib` 2 of 2 | yes |
| E1f | with `--features test-hooks`: `edit_lock_w48f` 4, `edit_lock_f751` F1c, F3a, F3b | 7 of 7 passed | yes |
| L1r | 200 of 200; median at most 15.14 s | **200 of 200 passed**; median 6.32 s (3.41 to 11.94 s) | yes |
| E2 | fmt and clippy clean, with and without the feature | `cargo fmt -p rcfd-project --check` clean; `cargo clippy -p rcfd-project --all-targets -- -D warnings` clean, and with `--features test-hooks` clean | yes |
| E3 | `docs_site` passes | 2 of 2 (in E1) | yes |

Mutations (scratch copy `f755-scratch/mut`, a `git archive` of 2782c8a3, `CARGO_TARGET_DIR=f755-scratch/target-mut`):

| | Mutation | Run against | Result |
|---|---|---|---|
| M1 | ADR 0669's M1 in the source (no claim, whatever the knob) | S2: **0 of 10 accepted**, every trial `LockBusy` after about 2.0 s | caught (ADR 0669's surviving mutation is now caught) |
| M2 | the old drop in the source | D2 (lock present at return, 11.1 ms), D3 (no line), D4 (3: lock left) | caught |
| M3 | retry kept, failure not logged | D3, D4 (12: no line) | caught |
| M4 | `NotFound` logged as a failure | D5 (1 line) | caught |
| M5 | the `claim_off` knob ignored | S1: 0 of 10 `LockBusy` | caught |
| M6 | `before_attempt` never called | S1: 0 of 10 `LockBusy` (without the delay the old acquire is not starved by this holder, as in ADR 0669 F1c) | caught |
| M7 | `fail_removals` ignored | D4 (12: lock absent, no line) | caught |

Deviations:

1. The registered run of S1 to D5 (`run.log`) added `--nocapture` to the gate's per-test command, so that the measured numbers
   were printed; the pass rule is the gate's. E1, E1f, L1r and the mutation runs used the gate's command unchanged.
2. E1 named `rcfd-workspace feed_f273`, copied from ADR 0669's list; no such file exists (`feed_f273.rs` is in `rcfd-mcp` and in
   `rcfd-studio`). The `rcfd-studio` `feed_f273` (3 tests, which read the journal the change writes to) was run instead: 3 of 3.
3. `docs_site` was run inside E1 (it is E3).

What the measurements say:

* Clauses a2 to a4: with the adversarial delay, the old acquire starved to `LockBusy` in 10 of 10 trials and the claim of ADR 0669
  was accepted in 10 of 10 within 177 ms; ADR 0669's mutation M1, which survived F1 and F1c there, is caught by S2. The delay is a
  forced schedule (the waiter makes its attempt only while the holder has at least 50 ms of hold left, unless it has yielded); it
  shows that the old acquire can starve and the claim cannot, not how often the field starved (ADR 0669's NC0 and L1 remain that).
* Clauses a5, a6: a handle without delete sharing on the lock made the old drop leave the lock (the next edit `LockBusy` until the
  lock is 30 s old); the retried drop removed it 348 ms after the handle opened (about 300 ms of hold plus the next backoff step),
  and a handle held past the 527 ms retry budget is logged as `edit_lock_remove_failed` with the sharing violation. This is the
  alternative cause of `LockBusy` named in ADR 0669 amendment 1, now reproduced and mitigated; whether the gate's `LockBusy` had
  this cause is still not known.
* Clause a7: default builds have no hook code (G1 to G3); an ordinary edit's removal succeeds at the first attempt and logs
  nothing, so its bytes are as before; every ADR 0135 and ADR 0669 test passed with the default build and with the feature, and
  ADR 0669's L1 gave 200 of 200 again. Public additions only under the feature (`atomic::test_hooks`); the journal has one new kind,
  `edit_lock_remove_failed`, documented in `docs/mcp-tools.md`.

Decision by the registered rule: **done** (every check met, M1 to M7 caught).

Remaining questions:

1. The daily gate builds without `--features`, so `edit_lock_f755` has no tests there. Options: (a) add a gate step
   `cargo test -p rcfd-project --release --features test-hooks --test edit_lock_f755` (about 30 s of tests); (b) leave the file run
   by hand. Recommendation: (a), as the follow-up below; not done here because the gate's workflow and `gate_test.py` are the
   lead's (ADR 0653).
2. A lock left after the retries still blocks cooperating writers until it is 30 s old (`EDIT_LOCK_STALE`). Options: (a) a writer
   that finds an `edit_lock_remove_failed` line for the lock breaks it at once; (b) leave the 30 s. Recommendation: (b) until the
   gate or a user shows such a line: the retry already covers a handle shorter than half a second, and breaking a lock on a log
   line adds a second, non-atomic path.
3. ADR 0669 remaining question 3 (the journal's own lock) is unchanged; the same recommendation.

## For the lead: roadmap entries

> Proposed status: done.

> Gate and test reliability. Follow-up of F-751 (ADR 0669). DONE. ADR 0676, 2026-11-17: a forced control of the edit lock's starvation and of a lock left behind. Test-only hooks in rcfd-project under the feature test-hooks (thread-local; off in every default build and enabled by no other crate, shown by cargo tree --workspace -e features): a delay before each attempt of the acquire, the old acquire by a claim_off knob (ADR 0669's M1), the old drop, a callback after the lock is held, synthetic removal failures, and on Windows a handle on the lock file without FILE_SHARE_DELETE (non-Windows: those three tests are not compiled; the retry is checked there with the synthetic error). Under a delay that lets the waiter try only while the holder has at least 50 ms of hold left, the old acquire starved to LockBusy in 10 of 10 trials and the claim was accepted in 10 of 10 within 177 ms; ADR 0669's surviving mutation M1 is now caught. EditLock::drop now retries a transient removal error with the default retry (527 ms) and logs edit_lock_remove_failed when it still fails: a handle held 300 ms left the lock behind with the old drop (the next edit LockBusy) and the new drop removed it 348 ms after the handle opened; a 2 s handle was logged with the sharing violation. Defaults unchanged (an ordinary edit's removal succeeds at once and logs nothing); every ADR 0135 and ADR 0669 test passed, ADR 0669's L1 200 of 200 again; seven of seven mutations caught. The daily gate does not build the feature, so the new tests run only by their own command.

> Follow-up proposed: The daily gate runs the feature-gated edit-lock controls. Deps F-755, F-723. Acceptance: Every limit not stated here is registered in the ADR before the run. The daily gate runs cargo test -p rcfd-project --release --features test-hooks --test edit_lock_f755 as its own step with its failures attributed to ADR 0676, without enabling the feature in any other build step (cargo tree of the workspace step shows no test-hooks); the gate's other results unchanged.
