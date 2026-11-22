---
type: source
tags: [interface, case, numerics]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: pressure-lag check of PIMPLE

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Pressure-lag check of PIMPLE (`solver.pressure_lag_check`)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`solver.pressure_lag_check` (`warn` default, `raise`, `off`) for `piso`/`pimple` with `solver.n_correctors = 1` (ADR 0073 B4, ADR 0077): one estimate per run of the per-step amplification of the lagged pressure error.

## Key points

- Costs "about as much as 15 steps, once per run" (48 x 48 PISO cavity).
- `warn`: an amplification of at least 1 gives a `warning` event (`kind = "pressure_lag"`); `raise`: state `diverged`, exit 6.
- With more correctors no estimate is made (in two-corrector runs the estimate exceeded 1 where nothing grew, ADR 0077 amendment 3).
- A second corrector does not remove the lag of short pressure waves across thin cells (ADR 0214); see [[doc-case-schema-consistent-correctors]].

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[divergence-guard]], [[status-stream]]
