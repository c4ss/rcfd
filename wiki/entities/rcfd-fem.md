---
type: entity
tags: [crate]
sources: [doc-architecture, doc-roadmap-overview, doc-structural-solver, doc-solid-case-file]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd-fem

**What it is:** Finite-element structural solver: the solid data model and structured builders. ([[doc-architecture]])
**Where:** `crates/rcfd-fem/` — layer 2 (gpu / dist / runner / fem) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]], [[rcfd-linalg]]
- Used by: [[rcfd-post]]

## Details

- Decision records: ADR 0271, ADR 0270 ([[doc-architecture]]).
- Plan ADR 0270 (approved 2026-11-12, with ADR 0290 and 0310); not validated against a measured case ([[doc-roadmap-overview]]).
- Scope, demonstration and limits of the solver are on [[structural-solver]] ([[doc-structural-solver]]).
- Library only: "There is no CLI command and no MCP tool for the structural solver yet"; it is used from Rust or through the solid case file ([[doc-structural-solver]]).
- Modules named: `builders` (`upgrade_to_tet10`, `fan_blade_tet4`, `fan_blade_tet10`, `computer_fan_blade_tet10`, ADR 0505), `transient`, `harmonic` ([[doc-structural-solver]]). Agents start with `crates/rcfd-fem/src/lib.rs` (the module list) ([[doc-structural-solver]]).
- Static solves also run on ranks of [[rcfd-dist]] and on a CUDA device (F-433: Jacobi CG on 304 842 unknowns in 0.60 s, 4.50 s on 16 threads) ([[doc-structural-solver]]).
- The [[solid-case-file]] (F-386, ADR 0438) is read by `rcfd_api::solid_case::run_solid_case_file`; static or modal, `direct` or `cg` (default preconditioner `ic_fill`, ADR 0357), report format `rcfd-solid-report/1` ([[doc-solid-case-file]]).

## Open questions

_Filled in by P1-P3 ingests._
