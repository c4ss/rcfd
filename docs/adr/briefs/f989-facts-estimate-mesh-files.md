# Brief: F-989, case.facts refuses by an estimate for mesh files and the generators that have none

Branch: f989-facts-estimate-mesh-files
Items: F-989
ADR: 0844
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 96830

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f989-facts-estimate-mesh-files`, on the branch `f989-facts-estimate-mesh-files`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f989-facts-estimate-mesh-files-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f989-facts-estimate-mesh-files.md (F-989, ADR 0844) is carried out on the branch f989-facts-estimate-mesh-files, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f989-facts-estimate-mesh-files, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0844 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0812-f823-registration-case-facts-refuses-by-the-counts-estimate.md 31389
- crates/rcfd-api/src/mesh_preview.rs 11518
- crates/rcfd-api/src/facts.rs 39234

Also in the repository, as needed: `crates/rcfd-api/tests/case_facts_estimate_adr0812.rs` and its mutation patch, ADR 0737
(case.facts), `crates/rcfd-api/tests/mesh_preview_adr0732.rs`, the mesh importers of the engine (each format it reads), the
mesh generators and their existing estimates, `crates/rcfd-workspace/src/facts.rs`, the repository's mesh files under
`crates/*/tests/`.

## What the lead decided (2026-11-28)

ADR 0812 made case.facts refuse by a counts estimate for the generators that have one; mesh files and the generators without
an estimate were left out (F-989). The lead issues it, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always), in line 1 of the priority (Claude and the MCP server).

## Task

One registration, `docs/adr/0844-f989-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the list of mesh formats the engine imports and of generators without an estimate, each with
   its file and line; how each estimate reads the cell count without building the mesh (a header, a section count, the
   generator's parameters); the registered set of cases (every mesh file of the repository in each format, and each
   generator over a registered parameter set), each with the built count; the two routes of the check (the estimate against
   the built mesh, and against an independent count, for example the file's own element section read by a second reader);
   case.facts with a limit refuses by the estimate for each, within ADR 0812's time and memory limits, with
   FactsError::TooLarge; negative controls and mutations (rule 16), one committed as a patch with `--target-dir
   target/mutation`; the numbers fixed in advance (rule 15). `baseline_hashes` unchanged; no solver default changes.
2. **Build** the estimates in `mesh_preview` and their use in case.facts.

Decision rule to register: done for F-989 if every estimate equals the built count over the registered set by both routes,
case.facts refuses by it for each within ADR 0812's limits and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: a format whose count cannot be read without building the
mesh, and any question of scope.

Files you own: `crates/rcfd-api/src/mesh_preview.rs`, the estimate's use in `crates/rcfd-api/src/facts.rs`, new tests, ADR 0844,
`docs/roadmap/proposals/0844-F-989.json`. No solver numerics, no Studio files.

## Common to this work

The acceptance of F-989 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0844-F-989.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
