---
type: source
tags: [adr-thread, structural, fem, case-file, fan, demonstration]
sources: [doc-structural-solver, doc-solid-case-file]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, the solid case file and the fan-blade demonstration

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda and a maintainer-answers section), 4 ADRs, all 2026-11-14.

How a user reaches [[rcfd-fem]] without writing Rust: the [[solid-case-file]] run by a library function (F-386), its iterative
solve to tolerance (F-526), the fan-blade demonstration on one blade of `ComputerFan::default()` (F-387) and the library blade
mesher with its proposed case-file keys (F-593).

## Current state

- **The solid case file exists** (F-386, ADR 0438): `rcfd_api::solid_case`, a TOML file with its own schema (`[mesh]` box or Gmsh,
  `[node_sets]`, `[materials]`, `[constraints]`, `[loads]` traction / pressure / nodal_force / gravity, `[analysis]` static or
  modal, `[solver]` direct or cg with block_jacobi, ic0, ic_fill, amg, amg_quadratic, amg_quadratic_smoothed, `[output]`);
  `run_solid_case_file` writes the `.vtu` and a JSON report `rcfd-solid-report/1`. **Partial**: A14 (iterative route to 1e-10 on the
  Tet10 file cantilever) missed for all six preconditioners, a residual floor. F-526 (ADR 0443) met the 1e-10 on the F-381 Hex8 file
  and printed both floors (ADR partial by an amendment after a run; item done).
- **No CLI subcommand and no MCP tool:** ADR 0438 leaves them as "the maintainer's step: each changes a golden" (ADR 0438 Q3,
  recommendation: a separate item once the maintainer approves the golden change). That item is F-405, todo
  ([[roadmap-structural-solver]]).
- **Fan-blade demonstration** (F-387, ADR 0500, done): static response to centrifugal and pressure load and the modes at rest, with
  a user page; numbers on [[structural-solver]] ([[doc-structural-solver]]).
- **Library blade mesher** (F-593, ADR 0505, partial): `rcfd_fem::builders` `fan_blade_tet4`, `fan_blade_tet10`,
  `computer_fan_blade_tet10` with the node sets `all`, `hub`, `tip` and surface sets `hub`, `tip`, `pressure`, `suction`; at the
  F-387 divisions the meshes and results equal the test's construction bit for bit. The case-file half (`[mesh] generator =
  "fan_blade"`, `[loads.<name>] kind = "centrifugal"`) is not built.
- **Maintainer answer of 2026-11-18** (written into ADR 0505 on 2026-11-19): the case-file keys are "Approved under the class rule
  for additive golden edits (agent rules, rule 3, from version 2)"; "The item may be issued. The same keys are proposed in ADR 0500
  (open question 1)".

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0438](../../docs/adr/0438-f386-registration-solid-case-file.md) | 2026-11-14 | registration (hub) | F-386 | A solid case file of its own and the library entry that runs it | partial; A14 missed (residual floor 5.7e-10 to 6.2e-10 against 1e-10) | 0443 |
| [0443](../../docs/adr/0443-f526-registration-iterative-solid-case-and-box-bound.md) | 2026-11-14 | registration | F-526 | Iterative solve of a solid case file to its tolerance, and the box bound | ADR partial (amendment after a run); item done | |
| [0500](../../docs/adr/0500-f387-registration-fan-blade-static-and-modal-demonstration.md) | 2026-11-14 | registration (demonstration) | F-387 | Fan blade: static response to centrifugal and pressure load, modes at rest; user page | done (S0 after amendment 1) | 0505 |
| [0505](../../docs/adr/0505-f593-registration-library-fan-blade-mesher-in-rcfd-fem-builders.md) | 2026-11-14 | registration + maintainer answers | F-593 | Library fan-blade mesher; case-file keys proposed | partial (library half done); keys approved 2026-11-18 | |

## Key numbers and limits

- F-386 gap: "On the Tet10 file cantilever every CG preconditioner stops at a true residual of 5.7e-10 to 6.2e-10 of |b| against
  the registered 1e-10 (a rounding floor)" ([[roadmap-structural-solver]]).
- F-526 (ADR 0443): "six preconditioners reach 1e-10 on the F-381 Hex8 file with agreement within 1.2e-12".
- F-387 (from the user page): first frequency 188.8 -> 186.1 Hz coarse to fine, reactions balance to about 1e-11; E = 2.5 GPa,
  nu = 0.38, 1300 kg/m^3, 2000 rpm and 50 Pa ([[doc-structural-solver]]).
- Proposed `fan_blade` keys (ADR 0505): `divisions` default `[12, 2, 8]`, `n_c >= 2`; `centrifugal` with `axis`, `origin` (default
  `[0, 0, 0]`), `speed` in rad/s, "one unit only, no `rpm` alias".

## Open questions

- ADR 0438 Q3 / F-405: a CLI subcommand and MCP tool for the solid file (golden edits), not yet issued at this snapshot.
- ADR 0505 Q2: mid-edge nodes on the curved blade surface, recommendation (a) keep straight-edge midpoints for now.

> [!warning] Contradiction — are the F-593 case-file keys still waiting for the maintainer?
> The F-593 gap in `docs/roadmap.json` ([[roadmap-structural-solver]]) says the case-file half "needs the user to approve the
> case-file keys proposed in ADR 0505", and [[doc-structural-solver]] says the keys "wait for the maintainer". ADR 0505's answers
> section (maintainer, 2026-11-18, written in 2026-11-19) approves them and says "The item may be issued". Within the repo docs;
> the ADR is newer.

## Related

- [[solid-case-file]], [[structural-solver]], [[rcfd-fem]], [[rcfd-api]], [[roadmap-structural-solver]]
- [[fan-01]], [[adr-thread-structural-linear-solvers]]
