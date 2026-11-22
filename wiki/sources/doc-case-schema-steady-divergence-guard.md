---
type: source
tags: [interface, case, run]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: divergence guard of the steady solvers

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Divergence guard of the steady solvers (`solver.steady_divergence_guard`)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`solver.steady_divergence_guard` (`on` default, `off`) for `simple`, `simplec` and `coupled` (ADR 0077).

## Key points

- Always (also with `off`): any non-finite residual, continuity, cell velocity, pressure or face flux.
- With `on`, from the second iteration: continuity over 1e6 times the larger of its smallest value over the previous 50 iterations and 1e-6 `U_ref`; largest cell speed over 1e6 `U_ref`; pressure range over 1e12 `P_ref`.
- Trip kinds `non_finite`, `continuity_growth`, `velocity_runaway`, `pressure_runaway`; `last_good` is the iteration with the smallest continuity before the trip.
- The run-control driver ends the run with state `diverged`, exit 6, the record in the `finish` event (`divergence`). A simulation stepped directly (GUI, library) keeps iterating and exposes `Simulation::steady_divergence`. Results bit-identical with `on`/`off`.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[divergence-guard]], [[status-stream]], [[exit-codes]]
