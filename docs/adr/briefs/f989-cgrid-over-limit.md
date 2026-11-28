# Brief: F-989, case.facts' estimate for mesh files: an over-limit C-grid fan case that builds

Branch: f989-cgrid-over-limit
Items: F-989
ADR: 0846
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 75437

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f989-cgrid-over-limit`, on the branch `f989-cgrid-over-limit`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f989-cgrid-over-limit-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f989-cgrid-over-limit.md (F-989, ADR 0846) is carried out on the branch f989-cgrid-over-limit, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f989-cgrid-over-limit, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0846 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0844-f989-registration-case-facts-estimate-for-mesh-files.md 32850
- crates/rcfd-api/tests/case_facts_estimate_files_adr0844.rs 27898

Also in the repository, as needed: docs/adr/0812-f823-registration-case-facts-refuses-by-the-counts-estimate.md (the time and memory limits), `crates/rcfd-api/src/mesh_preview.rs`
(`cell_estimate`), the axial_fan_sector generator and its C-grid topology (its leading-edge fan, `front_gap`, `n_nose`,
`n_span`, `n_gap`), ADR 0844's scratch folder `E:/projects/cfd/w49i-work/f989-facts-estimate-mesh-files-scratch` (H11 and
H11b, their logs).

## What the lead decided (2026-11-28)

ADR 0844 left F-989 partial on one case: the over-limit axial_fan_sector C-grid case, whose reference build folded at the
leading-edge fan at `front_gap 0.3` with `n_nose` 10 and with 4 (H11, H11b), so its time and memory against ADR 0812's limits
are not decided. On its Q2 the lead took option (a), on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always): a further registration under F-989 with an over-limit C-grid case
that builds, raising only `n_span` and `n_gap`, with a small build of the leading-edge fan first.

## Task

One registration, `docs/adr/0846-f989-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the small build first (the C-grid at the parameters of an ADR 0844 small case that built,
   with the `n_nose` and `front_gap` of the over-limit case), and what it shows that the leading-edge fan does not fold; the
   over-limit case's parameters (only `n_span` and `n_gap` raised from a case that built), its estimated count against the
   limit; the time and memory of the refusal and of the reference build, measured as ADR 0844 measured H1 to H10, against
   ADR 0812's limits (T_r at most 1 s and T_r/T_b at most 0.5, P_r at most 200 MB and P_r/P_b at most 0.5); the estimate
   against the built count; negative controls and mutations (rule 16), one committed as a patch with `--target-dir
   target/mutation`; the numbers fixed in advance (rule 15). If the small build folds, report it with the case's parameters;
   no generator code changes.
2. **Run** as registered. `baseline_hashes` unchanged; no solver default changes.

Decision rule to register: done for F-989 if the over-limit C-grid case builds, its estimate equals the built count, its
refusal is within ADR 0812's limits and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: a C-grid that folds at every count, and any question
of scope.

Files you own: new tests and their additions to `case_facts_estimate_files_adr0844.rs`, ADR 0846,
`docs/roadmap/proposals/0846-F-989.json`. No solver numerics, no generator code, no Studio files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0846-F-989.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
