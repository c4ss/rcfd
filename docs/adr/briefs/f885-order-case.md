# Brief: F-885, why the pimple step's time-order case is first order, then the remedy's order and check H

Branch: f885-order-case
Items: F-885
ADR: 0822
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 96207

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f885-order-case`, on the branch `f885-order-case`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f885-order-case-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f885-order-case.md (F-885, ADR 0822) is carried out on the branch f885-order-case, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f885-order-case, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0822 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0821-f885-registration-the-frozen-convection-gradient-its-time-order-a-steady-case-and-check-h.md 34570
- crates/rcfd-api/tests/f885_frozen_convection_check_h.rs 46948

Also in the repository, as needed: ADRs 0767, 0183 and 0820 (check H, its outer rule, the setting), the pimple step and its BDF2
time scheme in `crates/rcfd-fv/src/pimple.rs`, the wall-bounded case f2c of `fractional_step.rs`, the existing time-order tests
of the solvers, `docs/investigations/f885-frozen-convection-check-h/`.

## What the lead decided (2026-11-27)

ADR 0821's steady check held (the cavity's steady answer with `outer_frozen_convection_gradient` on against off within 3.4e-13),
but its order check did not decide: on the Taylor-Green set-up the pimple step is first order without the setting too (1.015;
central convection 1.012, 20 outer iterations 1.015, implicit Euler 1.047), so check H was not run. On ADR 0821's questions the
lead takes the recommendations, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always):
(1) a diagnosis under F-885 of why the set-up is first order, with the wall-bounded case f2c of `fractional_step.rs` as its
control, then the order check again on a corrected set-up; (2) the setting stays opt-in; (3) no new item.

## Task

One registration, `docs/adr/0822-f885-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: hypotheses for the first order from the code and the set-up, each with its file and line, the
   order it predicts and its control (for example the start-up step of BDF2, the time level of a boundary condition or of the
   periodic partner, the pressure's time level in the correction, an initial field that is not the exact solution at t0, a
   spatial error that dominates at the dts used, the error norm's time), the f2c case as a control of the solver's order; the
   corrected order check (setting off, then on, `p_f >= 1.9` as ADR 0821 registered it); then check H as ADR 0821 registered it,
   only if the order check holds with the setting on and off; mutations (rule 16); the numbers fixed in advance (rule 15).
2. **Run** as registered. A defect in solver code found by the diagnosis is a finding: report it with its file and line and the
   remedy as an option; fix only a test set-up on this branch. `baseline_hashes` unchanged; no solver default changes. Check that
   the committed mutation patches of the F-885 tests still apply, and re-base them if not, the mutation unchanged.
3. Mutations on scratch copies of the full commit; one committed as a patch. A mutation must be able to change what its test
   reads, and its test must pass without it.
4. Report each row, check H's outcome by its own rule if run, and what F-885's acceptance needs next, as options with a
   recommendation.

Decision rule to register: done for F-885 if the order check holds with the setting on and off, check H holds as registered and
each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: a solver defect's remedy, and any question of scope.

Files you own: new tests and investigation files, ADR 0822, `docs/roadmap/proposals/0822-F-885.json`. No solver source files,
no Studio files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0822-F-885.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
