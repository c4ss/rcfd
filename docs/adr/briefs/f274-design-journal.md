# Brief: F-274, journal lines of design runs and of stored design records

Branch: f274-design-journal
Items: F-274
ADR: 0794
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-25
Size (lead): done; 0; 0; 0; 94566

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f274-design-journal`, which the lead made with the branch `f274-design-journal` at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f274-design-journal-scratch`.

## Goal

Your work is started with this line as its whole prompt:

    /goal The brief docs/adr/briefs/f274-design-journal.md (F-274, ADR 0794) is carried out on the branch f274-design-journal, checked out in this directory, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of the ADR as the first commit, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `cargo fmt` (check), clippy with `-D warnings` and the tests of rcfd-workspace, rcfd-mcp and rcfd-project the brief names, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 80 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md 36958 (S10, Q6 and the addendum's item 4)
- docs/adr/0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md 44617 (J1 to J8)

Also in the repository, as needed: `crates/rcfd-workspace/src/{runs,design,journal,feed}.rs`, `crates/rcfd-project/src/journal.rs`, `crates/rcfd-mcp/tests/feed_f273.rs`, the tests of F-274, F-287 and F-288 under `crates/rcfd-workspace/tests` and `crates/rcfd-mcp/tests`, ADR 0173 D4 (c), ADR 0200 and ADR 0201, `docs/mcp-tools.md`.

## What the maintainer decided (2026-11-25)

On 2026-11-25 the maintainer answered "Yes" to resuming the roadmap by the priority lines; F-274 is of the first line (the workspace layer, F-270 to F-275). Its one missing piece is the journal of ADR 0189 S10. Decided by the lead (a detail, Q6): the recommendation (a): a design evaluation submitted to the runner writes the `run_submitted` line of ADR 0188 with its study, variant and condition, and each evaluation stored writes one `design_recorded` line, the kind ADR 0173 D4 (c) names. An adopted run (F-288) was submitted once and gets no second `run_submitted` line. The files the tools write under `designs/` are then in the feed as the workspace's own and not as `external`. F-287 and F-288 are read by the lead separately and are not waited for.

## Task

One registration, `docs/adr/0794-f274-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code or test**: the fields of both lines (additive, numbered as every line); for a cold
   study, a chained study, a study with `max_steps`, an operating map, an adopted run after a restart, a failed evaluation
   and a refused submission, the exact lines and their count; that the line of a stored evaluation is written after
   the store holds it and that a journal that cannot be appended does what ADR 0188 registered for the other
   commands; that the feed shows each change once, with its actor and reason where the call has one, and
   `external` only for a write made outside the workspace; that the records, metrics, run records, replies and tool
   list are bytes of main, and that without `--project` nothing is written. Read whether a second writer under the edit
   lock (two servers in one project) keeps the sequence numbers unique and register a check of it. If a client of the
   feed (the Electron Studio) refuses or misreads a kind it does not know, report it and do not change it.
2. **Change** the workspace, the smallest change that does it; `docs/mcp-tools.md` gets the sentence it needs.
3. **Tests** in new files `crates/rcfd-workspace/tests/design_journal_f274.rs` and `crates/rcfd-mcp/tests/design_journal_f274.rs`.
   Mutations, each with its row: the submission line left out; written for an adopted run; the record line left out;
   written before the store; written for a refused submission; the tags left out; the actor left out; the file
   under `designs/` still `external`. Commit two as `.mutation.patch` files as earlier files did.
4. Run the new files, every test file of the two crates whose name holds f273, f274, f287 or f288, `cargo test -p
   rcfd-project --release --lib`, `cargo test -p rcfd-workspace --release --lib`, all unedited, and the checks rule 7 lists.

Decision rule to register: done if every registered row holds, each mutation is caught by its row and the named existing tests pass unedited; partial otherwise, with the piece named.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `crates/rcfd-workspace/src/{runs,design,journal,feed}.rs` (and the one call in `crates/rcfd-mcp/src` the registration names), the two new test files and their patches, sentences of `docs/mcp-tools.md`. No golden and no existing test; if one holds that no line is written, stop and report.

## Common to this work

The acceptance of F-274 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0794-F-274.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
