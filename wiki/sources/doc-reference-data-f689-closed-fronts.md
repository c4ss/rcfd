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

# Reference data: F-689 the interaction integral on closed fronts and at mirror-plane stations (ADR 0618)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-689 the interaction integral on closed fronts and at mirror-plane stations (ADR 0618)" (lines 2328-2339) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-689 interaction integral on closed fronts (ADR 0618).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-689. The closed forms are those of ADR 0610 (F-682 above) with the shear direction rotated in the crack plane (ADR 0618 D4), verified by plain arithmetic (`f689-scratch/py/pre.py`, numpy, no rcfd code).

| Item | Status | Source |
|---|---|---|
| Penny crack, remote tension `sigma` along `(sin beta cos psi, sin beta sin psi, cos beta)`: `K_I = 2 sigma cos^2 beta sqrt(a/pi)`, `K_II = A_II cos(theta - psi)`, `K_III = A_III sin(theta - psi)`, `A_II = 4 tau sqrt(a/pi)/(2 - nu)`, `A_III = 4 (1 - nu) tau sqrt(a/pi)/(2 - nu)`, `tau = sigma sin beta cos beta` (frame `(e, n, e x n)`, `theta` from `x`) | derived, verified | ADR 0618 D4: ADR 0610 D2 rotated by `psi - pi/2` (in-plane isotropy); `psi = pi/2` gives ADR 0610's forms exactly; the projection of the rotated slip jump onto `e` and `e x n` equal to the formulas to `4.2e-16`; the front mean of `G` independent of `psi` |
| `K_III = 0` at a station on a mirror plane normal to the front (the tearing displacement odd under the reflection) | derived | ADR 0618 D2 |
| Perimeter of the 16 quadratic edges of the ellipse `b = a/2` through equal steps of the parametric angle: `-2.45e-5` relative to `4 a E(k)` | computed | ADR 0618 arithmetic 1 |
| Measured: whole penny (66 174 unknowns, closed front, 16 stations) under tension inclined `beta = 45 deg`, `psi = 35 deg`, ring 2: `K_I` within `7.0e-4`, `K_II` within `7.3e-4 A_II`, `K_III` within `4.9e-3 A_III` of the hat-averaged closed forms at every station; the half model of ADR 0610 with the mirror option against the whole model under the same load within `2.2e-13 A`; whole ellipse `b/a = 0.5` mode I within `1.1e-2` of Irwin | computed | ADR 0618 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
