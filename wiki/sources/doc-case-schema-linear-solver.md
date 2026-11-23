---
type: source
tags: [interface, case, gpu]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: linear solver device

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 22: `linear_solver` (F-101, ADR 0038)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`linear_solver` selects where the segregated pressure solve (`simple`, `piso`, `pimple`) runs.

## Key points

- `device`: `cpu` (default, bit-identical), `cuda` (preconditioned CG on an NVIDIA device; a missing device is a located error), `auto` (device for 100000 unknowns or more when usable).
- `preconditioner` `jacobi` (default) or `chebyshev` (`chebyshev_degree` 4, range 0..32).
- `cuda`/`auto` with `coupled`, compressible/energy or non-constant density is an error; without the cargo feature `cuda`, `auto` falls back to the CPU.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-gpu]]
