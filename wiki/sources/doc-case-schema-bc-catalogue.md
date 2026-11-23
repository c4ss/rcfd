---
type: source
tags: [interface, case, boundary]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: boundary-condition catalogue bc_kind (wave 16B)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 16B: boundary-condition catalogue `bc_kind` (F-200, ADR 0022)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`boundaries.<patch>.bc_kind` selects a kind that owns the velocity and pressure conditions, for the constant-density segregated algorithms only; plus the convergence controls of ADR 0023.

## Key points

- Kinds: `mass_flow`, `outflow`, `pressure_outlet`, `opening`, `vent`, `total_pressure_inlet`, `radial_equilibrium_outlet`, `target_mass_flow_outlet`. Default `none` reads the entry as before.
- Pressures of the new kinds are in Pa (`bc_pressure`); the older `pressure_value` stays kinematic.
- Signs: `mass_flow_value` positive into the domain, `target_mass_flow` positive out, the `mass_flow` report positive out.
- `radial_frame_rate`: in a `rotating_frame` case set it to the frame's angular velocity (47 % pressure error on the X1 check otherwise).
- `pressure_update = "secant"` "is a known miss"; use `bernoulli` or `guarded_secant` (ADR 0055 D5: 4e-16 relative error in 1880 iterations).
- Convergence controls: `solver.residual_norm` `reference`/`backward`/`scaled`; `solver.pseudo_control` `off`/`ser`/`ser2` "are NOT validated (known misses, ADR 0023 addendum A8); fixed `pseudo_cfl` is the reliable setting".

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[known-miss]], [[case-file]]
