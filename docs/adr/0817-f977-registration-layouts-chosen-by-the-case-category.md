# ADR 0817: F-977 registration: the layout of results chosen by the category of the case

Status: **REGISTERED 2026-11-26, written before any code of F-977 and before any run of its checks.** Before this text no
library, Studio or test code of F-977 was written and no file of the repository was changed. What was done: reading
(below), searches of the source tree, and, on the unchanged tree, a release build of `rcfd`, `rcfd-mcp`, `rcfd-runner`
and the example `f447_bench` into the scratch target, `npm ci` in `apps/studio-web`, and one probe of the set-up of E0:
`rcfd run` of the still pool's text of the Checks section into the scratch folder (50 steps, 0.14 s, a field store of 5
snapshots). None of these is a check. Results go
in addenda at the end; nothing above them is changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a
background agent of the lead.

It answers the brief `docs/adr/briefs/f977-category-layouts.md` ([the file](briefs/f977-category-layouts.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f977-category-layouts` from d8b506d8, scratch
`w49i-work/f977-category-layouts-scratch`. The decision it comes from is
[ADR 0803](0803-the-studio-as-claudes-canvas-record.md) (ability 2: results shown in different ways by the category of
the case); it builds on [ADR 0805](0805-f976-registration-a-view-specification-and-show-view.md) (F-976: the view
specification `rcfd-view` 1, `show_view`, `read_view`, the Canvas tab) and
[ADR 0807](0807-f979-registration-live-meshes-settings-and-monitors-on-the-canvas.md) (F-979: the mesh panel of a case,
the setting kind, the monitor panel of a case).

## What is asked

Roadmap item F-977 (deps F-976), the binding acceptance, read with a script:

> Every limit not stated here is registered in the ADR before the run. The case categories are fixed in the registration from the case schema (at least external aerodynamics, internal flow and turbomachinery, heat transfer, structural, multiphase), each with a default layout of views derived from the case and run records. show_view with a run and no layout takes its category's layout; a case of no known category takes the general layout. Each category is shown by an e2e test on an example case of the repository, with the views chosen named in the registration.

Clauses: **a1** the categories are fixed from the case schema, at least the five named (read as five: "internal flow and
turbomachinery" is one); **a2** each has a default layout of views derived from the case and run records; **a3**
`show_view` with a run and no layout takes its category's layout; **a4** a case of no known category takes the general
layout; **a5** each category is shown by an e2e test on an example case of the repository, with the views chosen named
here. From the brief and G4: the view specification changes only by addition, and every other default is unchanged
(**a6**).

## What was read

`AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, whole; [ADR 0803](0803-the-studio-as-claudes-canvas-record.md)
whole; [ADR 0807](0807-f979-registration-live-meshes-settings-and-monitors-on-the-canvas.md) whole (its registration,
amendment 1, addendum 1 and answers); of [studio-guardrails.md](../studio-guardrails.md) G3, G4, G13, G16 and G23;
`crates/rcfd-workspace/src/view.rs` whole; `apps/studio-web/src/renderer/canvas/model.ts` and `Canvas.tsx` whole;
`apps/studio-web/tests/e2e/canvas-f976.spec.ts` whole, of `canvas-f979-a1.spec.ts` its set-up and `afterAll`; of
`tests/support/project.ts` `findMcp`, `copyExample`, `mcpPids` and `copyRun`, of `tests/support/f447.ts` how the bench is
found, of `src/main/engine/locate.ts` `checkoutRelease`; of `crates/rcfd-mcp/src/project.rs` the entries of `show_view`
and `read_view`; the head of `crates/rcfd-mcp/tests/canvas_view_f976.rs`; of `crates/rcfd-api/src/schema.rs` the top
level of the case schema (lines 1975 to 2110), the mesh generators (line 78), the boundary velocity and pressure
conditions (lines 509 and 555), `energy.enabled` (line 399), the report types (line 1290), `sources.<name>.type` (line
2100), `output.store_fields` (line 1468); `crates/rcfd-api/src/vof.rs` lines 36 to 100 (`multiphase`), of
`particles.rs` line 248, of `phase_change.rs` line 40, of `mixing_run.rs` line 50; the head of
`crates/rcfd-api/src/solid_case.rs` and `SolidCase::parse_toml_in` (line 492), the head of
[docs/solid-case-file.md](../solid-case-file.md); of `crates/rcfd-workspace/src/facts.rs` `read_case_text`, of `plan.rs`
`parse_case`; of `crates/rcfd-api/tests/vof_tree.rs` `pool_text`; `example-projects/README.md` and `manifest.json`; every
case file of `example-projects/` (14) for its mesh, physics, energy, sources, boundaries and reports; of every committed
example run its `run.json`, the first line of `residuals.jsonl` and the `store_fields` of its `case.snapshot`; of
`scripts/process_report.py` `sec_c` and `sec_r`. Bytes read: about 200 000.

## Findings of the reading (before any code)

1. **The schema has no field that names a category.** The flow case schema (`case_schema`, `schema.rs` from line 1975)
   switches physics on by blocks: `multiphase.enabled` (VOF, with `multiphase.mixture` and `multiphase.cavitation` inside
   it; `vof.rs` line 58), `particles.enabled` (one-way Lagrangian particles; `particles.rs` line 248), `energy.enabled`
   (`schema.rs` line 399), `phase_change.enabled` (`phase_change.rs` line 40), `sources.<name>.type` with
   `rotating_frame` and `mrf` (`schema.rs` line 2100), `mixing_plane.enabled` (`mixing_run.rs` line 50), the generator
   `axial_fan_sector` (`schema.rs` line 78), and per patch `boundaries.<patch>.velocity` / `.pressure` with the value
   `freestream` (lines 509 and 555), the far-field condition. A structural case is not a flow case at all: it is a solid
   case file with a schema of its own (`solid_case.rs` line 51), which the flow reader refuses and which reads by
   `SolidCase::parse_toml_in` (line 492; [docs/solid-case-file.md](../solid-case-file.md)). The category is therefore a
   rule over these blocks (D1).
2. **What a run records.** Every committed example run holds `run.json` (its id and the project-relative `case`),
   `case.snapshot` (the text of the case as the run read it; the runner runs that file, `rcfd-runner/src/runner.rs`
   line 696), `residuals.jsonl` (per line `residuals` and `monitors.<report>`), and `fields/` (the field store, with
   the names `output.store_fields` gave). The case of a run is the snapshot: the file under `cases/` may have changed or
   gone since.
3. **The example cases of the repository and what the rule gives them** (by D1, worked by hand on the files):
   `axial-fan/cases/fan01-passage-upwind.toml` has `sources.frame.type = "rotating_frame"`; the two cavities of
   `quick-start` have patches `lid` and `walls`, none free stream; the two heated cavities have `energy.enabled = true`;
   the two cylinders and the four NACA 0012 cases of `wind-tunnel` have `boundaries.farfield.velocity = "freestream"`;
   the two steps have four patches, none free stream. No committed case of `example-projects/` or `cases/` is a
   multiphase case; the repository's VOF cases are case texts in tests: the still pool of ADR 0049 is
   `pool_text` of `crates/rcfd-api/tests/vof_tree.rs` (10 x 20 cells, PISO, `dt` 1e-3). The repository's solid cases
   are `crates/rcfd-api/tests/solid_cases/*.toml`.
4. **No run of a structural case can exist in a project.** `start_run` runs `rcfd run` on the snapshot, and the flow
   reader refuses a solid case file (finding 1); no runner, CLI command or tool runs a solid case file as a project run
   (searched: `run_solid_case_file` is called only by tests of `rcfd-api`). So `show_view` with a run can never reach
   the structural layout of a real run, and clause a5 cannot be shown for structural. The rule and the layout are
   built and checked on a run folder made by the unit test (R1, L2), and the e2e row of structural is registered as
   not runnable (E8, question Q2).
5. **The stored fields and the monitors of the example runs.** `ux, uy, p` (cavities, steps), `ux, uy, p, T` (heated
   cavities), `ux, uy, p, vorticity_z` (cylinders), `ux, uy, p, nut` (NACA 0012), `ux, uy, uz, p, nut` (fan). Monitors:
   `lid_drag` (force), `nu_hot` (expression) and `q_hot` (heat_flow), `cd` and `cl` (force_coefficient), `p_inlet`
   (area_average), and the fan's `m_blade`, `m_hub`, `m_shroud` (moment), `m_in`, `m_out` (mass_flow), `p_in`, `p_out`
   (area_average). The VOF solver stores the phase fraction as `alpha` (`vof.rs` line 989) when `store_fields` names it.
6. **Where a run form fits the specification.** The tool's input schema gives `view` as an object without an inner
   schema (`crates/rcfd-mcp/src/project.rs`, the `show_view` entry), so a new form of the specification needs no change
   of the tool list. A specification with a key `run` and no `panels` is refused today (`run` is not a key; `panels` is
   required), so accepting it is an addition (G4). The tool's description lists only F-976's forms; changing it is not
   in the additive class (rule 3), so it is left (question Q1, as ADR 0807 Q1).
7. **The Studio shows any valid specification** with its existing views (`Canvas.tsx`); if `show_view` writes the chosen
   layout as an ordinary specification, no Studio code is needed for the layout itself. The panels carry
   `data-canvas-kind`; nothing in the document names a panel's field or monitor, which the e2e needs to read the views
   chosen: two test hooks are added (D3).
8. **The time of a showing.** F-976's e2e showed a seven-kind view 508 ms after the tool's result (ADR 0807 addendum 1,
   L2 row); F-979's E2 342 ms.

## Design

### D1. The categories and the rule (in `crates/rcfd-workspace/src/view.rs`)

Six categories, ids and labels: `multiphase` (Multiphase), `heat_transfer` (Heat transfer),
`internal_flow_and_turbomachinery` (Internal flow and turbomachinery), `external_aerodynamics` (External aerodynamics),
`structural` (Structural), and `general` (General) for a case of no known category.

The case of a run is read from `runs/<id>/case.snapshot`, or, where the run has none, from the case file its `run.json`
names; it is read as a flow case (`parse_case`, as `read_case_text` reads a project case), and, if that fails, as a
solid case file (`SolidCase::parse_toml_in`). The rule, first match wins, each with the rule text the reply gives:

| # | test (schema place) | category | rule text |
|---|---|---|---|
| 1 | the text reads as a solid case file and not as a flow case (`solid_case.rs` 492) | structural | `a solid case file (docs/solid-case-file.md)` |
| 2 | the text reads as neither | general | `the case of the run does not read (<first reason>)` |
| 3 | `multiphase.enabled = true` (`vof.rs` 58) | multiphase | `multiphase.enabled = true` |
| 4 | `particles.enabled = true` (`particles.rs` 248) | multiphase | `particles.enabled = true` |
| 5 | `energy.enabled = true` (`schema.rs` 399) | heat_transfer | `energy.enabled = true` |
| 6 | `phase_change.enabled = true` (`phase_change.rs` 40) | heat_transfer | `phase_change.enabled = true` |
| 7 | a `sources.<name>.type` of `rotating_frame` or `mrf` (`schema.rs` 2100), the first name in ascending order | internal_flow_and_turbomachinery | `sources.<name>.type = <type>` |
| 8 | `mixing_plane.enabled = true` (`mixing_run.rs` 50) | internal_flow_and_turbomachinery | `mixing_plane.enabled = true` |
| 9 | `mesh.generator = axial_fan_sector` with `mesh.source = generator` (`schema.rs` 78) | internal_flow_and_turbomachinery | `mesh.generator = axial_fan_sector` |
| 10 | a patch whose `velocity` or `pressure` is `freestream` (`schema.rs` 509, 555), patches ascending, velocity before pressure | external_aerodynamics | `boundaries.<patch>.<velocity or pressure> = freestream` |
| 11 | at least one patch named under `boundaries` | internal_flow_and_turbomachinery | `boundaries: <n> named, none freestream` |
| 12 | otherwise | general | `no boundaries are named` |

Derivation (first principles of what a result means): the category is the physics a case adds to a single-phase
isothermal flow, the most consequential first. A second phase changes what every field means (each cell is a mixture,
the interface is the result), so it is tested first; then heat, where the temperature is the result; then a rotating
frame or a stage, where the flow is the machine's; then whether the flow is around a body in a free stream (a far field)
or bounded by its own patches (internal). A case whose patches are not named gives the rule nothing to read and takes the
general layout, as does a case that does not read.

### D2. The layouts (panels, and the record each reads)

Each layout is an ordered list of panel templates; a template whose record is missing is left out (the negative controls
L2), so a layout never names what the run does not have. Templates:

| template | panel | record it reads; included when |
|---|---|---|
| C | `case` {case} | the case file `run.json` names; it exists under `cases/` (as F-976's `case` panel checks) |
| M | `mesh` {case} | that case file; it reads as a flow case (as F-979's mesh panel checks) |
| F(f) | `field` {run, field f} | the run's field store; its last snapshot stores `f` |
| N(types, k) | `monitor` {run, monitor} per report | the reports of the run's case whose `type` is in `types`, ordered by the type's place in `types`, then by name, whose series `monitors.<name>` is in `residuals.jsonl`; at most `k` |
| P | `plot` {run} | `residuals.jsonl`; it exists and is not empty |
| T | `table` {runs: [run]} | `run.json` (always) |

Every panel has a title: the field or monitor name, `Mesh`, `Case`, `Residuals` or `Run`. The view's title is
`<label>: <case> (<run>)`; its layout `{columns: 2}`. A `title` or `layout` given with the run replaces these.

| category | templates in order |
|---|---|
| external_aerodynamics | F(ux), F(p), N([force_coefficient, force, moment], 2), P, T |
| internal_flow_and_turbomachinery | M, F(ux), F(p), N([mass_flow, area_average, moment, force], 3), P |
| heat_transfer | F(T), F(ux), N([heat_flow, expression], 2), P, T |
| multiphase | F(alpha), F(ux), F(p), N([volume_integral, volume_average], 2), P, T |
| structural | C, T |
| general | C, P, T |

At most 8 panels by construction (internal: 7), within `MAX_PANELS`.

### D3. The specification and the tools (additive)

- `rcfd-view` 1 gains the top-level key `run`: `{version: 1, run, title?, layout?}` with no `panels` is a view of the
  run's category's layout. Checks: `run` as a `run` panel's `run` (`run_ref`, errors at `run`); `run` and `panels`
  together are refused at `panels` with "a view with run takes the layout of its case's category: give run or panels,
  not both". A view with neither keeps F-976's error at `panels`, word for word. Every other check is unchanged.
- `show_view` of the run form builds the layout (D1, D2), checks it with `view_errors` (an error there is an internal
  error, not shown), and writes it as the shown view: the Studio reads an ordinary specification. The shown file gains
  `chosen` {`run`, `case`, `category`, `rule`, `from` (the snapshot or the case file read)}; the reply gains `category`
  and `rule`; `read_view` gives `chosen` when the shown file has it. A view with panels gets F-976's reply, file and
  `read_view`, key for key. The journal line is unchanged.
- No tool, CLI flag, golden or description changes. The workspace method `show_view` is the same function.
- Studio (`src/renderer/canvas/Canvas.tsx`): each panel's wrapper gains the test hooks `data-canvas-field` (its `field`)
  and `data-canvas-monitor` (its `monitor`), absent where the panel has none. Nothing else of the Studio changes.

## Checks (rule 14)

Release profile. New tests: unit tests in `crates/rcfd-workspace/src/view.rs`, `crates/rcfd-mcp/tests/canvas_layout_f977.rs`
(the binary), `apps/studio-web/tests/e2e/canvas-f977.spec.ts`. The unit and e2e project ("the f977 project") is a copy of
`example-projects/quick-start` with, added: `cases/cylinder-re100.toml` and run `r-20261028T152128-d5cb` from
`wind-tunnel`, `cases/fan01-passage-upwind.toml` and run `r-20261030T223519-0489` from `axial-fan`, and (e2e only)
`cases/still-pool.toml`, the text of `pool_text("", "")` of `vof_tree.rs` with `end_time` 0.05 and an `[output]` block
`store_interval = 10`, `store_fields = ["alpha", "ux", "uy", "p"]`. Panels below are written kind{field or monitor}.

The registered views chosen (a2, a5):

| run (case) | category, rule | panels |
|---|---|---|
| `r-20261028T152128-d5cb` (cylinder-re100) | external_aerodynamics, `boundaries.farfield.velocity = freestream` | field{ux}, field{p}, monitor{cd}, monitor{cl}, plot, table |
| `r-20261030T223519-0489` (fan01-passage-upwind) | internal_flow_and_turbomachinery, `sources.frame.type = rotating_frame` | mesh, field{ux}, field{p}, monitor{m_in}, monitor{m_out}, monitor{p_in}, plot |
| `r-20261028T152641-f8c2` (cavity-re100) | internal_flow_and_turbomachinery, `boundaries: 2 named, none freestream` | mesh, field{ux}, field{p}, monitor{lid_drag}, plot |
| `r-20261028T152641-3900` (heated-cavity-ra1e4) | heat_transfer, `energy.enabled = true` | field{T}, field{ux}, monitor{q_hot}, monitor{nu_hot}, plot, table |
| the still pool's run (still-pool) | multiphase, `multiphase.enabled = true` | field{alpha}, field{ux}, field{p}, plot, table |
| a run folder made by the unit test whose snapshot is `solid_cases/cantilever_hex8.toml` | structural, `a solid case file (docs/solid-case-file.md)` | case, table |
| a copy of `f8c2` whose snapshot is `[mesh]` `generator = "cavity_2d"` only | general, `no boundaries are named` | case, plot, table |

| Id | Clause | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| R1 | a1 | `view.rs` unit: the rule on the 14 case files of `example-projects/`, `solid_cases/cantilever_hex8.toml` and the still pool's text | the category of finding 3 for each (fan: internal_flow_and_turbomachinery; 2 cavities, 2 steps: the same; 2 heated cavities: heat_transfer; 2 cylinders, 4 NACA 0012: external_aerodynamics; cantilever: structural; still pool: multiphase); 16 of 16 | F-977 acceptance (the five categories); D1 | 16 of 16 | exact | code verification |
| R2 | a1, a4 | `view.rs` unit: 5 texts: cylinder-re100 with `energy.enabled = true`; cavity-re100 with `[sources.spin] type = "mrf"`; `[mesh] generator = "cavity_2d"` alone; `not a case = [`; cylinder-re100 with the far field's velocity `fixed` and pressure `freestream` | heat_transfer `energy.enabled = true`; internal_flow_and_turbomachinery `sources.spin.type = mrf`; general `no boundaries are named`; general, a rule text starting `the case of the run does not read`; external_aerodynamics `boundaries.farfield.pressure = freestream`; 5 of 5 | D1 (the order and the texts) | 5 of 5 | exact | code verification |
| L1 | a2, a3 | `view.rs` unit, the f977 project: the layout of `d5cb`, `0489`, `f8c2`, `3900` | the panels of the table above, key for key with their titles, the view's title and `columns` 2, and `view_errors` empty for each; 4 of 4 | D2 | 4 of 4 | exact | code verification |
| L2 | a2, a4 | `view.rs` unit, negative controls: (a) a copy of `3900` without `fields/`; (b) a copy of `3900` without `residuals.jsonl`; (c) `f8c2` with `cases/cavity-re100.toml` removed; (d) the structural run folder; (e) the general copy of `f8c2` | (a) monitor{q_hot}, monitor{nu_hot}, plot, table; (b) field{T}, field{ux}, table; (c) field{ux}, field{p}, monitor{lid_drag}, plot; (d) case, table; (e) case, plot, table; each valid; 5 of 5 | D2 (a missing record leaves its panel out) | 5 of 5 | exact | code verification |
| V5 | a3, a6 | `view.rs` unit: `{version 1, run 3900}` and the same with a title and `layout.columns` 3 | no error | D3 | no error | exact | code verification |
| V6 | a3, a6 | `view.rs` unit: 4 invalid run forms: `run` and `panels`; `run` 42; `run` `r-20991231T000000-0000`; `run` with a key `colour` | one error each at `panels` ("not both"), `run` ("required"), `run` ("no such run"), `colour` ("not a key"); 4 of 4 | D3; F-976 acceptance (located errors) | 4 of 4 | exact | code verification |
| B1 | a3 | binary: an `rcfd-mcp --project --canvas` process and a client on a copy of `quick-start`; `show_view {view: {version 1, run 3900}}` | shown, `studios` 1, `category` heat_transfer, `rule` `energy.enabled = true`, `panels` 6; the canvas's `read_view` gives the heat_transfer panels of the table and `chosen` {run 3900, case `cases/heated-cavity-ra1e4.toml`, heat_transfer, the rule, from `runs/r-20261028T152641-3900/case.snapshot`} | D3 | as stated | exact | code verification |
| B2 | a6 | binary: `show_view` of the six-kind view of `canvas_view_f976` | the reply's keys exactly `shown, id, path, hash, studios, panels`; `read_view`'s exactly `exists, path, id, hash, shown, actor, reason, view` | rule 3; G4 | as stated | exact | unchanged default output |
| B3 | a3, a6 | binary: `show_view` of `{version 1, run r-20991231T000000-0000}` and of `{version 1, run 3900, panels [a plot]}` | 1003 at `run` and at `panels`; `.rcfd/views/current.json` and the journal byte-identical after both | F-976 acceptance (refused, the shown view unchanged) | as stated | exact | code verification |
| E0 | a5 | e2e set-up: the packaged Studio has the f977 project open; the test, as MCP client (`rcfd-mcp --project`), runs `start_run` of `cases/still-pool.toml` and `wait_run` | the run finishes (time recorded) | set-up | finishes | exact | code verification |
| E1 | a3, a5 | e2e: `show_view {view: {version 1, run d5cb}}` | within 1.6 s of the tool's result the canvas shows the view (its id) with the external_aerodynamics panels of the table in order (`data-canvas-kind`, `data-canvas-field`, `data-canvas-monitor`) and its title starting `External aerodynamics: `; then every panel holds its view (field `section.fv`, monitor and plot `.plot-frame`, table the run's row) | ADR 0188 C3-St (4 Studio periods) at `FEED_POLL_MS` 400 ms, as ADR 0805 D5 | 0.6 s | 2.7 | code verification |
| E2 | a3, a5 | e2e: the same for `0489` | internal_flow_and_turbomachinery panels and title, as E1; the mesh panel drawn (`data-drawn` true) | as E1 | 0.6 s | 2.7 | code verification |
| E3 | a3, a5 | e2e: the same for `f8c2` | internal_flow_and_turbomachinery (the bounded flow) panels and title, as E2 | as E1 | 0.6 s | 2.7 | code verification |
| E4 | a3, a5 | e2e: the same for `3900` | heat_transfer panels and title, as E1 | as E1 | 0.6 s | 2.7 | code verification |
| E5 | a3, a5 | e2e: the same for the still pool's run of E0 | multiphase panels and title, as E1 | as E1 | 0.6 s | 2.7 | code verification |
| E6 | a4, a5 | e2e negative control: the general copy of `f8c2` (copied by `copyRun`, its snapshot replaced) | general panels (case, plot, table) and title `General: `, as E1 | as E1 | 0.6 s | 2.7 | code verification |
| E7 | a2, a5 | e2e negative control: a copy of `3900` without `fields/` | monitor{q_hot}, monitor{nu_hot}, plot, table and title `Heat transfer: `, as E1 | as E1 | 0.6 s | 2.7 | code verification |
| E8 | a5 | e2e: a structural example case of the repository shown from a run | not runnable: no project run of a solid case can exist (finding 4); recorded as not run | F-977 acceptance | not run | exact | code verification |
| L3 | a6 | unedited existing tests: `canvas_view_f976`, `canvas_live_f979`, `tool_list_f275`, `project_list_f270`, `feed_f273`; `rcfd-workspace --lib` (ADR 0805 V1, V2; ADR 0807 V3, V4); `rcfd-mcp --lib` | pass | rule 3; G4 | pass | exact | unchanged default output |
| L4 | a5, a6 | `npm run typecheck` and `npm test` in `apps/studio-web` (every unit and e2e spec, `canvas-f976.spec.ts` and `canvas-f979-a1.spec.ts` unedited among them, against the branch's engine; main skips 4 tests, ADR 0807's skipped record) | pass | rule 7; G13 | pass | exact | unchanged default output |
| L5 | - | `cargo fmt -p rcfd-workspace -p rcfd-mcp --check`; `cargo clippy -p rcfd-workspace -p rcfd-mcp --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check` of the proposal file | clean; pass | rule 7; rule 9 | pass | exact | code verification |

No shared solver file is touched, so `baseline_hashes` is not run. `python scripts/process_report.py --check` is run last
and its result reported (rule 7). The CPU load is recorded with each timed run.

### Negative controls and mutations (rule 16)

The controls are R2 (no boundaries, a text that does not read), L2 and E6, E7 (a missing record, the general layout),
B2 (a view with panels unchanged) and B3. Mutations, run on a scratch copy of the final commit, each restored afterwards:

| Id | Mutation | Must fail |
|---|---|---|
| M1 | test 5 of D1 (`energy.enabled`) removed | B1 (and R1) |
| M2 | test 10 of D1 (free stream) removed | R1 |
| M3 | test 10 of D1 (free stream) moved before test 5 (external before heat) | R2 |
| M4 | the report-type filter of N removed (every monitor of the run, by name, up to k) | L1 |
| M5 | F included without the store check | L2 |
| M6 | the run form writes the request instead of the chosen layout | B1 |

M1 is committed as `crates/rcfd-mcp/tests/canvas_layout_f977.mutation.patch`, first line
`# expect-fail: cargo test --release -p rcfd-mcp --test canvas_layout_f977`. The others are run with the named test only.

## Size

Size (agent): partial; 1; 2; 2; 220000

(Expected outcome partial, since E8 cannot run; one further registration, an amendment, expected; two heavy runs on the
shared machine, the whole `npm test` of the Studio and the build of the scratch copy for the mutations; two questions
for the maintainer, Q1 and Q2; about 220 000 bytes read.)

## Decision rule

**done** if R1, R2, L1, L2, V5, V6, B1 to B3, E0 to E8 and L3 to L5 hold and M1 to M6 are each caught; **partial**
otherwise, naming the check that missed or did not run. A missed limit is recorded with its number and held as a
`*_known_miss` record (rule 2).

## Not covered

- A run of a solid case file in a project (finding 4, Q2); the structural layout is checked on a run folder the unit test
  makes, not on a run.
- Categories finer than these six (compressible flow, combustion, free-surface against dispersed flow, rotor against
  stator): the rule names its test in the reply, so a finer split is a later addition.
- A layout chosen for a case without a run (`show_view` with `case` and no layout): a run is what the acceptance names.
- Two Studios, Linux and macOS, as in ADR 0805.

## Open questions for the maintainer

- **Q1. The description of `show_view`.** It lists F-976's forms only; the run form of D3 is not in it, so a client
  learns of it from `docs/mcp-tools.md` or a refusal's reason. (a) the lead changes the description and re-records the
  tool list (an entry changed, not the additive class), as F-987 does for F-979's forms; (b) leave it. Recommendation:
  (a), since the description is what Claude reads. Not taken here (rule 3, G4).
- **Q2. Structural runs in a project (scope).** (a) an item that runs a solid case file as a project run (`start_run`
  of a solid case, its records: `run.json`, the snapshot, the report and the `.vtu`), after which the structural layout
  shows its results (the displacement and stress fields, the report) and gets its e2e row; (b) keep the structural
  category recognized and its layout of the case only. Recommendation: (a), entered as an item of its own. Not taken
  here (it changes the scope of the item, rule 6).
