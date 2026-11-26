# Brief: F-975, the diagnosis test DG1 of the wheel sector fails on Linux

Branch: f975-dg1-on-linux
Items: F-975
ADR: 0814
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 56156

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f975-dg1-on-linux`, on the branch `f975-dg1-on-linux`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f975-dg1-on-linux-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f975-dg1-on-linux.md (F-975, ADR 0814) is carried out on the branch f975-dg1-on-linux, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f975-dg1-on-linux, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0814 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of rcfd-fem, its `--lib` tests on Windows and in the Ubuntu 24.04 container, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0806-f975-registration-normwise-deflation-fallback-of-the-state-qr.md 34541
- crates/rcfd-fem/src/rotating/whirl/state/f975_diagnosis.rs 6926

Also in the repository, as needed: the whirl state solver beside it, ADR 0799 (F-672), the lead's re-record of H_W in
`crates/rcfd-fem/tests/sa_beam_opt_ins_default_f672_bits.rs` (Windows and Linux values differ), the container image
`rcfd-linux:ubuntu24.04-rust1.99.0` and the record-run recipe in `docs/process/lead-process.md` or the reference notes.

## What the lead decided (2026-11-26)

On GitHub Actions (run 45881197914, ubuntu-24.04, `cargo test --workspace --release --locked --lib`), the lib test
`rotating::whirl::state::f975_diagnosis::dg1_dg2_dg3_the_stalled_window_of_the_wheel_sector` fails at its DG1 assertion
(f975_diagnosis.rs:177); it passes on Windows, where ADR 0806 registered it. DG1 asserts which (k, omega) pairs stall
before the fallback: exactly (1, 0) and (2, 0). The floating-point path differs on Linux (H_W differs between the two
platforms too). It is a further registration under F-975 (rule 13): the test is not ignored, skipped or loosened to pass.

## Task

One registration, `docs/adr/0814-f975-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: a run of the test in the Ubuntu 24.04 container (the image named above, `--release
   --locked`) that prints DG1's stalled set; the hypotheses for a difference (the stall's threshold met on one platform and
   not the other near its limit; an order of summation; a library's result), each with its control; a rule fixed for each
   outcome; and what the remedy would be for each: a test whose stall set is stated per platform from a registered run, with
   the reason (rule 2: a known miss becomes an ignored `*_known_miss` record only by an amendment), or a diagnosis input
   that stalls on both platforms for the same reason, or a code defect.
2. **Run** as registered; then the remedy, only in the files the registration names. No solver default changes; H_W and
   `baseline_hashes` unchanged.
3. Mutations on scratch copies, each recorded with what caught it; one committed as a patch (rule 16).
4. Run the checks rule 7 lists, on both platforms.

Decision rule to register: done if the test passes on both platforms as registered and each mutation is caught; partial
otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: `crates/rcfd-fem/src/rotating/whirl/state/f975_diagnosis.rs`, new tests, ADR 0814,
`docs/roadmap/proposals/0814-F-975.json`.

## Common to this work

The acceptance of F-975 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0814-F-975.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
