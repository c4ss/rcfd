---
type: source
tags: [adr-thread, studio, service, ui, engine, mesh-preview, provenance, trash, derived-pictures]
sources: [doc-service-protocol, doc-mcp-tools, doc-studio-coverage]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the engine entry points the Electron Studio needed (F-809, F-826, F-828, F-829, F-830, F-835, F-836, F-837, F-475)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread: seven engine proposals, all proposed, accepted and built on 2026-11-17, plus one earlier registration for the egui Studio's preview (ADR 0377); 8 ADRs, 2026-11-12 to 2026-11-17.

Guardrails G2 ("Rust is the engine") and G16 ("No UI-only capability") mean that when a Studio item needs a number, a check or a write, the engine must give it, additively (G4). ADR 0801's section "Engine follow-ups of the Studio items" (2026-11-17) collected every such gap in one table (E-809a to E-837d), "so that one engine ADR can take them together"; the ADRs of this thread are those engine ADRs. Each one was proposed with options and a recommendation, the maintainer took the recommended option of every open question the same day (one question dropped: ADR 0735 Q4), and each was built the same day on its own branch. The common rule: nothing changes without an opt-in switch or a new name, defaults and `baseline_hashes.rs` stay as they are, and every new capability has a CLI or MCP twin so Claude Code can do what the Studio does.

## Current state

- **An opt-in UI family on the service.** `rcfd serve --ui` switches on `rcfd-ui/1` (ADR 0730): `settings.schema {ui: true}` with labels, advanced flags, help lines, docs links and recommended ranges; `help.glossary` and `help.errors`; then `mesh.preview` (ADR 0732) and `case.facts`, `mesh.refine`, `case.save` (ADR 0737). `protocol_version` stays 1.1; without `--ui` "`settings.schema`, `rcfd schema`, `service.info` and the method list are byte-identical to the code before this ADR". F-826 done; F-830 partial ("A learner's view-and-report step (G19) is not done").
- **A project from the engine** (ADR 0731, F-835 done): `rcfd project init <dir> [--name <name>]` and MCP `create_project {path, name?}` over `Project::create`; the Studio's New project… and Make a project here run it.
- **A mesh preview before a run** (ADR 0732, F-836 done): `mesh.preview {session, dir, adapt?, force?}` writes a mesh-only field store under `<dir>/<mesh hash>/`, reused when the hash matches; the case editor draws it with one colour per patch.
- **Provenance records** (ADRs 0734, 0735, F-828 partial): `runs/<id>/launch.json` with the exact argv, the solver's `fnv64:` build hash and git revision; `time_unit: "s"` in replies; `rcfd-runner rerun <id>` and `start_run {from_run}`; `hash_project_file`; session journals on disk under `rcfd serve --journal-dir` so a session number has a reproduce command `rcfd run --journal <file>`. F-828's remaining gap is Studio work: "the Runs residual columns, the compared run's residuals, legend ticks, domain size, scale bar, runner budgets and home counts". Claude Code session discovery from the engine (E-814) was declined: it "stays in the Studio's main process".
- **Safe deletes and conflicts** (ADR 0736, F-829 done): a project trash `.rcfd/trash/<token>/` with six tools (`trash_run`, `trash_results`, `trash_case`, `list_trash`, `restore_trash`, `empty_trash`), a new access class `delete` for `empty_trash` alone, and compare-and-set on `settings.set` with `base_value` or `base_hash` (error 1007 `CONFLICT`).
- **Run set-up facts** (ADR 0737, F-837 done with ADR 0733): `case.facts` / `case_facts` / `rcfd check --facts`; `start_run {run_time, dry_run}`; Refine x2 (`mesh.refine`, `refine_case_mesh`, factor 2 only; a mesh-file case refused with the new code 1009 `MESH_FILE`); `case.save` of a session outside a project.
- **Derived pictures from the engine** (ADR 0738, F-809 done): iso-surfaces, streamlines, cut surfaces and vector glyphs of a stored field through the project tool `derive_picture` and `rcfd post derive` (GLB files); the Studio draws the engine's output, and the engine is the reference. `data.open` gains `length_unit: "m"`. "Not built: P3's first bullet (`render_field` with `axis`, `slice`, `vectors`; R1) and `rcfd post render`."
- **The egui preview of an adaptive case** (ADR 0377, F-475 done): the preview showed the mesh after the run's first step (2799 cells), not `Simulation::new`'s base lattice (576 cells). ADR 0732 carries the same rule into the engine: `adapt: "base"` by default with `adapt.pending: true`, `"final"` on request.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0377](../../docs/adr/0377-f475-registration-studio-preview-of-an-adaptive-case.md) | 2026-11-12 | registration | F-475 | The egui preview of a `[mesh.adapt]` case shows the adapted mesh | every check met; partial by strict Rule I (M4 caught after amendment 2), item now done | the egui Studio was deleted 2026-11-16; rule carried on by 0732 |
| [0730](../../docs/adr/0730-f826-f830-proposal-ui-metadata-in-the-case-schema-and-help-from-the-engine.md) | 2026-11-17 | proposal, accepted, built | F-826, F-830 | UI metadata in the schema; glossary and error catalogue from the engine | built; F-826 done, F-830 partial | |
| [0731](../../docs/adr/0731-f835-proposal-an-engine-command-that-creates-a-project.md) | 2026-11-17 | proposal, accepted, built | F-835 | `rcfd project init` and MCP `create_project` | built; done | |
| [0732](../../docs/adr/0732-f836-proposal-a-mesh-preview-the-studio-can-draw-before-a-run.md) | 2026-11-17 | proposal, accepted, built | F-836 | `mesh.preview` for the case editor | built; done; M1 bit for bit on the 11 shipped cases | |
| [0735](../../docs/adr/0735-f828-f814-proposal-session-journals-on-disk-and-claude-code-session-discovery.md) | 2026-11-17 | proposal, partly declined, built | F-828, F-814 | Session journals on disk; Claude Code session discovery in the engine | P1, P2 built; P3 declined (Q4 dropped) | |
| [0736](../../docs/adr/0736-f829-proposal-a-project-trash-with-restore-and-compare-and-set-for-settings-set.md) | 2026-11-17 | proposal, accepted, built | F-829 | A project trash with restore; compare-and-set for `settings.set` | built; done | |
| [0737](../../docs/adr/0737-f837-proposal-case-facts-and-a-dry-run-start-a-mesh-refine-and-saving-a-case-outside-a-project.md) | 2026-11-17 | proposal, accepted, built; change of 2026-11-17 | F-837 | Case facts and a dry-run start, Refine x2, saving a case outside a project | built; 1009 `MESH_FILE` added after the first Studio use | |
| [0738](../../docs/adr/0738-f809-proposal-derived-pictures-of-a-stored-field-iso-surfaces-streamlines-slices-and-the-length-unit.md) | 2026-11-17 | proposal, accepted, built (engine and Studio halves) | F-809 | Iso-surfaces, streamlines, slices and the length unit from the engine | built except `render_field` slices and `rcfd post render`; done | |

ADR 0734 (run records) belongs to this set as well; it is on [[adr-thread-runner-records-and-run-sites]] because its records are the runner's.

## Key numbers and limits

- **Schema metadata counts** (ADR 0730 Result): "327 entries in the data file (746 of 1073 schema paths have none); 169 advanced (approved); **1 recommended range with a source** (`turbulence.nutilde_ratio` 3 to 5, Spalart and Rumsey, AIAA Journal 45(10), 2007". Help coverage over 1073 nodes (951 leaves): written 148, first sentence of the docs 19, of the schema text 762, none 144. Change of 2026-11-17: `output.store_time_interval` taken off the advanced list (168 advanced keys now), after "a Studio-started transient run saved no fields".
- **Help rules** (ADR 0730 answers): a help line is at most 160 characters; a recommended range only "with a cited source, public literature (author, title, year) or one of this repository's validation ADRs. No source, no range; no citation is invented; the project is clean-room." No help MCP tools; `rcfd help glossary|errors` covers Claude Code.
- **`create_project` errors** (ADR 0731): 1200 when the folder already holds `rcfd-project.json`; 1004 a file or outside `--root`; -32602 an empty name, over 200 characters or with a control character; `--read-only` 1100. A non-empty folder is allowed and nothing in it is replaced (Q3 (a)).
- **`mesh.preview`** (ADR 0732): the estimate's limit is "2 000 000 cells, the egui preview's"; `force: true` skips the estimate only, `max_cells` holds after every build; for the adaptive cavity base 576 cells, final 2799 cells, hash `05bb8beae5da57da`. The case editor counts 51 chrome words with the preview drawn, its budget of 51.
- **The adaptive preview** (ADR 0377): "2,799 cells, mesh hash 05bb8beae5da57da; adapted from the base lattice of 576 cells by 3 solves (576, 1,269, 2,799 cells; 97 + 122 + 188 iterations) in 2.59 s"; a newer request stopped the loop with the thread ended at 0.288 of a full build (limit 0.6).
- **Trash** (ADR 0736): token `t-<yyyymmddThhmmss>-<4 hex>`; `trash_run` refused (1002) unless the run is terminal and (1003) while an unfinished run continues from it; `restore_trash` refused (1200) when the original path is occupied; "there is no automatic emptying" (Q7 (a)). `destructiveHint` is true only for the class `delete`; no first list, "Allow this from now on" or allow-list file can hold a delete tool or a whole-server rule.
- **Session journals** (ADR 0735): one `<session>-<created yyyymmddThhmmss>.journal.json` per session, replaced on each accepted edit, only under `rcfd serve --journal-dir`; `rcfd run --journal` refuses a changed source with exit 3 unless `--journal-ignore-hash`; with journals on, `service.info` lists `journal.save` after the 69 methods.
- **Derived pictures** (ADR 0738 answers): refuse "over 2 000 000 triangles, or over 1 000 seeds or 10 000 steps, with 1004"; streamlines by "classical RK4, step 0.25 of the local cell length"; the Studio's arrows within 2^-20 of the largest extent (T3); the streamline radius error converged "at order 3.2 (superconvergence of the symmetric means), not 2" on a uniform disc, so S2 asserts second order on a graded mesh (order 2.0, in [1.8, 2.2]).
- **Error codes added**: 1007 `CONFLICT` (ADR 0736), 1009 `MESH_FILE` (ADR 0737 change; "the next free code of the service's 1000 range (1001 to 1008 are taken; the project tools' codes start at 1200 and the MCP server's at 1100)").

## Open questions

- F-830: the learner's view-and-report step (G19); the catalogue check on the Studio side is a unit test over `rcfd help errors`, "not an e2e walk that provokes each error; and no screenshot view-and-report was made for the new focus detail. Both are left for the maintainer to ask for" (ADR 0730).
- ADR 0377 Q2 and Q4 (the egui preview's G1 check and cancelling within a solve) lapsed with the egui Studio; no later ADR answers them.
- ADR 0738: `render_field` with `axis`, `slice`, `vectors` (R1) and `rcfd post render` are not built.
- ADR 0734's building decisions (a missing mesh file recorded without `mesh_hash`; a rerun not a member of the study) are listed "for the maintainer" ([[adr-thread-runner-records-and-run-sites]]).

> [!warning] Contradiction (within the repo docs)
> ADR 0801's table "Engine follow-ups of the Studio items" still marks rows E-809a to E-809d, E-828a, E-828b, E-828d, E-828e, E-829a and E-829b as "proposed; awaiting the maintainer", and the paragraph under it says "status Proposed, nothing built". ADRs 0734, 0736 and 0738 at the same snapshot say "ACCEPTED 2026-11-17" and built. The table's rows E-809e, E-813, E-828c, E-837a to E-837d were updated; the others were not.

## Related

- [[rcfd-service-protocol]] (the `rcfd-ui/1` family), [[rcfd-api]], [[rcfd-workspace]], [[rcfd-mcp]], [[rcfd-cli]], [[rcfd-project]], [[studio]], [[studio-guardrails]], [[project-journal]], [[mcp-access-class]], [[provenance]], [[mesh-adaptation]], [[field-store]]
- Threads: [[adr-thread-studio-electron]], [[adr-thread-runner-records-and-run-sites]], [[adr-thread-mcp-tools]], [[adr-thread-workspace-layer]]
- Areas: [[roadmap-studio-desktop-app]], [[roadmap-claude-and-the-mcp-server]]
