---
type: source
tags: [interface, study, cli]
sources: []
origin: docs/studies.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Parametric studies

**Origin:** [docs/studies.md](../../docs/studies.md) at `origin/main` 2e94e5ef (F-095, ADR 0034)
**Kind:** reference page

## Summary

`rcfd study <case>` runs one simulation per design point of the case's `study` section and writes a result table. Covers sweeps, ordering, flags, the table and its columns, failure isolation, resume and exit codes.

## Key points

- Flags: `--out` (default `study`), `--parallel n`, `--resume`, `--max-new-points n`, `--no-fields`, `--list`, `--json`, `--set`.
- A design point is the base case plus overrides "applied with the same mechanism as `rcfd run --set`, so a point is bit-identical to `rcfd run --set ...` of the same values (tested)".
- Order: parameters sorted by name; `grid` row-major with the first name slowest; `zip` walks together; the point index is the only ordering key.
- Outputs: `table.csv`, `table.json` (written atomically after every point), `points/pNNNN/`. The table has no wall-clock times or thread numbers: "byte-identical for any `--parallel` (tested, 1 against 3)".
- Columns: `point`, the parameters, `status` (`ok` only for exit 0), `exit_code`, `state`, `iterations`, the reports, `message`; JSON rows add the point `fingerprint` (FNV-1a of the resolved settings without `study`).
- Failure isolation: a failing point gives a failed row and the others run; exit = the code of the lowest-index failed point (1, 3, 4, 5, 6), 0 when all ok, 3 for static sweep errors, 2 for usage.
- `--resume` keeps matching `ok` rows and retries failed points; a resumed study ends byte-identical to a fresh one (tested).
- Not delivered (F-095 stays `partial`): service and MCP tools for studies, named-expression parameters, chained studies, adaptive exploration, resuming a half-run point, a distributed runner, monitor columns.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[parametric-study]], [[exit-codes]], [[rcfd-python]]
