---
type: source
tags: [adr-thread, workspace, mcp, studio, project-folder, runner, design-study]
sources: [doc-mcp-tools, doc-run-control, doc-architecture]
origin: docs/adr/ (ADR list below)
origin_commit: 82985b47
created: 2026-11-22
updated: 2026-11-25
status: draft
---

# ADR thread: one workspace layer between the project folder, the MCP server and the Studio (F-270 to F-275, F-287, F-288)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread: one proposal (ADR 0173) and its six amendments, which are the registrations of F-270 to F-275, two follow-up registrations (F-287, F-288) and a further registration of F-271 under rule 13; 10 ADRs, 2026-11-04 to 2026-11-21.

ADR 0173 (2026-11-04) answers three requirements of the maintainer: "R1 The MCP server can run new simulations directly", "R2 The MCP server can edit and set up meshes", "R3 The Studio updates from the MCP server's changes". Its principle: "in project mode, a capability exists only as a workspace command that writes the project folder, and the MCP server and the Studio offer the same commands. So whatever Claude changes, the Studio shows, and whatever the Studio can change, Claude can change." It proposes one crate, `rcfd-workspace`, and eight decisions D1 to D8, each with a recommendation; the maintainer set F-270 to F-275 as "the current priority on 2026-11-04, ahead of the FAN-01 diagnosis". Each item then got its own amendment (ADRs 0185 to 0189, 0195) that fixed the check limits before any code. All six were built on 2026-11-05 and 2026-11-06; the egui Studio they were written against was deleted on 2026-11-16, and the maintainer's five answers of 2026-11-18 re-scoped what remained to the Electron Studio.

## Current state

- **ADR 0173 itself** is still "proposed 2026-11-04, revised the same day ... Not yet accepted by the maintainer. Decisions D1 to D8 below are open" in its status line. Its decisions were built through the amendments on their recommendations, and two were later answered by the maintainer: D7 (the shared start plan) approved on 2026-11-18 (ADR 0186 answer Q2), and D2 (b) / D5 (b) built by F-275 (ADR 0195) and accepted in part on 2026-11-06 and 2026-11-18.
- **F-270 ~~partial~~ done** (the lead's reading of 2026-11-25, 82985b47) (ADR 0185): the crate exists and the MCP project tools are an adapter over it ("`rcfd-mcp/src/project.rs` keeps the 17 descriptors byte-for-byte and is now an adapter around them (272 lines, from 1,286)"). The maintainer's answer of 2026-11-18 re-scoped the acceptance to the Electron Studio, which "saves a project case through `edit_case`, which writes the changed lines and keeps comments". The roadmap gap: the Electron Studio's save tests "need the engine, and the hosted job packages without it. The lead confirms a local pass and then decides done."
- **F-271 done** (ADRs 0186, 0741): `new_case` creates a case under `cases/` and `start_run {new_case}` starts it in one call. Since ADR 0741 (2026-11-19) "the field-store plan is the default of every start_run in project mode" (`PLAN_FOR_PROJECT_CASES` true), with `store: "none"` as the way back. The description of `start_run`'s `store` argument is now wrong ("Default plan with new_case, none with case"); the maintainer approved its correction on 2026-11-21 as a separate item, F-862 (todo).
- **F-272 ~~partial~~ done** (the lead's reading of 2026-11-25, 82985b47) (ADR 0187): `import_mesh`, `check_case_mesh`, `preview_mesh` and `list_meshes`, and a mesh check inside `edit_case`. "A mesh recipe format is not included (a later ADR)." Mesh-size limits: the 2026-11-06 answer (take the Studio's limits after F-293) was withdrawn on 2026-11-18; "no size limit on a mesh build, as built, until a large mesh is seen to block the server".
- **F-273 partial** (ADR 0188): the project journal `.rcfd/edits.jsonl` with `seq`, `actor`, `changes` and `reason`, and the change feed `read_feed`. Linking an MCP actor to a Claude Code session stays not built ("Q4 stays (c)"). What the egui Studio deferred to F-293 (an actor on run starts, a feed panel, a notes list) is to be checked in the Electron Studio and issued as Studio items (2026-11-18). The Electron Studio names its actor through `rcfd-mcp --client` (ADR 0801 Addendum 23).
- **F-273 stays partial, with another gap** (the lead's reading of 2026-11-25 in ADR 0188, 82985b47): the Studio's actor and the activity line are confirmed by its tests of 2026-11-24, but no view of the Electron Studio shows the reason (why) of a feed entry, which the acceptance asks. F-274's journal lines are issued (ADR 0794).
- **F-274 partial** (ADR 0189): with `--project`, `design_run_study` and `design_evaluate` evaluate each candidate as one runner run tagged `study`, `variant`, `condition` in `run.json`. No journal line for design runs (Q6, "no item was registered").
- **F-275 ~~partial~~ done** (the lead's reading of 2026-11-25, 82985b47) (ADR 0195): with `--project` "the 16 session-only tools are neither listed nor callable (40 tools)"; the plugin's edit hook "refused a Write and an Edit under `cases/` in a real Claude Code 2.1.319 session". The hook "fails open when it cannot decide", Bash and PowerShell writes are not guarded, and its behaviour under `bypassPermissions` stays untested (accepted 2026-11-06, to be stated in the plugin's documentation, 2026-11-18).
- **F-287 and F-288 ~~partial, nothing measured missing~~ done** (the lead's reading of 2026-11-25, 82985b47) (ADRs 0200, 0201): runner runs that continue from another run with a step cap, and adoption of matching runs after an MCP server restart. Fields approved on 2026-11-18; "The lead decides the status of F-287 by the registered rule."

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0173](../../docs/adr/0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) | 2026-11-04 | proposal (hub), D1 to D8 | F-270 to F-275 | One layer that both the MCP server and the Studio use in project mode | proposed, not accepted; built item by item through the amendments | the amendments below |
| [0185](../../docs/adr/0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) | 2026-11-05 | amendment 1, registration | F-270 | Extract `rcfd-workspace`; move the MCP server and the egui Studio onto it | built E1 to E8, A1 to A7 met; gate 2791 / 0 / 799; partial after the re-scope of 2026-11-18 | |
| [0186](../../docs/adr/0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md) | 2026-11-05 | amendment 2, registration | F-271 | `new_case` and `start_run {new_case}` with the shared start plan | met (C-7, C-S, C-P, C-N, C-L, C-1); 51 tools; plan on by default only with `new_case` until ADR 0741 | 0741 (the default) |
| [0187](../../docs/adr/0187-adr-0173-amendment-3-f272-registration-mesh-set-up-from-the-mcp-server.md) | 2026-11-05 | amendment 3, registration | F-272 | Mesh import, check, preview and listing from the MCP server | met (C8-I, C8-C, C8-P, C8-E, C8-L, C-M, C-L); 55 tools; public elbow not fetched | |
| [0188](../../docs/adr/0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) | 2026-11-05 | amendment 4, registration; addendum 2 by the lead | F-273 | Journal with actor, changes and reason; one change feed | met; 56 tools; journal lock wait raised 250 ms to 2 s (2026-11-12) | |
| [0189](../../docs/adr/0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md) | 2026-11-05 | amendment 5, registration | F-274 | Design-study evaluations as runner runs | met for single-condition studies; chained studies and `max_steps` left to F-287, restarts to F-288 | 0200, 0201 (the deferred parts) |
| [0195](../../docs/adr/0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md) | 2026-11-06 | amendment 6, registration | F-275 | Hide session tools in project mode; plugin `PreToolUse` edit hook | built P1 to P11; 40 tools with `--project`; gate 2918 / 0 / 821 | |
| [0200](../../docs/adr/0200-f287-registration-runner-runs-that-continue-from-a-previous-run-with-a-step-cap.md) | 2026-11-06 | registration | F-287 | `rcfd run --continue-from` and a step cap as runner run settings | met: a run continued after 25 steps ends where a 50-step run does, bit for bit; chain agrees to 6.7e-10 relative (limit 1e-4) | |
| [0201](../../docs/adr/0201-f288-registration-adopt-matching-runs-after-an-mcp-server-restart.md) | 2026-11-06 | registration | F-288 | Adopt a run with the same tags, snapshot hash and entries after a restart | met: no new run, three adopted, records bit for bit those of an uninterrupted study | |
| [0741](../../docs/adr/0741-f271-registration-the-field-store-plan-as-the-default-of-start-run.md) | 2026-11-19 | registration (rule 13, approved default change) | F-271 | The field-store plan as the default of every `start_run` | done by its rule, rows 1 to 12 met; one additive golden edit for the lead | F-862 (description of `store`), done by ADR 0774 on 2026-11-23 ([[adr-thread-mcp-tools]]) |

## Key numbers and limits

- **Tool counts as each amendment left them** (C-L checks, byte-compared against the F-270 golden, never re-recorded): 50 tools, 38 978 bytes (ADR 0185 A2); 51 tools, 42 103 bytes (ADR 0186); 55 tools, 45 592 bytes (ADR 0187); 56 tools, 48 294 bytes (ADR 0188); with `--project` after F-275, 40 tools (ADR 0195). Later counts are on [[adr-thread-mcp-tools]].
- **The two differences F-270 found** (ADR 0185): `replace_if_hash` creates a missing parent folder, `save_if_base` did not; the Studio's worker waits up to `EDIT_LOCK_WAIT` (2 s) for a held lock, and a lock left by a crashed writer blocks saves "until the lock is 30 s old (`EDIT_LOCK_STALE`)".
- **The start plan** (ADR 0186 C-7, ADR 0741 rows 1 to 3): `cases/sod.toml` gets `store.set` [`output.store_time_interval=0.002`, `output.store_precision=f32`], `snapshots` 100, `estimated_bytes` 160 000; a plain 30-step case gets [`output.store_interval=1`, `output.store_precision=f32`], `snapshots` 30, `estimated_bytes` 69 120. Row 6: the store does not change the solution (`lid_drag` -1.025564180656281e-2 in both, equal bits).
- **ADR 0741's additive golden edit** (for the lead to re-record): `start_run` in `crates/rcfd-mcp/tests/golden/project_shapes.json` gains `store`; "Until it is re-recorded, `cargo test -p rcfd-mcp --release --test project_runs_w48f` fails in `f3_f11_project_tools_on_runner_made_runs` at this shape and nowhere else."
- **Mesh checks** (ADR 0187): the wedge's `max` non-orthogonality 19.911 degrees equal to the report's; the sheared Fluent mesh (max non-orthogonality 87.1 degrees) is written with `mesh.ok` false; "an edit whose mesh builds but fails the quality check is written with `mesh.ok` false, and `rcfd run` still refuses such a mesh" (accepted 2026-11-18).
- **Feed latency** (ADR 0188 C3): an `edit_case` with a reason was in another process's feed "at poll 1 of 4 (0.004 s after the result was read)"; a plain write showed as `external` "at poll 3 of 4 (0.508 s)"; the egui Studio showed an MCP edit after 0.322 s (limit 2 s). Reasons over 500 characters are -32602.
- **Journal lock** (ADR 0188 addendum 2): `LOCK_WAIT` raised from 250 ms to 2 s after `c3s_two_appenders_without_a_shared_edit_lock_number_every_line_once` failed in the gate of 2fa39daf "with 3 of 600 lines unnumbered"; it stays below `LOCK_STALE` (5 s). "The lock stays unfair: under heavier contention a line can still be appended without `seq`, which the journal's best-effort contract allows."
- **Design runs on the runner** (ADR 0189): three points on the 32 x 32 cavity gave three tagged runs, "at most `max_runs` = 2 at once, with records equal to the in-process ones bit for bit. One evaluation took 0.64 s on the runner against 0.23 s in process."
- **Gates of the series** (each on a `git archive` copy): 2791 / 0 / 799 (F-270), 2810 / 0 / 800 (F-271), 2819 / 0 / 800 (F-272), 2846 / 2 / 800 (F-273; two `rcfd-runner` timing tests), 2854 / 0 / 800 (F-274), 2918 / 0 / 821 (F-275), 2934 / 3 / 823 (F-287 and F-288; flakes).

## Open questions

- ADR 0173 is not accepted as a whole (status line at the snapshot); its D1 to D8 were taken on their recommendations by the amendments.
- F-270: a comment line surviving a save from the Electron Studio is "Not yet shown by a test" (ADR 0185, 2026-11-18).
- F-273: which of the egui Studio's deferred pieces the Electron Studio has (actor on run starts and controls, a feed panel, a notes list) is for the lead to check and issue (ADR 0188, 2026-11-18).
- F-274 Q6: no journal line for design runs and stored evaluations; "no item was registered for the recommendation (a)" (ADR 0189).
- F-275: the hook under `bypassPermissions` and `--dangerously-skip-permissions` is NOT VERIFIED; the plugin installed from the marketplace and `NotebookEdit` / `MultiEdit` in a real session are known misses that "were not asked and stand as recorded" (ADR 0195).
- ADR 0186's question found while building: `start_run` keeps class `execute` but can create a case file through `new_case`; "the other half of option (b), `start_run` with `new_case` also checked as `write_files`, was not part of the answer" (ADR 0195 decision record item 9).
- F-862: correcting the description of `start_run`'s `store` argument (approved 2026-11-21, todo).
- F-288's test `adopt_f288` A-N failed once in a left-out run (a replace of a new `run.json` refused with access denied); diagnosed and fixed in the test by ADR 0773 (F-876, done 2026-11-23): [[adr-thread-daily-gate-and-edit-lock]]. Two follow-ups on `adopt_f288` are proposed and not entered.

## Related

- Predecessor (added by p3-records-a): the project folder library `rcfd-project` is ADR 0122 (wave 47A, layer L1); see [[adr-thread-studio-foundation-crates]].
- [[rcfd-workspace]], [[rcfd-mcp]], [[rcfd-project]], [[rcfd-runner]], [[project-journal]], [[field-store]], [[mcp]], [[mcp-access-class]], [[claude-plugin]], [[design-loop]], [[studio]]
- Threads: [[adr-thread-mcp-tools]] (the tool list over time), [[adr-thread-studio-engine-entry-points]], [[adr-thread-runner-records-and-run-sites]], [[adr-thread-studio-electron]]
- Areas: [[roadmap-claude-and-the-mcp-server]], [[roadmap-studio-desktop-app]]
