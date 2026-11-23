# Brief: F-875, a simulation milestone for the structure and vibration of a fan rotor, with the cyclic rotor

Branch: f875-fan-structure-milestone
Items: F-875
ADR: 0779
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 1; 147185

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f875-fan-structure-milestone`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f875-fan-structure-milestone-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0756-f865-registration-per-cent-complete-for-the-roadmap-and-for-each-simulation-milestone.md 33266 (the membership rule, the classes and the command)
- docs/adr/0757-f867-registration-the-fan-milestone-as-the-whole-design-goal.md 38644 (the fan milestone, its question 5 and the answer (c) that gave this item)
- docs/adr/0775-f871-registration-six-further-simulation-milestones.md 37434 (the latest use of the command, with the sample check)
- scripts/roadmap_milestones.py 15550 (the --apply command and --check)
- docs/milestones.json 9300 (the milestones as they stand)

Also in the repository, as needed: the mapping and definitions files of ADRs 0756, 0757 and 0775 as the form of a mapping; `docs/structural-solver.md`; the pages of the repository that record the structural fan references; the items F-387, F-406, F-436, F-593 and F-604 and the cyclic rotor in `docs/roadmap.json`, read with a script.

## What the maintainer decided (2026-11-23)

The maintainer answered question 5 of ADR 0757 with option (c): the structure and vibration of a fan rotor are outside the milestone fan and get a milestone of their own. The sentence this work must be able to write: this milestone is defined with its cases, its reference and its headline items, the five fan-titled structural items and the cyclic rotor are placed, every item of the roadmap is read for it, and one command adds it on main, leaving the other milestones byte for byte as they are.

## Task

One registration, `docs/adr/0779-f875-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the mapping** the definition (cases, the published reference as a page of the
   repository records it, the headline items: F-406 or F-436, or the reason for another) and how the membership rule of
   ADR 0756 is read for it. If the roadmap holds no fitting headline item, report the item that would have to be
   entered and do not add the milestone.
2. **Read every item of the roadmap** against the rule (with a script that reads `docs/roadmap.json`; never the whole
   file by eye), with a reason for every membership; read a sample drawn by a registered seed a second time; report
   the disagreements.
3. **One command** (`python scripts/roadmap_milestones.py --apply MAPPING.tsv --definitions FILE`) adds it; show on a
   copy of main that it leaves every existing milestone byte for byte as it is and that `--check` passes. You do not run
   it on `docs/milestones.json` itself: the lead does at the merge.

Decision rule to register: done if the milestone is defined and mapped by the registered rule and the command is shown on a copy; partial if it cannot be defined as registered.

Open questions for the maintainer, with options and a recommendation, not decided by you: a headline item that would have to be entered, with options and a recommendation.

Files you own: the mapping and definitions files under `docs/adr/` named after this ADR. Do not edit `docs/milestones.json`, `docs/roadmap.json` or `scripts/roadmap_milestones.py`; if the command needs a change, stop and report.

## Common to this work

The acceptance of F-875 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. This item runs no solve. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0779-F-875.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
