---
type: source
tags: [interface, case]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: initialisation methods, zone sources, user transport equations (wave 15A)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 15A: initialisation methods, zone sources, user transport equations (F-205, F-201, F-202, ADR 0019)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`initial.method` (`uniform`, `potential`, `coarse`), `zone_sources.<name>` for temperature and scalars, and user equation forms for `scalars.<name>`.

## Key points

- `initial.method`: `uniform` (default, bit-identical), `potential` (Laplace potential start), `coarse` (`coarse_factor` 2, `coarse_iterations` 200; generated meshes only). Both apply to steady `simple`/`simplec`. "Not verified: that a potential start reduces the iterations of a recirculating flow (it did not for the backward-facing step, ADR 0019 I2)."
- `zone_sources.<name>`: `target` `temperature` or a scalar, `kind` `source` or `fixed_value`, `value_expression` (dimension checked), `linear_coefficient` treated implicitly. Not delivered: mass/momentum sources, turbulence quantities, variable-density.
- `scalars.<name>.equation`: `transport` (default), `diffusive`, `poisson`, `algebraic`; `diffusivity_expression`, `source_expression`, `initial_expression`, `expression`. Not delivered: vector variants, feeding user variables into properties.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[case-file]], [[bit-identical]]
