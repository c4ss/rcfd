# Brief: F-672, the whirl and full_rotor beam opt-ins lifted by default

Branch: f672-beam-opt-ins-default
Items: F-672
ADR: 0799
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-25
Size (lead): done; 0; 0; 0; 73637

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f672-beam-opt-ins-default`, which the lead made with the branch `f672-beam-opt-ins-default` at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f672-beam-opt-ins-default-scratch`.

## Goal

Your work is started with this line as its whole prompt:

    /goal The brief docs/adr/briefs/f672-beam-opt-ins-default.md (F-672, ADR 0799) is carried out on the branch f672-beam-opt-ins-default, checked out in this directory, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of the ADR as the first commit, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `cargo fmt` (check) and clippy with `-D warnings` of rcfd-fem, and the tests the brief names, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0744-f672-registration-beam2-in-the-cyclic-constraint-by-default.md 35739 (the form of a default lift: the changed results case by case, the old refusals kept as ignored records and restated, the maintainer's answer to question 1)
- docs/adr/0601-f674-registration-full-rotor-and-expansion-of-a-cyclic-sector-with-beam2.md 24907 (what `full_rotor_beams` opts into)

Also in the repository, as needed: `CyclicOptions` in `crates/rcfd-fem/src/model.rs` (`beams`, `full_rotor_beams`), `crates/rcfd-fem/src/{cyclic,rotating}.rs` (`RotatingOptions::beams` and the whirl paths), ADR 0598 (F-675) and ADR 0602 (F-679) for the whirl rows, and the test files that set either option or assert a refusal without it (found by search: `sa_cyclic_beams_*`, `sa_rotating_cyclic_beams_f675.rs`, `sa_ring_whirl_beams_f679.rs`, `sa_free_sector_prestress_f680.rs`).

## What the maintainer decided (2026-11-25)

On 2026-11-21 the maintainer answered ADR 0744 question 1 "Yes, both" on its recommendation (b): the whirl paths and full_rotor take a beam sector without the `beams` and `full_rotor_beams` options, in one further registration, the two options kept as no-ops. That is this work. Decided by the lead (details): the options stay in the API, accepted and without effect, with their doc comments saying so; a call that sets an option on gives bits equal to a call that does not; a model without beams keeps its bits.

## Task

One registration, `docs/adr/0799-f672-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code or test**: every refusal that lifts (path, error, the test that asserts it now), and every
   result that changes, case by case; the rows by which a beam sector without the options equals the same sector with them,
   bit for bit, on each whirl path and on full_rotor; models without beams and calls with the options set, against a record of
   main made before the change, on Windows and Linux x86_64; the refusals that stay, by name. Each old assertion of a refusal
   becomes an ignored record and is restated in a new test, as ADR 0744 did.
2. **Change rcfd-fem**, the smallest change that does it.
3. **Tests** in a new file `crates/rcfd-fem/tests/sa_beam_opt_ins_default_f672.rs` (and a bits file if the registration needs one).
   Mutations: each option made to matter again, per path; the refusal kept on one path; a beam sector given the old default.
   Commit one as a patch.
4. Run the new files, every test file of rcfd-fem whose name holds cyclic, rotating, whirl, beam or full_rotor, `cargo test -p
   rcfd-fem --release --lib` and `baseline_hashes`, all unedited except the old refusal assertions made ignored records, and
   the checks rule 7 lists.

Decision rule to register: done if every registered row holds, each mutation is caught, the named existing tests pass with only the registered refusal assertions made ignored records, and models without beams are bit-identical to main; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `crates/rcfd-fem/src` as the registration names it, the new test files with their patch and records, and the old refusal assertions made ignored records with their restated rows. No other crate, no golden.

## Common to this work

The acceptance of F-672 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. This work changes the structural library; a defect found outside its paths is reported, not repaired. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0799-F-672.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
