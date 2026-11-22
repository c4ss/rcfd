---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 23C population balance references (F-228, ADR 0041)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 23C population balance references (F-228, ADR 0041)" (lines 911-929) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 23C population balance references (ADR 0041), all unverified.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No paper was read and no web check was made in this wave: every literature entry below is a citation written from memory and is marked UNVERIFIED. None of them supplies a number used by a test. Every acceptance value is a closed form derived in ADR 0041 from the moment equations (and checked by the registered tests), or an identity of the code.

| Quantity | Source | Status |
| --- | --- | --- |
| Constant-kernel exponential solution `n(v,t) = N0/(1+tau)^2 exp(-v/(1+tau))`, `tau = K N0 t/2`; moments `m_k = k! N0 (1+tau)^(k-1)` | Smoluchowski (1917); W. T. Scott (1968), J. Atmos. Sci. 25, 54-65 (author initials CORRECTED in wave 25B, see the wave 25B verification section) | citation record verified in wave 25B (Crossref); the solution is DERIVED in ADR 0041 (substitution into the moment equation, every `k`) |
| Discrete (Flory) constant-kernel distribution `N_k = N0 tau^(k-1)/(1+tau)^(k+1)` | discrete Smoluchowski system on the integer grid | DERIVED; reproduced by the integrated bin solution (test B1, 1.2e-11) |
| Sum-kernel `m_0 = N0 e^(-B m_1 t)`, `m_2 = m_2(0) e^(2 B m_1 t)`; product-kernel `m_0 = m_0(0) - C m_1^2 t/2`, `m_2 = m_2(0)/(1 - C m_2(0) t)`, `m_3 = m_3(0)(1 - C m_2(0) t)^-3`, gel time `1/(C m_2(0))` | moment equations | DERIVED (ADR 0041); not compared to a paper |
| Breakup moment laws (constant rate: uniform `exp(Gamma t (1-k)/(1+k))`, binary `exp(Gamma t (2^(1-k) - 1))`; linear rate `m_0 = m_0(0) + m_1 t`) | moment equations | DERIVED (ADR 0041) |
| Fixed-pivot discretisation | S. Kumar and D. Ramkrishna (1996), Chem. Eng. Sci. 51, 1311-1332 | citation UNVERIFIED (memory); scheme restated and its conservation properties derived in ADR 0041 |
| QMOM | R. McGraw (1997), Aerosol Sci. Technol. 27, 255-265 | citation UNVERIFIED (memory) |
| Product-difference algorithm; Wheeler's algorithm | R. G. Gordon (1968), J. Math. Phys. 9, 655-663; J. C. Wheeler (1974), Rocky Mountain J. Math. 4, 287-296 | citations UNVERIFIED (memory); the Wheeler recurrence was reconstructed from the modified Chebyshev algorithm and is validated by exact recovery of finite distributions (Q1) and Gauss-Laguerre abscissae `3 +- sqrt(3)` (Q2) |
| Brownian kernel `(2kT/3mu)(u^(1/3)+v^(1/3))(u^(-1/3)+v^(-1/3))` and laminar shear kernel `(4/3) gdot (r_u+r_v)^3` | Smoluchowski (1917) | citation UNVERIFIED (memory); only equal-size values and symmetry are tested |
| Turbulent shear kernel `sqrt(8 pi/15) sqrt(eps/nu) (r_u+r_v)^3` | Saffman and Turner (1956), J. Fluid Mech. 1, 16-30 | citation and constant UNVERIFIED (memory); only the identity with the laminar-shear kernel is tested, NOT validated against a turbulent case |
| Symmetric-beta daughter distribution (integer order) | Hill and Ng (1995), AIChE J. 41, 1204-1216 | citation UNVERIFIED (memory); the form is defined in ADR 0041 and its moments are verified by quadrature (B10) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
