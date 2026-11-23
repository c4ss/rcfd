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

# Reference data: F-254 particle wall interaction (ADR 0194)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-254 particle wall interaction (ADR 0194)" (lines 1408-1421) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-254 particle wall interaction references (ADR 0194).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Nothing in F-254 is compared with measured data. The erosion correlation is read from its source; the rough-wall model is taken from secondary sources, and its realizability rule and rebound-angle distribution are derived.

| Item | Status | What was read (source) |
|---|---|---|
| Erosion $E = m_p K U_p^n F(\alpha)$, $F(\alpha) = \sum_{i=1}^{8} (-1)^{i+1} A_i (\alpha\pi/180)^i$, $A_1 \ldots A_8$ = 9.370, 42.295, 110.864, 175.804, 170.137, 98.398, 31.211, 4.170; $\alpha$ the angle between the particle path and the surface; depth rate $E/(\rho_t A_t)$ | read | DNV RP O501 "Erosive wear in piping systems", Revision 4.2-2007 (amended January 2011), eq. 7.1 to 7.3, Table 7-1, Figure 7-1, eq. 8.1, read 2026-11-06 from a third-party copy (wiki.pengtools.com); the binding dnv.com version and the later DNVGL-RP-O501 editions not read; Haugen et al. (1994), the source DNV cites for F, not read |
| $K$, $n$ and density of Table 7-2: steel grades 2.0e-9 (m/s)^-n, 2.6, 7800 kg/m^3; titanium 2.0e-9, 2.6, 4500; GRP/epoxy 0.3e-9, 3.6, 1800; GRP/vinyl ester 0.6e-9, 3.6, 1800 | read | the same pages; quoted in the docs, not used as defaults. The steel row is the opt-in preset `particles.erosion.material = "dnv_o501_2007_steel"` (ADR 0197); the other rows are quoted only. The Table 7-2 values come from a third-party copy of DNV RP O501 Rev 4.2-2007 that has not been checked against the maintainer's edition; a preset gives an indicative, not certified, erosion rate |
| Rough wall: impact angle $\theta' = \theta + \Delta_\gamma \xi$, $\xi$ standard normal; a virtual wall tilted by the roughness angle; the shadow effect described in words | verified (secondary) | Tian, Inthavong, Tu and Yeoh, ANZIAM J. 48 (CTAC2006) C509-C526 (2007), eq. 5 and Figure 2; Lain and Sommerfeld, slides of the 14th Workshop on Two-Phase Flow Predictions (Halle, 2015); Radenkovic, doctoral dissertation, University of Belgrade (2019), eq. 7.21 and 7.27; Sommerfeld (1992) and Sommerfeld and Huber (1999) not accessed |
| Realizability rule (redraw until the parcel reaches the virtual wall from the fluid side and rebounds away from the real wall) and the resulting rebound-angle distribution | derived | ADR 0194 W4 and W5; not the effective distribution of Sommerfeld and Huber (1999), which was not read |
| One-sample Kolmogorov-Smirnov test, Kolmogorov distribution $1 - 2 \sum (-1)^{k-1} \exp(-2 k^2 x^2)$, $K_{0.01} = 1.6276$ | verified (secondary) | Wikipedia "Kolmogorov-Smirnov test", read 2026-11-06; the originals not accessed |
| Measured erosion rates or rebound distributions | not used | no comparison attempted |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
