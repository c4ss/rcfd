---
type: source
tags: [interface, case, cli, post]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: decomposition, postprocessing, rcfd partition / sample / inflate (wave 16W)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 16W: `decomposition` and `postprocessing`, `rcfd partition`, `rcfd sample`, `rcfd inflate` (F-236, F-072, F-108, ADR 0021)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Sections and commands that surface existing library features. "The solver does not use the partitions or the reordering: they are reports and a VTK cell field."

## Key points

- `decomposition.method`: `none`, `bisection`, `inertial`, `sectors`, `graph`; `rcfd partition ... --json` prints imbalance, edge cut, ghost cells; invalid settings exit 3; service method `mesh.partition`.
- `postprocessing.derived`: `vorticity`, `q_criterion`, `total_pressure`, `mach`, `wall_shear`, `wall_y_plus`; `samples.<name>` lines or planes written to `sample_<name>.csv`. `wall_shear`/`wall_y_plus` are refused with a turbulence model (they use the laminar `physics.nu`).
- `rcfd sample <case> ...` runs the case first ("There is no solution-file reader yet"); service method `field.sample`.
- `rcfd inflate <case|mesh> --patch p --layers n ...` adds prismatic layers to any patch; service method `mesh.inflate`; no case key inflates a mesh yet. Exit codes: 1 failure, 2 usage, 4 quality errors.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-cli]], [[exit-codes]], [[rcfd-post]]
