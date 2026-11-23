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

# Case schema: divergence guard of the transient solvers

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Divergence guard of the transient solvers (`solver.divergence_courant_limit`)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`solver.divergence_courant_limit` (float >= 0, default 0) for `piso` and `pimple` (ADR 0067).

## Key points

- The run stops at the first step whose velocity, predicted flux, pressure or flux is not finite, or whose Courant number is not finite or exceeds the limit. 0 means the built-in limit 1e6; `1e300` disables the Courant criterion in practice; a non-finite state is always fatal.
- The run ends with state `diverged`, exit code 6 and a located message naming quantity, step, outer iteration, stage and cell.
- The checks only read the state, so results are bit-identical to runs without the key.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[divergence-guard]], [[exit-codes]]
