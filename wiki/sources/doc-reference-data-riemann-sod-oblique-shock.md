---
type: source
tags: [reference-data, benchmark]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Exact Riemann solver, Sod problem and oblique shock

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Exact Riemann solver, Sod problem and oblique shock" (lines 107-123) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Exact Riemann solver, Sod problem and oblique-shock reference values; a Toro table attribution removed and the Darwish-Moukalled citation corrected.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Used in `crates/rcfd-fv/src/riemann.rs` tests, `compressible.rs` wedge tests.

| Value | Source | Result |
|---|---|---|
| Sod (rho,u,p) = (1,0,1) / (0.125,0,0.1): p* = 0.30313, u* = 0.92745 (also rho*L 0.42632, rho*R 0.26557) | arXiv:2108.01709 (analytic Sod solution table) | verified, secondary |
| Attribution "Toro (2009) Table 4.3 test 1" | Toro's test-suite Test 1 is the *modified* Sod with uL = 0.75 (Toro lab handout, edanya.uma.es) | table number unverifiable, removed from the comment |
| theta-beta-M relation tan(theta) = 2 cot(beta)(M^2 sin^2 beta - 1)/(M^2(gamma + cos 2beta) + 2) | Wikipedia "Oblique shock" (same form as Anderson ch. 4) | verified |
| M = 2, theta = 10 deg, gamma 1.4: weak beta = 39.31 deg | independent bisection solve (39.314 deg; strong 83.70, theta_max 22.97 deg, p2/p1 = 1.7066); an online oblique-shock calculator quotes 39.3 deg and p2/p1 about 1.71 by normal-shock relations | verified (computed + corroborated) |
| theta = 30 deg at M = 2 detaches | theta_max = 22.97 deg from the solve | verified |
| NACA 1135 chart attribution | not accessed | unverifiable, removed from the comment |
| Reference list entries Roe 1981 JCP 43, Harten 1983 JCP 49, Liou 1996 JCP 129, Shu-Osher 1988 JCP 77, Toro et al. 1994 | bibliographic recollection | wave 13: Roe, Harten, Liou, Shu-Osher confirmed via Crossref; Toro 1994 not re-checked |

Corrected: `compressible.rs` cited "Darwish and Moukalled, Numer. Heat Transfer B 44 (2003)"; the TVD paper is Int. J. Heat Mass Transfer 46(4) (2003) 599-611 (confirmed by multiple citing sources). Fixed the doc comment.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
