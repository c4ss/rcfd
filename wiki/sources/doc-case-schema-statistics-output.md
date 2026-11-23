---
type: source
tags: [interface, case, output]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: statistics and output policy

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`statistics` and `output` policy (F-116)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section (a pointer to run control)

## Summary

`statistics.*` collects the time-weighted mean, rms, std, min and max of transient incompressible runs as `<name>_mean` etc.; `output.*` controls snapshots, checkpoint retention, written fields and the field store. Details are in [[doc-run-control]].

## Key points

- `output.store_interval`, `store_time_interval`, `store_fields` write the field store `<output directory>/fields/` (ADR 0124; off by default); `output.store_precision` `"f64"` (default) or `"f32"` (lossy, ADR 0133).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[field-store]], [[doc-run-control]]
