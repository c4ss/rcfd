# ADR 0730: F-826 and F-830 proposal: UI metadata in the case schema, and help and an error catalogue from the engine

Status: **ACCEPTED 2026-11-17.** The maintainer took every open question on its recommendation (the answers are recorded
under "Answers"). Built on branch adr0730 the same day; the results are under "Result". The advanced list below was
proposed by the lead and **approved by the maintainer as it stood, 2026-11-17**.

Roadmap items F-826 (Engine: UI metadata in the case schema) and F-830 (Electron Studio: explain in place from the engine's
docs). F-830 depends on F-826 (`docs/roadmap.json`, F-830 `deps`), and both need the same thing from the engine: text and
flags the Studio shows but must not own. So they share this ADR. Parents: [ADR 0800](0800-electron-studio-roadmap.md),
guardrails G2, G4, G15, G16 and G19 (`docs/studio-guardrails.md`). Number 0730: 0729 is the highest ADR on main (d56933a8)
and on every branch; 08xx is the Studio range.

## Context

What the engine gives today:

- The case schema is built in code by `rcfd_api::schema::case_schema` (`crates/rcfd-api/src/schema.rs:66`). Each node is
  a `rcfd_core::settings::Schema` (`crates/rcfd-core/src/settings.rs:82`) with a kind, a `doc` string, an `optional` flag,
  a display `unit` (`settings.rs:231`, exported as `x-rcfd-unit`) and `hash_omit_default`. Floats carry a hard range and an
  SI dimension (`settings.rs:37`, exported as `x-rcfd-si-dimension`).
- The export is `Schema::to_json_schema` (`settings.rs:271`, node writer `settings.rs:279`). There is no label, no
  recommended range, no "advanced" flag and no short help. `doc` is often a paragraph: `mesh.generator`'s is over 3000
  characters (`schema.rs:75-80`).
- The service returns that export from `settings.schema` (`crates/rcfd-api/src/service.rs:429-432`), which ignores its
  params. `rcfd schema` prints the same export (`crates/rcfd-cli/src/main.rs:301-319`).
- Error codes are spread over several places: the service's `code` module (`service.rs:35-58`) and its table in
  `docs/service-protocol.md:32-46`; the workspace codes 1200-1202 and 1002-1004 (`crates/rcfd-workspace/src/lib.rs:55-66`)
  and their list in `docs/mcp-tools.md:145-149`; the MCP policy refusal 1100 (`docs/mcp-tools.md:18`); the runner's exit
  codes (`crates/rcfd-runner/src/runner.rs:26`, `crates/rcfd-runner/src/main.rs`). No record gives a recovery for a code.
- The glossary is `docs/glossary.md` (25 lines). No engine call returns it.
- Opt-in method families already exist: `data.*` is off unless the embedder calls `enable_data`, and only then does
  `service.info` report protocol 1.2 and the extra methods (`crates/rcfd-api/src/data_service.rs:30-34`, `:109-118`).

What the Studio does today:

- F-825 lays out the 948 case keys from `settings.schema` (`apps/studio-web/src/renderer/settings/schema.ts:109`, `:121`).
  With no importance metadata it guesses tiers from structure: the first 8 own keys of a section are shown, the rest are one
  step away (`apps/studio-web/src/renderer/settings/disclosure.ts:4-20`). It shows the raw `description` as a tooltip
  (`apps/studio-web/src/renderer/settings/KeyRow.tsx:74`, `:85`).
- The first path keeps its own explanatory words and cites a doc by name
  (`apps/studio-web/src/renderer/first/words.ts:47-55`). G19 and F-830 ask for no help text that lives only in the Studio.
- The Studio calls only allow-listed service methods (`apps/studio-web/src/shared/service.ts:21-36`), each through a named
  action (`apps/studio-web/src/shared/actions/registry.ts:85`, `:127-132`).

## Proposal

### P1. Metadata on schema nodes (F-826)

A schema node gets optional UI metadata. None of it is used by validation, parsing, unit conversion or hashing.

| Field | JSON key (export) | Meaning |
|---|---|---|
| label | `x-rcfd-label` | A short human name, for example "Cells in x". |
| advanced | `x-rcfd-advanced` | `true`: one step away in a learner-first layout. Absent means common. |
| recommended | `x-rcfd-recommended` `{minimum, maximum}` | A usual working range, inside the hard range. Advice only; never refused. |
| help | `x-rcfd-help` | One line, at most 160 characters, no newline. |
| see | `x-rcfd-see` | The docs page and heading that explain the key, for example `case-schema.md#mesh-generators-and-boundary-layer-inflation`. |
| unit | `x-rcfd-unit` | Only for a node that has neither `x-rcfd-unit` nor `x-rcfd-si-dimension` today (a count, a ratio), and only in the `ui: true` export. Floats with a dimension already export its text (`settings.rs:314-316`); those are not repeated. |

Storage (see Q1): a data file in `rcfd-api` keyed by dotted schema path (`mesh.nx`, `boundaries.*.velocity` with `*` for a
named-object segment), read with `include_str!` and applied when the schema is exported. `schema.rs` is not edited.

### P2. Delivery (F-826)

- Service: `settings.schema {ui: true}` returns the same reply with the P1 keys added to the nodes that have them.
  `settings.schema` with no params, or `ui: false`, returns today's reply byte for byte. An unknown param type is -32602.
- CLI: `rcfd schema --ui [--out <file>]`. `rcfd schema` without `--ui` prints today's text byte for byte.
- MCP: no new tool. Claude Code gets the same text with `rcfd schema --ui` (G16).

### P3. Help and the error catalogue (F-830)

Two read-only service methods, in an opt-in family `help.*`:

- `help.glossary {}` returns `{source: "docs/glossary.md", terms: [{term, meaning, page}]}`, parsed from the table in
  `docs/glossary.md` at build time.
- `help.errors {}` returns `{errors: [{surface, code, name, meaning, recovery}]}`. `surface` is one of `service`,
  `workspace`, `mcp`, `runner_exit`, `cli_exit`. `meaning` is the text of the existing tables; `recovery` is one new line
  per code that says what the user can do next ("Open the case and fix the setting named in `data.diagnostics`").
- Errors of these methods: -32602 for any params other than `{}`; nothing else can fail.

The catalogue is a Rust constant (`rcfd_api::errors::CATALOGUE`). Messages that the engine already returns do not change.
The Studio shows an error as the engine's message (the problem) followed by the catalogue's `recovery` for its code. That
is how "every error message names the problem and the recovery" is met without editing any reply.

CLI: `rcfd help glossary` and `rcfd help errors`, each printing the same JSON (G16).

### P4. Opt-in and protocol version

The `ui` param and the `help.*` methods are on only when the server is started with `rcfd serve --ui`. Then, as
`data_service::extend_info` does for 1.2, `service.info` adds `"ui": "rcfd-ui/1"` and the two method names to `methods`.
Without `--ui`, `service.info` and every method answer as now, and `help.*` is -32601. The Studio's spawn adds `--ui`
(`apps/studio-web/src/main/service/spawn.ts:90`) and the three names join its allow-list. See Q2 for the version number.

### How defaults and `baseline_hashes.rs` stay unchanged

- No solver, mesher, parser or default value is touched. Metadata lives outside `Kind` and outside the data, and
  `SettingsTree::content_hash` hashes the data only (`settings.rs:561-567`), so every case hash stays the same.
- The default reply of `settings.schema`, the default `rcfd schema` text, `service.info` without `--ui` and the MCP tool
  lists do not change. Tests T1 and T2 hold them to the bytes recorded at the base commit.
- `crates/rcfd-api/tests/baseline_hashes.rs` and every golden are not edited or re-recorded. No existing test is edited.

## Alternatives

1. **Metadata inline in `schema.rs` (`.label(..).advanced()`) vs a side data file keyed by path.** Inline keeps the label
   next to the key, so whoever adds a key sees it. The side file leaves the 2530-line `schema.rs` alone and can be reviewed
   as data, with a test that every path in it names a real node. Recommendation: side file (Q1).
2. **Add the keys to the default `settings.schema` reply vs opt-in.** Adding is simpler for clients, but it changes an
   existing reply, which G4 forbids without the maintainer. Recommendation: opt-in (`ui: true` behind `--ui`).
3. **A separate method `settings.ui` returning a path-to-metadata map.** It keeps the schema reply clean, but the Studio
   would have to merge two trees and keep the merge in step with named-object paths. Recommendation: no; one tree with
   `ui: true`.
4. **Short help taken automatically from the first sentence of `doc`.** No new text to write, but many first sentences
   are not good help ("A list of three numbers; ...") and some docs open with a clause. Recommendation: hand-written
   `help` where it exists, with the first sentence of `doc` as the fallback the Studio shows (Q4).
5. **The Studio reads `docs/*.md` from disk.** Free on the engine side, but it puts parsing of engine docs in the Studio
   (G2) and breaks when the docs are not installed. Recommendation: no; the engine serves them.
6. **Recovery text inside each error reply (`data.recovery`).** Changes every reply shape. Recommendation: no; by code
   from the catalogue.

## Open questions for the maintainer

1. **Where the metadata lives.** (a) a side data file in `rcfd-api` keyed by path; (b) inline builder calls in
   `schema.rs`; (c) both, inline winning. Recommendation: (a).
2. **Opt-in and version.** (a) `rcfd serve --ui`, a `ui` key in `service.info`, `protocol_version` unchanged at 1.1 (1.2
   with `data.*`); (b) the same flag with `protocol_version` 1.3 when on; (c) always on, adding to `service.info` (needs
   your approval under G4). Recommendation: (a), so versions stay a property of the method families.
3. **Who decides "advanced".** (a) the lead proposes a list from the docs, you approve it once, then each new key's
   author sets it; (b) a rule (every key outside the quick-start cases is advanced); (c) no flag, only the order.
   Recommendation: (a).
4. **Help text source.** (a) hand-written one-liners, fallback to the first sentence of `doc`; (b) only the first
   sentence of `doc`, no new text; (c) hand-written for every leaf before F-826 is done. Recommendation: (a), with a
   coverage count reported by a test but not gated.
5. **Recommended ranges.** (a) only where `docs/case-schema.md` or an ADR already states one, cited in the data file;
   (b) the lead proposes ranges from experience; (c) none in F-826. Recommendation: (a); no number without a source.
6. **Docs embedded in the binary.** (a) `include_str!` of `docs/glossary.md` (and later headings of
   `docs/case-schema.md` for `see`); (b) read from an install folder at run time. Recommendation: (a); a test fails when
   the file changes shape.
7. **Error catalogue scope.** (a) every code of the service, workspace, MCP policy and the runner and CLI exit codes;
   (b) service and workspace codes only. Recommendation: (a).
8. **Should `help.*` and `rcfd help` also be MCP tools?** (a) no, `rcfd help` covers Claude Code; (b) yes, two read
   tools (changes the tool list goldens, needs a registered restatement). Recommendation: (a).

## Answers (the maintainer, 2026-11-17)

1. (a) The metadata lives in a separate data file keyed by setting path, beside the schema code
   (`crates/rcfd-api/src/schema_ui.toml`). A test checks that every path in it exists in the schema and reports the
   schema paths it lacks.
2. (a) Opt-in behind `rcfd serve --ui` (the `ui` key of `service.info`); the schema and protocol version stay at 1.1.
   Without the switch `settings.schema`, `rcfd schema`, `service.info` and the method list are byte-identical to the
   code before this ADR; T1, T2, P4 and the pinned MCP tests prove it (restated without a frozen recording, see
   "Result", the fixture note).
3. (a) The lead proposes the advanced list and the maintainer approves it once. The list is in the data file, marked
   proposed at first and approved by the maintainer as it stood on 2026-11-17 (`[advanced_list] status = "approved"`),
   and as the table below with one reason per setting. Settings not on it are not advanced.
4. (a) Hand-written help, falling back to the first sentence of the matching docs section and then of the schema
   description. Coverage is reported, not gated. Every key the example projects set has a written help line and label.
5. (a), tightened: a recommended range only with a cited source, public literature (author, title, year) or one of this
   repository's validation ADRs. No source, no range; no citation is invented; the project is clean-room.
6. (a) The docs are built into the binary with `include_str!` (`docs/glossary.md`, `docs/case-schema.md`).
7. (a) The catalogue covers every surface: the service, the project tools, the MCP server, and the exit codes of `rcfd`,
   `rcfd-runner`, `rcfd-mcp` and its edit hook, one recovery line per code. A test reads the codes from the sources and
   fails on one that is not in the catalogue.
8. (a) No help MCP tools; `rcfd help glossary|errors` covers Claude Code. M3 checks that no help tool is listed.

### The advanced list (approved by the maintainer, 2026-11-17)

169 settings. Each sits one step away, behind "Show all" in the Studio (G15: never hidden for good; a setting the case
sets is always shown). Every other setting is common. From the approval on, the author of each new key sets its flag.

| Setting | Why it is one step away |
|---|---|
| `boundaries.*.backflow_direction` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.backflow_pressure` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.bc_kind` | The boundary-condition catalogue, another way to state a condition; the plain keys suit a first case. |
| `boundaries.*.bc_pressure` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.compressible_bc` | The density-based compressible solver only. |
| `boundaries.*.density_value` | The variable-density (ideal gas) solver only. |
| `boundaries.*.electric` | Electric conduction only (electric.enabled). |
| `boundaries.*.electric_current` | Electric conduction only (electric.enabled). |
| `boundaries.*.electric_potential` | Electric conduction only (electric.enabled). |
| `boundaries.*.emissivity` | The radiation model only. |
| `boundaries.*.epsilon_value` | Explicit inlet turbulence values; intensity and length scale are the usual way. |
| `boundaries.*.flow_angle` | The density-based compressible solver only. |
| `boundaries.*.flow_direction` | The density-based compressible solver only. |
| `boundaries.*.flow_rate_weight` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.free_stream_temperature` | Settings of temperature = convection only. |
| `boundaries.*.heat_flux_expression` | A varying value from a formula, a user function or a table; a constant value is the usual start. |
| `boundaries.*.heat_flux_udf` | A varying value from a formula, a user function or a table; a constant value is the usual start. |
| `boundaries.*.heat_transfer_coefficient` | Settings of temperature = convection only. |
| `boundaries.*.inlet` | The variable-density (ideal gas) solver only. |
| `boundaries.*.k_value` | Explicit inlet turbulence values; intensity and length scale are the usual way. |
| `boundaries.*.loss_coefficient` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.mach_value` | The density-based compressible solver only. |
| `boundaries.*.marangoni_coefficient` | A special wall condition (shear, slip or Marangoni) only. |
| `boundaries.*.mass_flow_fraction` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.mass_flow_shape_expression` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.mass_flow_source_patch` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.mass_flow_value` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.mean_free_path` | A special wall condition (shear, slip or Marangoni) only. |
| `boundaries.*.nutilde_value` | Explicit inlet turbulence values; intensity and length scale are the usual way. |
| `boundaries.*.omega_value` | Explicit inlet turbulence values; intensity and length scale are the usual way. |
| `boundaries.*.pressure_average` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.pressure_relax` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.pressure_update` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.prevent_reverse_flow` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.profile` | A varying value from a formula, a user function or a table; a constant value is the usual start. |
| `boundaries.*.radial_frame_rate` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.radial_reference` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.radiation_temperature` | Settings of temperature = convection only. |
| `boundaries.*.reflection_value` | The density-based compressible solver only. |
| `boundaries.*.shear_stress` | A special wall condition (shear, slip or Marangoni) only. |
| `boundaries.*.slip_length` | A special wall condition (shear, slip or Marangoni) only. |
| `boundaries.*.specularity` | A special wall condition (shear, slip or Marangoni) only. |
| `boundaries.*.swirl_axis` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.swirl_origin` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.swirl_rate` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.target_mass_flow` | A setting of one catalogue kind (bc_kind) only. |
| `boundaries.*.temperature_expression` | A varying value from a formula, a user function or a table; a constant value is the usual start. |
| `boundaries.*.temperature_udf` | A varying value from a formula, a user function or a table; a constant value is the usual start. |
| `boundaries.*.total_pressure_value` | The variable-density (ideal gas) solver only. |
| `boundaries.*.velocity_expression_x` | A varying value from a formula, a user function or a table; a constant value is the usual start. |
| `boundaries.*.velocity_expression_y` | A varying value from a formula, a user function or a table; a constant value is the usual start. |
| `boundaries.*.velocity_expression_z` | A varying value from a formula, a user function or a table; a constant value is the usual start. |
| `boundaries.*.velocity_udf` | A varying value from a formula, a user function or a table; a constant value is the usual start. |
| `boundaries.*.wall_conductivity` | Settings of temperature = convection only. |
| `boundaries.*.wall_emissivity` | Settings of temperature = convection only. |
| `boundaries.*.wall_rotation_axis` | Moving or rotating walls only. |
| `boundaries.*.wall_rotation_origin` | Moving or rotating walls only. |
| `boundaries.*.wall_rotation_rate` | Moving or rotating walls only. |
| `boundaries.*.wall_thickness` | Settings of temperature = convection only. |
| `boundaries.*.wall_velocity_projection` | Moving or rotating walls only. |
| `energy.heat_source` | The ideal-gas solver only. |
| `energy.relax` | Numerics of the temperature equation; the default is tested. |
| `energy.viscous_heating` | The ideal-gas solver only. |
| `initial.coarse_factor` | Settings of method = coarse only. |
| `initial.coarse_iterations` | Settings of method = coarse only. |
| `initial.velocity_expression_x` | A varying start from a formula; a uniform start is the usual one. |
| `initial.velocity_expression_y` | A varying start from a formula; a uniform start is the usual one. |
| `initial.velocity_expression_z` | A varying start from a formula; a uniform start is the usual one. |
| `mesh.core_fraction` | One generator only (pipe_sweep, wedge_2d or nozzle_2d). |
| `mesh.exit_height` | One generator only (pipe_sweep, wedge_2d or nozzle_2d). |
| `mesh.isotropic_interface` | Rotor-stator and sector generators only. |
| `mesh.layer_reference_length` | Which walls get layers and the inputs of the y+ estimate; the defaults estimate them. |
| `mesh.layer_reference_velocity` | Which walls get layers and the inputs of the y+ estimate; the defaults estimate them. |
| `mesh.layer_u_tau` | Which walls get layers and the inputs of the y+ estimate; the defaults estimate them. |
| `mesh.layer_walls` | Which walls get layers and the inputs of the y+ estimate; the defaults estimate them. |
| `mesh.nx_right` | The second block of an interface or stage generator only. |
| `mesh.ny_right` | The second block of an interface or stage generator only. |
| `mesh.nz_right` | The second block of an interface or stage generator only. |
| `mesh.periodic_x` | Cyclic sides, for periodic channel or pipe flows only. |
| `mesh.periodic_y` | Cyclic sides, for periodic channel or pipe flows only. |
| `mesh.periodic_z` | Cyclic sides, for periodic channel or pipe flows only. |
| `mesh.r_interface` | Rotor-stator and sector generators only. |
| `mesh.ramp_angle` | One generator only (pipe_sweep, wedge_2d or nozzle_2d). |
| `mesh.ramp_start` | One generator only (pipe_sweep, wedge_2d or nozzle_2d). |
| `mesh.sector_angle` | Rotor-stator and sector generators only. |
| `mesh.throat_height` | One generator only (pipe_sweep, wedge_2d or nozzle_2d). |
| `output.retain_checkpoints` | Disk housekeeping; keeping everything is the default. |
| `output.retain_snapshots` | Disk housekeeping; keeping everything is the default. |
| `output.store_time_interval` | Stores by simulated time instead of by step, for time-dependent runs only. |
| `output.write_time_interval` | Landing on exact times, for time-dependent runs only. |
| `output.write_times` | Landing on exact times, for time-dependent runs only. |
| `physics.body_force` | A uniform driving force, mainly for periodic flows. |
| `reports.*.component` | Splits a force into its pressure and viscous parts; total is the usual choice. |
| `reports.*.conductivity` | heat_flow reports at constant density only; 0 takes the fluid’s conductivity. |
| `reports.*.l_ref` | Moment reports only. |
| `reports.*.origin` | Moment reports only. |
| `reports.*.scalar` | Scalar reports only. |
| `reports.*.statistic` | y_plus reports only. |
| `solver.blend_factor` | Tunes one convection scheme only, when solver.convection picks it. |
| `solver.bulk_direction` | Drives periodic channel and pipe flows only. |
| `solver.bulk_velocity` | Drives periodic channel and pipe flows only. |
| `solver.component_diagonal` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.compressive_k` | Tunes one convection scheme only, when solver.convection picks it. |
| `solver.corrector_control` | A variant of the transient loop; the default PISO/PIMPLE loop suits most runs. |
| `solver.ddt_correction` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.divergence_courant_limit` | A safety check of the solver; its default already watches for trouble. |
| `solver.dt_growth` | Fine control of adaptive time stepping; the Courant target alone is enough to start. |
| `solver.dt_safety` | Fine control of adaptive time stepping; the Courant target alone is enough to start. |
| `solver.dt_shrink` | Fine control of adaptive time stepping; the Courant target alone is enough to start. |
| `solver.error_tolerance` | Fine control of adaptive time stepping; the Courant target alone is enough to start. |
| `solver.extrapolate_initial` | A variant of the transient loop; the default PISO/PIMPLE loop suits most runs. |
| `solver.fractional_boundary` | A variant of the transient loop; the default PISO/PIMPLE loop suits most runs. |
| `solver.fractional_form` | A variant of the transient loop; the default PISO/PIMPLE loop suits most runs. |
| `solver.frozen_flow` | Prescribes the flow instead of solving it, for heat or scalar studies only. |
| `solver.gradient_boundary` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.gradient_face_correction` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.gradient_limiter_k` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.gradient_secondary_limiter` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.h_operator` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.high_resolution_beta` | Tunes one convection scheme only, when solver.convection picks it. |
| `solver.linear_upwind_limiter` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.max_courant_rms` | Fine control of adaptive time stepping; the Courant target alone is enough to start. |
| `solver.max_dt` | Fine control of adaptive time stepping; the Courant target alone is enough to start. |
| `solver.min_dt` | Fine control of adaptive time stepping; the Courant target alone is enough to start. |
| `solver.min_iterations` | Only matters with the stop conditions of the convergence section. |
| `solver.momentum_max_iter` | Inner linear-solver limits; 0 keeps the algorithm’s tested default. |
| `solver.momentum_rel_tol` | Inner linear-solver limits; 0 keeps the algorithm’s tested default. |
| `solver.p_ref_cell` | Pins the pressure level when no boundary fixes it; the default cell works. |
| `solver.p_ref_value` | Pins the pressure level when no boundary fixes it; the default cell works. |
| `solver.period` | Fine control of adaptive time stepping; the Courant target alone is enough to start. |
| `solver.periodic_gradient` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.pressure_abs_tol` | Inner linear-solver limits; 0 keeps the algorithm’s tested default. |
| `solver.pressure_interpolation` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.pressure_lag_check` | A safety check of the solver; its default already watches for trouble. |
| `solver.pressure_max_iter` | Inner linear-solver limits; 0 keeps the algorithm’s tested default. |
| `solver.pressure_rel_tol` | Inner linear-solver limits; 0 keeps the algorithm’s tested default. |
| `solver.pseudo_cfl` | Pseudo-transient steady solving, a way to rescue hard cases; off by default. |
| `solver.pseudo_control` | Pseudo-transient steady solving, a way to rescue hard cases; off by default. |
| `solver.pseudo_dt` | Pseudo-transient steady solving, a way to rescue hard cases; off by default. |
| `solver.pseudo_factor` | Pseudo-transient steady solving, a way to rescue hard cases; off by default. |
| `solver.pseudo_mode` | Pseudo-transient steady solving, a way to rescue hard cases; off by default. |
| `solver.pseudo_scale_max` | Pseudo-transient steady solving, a way to rescue hard cases; off by default. |
| `solver.residual_norm` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.simplec_boundary_compensation` | A SIMPLEC flux correction for skewed or sheared meshes; off suits most meshes. |
| `solver.simplec_non_orthogonal_compensation` | A SIMPLEC flux correction for skewed or sheared meshes; off suits most meshes. |
| `solver.steady_divergence_guard` | A safety check of the solver; its default already watches for trouble. |
| `solver.steps_per_period` | Fine control of adaptive time stepping; the Courant target alone is enough to start. |
| `solver.stop_when` | Only matters with the stop conditions of the convergence section. |
| `solver.time_advance` | A variant of the transient loop; the default PISO/PIMPLE loop suits most runs. |
| `solver.velocity_correction` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `solver.viscous_stress` | A discretisation detail of the solver kernel; the default is the tested choice. |
| `turbulence.convection` | Numerics of the turbulence equations; the defaults are tested. |
| `turbulence.e_wall` | A model constant; change it only to reproduce a published set-up. |
| `turbulence.epsilon_initial` | Explicit start values; by default they come from the start intensity and viscosity ratio. |
| `turbulence.k_initial` | Explicit start values; by default they come from the start intensity and viscosity ratio. |
| `turbulence.kappa` | A model constant; change it only to reproduce a published set-up. |
| `turbulence.length_scale` | For the algebraic models (smagorinsky, wale, mixing_length) only. |
| `turbulence.mixing_profile` | For the algebraic models (smagorinsky, wale, mixing_length) only. |
| `turbulence.omega_initial` | Explicit start values; by default they come from the start intensity and viscosity ratio. |
| `turbulence.plugin` | For a registered plugin model only. |
| `turbulence.realizable_a0` | A model constant; change it only to reproduce a published set-up. |
| `turbulence.relax` | Numerics of the turbulence equations; the defaults are tested. |
| `turbulence.relax_nut` | Numerics of the turbulence equations; the defaults are tested. |
| `turbulence.roughness_constant` | Rough walls only; walls are smooth by default. |
| `turbulence.roughness_height` | Rough walls only; walls are smooth by default. |
| `turbulence.sst_variant` | A model variant or wall treatment; the default is the standard form. |
| `turbulence.wall_damping` | For the algebraic models (smagorinsky, wale, mixing_length) only. |
| `turbulence.wall_omega` | A model variant or wall treatment; the default is the standard form. |
| `turbulence.wall_treatment` | A model variant or wall treatment; the default is the standard form. |

## Tests that would prove it

- **T1** `settings.schema` with no params and with `{ui: false}` equals, byte for byte, the reply saved from the base
  commit into a new fixture before any code. `rcfd schema` the same.
- **T2** `service.info` of a service without `--ui` equals the base commit's reply; `help.glossary` is -32601 there.
- **T3** every path in the metadata file names a node of `case_schema()`; a path that names nothing fails with its line.
- **T4** every `x-rcfd-recommended` lies inside the node's hard `minimum` and `maximum`, and only on numeric nodes.
- **T5** every `x-rcfd-help` is one line of at most 160 characters; every `x-rcfd-see` names an existing docs file and
  heading.
- **T6** the export with `ui: true`, with every `x-rcfd-*` key added by this ADR removed, equals the default export.
- **T7** `help.glossary` returns every row of `docs/glossary.md`; a changed table shape fails the test.
- **T8** the catalogue holds every constant of `service::code` and every workspace code, each with a non-empty recovery,
  and agrees with the tables in `docs/service-protocol.md` and `docs/mcp-tools.md`.
- **M3** the MCP `tools/list` result, without and with `--project`, names no help tool: the ADR adds no MCP tool. The
  bytes of the plain lists are held by the pinned `default_f1.rs` and `tool_list_f275.rs`.
- **T9** the existing case and run hash tests (`baseline_hashes.rs`, `case_hashes_w48u.rs`, `w48v_case_hashes.rs`) pass
  unedited.
- **Studio (F-825, F-830)** the disclosure reads `x-rcfd-advanced` instead of `COMMON_KEYS` when present; a test adds a
  key with metadata to a test schema and sees it placed with no Studio change; an e2e test calls every catalogue code's
  error and checks the shown text names the recovery; a static check that `first/words.ts` keeps no help text of its own;
  a view-and-report step (G19).

## Result (2026-11-17, branch adr0730)

Built:

- Engine: `crates/rcfd-api/src/ui.rs` (the metadata layer, `help.*`, the `--ui` family `rcfd-ui/1`),
  `crates/rcfd-api/src/schema_ui.toml` (the data file), `crates/rcfd-api/src/errors.rs` (the error catalogue),
  `Service::enable_ui`; `rcfd schema --ui`, `rcfd serve --ui`, `rcfd help [glossary | errors]`.
- Docs: `docs/service-protocol.md` (the `rcfd-ui/1` section), 13 new rows of `docs/glossary.md` (the terms the first
  path explains).
- Studio (F-826): the schema model reads `x-rcfd-label`, `-help`, `-help-from`, `-see`, `-advanced`, `-recommended` and a
  list's own `x-rcfd-unit`. With `x-rcfd-ui` on the root the disclosure shows the common keys and every set key, and
  puts the advanced ones behind "Show all: N advanced keys of X"; without it the old first-eight rule stays. A key row is
  named by the engine's label (its path beside it), its tooltip is the help line, and its focus detail adds the help,
  "Usually a to b (source: ...)" and the docs heading. Search reads label and help. The Studio starts `rcfd serve --ui`
  and asks `settings.schema {ui: true}`.
- Studio (F-830): `src/renderer/help/engine.ts` asks `help.glossary` and `help.errors` once per service instance. The
  first path's Explain terms name a glossary term and show the engine's row with its source; `first/words.ts` keeps no
  meaning of its own. An engine error in the settings surface, the case editor's open failure and run control's
  refusals gets the catalogue's recovery line for its code.

Counts: 327 entries in the data file (746 of 1073 schema paths have none); 169 advanced (approved); **1 recommended
range with a source** (`turbulence.nutilde_ratio` 3 to 5, Spalart and Rumsey, AIAA Journal 45(10), 2007; no other range
had a source this lead could cite with confidence). Help coverage over 1073 nodes (951 leaves): written 148 (130
leaves), first sentence of the docs 19, first sentence of the schema text 762, none 144.

Word budgets (G22): unchanged. Labels are marked `data-words="data"` like the key names they replace; help, ranges and
sources are focus detail, and the recovery lines are exception text, so the budgets at rest hold with no change.

Tests: T1 to T8, P2, P4, M3 and the CLI test (`crates/rcfd-api/tests/ui_byte_identity_adr0730.rs`,
`ui_metadata_adr0730.rs`, `errors_catalogue_adr0730.rs`, `crates/rcfd-cli/tests/ui_cli_adr0730.rs`,
`crates/rcfd-mcp/tests/tool_list_adr0730.rs`) pass; T9 passes with the hash tests unedited. The Studio's specs for the
schema model, the first path, the action registry, the help catalogue and the affected e2e walks (settings, case editor,
first path, run control, service, word budget, a11y) pass.

View-and-report for F-826 (the F-810/F-812 precedent): `apps/studio-web/tests/e2e/settings-ui-f826.spec.ts` runs the
settings surface against `rcfd serve --ui` and writes four screenshots at 1440 by 900 to
`apps/studio-web/test-results-visual/f826/` (solver at rest, Show all open, a focus detail with the help line, the usual
range of `turbulence.nutilde_ratio` with its source). With the list approved, F-826 is done; F-830 stays partial.

Not as planned: the catalogue check on the Studio side is a unit test over the real `rcfd help errors`
(`apps/studio-web/tests/unit/help-engine.spec.ts`: every JSON-RPC code gives its recovery line), not an e2e walk that
provokes each error; and no screenshot view-and-report was made for the new focus detail. Both are left for the
maintainer to ask for.

The fixture note (restated 2026-11-17, on merging main 14a2d490). T1, T2, P4, T6 and M3 were first written against
replies recorded from the f86339e8 binaries before any code (`crates/rcfd-api/tests/fixtures/adr0730_base_replies.json`).
Main had added MCP tools since (ADR 0731 `create_project`, runner_config on the allow-list) and more are coming (ADRs
0734 to 0736), so a frozen recording would need re-recording on every merge, which the maintainer does not do. The fixture
is removed (it stays in the history of branch adr0730, commit d7b68d88, as evidence that the base replies were taken before
the code); nothing asserts against a recording now. The tests are restated:

- T1: `settings.schema` without `ui: true`, with and without `--ui`, equals byte for byte the serialisation of
  `{case: case_schema().to_json_schema(), monitors: monitor_json_schema()}`, the code path this ADR leaves as it was;
  `rcfd schema` equals `to_string_pretty` of the same export (`crates/rcfd-cli/tests/ui_cli_adr0730.rs`).
- T2: `service.info` without `--ui` has no `ui` key and lists exactly `METHODS`; `help.*` is the plain -32601.
- P4: the `--ui` reply equals the plain reply of the same build plus `ui` and `UI_METHODS` at the end of the method list,
  in process and through `rcfd serve --stdio`.
- T6: the `ui: true` export less the keys this ADR adds equals the plain export of the same build.
- M3: no help tool in either MCP list; the bytes of the plain lists are the pinned `default_f1.rs` (the ADR 0135 golden)
  and `tool_list_f275.rs`, which every ADR that adds a tool restates. No byte-pinned test of the schema text exists in
  the repository; T1's comparison with the untouched builder and T9's case hashes stand for it.
