---
type: source
tags: [interface, case, udf]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: user functions (udf)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`udf` user functions (F-122)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section (a pointer to the ABI page)

## Summary

`udf.<name>` loads a sandboxed WebAssembly module (`file`, `fuel_per_call`) that may import nothing; role flags select what it supplies. Full contract in [[doc-udf-abi]].

## Key points

- Roles: `momentum_source`, `viscosity`, `monitor`; hook catalogue v2 (F-239) adds `conductivity`, `energy_source`, `initialise`, `at_end` (monitor `udf_end_<name>`).
- Boundary hooks: `velocity_udf`, `temperature_udf` (with `temperature = "fixed"`), `heat_flux_udf` (with `"heat_flux"`); `scalars.<name>.source_udf`.
- "Source and property roles act in steady SIMPLE/SIMPLEC runs only."

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[udf-sandbox]]
