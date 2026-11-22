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

# Reference data: F-588 the Hex8 formulations on the remaining linear paths (ADR 0525)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-588 the Hex8 formulations on the remaining linear paths (ADR 0525)" (lines 2155-2168) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-588 Hex8 formulations on the remaining linear paths (ADR 0525).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-588. The methods are those of F-390 (ADR 0493: the mean-dilatation B-bar of Hughes and the incompatible modes of Wilson, Taylor, Doherty and Ghaboussi with the correction of Taylor, Beresford and Wilson, cited there from memory); the extensions (the thermal load, per-point elasticity, the condensed load of the modes) are derived in ADR 0525 arithmetic 1 to 4 and checked by plain numpy probes (`f588-scratch/est588.py`, `est588b.py`), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Thermal load of the formulations: $f = \sum_q w_q \bar{B}_q^T D_q \varepsilon_{\mathrm{th},q}$ (B-bar); $f = f_u - K_{ua} K_{aa}^{-1} f_a$, $f_a = \sum_q w_q G_q^T D_q \varepsilon_{\mathrm{th},q}$, modes $\alpha = K_{aa}^{-1} (f_a - K_{au} u)$ (incompatible); the free expansion $u = \varepsilon_{\mathrm{th}} (x - x_A)$ gives $\alpha = 0$ and zero stress for a constant $\varepsilon_{\mathrm{th}}$ | derived | ADR 0525 arithmetic 1 |
| The stiffness of the incompatible element is not additive in $D$ (`max |K(D1 + D2) - K(D1) - K(D2)| / max |K|` `1.0e-2` in the prototype, `7.6e-3` measured); that of the B-bar element is (`1.6e-16`) | computed | ADR 0525 arithmetic 4, N-LIN, LIN1 |
| Euler's buckling load of the clamped-free column $P_{\mathrm{cr}} = \pi^2 E I / (4 L^2)$; the strip $L = 2$, $h = 0.1$ ($I = h^4/12$), $E = 2 \times 10^{11}$: $P_{\mathrm{cr}} = 1.0281 \times 10^{6}$ N (not a measured value, a closed form) | derived | Euler's column (textbook closed form, not read) |
| Measured: one-layer strip `20 x 1 x 1`, $\lambda P / P_{\mathrm{cr}}$ `0.99949` (Incompatible, $\nu = 0$), `1.00306` ($\nu = 0.3$), `1.498`, `1.542` (Full), `1.074`, `0.826` (B-bar) | computed | ADR 0525 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
