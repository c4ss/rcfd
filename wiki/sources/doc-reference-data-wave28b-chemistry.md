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

# Reference data: Wave 28B chemistry acceleration and combustion extensions (F-223, F-129, ADR 0053)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 28B chemistry acceleration and combustion extensions (F-223, F-129, ADR 0053)" (lines 1151-1168) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 28B chemistry acceleration and combustion extension references (ADR 0053).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read in this wave. Every reference below is recalled from memory and is unverified.

| Item | Status | Note |
|------|--------|------|
| Pope (1997), in-situ adaptive tabulation: ellipsoid of accuracy, retrieve / grow / add | unverified | the algorithm as coded is DERIVED from the recalled outline (Sherman-Morrison growth, linear error test at the query); the paper was not read |
| Lu and Law (2005), directed relation graph reduction | unverified | interaction coefficients and graph search coded from the recalled outline; no published reduced-mechanism size is claimed |
| Clustering of similar cells with increment transfer | derived | leader clustering in the scaled max-norm; no source |
| Mixture fraction (Bilger element definition), unity-Lewis linear enthalpy mixing | unverified | derived here from element conservation |
| Beta PDF of the mixture fraction, moments and parameters | derived | quadrature of the table is exact; the incomplete beta by continued fraction (modified Lentz), lnGamma by Stirling plus recurrence, tested to 4e-15 |
| Burke and Schumann (1928) infinitely fast chemistry | unverified | the closed form is derived for a one-step toy mechanism with constant cp (Z_st 3/19, T_st 2194.7 K) |
| Peters (1984) steady flamelet equations, chi(Z) from erfc^-1 | unverified | equations derived with unity Lewis number; the scalar-dissipation profile is the recalled form |
| GRI CH4/air maximum equilibrium temperature about 2225 K | unverified | from memory, reported by test B7 only; not a check |
| Fisher / KPP (1937) front speed 2 sqrt(Gamma k) for the Zeldovich cubic | unverified | the closed form m = sqrt(Gamma k/2) of the cubic was derived; the paper was not read |
| Sandia flames D to F, any published flame | not used | no comparison attempted |
| Published ignition-delay measurements | not used | the 5 percent point is met against the same mechanism integrated directly only |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
