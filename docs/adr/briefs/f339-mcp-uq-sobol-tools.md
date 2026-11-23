# Brief: F-339, MCP tools for tolerance and Sobol studies of a named evaluator

Branch: f339-mcp-uq-sobol-tools
Items: F-339
ADR: 0784
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 0; 173211

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f339-mcp-uq-sobol-tools`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f339-mcp-uq-sobol-tools-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0240-f268-registration-sobol-sensitivity-indices.md 46268 (sensitivity::run and question 2, which gave this item)
- crates/rcfd-api/src/design/uq.rs 33111 (uq::run)
- crates/rcfd-api/src/design/sensitivity.rs 13474 (sensitivity::run)
- crates/rcfd-api/src/design/tools.rs 67367 (the design tools as they stand: the sandbox, the ledger, the evaluators)

Also in the repository, as needed: `crates/rcfd-mcp/src/lib.rs` and `design_tests.rs` for the design tools of the MCP server, the tools-list goldens under `crates/rcfd-mcp/tests/golden/` and the tests that read them (`tool_list_f275.rs`, `tool_list_adr0730.rs`, `project_list_f270.rs`), `docs/mcp-tools.md` and `docs/design-loop.md`.

## What the maintainer decided (2026-11-23)

ADR 0240 built Sobol indices in the library and asked (question 2) for MCP tools; the recommendation was taken on the maintainer's standing instruction of 2026-11-11. The maintainer approved on 2026-11-18 the golden edit this item needs under the class rule of rule 3: a change that only adds entries is re-recorded by the lead, with the diff in the ledger; you report the additive diff and do not edit the goldens. The sentence this work must be able to write: `design_uq` and `design_sobol` run `uq::run` and `sensitivity::run` on a named evaluator, the reduced-order fan evaluator first, inside the sandbox and with the ledger, and every other tool is unchanged.

## Task

One registration, `docs/adr/0784-f339-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the change** the two tools' arguments, replies and errors, the evaluator names they
   accept, the check that each reproduces the library's result on the reduced-order fan evaluator bit for bit, the
   checks of the sandbox and the ledger, and a committed mutation that a registered test must catch.
2. **Add the tools** and their tests; document them in `docs/mcp-tools.md`. Run the registered checks, the MCP tests,
   fmt and clippy with warnings as errors for the crates you touch.
3. Report the tools-list golden diff as additive (entries added, none changed), with the diff; do not edit a golden.

Decision rule to register: done if both tools reproduce the library bit for bit on the registered evaluator inside the sandbox with the ledger and the golden diff is additive; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `crates/rcfd-mcp/src/` (the design tools), `crates/rcfd-api/src/design/tools.rs` additively, new test files, and `docs/mcp-tools.md`. No golden, and not `crates/rcfd-mcp/src/project.rs` beyond the tools table if the tools live there.

## Common to this work

The acceptance of F-339 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. The evaluator is the reduced-order fan model; no field solve. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0784-F-339.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
