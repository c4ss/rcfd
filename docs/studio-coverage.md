# Studio capability inventory

This page lists what the rcfd engine can do, read from the engine itself (F-824, [ADR 0800](adr/0800-electron-studio-roadmap.md)):
every service method, MCP tool, CLI command and option, case-schema key and monitor format key. Guardrail G15, which held
each of them to a way in by hand, was retired on 2026-11-26 ([ADR 0803](adr/0803-the-studio-as-claudes-canvas-record.md));
its map went with ADR 0808 (F-981). The Studio is the canvas Claude drives (G23 of the [Studio guardrails](studio-guardrails.md)),
and every capability is reached through the engine's own interfaces (G16).

The page is generated; do not edit it. `npm run capabilities` in `apps/studio-web` asks the binaries (`rcfd serve --stdio`
for `service.info` and `settings.schema`, `rcfd-mcp` for its MCP `tools/list` with and without `--project`, and the
`--help` of `rcfd`, `rcfd-runner` and `rcfd-mcp`), writes `apps/studio-web/capabilities/inventory.json`, and writes this
page from it. The Studio's unit tests (`tests/unit/capabilities.spec.ts`, run by `npm test`) fail when this page is not
the generated one and when the inventory differs from the engine (that last check is skipped, with the reason, when no
`rcfd` binary is found).

Engine read: rcfd 0.1.0, service protocol rcfd-service/1 version 1.1, MCP server rcfd 0.1.0 (MCP protocol 2025-06-18).

## Summary

| Kind | Capabilities |
|---|---|
| Service methods | 69 |
| MCP tools | 75 |
| CLI commands | 54 |
| CLI options | 221 |
| Case schema keys | 951 |
| Monitor format keys | 6 |
| **All** | **1376** |

A case key stands for every value of its enumeration (the solver options of that key, listed in the inventory). A CLI
option is listed under its command.

## Service methods (69)

`cosim.advance`, `cosim.checkpoint`, `cosim.describe`, `cosim.forces`, `cosim.list`, `cosim.open`, `cosim.restore`, `cosim.time`, `design.compare`, `design.create_study`, `design.create_variant`, `design.evaluate`, `design.get_metrics`, `design.ledger_append`, `design.ledger_export`, `design.ledger_query`, `design.ledger_rerun_check`, `design.list_study`, `design.propose`, `design.record`, `design.run_map`, `design.run_study`, `events.poll`, `field.export_vtk`, `field.grid`, `field.sample`, `field.slice`, `field.stats`, `journal.get`, `journal.replay`, `log.tail`, `mesh.check`, `mesh.inflate`, `mesh.partition`, `monitors.get`, `panel.snapshot`, `particles.report`, `render.glb`, `render.png`, `report.export`, `reports.get`, `run.check`, `run.headless`, `run.initialize`, `run.pause`, `run.resume`, `run.start`, `run.status`, `run.step`, `run.stop`, `run.wait`, `service.info`, `session.close`, `session.create`, `session.list`, `settings.get`, `settings.remove`, `settings.schema`, `settings.set`, `settings.validate`, `status.read`, `workflow.create`, `workflow.delete`, `workflow.get`, `workflow.insert`, `workflow.list`, `workflow.load`, `workflow.run`, `workflow.save`.

## MCP tools (75)

`cancel_run`, `case_facts`, `check_case_headless`, `check_case_mesh`, `check_mesh`, `compare_runs`, `create_case`, `create_project`, `create_workflow`, `derive_picture`, `design_compare_variants`, `design_create_study`, `design_create_variant`, `design_evaluate`, `design_get_metrics`, `design_ledger_append`, `design_ledger_export`, `design_ledger_query`, `design_ledger_rerun_check`, `design_list_study`, `design_propose`, `design_record`, `design_run_operating_map`, `design_run_study`, `design_sobol`, `design_uq`, `edit_case`, `edit_workflow`, `empty_trash`, `export_view`, `export_vtk`, `field_summary`, `get_field_stats`, `get_residuals`, `get_setting`, `hash_project_file`, `import_mesh`, `list_cases`, `list_changes`, `list_meshes`, `list_runs`, `list_sites`, `list_trash`, `list_workflow_tasks`, `load_mesh`, `new_case`, `pause_run`, `preview_mesh`, `project_info`, `read_feed`, `read_metrics`, `read_project_file`, `read_residuals`, `read_status_stream`, `read_view`, `refine_case_mesh`, `render_field`, `render_image`, `restore_trash`, `resume_run`, `run`, `run_headless`, `run_status`, `run_workflow_task`, `runner_config`, `set_runner_config`, `set_setting`, `show_view`, `start_run`, `status`, `trash_case`, `trash_results`, `trash_run`, `wait_run`, `write_note`.

## CLI commands (54)

`rcfd acoustics`, `rcfd call`, `rcfd cases`, `rcfd check`, `rcfd check-mesh`, `rcfd convert`, `rcfd data list`, `rcfd data provenance`, `rcfd data verify`, `rcfd duct-modes`, `rcfd fwh`, `rcfd help`, `rcfd inflate`, `rcfd map`, `rcfd partition`, `rcfd post`, `rcfd post derive`, `rcfd project empty-trash`, `rcfd project hash`, `rcfd project init`, `rcfd project restore`, `rcfd project trash`, `rcfd project trash-list`, `rcfd reactor`, `rcfd restart`, `rcfd run`, `rcfd sample`, `rcfd schema`, `rcfd serve`, `rcfd site check`, `rcfd site list`, `rcfd study`, `rcfd validate`, `rcfd version`, `rcfd workflow run`, `rcfd workflow show`, `rcfd workflow templates`, `rcfd-mcp`, `rcfd-runner cancel`, `rcfd-runner config`, `rcfd-runner config set`, `rcfd-runner ensure`, `rcfd-runner list`, `rcfd-runner pause`, `rcfd-runner rerun`, `rcfd-runner resume`, `rcfd-runner serve`, `rcfd-runner shutdown`, `rcfd-runner site`, `rcfd-runner site check`, `rcfd-runner site list`, `rcfd-runner start`, `rcfd-runner status`, `rcfd-runner wait`.

## CLI options (221)

By command: `rcfd` (2), `rcfd acoustics` (3), `rcfd call` (1), `rcfd cases` (1), `rcfd check` (6), `rcfd check-mesh` (1), `rcfd convert` (3), `rcfd data list` (3), `rcfd data provenance` (3), `rcfd data verify` (3), `rcfd duct-modes` (6), `rcfd fwh` (9), `rcfd inflate` (12), `rcfd map` (10), `rcfd partition` (15), `rcfd post` (2), `rcfd post derive` (19), `rcfd project empty-trash` (3), `rcfd project init` (1), `rcfd project restore` (1), `rcfd project trash` (6), `rcfd reactor` (1), `rcfd restart` (4), `rcfd run` (30), `rcfd sample` (7), `rcfd schema` (2), `rcfd serve` (7), `rcfd site check` (1), `rcfd site list` (1), `rcfd study` (8), `rcfd version` (1), `rcfd workflow run` (6), `rcfd workflow show` (2), `rcfd-mcp` (11), `rcfd-runner` (1), `rcfd-runner config set` (3), `rcfd-runner ensure` (4), `rcfd-runner rerun` (5), `rcfd-runner site` (2), `rcfd-runner site check` (1), `rcfd-runner site list` (1), `rcfd-runner start` (12), `rcfd-runner wait` (1).

## Case schema keys (951)

By section: `boundaries` (72), `chain` (5), `chemistry` (21), `components` (33), `compressible` (17), `convergence` (5), `cosim` (7), `decomposition` (17), `dynamic_mesh` (48), `electric` (5), `energy` (5), `expressions` (1), `fluid` (23), `initial` (12), `limits` (6), `linear_solver` (3), `mesh` (105), `mixing_plane` (11), `multiphase` (60), `output` (11), `particles` (89), `phase_change` (6), `physics` (8), `postprocessing` (15), `properties` (42), `radiation` (4), `regions` (6), `registers` (15), `reports` (14), `restart` (3), `rheology` (19), `scalars` (14), `solids` (10), `solver` (73), `sources` (29), `species` (9), `statistics` (3), `study` (12), `tables` (6), `thermal_interfaces` (6), `title` (1), `turbulence` (80), `udf` (9), `zone_sources` (11).

## Monitor format keys (6)

`format`, `series.*.unit`, `series.*.x`, `series.*.y`, `version`, `x_label`.

## What the inventory leaves out

- The bulk data methods `data.*` (protocol 1.2, ADR 0350): they exist only in a service that an embedding program
  switches on, and `rcfd serve` does not, so `service.info` does not list them. F-804 is their Studio item.
- The co-simulation participants that `rcfd serve --cosim <case>` hosts: the `cosim.*` methods are listed, the
  participants depend on a case.
- `rcfd-studio` and `rcfd-gui`: removed on 2026-11-16 (G14, F-449); they were not engine capabilities.
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

## Where to go next

- [Studio guardrails](studio-guardrails.md): G16 and G23.
- [Service protocol](service-protocol.md), [MCP tools](mcp-tools.md) and [case schema](case-schema.md): what each capability does.
