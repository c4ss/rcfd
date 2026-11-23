---
type: source
tags: [interface, case, turbulence]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: regions

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 21B: `regions` (F-216, ADR 0036)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`regions.<name>` assigns a model to a cell zone: `turbulence = "model"` (default) or `"laminar"`.

## Key points

- A laminar region sets the momentum face viscosity to `physics.nu` on faces touching it; turbulence transport is still solved there.
- Errors: no turbulence model, Reynolds stress or plugin model, rheology, a temperature-dependent viscosity, a zone without cells.
- Without any `regions` entry recorded baseline hashes are bit-identical.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[bit-identical]]
