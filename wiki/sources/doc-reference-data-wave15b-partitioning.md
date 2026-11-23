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

# Reference data: Wave 15B partitioning, reordering, inflation and derived fields (F-236, F-108, F-072, ADR 0020)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 15B partitioning, reordering, inflation and derived fields (F-236, F-108, F-072, ADR 0020)" (lines 669-681) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 15B partitioning, reordering, inflation and derived-field references (ADR 0020).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Every literature statement below was written from memory; none was checked against a source. None of the registered checks depends on them: every expected value is a closed form or invariant derived in the ADR.

| Item | Source | Status |
| --- | --- | --- |
| Boundary refinement by single-cell moves with positive gain under a balance cap (Kernighan-Lin / Fiduccia-Mattheyses family) | Kernighan and Lin (1970); Fiduccia and Mattheyses (1982) | unverified (memory); only the idea of gain-driven moves is used, the implementation is a plain greedy variant, not a published algorithm |
| Recursive coordinate bisection and inertial bisection | Berger and Bokhari (1987); Taylor and Nour-Omid (1994) | unverified (memory); cuts checked against closed forms in the ADR |
| Reverse Cuthill-McKee ordering with a pseudo-peripheral start | Cuthill and McKee (1969); George and Liu (1981) | unverified (memory); the bandwidth bound $2 n_2 n_3 - 1$ for grids is derived in ADR 0020 A2 from the level-set structure |
| Isentropic total-pressure relation $p_0/p = \left(1 + \frac{\gamma-1}{2} M^2\right)^{\gamma/(\gamma-1)}$ | standard gas dynamics | derived series expansion to $M^4$ in ADR 0020 C2 (binomial); the closed form itself is textbook, unverified (memory) |
| Edge-isoperimetric minimum bisection width of a grid (cross-section of the longest axis) | Bollobas and Leader (1991) | unverified (memory); NOT used: only the upper bound given by the slab is registered |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
