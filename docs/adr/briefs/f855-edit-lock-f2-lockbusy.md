# Brief: F-855, the edit lock's F2 failed once in a full gate with four probe calls in LockBusy

Branch: f855-edit-lock-f2-lockbusy
Items: F-855
ADR: 0751
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-21
Size (lead): done; 0; 0; 1; 118641

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f855-edit-lock-f2-lockbusy`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f855-edit-lock-f2-lockbusy-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- crates/rcfd-project/tests/edit_lock_f751.rs 10007
- crates/rcfd-project/src/atomic.rs 25347
- docs/adr/0669-f751-registration-the-two-failures-of-the-first-daily-gate.md 28688 (F2 at line 134 onwards; amendment 1 on a lock left behind)
- docs/adr/0676-f755-registration-a-forced-control-of-the-edit-locks-starvation-and-of-a-lock-left-behind.md 26575 (the test hooks, S1, D1)
- .github/workflows/gate.yml 16121 (how the gate runs the tests and how many at once; read only, not to be changed)

Also in the repository, as needed: `crates/rcfd-project/tests/edit_lock_f755.rs` (the feature-gated controls). Outside the
repository, for reading: the artifacts of the gate in `E:/projects/cfd/w49i-work/gate-e9378ecb` (per shard `test.log`,
`times.tsv`; the attribution in the folder `gate-attribution-...-shard-3`); for comparison the gates in which F2 passed:
`gate-46cea4f8`, `gate-24a26223`, `gate-5637150a`, `gate-a9570de6`, `gate-5819a15c` in the same folder.

## What happened

The full gate of main at `e9378ecb` (run 44828624630, four shards, 2026-11-21) is red on one test alone: `rcfd-project`
`edit_lock_f751::f2_a_cooperating_writers_renames_never_refuse_or_fail_another_writers_path` (ADR 0669 F2: 10000 of 10000
probe calls return `Conflict`; the racer has 0 errors). Printed by the test:

    F2: probe 9996 of 10000 conflicts, 4 other (first Some("Err(LockBusy { path: ...cases\\c.txt, lock: ...\\.rcfd\\locks\\e7204774bef2604e.lock })")); racer 5395 accepted, 0 errors (first None)

All four are `LockBusy` on the same file. The binary took 112.9 s (ADR 0676 E1 records 68.4 s for F2 on the agent's
machine). The gate's own re-run passed (the attribution calls it flaky), and the test passed in the five other four-shard
gates named above. A `LockBusy` means that the probe waited `EDIT_LOCK_WAIT` (2 s) for the lock while the racer took and
released it continuously. ADR 0676 showed that the old acquire can starve under a forced delay and that the claim of ADR
0669 does not under that hook's scheduling; it did not show how often either happens on the gate's runner. Whether this
failure is a starved probe in spite of the claim, a lock left behind for more than 2 s, a stall of the runner longer than
2 s inside one call, or something else, is not known. It is yours to show.

## Task

One registration, `docs/adr/0751-f855-registration-<slug>.md`, the first commit of this work, alone.

- Show the cause before changing anything. Register a number of repeated runs of F2 with nothing else running, and the
  same number under a registered, reproducible load (for example a fixed number of processes that keep every core busy, and
  a second load that keeps the disk of the scratch folder busy), with the number of failures and, for each failed call,
  what the logs and the hooks of ADR 0676 can tell: how long the call waited, whether a claim file existed, whether the
  lock file outlived its holder, whether `edit_lock_remove_failed` was logged. Instrument through the existing test-only
  feature or a registered extension of it; nothing is added to a default build. If F2 does not fail under any load you
  can register, say so and report the counts; do not invent a cause.
- Decide and register what changes. Candidates, not instructions: a fault of the claim or of the removal that the control
  exposes is fixed in `atomic.rs`; a wait that is too short for a stalled runner is a question of `EDIT_LOCK_WAIT` and
  belongs to the ADR that owns it; a test that asks more than the lock promises (a probe may be refused after the wait
  when the machine stalls) is an amendment of ADR 0669's F2. The count of 10000 is not lowered. A change after which a
  refused, failed or starved probe would no longer fail the test is not acceptable: the registration names the defects
  the changed check must still catch, and mutations show it (ADR 0669's M2 and M6 were caught by F2 alone and must stay
  caught), each with its committed patch and `# expect-fail:` line as the rules ask.
- A change of what F2 asserts is an amendment appended to ADR 0669 (append-only, nothing above it changed), committed
  before the code. A change of the default behaviour of the lock (the wait, the claim, the removal) is registered with
  its check that every ADR 0135, ADR 0669 and ADR 0676 test keeps its result.
- The test is not ignored, skipped, removed or retried by the gate to make it pass, and `gate.yml` is not changed. If
  the honest result is that the cause cannot be reproduced and nothing can be changed with a reason, that is a finding:
  report it with the counts and leave the test as it is.
- Run the changed test the registered number of times in release without load and under each registered load, and
  report every result.

Decision rule to register: done only if the cause is shown by a control that fails before the change and passes after
it, F2 passes every registered repeat with and without the loads, and every mutation is caught; partial if the cause is
not reproduced, or a registered run was not made, or the change rests on the repeated runs alone; stopped and reported
if anything outside `crates/rcfd-project` and its ADRs would have to change, or if the change would alter what another
writer of a project file observes (a longer wait, another error) beyond what ADR 0669 and ADR 0676 registered: that is a
question for the lead, written into the registration under "Open questions", not decided by you.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Run in the background anything that takes longer than a few minutes.

## Report (under 400 words)

The registration's commit, any amendment and the code commits; the failures out of the repeats before and after, with
and without each load; the cause and the control that shows it, or that it was not reproduced; what changed and why it
still catches a refused or starved probe; mutations; deviations, each with the commit that precedes its corrected run;
what was not run; open questions; your verdict by the registered rule.
