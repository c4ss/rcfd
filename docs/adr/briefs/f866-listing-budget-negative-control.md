# Brief: F-866, the negative control of listing_budget L1 fails just under the budget

Branch: f866-listing-budget-negative-control
Items: F-866
ADR: 0759
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-21
Size (lead): done; 0; 0; 1; 68387

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f866-listing-budget-negative-control`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f866-listing-budget-negative-control-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- crates/rcfd-project/tests/listing_budget.rs 3875 (the whole test; the control is at its end)
- docs/adr/0128-audit-fixes-runner-claude-project-tsstore-render-api.md 36783 (finding 12 at line 196 and amendment 3 at line 331)
- docs/adr/0137-linux-gate-platform-values.md 14738 (the rule applied per run, line 63 onwards, and its amendment)

Outside the repository, for reading: the artifacts of the failed gate in `E:/projects/cfd/w49i-work/gate-62a6fec3` (per
shard `test.log`, `times.tsv`) and of the gates in which the test passed, `gate-a5c25ae8` and `gate-f6e68b4a`.

## What happened

The full gate of main at `62a6fec3` (run 44942062298, four shards, 2026-11-21) was red on one test alone: `rcfd-project`
`listing_budget::l1_listing_hundreds_of_runs_within_budget`, with the message "negative control: the budget does not
catch a 10x slower listing". The test printed: worst of 3 listings 51.59 ms, budget 500 ms, the listing 10 times
498.15 ms. The gate's re-run passed, and the test passed in the gate before and the gate after.

The lead's reading of the test, to be checked by you and not taken on trust: the rule of ADR 0128 finding 12 as ADR 0137
applies it asserts the control when `worst * 10 > budget` and then asks that ten real listings together exceed the
budget. Ten listings cost about ten times the mean, not ten times the worst of three, so whenever the worst is above a
tenth of the budget and the mean is below it, the control fails although nothing is wrong with the listing or the
budget. On the gate's runner the listing is near 50 ms, which is exactly that band.

The maintainer's decision of 2026-11-21: this test and the edit lock's F2 are fixed before anything else is issued.

## Task

One registration, `docs/adr/0759-f866-registration-<slug>.md`, the first commit of this work, alone. A change of the
control's rule is an amendment of ADR 0128 finding 12 and of ADR 0137's reading of it, written in your ADR and named as
such; you do not edit those ADRs.

1. **Show the defect before changing anything.** A control, registered before it is run, that reproduces the failure
   from the gate's own numbers without depending on the speed of your machine (the decision of the control taken as a
   function of measured durations, or the listing slowed by a registered factor through a test-only means), failing
   before your change. Report from the three gates' logs how close the listing is to a tenth of the budget on the runner.
2. **Decide and register the control that replaces it.** What must hold afterwards, from the acceptance of F-866: the
   budget of 500 ms and the assertion that a listing stays within it are unchanged; the control still fails for a listing
   that is ten times slower than measured, shown by a mutation; it cannot fail on a machine of any speed when the listing
   and the budget are sound; the test is not ignored and not retried. Candidates, not instructions: decide and assert on
   the same quantity (the time of the ten listings measured, or ten times a registered statistic of the single listings);
   or make the control independent of the clock by checking the budget assertion against a duration multiplied by ten.
   Say why your choice still tests that the budget would catch a real slowdown, which is what finding 12 asked for, and
   what it no longer tests.
3. **Show it.** Your control of task 1 passes after the change; the test passes a registered number of repeats on your
   machine and under a registered load that brings the listing near a tenth of the budget; mutations: the budget raised
   tenfold, the listing made ten times slower, the control's condition inverted, each caught or reported.

Decision rule to register: done only if the control of task 1 fails before and passes after, the three mutations are
caught and the budget and its assertion are unchanged; partial if a mutation survives or the band could not be reproduced;
stopped and reported if no control can be given that both cannot fail on a sound listing and catches a tenfold slowdown.

Run: `cargo test -p rcfd-project --release --test listing_budget`, the debug build of the same test, and
`python scripts/process_report.py --check`. File you own: `crates/rcfd-project/tests/listing_budget.rs`. Another agent
works at the same time in `crates/rcfd-project/src/atomic.rs`, `journal.rs` and the `edit_lock_*` tests (F-855): do not
touch them, and change no file under `crates/rcfd-project/src/`; if the defect turns out to need a change there, stop and
report.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json` or a generated page; you write your roadmap entries as quoted
paragraphs and as the proposal file the rule names, checked with `python scripts/roadmap_proposals.py check`. Use your own
target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`), and delete nothing outside your
worktree and your scratch folder.

## Report (under 300 words)

The registration's commit and the others; whether the lead's reading of the defect holds; the control before and after;
the new rule in two sentences and what it no longer tests; the repeats and the mutations; deviations; what was not run;
open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your verdict
by the registered rule.
