---
type: source
tags: [interface, case]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: registers and initial patches

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`registers` and `initial.patches` (F-124, ADR 0011)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`registers.<name>` defines a named set of cells; `initial.patches.<n>` overwrites the initial field inside one. Semantics are in [[doc-interchange-formats]].

## Key points

- `type`: `box` (default), `expression`, `field` (ADR 0095, initial `velocity_magnitude`/`ux`/.../`temperature`, evaluated once), `boundary_adjacent`, `cell_zone`, `combine`; then `grow`/`shrink` with `neighbours` `faces` or `points`.
- `sources.<n>.zone = "register"` restricts a source.
- `initial.patches.<n>`: `register`, `variable` (`velocity`, `pressure`, `temperature`, `k`, `epsilon`, `omega`, `nutilde`), `value`.
- Example errors: `sources.bed.register: unknown register 'x' (registers: core)`.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[located-error]], [[interchange-formats]]
