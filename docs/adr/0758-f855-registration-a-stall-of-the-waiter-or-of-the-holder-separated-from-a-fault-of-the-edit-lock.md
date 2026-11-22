# ADR 0758: F-855 registration: a stall of the waiter or of the holder separated from a fault of the edit lock, after F2 failed in a second gate

Status: **REGISTERED 2026-11-21, written before any code of F-855 under this registration and before any run of its
checks.** Before this text no library or test code of this work was written, nothing was built and no test was run. What
was done: reading (below), and one reading script over the gates' `times.tsv` files, kept in the scratch folder
(`readings.py`). Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f855-edit-lock-f2-second-gate-failure.md` and follows
`docs/process/agent-rules.md`, version 5. Branch `f855-edit-lock-f2-second-gate-failure` from main cd74f388, scratch
`w49i-work/f855-edit-lock-f2-second-gate-failure-scratch`. It is the second registration under F-855 (rule 13). Parent
records, none of them edited:
[ADR 0751](0751-f855-registration-the-edit-locks-f2-lockbusy-diagnosed-under-loads.md) (the first diagnosis, changes A
and B, the diagnostic F2d, its change C proposed and not made),
[ADR 0669](0669-f751-registration-the-two-failures-of-the-first-daily-gate.md) (F2, the claim D2) and
[ADR 0676](0676-f755-registration-a-forced-control-of-the-edit-locks-starvation-and-of-a-lock-left-behind.md) (the test
hooks). This text contains an **amendment of ADR 0669** (what F2 asserts), under that heading below.

Size (agent): done; 0; 5; 2; 200000

## What is asked

The acceptance of F-855 (the roadmap at cd74f388), clauses as ADR 0751 numbered them: **a1** every limit registered
here; **a2** the cause of a probe call that waits `EDIT_LOCK_WAIT` and ends in `LockBusy` shown by a registered control;
**a3** a registered number of repeated runs with and without a registered load; **a4** the code or the test changed so
that the registered repeats pass; **a5** a mutation shows that the check still catches a refused or failed probe; **a6**
the count stays 10000, F2 is not ignored, skipped or retried by the gate, and a change of what it asserts is an
amendment of ADR 0669. The brief adds: the two failures read together before any run; a control for each candidate that
survives ADR 0751; a harder trial; a refused, failed or starved probe must still fail a test; a `LockBusy` is accepted
only on a registered, measured fact, counted, printed and bounded; what a further failure in a gate would mean.

## What was read

`AGENTS.md`; `docs/process/agent-rules.md` (version 5, whole); the brief; `crates/rcfd-project/tests/edit_lock_f751.rs`,
`edit_lock_f755.rs`, `edit_lock_f855.rs` and `tests/common/mod.rs` (whole); `crates/rcfd-project/src/atomic.rs` and
`src/journal.rs` (whole), the doc of `ProjectError::LockBusy` in `src/lib.rs` ("Another cooperating writer held the
file's edit lock for the whole wait"); `crates/rcfd-project/Cargo.toml`; ADR 0751, ADR 0669 and ADR 0676 (whole); the
acceptance and gap of F-855, F-858 and F-859 in `docs/roadmap.json`; the lines of `.github/workflows/gate.yml` that name
`test-hooks`; ADR 0755 D1 (form 1). Of the gates e9378ecb, a5c25ae8, 62a6fec3 and f6e68b4a: `times.tsv` of all sixteen
shards, `progress.log` and `test.log` of the four shards that ran F2, `attribution.md` of f6e68b4a. Of the scratch
folder of ADR 0751: the printed `F2D run` lines of its series B0 to B3 and AD.

## Readings of the four gates (before any run)

F2 in each gate:

| Gate | Shard | F2 seconds | Outcome | Printed | Finished as | Beside it when it ended |
|---|---|---|---|---|---|---|
| e9378ecb | 3 | 112.93 | FAILED | probe 9996 of 10000, 4 `LockBusy`; racer 5395 accepted, 0 errors | 36 of 1484, at 0:42:06 | `f249_warm` (62.6 s), `seam_flux_w33a` (37.3 s) |
| a5c25ae8 | 4 | 75.24 | ok | nothing (libtest drops the output of a passed test) | 28 of 1483, at 0:50:31 | `piso_thin_gap_f299` (73.6 s), `cosim_f362` (93.4 s) |
| 62a6fec3 | 4 | 70.09 | ok | nothing | 26 of 1483, at 0:40:51 | `transient_sources` (108.7 s), `si_fbar_drift` (75.5 s) |
| f6e68b4a | 2 | 89.30 | FAILED | probe 9999 of 10000, 1 `LockBusy`; racer 6146 accepted, 0 errors | 25 of 1482, at 0:49:42 | `immersed_unsteady_force_f346` (running since the start of the step), `si_fbar` (57.6 s), then `overset_f368` (92.7 s) |

* In all four F2 ran in the first fifty minutes of the test step, among the slowest tests (`gate_test.py` starts those
  first), three processes at a time. The two tests beside it were single solver tests of 37 to 109 s and 7 to 351 MB in
  every case. Nothing in the names, times or memory of the neighbours is common to the two failed shards and absent from
  the two passing ones. The attribution of f6e68b4a re-ran F2 and it passed ("ok (flaky)").
* F2's time does not separate them either: ADR 0751 lists passes of 53.04 to 87.15 s in five earlier gates; the second
  failure took 89.30 s, of which 2.0 s is the refused call.
* The racer's rate is alike in both failures (5395 accepted in about 105 s without the four waits, 6146 in about 87 s:
  51 and 70 a second) and not far from the rate on the agents' machine (ADR 0751 B0: 6405 to 7915 in 58 to 71 s).
* Signs of a stalled process in a shard: for every test whose median time in the other three gates is under 1.0 s (about
  1100 a shard) the script counts those that took at least 0.5, 1.0 and 2.0 s longer than that median. e9378ecb: shard 3
  (the one of the failed F2) 2, 1 and 1, the largest 3.52 s (`mesh_f272::c8i`, at 0:58:58) beside F3a's 5.77 s against
  2.03 s (at 0:58:16); its other shards 0. a5c25ae8: one test over 0.5 s (0.79 s). 62a6fec3: shard 2 has 20, 5 and 0,
  the largest 1.93 s (F2 ran in shard 4 of that gate: 1, 0, 0). f6e68b4a: shard 2 (the failed F2) **0, 0 and 0**, the
  largest 0.46 s; its other shards 4, 2 and 2 over 0.5 s, none over 1.0 s.
* So: a process of a runner was delayed by 3.5 s and more in the shard of the first failure, sixteen minutes after F2
  ended, and delays of 1 to 2 s are seen in a shard of a passing gate. In the shard of the second failure no small test
  shows a delay.

What the artifacts cannot tell: where in the 89 s the refused call lay; whether the process, the racer or the disk
stalled at that moment (the small tests that could show a stall all run in the last minutes of a shard, and a delay of
2 s inside a test of a minute is not visible in its time); how long the racer held the lock; whether a claim, a lock or
the journal's lock was left; the journal. The project folder was not kept (F-858). The second failure cannot be the
journal lock left behind by one failed removal (change A of ADR 0751 is in f6e68b4a) unless a handle was held on it
longer than the 527 ms of the retries.

Readings of ADR 0751's own series, used below: over 50 runs of F2d on the agents' machine the longest probe call was
0.033 to 0.070 s without load and with 16 busy processes (0.130 s once), 0.130 to 0.259 s with four disk writers, and
2.002 s only in the one run with the forced journal fault. A wait is normally 6 ms and never came within a factor of
seven of `EDIT_LOCK_WAIT`; the claim (`EDIT_CLAIM_AFTER`, 50 ms) is rarely reached. A refused call is therefore an event
of another kind than a slow machine: something stood still for two seconds.

## Candidate causes that survive ADR 0751, and what the code says of each

The signature: one probe call refused after the whole wait, the racer without an error.

* **W, the waiter itself is stalled.** `acquire` measures its wait on the wall clock from `t0` and gives up at the first
  failed look at which `t0.elapsed() >= EDIT_LOCK_WAIT`. A waiter that looked once, found the lock busy, and was then not
  run for 2 s (the whole process or the whole virtual machine stopped, or its thread not scheduled) gives up at its
  second look if the lock is busy at that moment, although it looked twice and the lock may have been free in between.
  That is not "held the file's edit lock for the whole wait". In F2 the probe spends most of its time waiting (a call
  of 6 ms is a wait for one hold of the racer), and when the whole process stops while the racer holds the lock, the
  racer still holds it when both wake: arithmetic says a stop of the process of 2 s or more during F2 ends one probe
  call in `LockBusy` with a probability near the share of time the probe waits, and leaves the racer without error
  (the racer waits only for the probe's short holds). This is a fault of the lock's accounting, it fits the signature,
  and ADR 0751 did not test it: none of its loads stops a process. ADR 0751's readings and those above show stops of
  1.9 to 3.7 s on the runners.
* **H, the holder is stalled inside one hold** (ADR 0751's S): the racer's read, write with fsync, rename or journal
  append takes 2 s once while the probe keeps looking. The refusal is then what the lock promises, and F2 asks more
  than the lock promises. Not seen on the agents' machine in ADR 0751 (longest racer call 0.259 s under four disk
  writers); not excluded for the runner.
* **Q, the journal's wait inside the edit lock** (question 3 of ADR 0751): in F2 only the racer appends (a probe call
  ends in `Conflict` and logs nothing), so the journal's lock is busy for the racer only when it was left behind (J of
  ADR 0751, fixed for a handle of up to 527 ms) or when its file cannot be created for a time. A wait of the full
  `LOCK_WAIT` leaves a journal line without `seq`; a shorter one leaves no trace in the journal and lengthens one hold.
  From outside the library it is a case of H, and the amended F2 below separates the part it can: a journal lock that
  is older than the racer's edit lock was left before the hold began.
* **V, a starved probe in spite of the claim**: the lock changed hands during the wait. Needs the claim to fail (its
  creation refused, or its `metadata` failing in the racer's `claimed_by_other`). Stays a fault.
* **C, L, a claim or an edit lock that outlives its holder**: both refuse the racer too (ADR 0751), which the print
  excludes unless the file disappears within the racer's own wait. Stay faults.
* **K, the racer stalled while it holds a claim and not the lock**: the probe does not try while another writer's fresh
  claim exists. Needs the racer's thread alone to stand still for 2 s while the probe's thread runs. Not separable from
  C from outside; stays a fault (class C below).

No control can say which of these the gate met: that needs the project folder or the test's own record, which the gate
did not have. What can be done is to remove W from the lock, to let F2 recognise H by measured facts, and to make F2
print the record that tells the next failure apart.

## Design (fixed now)

### Change W: a step in which the waiter did not look counts for at most 100 ms of the wait (`atomic.rs`)

`acquire` keeps a second measure beside `t0`: `seen`, the sum over its looks at a busy lock (or at another writer's
fresh claim) of the time since the previous look, each term capped at a new constant `EDIT_LOCK_WAIT_STEP` = 100 ms. It
gives up with `LockBusy` when `seen >= EDIT_LOCK_WAIT`, where it now tests `t0.elapsed() >= EDIT_LOCK_WAIT`. Nothing
else changes: the claim is still made from `t0.elapsed() >= EDIT_CLAIM_AFTER`, the sleep is 2 ms, the stale ages,
`EDIT_LOCK_WAIT`, the error and its text are as they are.

Derivation (first principles). `LockBusy` states that the lock was held for the whole wait. The waiter knows the state
of the lock only at its looks. A look every few milliseconds over 2 s supports the statement; two looks 2 s apart do
not. So the wait is counted in observed time: a step between two looks counts in full while it is short and for a
bounded amount when it is long. The bound is `2 * EDIT_CLAIM_AFTER`: 50 ms is the time ADR 0669 D2 fixed as the longest
a waiter stays unannounced, and a step of more than twice that is longer than any step of a waiter that runs (2 ms of
sleep and three file operations; the adversarial delay of ADR 0676 S1, the slowest registered waiter, steps by about
52 ms, which is under the cap, so S1 keeps its times). Consequences, by arithmetic: when no step exceeds 100 ms, `seen`
equals `t0.elapsed()` at every test, so every call that is not stalled gives up at the same look as before;
`LockBusy` now means at least 20 failed looks, no two counted more than 100 ms apart; a waiter stopped for T seconds
returns up to T seconds later than before and is otherwise unchanged; `seen` is never more than the wall time, so no
call gives up earlier than before.

Effect on default results: no file operation, journal line, error kind, message or constant changes, and a call none of
whose steps exceeds 100 ms returns what it returned at the same look. What changes is the time at which a stalled waiter
gives up. That is a change of what a caller can observe in time only; it is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation) and put to the lead as question 1. `journal::lock` has the same
wall-clock wait and is not changed (its timeout drops a `seq`, it refuses nobody; ADR 0188).

### Amendment of ADR 0669: what F2 asserts

ADR 0669 registered F2 as "10000 of 10000 probe calls return `Conflict`; the racer has 0 errors". Two things are mixed
in it: the property the test is named for (no cooperating writer's rename refuses or fails another writer's path), and
the claim that the racer never holds the lock for `EDIT_LOCK_WAIT`, which is a property of the machine. From this
amendment on F2 (`f2_a_cooperating_writers_renames_never_refuse_or_fail_another_writers_path`, the same name, project,
racer and 10000 probe calls, not ignored) asserts:

1. every probe call ends in `Conflict` or in a `LockBusy` of class S below; any other result fails the test as before
   (a refused path, an I/O error, an accepted write);
2. the racer has 0 errors, as before;
3. the number of calls of class S is at most **4**, and is printed;
4. if there is any call of class S: every journal line has `seq`, and none has `journal_lock_broken_s` or the kind
   `edit_lock_remove_failed` or `stale_lock_broken`.

The record F2 keeps (monotonic clock, seconds since the test began): start and end of every probe call; start, end and
outcome of every racer call; at once after a probe call that is not `Conflict`, a snapshot, taken in this order:
whether a claim file exists, the text and the modification time of the edit lock file, the modification time of
`journal.lock`.

The class of a probe call that ended in `LockBusy`, with wait `[p0, p1]`, decided in this order:

| Class | Rule | Outcome |
|---|---|---|
| E | `p1 - p0` is less than `EDIT_LOCK_WAIT` (refused before the wait was over) | fails |
| J | `journal.lock` exists at the snapshot and was modified before the edit lock file, or exists at least 1.0 s old with no edit lock file (left before this hold) | fails |
| C | a claim file exists at the snapshot | fails |
| S | the edit lock file exists at the snapshot, its text names the writer `racer`, its age is at least `p1 - p0 - 0.1 s`; and one racer call began at `p0 + 0.1 s` at the latest, ended after `p1` and returned `Ok` | accepted, counted |
| V | anything else (the lock changed hands, was free, was not the racer's, or the racer's call failed) | fails |

The measured facts behind S: the lock file that refused the probe was created once, by the racer, before the probe had
waited 100 ms, and was still there after the wait (the file's own time and text); the racer's own clock says it was
inside one call from then until after the refusal, and that the call ended as an accepted write; and the journal shows
that no lock was left or broken in the run. Together: one hold of a cooperating writer lasted the whole wait and ended
well, which is the case the doc of `LockBusy` describes. 100 ms is `EDIT_LOCK_WAIT_STEP` (and ADR 0751's figure for
the same rule): until the probe has claimed at 50 ms the racer may take the lock again (ADR 0669 D2); a hold that
began later than that is a failure of the claim, class V. What S does not show is why the hold was long (the disk, the
scheduler, or the library's own bounded retries of a rename or of a removal inside the hold); the record prints the
hold's length beside the longest other racer calls.

The limit 4: the largest number of refused calls a gate has shown in one run (e9378ecb, ADR 0751, "Readings of the
gate's artifacts": 9996 of 10000). A run with more is failed whatever their class: the machine then stood still for
more than 10 s of a test of about 80, and the run says nothing about the lock.

A waiter stall (W) is not a class: with change W it no longer ends in `LockBusy`. F2 also runs a ticker thread (5 ms
sleeps) that records every gap of 0.25 s or more between two of its wake-ups; the gaps are printed and are not part of
the verdict. F2 prints, with or without a failure: one line for every `LockBusy` call (index, class, wait, `p0`, claim,
lock text and age, journal lock age, the number of racer calls that ended in the wait, the racer call that spans it),
and one line for the run (conflicts, calls by class, other results, racer accepted and errors, journal lines and the
four journal counts, files left in `.rcfd/locks`, the longest probe call, the three longest racer calls with their
start, the ticker's gaps, the total time). On a failure the assertion's message holds the same two kinds of line, so
the gate's log has them.

Form (rule 2 and the brief together): F2 is changed in place and stays in the gate, as the brief and the acceptance
ask; the body ADR 0669 registered is kept in the same file as an ignored record,
`f2_adr_0669_body_known_miss`, unchanged but for the name of its scratch folder, with the two gate results in its
comment (9996 and 9999 of 10000). Question 2 for the lead.

The record, the classes and the verdict are one module, `crates/rcfd-project/tests/edit_lock_f2/mod.rs`, included by
`edit_lock_f751.rs` and by the new control file with `#[path]` (a directory, so it is no test binary of its own). Its
entry takes the number of probe calls and two closures run once on the racer's and on the probe's thread before their
loops (empty in F2; the controls set their thread-local hooks there), and a verdict takes the rule: `Adr0669` (every
call `Conflict`) or `Adr0758` (the four clauses above). No hook is added to the library: the controls use
`before_attempt`, `after_lock` and `claim_off` of ADR 0676.

### The forced controls (new file `crates/rcfd-project/tests/edit_lock_f855_stall.rs`, feature `test-hooks`)

Its leading comment names ADR 0758 and F-855. All three run on every platform.

* **CW** (`cw_a_waiter_stalled_past_the_wait_is_not_refused_at_its_second_look`). A lock file of `notes/a.md` made by
  hand. A writer calls `replace_if_hash`; its `before_attempt` hook, at its second call, tells the test and sleeps
  2.2 s (the stall). The test, told: waits 100 ms, removes the lock (free for the next 1.8 s), creates it again 1.9 s
  after the signal, and removes it for good 100 ms after the hook's third call (or after 3 s without one). Arithmetic
  for the unchanged library: the second look, 2.2 s after `t0`, finds the lock busy (re-created 0.3 s earlier) and
  `t0.elapsed()` is over 2 s: `LockBusy` after two looks, the lock having been free for 1.8 s of the "wait". With
  change W: `seen` is about 0.1 s at the second look, the writer goes on, claims, and gets the lock when it is removed:
  `Ok` about 2.35 s after `t0`. Limits: the edit `Ok`; under 3.2 s (the stall of 2.2 s and the 1.0 s of ADR 0669 F1
  for a waiter that is served); at least three looks; no claim file left. Run once on the commit before change W
  (expected to fail) and after it.
* **CH** (`ch_a_holder_stalled_for_the_whole_wait_is_class_s`). The F2 module with 300 probe calls; the racer's
  `after_lock` hook sleeps 2.4 s once, at its 20th hold. Expected by arithmetic: exactly one probe call waits 2.0 s and
  is refused while the racer holds; the snapshot finds the racer's lock, 2.0 s old; the racer's call spans the wait and
  is accepted. Limits: exactly 1 `LockBusy` call, of class S; 299 `Conflict`; the verdict by `Adr0669` fails (the
  negative control, inside the test as S1 and D1 of ADR 0676 are) and the verdict by `Adr0758` passes with count 1.
* **CV** (`cv_a_starved_probe_is_class_v_and_fails`). The F2 module with 200 probe calls. The racer's `after_lock` hook
  publishes the start of each hold and sleeps 30 ms in it (so that a hold is long enough to aim at). The probe's thread
  has `claim_off` (it makes no claim, ADR 0669 M1) and a `before_attempt` hook that, for 6 s from its 100th call on,
  returns only while the racer is in the first 10 ms of a hold (or after 80 ms, under the cap of change W): the probe
  looks only while the racer holds, the racer goes on writing, the lock changes hands about sixty times in each wait.
  This is the forced schedule of ADR 0676 S1 with the library's own writer as the holder. Limits: at least 1
  `LockBusy` call; every one of class V; the verdict by `Adr0758` fails.
* **FS** (`fs_the_classes_and_the_verdict_over_made_up_records`, in `edit_lock_f751.rs`, default build, so the gate runs
  it): over records written by hand, the class and the verdict by `Adr0758` of: a racer hold over the whole wait with
  the racer's lock (S, passes, count 1); five of them (fails, limit 4); the same with one journal line without `seq`
  (fails); a hold that begins 150 ms into the wait (V); a racer call that ends inside the wait (V); a racer call that
  failed (V); the lock text of another writer (V); no lock file (V); a claim present (C); a wait of 0.5 s (E); a
  journal lock modified before the edit lock (J); a result that is neither `Conflict` nor `LockBusy` (fails); one racer
  error (fails); and 10 calls all `Conflict` (passes, count 0).

### The harder trial: loads and series

Scripts in the scratch folder (`series.py`, `loads.py`; Python 3.9, standard library and `ctypes`). Every run is the
gate's per-test command with `--nocapture` added so that the printed lines are kept (`<exe> <name> --exact
--test-threads=1 --nocapture`, from the package directory with `CARGO_MANIFEST_DIR` set), counted by the gate's rule.
The project is in the default temporary folder on C:, as on the runner (no `RCFD_PROJECT_SCRATCH`).

* **K0**: no load started by this work. The machine is shared with other agents; that is not controlled.
* **K5, the shard**: the gate ran F2 as one of three test processes on a runner of four processors, the other two being
  solver tests that compute for a minute (the table above). Here: the test process and four endless arithmetic loops
  (one process each) are all bound to the same four logical processors. Four loops and not two, because a solver test
  may use more than one thread; the processors are then full, as ADR 0751's K1 made them, but on the runner's count.
* **K4, a stopped process** (a forced cause, named as such wherever its counts are given): a script starts the test
  and, from 8 s on, every 12 s suspends the whole test process for 2.5 s and resumes it (`NtSuspendProcess` and
  `NtResumeProcess` on the child's own handle). 2.5 s is above `EDIT_LOCK_WAIT` (2 s) and below `EDIT_CLAIM_STALE`
  (4 s) and the journal's `LOCK_STALE` (5 s), so that no stale rule acts. It is the model of a runner that stops a
  process or the whole machine; it needs no hook and acts on the default build. About six stops in a run.

Series, each run made once. "Old F2" is F2 of the test binary built at this registration's commit (the body of ADR
0669, the library before change W); "new F2" is the amended F2 at the tip.

* **BK5**: old F2, 10 runs under K5.
* **BK4**: old F2, 5 runs under K4.
* **AK0**: new F2, 20 runs under K0. **AK5**: new F2, 20 runs under K5. **AK4**: new F2, 10 runs under K4.

A series under K4 and one under K5 may run at the same time on disjoint sets of four processors; AK0 runs with no
other series of this work. No series is run under K0 before the change: ADR 0751 made 40 such runs of the same body
(B0, A0, A0c, and B1 to B2 under heavier loads than the gate's) without a refused call, and ten more would add nothing.
The counts 20 and 10: ADR 0751 used 10 a series; twice that for the two series that carry clause a3, in about 25
minutes each on a shared machine. With 20 runs a failure rate of 1 in 4 (the gate's two in four) shows at least once
with probability 0.997 and one of 1 in 20 with 0.64. That limit of the series is known beforehand.

## Table of checks

Release builds, `CARGO_TARGET_DIR=<scratch>/target`. "The brief" is
`docs/adr/briefs/f855-edit-lock-f2-second-gate-failure.md`.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| BK5 | old F2 before change W, 10 runs under K5: failed runs and what each printed | none (reported) | the brief ("report the counts whatever they are") | 0 of 10 fail | n/a | code verification (a diagnosis without a limit) |
| BK4 | old F2 before change W, 5 runs under K4: failed runs and what each printed | none (reported); W counts as reproduced by the forced cause when at least one run fails with only `LockBusy` among the probe's other results | the brief ("a forced stall", "report the counts whatever they are") | 5 of 5 fail, 3 to 6 `LockBusy` a run, racer errors 0 or few | n/a | code verification (negative control) |
| CW-before | CW on the commit before change W | fails with `LockBusy` | the brief ("the forced control of your cause fails before the change") | fails after about 2.2 s | n/a | code verification (negative control) |
| CW | CW after change W | `Ok` in under 3.2 s; at least 3 looks; no claim left | 1.0 s after the stall: ADR 0669 F1 (a waiter served in under 1.0 s) | about 2.35 s | 1.36 | code verification |
| CH | the holder stalled 2.4 s once | exactly 1 `LockBusy`, class S; the verdict by `Adr0669` fails and by `Adr0758` passes with count 1 | the amendment of ADR 0669 in this registration; the brief ("a call that ended in `LockBusy` is accepted only when a registered, measured fact shows the wait was consumed by a stall") | as limited | n/a | code verification |
| CV | the probe starved by a forced schedule with the claim off | at least 1 `LockBusy`; all of class V; the verdict by `Adr0758` fails | the brief ("a refused, failed or starved probe must still fail a test") | 2 or 3 calls, class V | n/a | code verification |
| FS | the classes and the verdict over made-up records | every case as listed above | the amendment of ADR 0669 in this registration | passes | n/a | code verification |
| AK0 | new F2, 20 runs under K0 | 20 of 20 pass | the acceptance of F-855 ("the registered number of repeats passes") | 20 of 20, no class S | n/a | code verification |
| AK5 | new F2, 20 runs under K5 | 20 of 20 pass | the acceptance of F-855, the same | 20 of 20, no class S | n/a | code verification |
| AK4 | new F2, 10 runs under K4 | 10 of 10 pass; no `LockBusy` call of any class | the acceptance of F-855, the same; the brief ("fails before the change and passes after") | 10 of 10 | n/a | code verification |
| S-count | calls of class S in one run of F2 | at most 4 | ADR 0751, "Readings of the gate's artifacts" (the most refused calls a gate has shown, 4 of 10000) | 0 | n/a | code verification |
| E1 | `cargo test -p rcfd-project --release` (default build, every test of the crate once) | every test passes with its own limits | the brief ("Run") | all pass | n/a | unchanged default output |
| E1f | `cargo test -p rcfd-project --release --features test-hooks` | every test passes (ADR 0676 S1, S2, D1 to D5; ADR 0751 CJ, CC; CW, CH, CV) | the brief ("Run") | all pass | n/a | unchanged default output |
| G | `cargo tree --workspace -e features -i rcfd-project` has no `test-hooks`; the default build of `edit_lock_f855_stall` lists 0 tests | holds | ADR 0676 G1, G3; the brief ("Nothing is added to a default build") | holds | n/a | unchanged default output |
| M | the mutations below | each caught by the named test | rule 16; the brief (a refused, a failed and a starved probe) | all caught | n/a | code verification |
| P | `python scripts/process_report.py --check`; `cargo fmt -p rcfd-project --check`; `cargo clippy -p rcfd-project --all-targets -- -D warnings`, and with `--features test-hooks`; `rcfd-cli` `docs_site`; `python scripts/roadmap_proposals.py check` | 0 failures; clean; passes | rule 7, rule 9 | clean | n/a | code verification |

## Mutations (on a scratch copy of the committed tree, its own target folder)

* **MW1** (committed as `crates/rcfd-project/tests/edit_lock_f855_stall.mutation.patch`, first line `# expect-fail:
  cargo test --release -p rcfd-project --features test-hooks --test edit_lock_f855_stall`): the step is not capped
  (`seen` is the wall time, the code before change W). Expected: CW fails.
* **ME1** (a refused probe): `acquire` gives up at its first look at a busy lock. Expected: the new F2 fails, its
  refused calls of class E; one run.
* **MS1** (a starved probe): the class of every `LockBusy` call is S. Expected: CV fails, FS fails.
* **MS2** (a failed probe): the verdict by `Adr0758` does not look at results that are neither `Conflict` nor
  `LockBusy`. Expected: FS fails.
* **MS3**: the limit of 4 is not applied. Expected: FS fails.
* **MS4**: the journal is not looked at. Expected: FS fails.

ADR 0669's M2 and M6 are not run (F-859). This work does not touch what they mutate (`paths.rs`, the second resolve and
its key in `replace_if_hash_with`), and the amended F2 fails on a refused path as the old one did (clause 1); whether F2
meets one on a given machine is F-859's question, unchanged.

## Decision rule

The brief's, with its terms fixed:

* **done**: CW fails before change W and passes after it, and BK4 reproduces the print of the gate by the forced stop
  and AK4 passes (a cause shown by a control that fails before and passes after); or, without that, CH, CV and FS are
  met (the amended F2 separates a stall from a fault by measured facts); and in either case every mutation is caught,
  every run of AK0, AK5 and AK4 passes, and E1, E1f, G and P are met. That the gate's own two events were W or H is not
  shown by any of this and is said so in the addendum; it does not by itself make the work partial, because the rule
  of the brief asks for a cause shown by a control or for the separation.
* **partial**: the changes are made and AK0 and AK5 pass, but CW-before does not fail or BK4 has no failed run, and CH,
  CV or FS is not met; or a mutation survives; or a registered run was not made; or a run of an A-series fails with a
  record that names its class.
* **stopped and reported**: a run of an A-series fails with a `LockBusy` of a class the record cannot place, or a
  further change of the lock's waits or of F2's rule would be needed; nothing is loosened then.

## Open questions (for the lead; none of scope or priority)

1. Change W lets a stalled waiter return later than `EDIT_LOCK_WAIT` on the wall clock (by the length of its stalls
   beyond 100 ms each). Options: (a) keep it; (b) leave the library as it is and let F2 accept a refusal when the
   ticker shows a stop of the process over the wait. Recommendation: (a): the refusal after two looks is wrong for a
   user's process too (a laptop that slept), and (b) would accept a `LockBusy` on a weaker fact.
2. F2 is amended in place and its old body kept as an ignored record. Options: (a) as registered; (b) in place without
   the record. Recommendation: (a), rule 2 and the acceptance are both met.

## What is not covered

* Which candidate the gates met: not shown, and not showable from their artifacts.
* A holder stalled while the snapshot is late (the racer lets go between the refusal and the snapshot): class V, a
  failed test, although it was a stall. The window is the microseconds between the two.
* A wait of the journal's lock inside the hold that is shorter than `LOCK_WAIT` and whose lock file is gone at the
  snapshot is accepted as class S: from a default build it cannot be told from a slow disk. `journal::lock` is not
  changed.
* A thread of the racer stalled alone while it holds a claim (K) fails the test as class C.
* The printed lines of a passing F2 are dropped by libtest in the gate; the count of class S calls of a passing gate
  run is therefore not in its log (F-858 could keep it).
* The controls CW, CH and CV are not run by the gate until `gate.yml` names their file (F-858, the lead's file).
* Linux and macOS: nothing here is Windows-only, but only Windows 11 is measured. K4 uses a Windows call.

## Amendment 1 (2026-11-22, after BK4, BK5, CW-before, CW, CH, CV, FS, E1, E1f, G and the mutations but ME1; before any run of BK6 or AK6)

What happened, as far as it bears on what is registered now (the full results are in the addendum):

* The first start of BK5 and BK4 ended before any test ran: `series.py` gave the test binary by a relative path with
  another working directory (`FileNotFoundError`). The script was corrected and the two series were started again from
  run 1. No test had started, so no run was made twice.
* BK4 (the forced stop): 5 of 5 runs failed, with 3, 4, 3, 3 and 3 probe calls in `LockBusy` and 1, 0, 0, 2 and 2 racer
  errors. Run 2 printed `probe 9996 of 10000 conflicts, 4 other`, `racer 4201 accepted, 0 errors`: the print of the gate
  of e9378ecb.
* BK5 (the shard's load): **run 5 of 10 failed**, `probe 9999 of 10000 conflicts, 1 other` (`LockBusy`), `racer 3152
  accepted, 0 errors`, 125.8 s: the print of the gate of f6e68b4a, without a forced cause. On this machine the project
  folder is kept. Its journal has 3152 lines, every one with `seq`, none of another kind than `edit`, no file left in
  `.rcfd/locks`; the times of consecutive lines are 0.029 s apart in the median and three times far apart: 1.32 s (at
  00:04:36), 1.50 s (00:05:37) and **2.38 s** (00:06:02 to 00:06:04). In the nine passing runs the largest distance is
  0.23 to 0.91 s. So in the failed run the racer wrote nothing for 2.4 s once, and nothing was left or broken.
* The series was not alone on the machine, against what the registration intends for a series: while BK5 ran, this
  work also built and ran the mutation copy (release builds of the crate's test binaries, 00:04:30 to 00:05:43, all
  processors) and built the 20 default test binaries of E1 (from about 00:06:00, bound to processors 0 to 7). The three
  standstills of run 5 fall into those builds. The other agents' work on the machine is not known.

The old F2 keeps no record, so run 5 does not say whether the probe kept looking while the racer stood still (H) or
stood still with it (W). The load that produced it was not the registered K5 alone. Both are corrected by two further
series, registered now, each run made once:

* **K6, the shard's load and a build**: K5, and beside it a loop that rebuilds and links the test binaries of
  `rcfd-project` in release in the scratch copy (`cargo test -p rcfd-project --release --no-run` after a changed comment
  in `src/time.rs`, again and again until the series ends), bound to processors 0 to 7.
* **BK6**: the amended F2 with its record on the **library before change W** (the test binary of `edit_lock_f751`
  built from commit 10ddf63e), 10 runs under K6. No limit; reported: failed runs, the class of every refused call, the
  ticker's gaps, the journal counts. It is a diagnosis: with the record, a refused call under the old library is
  class S whether the probe looked or not, so what it adds is the ticker and the snapshot.
* **AK6**: the amended F2 at the tip, 10 runs under K6. Limit: 10 of 10 pass (source: the acceptance of F-855, "the
  registered number of repeats passes"); the number of calls of class S of every run is reported. Kind: code
  verification.

BK6 and AK6 run at the same time under one build loop, each on its own four processors with its own four busy loops,
so that the two libraries meet the same minutes of the machine. Read together: refused calls in BK6 that are absent in
AK6 speak for W (the probe stood still too, and change W removed the refusal); calls of class S in AK6 speak for H.
Ten runs each cannot settle a rate of one in ten; that is known beforehand.

The decision rule gains AK6 beside AK0, AK5 and AK4 ("every run passes"). AK5 and AK4 as registered are running while
this is written; no build is started until AK5 has ended, and AK0 runs after BK6 and AK6 with nothing else of this work.
