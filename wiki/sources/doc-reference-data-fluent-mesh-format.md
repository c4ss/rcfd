---
type: source
tags: [reference-data, model-constants]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Fluent mesh format details (F-118)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Fluent mesh format details (F-118)" (lines 277-288) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Fluent mesh-format details (F-118): boundary type ids typed from memory and the elbow.msh check.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


All from memory of the public format description and not checked against a primary source in this session, except where a real file confirmed them. Treat as unverified.

- Boundary type ids: 2 interior, 3 wall, 4 pressure-inlet, 5 pressure-outlet, 7 symmetry, 8 periodic-shadow, 10 velocity-inlet, 12 periodic, 20 mass-flow-inlet, 31 parent (hanging node), 36 outflow, 37 axis. The names in section 45 take precedence. The real file `elbow.msh` confirmed 2 (interior), 3 (wall), 5 (pressure-outlet), 10 (velocity-inlet) with names and the 2D layout (`(10 (zone first last type nd)`, face records `n0 n1 right-cell left-cell`, hex numbers in headers).
- Section 18 as periodic shadow faces; the binary section ids 2010/3010 (nodes single/double), 2012/3012, 2013/3013 and the `End of Binary Section` terminator; little-endian 32-bit integers in binary face data. No real binary file was read.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
