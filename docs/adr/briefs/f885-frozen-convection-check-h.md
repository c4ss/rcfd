# Brief: F-885, the remedy outer_frozen_convection_gradient, its order and steady checks, then check H on the fan

Branch: f885-frozen-convection-check-h
Items: F-885
ADR: 0821
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 141096

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f885-frozen-convection-check-h`, on the branch `f885-frozen-convection-check-h`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f885-frozen-convection-check-h-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f885-frozen-convection-check-h.md (F-885, ADR 0821) is carried out on the branch f885-frozen-convection-check-h, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f885-frozen-convection-check-h, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0821 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0820-f885-registration-the-slow-outer-mode-by-freezing-the-lagged-parts-of-the-predictor.md 40699
- docs/adr/0767-f885-registration-check-h-by-the-velocity-change-of-the-outer-iterations.md 41959
- crates/rcfd-api/tests/f885_slow_outer_mode.rs 43749

Also in the repository, as needed: ADR 0183 (check H's outer rule), ADRs 0810, 0811, 0815, 0816 and 0819 with their opt-ins and
records, the pimple step in `crates/rcfd-fv/src/pimple.rs`, the time-order tests of the BDF2 scheme, `docs/investigations/`.

## What the lead decided (2026-11-26)

ADR 0820 named the slow outer mode: linear upwind's deferred correction, formed again from each outer iterate's gradient,
carries it (with that correction frozen from the step's first outer iteration the fan's factor fell from 0.834 to 0.359 and C
from 0.0143 to 1.1e-8; d5/d1 at most 5.8e-3 in all 10 classed steps). On ADR 0820's questions the lead takes the
recommendations, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always): (1) the remedy
is `outer_frozen_convection_gradient`, registered with a BDF2 time-order check and a check that a steady case gives the same
answer; (2) check H is run in this registration, after the order check; (3) it is a solver setting with no case key (no schema
golden changes).

## Task

One registration, `docs/adr/0821-f885-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code or run**: the setting (its name, where the frozen correction is kept and reset, every default
   bit-identical); the order check (a case with a known solution or a refinement study in dt, the observed order of BDF2 with the
   setting on against off, the limit and its source); the steady check (a converged steady case, the answer with the setting on
   against off, the tolerance and its source); then check H as ADR 0767 and ADR 0183 state it, on the fan at dt and dt/2 with the
   opt-ins named, its decision rule unchanged; the cavity as a control; mutations (rule 16); the numbers fixed in advance (rule
   15). A long run only where check H's registration requires it, with its expected duration.
2. **Build** the setting from ADR 0820's opt-in (or promote it), and run as registered: the order check and the steady check
   first; check H only if both hold. `baseline_hashes` unchanged; no solver default changes. Check that the committed mutation
   patches of the F-885 tests still apply, and re-base them in your branch if not, the mutation unchanged.
3. Mutations on scratch copies of the full commit; one committed as a patch. A mutation must be able to change what its test
   reads.
4. Report each row, check H's outcome by its own rule, and what F-885's acceptance needs next, as options with a recommendation.

Decision rule to register: done for F-885 if the order check, the steady check and check H all hold as registered and each
mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: whether the setting becomes a default (not on this
branch), and any question of scope.

Files you own: the setting in the pimple step's files (named in the registration), new tests and investigation files, ADR 0821,
`docs/roadmap/proposals/0821-F-885.json`. No Studio files, no schema files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0821-F-885.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
