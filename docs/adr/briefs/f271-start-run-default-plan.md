# Brief: F-271, the field-store plan as the default of start_run

Branch: f271-start-run-default-plan
Items: F-271
ADR: 0741
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-19
Size (lead): done; 0; 0; 0; 103070

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f271-start-run-default-plan` (the lead creates it from main). Scratch folder: `E:/projects/cfd/w49i-work/f271-start-run-default-plan-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- docs/adr/0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md 25977
- docs/adr/0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md 27642
- crates/rcfd-mcp/src/project.rs 37548

## Task

A further registration of F-271 (rule 13): `docs/adr/0741-f271-registration-<slug>.md`. The maintainer approved decision D7
and option (a) of ADR 0186 Q2 on 2026-11-18 (the answers are at the end of ADR 0186): the field-store plan is on by default
for every `start_run` in project mode, not only with `store: "plan"`.

- This is an approved change of a default. The registration lists, case by case, which outputs of `start_run` change (a case
  with no store setting now gets the plan; a case that sets its own store keeps it) and which existing tests or records
  assert the old default. Such a test is not edited: it is retired by this registration as an ignored record and a new test
  asserts the new default (rule 2). A way to ask for the old behaviour stays (name it and test it).
- Rule 3 otherwise holds: `baseline_hashes.rs`, the hash tables and the goldens are not edited. If the MCP tools list or
  a schema golden changes only by added entries, report it as an "additive golden edit" with the diff. If the description
  of an existing tool has to change, that is not additive: leave it out and report it.
- The second half of F-271's gap, whether the Electron Studio (`apps/studio-web`) uses the shared start plan, is a reading
  task: say what you find, with file and line, and propose the follow-up if it does not. Do not change the Studio here.
