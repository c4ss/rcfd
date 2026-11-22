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

# Case schema: tables and profile inputs

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`tables` and profile inputs (F-105)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`tables.<name>` reads a CSV lookup table or profile (a `file` relative to the case, or inline `data`), callable from expressions and usable as a boundary `profile`.

## Key points

- The header names the columns; a unit in brackets is converted to SI; comma, semicolon or tab; `#` comments.
- `inputs` 1 to 3; `extrapolation` `error` (default) or `clamp`. Linear / multilinear on a rectilinear grid, otherwise inverse-distance weighting (power 2, nearest 6 or 8 points).
- Callable as `<name>(args)` or `<name>_<column>(args)`, dimension-checked.
- Under the service sandbox (`--root`) table files must lie inside the root.
- Not implemented: Fluent profile-file syntax, cubic/monotone interpolation, values outside the range during a transient run (NaN; only the t = 0 state is checked). Example `cases/channel/profile_inlet.toml`.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[case-file]]
