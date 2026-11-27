# Brief: F-870, FAN-01 design point: a time march to a settled state on B1 at two steps as F-870's convergence rule

Branch: f870-time-march-b1
Items: F-870
ADR: 0831
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 109004

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f870-time-march-b1`, on the branch `f870-time-march-b1`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f870-time-march-b1-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f870-time-march-b1.md (F-870, ADR 0831) is carried out on the branch f870-time-march-b1, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f870-time-march-b1, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0831 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0827-f879-registration-check-h-class-torque-and-pressure-rise-in-time.md 26683
- docs/adr/0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md 67632

Also in the repository, as needed: ADR 0824 (the four opt-ins of the pimple step and check H's runs on B2), ADR 0766 (check
H's case and its start state), `crates/rcfd-api/tests/f870_fan01_design_point.rs`, `f879_check_h_class.rs` and
`f885_kept_gradient_lag.rs`, `docs/validation-data.md` (FAN-01's measured values), ADRs 0765 and 0769 (the compared quantities
beside the pressure rise), `docs/investigations/f879-check-h-class/`.

## What the lead decided (2026-11-27)

F-879 is done (ADR 0827): on B2 the hub pocket classes STEADY with the four opt-ins of ADR 0824, and torque and pressure rise
are steady to 1 % of their mean at dt and dt/2. On its questions the lead takes the recommendations, on the maintainer's
standing instruction of 2026-11-26 (go with the recommended option always): (1) F-870's convergence rule is option (c) of
ADR 0827 section 4, a time march to a settled state on B1 (`pimple` with the four opt-ins, uniform start, `dt` and `dt/2`),
converged when the half range over mean of torque and `dp_ts` over the last flow-through time is at most 1 % and the two
steps' means are within 1 %; (2) F-879's class holds with the four opt-ins, as for F-885. This work runs it on B1 and
compares the design-point quantities of F-870 with it.

## Task

One registration, `docs/adr/0831-f870-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the case (B1, the opt-ins, the start, `dt` and `dt/2` with their sources, the time run and
   its length in flow-through times, a wall-clock record); the convergence rule of option (c) as ADR 0827 fixed it; the
   quantities F-870 compares (pressure rise, shaft power, and beside them the quantities of ADRs 0765 and 0769) with their
   bands as F-870's acceptance and ADR 0762 state them, read from the converged state; what a run that does not settle
   records; the comparison with ADR 0762's B1 SIMPLEC sample; negative controls and mutations (rule 16); the numbers fixed in
   advance (rule 15).
2. **Run** as registered: the two runs in the background, each once, its record committed under
   `docs/investigations/`. `baseline_hashes` unchanged; no solver default changes.
3. A test in a new file reads the committed records and holds each row; one mutation committed as a patch, its expect-fail
   command with `--target-dir target/mutation`.

Decision rule to register: done for F-870 if the march settles by the rule, every compared quantity is within its band and
each mutation is caught; partial otherwise, with each miss recorded with its number.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: a new test file, new investigation files, ADR 0831, `docs/roadmap/proposals/0831-F-870.json`. No solver files,
no Studio files.

## Common to this work

The acceptance of F-870 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0831-F-870.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
