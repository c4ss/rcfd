# Brief: F-998, The refused case-file replace: arm H with the project outside the temporary folder and the case written plainly

Branch: f998-replace-outside-temp
Items: F-998
ADR: 0833
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 39755

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f998-replace-outside-temp`, on the branch `f998-replace-outside-temp`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f998-replace-outside-temp-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f998-replace-outside-temp.md (F-998, ADR 0833) is carried out on the branch f998-replace-outside-temp, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f998-replace-outside-temp, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0833 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0832-f978-registration-the-hold-of-a-refused-file.md 18936
- crates/rcfd-mcp/tests/replace_hold_f978.rs 6130

Also in the repository, as needed: ADR 0830 (the trace, arms A and B), `crates/rcfd-project/src/atomic.rs` (the replace, its
12-attempt budget, `write_new` and its hard link), `crates/rcfd-mcp/src/project.rs` (`new_case`, `edit_case`), the scratch
records of ADR 0832 in `E:/projects/cfd/w49i-work/f978-replace-hold-scratch`.

## What the lead decided (2026-11-27)

ADR 0832 measured the refused replace of a case file under load (arm H: no canvas server, 16 spinning threads, 300 rounds):
15 of 900 edits were refused, every holder was the System process (pid 4), never an rcfd process, and the holds ran from 1.8 s
to over 18.8 s, so no replace budget under 2 s covers them and F1 was not made. B1 to B3 of F-978 held in two runs. On its
questions the lead takes the recommendations, on the maintainer's standing instruction of 2026-11-26 (go with the recommended
option always): (Q2) the refused replace is its own item, F-998, with the acceptance "0 refusals of 900 edits under arm H's
load"; (Q1) its first registration runs arm H with the project outside `%TEMP%` and with the case created by a plain write
instead of `write_new`'s hard link, to tell which of the two the holder reacts to.

## Task

One registration, `docs/adr/0833-f998-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the arms, each 300 rounds of arm H's load with ADR 0832's trace beside it: H (as ADR 0832, the
   control, in `%TEMP%` with the hard link), HO (the project outside `%TEMP%`, in a folder under the scratch folder), HP (the
   case created by a plain write), HOP (both); the measure (refusals of 900 edits, the holder, the hold's length); the rule that
   reads a cause from the arms (for example HO with 0 refusals and H with 5 or more); the remedy each outcome points to (a
   project's default place, a plain write in `write_new`, or a client-side retry reported as transient), named as an option and
   not made unless the registration names its check; negative controls and mutations (rule 16), one committed as a patch with
   `--target-dir target/mutation`; the numbers fixed in advance (rule 15).
2. **Run** as registered, each arm once. No change to `atomic.rs` or any default unless the registration names it and its
   check.
3. Every test that starts a runner ends it (ADR 0828's helpers).

Decision rule to register: done for F-998 if an arm meets "0 refusals of 900 edits" by a change that is made and checked and
each mutation is caught; partial otherwise, with the arms' numbers.

Open questions for the maintainer, with options and a recommendation: a remedy that changes where projects are made or how a
file is written, and any question of scope.

Files you own: `replace_hold_f978.rs` (new arms), new tests and investigation files, ADR 0833,
`docs/roadmap/proposals/0833-F-998.json`. No solver files, no Studio files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0833-F-998.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
