---
type: source
tags: [interface, case, particles]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: particles

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`particles`", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Lagrangian particle tracking (F-056, ADR 0028): one-way by default, two-way coupling (F-128, ADR 0059), wall film (F-253, ADR 0192), dispersion from the turbulence model (F-255, ADR 0208), erosion and rough walls (F-254, ADR 0194/0197), droplet evaporation and TAB breakup (F-326, ADR 0348), and the run-path outputs (ADR 0037).

## Key points

- `particles.enabled = false` by default; numeric keys use the sentinel `-1` with a located error when needed.
- Defaults: `drag = schiller_naumann`, `integration = analytic`, `interpolation = gradient`, `wall_interaction = reflect`, `seed` 1. Injection kinds `point`/`surface`; distributions `constant`, `rosin_rammler`, `uniform`; `dispersion` `none` or `drw`.
- Optional tables (`film`, `dispersion_from_model`, `erosion`, `rough_wall`, `evaporation`, `breakup`) are absent from the case data when not written and change nothing; some keys are left out of the settings hash at `false`.
- Erosion preset `dnv_o501_2007_steel` (`k = 2.0e-9`, `n = 2.6`, `density = 7800.0`) "come from a third-party copy of DNV RP O501 Rev 4.2-2007 that has not been checked against the maintainer's edition; a preset gives an indicative, not certified, erosion rate".
- Run path: the tracker is built by `Simulation::new` (bad patch/point = located error, `--check-only` exit 3); transient runs track every step; steady runs track the final field once. Outputs `particles.vtp`, `particle_report.json`, optional CSVs.
- "A transient run with particles is not restartable"; the flow fields are bit-identical with particles on or off (ADR 0037 W3).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-particles]], [[checkpoint]], [[unverified]]
