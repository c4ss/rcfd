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

# Case schema: multiphase mass transfer, waves, reference point (wave 28A)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 28A: `multiphase.mass_transfer`, `multiphase.waves`, `multiphase.reference_point` (F-226, F-227, ADR 0052)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Thermal mass transfer (Lee, Hertz-Knudsen) with a prescribed temperature field, a wave inlet with a relaxation-zone beach, and the reference point of the gravity potential.

## Key points

- `mass_transfer.model` `none`/`lee`/`hertz_knudsen`; needs a `pressure = "fixed"` boundary for the volume to leave.
- LIMITATIONS (as the doc capitalises them): "There is NO energy equation in the multiphase path"; the temperature is prescribed; no latent-heat feedback; no Stefan problem.
- Waves: 2-D tank, +x propagation, gravity along -y; Airy or second-order Stokes; `beach = "still_water"` relaxation zone; the added volume is reported as `relax_volume`.
- Not delivered: irregular waves, a wave-making piston, 3-D tanks.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[case-file]]
