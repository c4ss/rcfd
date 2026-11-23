---
type: source
tags: [interface, case, multiphase]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: multiphase (VOF)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 27A: `multiphase` (F-055, ADR 0049)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`multiphase.enabled = true` selects the volume-of-fluid free-surface solver (`rcfd_fv::vof`) for two incompressible immiscible fluids, with interface compression, CSF surface tension and well-balanced `p_rgh` gravity.

## Key points

- Required: `density_1/2`, `viscosity_1/2` (`-1` unset), `initial.shape` (`box`, `sphere`, `half_space`).
- Defaults: `compression` 1, `alpha_courant` 0.25, `curvature_smoothing` 2.
- Defaults since wave 44D (ADR 0114, "user decision 2026-10-24"): `curvature = "height_function"`, `viscous_stress = "full"`, `component_diagonal = "corrected"`; results of cases without these keys "therefore differ from runs before ADR 0114".
- `piso`/`pimple` only with `euler`, fixed `solver.dt`; refused with turbulence, energy, scalars, sources, dynamic mesh, particles, periodic meshes and more. Single-density reports (force, moment, y_plus, heat_flow, mass_flow, ...) are refused.
- Pressure is in Pa; fields `p`, `p_rgh`, `alpha`, `rho`. Pressure solve: CG with smoothed-aggregation AMG above 2000 cells.

## Contradictions with the wiki

_None found at ingest._

> [!note] Exception to the bit-identical default
> The VOF defaults were flipped by ADR 0114 on a user decision, so pre-ADR-0114 VOF results are not reproduced by a case that does not write the keys (see [[bit-identical]]). The doc records this as a decision, not a defect.

## Pages this source informs

- [[bit-identical]], [[report]]
