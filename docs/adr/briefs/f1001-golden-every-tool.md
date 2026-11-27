# Brief: F-1001, The project tool-list golden holds every tool added since 7c960bb

Branch: f1001-golden-every-tool
Items: F-1001
ADR: 0841
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 55075

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f1001-golden-every-tool`, on the branch `f1001-golden-every-tool`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f1001-golden-every-tool-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f1001-golden-every-tool.md (F-1001, ADR 0841) is carried out on the branch f1001-golden-every-tool, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f1001-golden-every-tool, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0841 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0839-f999-registration-canvas-tools-in-the-project-golden.md 16953
- crates/rcfd-mcp/tests/project_list_f270.rs 23433

Also in the repository, as needed: ADR 0185 (A2, the golden of 7c960bb), ADR 0805 (D2), the re-records 5eb2be45 and 43f126e0,
`crates/rcfd-mcp/tests/golden/tools_list_project_f270.json`, ADR 0839's `make_golden.py` in its scratch folder
`E:/projects/cfd/w49i-work/f999-canvas-tools-in-golden-scratch`, the other tool-list tests.

## What the lead decided (2026-11-27)

ADR 0839 added show_view, read_view and export_view to the project golden and had `project_list_f270` compare them. The other
tools added since 7c960bb (the trash tools, list_sites and others) are still taken off the live list and caught by no golden.
On its question 2 the lead took option (b), on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always): F-1001 adds them the same way. ADR 0839's answer to its question 1 reads ADR
0185 A2 as "the golden's first 52 entries are 7c960bb's, byte for byte"; this work keeps the first 55 byte-identical.

## Task

One registration, `docs/adr/0841-f1001-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the list of every tool the test takes off the live list, each with the commit that added it;
   any that must stay off, with the reason; the entries appended by a script from the live list, in live order; a check that
   the golden's first 55 entries are byte-identical to origin/main's; the test change (compare each, then take it off both
   lists), every other assertion unchanged; negative controls (the old test with a mutated description passes) and mutations
   (rule 16: a change to each added tool's description fails the test; at least one per added group), one committed as a
   patch with `--target-dir target/mutation`; the numbers fixed in advance (rule 15).
2. **Build** the golden's entries and the test change. No tool's text changes.

Decision rule to register: done for F-1001 if every tool is in the golden or named with its reason, the first 55 entries are
byte-identical, every tool-list test passes and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: `tools_list_project_f270.json` (appended entries only), `project_list_f270.rs` (the removals), new tests, ADR
0841, `docs/roadmap/proposals/0841-F-1001.json`. No solver files, no Studio files.

## Common to this work

The acceptance of F-1001 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0841-F-1001.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
