# Studio capability coverage

This page lists what the rcfd engine can do and whether the Electron Studio can reach it, as guardrail G15 of the
[Studio guardrails](studio-guardrails.md) asks: every case-schema key, service method, MCP tool and CLI command has a UI
path or a recorded, dated exemption that names the roadmap item expected to cover it (F-824, [ADR 0800](adr/0800-electron-studio-roadmap.md)).
It is for the maintainer and for whoever builds a Studio surface: the gaps below are the work left.

The page is generated; do not edit it. `npm run capabilities` in `apps/studio-web` asks the binaries (`rcfd serve --stdio`
for `service.info` and `settings.schema`, `rcfd-mcp` for its MCP `tools/list` with and without `--project`, and the
`--help` of `rcfd`, `rcfd-runner` and `rcfd-mcp`), writes `apps/studio-web/capabilities/inventory.json`, and writes this
page from the inventory and the hand-kept map `apps/studio-web/capabilities/coverage.json`. The Studio's unit tests
(`tests/unit/capabilities.spec.ts`, run by `npm test`) fail when a capability has no entry in the map, when an entry is
malformed or maps nothing, when this page is not the generated one, and when the inventory differs from the engine
(that last check is skipped, with the reason, when no `rcfd` binary is found).

Engine read: rcfd 0.1.0, service protocol rcfd-service/1 version 1.1, MCP server rcfd 0.1.0 (MCP protocol 2025-06-18).

## Summary

| Kind | Capabilities | UI path | Exemption | Exemption whose item does not name it |
|---|---|---|---|---|
| Service methods | 69 | 19 | 50 | 39 |
| MCP tools | 56 | 20 | 36 | 27 |
| CLI commands | 36 | 9 | 27 | 21 |
| CLI options | 156 | 8 | 148 | 114 |
| Case schema keys | 948 | 948 | 0 | 0 |
| Monitor format keys | 6 | 6 | 0 | 0 |
| **All** | **1271** | **1010** (79.5 %) | **261** | **201** |

A case key stands for every value of its enumeration (the solver options of that key, listed in the inventory): the
schema-driven surface of F-825 offers them all. A CLI option is mapped with its command. "Item does not name it" marks an
exemption whose item is only the nearest home: its acceptance does not cover the capability, so it needs a clause or a
new item (see the last sections).

## Gaps by kind and target item

### Service methods (50 without a UI path)

| Item | Count | Capabilities |
|---|---|---|
| F-807 run control (quick run, stop, pause, queue) | 29 | `cosim.advance` (not named), `cosim.checkpoint` (not named), `cosim.describe` (not named), `cosim.forces` (not named), `cosim.list` (not named), `cosim.open` (not named), `cosim.restore` (not named), `cosim.time` (not named), `design.create_study` (not named), `design.create_variant` (not named), `design.evaluate` (not named), `design.ledger_append` (not named), `design.ledger_export` (not named), `design.propose` (not named), `design.record` (not named), `design.run_map` (not named), `design.run_study` (not named), `events.poll` (not named), `run.headless` (not named), `run.wait` (not named), `status.read` (not named), `workflow.create` (not named), `workflow.delete` (not named), `workflow.get` (not named), `workflow.insert` (not named), `workflow.list` (not named), `workflow.load` (not named), `workflow.run` (not named), `workflow.save` (not named) |
| F-808 field viewport on three.js WebGPU | 6 | `field.export_vtk` (not named), `field.grid`, `field.slice`, `field.stats`, `render.glb`, `render.png` |
| F-809 derived pictures (iso-surfaces, streamlines, cut planes, vectors) | 1 | `field.sample` |
| F-810 run bands and linked time scrubbing | 1 | `panel.snapshot` |
| F-811 residual and monitor plots | 3 | `particles.report` (not named), `report.export` (not named), `reports.get` |
| F-812 case editor validated by the engine's schema | 3 | `mesh.inflate` (not named), `mesh.partition` (not named), `run.check` |
| F-815 compare two runs | 5 | `design.compare` (not named), `design.get_metrics` (not named), `design.ledger_query` (not named), `design.ledger_rerun_check` (not named), `design.list_study` (not named) |
| F-828 provenance inspector and reproduce-from-CLI | 2 | `journal.get`, `journal.replay` |

### MCP tools (36 without a UI path)

| Item | Count | Capabilities |
|---|---|---|
| F-805 project home and the live project folder | 3 | `import_mesh` (not named), `list_changes`, `write_note` (not named) |
| F-807 run control (quick run, stop, pause, queue) | 18 | `create_workflow` (not named), `design_create_study` (not named), `design_create_variant` (not named), `design_evaluate` (not named), `design_ledger_append` (not named), `design_ledger_export` (not named), `design_propose` (not named), `design_record` (not named), `design_run_operating_map` (not named), `design_run_study` (not named), `edit_workflow` (not named), `list_workflow_tasks` (not named), `read_status_stream` (not named), `run` (not named), `run_headless` (not named), `run_workflow_task` (not named), `status` (not named), `wait_run` (not named) |
| F-808 field viewport on three.js WebGPU | 5 | `export_vtk` (not named), `field_summary`, `get_field_stats`, `render_field`, `render_image` |
| F-811 residual and monitor plots | 2 | `get_residuals`, `read_metrics` |
| F-812 case editor validated by the engine's schema | 1 | `preview_mesh` (not named) |
| F-813 a learner's first path | 1 | `list_cases` |
| F-815 compare two runs | 6 | `compare_runs`, `design_compare_variants` (not named), `design_get_metrics` (not named), `design_ledger_query` (not named), `design_ledger_rerun_check` (not named), `design_list_study` (not named) |

### CLI commands (27 without a UI path)

| Item | Count | Capabilities |
|---|---|---|
| F-805 project home and the live project folder | 2 | `rcfd convert` (not named), `rcfd serve` (not named) |
| F-807 run control (quick run, stop, pause, queue) | 11 | `rcfd map` (not named), `rcfd reactor` (not named), `rcfd restart` (not named), `rcfd run`, `rcfd study` (not named), `rcfd workflow run` (not named), `rcfd workflow show` (not named), `rcfd workflow templates` (not named), `rcfd-runner serve` (not named), `rcfd-runner shutdown` (not named), `rcfd-runner wait` (not named) |
| F-809 derived pictures (iso-surfaces, streamlines, cut planes, vectors) | 3 | `rcfd acoustics` (not named), `rcfd fwh` (not named), `rcfd sample` |
| F-811 residual and monitor plots | 1 | `rcfd post` (not named) |
| F-812 case editor validated by the engine's schema | 4 | `rcfd check`, `rcfd check-mesh`, `rcfd inflate` (not named), `rcfd partition` (not named) |
| F-813 a learner's first path | 1 | `rcfd cases` |
| F-825 schema-driven case and settings surface | 1 | `rcfd schema` |
| F-827 one action layer for every UI action | 1 | `rcfd call` (not named) |
| F-828 provenance inspector and reproduce-from-CLI | 3 | `rcfd data list` (not named), `rcfd data provenance` (not named), `rcfd data verify` (not named) |

### CLI options (148 without a UI path)

| Item | Count | Capabilities |
|---|---|---|
| F-805 project home and the live project folder | 7 | `rcfd convert` (3), `rcfd serve` (4) |
| F-807 run control (quick run, stop, pause, queue) | 69 | `rcfd map` (10), `rcfd reactor` (1), `rcfd restart` (4), `rcfd run` (28), `rcfd study` (8), `rcfd workflow run` (6), `rcfd workflow show` (2), `rcfd-runner ensure` (4), `rcfd-runner start` (5), `rcfd-runner wait` (1) |
| F-809 derived pictures (iso-surfaces, streamlines, cut planes, vectors) | 19 | `rcfd acoustics` (3), `rcfd fwh` (9), `rcfd sample` (7) |
| F-811 residual and monitor plots | 2 | `rcfd post` (2) |
| F-812 case editor validated by the engine's schema | 30 | `rcfd check` (2), `rcfd check-mesh` (1), `rcfd inflate` (12), `rcfd partition` (15) |
| F-813 a learner's first path | 1 | `rcfd cases` (1) |
| F-814 Claude Code panel | 6 | `rcfd-mcp` (6) |
| F-825 schema-driven case and settings surface | 1 | `rcfd schema` (1) |
| F-827 one action layer for every UI action | 1 | `rcfd call` (1) |
| F-828 provenance inspector and reproduce-from-CLI | 11 | `rcfd` (1), `rcfd data list` (3), `rcfd data provenance` (3), `rcfd data verify` (3), `rcfd run` (1) |
| F-830 explain in place from the engine's docs | 1 | `rcfd` (1) |

## Capabilities no Studio item names yet

These exemptions point at their nearest item, whose acceptance does not cover them. Each group needs the maintainer:
a clause added to that item, a new item, or a decision that the Studio leaves it to Claude Code and the CLI (G16 still
holds the other way round). The reasons are in `coverage.json`.

- Acoustics and 0D reactor tools (acoustics, fwh, reactor): no Studio item covers them; nearest homes derived pictures (F-809) and run control (F-807). (F-807, F-809): `rcfd acoustics`, `rcfd fwh`, `rcfd reactor`, `rcfd acoustics --*`, `rcfd fwh --*`, `rcfd reactor --*`.
- Calling any service method by name (rcfd call): no item names a generic call; nearest home the action layer (F-827). (F-827): `rcfd call`, `rcfd call --*`.
- Choosing or starting the solver service from the window: the service client (F-803) starts rcfd serve from the environment and no item gives it a UI; nearest home project home (F-805). (F-805): `rcfd serve`, `rcfd serve --*`.
- Co-simulation (the cosim methods): no Studio item covers it; nearest home run control (F-807). (F-807): `cosim.advance`, `cosim.checkpoint`, `cosim.describe`, `cosim.forces`, `cosim.list`, `cosim.open`, `cosim.restore`, `cosim.time`.
- Design studies, operating maps and the design ledger: no Studio item covers them; nearest homes run control (F-807) for running and compare (F-815) for results. (F-807, F-815): `design.compare`, `design.create_study`, `design.create_variant`, `design.evaluate`, `design.get_metrics`, `design.ledger_append`, `design.ledger_export`, `design.ledger_query`, `design.ledger_rerun_check`, `design.list_study`, `design.propose`, `design.record`, `design.run_map`, `design.run_study`, `design_compare_variants`, `design_create_study`, `design_create_variant`, `design_evaluate`, `design_get_metrics`, `design_ledger_append`, `design_ledger_export`, `design_ledger_query`, `design_ledger_rerun_check`, `design_list_study`, `design_propose`, `design_record`, `design_run_operating_map`, `design_run_study`, `rcfd map`, `rcfd study`, `rcfd map --*`, `rcfd study --*`.
- Events of the service's in-memory sessions: the runs table (F-806) follows the runner's runs through the project server; a run of a service session (quick run) would report through it; nearest home run control (F-807). (F-807): `events.poll`.
- Exports to files (VTK, report files): no Studio item names them; nearest home the surface that shows the data. (F-808, F-811): `field.export_vtk`, `report.export`, `export_vtk`.
- How the started runner logs and whether it stays attached (--foreground, --log-file, --quiet, --stdout): the Studio starts it detached with its defaults (F-807, Start the runner). (F-807): `rcfd-runner ensure --*`.
- Mesh tools (partition, inflation layers, conversion, import, mesh preview): no Studio item names them; nearest homes the case editor (F-812) and project home (F-805). (F-805, F-812): `mesh.inflate`, `mesh.partition`, `import_mesh`, `preview_mesh`, `rcfd convert`, `rcfd inflate`, `rcfd partition`, `rcfd convert --*`, `rcfd inflate --*`, `rcfd partition --*`.
- Options of rcfd-runner start that the Studio does not set (--foreground, --log-file, --no-runner, --quiet, --stdout): it starts runs through start_run, which takes priority, threads, mem_mb and set (F-807). (F-807): `rcfd-runner start --*`.
- Particle-tracking reports: no item names them; nearest home the plots (F-811). (F-811): `particles.report`.
- Re-creating reports from a results folder (rcfd post): no item names it; nearest home the plots (F-811). (F-811): `rcfd post`, `rcfd post --*`.
- Restarts from a checkpoint or another solution, headless runs under the run-control contract, status streams and the runner's shutdown: F-807 names quick run, stop, pause and queue only; nearest home F-807. (F-807): `run.headless`, `status.read`, `read_status_stream`, `run_headless`, `rcfd restart`, `rcfd-runner shutdown`, `rcfd restart --*`, `rcfd run --checkpoint`, `rcfd run --checkpoint-every`, `rcfd run --continue-from`, `rcfd run --interpolate-case`, `rcfd run --interpolate-checkpoint`, `rcfd run --interpolate-mode`, `rcfd run --method`, `rcfd run --restart-from`, `rcfd run --source-case`.
- Runs of an in-memory service session (the run methods of a session, the MCP run and status tools): F-807 runs through the runner only, and whether the Studio also runs service sessions is open; nearest home F-807. (F-807): `run.wait`, `run`, `status`.
- Task workflows (the workflow methods, tools and commands): no Studio item covers them; nearest home run control (F-807). (F-807): `workflow.create`, `workflow.delete`, `workflow.get`, `workflow.insert`, `workflow.list`, `workflow.load`, `workflow.run`, `workflow.save`, `create_workflow`, `edit_workflow`, `list_workflow_tasks`, `run_workflow_task`, `rcfd workflow run`, `rcfd workflow show`, `rcfd workflow templates`, `rcfd workflow run --*`, `rcfd workflow show --*`.
- The Claude Code panel (F-814) runs rcfd-mcp with the engine's defaults for case roots, the edit hook and the run and design limits; settings for them are not part of F-814. (F-814): `rcfd-mcp --*`.
- The public validation-data pack (rcfd data): no Studio item covers it; nearest home provenance (F-828). (F-828): `rcfd data list`, `rcfd data provenance`, `rcfd data verify`, `rcfd data list --*`, `rcfd data provenance --*`, `rcfd data verify --*`.
- The runner itself in the foreground: the Studio never runs serve directly; Start the runner runs rcfd-runner ensure, which starts serve detached (F-807). (F-807): `rcfd-runner serve`.
- Waiting for a run to end is a call for scripts: the Studio shows the end live in Project > Runs (F-806) and needs no waiting call; nearest home run control (F-807). (F-807): `wait_run`, `rcfd-runner wait`, `rcfd-runner wait --*`.
- Writing notes: F-805 lists notes but names no editor; nearest home F-805. (F-805): `write_note`.

## What the inventory leaves out

- The bulk data methods `data.*` (protocol 1.2, ADR 0350): they exist only in a service that an embedding program
  switches on, and `rcfd serve` does not, so `service.info` does not list them. F-804 is their Studio item.
- The co-simulation participants that `rcfd serve --cosim <case>` hosts: the `cosim.*` methods are listed, the
  participants depend on a case.
- `rcfd-studio` and `rcfd-gui`: frozen until parity and then removed (G14, F-449); they are not engine capabilities.
- The arguments of each MCP tool and service method (their input schemas): the tool or method is the capability.
- Exit codes, event kinds of `events.poll` and error codes: the engine prints or documents them but has no list to read.

## Engine-side gaps (proposals under G4)

G4 lets the engine grow only by addition. None of these is needed for the inventory to be complete today; each would
make it less fragile, and each needs its own ADR and the maintainer before any Rust changes:

1. A machine-readable command list. `rcfd <command> --help` prints the whole help, not the command's, and no command
   prints the list as data, so the generator parses the help text by its layout (a four-space indent opens a command).
   Proposal: a new `rcfd help --json` (and the same in `rcfd-runner` and `rcfd-mcp`) that prints every command with its
   usage and options; the help text stays byte for byte.
2. Opt-in method groups. `service.info` lists only the methods that are on. Proposal: an additive member (for example
   `optional_methods`) naming the groups an embedding program can switch on (`data.*`), so the inventory can list them.
3. The project-mode tool list. `rcfd-mcp` lists its project tools only when started on a project folder, so the
   generator writes a scratch `rcfd-project.json`. Proposal: `rcfd-mcp --list-tools [--project]` printing `tools/list`
   with no folder.

## Open for the maintainer

- **Granularity of the map** (applied, reversible): service methods, MCP tools and CLI commands are mapped one by
  one; case keys by top-level section and CLI options by command. A new method, tool, command or case section fails
  the test; a new key of a known section or a new option of a known command is covered by its parent's rule, as the
  schema-driven surface of F-825 will cover it. The alternatives were one rule per id (about 1 300 rules, every new
  key fails) and one rule per kind (nothing fails, against G15).
- **Capabilities no item names** (applied, reversible): they point at their nearest item and carry `unplanned`.
  Recommended: add items or acceptance clauses for the groups above, or decide which of them stay with Claude Code and
  the CLI only.
- **Service sessions or the runner**: F-807 runs through the runner only. Whether the Studio also offers the
  in-memory session run of the service (`run.*`, the MCP `run` and `status` tools) is open.
- **The engine check in CI**: the studio-web job builds no `rcfd`, so the check against the engine is skipped there.
  Recommended: build `rcfd-cli`, `rcfd-runner` and `rcfd-mcp` in that job (F-823) so a stale inventory fails CI.

## Where to go next

- [Studio guardrails](studio-guardrails.md): G15 and G16, the rules this page checks.
- [Roadmap](ROADMAP.md): the items named above (F-805 to F-832).
- [Service protocol](service-protocol.md), [MCP tools](mcp-tools.md) and [case schema](case-schema.md): what each capability does.
