# Brief: F-262, the cancel of a held run in the run service (S7 of ADR 0123 fails intermittently)

Branch: f262-s7-cancel-of-a-held-run
Items: F-262
ADR: 0742
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-19
Size (lead): partial; 1; 0; 0; 179002

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f262-s7-cancel-of-a-held-run` (the lead creates it from main). Scratch folder: `E:/projects/cfd/w49i-work/f262-s7-cancel-of-a-held-run-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- docs/adr/0123-wave47b-run-service.md 31556
- crates/rcfd-runner/tests/runner_w47b.rs 34438
- crates/rcfd-runner/src/runner.rs 73298
- crates/rcfd-runner/src/client.rs 27807

## Task

A diagnosis under F-262 (rule 13): `docs/adr/0742-f262-diagnosis-<slug>.md`, registered before any code.

What is known (the lead, 2026-11-19, release, Windows, the shared machine under load): `cargo test -p rcfd-runner --release
--test runner_w47b` fails in `s7_cancel_leaves_a_consistent_folder` in 3 of 20 runs at main 10ba1a34 and 3 of 10 with the
merge of `f848-site-record-final` on top (that merge changes `site/remote.rs` only). Every failure is on the held run (paused,
then cancelled): `folder_problems` reports "last state event Some(String("paused")) differs from status.json", with a
left-over `cancel` file and in two of six cases a left-over `pause` file. The three cancels finish in about 0.15 s. The
reading that fits is that the terminal `status.json` of a cancelled paused run becomes visible before its last state event is
appended and before the request files are removed, so a reader that waits for the terminal status sees an unfinished folder.
That is an inference: confirm or refute it from the code and by a registered reproduction.

- Register first: the hypothesis, how the order of the final writes is read from `runner.rs`, a reproduction that does not
  depend on machine load (a test that makes the window wide, or a loop count with its expected failure rate derived from the
  six in thirty above), and the decision rule.
- If the cause is in the library, the remedy is the order of the final writes (status last), as ADR 0739 Diagnosis 1 did for
  the site record. Local run records must stay byte for byte for a run that is not cancelled (F-848's acceptance says so);
  show it.
- `runner_w47b.rs` is not edited (rule 2) and S7 is not ignored, skipped or loosened. A new test file carries your checks.
- If the cause is in the test's own reading (it reads the folder before the runner is done, and the ADR does not promise
  the order), say so and propose the amendment of ADR 0123 that would correct the set-up; do not make it.
