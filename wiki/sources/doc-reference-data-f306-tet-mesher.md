---
type: source
tags: [reference-data, f-item-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-306 the folded edge removal of the tetrahedral mesher (ADR 0228)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-306 the folded edge removal of the tetrahedral mesher (ADR 0228)" (lines 1580-1587) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-306 folded edge removal of the tetrahedral mesher (ADR 0228).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-306. The cause and the fix are derived on the code of `crates/rcfd-mesh/src/tetmesh.rs`.

| Item | Status | What was read (source) |
|---|---|---|
| Edge removal: a ring of cells around an interior edge `(d, e)` replaced by the cells `(a, b, c, d)` and `(a, b, c, e)` of each triangle of a triangulation of the ring polygon; it is valid when every triangle, taken in the ring's turning order, has `d` on the side `-s` and `e` on the side `s` of its plane (`s = orient3d(d, e, poly[0], poly[1])`), and the old test (`d`, `e` on opposite sides) also passes triangles folded across a reflex corner | derived | ADR 0228 "The cause"; the same rule as F-247's `cand_removal` (ADR 0215 M3, M4) |
| Edge removal and its dynamic-programming triangulation (Shewchuk's topological improvement of tetrahedral meshes; Klincsek's polygon triangulation) | from memory | NOT read; the existing code's choice of triangulation is unchanged |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
