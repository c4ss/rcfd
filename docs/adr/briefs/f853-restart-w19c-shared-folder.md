# Brief: F-853, three tests of restart_w19c share one temporary folder

Branch: f853-restart-w19c-shared-folder
Items: F-853
ADR: 0753
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-21
Size (lead): done; 0; 0; 1; 24052

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f853-restart-w19c-shared-folder`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f853-restart-w19c-shared-folder-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- crates/rcfd-api/tests/restart_w19c.rs 12149

Also in the repository, as needed: `docs/adr/0031-wave19c-restart-chain.md` (what S5 registers).

## What happened

At the merge of F-471 (2026-11-19) `s5_coarse_to_fine_iteration_counts_are_reported` failed once with "The system cannot
find the file specified" at `load_restart` after 0.73 s; 8 of 8 repeats passed. `s5_numbers()` (line 309) is called by
three tests of one process; it takes `tmp("s5")` (line 36), which is keyed by the process id alone and removes the folder
at each call. The defect is in the test and was on main before F-471.

## Task

One registration, `docs/adr/0753-f853-registration-<slug>.md`, the first commit of this work, alone.

- Show the failure before the change: a registered number of repeated runs of the file's three S5 tests in parallel, or a
  forced control, that fails on the base commit. If the registered repeats do not fail, say so with the count and register
  a forced control before going on.
- Change the test file alone so that no two tests of the file use one folder. Look at every other call of `tmp` in the
  file for the same fault and say what you found. No assertion and no number of the file changes; no library code
  changes; `baseline_hashes` is not touched.
- Run the registered repeats after the change and report every result. One mutation with its committed patch and
  `# expect-fail:` line: the shared folder put back fails the control.

Decision rule to register: done only if the control fails before and passes after and the registered repeats all pass
after; partial if the failure is not reproduced before the change; stopped and reported if anything outside the test file
would have to change.

Other agents may be compiling on this machine; a time is not a result here, and no limit of time is registered.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Run in the background anything that takes longer than a few minutes.

## Report (under 300 words)

The registration's commit and the code commits; failures out of the repeats before and after; the control; what the
other calls of `tmp` showed; the mutation; deviations, each with the commit that precedes its corrected run; what was not
run; open questions; your verdict by the registered rule.
