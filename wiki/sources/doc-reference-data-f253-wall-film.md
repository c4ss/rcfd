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

# Reference data: F-253 particle wall film (ADR 0192)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-253 particle wall film (ADR 0192)" (lines 1384-1395) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-253 particle wall film references (ADR 0192).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Nothing in F-253 is compared with measured data. The film model is derived; two published thresholds set its range.

| Item | Status | What was read (source) |
|---|---|---|
| Nusselt film profile `u = (rho g sin(theta)/mu)(h y - y^2/2)`, `u_bar = rho g sin(theta) h^2/(3 mu)`, `Gamma = rho^2 g sin(theta) h^3/(3 mu)` and the thickness `h = (3 mu Gamma/(rho^2 g sin(theta)))^(1/3)` | derived | momentum balance of a steady laminar film with no slip at the wall and no shear at the surface, worked out in ADR 0192 M1 |
| Laminar wave-free film for `Re = 4 Gamma/mu_l` below about 30 (wavy laminar to about 1800, turbulent above) | verified (secondary) | two university lecture notes on film condensation found by a web search on 2026-11-05 (the original texts they draw on not accessed) |
| Long-wave instability of a film on a plane inclined at `theta` above `R_c = (5/4) cot(theta)`, `R = rho h U_s/mu`, `U_s` the Nusselt surface speed (`(5/6) cot(theta)` for a Reynolds number on the mean velocity) | verified (secondary) | Thompson, Tseluiko and Papageorgiou, "Falling liquid films with blowing and suction", J. Fluid Mech. 2015, arXiv:1506.01579, read 2026-11-05; a second search result gives the mean-velocity form; Benjamin (1957) and Yih (1963) not accessed. In `Re_f = 4 Gamma/mu` the threshold is `(10/3) cot(theta)`, 5.774 at 30 degrees |
| Donor-cell transport, Courant limit on the kinematic wave speed `3 u_bar`, steady state exact below the feed | derived | ADR 0192 M5 to M7 |
| Falling-film thickness against a measured data set | not used | no comparison attempted |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
