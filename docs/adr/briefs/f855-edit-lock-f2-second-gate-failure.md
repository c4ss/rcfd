# Brief: F-855, the edit lock's F2 failed in a second full gate after the fix of ADR 0751

Branch: f855-edit-lock-f2-second-gate-failure
Items: F-855
ADR: 0758
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-21
Size (lead): done; 0; 0; 2; 169395

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f855-edit-lock-f2-second-gate-failure`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f855-edit-lock-f2-second-gate-failure-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- crates/rcfd-project/tests/edit_lock_f751.rs 10007 (F2)
- crates/rcfd-project/src/atomic.rs 26472 (the edit lock, the claim, EDIT_LOCK_WAIT)
- crates/rcfd-project/src/journal.rs 6673 (the journal's lock, taken inside the edit lock)
- crates/rcfd-project/tests/edit_lock_f855.rs 12418 (the controls CJ and CC of ADR 0751)
- docs/adr/0751-f855-registration-the-edit-locks-f2-lockbusy-diagnosed-under-loads.md 45571 (the first diagnosis; read all of it, the addendum and the lead's answers above all)
- docs/adr/0669-f751-registration-the-two-failures-of-the-first-daily-gate.md 28688 (F2 from line 134; M2 and M6)
- docs/adr/0676-f755-registration-a-forced-control-of-the-edit-locks-starvation-and-of-a-lock-left-behind.md 26575 (the test hooks, S1, D1)

Also in the repository, as needed: `crates/rcfd-project/tests/edit_lock_f755.rs`, `.github/workflows/gate.yml` (read
only). Outside the repository, for reading: the artifacts of the failed gate in `E:/projects/cfd/w49i-work/gate-f6e68b4a`
(per shard `test.log`, `times.tsv`, `progress.log`; the attribution in the folder `gate-attribution-...-shard-2`), of the
first failure in `gate-e9378ecb`, and of the two gates between them in which F2 passed, `gate-a5c25ae8` and
`gate-62a6fec3`.

## What happened

ADR 0751 (F-855) diagnosed the failure of F2 in the gate of e9378ecb (4 of 10000 probe calls in `LockBusy`), found and
fixed a fault of the journal's lock, and said that F2 itself was unchanged and could fail again, because the gate's
failure could not be tied to that fault. It has: the full gate of main at `f6e68b4a` (run 44952361621, four shards,
2026-11-21) is red on this test alone, 5936 passed, 1 failed, 1727 ignored. Printed by the test:

    assertion `left == right` failed: F2: ["Err(LockBusy { path: ...\\rcfd-project-f751-f2-7180\\p\\cases\\c.txt, lock: ...\\p\\.rcfd\\locks\\e7204774bef2604e.lock })"]
      left: 9999
     right: 10000

One call of 10000, the same file and lock name as before, the binary 89.3 s. The fix of ADR 0751 is in that commit, so
the fault it fixed is not the cause of this failure, or not the only one. F2 passed in the gates of a5c25ae8 and 62a6fec3
between the two failures: two failures in four gates. The project folder of the failed test was again not kept (F-858 is
not done), so what the gate's artifacts hold is the test's output, the times of the shard and what ran beside it.

The maintainer's decision of 2026-11-21: this test is fixed before anything else is issued. While the newest gate line is
red the lead starts no other agent (ADR 0754), so the whole programme waits on this.

## Task

One registration, `docs/adr/0758-f855-registration-<slug>.md`, the first commit of this work, alone. It is a second
registration under F-855 and does not edit ADR 0751, ADR 0669 or ADR 0676; a change of what F2 asserts is an amendment of
ADR 0669 written in your ADR and named as such.

1. **Read the two failures together before any run.** What ran in the same shard at the time of F2 in each of the four
   gates (from `times.tsv` and `progress.log`), how long F2 took in each, and whether the two failed gates share
   something the two passing ones lack. Say plainly what the artifacts cannot tell.
2. **Name the candidate causes that survive ADR 0751** and a control for each, registered before the run: a probe that
   waits the whole of `EDIT_LOCK_WAIT` because the runner stalls the holder or the probe for longer than the wait (ADR
   0751 did not exclude a 2 s stall); the journal's wait of 2 s inside the edit lock's 2 s (question 3 of ADR 0751); a
   starved probe in spite of the claim; a lock or claim file that outlives its holder on the runner's file system
   (antivirus or indexer holding the file, a delayed delete on Windows). ADR 0751 could not reproduce the failure under
   four loads with 10 runs each: register a harder trial (more repeats, the load of the gate's own shard reproduced from
   its `times.tsv`, a forced stall through the test-only hooks) and report the counts whatever they are. Nothing is added
   to a default build; instrumentation goes through the feature `test-hooks` or a registered extension of it.
3. **Decide and register the change, then make it.** The acceptance of F-855 stands: the count of 10000 is not lowered,
   the test is not ignored, skipped or retried by the gate, and a refused, failed or starved probe must still fail a
   test. Within that, candidates and not instructions: a fault found by a control is fixed in `atomic.rs` or
   `journal.rs`; if the cause is a stall of the machine longer than the wait, then F2 asks more of the lock than the lock
   promises, and the registered amendment of F2 separates the two things it now mixes: the property (no cooperating
   writer's rename refuses or fails another writer's path) stays asserted on every call, and a call that ended in
   `LockBusy` is accepted only when a registered, measured fact shows the wait was consumed by a stall and not by the
   lock (for example the holder's own recorded hold times, or a monotonic clock gap across the call), with the number of
   such calls printed and bounded by a registered limit. A change that simply tolerates some number of `LockBusy` is not
   acceptable. If you find no cause and no measured fact that separates a stall from a fault, stop and report; do not
   loosen the test.
4. **Show it.** The registered number of repeats of F2 passes with and without the registered load; the forced control
   of your cause fails before the change and passes after; mutations show that the changed check still catches a refused
   probe, a failed probe and a starved probe (ADR 0669's M2 and M6 are the business of F-859 and are not asked here, but
   say whether your change touches them).
5. **Say what a gate will show.** One paragraph for the lead: after this change, what would a further F2 failure in a
   gate mean, and what the test prints then that it did not print before, so that the next failure can be read without
   the project folder.

Decision rule to register: done only if a cause is shown by a control that fails before and passes after, or the amended
F2 separates a stall from a fault by a measured fact and the mutations are caught, and the registered repeats pass;
partial if the change is made and the repeats pass but the cause of the gate's failure is not shown; stopped and reported
if neither a cause nor a separating fact is found.

Run: `cargo test -p rcfd-project --release` with and without `--features test-hooks`, the tests you register, and
`python scripts/process_report.py --check`. Files you own: `crates/rcfd-project/src/atomic.rs`, `journal.rs`, the
`edit_lock_*` tests and a new test file of your own. Another agent works at the same time in
`crates/rcfd-project/tests/listing_budget.rs` (F-866): do not touch it. Do not change `.github/workflows/gate.yml`
(F-858).

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json` or a generated page; you write your roadmap entries as quoted
paragraphs and as the proposal file the rule names, checked with `python scripts/roadmap_proposals.py check`. Use your own
target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`), and delete nothing outside your
worktree and your scratch folder.

## Report (under 400 words)

The registration's commit and the others; what the four gates' artifacts show; the cause, or that none was found, with
the counts of every registered trial; the change in one paragraph, and whether it amends F2; the control before and
after; the mutations; the paragraph of task 5 in two sentences; deviations; what was not run; open questions with
options and a recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered
rule.
