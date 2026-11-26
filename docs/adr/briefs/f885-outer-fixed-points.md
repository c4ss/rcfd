# Brief: F-885, why the relaxed and the unrelaxed outer iterations of a pimple step have different fixed points

Branch: f885-outer-fixed-points
Items: F-885
ADR: 0810
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 105889

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f885-outer-fixed-points`, on the branch `f885-outer-fixed-points`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f885-outer-fixed-points-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f885-outer-fixed-points.md (F-885, ADR 0810) is carried out on the branch f885-outer-fixed-points, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f885-outer-fixed-points, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0810 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0767-f885-registration-check-h-by-the-velocity-change-of-the-outer-iterations.md 41959 (Addendum 1, the open questions, the lead's reading and decisions)
- crates/rcfd-api/tests/f885_check_h_outer_record.rs 49241

Also in the repository, as needed, by section: ADR 0183 (check H and its outer-iteration rule), ADR 0766 (F-879, check H
in time), the pimple step and its relaxation in the transient solver (the record of F-885 names the files), the cavity
case of the record's own tests.

## What the lead decided (2026-11-25)

ADR 0767 found that in every step the 5th, unrelaxed outer iteration of the pimple loop moves the velocity about as far as
the 1st (d5/d1 0.93 at dt, 0.96 at dt/2, and the same on a cavity), so ADR 0183's outer-iteration rule cannot be met and
check H does not decide. On its question 1 the lead took option (a), recorded under "Decisions (2026-11-25), the lead's":
a diagnosis registered under F-885 (rule 13), on the cavity and on 20 steps of H(dt), with no long run, that finds where
and why the relaxed and the unrelaxed iterations have different fixed points; the remedy is decided on its result, in a
registration of its own. Question 2 stays at option (a) until this shows the outer loop converges within a step.

## Task

One registration, `docs/adr/0810-f885-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the hypotheses, each from the code of the pimple step with its file and line (for example
   a relaxation that enters the momentum matrix and is not removed at convergence, a face flux or a pressure correction
   that keeps a relaxed term, the final iteration solving different equations), each with the control that separates it:
   the record of ADR 0767 with the cell and the term of the largest change, and the same steps with alpha 1 in every outer
   iteration and with a higher outer count, on the cavity and on 20 steps of H(dt); the registered numbers each control
   decides by, fixed in advance (rule 15: a figure of your own is not a source; a closed form or the code's own algebra is).
2. **Run** the controls as registered. Any instrumentation is opt-in and leaves every default bit-identical
   (`baseline_hashes` unchanged); no solver default changes in this work.
3. Mutations of the instrumentation on scratch copies, each recorded with what caught it; one committed as a patch (rule 16).
4. Report which hypothesis the controls support, with the numbers, and the remedies it points to as options with a
   recommendation for the lead (a remedy is a registration of its own).

Decision rule to register: done for this registration if every control ran as registered and the result names one
hypothesis or rules all out; F-885 itself stays partial until check H decides.

Open questions for the maintainer, with options and a recommendation: the remedy, and any question of scope.

Files you own: new tests and investigation files of this work, opt-in instrumentation in the pimple step's files (named in
the registration), ADR 0810, `docs/roadmap/proposals/0810-F-885.json`. No Studio files: another agent runs the Studio's
tests now; your runs are Rust only.

## Common to this work

The acceptance of F-885 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0810-F-885.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
