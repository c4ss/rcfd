---
type: source
tags: [interface, case, studio]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: labels, help and the advanced flag

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Labels, help and the advanced flag (`rcfd schema --ui`)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`rcfd schema --ui`, and `settings.schema {ui: true}` on a service started with `rcfd serve --ui`, add labels and help to the schema (ADR 0730) from `crates/rcfd-api/src/schema_ui.toml`, one entry per dotted path.

## Key points

- A node may carry a short label, one line of help, a display unit, the docs heading that explains it, and an advanced flag (one step away in the Studio, behind "Show all").
- "A usual range is given only with its published source, as advice; the allowed range is still the key's `minimum` and `maximum`."
- Missing help falls back to the first sentence of the docs heading, else the key's description.
- "Validation, parsing, unit conversion and the case hash never read these keys, and without `--ui` the schema is unchanged."

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[studio]], [[rcfd-service-protocol]]
