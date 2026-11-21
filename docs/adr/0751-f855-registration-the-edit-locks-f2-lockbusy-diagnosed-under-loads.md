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
