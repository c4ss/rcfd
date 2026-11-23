# Brief: F-856, UDF interpreter: wasmi 0.31 to 1.x

Branch: f856-wasmi-1x
Items: F-856
ADR: 0782
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 0; 72272

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f856-wasmi-1x`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f856-wasmi-1x-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- crates/rcfd-api/src/udf.rs 24093 (the one user of wasmi: the engine, the store, the fuel and the imports)
- crates/rcfd-api/tests/udf.rs 15054 (the UDF tests that must pass unedited)
- crates/rcfd-api/tests/udf_hooks_w38a.rs 18737 (the hook tests that must pass unedited)
- deny.toml 1397 (the licence and advisory rules cargo deny applies)

Also in the repository, as needed: `crates/rcfd-cli/tests/check_inputs_adr0739.rs`, the README's section Build and test (the minimum Rust), `Cargo.toml` and `Cargo.lock`, `crates/rcfd-api/tests/baseline_hashes.rs`, and the wasmi 1.x interface as its crate's own source and API listing on docs.rs show it (a dependency's interface is not a source of an algorithm; nothing of a CFD vendor's documentation is used).

## What the maintainer decided (2026-11-23)

Dependabot's pull request 31 moved wasmi from 0.31.2 to 1.1.0 in `Cargo.toml` and `Cargo.lock` alone and failed clippy, the type-check with Rust 1.90 and the unit tests, because `udf.rs` uses the 0.31 interface. The maintainer decided ("3. Yes as recommended") to port `udf.rs` to wasmi 1.x, for the fixes of wrong translation in 1.0.5 to 1.0.9; the pull request is closed until this item is done. The sentence this work must be able to write: the workspace uses the newest wasmi 1.x, every existing UDF test passes unedited, the limits and the refusal of an import behave as before, and the baseline hashes are unchanged.

## Task

One registration, `docs/adr/0782-f856-registration-<slug>.md`, the first commit of this work, alone.

1. **Before any code**, read the minimum Rust that the newest wasmi 1.x declares. If it is above the
   README's minimum for the type-check (Rust 1.90), stop and report it to the lead with options and a recommendation;
   do not decide it.
2. **Register** the version, the checks of the acceptance (the fuel or step limit, the memory limit, the refusal of a
   module that imports anything not offered, each by an existing or a new test), and a committed mutation that a
   registered test must catch.
3. **Port `udf.rs`** and run clippy with warnings as errors, the type-check with Rust 1.90 if it is installed (report
   if not), the unit tests of rcfd-api, the three named test files unedited, `baseline_hashes` and `cargo deny check`.

Decision rule to register: done if every check of the acceptance holds with the newest wasmi 1.x; partial if one does not, with its output; todo if the minimum Rust stops the work at step 1.

Open questions for the maintainer, with options and a recommendation, not decided by you: the minimum Rust of wasmi 1.x, if it is above the README's minimum, with options and a recommendation.

Files you own: `crates/rcfd-api/src/udf.rs`, the wasmi line of `Cargo.toml`, `Cargo.lock`, and new test files. No other source, no existing test file, no golden or hash table.

## Common to this work

The acceptance of F-856 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. The solver's results must not change: the baseline hashes hold the default bits. If a defect of the solver appears, stop and report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0782-F-856.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
