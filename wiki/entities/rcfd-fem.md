---
type: entity
tags: [crate]
sources: [doc-architecture, doc-roadmap-overview, doc-structural-solver, doc-solid-case-file, adr-thread-structural-linear-solvers, adr-thread-structural-case-file-and-fan-blade, adr-thread-structural-modal-buckling-cyclic, adr-thread-structural-incompatible-hex8-rule-l, adr-thread-structural-hyperelastic-and-follower, adr-thread-structural-beam-warping, adr-thread-structural-fracture, adr-thread-structural-creep-and-temperature, adr-thread-structural-plan]
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
- **How it was planned** ([[adr-thread-structural-plan]]): three plan ADRs of 2026-11-12 (0270 basic, 0290 intermediate, 0310 advanced) reserved 66 work packages, F-370 to F-436; the maintainer named the crate `rcfd-fem` in layer 2 (ADR 0270 M2, proposed as `rcfd-solid`). The basic tier's registrations kept the plan's numbers (ADRs 0271 to 0286, [[adr-thread-structural-basic-tier]]); later packages were registered under new numbers.

## Open questions

_Filled in by P1-P3 ingests._
- Many structural items in ADR 0354 to 0518 are partial only because "The maintainer decides done" (a mutation that was not caught, or an amendment made after a run). The list is on [[roadmap-structural-solver]] ([[adr-thread-structural-modal-buckling-cyclic]]).
- F-405 (CLI subcommand and MCP tools) is still open. ADR 0438 Q3 leaves it to the maintainer because each changes a golden file ([[adr-thread-structural-case-file-and-fan-blade]]).

## ADR threads

- Modules named in the threads: `cyclic`, `mpc`, `transient`, `harmonic`, `rotor`, `fatigue`, `random`, `estimate`, `follower`, `contact.rs`, `laminate.rs`, `mat_j2.rs`, `mat_visco.rs`, `mat_creep.rs`, `hex8x`, `builders`, plus `rcfd_api::solid_case` and `rcfd_api::solid_parallel` ([[adr-thread-structural-case-file-and-fan-blade]], [[adr-thread-structural-linear-solvers]]).
- Thread pages: [[adr-thread-structural-verification-elements]], [[adr-thread-structural-linear-solvers]], [[adr-thread-structural-modal-buckling-cyclic]], [[adr-thread-structural-rotordynamics]], [[adr-thread-structural-dynamics-harmonic]], [[adr-thread-structural-fatigue-and-radiated-sound]], [[adr-thread-structural-beams-shells]], [[adr-thread-structural-materials-nonlinear]], [[adr-thread-structural-contact]], [[adr-thread-structural-fsi-coupling]], [[adr-thread-structural-case-file-and-fan-blade]].
- Second half of the category (ADR 0519 to 0746, p3-structural-b): modules `nl_incompatible` ([[adr-thread-structural-incompatible-hex8-rule-l]]), `mat_hyper` ([[adr-thread-structural-hyperelastic-and-follower]]), `beam_warping` ([[adr-thread-structural-beam-warping]]), `fracture` ([[adr-thread-structural-fracture]]), `thermal` temperature fields ([[adr-thread-structural-creep-and-temperature]]), and in `rcfd-linalg` `eigen_pencil` and `sparse_chol::SupernodalAnalysis` ([[adr-thread-structural-hyperelastic-and-follower]], [[adr-thread-structural-linear-solvers]]). Concepts: [[incompatible-modes-hex8]], [[hyperelastic-materials]], [[arc-length-continuation]], [[beam-warping]].
- Two default changes made in the library on maintainer approval of 2026-11-18: Beam2 in the cyclic constraint (ADR 0744) and hyperelastic materials on structural elements (ADR 0746) ([[adr-thread-structural-modal-buckling-cyclic]], [[adr-thread-structural-hyperelastic-and-follower]]).
