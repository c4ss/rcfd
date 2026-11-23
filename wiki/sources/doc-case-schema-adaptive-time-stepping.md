---
type: source
tags: [interface, case, time]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: adaptive time stepping

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Adaptive time stepping (F-209, ADR 0050 Part 2)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

The time-step controller of ADR 0050 for transient incompressible `piso`/`pimple` runs. Every key is optional; with all keys unset the run is the fixed-`solver.dt` run bit for bit.

## Key points

- Keys: `max_courant`, `max_courant_rms`, `dt_safety` (0 = 0.9), `dt_growth` (1.2), `dt_shrink` (0.5), `min_dt`/`max_dt`, `error_tolerance` (step doubling; implicit Euler, laminar constant density only), `period` + `steps_per_period`, `extrapolate_initial`, `output.write_times`, `output.write_time_interval`.
- Located errors: inconsistent values, a write time beyond `end_time`, error control with energy/turbulence/etc., any key on a steady run. Restart files of such runs carry the controller state.
- Not wired: Aitken relaxation, local timescale factors, residual smoothing, the compressible solver.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[checkpoint]], [[bit-identical]]
