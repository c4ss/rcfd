---
type: source
tags: [interface, structural, case]
sources: []
origin: docs/solid-case-file.md
origin_commit: 045708b9
created: 2026-11-22
updated: 2026-11-25
status: draft
---

# The solid case file

**Origin:** [docs/solid-case-file.md](../../docs/solid-case-file.md) at `origin/main` 09647baa (first read at 2e94e5ef; the section "Reduced parts and superelements" added by F-616)
**Kind:** reference page

## Summary

A TOML file of its own for a linear solid model of [[rcfd-fem]] (F-386, ADR 0438). It is not a flow case: the flow reader (`rcfd run`, `Case::load`) refuses it and the solid reader refuses a flow case. It is read and run by the library function `rcfd_api::solid_case::run_solid_case_file(path, out_dir)`, which writes the `.vtu` and a JSON report. "There is no CLI subcommand or MCP tool for it yet."

## Key points

- Values are SI numbers or unit strings (`"200 GPa"`) checked against the key's dimension; named tables are processed in ascending name order; every problem is reported with its dotted key and line, all problems of a file together ([[located-error]]).
- Tables: `[mesh]` (generator `box` with `hex8`/`tet4`/`tet10`, or a Gmsh `.msh` file, `quadratic` upgrades Tet4 to Tet10), `[node_sets.<name>]` (boxes), `[materials.<name>]` (`youngs_modulus`, `poisson_ratio`, `density` 0, `expansion` 0), `[constraints.<name>]`, `[loads.<name>]` (`traction`, `pressure`, `nodal_force`, `gravity`), `[analysis]` (`static` default or `modal`, `modes` 6), `[solver]` (`direct` default or `cg`), `[output]` (`result.vtu`, `report.json`).
- CG preconditioners: `block_jacobi`, `ic0` (ADR 0282), `ic_fill` (the default, ADR 0357), `amg` (ADR 0305), `amg_quadratic` (ADR 0370), `amg_quadratic_smoothed` (ADR 0375) for Tet10.
- Report format `rcfd-solid-report/1`: displacement, stress (von Mises), reactions per constraint, solver stats; modal: frequencies (Hz).
- Since ADR 0785 (F-616, 09647baa) two more tables, "both off unless given"; "a case without them runs as before, bit for bit". A case with either is run by `run_solid_case_any(path, out_dir)`, "which runs every solid case file"; `run_solid_case_file` refuses such a case and names `run_solid_case_any` ([[adr-thread-structural-case-file-and-fan-blade]]).
  - `[reduction]` makes a **part case**: `interface` (a node set, required), `modes` (fixed-interface modes kept, default 0 = static condensation), `interface_enrichment` (Krylov blocks, default 0), `file` (default `part.cms`). The model is reduced to the interface by the fixed-interface reduction of ADR 0469; `interface_enrichment = p` adds $p$ Krylov blocks of ADR 0532. "Nothing is solved": the run writes the part file and a report with `analysis` = `reduction`. `[analysis]`, `[solver]`, `[superelements.<name>]` and `output.vtu` are refused; the constraints of a part must have the value 0.
  - `[superelements.<name>]` makes a **host case**: `file` (a reduced part file, relative to the case file) and `set` (the node set the part attaches to), both required. Each part is attached as a superelement (ADR 0536): "every boundary node of the part must lie on a node of the set (within 1e-9 of the largest extent of the mesh) and every node of the set must be a boundary node of the part". Parts are attached in ascending name order; the unknowns are the model's, then the generalised coordinates of each part.
  - A host run is `static` (`solver.method` must be `direct`) or `modal`. The report gains `superelements` (per part: `name`, `file`, `set`, `boundary_unknowns`, `generalised_coordinates`, `offset`) and `unknowns`. "Not available in a run with superelements: stresses and reactions, the displacements inside a part, a CG solver, and a harmonic analysis (the case file has none)."
- Example: a Tet10 cantilever from `crates/rcfd-api/tests/solid_cases/cantilever_tet10.toml`.

## Changes at 045708b9 (F-889)

`[reduction]` gains cutoff, residual_vectors and load_patterns; a model without an interior part is refused, and the output carries the reduction's diagnostics (ADR 0797) ([[adr-thread-structural-case-file-and-fan-blade]]).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[solid-case-file]], [[rcfd-fem]], [[rcfd-api]]
