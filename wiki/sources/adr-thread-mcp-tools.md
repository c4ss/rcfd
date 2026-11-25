---
type: source
tags: [adr-thread, mcp, tool-list, access-class, allow-list, claude-code]
sources: [doc-mcp-tools, doc-claude-plugin-skills]
origin: docs/adr/ (ADR list below)
origin_commit: d2cc5641
created: 2026-11-22
updated: 2026-11-25
status: draft
---

# ADR thread: the MCP tool list, its access classes and the Claude Code allow-list

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 09647baa (first written at 2e94e5ef; ADR 0774 added at 09647baa, the earlier ADRs unchanged since).
**Kind:** ADR thread, cross-cutting: the ADRs that changed what `rcfd-mcp` lists, how a tool is classed, or which tools Claude Code may call without asking. Most rows have their main record on another thread (column "Main thread"); this page follows the tool list through them. 12 ADRs of job p3-studio-mcp, 2026-11-05 to 2026-11-21. The origin of the project tools and the plugin (ADR 0135, wave 48F) and the solid-solver tools (ADR 0308) are in the list of job p3-records-b, which may add them here.

The rule every row keeps: an MCP tool list is a golden that is never re-recorded. A new tool is checked by restating the list tests ("the golden plus exactly this tool"), and without `--project` the list stays byte-identical (`default_f1.rs`, the ADR 0135 golden).

## Current state

- **Two modes** ([[doc-mcp-tools]]): session tools over in-memory service sessions, and `rcfd-mcp --project <dir>`, where the session tools that need the server's memory are hidden and uncallable (ADR 0195, "16 session-only tools"), `create_project` is not offered, and the project tools over [[rcfd-workspace]] are added.
- **Newest additions, by ADR**: `create_project` (ADR 0731, last tool of the default list, only without `--project`); `runner_config`, `set_runner_config` (ADR 0733); `hash_project_file` and `start_run {continue_from, max_steps, from_run}` (ADR 0734); the six trash tools (ADR 0736); `case_facts`, `refine_case_mesh`, `start_run {run_time, dry_run}` (ADR 0737); `derive_picture` (ADR 0738); `list_sites` and `start_run {site}` (ADR 0739); `start_run`'s `store` member in the reply (ADR 0741).
- **Access classes**: `read`, `mutate`, `execute`, `write_files`, and since ADR 0736 `delete`, held by `empty_trash` alone ([[mcp-access-class]]). `design_run_study` and `design_evaluate` became `execute` under `--project` on the maintainer's answer to ADR 0195 Q6 (2026-11-06). `start_run` stays `execute` although it can create a case through `new_case`.
- **The allow-list**: the Studio's first allow-list `PROJECT_READ_TOOLS` grew from 15 tools (the egui Studio) to "16 tools" (ADR 0733, `runner_config`) and "17 tools" (ADR 0734, `hash_project_file`); `list_trash`, `derive_picture` and `list_sites` were left off. ADR 0801 Addendum 23 (2026-11-17) removed the Studio's panel, its allow-list writing and the Rust twin; tests C5-A, F2 and A "are restated on the engine's own access classes and annotations (the 17 read-class tools, read-only hints, and the delete class being `empty_trash` alone)". In the Studio flow Claude Code's own permission prompts govern.
- **The plugin edit hook** (ADR 0195): refuses Edit and Write under `cases/`, `runs/` and `designs/`; fails open; not verified under the bypass modes.
- ~~**Open**: F-862 (todo), the description of `start_run`'s `store` argument, approved for correction on 2026-11-21 (ADR 0741).~~ **F-862 done** (ADR 0774, merged 77e54786 on 2026-11-23): the description of `start_run`'s `store` argument "now ends 'Default plan; none is the way back to a run that stores only what its case asks for'" (ADR 0741 question 1, option (a)), "held by a test that reads the live tools list; the whole list before and after is byte-equal with the old text put back, no golden, argument or default changed and no Studio call changed". The only source change is that one string in `crates/rcfd-mcp/src/project.rs`; at the merge the lead removed the clause of `docs/mcp-tools.md` that said the description still named the earlier default ([[doc-mcp-tools]]).
- ~~**Issued, not on main at 09647baa:** F-339 (`design_uq` and `design_sobol` tools on a named evaluator, with a registered edit of the tools-list goldens), issued on 2026-11-23 by the brief `docs/adr/briefs/f339-mcp-uq-sobol-tools.md` (ADR 0784); the item is todo ([[roadmap-design-studies-and-optimisation]]).~~ **F-339 partial** (ADR 0784, merged at 05175113, pushed at 9c0aa461): the MCP tools `design_uq` and `design_sobol` run `uq::run` and `sensitivity::run` on the named evaluator `rcfd.fan_rom` (the reduced-order fan, inputs `stagger_common_deg`, `chord_rel`, `tip_gap_delta`) with a design ledger, inside the sandbox and under `max_design_evaluations`, class `write_files`; the reply's `result` is the library's bit for bit (256 Monte Carlo samples, a Latin hypercube study with a stopping rule, 320 Sobol evaluations). They are not service methods. The lead made the additive golden edit (both tools-list goldens gain the two entries, 33 to 35 and 50 to 52; sixteen count literals raised by two) after a reader confirmed it additive. Partial: X1 (a fourth count of the list) missed by the registered rule, and the reader's findings 1-1 (no test sends the string or null bounds) and 2-1 (reference inputs that cannot tell a swapped or dropped parameter) ([[doc-mcp-tools]], [[doc-roadmap-json]]).
- ~~F-339 stays partial~~ (ADR 0791, merged at 26185c59, pushed at 81cfd554; the agent proposed done): the reader's findings 1-1 and 2-1 are closed. Every argument of `design_uq` and `design_sobol` is sent at a value that tells a wrong reading from the right one, 16 tests against the library, 40 mutations of `tools.rs` caught (M10, M11, M14 and M15 through the panic of K1, not the equality); `tools.rs` was right in every row. What is left is **K1**: a normal distribution open on one side only panics in the library and ends the server; "held as the ignored `k1_one_open_side_known_miss`". X1 of ADR 0784 stays recorded as missed ([[doc-roadmap-json]]).
- **F-894 done, and F-339 done with it** (ADR 0792, merged at c349be6e, pushed at 4404e24b; entered on the maintainer's answer "Yes, issue it" of 2026-11-25): `erfc` is exactly 0 at plus infinity, and a normal distribution open on one side is sampled from the smaller of its two tails; 884 points agree with a 760-digit reference within 0.024 of the registered limit of 1e-12 sd, K1 passes unedited, and the server on stdio answers such a call and the ping after it. Every other result is bit-identical to main, held by records for Windows and Linux x86_64 (24 of 35 groups differ between the two on main's own library, since the platform's `exp` differs). M10, M11, M14 and M15 of ADR 0791 are caught by the equality. X1 of ADR 0784 stays recorded as missed and is not what decides the status ([[doc-roadmap-json]]).

## ADRs

| ADR | Date | Kind | Item | Change to the tool list | Count after (as the ADR states it) | Main thread |
|---|---|---|---|---|---|---|
| [0186](../../docs/adr/0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md) | 2026-11-05 | registration | F-271 | `new_case`; `start_run` gains `new_case` and `store` | 51 tools, 42 103 bytes | [[adr-thread-workspace-layer]] |
| [0187](../../docs/adr/0187-adr-0173-amendment-3-f272-registration-mesh-set-up-from-the-mcp-server.md) | 2026-11-05 | registration | F-272 | `import_mesh`, `check_case_mesh`, `preview_mesh`, `list_meshes` | 55 tools, 45 592 bytes | [[adr-thread-workspace-layer]] |
| [0188](../../docs/adr/0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) | 2026-11-05 | registration | F-273 | `read_feed`; `reason` on eight tools | 56 tools, 48 294 bytes | [[adr-thread-workspace-layer]] |
| [0195](../../docs/adr/0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md) | 2026-11-06 | registration | F-275 | 16 session tools hidden with `--project`; four read tools join `PROJECT_READ_TOOLS`; edit hook | 40 tools with `--project` | [[adr-thread-workspace-layer]] |
| [0731](../../docs/adr/0731-f835-proposal-an-engine-command-that-creates-a-project.md) | 2026-11-17 | proposal, built | F-835 | `create_project` (`write_files`), default list only | default list + 1 | [[adr-thread-studio-engine-entry-points]] |
| [0733](../../docs/adr/0733-f837-proposal-set-runner-config-the-runner-writes-its-own-budgets.md) | 2026-11-17 | proposal, built | F-837 | `runner_config` (read), `set_runner_config` (`write_files`) | 42 tools, 25 project tools; allow-list 16 | [[adr-thread-runner-records-and-run-sites]] |
| [0734](../../docs/adr/0734-f828-proposal-run-records-launch-argv-build-id-time-unit-rerun-and-mesh-hash.md) | 2026-11-17 | proposal, built | F-828 | `hash_project_file` (read); three `start_run` properties | 43 tools, 26 project tools; allow-list 17 | [[adr-thread-runner-records-and-run-sites]] |
| [0736](../../docs/adr/0736-f829-proposal-a-project-trash-with-restore-and-compare-and-set-for-settings-set.md) | 2026-11-17 | proposal, built | F-829 | six trash tools; class `delete`; delete tools and whole-server rules refused by every allow-list | not stated in the read sections | [[adr-thread-studio-engine-entry-points]] |
| [0737](../../docs/adr/0737-f837-proposal-case-facts-and-a-dry-run-start-a-mesh-refine-and-saving-a-case-outside-a-project.md) | 2026-11-17 | proposal, built | F-837 | `case_facts` (read), `refine_case_mesh` (`write_files`); `start_run {run_time, dry_run}` | not stated in the read sections | [[adr-thread-studio-engine-entry-points]] |
| [0738](../../docs/adr/0738-f809-proposal-derived-pictures-of-a-stored-field-iso-surfaces-streamlines-slices-and-the-length-unit.md) | 2026-11-17 | proposal, built | F-809 | `derive_picture` (read, after `render_field`) | "27 project tools, 44 with `--project`" | [[adr-thread-studio-engine-entry-points]] |
| [0739](../../docs/adr/0739-f848-proposal-run-sites-a-remote-simulation-service-for-the-studio-cli-and-mcp.md) | 2026-11-17 | proposal, phase 1 built | F-848 | `list_sites` (read, after `refine_case_mesh`); `start_run {site}` gated on `assistant = true` | restated, not stated | [[adr-thread-runner-records-and-run-sites]] |
| [0741](../../docs/adr/0741-f271-registration-the-field-store-plan-as-the-default-of-start-run.md) | 2026-11-19 | registration | F-271 | `start_run` reply gains `store` (one additive golden edit) | "18 of 19 result shapes equal the golden" until re-recorded | [[adr-thread-workspace-layer]] |
| [0774](../../docs/adr/0774-f862-registration-the-description-of-start-run-store-argument.md) | 2026-11-22 | registration (rule 3 lifted for one string by the maintainer's answer in ADR 0741) | F-862 | The text of `start_run`'s `store` description (no tool, argument, default or golden changes) | rows 1 to 7 met, M1 caught; done | this page |
| [0784](../../docs/adr/0784-f339-registration-mcp-tools-for-tolerance-and-sobol-studies-of-the-reduced-order-fan.md) | 2026-11-23 | registration, amendment 1, addendum 1, a reader's report, the lead's reading | F-339 | MCP tools for tolerance and Sobol studies of a named evaluator | partial: the tools equal the library bit for bit; X1 missed; reader findings 1-1 and 2-1 | |
| [0791](../../docs/adr/0791-f339-registration-arguments-of-design-uq-and-design-sobol.md) | 2026-11-25 | registration, addendum 1, the lead's reading | F-339 | Can each argument of the two tools be told from a wrong reading of it, the string and null bounds among them? | partial: every row but the record K1 holds, 40 mutations caught; K1 (one open side) panics | |
| [0792](../../docs/adr/0792-f894-registration-a-normal-distribution-open-on-one-side.md) | 2026-11-25 | registration, addendum 1, amendment 1, addendum 2, the lead's reading | F-894 (and F-339) | Does a normal open on one side sample within its bound and agree with the closed form, with nothing else moved? | done: every row holds; K1 passes; F-339 done | |

## Key numbers and limits

- **Results in project mode** stay "under 36 000 bytes" ([[doc-mcp-tools]]).
- **Error classes**: the MCP server's codes start at 1100 (1100 permission denied under `--read-only`), the project tools' at 1200 (ADR 0737 change of 2026-11-17).
- **Hook contract** (ADR 0195): checked in "a real Claude Code 2.1.319 session"; Q5 and Q7 accepted 2026-11-06: "the hook's gaps are accepted, since the hook is a guide rail".
- **The Studio's registration** (ADR 0801 Addendum 23): `claude mcp add rcfd-project --scope local -- "<engine rcfd-mcp>" --project "<folder>"`, "full access, no `--read-only`: Claude Code's own permission prompts govern"; the Studio's own servers run `rcfd-mcp --client studio` and any other client is named "Claude Code" on the activity line.

> [!warning] Contradiction (within the repo docs)
> `docs/mcp-tools.md` says `rcfd-mcp --project <dir>` "adds 25 tools" ([[doc-mcp-tools]]), the count of ADR 0733's build ("42 tools, 25 project tools"). The same page's project-tool table names 36 tools in 34 rows (counted on this page, 2026-11-22: `pause_run`, `resume_run`, `cancel_run` share a row), and ADR 0738 already stated "27 project tools, 44 with `--project`" before the six trash tools and `list_sites` were counted. The wiki's [[mcp]] and [[rcfd-mcp]] pages repeat the 25 / 42.

> [!warning] Contradiction (doc and record)
> `docs/mcp-tools.md` gives `lo` and `hi` of a normal distribution as each optional ("no truncation on that side") ([[doc-mcp-tools]]); ADR 0791 (K1) records that one open side with one finite bound panics in the library (`erfc_inv needs 0 < y < 2`, `crates/rcfd-reactor/src/mixfrac.rs:161`, since `erfc` of infinity is not a number) and ends the `rcfd-mcp` process with exit code 101 and no reply. ~~Only both sides open or both finite work~~ ([[adr-thread-mcp-tools]]). Resolved on 2026-11-25 by F-894 (ADR 0792, merged at c349be6e): one open side is sampled and the server answers.

## Open questions

- ADR 0791 (the maintainer's, scope): ~~the follow-up one-open-side (the fix of K1)~~ (entered and done as F-894) and finding 7 (`dist` ignores a key of another distribution), not entered.
- ~~ADR 0784, open question 1 (the maintainer's): whether `design_uq` and `design_sobol` also become service methods (`design.uq`, `design.sobol`); recommended (a), MCP tools only until a second caller exists.~~ Closed on 2026-11-25 by the maintainer's answer "Yes, MCP only", option (a) (ADR 0784, "The maintainer's answer on open question 1").

- ~~F-862: the corrected text of `start_run`'s `store` description, "a change of the text of an existing tool, so it is made by a registration of its own" (ADR 0741, 2026-11-21).~~ Closed by ADR 0774 on 2026-11-23; its open questions for the maintainer: "None."
- Whether `start_run {new_case}` should also need `write_files` was never answered (ADR 0195 decision record item 9).
- `derive_picture` "is class `read` but is not added to the first allow-list ... that list is the maintainer's and was not asked about" (ADR 0738); `list_sites` "is not on the first Claude allow-list, because no answer adds it" (ADR 0739). Both lapsed as allow-list questions when the Studio's allow-list went (Addendum 23).

## Related

- [[mcp]], [[rcfd-mcp]], [[mcp-access-class]], [[claude-plugin]], [[rcfd-workspace]], [[project-journal]], [[studio]]
- Areas: [[roadmap-claude-and-the-mcp-server]], [[roadmap-studio-desktop-app]]
