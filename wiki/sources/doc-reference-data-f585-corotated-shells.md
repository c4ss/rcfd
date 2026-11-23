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

# Reference data: F-585 corotated shells in the arc-length continuation and the nonlinear transient (ADR 0522)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-585 corotated shells in the arc-length continuation and the nonlinear transient (ADR 0522)" (lines 2114-2127) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-585 corotated shells in arc-length and nonlinear transient (ADR 0522).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-585. The hinged cylindrical roof is the benchmark of K. Y. Sze, X. H. Liu and S. H. Lo, Finite Elem. Anal. Des. 40 (2004) 1551 (found by a web search of 2026-11-14; the paper was NOT read and none of its numbers is used: the published limit load is an open question of ADR 0522). The references of the checks are closed forms and in-house runs.

| Item | Status | Source |
|---|---|---|
| Hinged roof geometry: $R = 2540$, straight edges `508`, half-angle `0.1` rad, $t = 12.7$, $E = 3102.75$, $\nu = 0.3$, central point load, straight edges hinged, curved edges free | cited | Sze, Liu and Lo (2004): NOT read (geometry as given in the brief) |
| Registered reference: the limit load of the Hex20 `16 x 16 x 2` quarter (total Lagrangian St. Venant-Kirchhoff, mid-thickness line pinned), `2167.5` N (point load), `2186.4` N (patch load); the sequence `2213.9, 2184.5, 2167.5` N at $n = 4, 8, 16$ does not converge at second order (singular hinge line); not a valid reference | computed | ADR 0522 addendum 1 |
| Shell4 / Shell3 quarter, point load: limit load `2223.6` N (Shell4 `8 x 8`), `2222.4` N (Shell4 `16 x 16`), `2221.3` N (Shell3 `16 x 16`), apex deflection at the limit about `10.8` | computed | ADR 0522 addendum 1 |
| Trapezoidal (average-acceleration) period factor $2 \arctan(\omega h/2)/(\omega h)$, `0.997945` at 40 steps per period | derived | ADR 0522 arithmetic 3 |
| Euler-Bernoulli cantilever $f_1 = \beta_1^2/(2 \pi L^2) \sqrt{E I/(\rho A)}$, $\beta_1 = 1.875104068711961$; strip $L = 12$, $b = 1$, $t = 0.1$, $E = 1.2 \times 10^{6}$, $\rho = 1$: `0.122886` Hz | derived | ADR 0522 arithmetic 6 |
| Measured: Shell4 strip $f_1 = 0.122940$ Hz (`+4.3e-4` of Euler-Bernoulli); 100 periods at a tip amplitude `1e-3 L`: frequency `-2.05e-3` of $f_1$ (the period factor within `8e-8`), energy within `2.8e-8` | computed | ADR 0522 addendum 1 |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
