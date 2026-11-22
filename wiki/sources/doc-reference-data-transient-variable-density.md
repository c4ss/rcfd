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

# Reference data: Transient variable density (ADR 0009)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Transient variable density (ADR 0009)" (lines 407-428) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Transient variable-density checks of ADR 0009: closed forms and unverified acoustic and Couette citations.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Reference values used by `crates/rcfd-fv/tests/varden_transient*.rs` and `crates/rcfd-api/tests/transient_variable_density.rs`:

- Differentially heated cavity (T4): the steady Le Quere et al. 2005 values of the section above (Nu 8.85978, P/P0 0.856338, verified from the author's postprint). The transient approach is checked against them; no transient reference data exist or are claimed.
- Closed-box heating (T1, T1r), temporal order (T2): closed-form solutions derived in the ADR (uniform heating of a closed ideal gas: `p = rho0 R T`, `T = T0 + q t/(rho cv)`; sinusoidal source `T - T0 = q0/(rho0 cv w) (1 - cos w t)`). The real-gas energy balance uses the Peng-Robinson internal energy of the solver's own EOS (self-consistency, not an independent data set).
- Standing acoustic wave (T3): linear acoustics, `f = a/(2L)` of a tube closed at both ends and the Stokes-Kirchhoff decay estimate; textbook results (not looked up in this session; the code uses the formulae as written in the ADR). UNVERIFIED against a primary source here.
- Couette flow with viscous heating (T6): `k T'' + mu (U/H)^2 = 0`, profiles `T - T_w = mu U^2/(2k) (y/H)(1 - y/H)` and `(y/H)(2 - y/H)`. ADR 0009 cites Bird, Stewart and Lightfoot, Transport Phenomena 2nd ed., secs. 10.4-10.5, and Schlichting and Gersten, Boundary-Layer Theory 9th ed., sec. 13.3; neither was accessed in this session. The profiles follow from the stated equation by two integrations and the code agrees with them to 1.6e-4 of the scale. UNVERIFIED section numbers.
- Patterson and Imberger (1980), J. Fluid Mech. 100, 65: only the order of the boundary-layer filling time `L^2/(kappa Ra^(1/4))` is used to choose the 20 s run length of T4; not accessed, no number is compared with it.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
