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

# Reference data: F-663 distributed loads at a height on warping beams (ADR 0588)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-663 distributed loads at a height on warping beams (ADR 0588)" (lines 2246-2258) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-663 distributed loads at a height on warping beams (ADR 0588).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-663. Every reference used is derived in ADR 0588 (D1, D3, D4) and computed by a plain numpy script (`f663-scratch/arith.py`, `arith.log`), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Distributed load-height term of a force per length $q$ along $p$: $(1/2) \int (q \cdot (p - s)) \phi^2 dx$, on the Hermite twist $c M_H$, $M_H = (l/420) [156, 22l, 54, -13l; 22l, 4l^2, 13l, -3l^2; 54, 13l, 156, -22l; -13l, -3l^2, -22l, 4l^2]$ | derived | ADR 0588 D1 (from the point term of ADR 0582 D1; the block by direct integration) |
| End forces $K u$ of an element under a uniform line load: the end moments shifted by $q_z l^2/12$, the shears the mid-element value (the $L^2$ projection of the parabolic moment) | derived, measured `3.9e-14` | ADR 0588 D3, check E1 |
| Rayleigh-Ritz references, forks, $L = 3$, UDL $q_{\mathrm{ref}} = 1000$ N/m downward ($\lambda$, sine series $N = 81$, converged to $\le 2.4 \times 10^{-8}$ from $N = 41$): doubly symmetric I top flange `45.1856414209`, shear centre `59.0640165887`, bottom flange `77.1475881351`; ADR 0553 monosymmetric I at the shear centre `135.747863965` (larger flange compressed), `69.3139026678` (in tension), at the centroid `163.396623703`, on flange 1 `126.453805543` | computed | ADR 0588 D4 |
| Doubly symmetric UDL moment factor $C_1 = 1.1310$ | computed; the textbook `1.13` cited from memory, not used | ADR 0588 D4 |
| Measured, 16 elements: `1.7e-5`, `1.7e-5`, `7.4e-6` (doubly symmetric top flange, shear centre, bottom flange); monosymmetric within `3.5e-4` | computed | ADR 0588 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
