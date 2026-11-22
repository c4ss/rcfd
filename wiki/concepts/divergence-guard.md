---
type: concept
tags: [interface, run, numerics]
sources: [doc-case-schema-transient-divergence-guard, doc-case-schema-steady-divergence-guard, doc-case-schema-pressure-lag-check, doc-run-control, doc-case-schema-linear-upwind-limiter]
created: 2026-11-22
builds_on: [stability-and-cfl]
updated: 2026-11-22
status: draft
---

# Divergence guard

Read-only checks that stop a run which is blowing up, with state `diverged`, exit code 6 and a located record of where ([[doc-run-control]]).

## How rcfd uses it

- Transient (`piso`, `pimple`; ADR 0067): `solver.divergence_courant_limit`, 0 = built-in 1e6; stops at the first non-finite velocity, flux or pressure, or a Courant number over the limit ([[doc-case-schema-transient-divergence-guard]]).
- Steady (`simple`, `simplec`, `coupled`; ADR 0077): `solver.steady_divergence_guard` `on` (default) checks continuity growth over 1e6 times its recent minimum, cell speed over 1e6 `U_ref`, pressure range over 1e12 `P_ref`; non-finite values are checked also with `off` ([[doc-case-schema-steady-divergence-guard]]).
- The run-control driver ends the run; a simulation stepped directly (GUI, library) keeps iterating and exposes the record ([[doc-case-schema-steady-divergence-guard]]).
- The record goes into the `finish` event as `divergence` (kind, quantity, cell, position, value, limit, `last_good`) ([[doc-run-control]]).
- `solver.pressure_lag_check = "raise"` also ends a PISO/PIMPLE run as `diverged` ([[doc-case-schema-pressure-lag-check]]).
- The guards only read the state, so results are bit-identical with and without them ([[doc-case-schema-transient-divergence-guard]], [[doc-case-schema-steady-divergence-guard]]). The limiter test forced `barth_jespersen` on in 542 steady runs and counted guard trips ([[doc-case-schema-linear-upwind-limiter]]).

## Builds on

- [[stability-and-cfl]]: the Courant number the transient guard checks.
- Also relevant: [[slope-limiters-unstructured]] (the `barth_jespersen` test).

## Related

- [[exit-codes]], [[status-stream]], [[bit-identical]]
