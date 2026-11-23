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

# Reference data: Wave 19B meshing references (ADR 0030)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 19B meshing references (ADR 0030)" (lines 807-820) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 19B meshing references (ADR 0030), unverified.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No published source for the following was accessed in this environment. The algorithms were written from memory and are tested against closed-form or exact-arithmetic checks only.

| Quantity | Source | Status |
| --- | --- | --- |
| Incremental Delaunay insertion (cavity of circumspheres), boundary recovery by Steiner points | Bowyer (1981), Watson (1981) | UNVERIFIED: from memory, primary papers not accessed |
| Exact orient3d / insphere with floating-point filter and expansion arithmetic | Shewchuk (1997) | UNVERIFIED: technique from memory; correctness checked against exact i128 arithmetic (T9), not against the paper's code or constants |
| Radius-edge bound 2.0 for Delaunay refinement | Ruppert / Shewchuk refinement literature | UNVERIFIED: value taken as the commonly stated bound, from memory; the bound is not proven for this implementation (boundary splitting off) |
| Incremental isotropic surface remeshing (split 4/3 h, collapse 4/5 h, valence flip, tangential smoothing) | Botsch and Kobbelt (2004) | UNVERIFIED: ratios from memory |
| Curvature from vertex normals, size = theta / kappa | project definition | no source; checked on sphere and cylinder closed forms |



## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
