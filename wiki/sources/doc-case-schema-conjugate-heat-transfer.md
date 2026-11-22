---
type: source
tags: [interface, case, heat]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: conjugate heat transfer (wave 17B)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 17B: conjugate heat transfer (F-051, ADR 0024)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Solid cell zones, thermal contact resistance, electric conduction with Joule heat, enthalpy-method phase change, prescribed (frozen) flow, and the convection / thin-wall / radiation-to-ambient wall condition. Every new key has an explicit unset sentinel and the whole wave is additive (bit-identical without the keys).

## Key points

- `solids.<name>`: `zone`, `conductivity`, `density`, `specific_heat` (all `-1` = error when unset). Solids need `energy.enabled` and either `solver.frozen_flow = true` or `solver.algorithm = "coupled"` (ADR 0029).
- `thermal_interfaces.<name>`: `solid_a`, `solid_b`, `resistance` (m^2 K/W); wave 21A adds layers and `connection = "open"`. Not delivered (F-215 partial): layer capacity, in-plane shell conduction.
- `electric` (F-213, ADR 0035 D2): `div(sigma grad phi) = 0` with Joule heat; frozen flow only; at least one `potential` patch. Not delivered: Joule heating with a solved flow, magnetic induction.
- `phase_change` (F-212): frozen flow with zero flux, `piso`/`pimple`, `euler`; `momentum_constant` reserved (an error if set). Not delivered: melt flow, natural convection.
- `solver.frozen_flow`: momentum and pressure not solved; flux from `initial.velocity` at the face centres.
- `temperature = "convection"`: `heat_transfer_coefficient`, `free_stream_temperature`, thin-wall lists, `wall_emissivity`, `radiation_temperature`; solved by Newton.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[bit-identical]], [[case-file]]
