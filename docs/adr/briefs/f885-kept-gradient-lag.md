# Brief: F-885, the lag of the kept convection gradient that lowers the remedy's time order

Branch: f885-kept-gradient-lag
Items: F-885
ADR: 0824
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 92684

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f885-kept-gradient-lag`, on the branch `f885-kept-gradient-lag`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f885-kept-gradient-lag-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f885-kept-gradient-lag.md (F-885, ADR 0824) is carried out on the branch f885-kept-gradient-lag, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f885-kept-gradient-lag, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0824 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0822-f885-registration-the-order-floor-of-the-pimple-step-a-finer-order-case-and-check-h.md 31922
- crates/rcfd-api/tests/f885_order_case.rs 46073

Also in the repository, as needed: ADRs 0820 and 0821 (the setting `outer_frozen_convection_gradient`, its steady check), ADR
0073 (the commutator floor of the default step), ADR 0767 and ADR 0183 (check H), the pimple step in `crates/rcfd-fv/src/pimple.rs`
(the kept gradient and its boundary values near lines 2191 to 2200), `docs/investigations/`.

## What the lead decided (2026-11-27)

ADR 0822 found that the first order of ADR 0821's case is the default step's O(dt h^2) commutator floor (ADR 0073): at N = 64
the step is second order without the setting (2.0345) and the wall-bounded control holds (2.0320). With the setting on the
order is 1.8222 against 1.9, and e_ON/e_OFF grows as dt falls (1.36, 1.43, 1.66), so the kept gradient adds an error of lower
order; from the code, its boundary values (`bv`) may lag by O(dt) on the periodic seam. On ADR 0822's questions the lead takes
the recommendations, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always): (1) a
diagnosis under F-885 of the kept gradient's lag, with its arms fixed in advance; (2) the commutator floor stays recorded, and
order checks use the opt-in `rhie_chow_step_consistent` or a finer mesh; (3) no new item.

## Task

One registration, `docs/adr/0824-f885-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code or run**: the hypotheses for the lag (the kept gradient's boundary values; its periodic partner
   values; the time level at which it is kept; another source found in the code), each with its file and line, the order or
   the ratio e_ON/e_OFF it predicts and its arm (for example the boundary values extrapolated, `periodic_partner_extrapolated`,
   the gradient kept at a registered time level), on N = 64 and on N = 16 with `rhie_chow_step_consistent`; the corrected
   remedy, if an arm restores the order, as an opt-in variant of the setting (defaults bit-identical) with its order check
   (`p_f >= 1.9` setting on and off) and ADR 0821's steady check; then check H as ADR 0821 registered it, only if both hold;
   mutations (rule 16); the numbers fixed in advance (rule 15).
2. **Run** as registered. `baseline_hashes` unchanged; no solver default changes. Check that the committed mutation patches of
   the F-885 tests still apply, and re-base them in your branch if not, the mutation unchanged.
3. Mutations on scratch copies of the full commit; one committed as a patch. A mutation must be able to change what its test
   reads, and its test must pass without it.
4. Report each row, check H's outcome by its own rule if run, and what F-885's acceptance needs next, as options with a
   recommendation.

Decision rule to register: done for F-885 if the corrected remedy's order check and steady check hold, check H holds as
registered and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: the corrected variant in the pimple step's files (named in the registration), new tests and investigation files,
ADR 0824, `docs/roadmap/proposals/0824-F-885.json`. No Studio files, no schema files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0824-F-885.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
