# Brief: F-339, the two test gaps of design_uq and design_sobol a reader found (the string and null bounds, and inputs that cannot tell a wrong reading)

Branch: f339-uq-sobol-test-gaps
Items: F-339
ADR: 0791
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-25
Size (lead): done; 0; 0; 0; 84980

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f339-uq-sobol-test-gaps`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f339-uq-sobol-test-gaps-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0784-f339-registration-mcp-tools-for-tolerance-and-sobol-studies-of-the-reduced-order-fan.md 38927 (the registration, addendum 1 (X1 and deviation 2), the reader's report, the maintainer's answer on open question 1)
- crates/rcfd-mcp/tests/design_uq_sobol_f339.rs 33062 (the tests as merged: the reference models and inputs, and extra_model_and_quantiles_reach_the_library)

Also in the repository, as needed: `crates/rcfd-api/src/design/tools.rs` from `bound_at` on (the argument reading of the two tools), `docs/mcp-tools.md` where it states the two tools' arguments, the library's `design::uq` and `design::sensitivity`, the committed mutation `crates/rcfd-mcp/tests/design_uq_sobol_f339.mutation.patch`, and the reader's records, read only: `E:/projects/cfd/w49i-work/f339-reader-golden-edit-scratch`.

## What the maintainer decided (2026-11-25)

On 2026-11-25 the maintainer answered "Yes, issue it" to one agent for F-339's two test gaps, and "Yes, MCP only" to ADR 0784, open question 1 (no service methods). F-339 was merged on 2026-11-24 as partial: the reader's finding 1-1 (no test sends `"-inf"`, `"inf"` or `null` as a bound, though deviation 2 of addendum 1 says the extra test covers the string bounds) and finding 2-1 (the reference inputs give the same bytes with a swapped Beta `a` and `b` (2, 2), a dropped `mean` or `system_l` (0) or the default `seed`; `circular_arc`, `cmp "<="` and a non-zero `se_abs` are never sent). The sentence this work must be able to write: for each named wrong reading a committed or recorded mutation makes it, a registered test fails with the mutation and passes without it. X1's miss stays in the record as it is; the lead decides at the merge what it means for the status.

## Task

One registration, `docs/adr/0791-f339-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any test is written** one row per branch and per wrong reading named in findings 1-1 and 2-1:
   the input sent, the library call it must equal bit for bit, and the mutation of `tools.rs` that the row must catch. At
   least: `lo` and `hi` as `"-inf"`, `"inf"`, `null` and absent, in a study where a finite bound would change the bytes;
   a refusal of any other string; Beta `a` and `b` swapped; `mean` dropped; `system_l` dropped; `seed` dropped (the default
   used); `circular_arc` read as `naca4`; `cmp "<="` read as its opposite; `se_abs` dropped. Read the code for other
   arguments of the two tools with the same weakness and register them too, or say there is none.
2. **Write the tests** as a new file `crates/rcfd-mcp/tests/design_uq_sobol_args_f339.rs`. A row whose input the two
   readings cannot tell apart shows that first: assert in the test that the library's result differs between the right
   and the wrong input, so equality can fail. Existing tests are not edited.
3. Record every mutation run, and commit one as `design_uq_sobol_args_f339.mutation.patch` in the form of the existing
   one. If a test shows `tools.rs` wrong, record it as a failure first, register an amendment, then fix it; if the fix would
   change a golden or a default result, stop and report.
4. Run the new file, the existing `design_uq_sobol_f339`, `cargo test -p rcfd-mcp --release` and `cargo test -p rcfd-api
   --release --lib` unedited on your branch, and the checks rule 7 lists.

Decision rule to register: done if every registered row holds, each named wrong reading is caught by its mutation, and the existing tests of rcfd-mcp and rcfd-api --lib pass unedited; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `crates/rcfd-mcp/tests/design_uq_sobol_args_f339.rs` and its mutation patch; `crates/rcfd-api/src/design/tools.rs` only by an amendment under task 3. No golden, no tool-list text, no count literal.

## Common to this work

The acceptance of F-339 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0791-F-339.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
