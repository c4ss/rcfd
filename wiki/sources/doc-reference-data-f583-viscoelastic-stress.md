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

# Reference data: F-583 the harmonic stress of a viscoelastic material (ADR 0494)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-583 the harmonic stress of a viscoelastic material (ADR 0494)" (lines 2005-2014) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-583 harmonic stress of a viscoelastic material (ADR 0494).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-583. Every formula used is derived in ADR 0494 (arithmetic 1 to 7) or re-used from ADR 0491; the references of the checks are the modulus times the strain of the 1-D chain solved in the test.

| Item | Status | Source |
|---|---|---|
| Split stress of the isotropic viscoelastic solid, $\sigma^{*} = (G^{*}/G_0) \operatorname{dev} \sigma_0 + (K^{*}/K_0) \operatorname{vol} \sigma_0$, $\sigma_0$ the elastic stress of the base ($\operatorname{dev} \sigma_0 = 2 G_0 \operatorname{dev} \varepsilon^{*}$, $\operatorname{vol} \sigma_0 = K_0 \operatorname{tr} \varepsilon^{*} 1$); for equal weights $(E^{*}/E_0) \sigma_0$ | derived | ADR 0494 arithmetic 1 (the correspondence principle as in ADR 0491 arithmetic 1) |
| Stress of the rods of ADR 0491 with the lateral motion held: $\sigma_{xx} = (K^{*} + 4 G^{*}/3) \varepsilon$, $\sigma_{yy} = \sigma_{zz} = (K^{*} - 2 G^{*}/3) \varepsilon$ (axial), $\sigma_{xy} = G^{*} \gamma$ (shear) | derived | ADR 0494 arithmetic 2 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
