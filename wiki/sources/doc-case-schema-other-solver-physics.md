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

# Case schema: other solver and physics settings

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Other solver and physics settings", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Assorted `solver.*` and `physics.*` keys: body force, bulk velocity, H operator, component diagonal, reference pressure, the fractional-step advance, adaptive corrector control, `min_iterations`, and the `heat_flow` report conductivity.

## Key points

- `physics.body_force`: steady incompressible algorithms only. `solver.bulk_velocity`: steady algorithms only.
- `solver.component_diagonal` `averaged` (default) or `corrected` (ADR 0010, opt-in): wall-resolved cells next to a symmetry plane need it; `cases/packed_bed` "converges in 16 instead of stalling at 1700 iterations".
- `solver.time_advance = fractional_step` (F-210, ADR 0037): one pressure Poisson solve per step; not restartable; the default `iterative` leaves every existing run bit-identical.
- `solver.corrector_control = adaptive` (ADR 0156): at least `n_correctors`, at most 10, chosen so the lag amplification is at most 0.5; reported in `iteration`/`finish` events; the key at its default does not enter the case hash.
- `solver.min_iterations` delays only the `convergence.<name>` conditions; it "does not hold a run to a count" (see [[exit-codes]]).
- `heat_flow` report `conductivity`: 0 = unset, uses `physics.conductivity`; before the wave 15A fix it defaulted to 1.0 and silently replaced it (ADR 0019). A positive value with the variable-density solver is a located error (ADR 0021 item W).
- `energy.enabled` works with transient `piso`/`pimple` (F-050), first-order coupling in time.

## Contradictions with the wiki

> [!warning] Contradiction (within the repo doc)
> This section says "Temperature-dependent `properties`, scalars and species stay steady-only", while [[doc-case-schema-scalars-species-properties]] says scalars and species also run in transient `piso`/`pimple` since wave 15A.

## Pages this source informs

- [[report]], [[exit-codes]], [[bit-identical]], [[status-stream]]
