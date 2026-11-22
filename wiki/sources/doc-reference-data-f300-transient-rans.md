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

# Reference data: F-300 transient RANS follow-ups (ADR 0225)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-300 transient RANS follow-ups (ADR 0225)" (lines 1598-1604) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-300 transient RANS follow-ups (ADR 0225).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | Status | What was read (source) |
|---|---|---|
| Spalding (1961) law of the wall `y+ = u+ + (1/E)[exp(kappa u+) - 1 - kappa u+ - (kappa u+)^2/2 - (kappa u+)^3/6]`, used for the k-based wall eddy viscosity `nu (y*/u+ - 1)` of the opt-in `wall_nut_law = spalding` (`WallConstants::nut_wall_from_k_spalding`) and its inverse `SpaldingLaw::u_plus` | existing | the row "Spalding (1961) law of the wall" above (standard cubic form, two secondary sources, original paper not read); no new source was read for F-300 |
| The standard k-based wall eddy viscosity `nu (y* kappa/ln(E y*) - 1)` above `y*_lam` and 0 below, written as `nu (y*/u+ - 1)` with `u+ = min(y*, ln(E y*)/kappa)`; its slope jump `0.0684 nu` at `y*_lam = 11.53` | derived | from the existing `WallConstants::nut_wall_from_k` (Launder and Spalding 1974 as recorded above, not read) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
