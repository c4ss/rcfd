# Brief: F-998, The System process's holds of a new case file as the measure, and the plain write that removes them

Branch: f998-plain-write
Items: F-998
ADR: 0834
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 57484

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f998-plain-write`, on the branch `f998-plain-write`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f998-plain-write-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f998-plain-write.md (F-998, ADR 0834) is carried out on the branch f998-plain-write, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f998-plain-write, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0834 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0833-f998-registration-the-refused-replace-outside-temp-and-plainly-written.md 15386
- crates/rcfd-project/src/atomic.rs 27409

Also in the repository, as needed: ADR 0832 (the trace, arm H, the refusals of 900 edits), `crates/rcfd-mcp/tests/replace_hold_f978.rs`
(the arms H, HO, HP, HOP and the control TC4), every caller of `write_new` in the workspace, the existing tests of the atomic
write and replace (`hammer`, `edit_lock_f755` and `edit_lock_f855` with the `test-hooks` feature).

## What the lead decided (2026-11-27)

ADR 0833 ran the four arms of 300 rounds under arm H's load: the control H refused 0 of 900 edits (ADR 0832: 15), so no cause
was read. Without a refusal the trace saw the System process hold 18 of H's 300 new case files and 4 of HO's, for 141 to 237 ms
each, and none in HP or HOP, where the case is created by a plain write instead of `write_new`'s hard link. On its question the
lead takes the recommendation, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always):
option (b), the holds seen without a refusal as the measure, then the plain-write remedy registered with its check.

## Task

One registration, `docs/adr/0834-f998-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the measure (the count of new case files the System process holds, and the holds' lengths,
   over a registered number of rounds under arm H's load, with the trace and its control TC4); the arms H (as now) and P (the
   remedy: `write_new` creates the file by a plain write, with what it keeps of the current path's guarantees, named: no
   partial file seen by a reader, no file replaced that should not exist, its behaviour when the file exists); the rule (P's
   held count at most a registered fraction of H's, with H's count at least a registered floor, else not deciding); the
   remedy's checks (every caller of `write_new`, every existing atomic-write and replace test, with the `test-hooks` feature
   where a file needs it; the bytes written equal; a crash test or the existing one for the no-partial-file guarantee); then
   the 900 edits of F-998's acceptance under P; negative controls and mutations (rule 16), one committed as a patch with
   `--target-dir target/mutation`; the numbers fixed in advance (rule 15).
2. **Build** the remedy in `crates/rcfd-project/src/atomic.rs` only, behind no switch if every check holds (it is a change of
   how a file is created, not of a default result), and run as registered, each arm once.
3. Every test that starts a runner ends it (ADR 0828's helpers).

Decision rule to register: done for F-998 if the rule holds, the remedy's checks hold, the 900 edits are refused 0 times
under P and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: a guarantee of the current path that the plain write
cannot keep, and any question of scope.

Files you own: `crates/rcfd-project/src/atomic.rs` (`write_new` only), `replace_hold_f978.rs` (new arms), new tests and
investigation files, ADR 0834, `docs/roadmap/proposals/0834-F-998.json`. No solver files, no Studio files.

## Common to this work

The acceptance of F-998 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses (rule 2), with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Other agents run on
this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Delete nothing outside your worktree and your scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0834-F-998.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
