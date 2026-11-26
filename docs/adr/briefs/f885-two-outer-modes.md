# Brief: F-885, the two modes of the fan's outer loop: a limiter cycle and a slow contraction

Branch: f885-two-outer-modes
Items: F-885
ADR: 0816
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 76359

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f885-two-outer-modes`, on the branch `f885-two-outer-modes`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f885-two-outer-modes-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f885-two-outer-modes.md (F-885, ADR 0816) is carried out on the branch f885-two-outer-modes, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f885-two-outer-modes, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0816 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0815-f885-registration-the-fans-outer-stall-by-controls.md 30954
- crates/rcfd-api/tests/f885_outer_stall.rs 30716

Also in the repository, as needed: ADRs 0810 and 0811 (the cell record, the alpha-free face flux), ADR 0183 (check H's outer
rule), the pimple step in `crates/rcfd-fv/src/pimple.rs` with its opt-in switches, the Barth-Jespersen limiter of the
gradient code, `docs/investigations/f885-outer-stall/`.

## What the lead decided (2026-11-26)

ADR 0815 ruled out its five hypotheses: with the turbulence sweep or the rotating-frame source frozen within the step,
the seam implicit or the linear tolerances four orders tighter, the fan's outer loop still stalls (C between 0.017 and
0.033 against 1e-2). Its record shows two modes: with the limiter on, the interior cell c11837 changes by the same
5.57e-3 m/s in each outer iteration, a cycle; with the limiter off, the loop contracts by about 0.85 per iteration at c6245,
too slowly. On ADR 0815's questions the lead takes the recommendations, on the maintainer's standing instruction of
2026-11-26 (go with the recommended option always): (1) a further diagnosis of the two modes under F-885 (rule 13); (2) the
two freezes stay opt-in.

## Task

One registration, `docs/adr/0816-f885-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: for c11837, per outer iteration, the cell's momentum diagonal against the sum of its
   off-diagonal magnitudes (the relaxation's d = max(a_c, sum |a_nb|)), the limiter factor of the cell and its neighbours
   and the cell's face fluxes, with the hypotheses for a cycle (a limiter factor that switches between two values; a
   diagonal dominance lost at the cell; a face flux that changes sign) and the number that decides each; for c6245, the
   contraction factor per iteration against alpha_u and the corrector count (a registered grid of each, few runs), with the
   factor each hypothesis predicts; the cavity as a control; no long run; the numbers fixed in advance (rule 15).
2. **Run** as registered, from the switches already on main (ADRs 0811, 0815) and any new opt-in record, defaults
   bit-identical (`baseline_hashes` unchanged); no solver default changes. Check that the committed mutation patches of the
   F-885 tests still apply, and re-base them in your branch if not, the mutation unchanged.
3. Mutations of any new instrumentation on scratch copies; one committed as a patch (rule 16).
4. Report which hypothesis the record supports for each mode, with the numbers, and the remedies it points to as options
   with a recommendation (a remedy is a registration of its own).

Decision rule to register: done for this registration if every run was made as registered and the result names a cause or
rules each hypothesis out for each mode; F-885 stays partial until check H decides.

Open questions for the maintainer, with options and a recommendation: the remedy, and any question of scope.

Files you own: new tests and investigation files, opt-in instrumentation in the pimple step's files and the limiter's file
(named in the registration), ADR 0816, `docs/roadmap/proposals/0816-F-885.json`. No Studio files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0816-F-885.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
