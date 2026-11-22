---
type: source
tags: [interface, case, physics]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: rheology viscosity laws

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`rheology` viscosity laws (F-110)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Non-Newtonian viscosity laws for the momentum diffusion term.

## Key points

- `rheology.model`: `newtonian` (default, keeps `physics.nu`), `power_law`, `carreau`, `carreau_yasuda`, `cross`, `herschel_bulkley`, `bingham`; SI units (dynamic viscosities in Pa s).
- `rheology.shift` `arrhenius`/`wlf`/`table` adds a temperature dependence (needs `energy.enabled`).
- Works in steady SIMPLE/SIMPLEC and transient PISO/PIMPLE; not combined with turbulence models, the compressible solver or a user-function viscosity.
- Not implemented: the transposed-gradient term of a variable viscosity, yield-stress laws with a temperature shift, checkpoints that carry the viscosity history.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[case-file]]
