---
type: source
tags: [interface, case, physics, rotating]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: momentum sources (sources)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`sources` momentum sources (F-031)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`sources.<name>` adds a momentum source in a cell zone: `porous`, `rotating_frame`, `mrf`, `acceleration`, `actuator_disk`, `lorentz`.

## Key points

- Algorithms: `simple`, `simplec`, `piso`, `pimple` (ADR 0045: evaluated at the start of every outer iteration). `rotating_frame` runs with `pimple` on a static mesh (ADR 0183 P-C) and is refused with `piso`; `mrf` is steady-only.
- `zone`: `all` (default), `box`, `cell_zone` (and `register`, see [[doc-case-schema-registers]]).
- `porous`: Ergun or Darcy-Forchheimer, superficial-velocity form.
- `rotating_frame`: relative-velocity form $-2\,\boldsymbol{\Omega}\times\mathbf{u} - \boldsymbol{\Omega}\times(\boldsymbol{\Omega}\times\mathbf{r})$; `centrifugal = false` keeps Coriolis only.
- `mrf`: absolute-velocity frozen-rotor zone (ADR 0008); `cyclic_seams = true` (ADR 0113) for rotationally cyclic pairs; case-tree mesh `annulus_3d_sector` (ADR 0116). Validation errors: zone boundaries not surfaces of revolution, overlap with another zone, transient algorithms (ADR 0045 D4).
- "Not validated: any pump or turbine benchmark, turbulence models or the energy equation with an MRF zone."
- `actuator_disk` (ADR 0036): total force exactly `thrust` to round-off; `distribution` `smooth` (default) or `cell`.
- `lorentz` (ADR 0039): low magnetic Reynolds number MHD; steady SIMPLE/SIMPLEC only; Hartmann channel check.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[mrf]], [[fan-01]]
