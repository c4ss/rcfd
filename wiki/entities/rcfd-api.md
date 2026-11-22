---
type: entity
tags: [crate]
sources: [doc-architecture, doc-roadmap-overview, doc-process-agent-rules, doc-run-control, doc-solid-case-file, doc-case-schema-restart-chain-setup-rules, doc-case-schema-schema-ui, doc-udf-abi, doc-service-protocol, doc-tutorial-cavity, doc-case-schema-sources, doc-case-schema-components]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-api

**What it is:** The API crate over the solver crates. Hosts the milestone validation tests (cylinder, FAN-01) and `tests/baseline_hashes.rs`. ([[doc-architecture]])
**Where:** `crates/rcfd-api/` — layer 5 (api) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-fv]], [[rcfd-io]], [[rcfd-particles]], [[rcfd-tsstore]], [[rcfd-project]], [[rcfd-runner]]
- Used by: [[rcfd-workspace]], [[rcfd-mcp]], [[rcfd-cli]]

## Details

- Decision records: ADR 0737 ([[doc-architecture]]).
- ADR 0737: case.save outside a project and the runner's memory rule.
- `rcfd-api` (`runctl`, `workflow`) implements the run-control contract (flags, [[exit-codes]], [[status-stream]], [[checkpoint]]s, workflows) that the CLI, the service and the MCP tools share ([[doc-run-control]]).
- Holds the setup rule file `crates/rcfd-api/rules/setup_rules.json` ([[setup-rules]]) ([[doc-case-schema-restart-chain-setup-rules]]) and the schema UI metadata `crates/rcfd-api/src/schema_ui.toml` ([[doc-case-schema-schema-ui]]).
- `rcfd_api::solid_case::run_solid_case_file` reads the [[solid-case-file]] ([[doc-solid-case-file]]); `rcfd_api::facts` holds the `case.facts` rule ([[doc-service-protocol]]).
- Tests named: `tests/udf.rs`, `tests/udf_hooks_w38a.rs` ([[udf-sandbox]]) ([[doc-udf-abi]]), `tests/validation_cavity_ghia.rs` ([[doc-tutorial-cavity]]), `tests/mrf.rs`, `tests/lumped.rs`, `tests/heat_exchanger.rs`, `tests/mhd.rs` ([[doc-case-schema-sources]], [[doc-case-schema-components]]).

## Open questions

_Filled in by P1-P3 ingests._
