---
type: source
tags: [interface, case, compressible]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: compressible density-based solver

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`compressible` and `solver.algorithm = "compressible"`", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

The explicit density-based Euler solver (no viscosity, no heat conduction) selected by `solver.algorithm = "compressible"`.

## Key points

- `flux`: `rusanov`, `roe`, `hllc`, `ausm_plus`; `reconstruction`: `first_order` or a TVD limiter; `rk_order` 1 to 3; `cfl` 0.01 to 1, default 0.4; `mode` `transient` (to `solver.end_time`) or `steady`.
- `compressible.initial`: `uniform` or `riemann` (split along `split_axis` at `split_position`).
- `compressible_bc`: `auto`, `slip`, `extrapolate`, `fixed`, and the characteristic conditions of ADR 0006: `total_inlet`, `pressure_outlet`, `nonreflecting` (`reflection_value` default 0.02), `farfield` ("No vortex correction for lifting bodies").
- Refused with key and line: `rk_order = 1` with a limited reconstruction, turbulence, `energy.enabled`, `solver.pseudo_mode`, periodic meshes, velocity expressions, force/moment/force-coefficient/heat-flow/y+ reports, and more.
- `res_u` is the largest relative density change per step; in a transient run these "do not tend to zero".

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[case-file]], [[located-error]]
