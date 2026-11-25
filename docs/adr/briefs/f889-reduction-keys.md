# Brief: F-889, case-file keys for the cutoff, the load's residual vector and the load patterns of the reduction

Branch: f889-reduction-keys
Items: F-889
ADR: 0797
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-25
Size (lead): done; 0; 0; 0; 92884

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f889-reduction-keys`, which the lead made with the branch `f889-reduction-keys` at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f889-reduction-keys-scratch`.

## Goal

Your work is started with this line as its whole prompt:

    /goal The brief docs/adr/briefs/f889-reduction-keys.md (F-889, ADR 0797) is carried out on the branch f889-reduction-keys, checked out in this directory, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of the ADR as the first commit, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `cargo fmt` (check), clippy with `-D warnings` and the tests of rcfd-api and rcfd-fem the brief names, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 80 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0785-f616-registration-case-file-keys-for-superelements-and-interface-enrichment.md 36259 (the tables it added and the form of its rows)
- docs/adr/0474-f552-registration-craig-bampton-residual-vectors-cutoff.md 36128 (the cutoff and the residual vector)
- docs/solid-case-file.md 7506

Also in the repository, as needed: `docs/adr/0511-*` (load patterns) and `docs/adr/0532-*` (E1), the parts on the options; `crates/rcfd-api/src/solid_case.rs` and the tests of F-616 under `crates/rcfd-api/tests`; the reduction in `crates/rcfd-fem/src` (found by search from ADR 0474).

## What the maintainer decided (2026-11-25)

The maintainer's answer of 2026-11-23 ("Yes to both F-616 questions") entered this item with its acceptance; on 2026-11-25 the maintainer answered "Yes" to resuming the roadmap by the priority lines. Decided by the lead (details): the keys belong to the existing table `[reduction]` and are optional; a name of a load that the case does not have, a cutoff that is not positive and finite, a key the reduction cannot use (a load's residual vector without an interior load), and two keys that the library refuses together are refused by key and line, and where the library refuses, its refusal is what is reported. No golden holds the solid schema (ADR 0785, finding 1).

## Task

One registration, `docs/adr/0797-f889-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code or test**: the names, types, units and defaults of the keys, taken from the library's options;
   for each key alone, and for the combinations the library takes, a part file made from a case file against the
   library's own, byte for byte; the configuration E1 of ADR 0532 (cutoff 32964 rad/s, the load's vector, one block of enrichment)
   from a case file against the library's frequencies, in bits; load patterns named by entries of the case's
   loads, with the order of the patterns fixed by the case and not by a map; each refusal with its key and line; a case
   without the keys against records of main made before the change (per platform where a transcendental function enters: the Windows
   and the Linux x86_64 records, printed elsewhere); the schema's diff, additive. If E1 needs a model the case file cannot
   write, stop and report before code.
2. **Change `solid_case.rs`**, the smallest change that does it, and `docs/solid-case-file.md`. No change to the library's reduction.
3. **Tests** in a new file `crates/rcfd-api/tests/solid_case_reduction_keys_f889.rs`. Mutations: each key read and ignored; the
   cutoff in other units; the patterns in another order; the residual vector of another load; each refusal removed;
   a default other than absent. Commit one as `solid_case_reduction_keys_f889.mutation.patch`.
4. Run the new file, the test files of F-616, every `crates/rcfd-api/tests/solid_case*` file, `baseline_hashes` and the case
   hash tables, all unedited, and the checks rule 7 lists.

Decision rule to register: done if every registered row holds, each mutation is caught by its row and the named existing tests pass unedited; partial otherwise, with the key named.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `crates/rcfd-api/src/solid_case.rs` (and a module it already uses, if the registration names it), the new test file and its patch and records, `docs/solid-case-file.md`. No golden, no existing test.

## Common to this work

The acceptance of F-889 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source and no file of `crates/rcfd-fem/src`; if a defect of the library appears, stop and report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0797-F-889.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
