# Brief: F-895, design_uq and design_sobol refuse a key of another distribution in dist

Branch: f895-dist-foreign-keys
Items: F-895
ADR: 0793
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-25
Size (lead): done; 0; 0; 0; 82898

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: the one your session starts in; make the branch there from origin/main at the commit that adds this brief (the goal says how). Scratch folder: `E:/projects/cfd/w49i-work/f895-dist-foreign-keys-scratch`.

## Goal

Your session is started with this line as its first prompt:

    /goal The brief docs/adr/briefs/f895-dist-foreign-keys.md (F-895, ADR 0793) on origin/main is carried out on the branch f895-dist-foreign-keys, and this conversation shows each of these with the command and its output: (1) the branch was made in this session's worktree with `git fetch origin` and `git switch --no-track -c f895-dist-foreign-keys origin/main`, and AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0793 as the first commit, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the final outputs are shown of `cargo fmt -p rcfd-api -p rcfd-mcp -- --check`, clippy of both crates with `-D warnings`, the tests the brief names, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; (6) `git status --short` is empty, nothing is pushed or merged, and docs/roadmap.json, the ledger, goldens and existing tests are unchanged; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 60 turns.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0791-f339-registration-arguments-of-design-uq-and-design-sobol.md 32103 (finding 7, the form of the rows and mutations, the lead's readings)
- crates/rcfd-mcp/tests/design_uq_sobol_args_f339.rs 37804 (the argument tests as merged; none is edited)

Also in the repository, as needed: `crates/rcfd-api/src/design/tools.rs`: `object_at` and `study_params` (the one list of allowed keys of `dist`), `Dist::to_json` in `crates/rcfd-api/src/design/uq.rs`, the two tools' section of `docs/mcp-tools.md`, `crates/rcfd-mcp/src/lib.rs` (`validate_args` and the two descriptors, to see what reaches the handler), `f894_bit_identity.rs` (how a record of main is compared per platform), and ADR 0792's records of main.

## What the maintainer decided (2026-11-25)

On 2026-11-25 the maintainer answered "Yes, issue it and when you start new roadmap pieces make sure they write a clear `/goal` tool/skill call" to a check that refuses another distribution's keys (finding 7 of ADR 0791). `study_params` allows type, lo, hi, mean, sd, a and b for every type and ignores those the type does not have, so a uniform sent with a `mean` runs without a word. Decided by the lead: the key is refused whatever its value, `null` among them, so one rule holds for all; a key of no distribution keeps its present message; a type that is not known is refused first, as now. The sentence this work must be able to write: each of the eight foreign keys is refused by name in both tools with nothing written, and every other call gives main's bytes.

## Task

One registration, `docs/adr/0793-f895-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code or test** one row for each foreign key (mean, sd, a, b of a uniform; a, b of a normal;
   mean, sd of a beta) with a number, `null` and a string as its value, in both tools, through the handler and through the
   server, and for the second of two inputs as well as the first: the error code, the message (the path of the key and the
   keys of the type), no sample drawn and no file written. A row for two foreign keys at once (which is named, fixed
   by the rule and not by chance). Rows of what must not move: each `Dist::to_json` form sent back, every form of the
   open sides, and the results of both drivers against a record of main made before the change; the message of a key of no
   distribution and of a type that is not known. Say, from reading, whether another reader of `dist` or of a like
   tagged object in `tools.rs` has the same weakness; report it, do not change it.
2. **Change `study_params`**, the smallest change that does it. If `docs/mcp-tools.md` states the rule wrongly or not at
   all, correct that sentence; the tool-list text is a golden and is not changed.
3. **Tests** in a new file `crates/rcfd-mcp/tests/design_dist_keys_f895.rs`. Mutations: for each of the eight, the key
   accepted again, caught by its row; the check after the study has started; the message without the path. Commit one as
   `design_dist_keys_f895.mutation.patch`.
4. Run the new file, `design_uq_sobol_args_f339`, `design_uq_sobol_f339`, `f894_one_open_side_tools`, `cargo test -p
   rcfd-mcp --release`, `cargo test -p rcfd-api --release --lib`, all unedited, and the checks rule 7 lists.

Decision rule to register: done if every registered row holds, each mutation is caught by its row, the named existing tests pass unedited and the results that must not move are bit-identical to main; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `study_params` in `crates/rcfd-api/src/design/tools.rs`, `crates/rcfd-mcp/tests/design_dist_keys_f895.rs`, its mutation patch and its record of main, and one sentence of `docs/mcp-tools.md`. No golden, no existing test. If an existing test sends a foreign key, stop and report.

## Common to this work

The acceptance of F-895 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). No other agent of
the lead runs when this is issued; the machine may carry other load; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0793-F-895.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.

Started 2026-11-25 as a session of the app; the `/goal` line was in its first prompt behind other text and was not set as a command, as the session said; it works to the line as text.
