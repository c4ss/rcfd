# Brief: F-828, Electron Studio: the numbers that still have no provenance

Branch: f828-provenance-remaining
Items: F-828
ADR: 0795
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-25
Size (lead): done; 0; 0; 0; 48043

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f828-provenance-remaining`, which the lead made with the branch `f828-provenance-remaining` at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f828-provenance-remaining-scratch`.

## Goal

Your work is started with this line as its whole prompt:

    /goal The brief docs/adr/briefs/f828-provenance-remaining.md (F-828, ADR 0795) is carried out on the branch f828-provenance-remaining, checked out in this directory, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of the ADR as the first commit, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `npm run typecheck`, `npm run test:unit` and the Playwright files the brief names in apps/studio-web, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 90 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 14395 (G9, G17, G19 and the word budget)
- docs/adr/0734-f828-proposal-run-records-launch-argv-build-id-time-unit-rerun-and-mesh-hash.md 20657

Also in the repository, as needed: `docs/adr/0801-studio-web-dependencies-and-layout.md`, only addendum 8 (from the line `## Addendum 8`) and the sections `F-828` and `D-L1` found by search, never the whole file; `apps/studio-web/src/shared/provenance.ts`, `src/renderer/provenance/`, `src/renderer/runs/`, `src/renderer/workspace/`, `src/renderer/field/`, the home view, `tests/unit/provenance.spec.ts`, `tests/e2e/provenance.spec.ts`, `tests/unit/word-budget` and the a11y specs; `apps/studio-web/README.md` for how the engine and the tests are built.

## What the maintainer decided (2026-11-25)

On 2026-11-25 the maintainer answered "Yes" to resuming the roadmap by the priority lines; F-828 is of the Studio line. The acceptance says any number on screen opens its provenance; missing are the Runs table's residual columns, the compared run's residuals, the legend ticks, the domain size, the scale bar, the runner's budgets and the home counts. Decided by the lead (details): each uses the existing trigger, registry and popover, and no new dependency or design element; a number of a dense row or a row of ticks opens as a passive span named by its row does now, so that no Tab stop is added per number; the popover says where the number comes from and, where the record holds nothing, says `not recorded`; a number the Studio counts or rounds says what it counted or rounded and from which answer; a unit comes from the engine or is said not recorded, never converted in the Studio. A question of scope (a number not in this list) is reported, not built.

## Task

One registration, `docs/adr/0795-f828-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code or test**: the list of the numbers, found by reading every view (not only the seven
   groups above), each with its source (service method or file and field), its unit's source, its trigger and the facts
   its popover shows; for each a check in a real window that opens it by the pointer and by the keyboard (Ctrl+I or
   the palette) and reads the facts against the engine's own answer made outside the Studio; a check that finds a number
   on the named views with no provenance (a scan, with the exempt ones listed and their reason); the Tab stops and the
   word budget unchanged; no request or redraw at rest added (`idle-frames`). A number whose source cannot be named
   is recorded as missing and makes the verdict partial.
2. **Change** the Studio. No engine file. The Studio converts no unit and computes no physical quantity.
3. **Tests**: new files `tests/unit/provenance-f828.spec.ts` and `tests/e2e/provenance-f828.spec.ts`; existing specs unedited. Mutations:
   one per group (the source of another run, the unit of another field, the keyboard path removed, the number left out of the
   scan), each caught by its row.
4. Run the two new files, `provenance.spec.ts`, `a11y`, `word-budget`, `runs`, `compare`, `field`, `home` and `idle-frames` specs (their
   names as found) and `npm run test:unit`. One Playwright worker. A test that fails under load is reported with its first result.

Decision rule to register: done if every number of the registered list opens its provenance by pointer and keyboard with the registered facts, the scan finds none without, each mutation is caught and the named existing specs pass unedited; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: numbers outside the acceptance's reading, if any, with options and a recommendation.

Files you own: `apps/studio-web/src/renderer/**` and `apps/studio-web/src/shared/provenance.ts`, `apps/studio-web/src/main/provenance/**`, the two new specs and the mutation patch. No existing spec, no golden, no screenshot baseline, no package file. If an existing spec must change, stop and report.

## Common to this work

The acceptance of F-828 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no engine or solver source; if a defect of the engine appears, report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0795-F-828.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
