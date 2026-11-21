# ADR 0751: F-855 registration: the edit lock's F2 ended four probe calls in LockBusy; the cause diagnosed under registered loads

Status: **REGISTERED 2026-11-21, written before any code of F-855 and before any run of its checks.** Before this text
no library or test code of F-855 was written, nothing was built and no test was run. What was done: reading (below), searches
of the gate artifacts with `grep`, the arithmetic below, and one read-only query of this machine
(`Get-MpComputerStatus`: real-time protection of the installed virus scanner is on). Results go in addenda at the end;
nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f855-edit-lock-f2-lockbusy.md` and follows `docs/process/agent-rules.md`, version 3.
Branch `f855-edit-lock-f2-lockbusy` from main c547b175, scratch `w49i-work/f855-edit-lock-f2-lockbusy-scratch`. Parent
records: [ADR 0669](0669-f751-registration-the-two-failures-of-the-first-daily-gate.md) (F2, the claim D2, the second
resolve D1, mutations M2 and M6) and
[ADR 0676](0676-f755-registration-a-forced-control-of-the-edit-locks-starvation-and-of-a-lock-left-behind.md) (the test
hooks H, the retried removal R, S1, D1 to D5).

## What is asked

Roadmap item F-855, the binding acceptance:

> Every limit not stated here is registered in the ADR before the run. For rcfd-project edit_lock_f751::f2_a_cooperating_writers_renames_never_refuse_or_fail_another_writers_path (ADR 0669 F2: 10000 of 10000 probe calls return Conflict): the cause of a probe call that waits EDIT_LOCK_WAIT and ends in LockBusy while a cooperating racer replaces the file is shown by a registered control (the starvation of ADR 0676 S1, a lock left behind, or the load of the runner), with a registered number of repeated runs with and without a registered load; the code or the test is then changed under the registration so that the registered number of repeats passes, and a mutation shows that the check still catches a refused or failed probe. The count of 10000 is not lowered and the test is not ignored, skipped or retried by the gate to make it pass; a change of what F2 asserts is an amendment of ADR 0669.

Clauses: **a1** every limit registered here; **a2** the cause shown by a registered control; **a3** a registered number of
repeated runs with and without a registered load; **a4** the code or the test changed under the registration so that the
registered number of repeats passes; **a5** a mutation shows that the check still catches a refused or failed probe;
**a6** the count stays 10000, F2 is not ignored, skipped or retried, and a change of what it asserts is an amendment of
ADR 0669. The brief adds: the cause is shown before anything is changed; ADR 0669's M2 and M6 stay caught; `gate.yml` is
not changed; nothing is added to a default build.

## What was read

`docs/process/agent-rules.md` (version 3); the brief; `crates/rcfd-project/tests/edit_lock_f751.rs` (F2 at lines 155 to
198), `tests/edit_lock_f755.rs` (its head), `tests/common/mod.rs` (`scratch`: the project folder is under
`RCFD_PROJECT_SCRATCH` or the temporary folder and is left on disk after the test); `crates/rcfd-project/src/atomic.rs`
(whole), `src/journal.rs` (whole), `src/time.rs` (`now`); `crates/rcfd-project/Cargo.toml`; ADR 0669 and ADR 0676
(whole); `.github/workflows/gate.yml` (the lines of the test step and of the `test-hooks` step: the gate runs
`edit_lock_f755` alone with the feature) and the head of `scripts/gate_test.py` (one process per test, three at a time,
the slowest tests of the earlier run first); ADR 0750 (the form of a registration); the artifacts of the gate of e9378ecb
(shard 3: `test.log` lines 8379 to 8399, `times.tsv`, `progress.log`; `attribution.md`) and the `times.tsv` rows of
`edit_lock_f751` in the gates of 46cea4f8, 24a26223, 5637150a, a9570de6 and 5819a15c.

## Readings of the gate's artifacts (before any run)

* The failed F2 printed `probe 9996 of 10000 conflicts, 4 other`, all four `LockBusy` on `cases/c.txt`; `racer 5395
  accepted, 0 errors`. It took 112.93 s; in the five gates where it passed: 71.88, 59.84, 87.15, 79.01 and 53.04 s.
* It was test 36 of 1484 of its shard to finish (`progress.log`, at 0:42:06): `gate_test.py` starts the slowest tests
  first, so F2 ran beside the heaviest tests of the shard, three processes at a time on the runner.
* In the same shard F3a (`f3a_a_fresh_claim_of_another_writer_is_waited_for_then_busy`), which creates a project and waits
  `EDIT_LOCK_WAIT` once, took 5.77 s; in the five other gates 2.03, 2.04, 2.02, 2.03 and 2.07 s. So this runner delayed
  one process by more than 3 s at least once (at 0:58:16, sixteen minutes after F2 ended). This is a reading of the
  process time `gate_test.py` records; where inside the process the time went is not known.
* The test's project folder is not among the artifacts, so the gate's own journal and lock files cannot be read. What
  the gate's failure was is therefore an inference in every case below; the registered runs are on this machine.

## Candidate causes (from the code, before any run)

The signature to explain: the probe waits 2 s and is refused, several times in one run, while the racer has no error.

* **J, a journal lock left behind** (found by reading `journal.rs`; not named by the brief). The racer appends its edit
  line while it holds the edit lock (`replace_if_hash_with` calls `log_edit` before the lock is dropped). The append
  takes `.rcfd/locks/journal.lock` and removes it in `Held::drop` with one `remove_file` whose error is ignored. If that
  removal fails once (on Windows: another process has the file open without delete sharing, as a virus scanner or an
  indexer may), the journal lock stays. The racer's next append then waits `LOCK_WAIT` = 2 s for it **inside the edit
  lock**, appends its line without `seq` and returns `Ok`; the append after that waits 2 s again; the third finds the
  lock older than `LOCK_STALE` = 5 s after about 1 s more and breaks it. A probe that began its wait before the racer's
  append began is refused after `EDIT_LOCK_WAIT` = 2 s in each of the two long holds. Expected per event: 2 probe calls
  in `LockBusy`, 0 racer errors, about 5 s added, two journal lines without `seq` and one with `journal_lock_broken_s`.
  The gate's 4 would be two events; an even count fits, which is noted and proves nothing.
* **S, a holder stalled inside one critical section for 2 s or more** (the runner's load): the racer's read, write with
  fsync, rename or journal append takes more than 2 s once; the probe is refused, correctly by the contract of
  `ProjectError::LockBusy` ("held the file's edit lock for the whole wait"), and F2 asks more than the lock promises.
  One refusal per stall, 0 racer errors. The F3a reading above shows delays of that size on the gate's runner.
* **V, a starved probe** (ADR 0676 S1): the lock is released and taken again during the probe's wait. With the claim of
  ADR 0669 the racer does not take the lock while the probe's fresh claim exists, so this needs the claim to fail: its
  creation refused, or its `metadata` failing in the racer's `claimed_by_other` (which then reads "no claim").
* **C, a claim left behind**: `Claim::drop` removes the claim with one `remove_file`, error ignored. A claim left by a
  failed removal blocks **both** writers until it is `EDIT_CLAIM_STALE` = 4 s old, so the racer would have `LockBusy`
  errors too. This does not fit the gate's signature (0 racer errors); it is the same fault as J in another place.
* **L, an edit lock left behind**: since ADR 0676 the removal is retried and a failure is logged
  (`edit_lock_remove_failed`); a lock left behind blocks both writers for 30 s, which does not fit either.

## Design (fixed now)

### The diagnostic F2d (clauses a2, a3)

A new test file `crates/rcfd-project/tests/edit_lock_f855.rs`, `#![cfg(feature = "test-hooks")]` (so a default build has
no test in it, as `edit_lock_f755`), its leading comment naming ADR 0751 and F-855. Test
`f2d_f2_with_a_record_of_every_refused_probe_call`, `#[ignore]`d as a heavy record (rule 7; it runs as long as F2) and
run by its command with `--ignored`. Its body is F2's (the same project, racer, 10000 probe calls, the same two
assertions as its pass rule) with a record added:

* the probe: the start and end of every call (an `Instant` each); the racer: start, end and outcome of every call;
* at once after a probe call that is not `Conflict`: whether a claim file exists; whether the edit lock file exists, its
  text (`<pid> <who> <time>`) and age; whether `journal.lock` exists and its age;
* at the end: the number of journal lines, of lines without `seq`, of lines with `journal_lock_broken_s`, of kinds
  `edit_lock_remove_failed` and `stale_lock_broken`, the files left in `.rcfd/locks`, the longest probe call and the
  longest racer call;
* one printed line per refused call and one for the run (`F2D ...`), which the run script keeps.

The class of a refused probe call, with wait `[p0, p1]`, decided in this order:

| Class | Rule |
|---|---|
| X | the result is not `LockBusy` (printed verbatim) |
| J | `journal.lock` exists at the snapshot and is at least 1.0 s old |
| C | a claim file exists at the snapshot (the probe has removed its own on its way out, the racer makes none while it holds the lock) |
| S | the racer was inside one call from `p0 + 100 ms` at the latest until after `p1`, and that call returned `Ok` |
| V | anything else (the racer ended or began a call inside the wait: the lock changed hands or was free) |

100 ms is `EDIT_CLAIM_AFTER` (50 ms) doubled: until the probe has claimed, the racer may take the lock again (ADR 0669
D2), and a call of the racer that began later than that and held the lock is a failure of the claim, class V.

F2d adds no per-attempt hook, so that a passing call is timed as in F2 except for two clock readings; it is built with
the feature only because the file is. It is F2 with instrumentation, as the brief asks; the unchanged F2 of the default
build is run in the after-series.

### The loads (clause a3)

Scripts in the scratch folder (`loads.py`, Python 3.9, standard library and `ctypes`); every run has
`RCFD_PROJECT_SCRATCH=<scratch>/proj`, so the test's project is on the volume of the scratch folder (E:).

* **K0**: no load started by this work. The machine is shared with other agents' builds; that is not controlled and is
  said with the results.
* **K1, processors**: 16 processes (this machine has 16 logical processors), each an endless arithmetic loop at normal
  priority.
* **K2, disk**: 4 processes, each repeating: write a 64 MiB file in `<scratch>/load-disk` in 1 MiB pieces, `fsync`,
  close, delete.
* **K3, a reader of the lock files**: 1 process; every 250 ms it lists `<scratch>/proj/rcfd-project-*/p/.rcfd/locks`
  and opens every file there for reading with the share mode read and write, without delete sharing; it holds the
  handles 5 ms and closes them. This is the model of a scanner that opens a new small file; it forces candidates J, C
  and L in the running test. 5 ms is below the sum of the first three sleeps of `DEFAULT_RETRY` (1 + 2 + 4 ms), so a
  retried removal succeeds at its fourth attempt at the latest; 250 ms is chosen so that events are frequent but the run
  still ends: with the journal lock present about 3 % of the time (about 0.4 ms of a racer cycle of about 13 ms; an
  estimate), a poll meets it with probability 0.03, 0.12 times a second, about 8 events in a run of 70 s.

Each load is started 5 s before its series and stopped after it. K1 and K2 are the loads of a busy runner; K3 is a forced
cause and is named as such wherever its counts are given.

### The series (clause a3)

**10 runs** of F2d under each of K0, K1, K2 and K3, before any change of the library or of F2: series **B0, B1, B2,
B3**. Each run is the gate's per-test command for an ignored test (`<exe> <name> --exact --test-threads=1 --ignored
--nocapture`, from the package directory with `CARGO_MANIFEST_DIR` set), counted by the gate's rule (`test result: ok. 1
passed; 0 failed`, exit 0). The count 10 is fixed from the time a run takes (53 to 113 s on the gate, more under load):
40 runs before and at least 40 after are several hours on a shared machine. With 10 runs a failure rate of 1 in 6 (the
gate's one in six runs) shows at least once with probability 0.84; a rate of 1 in 60 would mostly not show. That limit
of the series is known beforehand.

After the changes: **A0 to A3**, 10 runs each of the unchanged-or-amended **F2 of the default build** by the gate's own
command (`<exe> <name> --exact --test-threads=1`), under K0 to K3; and F2d again, 10 runs, under every load whose
B-series had a failure (**AD-series**), so that before and after are compared on one test.

### The forced controls (clause a2), in `edit_lock_f855.rs`, Windows only

Both use `test_hooks::hold_without_delete_sharing` of ADR 0676 (a handle without delete sharing, which only Windows can
make fail a removal; on other platforms the tests are not compiled, as D1 to D3 of ADR 0676).

* **CJ** (`cj_a_journal_lock_whose_removal_fails_once_is_not_left_behind`). A registered extension of the hooks: `Hooks`
  gains `journal_locked: Option<Callback>`, called in `journal::append` with the path of the journal lock right after
  the lock is taken, on the appending thread; compiled only with the feature (the call site is an empty inline function
  in a default build, as the other hooks). Writer W, edit 1 of `cases/c.txt`: `journal_locked` opens a handle without
  delete sharing on the journal lock for 300 ms. Then edit 2 by W with `after_lock` set to release a probe thread and
  sleep 100 ms (so the probe's wait begins before W's append), the probe calling `replace_if_hash` with a wrong base.
  Limits: edit 1 `Ok`; the handle was opened; `journal.lock` absent when edit 1 returns, at least 300 ms after the
  handle opened; edit 2 `Ok` in under 1.0 s; the probe's result `Conflict` in under 1.0 s; every journal line has `seq`.
  Arithmetic for the unchanged library: the one removal fails, the lock is left; edit 2's append waits 2 s in the edit
  lock, so edit 2 takes about 2.1 s, its line has no `seq`, and the probe, waiting from the first 100 ms on, is refused
  with `LockBusy` after 2.0 s: the gate's signature (probe refused, writer without error). With change A the removal is
  retried at 1, 3, 7, 15, 31, 63, 127, 227 and 327 ms and succeeds at 327 ms.
* **CC** (`cc_a_claim_whose_removal_fails_once_is_not_left_behind`). The test creates the edit lock of `notes/a.md` by
  hand; a writer thread calls `replace_if_hash`; when its claim file appears (after 50 ms) the test opens a handle
  without delete sharing on the claim for 300 ms and removes the lock. Limits: the writer's edit `Ok`; no claim file
  once the handle is closed; a second edit of the file accepted in under 1.0 s. Arithmetic for the unchanged library:
  the claim's one removal fails, the claim is left, the second edit is refused with `LockBusy` after 2.0 s.

Each control is committed with the hook extension before any change of the library's behaviour and run once on that
commit (expected to fail); it is run again after the change (expected to pass).

### The changes (clause a4), each made only on its registered condition

* **A, the journal lock's removal is retried** (if CJ fails on the unchanged library): `journal::Held::drop` removes the
  lock with `with_retry("remove journal lock", .., &DEFAULT_RETRY, ..)`, the policy `EditLock::drop` has had since ADR
  0676 R (12 attempts, sleeps summing to 527 ms). A removal that still fails is left as today (the lock is broken after
  `LOCK_STALE`); it is not logged, because the log is the journal whose lock it is.
* **B, the claim's removal is retried** (if CC fails on the unchanged library): `Claim::drop` in the same way; a
  removal that still fails is left as today (stale after `EDIT_CLAIM_STALE`).
* Effect on default results of A and B: an append and a claim whose removal succeeds at the first attempt make the same
  file operations, journal lines and return values as before. Only a removal that fails changes: the writer retries for
  at most 527 ms (holding the edit lock if it has it) where today the lock or claim stays and the next writers wait 2 s
  each (journal) or are refused for up to 4 s (claim). No constant, error kind, message or code changes;
  `EDIT_LOCK_WAIT`, `LOCK_WAIT`, the claim and the stale ages keep their values. This is the trade ADR 0676 R registered
  for the edit lock's own removal, applied to the two other files of the same folder; it is taken on the maintainer's
  standing instruction of 2026-11-11 (take the recommendation) and repeated under "Open questions" for the lead.
* **C, F2 accepts a refusal that the lock's contract allows** (only if a run of B0, B1 or B2 has a refused probe call of
  class S; K3 does not count, and neither does reasoning alone). An amendment appended to ADR 0669 before the code. F2
  keeps its name, its project, its racer and its 10000 calls and times both threads' calls as F2d does. A probe call
  passes if it is `Conflict`, or if it is `LockBusy` of class S with no claim file at once after it (the table above,
  rules C and S, with the same 100 ms). Any other result fails the test as today: a refused path, an I/O error, a
  starved probe (class V), a claim left behind. In addition the journal must have `seq` on every line and no line with
  `journal_lock_broken_s`, `edit_lock_remove_failed` or `stale_lock_broken` (so a long hold made by a lock left behind,
  J or L, still fails), and the racer has 0 errors as today. The classification is one function of the test file with
  its own fast test **FS** over made-up call times (a whole-wait hold passes; a hold that begins 150 ms into the wait,
  a racer call that ends inside the wait, a failed racer call and a claim present each fail). The brief asks for the
  amendment in this form (F2 changed in place, not ignored); rule 2 would keep the old F2 as an ignored record beside a
  new test. The brief's form is taken, and the point is put to the lead under "Open questions".
  If no run of B0 to B2 shows class S, F2 is not changed; the design above is then a proposal in the addendum.
* If a refused call of class V or X appears, nothing is changed for it under this registration: it is reported with its
  record, and an amendment of this ADR precedes any further run.

## Table of checks

Release builds, `CARGO_TARGET_DIR=<scratch>/target`. "The brief" is `docs/adr/briefs/f855-edit-lock-f2-lockbusy.md`.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| B0 to B3 | F2d, 10 runs under each of K0, K1, K2, K3, on the unchanged library: failures, and the class of every refused call | none (reported); the cause counts as reproduced under a load when at least one run of its series fails and every refused call has a class other than V and X | the acceptance of F-855 ("a registered number of repeated runs with and without a registered load") | B0: 0 or 1 of 10; B1, B2: unknown; B3: 10 of 10, class J, about 16 refused calls a run | n/a | code verification (a diagnosis without a limit) |
| CJ-before | CJ on the commit before change A | fails (the journal lock present; the probe `LockBusy`) | the brief ("a control that fails before the change") | fails | n/a | code verification (negative control) |
| CC-before | CC on the commit before change B | fails (the claim present; the second edit `LockBusy`) | the brief, the same | fails | n/a | code verification (negative control) |
| CJ | CJ after change A | passes: edit 2 and the probe each under 1.0 s, the probe `Conflict`, `journal.lock` absent at least 300 ms after the handle opened, every line with `seq` | 1.0 s: ADR 0669 F1 (a waiter served in under 1.0 s); 300 ms: the hold of the handle, ADR 0676 D2 | edit 2 about 0.11 s, the probe about 0.11 s | 9 | code verification |
| CC | CC after change B | passes: no claim once the handle is closed; the second edit accepted in under 1.0 s | 1.0 s: ADR 0669 F1 | about 5 ms | 200 | code verification |
| A0 to A3 | F2 of the default build by the gate's command, 10 runs under each of K0, K1, K2, K3, after the changes | 10 of 10 pass in each series | the acceptance of F-855 ("the registered number of repeats passes"); F2's own limit, ADR 0669 (10000 of 10000, or as amended under change C) | A0, A3: 10 of 10; A1, A2: 10 of 10 unless class S exists and was not shown before | n/a | code verification |
| AD | F2d after the changes, 10 runs under every load whose B-series had a failure | 10 of 10 pass; no journal line without `seq` | the acceptance of F-855, the same | 10 of 10 | n/a | code verification |
| E1 | default build, each test once by the gate's command: `rcfd-project` `edit_lock_f751` (all 5), `edit_lock_w48f` (all 4), `journal_f273` (all); `rcfd-workspace` `journal_f273` (all); `cargo test -p rcfd-project --release --lib` | every test passes with its own limits (ADR 0135, ADR 0669, ADR 0188) | the brief ("every ADR 0135, ADR 0669 and ADR 0676 test keeps its result") | all pass | n/a | unchanged default output |
| E1f | with `--features test-hooks`: `edit_lock_f755` (all tests, ADR 0676 S1, S2, D1 to D5), each once | all pass | the brief, the same | all pass | n/a | unchanged default output |
| G | `cargo tree --workspace -e features -i rcfd-project` has no `test-hooks`; the default build of `edit_lock_f855` lists 0 tests | holds | ADR 0676 G1, G3; the brief ("nothing is added to a default build") | holds | n/a | unchanged default output |
| FS | only with change C: the classification's fast test | passes | change C of this registration, from the brief ("a refused, failed or starved probe" still fails) | passes | n/a | code verification |
| M | the mutations below | each caught by the named test | rule 16; the brief (M2 and M6 of ADR 0669 stay caught) | MJ1 to MJ3 caught; M2r caught; M6r caught in at least one of three runs with probability about 0.95 | n/a | code verification |
| P | `python scripts/process_report.py --check`; `python scripts/fmt_check.py rcfd-project`; `cargo clippy -p rcfd-project --all-targets -- -D warnings`, and the same with `--features test-hooks`; `rcfd-cli` `docs_site` | 0 failures; clean; passes | rule 7; the lead's instruction | clean | n/a | code verification |

## Mutations (on a scratch copy of the committed tree, its own target folder)

* **MJ1** (committed as `crates/rcfd-project/tests/edit_lock_f855.mutation.patch`, first line `# expect-fail: cargo test
  --release -p rcfd-project --features test-hooks --test edit_lock_f855`): `journal::Held::drop` with one removal, its
  error ignored (the code before change A). Expected: CJ fails.
* **MJ2**: `Claim::drop` with one removal (the code before change B). Expected: CC fails.
* **MJ3**: the hook `journal_locked` is never called. Expected: CJ fails at "the handle was opened".
* **M2r**: ADR 0669's M2 (no second resolve under the lock) on the changed library, F2 of the default build, one run.
  Expected: caught (ADR 0669: 13 of 10000 calls refused).
* **M6r**: ADR 0669's M6 (the lexical key joined with `\`), F2, at most three runs, stopped at the first failure; caught
  when one fails. ADR 0669 caught it with 1 call of 10000, so one run catches it with probability about 0.63 and three
  with about 0.95; a survival of all three is reported as such and makes the verdict partial.
* Only with change C: **MS1**, the classification accepts every `LockBusy`: expected caught by FS; and M2r and M6r are
  then run on the amended F2.

## Size

Size (agent): partial; 0; 3; 2; 120000

(Partial is expected because the gate's own event cannot be identified from its artifacts. The heavy runs are the B-series,
the A-series and the AD-series.)

## Decision rule

The brief's, with its terms fixed:

* **done**: a control (CJ, or CC, or a run of a B-series with its class) shows the cause, failing before the change and
  passing after it; every run of A0 to A3 and of AD passes; every mutation is caught; E1, E1f, G and P are met; and the
  cause shown is the one that a failed run of B0, B1 or B2 had (a failure reproduced without the forced load K3).
* **partial**: no run of B0, B1 or B2 fails (the cause of the gate's failure is then not reproduced: a forced control
  shows a fault that fits its signature, not that the gate met it); or a registered run was not made; or a change rests
  on the repeated runs alone; or a mutation survives; or a run of an A-series fails.
* **stopped and reported**: anything outside `crates/rcfd-project` and its ADRs would have to change, or a change would
  alter what another writer of a project file observes beyond what ADR 0669 and ADR 0676 registered (a longer
  `EDIT_LOCK_WAIT` or `LOCK_WAIT`, another error).

## Open questions (for the lead; none decided here beyond rule 6)

1. Changes A and B make a writer whose removal of the journal lock or of its claim fails retry for up to 527 ms. That is
   the trade of ADR 0676 R, not a longer `EDIT_LOCK_WAIT`; taken as inside the brief. If the lead reads it as a change
   of what another writer observes, the two changes are one commit each and can be left out.
2. Change C edits F2 in place, as the brief words it; rule 2 would keep the old F2 as an ignored record beside a new
   test, and the lead's instruction forbids ignoring a test to make a run pass. Recommendation: in place, with the old
   body kept in ADR 0669's amendment by its commit hash.
3. `journal::append` waits `LOCK_WAIT` = 2 s inside the edit lock, the same 2 s as `EDIT_LOCK_WAIT`: whenever the journal
   lock is busy for its whole wait, every waiter of the edit lock is refused. Whether the journal's wait should be
   shorter than the edit lock's belongs to ADR 0188; not changed here.
4. The gate runs only `edit_lock_f755` with the feature, so the controls CJ and CC are not run by it until `gate.yml`
   names `edit_lock_f855` too (the lead's file).

## What is not covered

* The gate's own event: its project folder was not kept, so no check here can say which candidate it was.
* A handle held on the journal lock or on a claim for longer than the 527 ms of the retries still leaves the file behind.
* Linux and macOS: CJ and CC are not compiled there (an open file can be unlinked); the series are run on Windows 11
  only.
* K0 is not an idle machine: other agents build on it.
* ADR 0669's L1 (200 runs of the `edit_lock_w48f` test) is not repeated.

## Amendment 1 (2026-11-21, after B0 to B3, the controls, A0, A1 and three runs of A2; before the corrected run of A2)

The after-series was started once as one background command (A0, A1, A2, A3, then AD). It was killed from outside during
A2: `series_after.log` ends at "A2 K2 run 3: ok 129.2 s", the last write in the scratch folder is of 13:49, and at 15:13
no process of the series, of its loads or of a test was alive (checked with the process list). No test failed or timed
out; the command's exit code was 4. What ran: A0 10 of 10 passed, A1 10 of 10 passed, A2 runs 1 to 3 passed (123.9,
141.5 and 129.2 s; journals without a line lacking `seq`). A3 and AD were not started.

A series cut off is not a series. The three results of the cut-off A2 are kept (`A2-cut` and `series_after_cut.log` in
the scratch folder) and reported as a deviation; they are not counted. Corrected run, registered now: **A2 again from
run 1**, 10 runs under K2, the same binary (`bin/f2_after.exe`, built at 0b75f63b), command and limit; then A3 and AD as
registered. The four files left by the dead disk load (`load-disk`, 256 MiB) were removed first, and no load process of
the dead series was running. A0 and A1 stand as run.

The project folder of the killed fourth run (`proj/rcfd-project-f751-f2-74452`) is left in the scratch folder: its
removal was refused by the tool's safety check and is not made another way. The run script no longer empties `proj` at
the start of a series (it still removes the folder of each run it made). Under K3 the reader of the lock files also
opens the files of that leftover folder, if any are there; they belong to no running test.

## Amendment 2 (2026-11-21, after A2, A3, AD, E1, E1f, G, MJ1 to MJ3 and the registered runs of M2r and M6r; before any corrected run)

The registered runs of M2r (one run) and M6r (three runs) were made on the scratch copy of the tip with
`RCFD_PROJECT_SCRATCH` on E:, as every run of this registration: F2 **passed all four** (10000 of 10000 with each
mutation). Both mutations survived these runs. That is a result and stays in the addendum.

The set-up differs from ADR 0669's in one thing that the mutations depend on: ADR 0669 ran its tests without
`RCFD_PROJECT_SCRATCH`, so the project lay in the temporary folder on C:, as on the gate's runner; here it lies on E:.
M2 and M6 are caught only when the first `resolve_in` of a probe call fails because of the racer's rename (the
canonical path of the replaced file under `\$Extend\$Deleted`, or delete-pending); how often that happens is a property
of the volume and of what else opens files on it, not of the code changed here (`paths.rs` is unchanged since ADR
0669, and changes A and B touch only the removal of two files). Whether the volume is the reason is not known; the
corrected runs say.

Corrected runs, registered now, each made once, all without `RCFD_PROJECT_SCRATCH` (the project in the default
temporary folder on C:), F2 of the default build, the scratch copy of the tip:

* **M2r-c**: M2, one run; caught when F2 fails.
* **M6r-c**: M6, at most three runs, stopped at the first failure; caught when one fails.
* **M2-base** (a control, only if M2r-c survives): M2 on a scratch copy of the base commit c547b175, one run; if it
  survives too, the survival does not come from changes A and B.
* **A0c** (added): F2 of the default build at the tip (`bin/f2_after.exe`), 10 runs under K0 with the project in the
  default temporary folder, by the gate's command; limit 10 of 10, source and kind as A0. It is the set-up of the gate,
  which the series on E: did not have.

If M2r-c or M6r-c survives, the verdict is partial by the registered rule (a mutation survives), and nothing is changed
to make it caught.

## Addendum 1: results (2026-11-21)

Commits: 4c735af1 (this registration, alone), 24cc2938 (the hook `journal_locked` and the tests F2d, CJ, CC; no change of
behaviour), 0b75f63b (changes A and B), 8dd25811 (the committed mutation MJ1), b7947f9c (amendment 1), ed535481
(amendment 2). Release builds, `CARGO_TARGET_DIR` in the scratch folder; Windows 11, 16 logical processors; the machine
was shared with other agents throughout. Logs in the scratch folder: `B0` to `B3`, `A0`, `A1`, `A2-cut`, `A2`, `A3`,
`AD3`, `A0c` (one log per run, `summary.tsv`), `controls_before.log`, `controls_after.log`, `e1.log`, `g1.txt`,
`mut_*.log`, `corrected.log`; scripts `loads.py`, `series.py`, `mutate.py`.

### The series before any change (F2d, library of 24cc2938)

| Series | Load | Failed runs of 10 | Refused probe calls and their class | Seconds per run |
|---|---|---|---|---|
| B0 | K0, none | 0 | none | 58.2 to 71.3 |
| B1 | K1, 16 busy processes | 0 | none | 48.3 to 57.2 |
| B2 | K2, 4 disk writers | 0 | none | 113.8 to 159.7 |
| B3 | K3, the reader of the lock files (a forced cause) | **1** (run 9) | 4, all class **J** | 63.1 to 71.2; the failed run 88.5 |

The failed run of B3 printed `conflicts=9996 refused=4 racer_ok=7665 racer_errs=0 no_seq=4 journal_lock_broken=2
remove_failed=0 stale_broken=0`, no file left in `.rcfd/locks`. The four refused calls (indices 6803, 6804, 6809 and
6814 of the 10000) each waited 2.001 to 2.002 s; no claim file existed after any of them; the edit lock file was the
racer's, 1.98 to 2.00 s old (one hold of the racer for the whole wait); `journal.lock` was 2.00, 4.00, 2.02 and 4.03 s
old. That is two events of candidate J as the arithmetic of the registration gives it: per event two refused probe
calls, two journal lines without `seq`, one line with `journal_lock_broken_s`, no racer error, about 10 s added. It is
the gate's print in every figure it has (9996 of 10000, 4 `LockBusy`, 0 racer errors, a longer run). The estimate of
the registration for B3 (10 of 10, about 8 events a run) was wrong by a factor of about 40: the reader met the journal
lock at the moment of its removal twice in ten runs (its counts over the series: 2617 polls, 38 opens of
`journal.lock`, most of them of the lock already left behind, 37 of a claim, 2033 of an edit lock).

No run under K0, K1 or K2 failed, and no refused call of class S, V, C or X was seen in any series. So the gate's
failure was **not reproduced without the forced load**; with 10 runs a rate of 1 in 6 would have shown with
probability 0.84 (the registration's own limit).

### The controls

| Check | Limit | Measured | Met |
|---|---|---|---|
| CJ-before (24cc2938) | fails | failed: the journal lock present when edit 1 returned (6.8 ms after the handle opened); edit 2 `Ok` after 2108.5 ms; the probe `LockBusy` after 2001.8 ms; 1 of 2 journal lines without `seq` | yes |
| CC-before (24cc2938) | fails | failed: the claim left; the second edit `LockBusy` after 2001.7 ms | yes |
| CJ (0b75f63b) | passes; edit 2 and the probe under 1.0 s | passed: edit 1 returned 337.0 ms after the handle opened, no journal lock; edit 2 in 107.8 ms; the probe `Conflict` in 108.5 ms; 0 lines without `seq` | yes |
| CC (0b75f63b) | passes; second edit under 1.0 s | passed: edit 1 in 396.7 ms, no claim left; edit 2 accepted in 6.0 ms | yes |

CJ-before is the gate's signature made on purpose: a writer whose removal of the journal lock fails once returns `Ok`
twice, and the other writer's call is refused with `LockBusy` after 2.0 s.

### The series after changes A and B (library of 0b75f63b)

| Check | Load | Limit | Measured | Met |
|---|---|---|---|---|
| A0 | K0 | 10 of 10 | 10 of 10 (56.5 to 62.0 s) | yes |
| A1 | K1 | 10 of 10 | 10 of 10 (49.6 to 59.9 s) | yes |
| A2 (corrected run, amendment 1) | K2 | 10 of 10 | 10 of 10 (117.0 to 156.3 s) | yes |
| A3 | K3 | 10 of 10 | 10 of 10 (63.4 to 113.2 s); the reader opened `journal.lock` 7 times and a claim 13 times in the series, and no journal has a line without `seq` | yes |
| AD (F2d under K3, the one load whose B-series failed) | K3 | 10 of 10; no line without `seq` | 10 of 10 (59.1 to 71.9 s), 0 refused calls, 0 lines without `seq`; the reader met no journal lock in this series (0 opens) | yes |
| A0c (amendment 2) | K0, project on C: | 10 of 10 | 10 of 10 (69.8 to 115.5 s) | yes |

In all 50 journals of A0 to A3 and AD every line has `seq`, none has `journal_lock_broken_s`, `edit_lock_remove_failed`
or `stale_lock_broken`, and no file was left in `.rcfd/locks`. What the K3 series can carry is small: at the rate of B3
(2 events in 10 runs) the unchanged library would also pass most runs, and AD met no event. The evidence that change A
removes the fault is CJ and MJ1, and the 7 opens of the journal lock in A3 without a line lacking `seq`.

### The other checks

| Check | Limit | Measured | Met |
|---|---|---|---|
| E1 | every test passes | `rcfd-project`: `edit_lock_f751` F1, F1c, F3a, F3b 4 of 4 and F2 in every run of A0 to A3 and A0c; `edit_lock_w48f` 4 of 4; `journal_f273` 4 of 4; `--lib` 4 of 4; `rcfd-workspace` `journal_f273` 9 of 9 | yes |
| E1f | every test passes | `edit_lock_f755` with the feature: 7 of 7 | yes |
| G | no `test-hooks` in the feature tree of the workspace; 0 tests in the default `edit_lock_f855` | 0 of 21 lines; `running 0 tests` | yes |
| FS | only with change C | not made (change C was not made) | n/a |
| M | each mutation caught | MJ1, MJ2, MJ3 caught; **M2r and M6r survived** (below) | **no** |
| P | 0 failures; clean; passes | 0 failures; `fmt_check.py rcfd-project` clean; clippy clean with and without the feature; `docs_site` passes | yes |

### Mutations

| | Mutation | Run against | Result |
|---|---|---|---|
| MJ1 (committed patch) | the removal of the journal lock not retried | CJ: lock left, edit 2 in 2118.9 ms, the probe `LockBusy` | caught |
| MJ2 | the removal of the claim not retried | CC: claim left, second edit `LockBusy` | caught |
| MJ3 | `journal_locked` never called | CJ: "the handle was not opened" | caught |
| M2r | M2 of ADR 0669 at the tip, project on E: | F2, one run: 10000 of 10000, passed | **survived** |
| M6r | M6 of ADR 0669 at the tip, project on E: | F2, three runs: passed each | **survived** |
| M2r-c (amendment 2) | M2 at the tip, project in the temporary folder on C: | F2, one run: passed | **survived** |
| M6r-c (amendment 2) | M6 at the tip, project on C: | F2, three runs: passed each | **survived** |
| M2-base (amendment 2, control) | M2 on the base commit c547b175 (no change of F-855), project on C: | F2, one run: passed | **survived** |

The unmutated scratch copy passed CJ and CC first. M2 and M6 are not caught by F2 on this machine today, on either
volume, and M2 is not caught on the base commit either, so the survival does not come from changes A and B. In ADR
0669 (2026-11-17, the same test, the same `paths.rs`) M2 refused 13 of 10000 probe calls and M6 one. Both depend on the
first `resolve_in` of a probe call failing because of the rename of the racer; that did not happen once in about 90000
probe calls of the mutation runs here. Why the race no longer shows on this machine was not looked into (the toolchain,
the system or its scanner may have changed since); it is open question 5. F2 is unchanged, so what it asserts about a
refused or failed probe is unchanged; what is not shown today is that it meets one.

### Deviations

1. The after-series was killed from outside in A2 after three runs (amendment 1, b7947f9c, committed before the
   corrected run). The three results (all passed) are kept and not counted; A2 was run again from run 1.
2. M2r and M6r survived their registered runs; corrected runs on C: by amendment 2 (ed535481, committed before them)
   survived too.
3. The A-series and A0c added `--nocapture` to the command of the gate so that the printed line of F2 would be kept;
   the pass rule is that of the gate. (libtest still dropped the line of a passed run; the journals were read instead.)
4. CJ, CC, E1 and E1f were run by `cargo test ... -- --test-threads=1` per test binary, not one process per test.
5. The estimate of the registration for the rate of events under K3 was about 40 times too high (above).
6. The removal of the project folder of the killed run in the scratch folder was refused by the safety check of the
   tool; the folder is left there (amendment 1), and under K3 in A3 and AD the reader also opened its one stale edit
   lock file at every poll (most of the 5979 and 4539 opens of an edit lock counted there).
7. Ten project folders of A0c and those of the corrected mutation runs are left in the temporary folder on C:, as every
   run of these tests without `RCFD_PROJECT_SCRATCH` leaves them.

### What was not run

Change C and its checks (FS, MS1): no run without the forced load showed class S. AD under K0, K1 and K2: their
B-series had no failure. L1 of ADR 0669. Anything on Linux or macOS.

### What the measurements say

* A fault of the library is shown by a control that fails before the change and passes after it: one failed removal
  of the journal lock (a handle without delete sharing for some milliseconds) made the next two appends wait 2 s each
  inside the edit lock, and a second cooperating writer was refused with `LockBusy` while the first had no error. In a
  running F2 a reader of the lock files produced the print of the gate exactly (run 9 of B3). The same fault in the
  removal of the claim (CC) refuses both writers and is not the signature of the gate; it is fixed with it.
* Whether the gate met this fault is not shown. Its project folder was not kept; under processor and disk load on this
  machine F2 did not fail in 20 runs, and without load in 10. The other candidate that fits the print, a holder stalled
  for 2 s by the runner (S), was not seen here and is not excluded; the F3a reading of the gate (5.77 s against 2.03 s)
  speaks for stalls of that size on the runner. If S is what the gate met, F2 will fail there again, and change C of
  this registration is the proposal for it.
* Defaults: an append and a claim whose removal succeeds at the first attempt do what they did; no constant, error or
  message changed; every ADR 0135, ADR 0669, ADR 0676 and ADR 0188 test named in E1 and E1f passed.

Size (agent), actual: partial; 0; 5; 6; 130000

### Decision by the registered rule

**partial**: no run of B0, B1 or B2 failed, so the cause of the failure of the gate is not reproduced (a forced control
shows a fault that fits its signature); and two mutations (M2r, M6r) survived. Changes A and B do not rest on the
repeated runs: each has its control, failing before and passing after, and its caught mutation.

## Open questions for the maintainer

None that rule 6 leaves open for the maintainer. For the lead, beyond the four of the registration:

5. M2 and M6 of ADR 0669 are no longer caught by F2 on this machine (also on the base commit). Options: (a) a follow-up
   that finds why the first `resolve_in` no longer fails under the rename of the racer here and gives the second
   resolve of D1 a forced control with the hooks, as ADR 0676 did for the claim; (b) leave it until the gate shows a
   refused path. Recommendation: (a).
6. Whether to make change C (F2 accepts a `LockBusy` whose whole wait lay inside one accepted call of the racer, with a
   clean journal) without a reproduction of class S. Recommendation: wait for the next failure of F2 in the gate; with
   changes A and B merged, a repeat would speak for S, and keeping the project folder of a failed test as a gate
   artifact (`gate.yml`, the file of the lead) would then say which it was.

## For the lead: roadmap entries

> Proposed status: partial.

> ADR 0751, 2026-11-21, partial: the gate failure of F2 (4 of 10000 probe calls LockBusy, racer without error) was not reproduced in 10 runs each without load, with 16 busy processes and with 4 disk writers; a reader of the lock files without delete sharing reproduced the print of the gate exactly in 1 of 10 runs, by a journal lock left behind when its one removal fails (the next two appends wait 2 s each inside the edit lock); the removal of the journal lock and of an edit claim is now retried as that of the edit lock is since ADR 0676, shown by the controls CJ and CC (fail before, pass after) and three caught mutations; F2 is unchanged and passed 10 of 10 under each load afterwards; M2 and M6 of ADR 0669 survived F2 on this machine, also on the base commit.

> gap: Not shown that the gate met the journal-lock fault (its project folder was not kept); a holder stalled 2 s by the runner is not excluded, and F2 is unchanged, so it can fail again in the gate. The mutations M2 and M6 of ADR 0669 are no longer caught by F2 on the agent machine (also on the base commit): the second resolve under the lock has no forced control. The controls CJ and CC (edit_lock_f855, feature test-hooks) are not run by the gate.

> Follow-up proposed: A forced control of the second resolve under the edit lock (ADR 0669 D1). Deps F-855. Acceptance: Every limit not stated here is registered in the ADR before the run. A test-only hook makes the first resolve of replace_if_hash fail as a concurrent rename does; with it a call without the second resolve (ADR 0669 M2) and one with the lexical key joined by the platform separator (M6) each fail a registered test in every run, and the unmutated library passes it; the default bits unchanged.

## The lead's answers (2026-11-21), after the merge

Merged at 26856a90 by `scripts/push_main.py` (22.9 minutes; the committed mutation MJ1 caught). The six questions are
not of scope or priority, so the lead decides them.

1. Changes A and B are inside the brief. A writer whose own removal fails retries for at most 527 ms, the trade ADR 0676
   made for the edit lock; no wait of another writer is lengthened, and a lock or claim left behind cost the others 2 s
   and more. Both stay.
2. and 6. Change C is not made. F2 stays as ADR 0669 registered it: without a reproduction of a stalled holder, a test
   that accepts a refused probe would hide the next occurrence. If F2 fails again in a gate with changes A and B merged,
   the question is opened again with that gate's evidence; in place or as a record is decided then.
3. The journal's wait of 2 s inside the edit lock's 2 s is noted and not changed. It belongs to ADR 0188 and has no
   failure of its own on record after change A; it is named in the follow-up below as a thing to measure, not to change.
4. The gate is to run `edit_lock_f855` with the feature beside `edit_lock_f755`, and is to keep the project folder of a
   failed edit-lock test as an artifact: roadmap item F-858.
5. Option (a): a forced control of the second resolve, so that M2 and M6 of ADR 0669 are caught again: roadmap item
   F-859. Until then the second resolve of ADR 0669 D1 has no test that fails without it on the lead's machine; that is
   said in the gap of F-855.

Status of F-855: partial, by the registered rule (the gate's failure was not reproduced without a forced cause; M2 and M6
survived). Finding on the push script, in F-851: the ledger's item field read "F-002" out of the branch name
(`...-f2-...`); the line is corrected to F-855 in the commit that adds this section.
