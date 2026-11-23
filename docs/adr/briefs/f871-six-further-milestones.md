# Brief: F-871, six further simulation milestones

Branch: f871-six-further-milestones
Items: F-871
ADR: 0775
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 2; 103811

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f871-six-further-milestones`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f871-six-further-milestones-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0756-f865-registration-per-cent-complete-for-the-roadmap-and-for-each-simulation-milestone.md 33266 (the membership rule, the classes and the command; the six proposed milestones)
- docs/adr/0757-f867-registration-the-fan-milestone-as-the-whole-design-goal.md 38644 (how a milestone was redefined and added by one command, with the sample check)
- scripts/roadmap_milestones.py 15550 (the --apply command and --check)
- docs/milestones.json 3360 (the three milestones as they stand)

Also in the repository, as needed: `docs/adr/0756-f865-mapping.tsv`, `docs/adr/0757-f867-mapping.tsv` and their definitions files as the form of a mapping; `docs/roadmap-overview.md`; the pages of the repository that record each case's reference.

## What the maintainer decided (2026-11-22)

ADR 0756 split the roadmap's per cent into the whole and one figure per simulation milestone, and proposed twelve further milestones; the maintainer approved six of them as this item: lid-driven cavity, backward-facing step, flat plate, heated cavity, shock tube and wedge, packed bed. The other six wait for a headline item each. The sentence this work must be able to write: these milestones are defined with their cases, references and headline items, every item of the roadmap is read for each, and one command adds them on main leaving the existing three byte for byte as they are.

## Task

One registration, `docs/adr/0775-f871-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the mapping** the six definitions (cases, the published reference as a page of the
   repository records it, the headline items) and how the membership rule of ADR 0756 is read for each. A milestone
   for which the roadmap holds no headline item is reported with the item that would have to be entered, and is not
   added.
2. **Read every item of the roadmap** against the rule for each milestone (with a script that reads
   `docs/roadmap.json`; never the whole file by eye), with a reason for every membership, and read a sample drawn by a
   registered seed a second time; report the disagreements.
3. **One command** (`python scripts/roadmap_milestones.py --apply MAPPING.tsv --definitions FILE`, as ADR 0757 used it)
   adds the milestones; show on a copy of main that it leaves the three existing milestones byte for byte as they are
   and that `--check` passes. You do not run it on `docs/milestones.json` itself: the lead does at the merge.

Decision rule to register: done if every milestone with a headline item is defined and mapped by the registered rule and the command is shown on a copy; partial if a milestone cannot be defined as registered.

Open questions for the maintainer, with options and a recommendation, not decided by you: headline items that would have to be entered, each with options and a recommendation.

Files you own: the mapping and definitions files under `docs/adr/` named after this ADR, as ADR 0756 and 0757 did. Do not edit `docs/milestones.json`, `docs/roadmap.json` or `scripts/roadmap_milestones.py`; if the command needs a change, stop and report.

## Common to this work

The acceptance of F-871 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0775-F-871.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
