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

# Reference data: Wave 16B boundary-condition catalogue (F-200, ADR 0022)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 16B boundary-condition catalogue (F-200, ADR 0022)" (lines 690-704) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 16B boundary-condition catalogue references (ADR 0022), including the Poiseuille check.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Every closed form below is a textbook relation written from memory and not checked against a source in this session; each was re-derived in the ADR or checked numerically as stated. No vendor data is used.

| Item | Source | Status |
| --- | --- | --- |
| Plane Poiseuille flow: `-dp/dx = 12 mu U / H^2`, profile `6 U (y/H)(1 - y/H)` | standard fluid mechanics | derived from the momentum balance (parabolic profile, mean `U`); unverified (memory) as a citation. The measured 0.498 % deficit is the cell-centred wall-gradient error `1.5 (h/gap)^2` predicted in the ADR |
| Annular Poiseuille flow: `Q = pi G / (8 mu) [R2^4 - R1^4 - (R2^2 - R1^2)^2 / ln(R2/R1)]` | Lamb, Hydrodynamics; Bird, Stewart and Lightfoot | unverified (memory); the closed form is evaluated with the equal-area radii of the 48-gon (0.499286, 0.998572), and the 0.398 % agreement with the solver is a consistency check, not a verification of the formula |
| Radial equilibrium `dp/dr = rho w^2 / r` of swirling flow, rigid rotation `p = p_a + rho Omega^2 (r^2 - <r^2>)/2` | standard (the F-200 roadmap text states the relation) | derived from the radial momentum balance; unverified (memory) as a citation |
| Total-pressure inlet `p = p0 - |u|^2/2`, Bernoulli for the frictionless duct | standard | textbook; unverified (memory) |
| Vent loss law `dp = K rho u |u| / 2` | common engineering practice | unverified (memory); the quadratic law with the dynamic head `rho u^2/2` as the unit is a project definition |
| Inlet turbulence `k = 1.5 (I U)^2`, `eps = Cmu^(3/4) k^(3/2) / l`, `eps = Cmu k^2 / (nu ratio)`, `omega = eps / (beta* k)` | standard | pre-existing helpers (F-044); the registered values 3.75e-3, 5.390521e-4, 1.597191, 1.265625e-6 are arithmetic of these formulas; unverified (memory) |
| Secant and dynamic-head sensitivity updates of the target-mass-flow pressure | generic root finding | project design; the secant option is a documented known miss (ADR 0022 addendum) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
