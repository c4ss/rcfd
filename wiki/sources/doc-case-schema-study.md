---
type: source
tags: [interface, case, study]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: study

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 20C: `study` (F-095, ADR 0034)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

The `study` section holds a parametric study in the settings tree; `rcfd study <case>` runs it ([[doc-studies]]). An empty `study.parameters` means no study.

## Key points

- `study.outputs` (`[]` = all reports, sorted), `study.combine` `grid` (default) or `zip`, `study.parallel` 1, `study.write_results` true.
- `study.parameters.<name>.path` (any path `--set` accepts), `kind` `list`/`linspace`/`logspace`, `values`/`strings`/`json`, `start`/`stop`/`count`.
- Static errors are located, exit 3, nothing runs.
- `Case::set` now creates named entries nested deeper than one level (`--set study.parameters.k.path=...`).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[parametric-study]]
