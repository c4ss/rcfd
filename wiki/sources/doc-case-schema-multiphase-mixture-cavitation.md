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

# Case schema: multiphase mixture and cavitation (wave 29B)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 29B: `multiphase.mixture` and `multiphase.cavitation` (F-127, ADR 0056)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

An algebraic-slip (drift-flux) mixture model and Schnerr-Sauer cavitation on the VOF path; both inert unless enabled.

## Key points

- Mixture: "It is NOT a multi-fluid (per-phase momentum) Euler-Euler model"; gravity-driven slip only; Stokes or Schiller-Naumann drag; zero slip reduces to plain VOF bit for bit.
- Cavitation: Schnerr-Sauer with `saturation_pressure`, `bubble_number_density`, `nucleus_radius` required; the source is coupled implicitly to the pressure (at most 60 repetitions per corrector, to 1e-10 of a vapour fraction).
- Limitations: isothermal, no vapour compressibility, no non-condensable gas, no turbulence correction; "the flow Courant number must stay below about 1 through a cavity collapse".
- Mixture with thermal `mass_transfer` or with `cavitation` are refused as "untested combinations".

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[case-file]]
