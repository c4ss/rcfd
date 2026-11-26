# Brief: F-885, an opt-in limiter factor kept from the step's first outer iteration, and mode B with ADR 0811's term off and on

Branch: f885-frozen-limiter
Items: F-885
ADR: 0819
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 91768

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f885-frozen-limiter`, on the branch `f885-frozen-limiter`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f885-frozen-limiter-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f885-frozen-limiter.md (F-885, ADR 0819) is carried out on the branch f885-frozen-limiter, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f885-frozen-limiter, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0819 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0816-f885-registration-the-two-modes-of-the-fans-outer-loop.md 34019
- crates/rcfd-api/tests/f885_two_outer_modes.rs 43060

Also in the repository, as needed: ADRs 0810, 0811 and 0815, ADR 0183 and ADR 0767 (check H and its outer rule), the pimple step
in `crates/rcfd-fv/src/pimple.rs` with its opt-in switches and `outer_cell_probe`, the Barth-Jespersen limiter of the gradient
code, `docs/investigations/f885-two-outer-modes/`.

## What the lead decided (2026-11-26)

ADR 0816 named mode A's cause: the x-component Barth-Jespersen factor of c11837 switches every outer iteration (change 0.22 to
0.56), and the velocity cycles with it. Mode B (limiter off, contraction about 0.85 per iteration at c6245) kept dominance and
did not follow alpha_u or the corrector count as its hypotheses predicted. On ADR 0816's questions the lead takes the
recommendations, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always): (1) an opt-in
that keeps each cell's limiter factor from the step's first outer iteration, registered under F-885; (2) a further diagnosis of
mode B, the alpha_u grid of ADR 0816 run with ADR 0811's term off and on. Fixing mode A alone does not let check H decide
(ADR 0815's run with the limiter off still stalls at C 0.017).

## Task

One registration, `docs/adr/0819-f885-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code**: the opt-in (its name, where the factor is kept and reset, every default bit-identical); its
   rows on the fan with the switch of ADR 0811 (mode A's cycle at c11837 gone: the factor's change per outer iteration and
   C over 20 steps of H(dt), with the class rule of ADR 0815); a check that a converged steady case gives the same answer with
   the opt-in on, within a registered tolerance and its source; mode B's grid (alpha_u 0.9, 0.7, 0.5 with ADR 0811's term off
   and on, the limiter off) with the factor each hypothesis predicts; the cavity as a control; no long run; the numbers fixed
   in advance (rule 15).
2. **Build** the opt-in and run as registered. `baseline_hashes` unchanged; no solver default changes. Check that the committed
   mutation patches of the F-885 tests still apply, and re-base them in your branch if not, the mutation unchanged.
3. Mutations on scratch copies; one committed as a patch (rule 16).
4. Report each row, and whether check H can now be run on the fan with the opt-in on, as options with a recommendation.

Decision rule to register: done for this registration if every row holds as registered and each mutation is caught; F-885
stays partial until check H decides.

Open questions for the maintainer, with options and a recommendation: the next step for check H, and any question of scope.

Files you own: the opt-in in the pimple step's files and the limiter's file (named in the registration), new tests and
investigation files, ADR 0819, `docs/roadmap/proposals/0819-F-885.json`. No Studio files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0819-F-885.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
