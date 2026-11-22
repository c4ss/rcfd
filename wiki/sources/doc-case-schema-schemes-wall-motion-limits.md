---
type: source
tags: [interface, case, numerics]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: schemes, wall motion, solution limits (wave 13)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 13: schemes, wall motion, solution limits", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Wave 13 (F-211, F-204, F-208, ADR 0016): convection schemes, the `bdf2_bounded` time scheme, wall velocity kinds, solution limits and the wiring of the wall options.

## Key points

- `solver.convection`: `blended` (`blend_factor` 0.75), `high_resolution` (Gamma, beta 0.2), `compressive` (k 1.5), `bounded_central`. The limited schemes are deferred-correction and the compressive members "can stall in a limit cycle".
- `bdf2_bounded`: "project-defined, no verified source, ADR 0016 D1".
- Wall kinds `moving_wall`, `shear`, `partial_slip`, `marangoni`: a turbulence model with one is a located error, except `moving_wall` with `k_omega_sst`. `wall_velocity_projection = "tangential"` for walls of revolution (ADR 0072: up to 1 % of the wall speed carried through the C-grid fan shroud facets).
- `limits.temperature` / `limits.scalar` clip; the count is `clipped_cells` in the run status.
- Wall options of ADR 0015 are applied by the build path and "no longer rejected".

## Contradictions with the wiki

See [[doc-case-schema-caveats]].

## Pages this source informs

- [[case-file]]
