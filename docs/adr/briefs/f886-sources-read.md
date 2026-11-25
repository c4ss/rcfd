# Brief: F-886, three formulas recorded from memory, read against their open sources

Branch: f886-sources-read
Items: F-886
ADR: 0796
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-25
Size (lead): done; 0; 0; 0; 76689

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f886-sources-read`, which the lead made with the branch `f886-sources-read` at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f886-sources-read-scratch`.

## Goal

Your work is started with this line as its whole prompt:

    /goal The brief docs/adr/briefs/f886-sources-read.md (F-886, ADR 0796) is carried out on the branch f886-sources-read, checked out in this directory, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of the ADR as the first commit, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of the numerical comparison of each formula and its negative control, the reference-data checks the brief names, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 60 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0056-wave29b-mixture-cavitation.md 27374 (the model and its sources)
- docs/adr/0039-wave23a-mhd-heat-exchanger.md 36324 (the effectiveness and NTU relations)

Also in the repository, as needed: in `docs/adr/0762-*` the part on the mesh-convergence estimate (found by `GCI`); the wiki pages `wiki/concepts/{cavitation-and-mass-transfer-models,heat-exchanger-effectiveness-ntu,gci-richardson}.md` and `wiki/sources/ref-{folden-aschmoneit-2023-cavitation-models,lienhard-ahtt-6e,celik-2008-jfe-discretisation-uncertainty}.md` (where the sources were found and what was read; the wiki is not a source and is not edited); the code of each formula, found from the ADRs; `docs/reference-data.d/0000-base.part` (the rows marked `unverified` and `from memory`) and `0762-F-870.part`; `docs/process/citation-batches.md`.

## What the maintainer decided (2026-11-25)

The maintainer's answer of 2026-11-22 ("Register a docs item") entered this item with its acceptance; on 2026-11-25 the maintainer answered "Yes" to resuming the roadmap by the priority lines. Decided by the lead (details): the source is read by you from the publisher's or author's public copy, the equation is read from the page (from its image where the text layer loses it), and its terms of use are stated; a source that cannot be read without a sign-in, a payment or a bot check is recorded as not read, its mark stays, and nothing is worked around. A copy whose terms do not allow keeping it is not kept in the repository.

## Task

One registration, `docs/adr/0796-f886-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any change**: for each of the three records the rows and lines of the repository that state the formula, the lines of
   code that implement it, the source with its edition, equation and page, and the comparison to be made: the
   source's form transcribed in the ADR's own notation, term by term against the code, the limits it states (the range, the
   exponent, the safety factor), and a closed-form or numerical check of each comparison at points chosen before the reading
   (two routes that do not share code). Register what counts as agreement (an algebraic identity shown, and the
   numbers equal to a stated rounding) and as a difference.
2. **Read the three sources** and make the comparison. A difference is a finding, not a fix.
3. **Write**: an addendum at the end of each of ADR 0039, 0056 and 0762; a fragment `docs/reference-data.d/0796-F-886.part`; the
   superseded marks struck through with the source cited, in the fragments and not in the generated page. A negative control
   of each comparison: a formula with one term changed is shown not to agree.
4. No code, default or test changes; show that `git diff --stat origin/main` names documents only. The checks rule 7 lists, with the
   two reference-data checks that the repository has (found by search in `scripts` and the tests of rcfd-cli).

Decision rule to register: done if the three sources were read and each comparison is recorded with its result, agreement or difference; partial if a source could not be read, with the one named.

Open questions for the maintainer, with options and a recommendation, not decided by you: whether a difference found is to be repaired, if any, with options and a recommendation.

Files you own: the addenda at the end of ADR 0039, 0056 and 0762, `docs/reference-data.d/0796-F-886.part` and the marks of the three rows in `docs/reference-data.d/0000-base.part` and `0762-F-870.part`. No code, no test, no generated page, no wiki page.

## Common to this work

The acceptance of F-886 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0796-F-886.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
