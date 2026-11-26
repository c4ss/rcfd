# Brief: F-885, why the fan's outer loop does not contract within a step

Branch: f885-fan-outer-stall
Items: F-885
ADR: 0815
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 87616

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f885-fan-outer-stall`, on the branch `f885-fan-outer-stall`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f885-fan-outer-stall-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f885-fan-outer-stall.md (F-885, ADR 0815) is carried out on the branch f885-fan-outer-stall, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f885-fan-outer-stall, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0815 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0811-f885-registration-a-face-flux-independent-of-alpha-u-then-check-h.md 37347
- docs/adr/0810-f885-registration-the-fixed-points-of-the-relaxed-and-unrelaxed-outer-iterations.md 35580

Also in the repository, as needed: ADR 0767 and ADR 0183 (check H and its outer rule), ADR 0766, the pimple step in
`crates/rcfd-fv/src/pimple.rs` with the opt-in `alpha_free_face_flux` of ADR 0811 and the cell record of ADR 0810, the
test files `crates/rcfd-api/tests/f885_*.rs`.

## What the lead decided (2026-11-26)

ADR 0811 removed the jump between the relaxed and unrelaxed fixed points (the window's d1 fell from about 1 m/s to 0.10 and
0.049 m/s), but check H still does not decide: on the fan, d2 to d5 stay about d1 (median d5/d1 0.95 at dt, 1.08 at dt/2),
the stall recorded before the run (C 0.026; median d_19 6.4e-3 m/s at the tip-gap cell c11837). On ADR 0811's questions
the lead takes the recommendations, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option
always): (1) the switch stays opt-in; (2) a diagnosis under F-885 (rule 13), on 20 steps with the cell record and the
controls fixed in advance.

## Task

One registration, `docs/adr/0815-f885-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: hypotheses for the stall from the code, each with its file and line and the control that
   separates it (for example the tip-gap cells' coupling, the rotating frame's source terms treated explicitly in the outer
   loop, a limiter or a non-orthogonal correction that switches between iterations, the linear solvers' tolerances within
   an outer iteration, the pressure reference), with the switch of ADR 0811 on, on 20 steps of H(dt) and on the cavity as
   a control that contracts; the numbers each control decides by, fixed in advance (rule 15); no long run.
2. **Run** the controls as registered. Any instrumentation is opt-in and leaves every default bit-identical
   (`baseline_hashes` unchanged); no solver default changes. Check that the committed mutation patches of the F-885 tests
   still apply, and re-base them in your branch if not, the mutation unchanged.
3. Mutations of the instrumentation on scratch copies; one committed as a patch (rule 16).
4. Report which hypothesis the controls support, with the numbers, and the remedies it points to as options with a
   recommendation (a remedy is a registration of its own).

Decision rule to register: done for this registration if every control ran as registered and the result names one
hypothesis or rules all out; F-885 stays partial until check H decides.

Open questions for the maintainer, with options and a recommendation: the remedy, and any question of scope.

Files you own: new tests and investigation files, opt-in instrumentation in the pimple step's files (named in the
registration), ADR 0815, `docs/roadmap/proposals/0815-F-885.json`. No Studio files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0815-F-885.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
