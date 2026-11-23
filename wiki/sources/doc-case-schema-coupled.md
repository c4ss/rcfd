---
type: source
tags: [interface, case, solver]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: block-coupled solver

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`solver.algorithm = "coupled"` block-coupled solver (F-032)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

The steady block-coupled solver: `(u, v[, w], p)` in one linear system per Picard iteration, right-preconditioned GMRES with ILU(0) on the momentum blocks and one aggregation-AMG V-cycle on the approximate pressure Schur complement.

## Key points

- `solver.alpha_u` is the implicit momentum relaxation (0.95 to 0.98 work on the cavity); no pressure relaxation; `pressure_rel_tol` / `pressure_max_iter` defaults 1e-3, 400.
- Wave 19A (ADR 0029): open boundaries (`zero_gradient` velocity with `fixed` pressure), body forces, energy, solid zones (solid cells removed from the pressure-velocity system with exactly zero flux at fluid-solid faces).
- Still located errors: turbulence models, rheology, user functions, momentum sources, lumped components, radiation, property laws, `bc_kind`, zone sources, `bulk_velocity`, non-orthogonal correctors, periodic patches, scalars together with solids.
- Known limits: steady only; least-squares gradient next to a solid sees the solid value; a decaying odd-even pressure mode next to a prescribed-flux inlet (ADR 0029 addendum A3).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-linalg]]
