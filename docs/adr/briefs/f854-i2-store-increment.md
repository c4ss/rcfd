# Brief: F-854, what I2 of monitors_filtered_f456 is to time

Branch: f854-i2-store-increment
Items: F-854
ADR: 0750
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-20
Size (lead): done; 0; 0; 1; 95138

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f854-i2-store-increment`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f854-i2-store-increment-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- docs/adr/0749-f854-registration-two-wall-clock-limits-measured-so-that-load-alone-does-not-fail-them.md 31356 (the loads L0, LS, LM, the diagnostic D-I2, the mutation MI1, question Q1)
- docs/adr/0353-f456-registration-monitors-get-builds-only-what-is-asked.md 29056 (I2 and its amendments 2 and 3)
- crates/rcfd-post/tests/monitors_filtered_f456.rs 5568
- crates/rcfd-post/src/monitors.rs 17255

Outside the repository, for reading: the gate artifacts in `E:/projects/cfd/w49i-work/gate-24a26223` and `gate-46cea4f8`.

## What happened

`monitors_filtered_f456::f456_i2_storing_an_increment_of_100_samples_at_1e5` failed three times on 2026-11-20 in full
gates on windows-latest: median 2.08 ms (run 44756958425), once in the unfinished scheduled run 44711018197, and 3.39 ms
(run 44801883205, where the gate's own re-run failed as well). ADR 0749 left the test as registered: its diagnostic D-I2
shows that about 99 percent of what I2 times is the reallocation of a cloned store without spare capacity, and that the
pushes alone take 0.013 ms without load and 0.035 ms under the memory load LM.

## The maintainer's answer

Asked by the lead on 2026-11-20 as question Q1 of ADR 0749, two options with a recommendation (time an increment of a
store that grew by pushes; leave I2 as it is). The maintainer answered on 2026-11-20: "Go with your recommendation". That
is option (b) of Q1: I2 times an increment of 100 samples per series of a store that grew by pushes, with the reallocation
outside the timer, limit 2 ms unchanged; the increment that crosses the end of the capacity is reported by a test without
a limit. The clause of F-456, "an increment of 100 samples at 1e5 stored in at most 2 ms", is read in that sense from now
on. Nothing else is approved: no other limit, test or acceptance changes.

## Task

A further registration under F-854 (rule 13): `docs/adr/0750-f854-registration-<slug>.md`, the first commit of this
work, alone. Its table of checks has every column rule 14 names, the kind among them; ADR 0749 lacked it and stopped its
merge. Run `python scripts/process_report.py --check` before your report; it must give 0 failures.

- Append to ADR 0749 and to ADR 0353, in the same commit as the registration or before it, a section "Answer of the
  maintainer (2026-11-20), written into this ADR by the lead's brief" with the paragraph above. Nothing else of either
  ADR is changed. The changed measurement of I2 is an amendment appended to ADR 0353, committed before the code.
- Follow rule 2 for the test: the present I2 is kept as a record of what it measured (decide and register its form
  under the rules: its body untouched, and whether it is an ignored record or a test that reports without a limit), and
  the new measurement is a new test. The capacity-crossing increment is measured and printed by a running test without a
  limit, with the reason in the test's comment. No other test of the file changes, and no library code changes unless
  the registration shows the test cannot be written without it; if so, stop and report.
- Show, with the loads L0, LS and LM of ADR 0749 and the same number of runs (30 each), the failures of the new test;
  and name a slowdown of `MonitorStore::push` that the new test must still catch, with its mutation committed beside
  the test (`# expect-fail:`). MI1 of ADR 0749 (a push whose cost grows with the stored length) is the candidate.
- The margin of the new limit will be large (the estimate is 0.013 to 0.035 ms against 2 ms). Say so in the table; the
  limit is the acceptance's and is not tightened here. Report the margin and what the smallest slowdown is that the test
  catches.

Decision rule to register: done only if the new test passes 30 of 30 under each of the three loads, the mutation is
caught, and the process check gives 0 failures; partial if a registered run was not made or the new test fails under a
load; stopped and reported if library code or any other test would have to change.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers.

## Report (under 350 words)

The commits; the form taken for the old I2 and why; failures of 30 under each load; the capacity-crossing increment as
measured; the mutation and the smallest slowdown caught; deviations; what was not run; your verdict by the registered rule.
