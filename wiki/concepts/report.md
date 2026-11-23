---
type: concept
tags: [case]
sources: [doc-glossary, doc-case-schema-turbulence, doc-case-schema-other-solver-physics, doc-run-control, doc-studies, doc-tutorial-mcp-design-loop]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Report

A number a run computes as it goes, named in the case's `[reports]` table, e.g. a force on a patch (`type = "force"`). The engine gives no unit for report values ([[doc-glossary]]).

## How rcfd uses it

_None yet._

- Report types named in the schema include `force`, `force_coefficient`, `moment`, `heat_flow`, `mass_flow`, `y_plus`, `area_average`, `expression`, `probe`, `scalar_flux`, `scalar_average`, and (wave 26C, ADR 0048) `volume_std`, `area_std`, `area_min`, `area_max`, `mass_weighted_average`, `vector_flux` ([[doc-case-schema-turbulence]], [[doc-case-schema-other-solver-physics]], [[doc-tutorial-mcp-design-loop]]).
- Force and moment `component`: `total` (default) and `viscous` use the face viscosity the momentum equation used, wall-function value included (ADR 0166 forces, ADR 0170 moments); `total_molecular` / `viscous_molecular` give the earlier molecular values. "The FAN-01 torques of ADRs 0075 to 0136 are such molecular moments" ([[doc-case-schema-turbulence]]).
- `heat_flow` `conductivity` 0 = unset (uses `physics.conductivity`); before the ADR 0019 fix it defaulted to 1.0 and silently replaced it ([[doc-case-schema-other-solver-physics]]).
- Report values are written to `monitors.json` / `monitors.csv` and, with `--status-monitors`, into each `iteration` event ([[doc-run-control]]); they become the columns of a [[parametric-study]] table ([[doc-studies]]) and the metrics the design tools optimise ([[doc-tutorial-mcp-design-loop]]).

## Related

- [[case-file]]
- [[run]]

## Open questions

- Report values carry no unit while the Studio guardrail G17 asks every number to show its unit ([[doc-product]]); how the Studio reconciles this is not yet ingested.
