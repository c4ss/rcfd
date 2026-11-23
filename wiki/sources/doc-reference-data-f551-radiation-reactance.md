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

# Reference data: F-551 radiation reactance and surface pressure of a baffled plane (ADR 0488)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-551 radiation reactance and surface pressure of a baffled plane (ADR 0488)" (lines 1744-1757) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-551 radiation reactance of a baffled plane (ADR 0488).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-551; every item is cited from memory and derived in ADR 0488 (derivations 1 to 6; numpy checks `f551-scratch/estimate.py` and `estimate2.py`). The impedance closed form with J1 and H1 is the one verified in ADR 0468. No measured data.

| Item | Status | Source |
|---|---|---|
| In-plane $\int_{\mathrm{polygon}} dS/R = \sum_{\mathrm{edges}} d \ln((R_+ + s_+)/(R_- + s_-))$ (signed distance $d$ to the edge line, tangent coordinates `s-`, `s+`) | derived | polar integration about the point; the edge-sum form of D. R. Wilton, S. M. Rao, A. W. Glisson, D. H. Schaubert, O. M. Al-Bundak and C. M. Butler, IEEE Trans. Antennas Propag. AP-32 (1984) 276-281: NOT read |
| Square of side $L$: centre $4 L \ln(1 + \sqrt{2})$, corner $2 L \ln(1 + \sqrt{2})$; rectangle $a \times b$ at a corner $a \ln((b + \sqrt{a^2+b^2})/a) + b \ln((a + \sqrt{a^2+b^2})/b)$; right triangle of legs $L$ at the right angle $\sqrt{2} L \ln(1 + \sqrt{2})$ | derived | from the edge sum by hand; numpy and the A1 check to 4.4e-16 |
| Potential of a uniform disk at an in-plane point, $\int_{\mathrm{disk}} dS/R = 4 a E(r/a)$ | from memory | checked at $r = 0$ ($2 \pi a$) and $r = a$ (`4 a`) by hand |
| Added mass of a baffled piston $8 \rho a^3/3$ (low-frequency limit of $\rho c S H_{1}(2 k a)/(k a)/\omega$) | derived | $H_{1}(x) = 2 x^2/(3 \pi) + O(x^4)$; Kinsler, Frey, Coppens and Sanders, and Morse and Ingard, chapter 7: NOT read |
| Surface continuity $P(x + h n) = P(x) - i \omega \rho V_n h + O(h^2)$ | derived | the Euler equation at the plane |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
