---
type: source
tags: [reference-data, model-constants]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Viscosity laws (F-110)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Viscosity laws (F-110)" (lines 289-310) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Viscosity laws of F-110 (Carreau-Yasuda, Cross, WLF, Papanastasiou and others) with their verification state.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


The law forms in `crates/rcfd-api/src/rheology.rs` are textbook expressions written from memory of the literature (Bird, Armstrong and Hassager 1987; Carreau 1972; Yasuda 1981; Cross 1965; Herschel and Bulkley 1926; Papanastasiou 1987; Williams, Landel and Ferry 1955) and were not checked against the papers in this session. Unverified:

- the Carreau-Yasuda exponent convention `(n-1)/a`;
- the Cross form `1/(1+(lambda g)^m)`;
- the regularised yield term `tau0 (1 - exp(-m g))/g`;
- the WLF constants 17.44 and 51.6 K (the "universal" values, relative to the glass transition temperature, offered only as defaults).

Wave 13: the Carreau-Yasuda `(n-1)/a` convention and the WLF constants 17.44 and 51.6 K are now confirmed, secondary: arXiv:1810.02292, IUPAC Gold Book goldbook.iupac.org/terms/view/12821; Cross and Papanastasiou remain unverified.

The checks that do not depend on those forms are analytic:

- the power-law plane channel profile and pressure gradient (`u = U (2n+1)/(n+1) [1 - |1-2y/H|^((n+1)/n)]`, derived by integrating the momentum balance);
- the Buckingham-Reiner plane solution of a Bingham fluid (`U = G h^2/(3 mu) (1 - 1.5 xi + 0.5 xi^3)`, `xi = tau0/(G h)`, mean speed verified to 1e-6 by quadrature of the profile in the test).


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
