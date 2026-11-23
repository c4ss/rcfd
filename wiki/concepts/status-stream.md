---
type: concept
tags: [interface, run]
sources: [doc-run-control, doc-case-schema-end-time-landing, doc-case-schema-steady-divergence-guard, doc-case-schema-other-solver-physics, doc-mcp-tools]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Status stream (rcfd-status)

JSON lines a run writes about itself (`--status-json` to stdout, `--status-file` to a file): every line has `format: "rcfd-status"`, `version: 1`, `event`, `seq` and `wall_s` ([[doc-run-control]]).

## How rcfd uses it

- Events: `start` (cells, algorithm, `case_hash`, `info`), `iteration` (`index`, `time`, `res_u`, `res_p`, `continuity`, `done`; `monitors` with `--status-monitors`; `correctors`/`corrector_cap` with adaptive correctors), `checkpoint` (`why`), `snapshot`, `warning`, `check` (only `--check-only`), `finish` (`state`, `exit_code`, `iterations`, `time`, `reason`, `elapsed_s`) ([[doc-run-control]]).
- Chains add `stage_start`, `restart`, `stage_finish`, `chain_finish`; every event carries `stage` ([[doc-run-control]]).
- `--status-every n` (default 10); iterations 1-5 and the last one are always reported ([[doc-run-control]]).
- A diverged run's `finish` has a `divergence` record (solver, kind, quantity, cell, position, value, limit, `last_good`) ([[doc-case-schema-steady-divergence-guard]]).
- Setup-rule warnings carry `rule`, `path`, `category`; pressure-lag warnings `kind = "pressure_lag"`; corrector cap `kind = "corrector_cap"` ([[doc-run-control]], [[doc-case-schema-other-solver-physics]]).
- A run whose summed steps land within 1e-12 of `solver.end_time` reports it exactly in the last `iteration` and `finish` events (ADR 0160) ([[doc-case-schema-end-time-landing]]).
- The MCP tool `read_status_stream` (service `status.read`) summarises a stream file; `run_headless` produces the same stream ([[doc-run-control]], [[doc-mcp-tools]]). The run service copies `monitors` into `residuals.jsonl` ([[doc-run-control]]).

## Related

- [[exit-codes]], [[checkpoint]], [[run]], [[rcfd-runner]]
