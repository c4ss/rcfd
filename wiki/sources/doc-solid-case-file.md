---
type: source
tags: [interface, structural, case]
sources: []
origin: docs/solid-case-file.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# The solid case file

**Origin:** [docs/solid-case-file.md](../../docs/solid-case-file.md) at `origin/main` 2e94e5ef
**Kind:** reference page

## Summary

A TOML file of its own for a linear solid model of [[rcfd-fem]] (F-386, ADR 0438). It is not a flow case: the flow reader (`rcfd run`, `Case::load`) refuses it and the solid reader refuses a flow case. It is read and run by the library function `rcfd_api::solid_case::run_solid_case_file(path, out_dir)`, which writes the `.vtu` and a JSON report. "There is no CLI subcommand or MCP tool for it yet."

## Key points

- Values are SI numbers or unit strings (`"200 GPa"`) checked against the key's dimension; named tables are processed in ascending name order; every problem is reported with its dotted key and line, all problems of a file together ([[located-error]]).
- Tables: `[mesh]` (generator `box` with `hex8`/`tet4`/`tet10`, or a Gmsh `.msh` file, `quadratic` upgrades Tet4 to Tet10), `[node_sets.<name>]` (boxes), `[materials.<name>]` (`youngs_modulus`, `poisson_ratio`, `density` 0, `expansion` 0), `[constraints.<name>]`, `[loads.<name>]` (`traction`, `pressure`, `nodal_force`, `gravity`), `[analysis]` (`static` default or `modal`, `modes` 6), `[solver]` (`direct` default or `cg`), `[output]` (`result.vtu`, `report.json`).
- CG preconditioners: `block_jacobi`, `ic0` (ADR 0282), `ic_fill` (the default, ADR 0357), `amg` (ADR 0305), `amg_quadratic` (ADR 0370), `amg_quadratic_smoothed` (ADR 0375) for Tet10.
- Report format `rcfd-solid-report/1`: displacement, stress (von Mises), reactions per constraint, solver stats; modal: frequencies (Hz).
- Example: a Tet10 cantilever from `crates/rcfd-api/tests/solid_cases/cantilever_tet10.toml`.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[solid-case-file]], [[rcfd-fem]], [[rcfd-api]]
