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

# Case schema: pressure interpolation of the steady kernel

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Pressure interpolation of the steady kernel (`solver.pressure_interpolation`)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`solver.pressure_interpolation` chooses the face pressures summed into Green-Gauss gradients of the steady `simple`/`simplec` kernel (ADR 0193): `linear` (default), `skew_corrected`, `body_force_weighted`.

## Key points

- `body_force_weighted` makes a fluid at rest under a linear conservative body force an exact fixed point on parallelepiped meshes: spurious speed 1.6e-2 to 3.7e-12 (SIMPLEC, `face_flux`) on a 38-degree skewed mesh.
- When the body force is not balanced, `body_force_weighted` raised the velocity error by up to 34 percent and the pressure error up to 4.4 times; `skew_corrected` changed them by at most 0.5 and 3 percent.
- Located errors outside the steady incompressible constant-density kernel, without a body force, or with `rotating_frame`, `acceleration`, `actuator_disk`, `lorentz` sources.
- With `linear`: bit for bit, left out of the settings hash.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[bit-identical]]
