---
type: concept
tags: [interface, structural, case]
sources: [doc-solid-case-file, doc-case-schema-reactor-fwh-files, adr-thread-structural-case-file-and-fan-blade]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Solid case file

A TOML file for a linear solid model of [[rcfd-fem]], separate from the flow [[case-file]]: the flow reader refuses it and the solid reader refuses a flow case (F-386, ADR 0438) ([[doc-solid-case-file]]).

## How rcfd uses it

- Run by the library function `rcfd_api::solid_case::run_solid_case_file(path, out_dir)`; "There is no CLI subcommand or MCP tool for it yet" ([[doc-solid-case-file]]).
- Static or modal analysis; direct or CG solver; outputs a `.vtu` and a `rcfd-solid-report/1` JSON report ([[doc-solid-case-file]]).
- One of several input formats besides the flow case: reactor cases and FW-H surface files also have their own JSON documents ([[doc-case-schema-reactor-fwh-files]]).

## Related

- [[rcfd-fem]], [[case-file]], [[located-error]]

## From the ADR threads

- ADR 0438 (F-386) gives the sections `[mesh]` (box hex8 / tet4 / tet10 or Gmsh `.msh`), `[node_sets.<name>]` (inclusive box), `[materials]`, `[constraints]`, `[loads]` (traction, pressure, nodal_force, gravity), `[analysis]` static or modal, `[solver]` direct or cg (block_jacobi, ic0, ic_fill, amg, amg_quadratic, amg_quadratic_smoothed) and `[output]`. A14 missed: the iterative route on the Tet10 file cantilever stops at a residual floor of 5.7e-10 to 6.2e-10 against 1e-10 ([[adr-thread-structural-case-file-and-fan-blade]]).
- Keys `[mesh] generator = "fan_blade"` and `[loads.<name>] kind = "centrifugal"` (axis, origin, speed in rad/s) were proposed in ADR 0505 and approved on 2026-11-18. They are not built at the snapshot (F-593 partial) ([[adr-thread-structural-case-file-and-fan-blade]]).
- A `[fatigue]` section (F-605) was approved on 2026-11-18 (ADR 0516 answers). `[contact]` (F-427), `step = energy_momentum` (F-570), `formulation = bbar` (F-587) and `[structural] geometric_terms = full` (F-558) are open maintainer questions ([[adr-thread-structural-fatigue-and-radiated-sound]], [[adr-thread-structural-materials-nonlinear]], [[adr-thread-structural-contact]], [[adr-thread-structural-beams-shells]]).
