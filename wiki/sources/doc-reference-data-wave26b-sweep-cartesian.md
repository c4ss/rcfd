---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 26B sweep and Cartesian-core meshing (F-235, F-234, ADR 0047)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 26B sweep and Cartesian-core meshing (F-235, F-234, ADR 0047)" (lines 1063-1072) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 26B sweep and Cartesian-core meshing references (ADR 0047).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | Status | Note |
|------|--------|------|
| Poiseuille pipe `u_c = G R^2/(4 nu)`, `U_b = G R^2/(8 nu)` | derived | from the axisymmetric momentum balance; no source read |
| Pappus volume of revolution, inscribed N-gon area `(N/2) sin(2 pi/N) R^2` | derived | first principles |
| Ghia et al. 1982 cavity centreline Re 100 | verified | two secondary copies (see wave 25B) |
| Ray-cast parity point-in-solid, 2:1 octree balance, hanging-node polyhedra | unverified | from memory of the general literature; not checked against a source |
| Layer-height rules (uniform, aspect ratio, last ratio) | derived | ADR 0047 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
