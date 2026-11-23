---
type: source
tags: [interface, case, rotating]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: mixing plane

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`mixing_plane`", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

A stage interface between a frozen-rotor zone and a stationary zone in the steady loop (F-126, ADR 0055, ADR 0059), joined by two boundary patches (typically the `annulus_3d_stage` generator). Off by default and bit-identical when off.

## Key points

- Circumferential averaging in radial bands (mass, angular momentum, normal momentum flux conserved), passed downstream as a fixed absolute velocity; band pressure passed back. Under-relaxed (`relax` 0.5), every `every` iterations.
- `bands` must be set and coincide with radial cell layers; `tolerance` 1e-6 on the monitor `res_mixing`.
- Only `simple`/`simplec`; not with `solver.frozen_flow`.
- Not delivered: transient and coupled solvers, compressible mixed-out state, rothalpy, sector periodicity, turbulence or scalar transfer through the plane.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[mrf]], [[fan-01]]
