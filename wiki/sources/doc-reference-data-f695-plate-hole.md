---
type: source
tags: [reference-data, structural]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-695 the converged peak stress of the two-layer plate with a hole (ADR 0627)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-695 the converged peak stress of the two-layer plate with a hole (ADR 0627)" (lines 2384-2389) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-695 converged peak stress of the two-layer plate with a hole (ADR 0627).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Quantity | Source | Value and use |
|---|---|---|
| Peak hoop stress at the hole edge of the ADR 0478 two-layer Hex20 plate (`r/W = 0.05`, length equal to width), per remote stress | computed in this repository, not a published value: the uniformly graded meshes of 24, 48 and 96 cells along the hole in ADR 0627 | `K_96 = 3.03697`; mesh error about 1.5e-5 relative. It is 1.23 percent above Kirsch's `3 sigma_0` (finite plate). The finite-width forms recorded above (Howland/Peterson, from memory, not read) bracket it but are not its source. Recorded on the maintainer's standing instruction (ADR 0627 Q5). |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
