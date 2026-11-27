# Brief: F-998, The plain write for new case files only, and H and P interleaved as the measure

Branch: f998-plain-write-cases-only
Items: F-998
ADR: 0835
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 50309

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f998-plain-write-cases-only`, on the branch `f998-plain-write-cases-only`; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f998-plain-write-cases-only-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f998-plain-write-cases-only.md (F-998, ADR 0835) is carried out on the branch f998-plain-write-cases-only, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f998-plain-write-cases-only, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat the branch f998-plain-write (ADR 0834, unmerged) merged with origin/main..HEAD` shows the registration of ADR 0835 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- crates/rcfd-project/src/atomic.rs 27409
- crates/rcfd-workspace/src/cases.rs 8211

Also in the repository, as needed: ADR 0834 on the branch f998-plain-write (the remedy, W1 to W3, N3, arms H and P, its
addenda and questions), ADR 0833, `crates/rcfd-mcp/tests/replace_hold_f978.rs`, `crates/rcfd-project/tests/write_new_f998.rs`,
every caller of `write_new` in the workspace (`rcfd-project` project.rs, `rcfd-runner` store.rs and site/agent.rs,
`rcfd-workspace` cases.rs and meshes.rs).

## What the lead decided (2026-11-27)

ADR 0834 built the plain write in `write_new` on Windows (`create_new`, share mode 0, written, synced, closed; no temp file
and no hard link); W1 to W3 and N3 held, every mutation was caught, and 900 `edit_case` under P were refused 0 times; but the
control H showed 0 holds, so its rule did not decide, and the remedy gives up a guarantee: a writer killed mid-write leaves a
partial file under the final name, for every caller of `write_new`. The lead did not merge it. On its questions the lead takes
the recommendations, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always): (Q1) option
(b), the plain write only for new case files, the temp file and hard link kept for every other caller (`run.json`,
`case.snapshot`, launch and trash records, meshes, the runner's records); (Q2) option (a), H and P interleaved in blocks of 50
rounds, 2000 rounds each, keeping ADR 0834's floor (8 held files in H) and fraction (P at most 0.1 of H). ADR 0834 and this
work are merged together.

## Task

Work on this branch, which starts from f998-plain-write merged with origin/main. One registration,
`docs/adr/0835-f998-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the split (a function of its own for a new case file, its name and its callers named by
   file and line, every other caller back on the temp file and hard link, bit-identical to origin/main's `write_new`); what a
   writer killed mid-write leaves for a new case file, and how a reader of the project treats it (a check that names it);
   W1 to W3 and N3 run against the new function, and a check that `write_new` is unchanged from origin/main; arms H and P
   interleaved as decided, with the trace, TC4 and CPU recorded per block; F-998's 900 edits under P; the L1 test
   `run_records_adr0734` run with `CARGO_TARGET_DIR` spelled with backslashes (ADR 0834's amendment 2); negative controls
   and mutations (rule 16), one committed as a patch with `--target-dir target/mutation`; the numbers fixed in advance
   (rule 15).
2. **Build** the split in `crates/rcfd-project/src/atomic.rs` and the new-case callers, and run as registered, each arm once.
3. Every test that starts a runner ends it (ADR 0828's helpers).

Decision rule to register: done for F-998 if ADR 0834's rule decides for P over the interleaved arms, the split's checks
hold, the 900 edits are refused 0 times under P and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: `crates/rcfd-project/src/atomic.rs`, the new-case callers in `crates/rcfd-workspace/src/cases.rs` (and in
`crates/rcfd-mcp/src/project.rs` if a new case is made there), `replace_hold_f978.rs`, `write_new_f998.rs`, new tests and
investigation files, ADR 0835, `docs/roadmap/proposals/0835-F-998.json`. No solver files, no Studio files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0835-F-998.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
