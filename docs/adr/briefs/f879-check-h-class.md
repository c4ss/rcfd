# Brief: F-879, FAN-01 hub pocket: check H's class from ADR 0824's runs, torque and pressure rise in time, and what it means for F-870

Branch: f879-check-h-class
Items: F-879
ADR: 0827
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 99849

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f879-check-h-class`, on the branch `f879-check-h-class`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f879-check-h-class-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f879-check-h-class.md (F-879, ADR 0827) is carried out on the branch f879-check-h-class, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f879-check-h-class, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0827 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0766-f879-registration-fan01-check-h-the-hub-pocket-in-time.md 54006
- docs/adr/0824-f885-registration-the-lag-of-the-kept-convection-gradient-its-arms-a-corrected-variant-and-check-h.md 31154

Also in the repository, as needed: ADR 0183 (check H, its classes and their rule), ADR 0767 (the outer-iteration rule), the
records `docs/investigations/f885-kept-gradient-lag/H_dt.csv` and `H_dt2.csv` (one row per step: torque, `dp_ts`, the site
cells), `crates/rcfd-api/tests/f885_kept_gradient_lag.rs` and `f879_fan01_check_h.rs`, `docs/investigations/f879-fan01-check-h/`,
ADR 0762 (F-870's steady runs B1 to B3), `docs/validation-data.md` (FAN-01's measured values).

## What the lead decided (2026-11-27)

F-885 is done (ADR 0824): with the opt-ins `alpha_free_face_flux`, `outer_frozen_limiter`, `outer_frozen_convection_gradient`
and `outer_frozen_convection_gradient_extrapolated_boundary`, check H of ADR 0183 ran on FAN-01's B2 passage at dt and dt/2,
the outer rule of ADR 0767 held in 100 % of the window's steps, and the hub pocket classes STEADY. F-879's acceptance asks
also for the measured amplitude and frequency of torque and pressure rise, and for what the class means for the convergence
rule of F-870, which waits on it (ADR 0766, question 2). This work reads them from ADR 0824's two records; a new run is
registered only if the records cannot decide a row.

## Task

One registration, `docs/adr/0827-f879-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before reading the records' numbers past what ADR 0824 printed**: the window (ADR 0766's), the measures of
   torque and `dp_ts` (mean, peak-to-peak amplitude, the dominant frequency and how it is found, for example a periodogram
   over the window with its resolution, and the record's length in flow-through and blade-passing periods); the comparison of
   dt and dt/2 (a band with its source); the comparison with the steady B2 run of ADR 0762 (2.0287 N m, 64.23 Pa) and the
   measured values; the class's meaning for F-870 as options for its convergence rule (for example a steady solve's residual
   rule with the torque's change over the last iterations, or a time average), each with what it needs; a test that reads the
   committed records and holds the numbers; negative controls and mutations (rule 16); the numbers fixed in advance (rule 15).
2. **Compute** as registered, from the committed records. No solver change; `baseline_hashes` unchanged.
3. A test in a new file reads the records and holds each measured row; one mutation committed as a patch, its expect-fail
   command with `--target-dir target/mutation`.
4. Report each row, F-879's verdict by ADR 0766's table, and the options for F-870's convergence rule with a recommendation.

Decision rule to register: done for F-879 if every row is decided and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: F-870's convergence rule, and any question of scope.

Files you own: a new test file, new investigation files, ADR 0827, `docs/roadmap/proposals/0827-F-879.json`. No solver files,
no Studio files.

## Common to this work

The acceptance of F-879 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0827-F-879.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
