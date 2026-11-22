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

# Reference data: F-653 flutter of follower loads, Beck's column (ADR 0580)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-653 flutter of follower loads, Beck's column (ADR 0580)" (lines 2369-2383) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-653 flutter of follower loads, Beck's column (ADR 0580).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-653. The flutter coefficient `20.05` of the roadmap acceptance is cited from memory (Beck's column; M. Beck, "Die Knicklast des einseitig eingespannten, tangential gedrueckten Stabes", ZAMP 3 (1952) 225, and V. V. Bolotin, Nonconservative Problems of the Theory of Elastic Stability, Pergamon 1963: NOT read, from memory). The value used is **derived**, not cited: ADR 0580 arithmetic 1 to 3, plain Python with numpy (`f653-scratch/beck_reference.py`, `beck_reference.log`, `sym_part.py`), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Beck's column, Euler-Bernoulli cantilever under a tangential follower tip force $P$: $E I w'''' + P w'' + m w_{tt} = 0$, $w(0) = w'(0) = 0$, $w''(L) = w'''(L) = 0$; flutter (coalescence of the lowest two $\omega^2$) at $p_{\mathrm{cr}} = P L^2/(E I) = 20.0509536$, $m \omega_{\mathrm{cr}}^2 L^4/(E I) = 121.342510$ | derived | the closed-form 2 x 2 determinant of the boundary conditions, $D = dD/dW = 0$ by Newton (ADR 0580 arithmetic 1); confirmed by Hermite cubic elements, $N = 80$ within `1.5e-8`, error $O(h^4)$ (arithmetic 2) |
| Unloaded cantilever, $\Omega = 3.516015, 22.034492$ | derived | the same determinant at $p = 0$ |
| Timoshenko beam ($\kappa = 5/6$, $\nu = 0$, rotary inertia) under the same load: $p_{\mathrm{cr}} = 19.8277, 19.9946, 20.0368$ at $L/h = 25, 50, 100$ | computed | transfer matrix of the first-order system, ADR 0580 arithmetic 3 |
| The static problem of Beck's column with the full load stiffness has no positive real factor (Hermite, $N = 10, 25, 50$); with the symmetric part of the follower term the factor $\pi^2$ appears (a spurious divergence); with a dead tip force $\pi^2/4$ (Euler) | derived, computed | ADR 0580 arithmetic 3 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
