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

# Reference data: F-302 periodic seam pair diffusivity: the property and user-function viscosities (ADR 0227)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-302 periodic seam pair diffusivity: the property and user-function viscosities (ADR 0227)" (lines 1588-1597) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-302 periodic seam pair diffusivity (ADR 0227).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Nothing was read for ADR 0227. The rule is the flux balance of a pair of finite-volume faces, derived in ADR 0205 D1, applied to the face formulas the code already uses on internal faces (`crates/rcfd-api/src/properties.rs`, the user-function face loop of F-122).

| Item | Status | Source |
|---|---|---|
| Both faces of a periodic pair take one diffusivity, the internal-face formula with the pair's weight `w = dn_g / (dn_f + dn_g)`: `mu(w T_P + (1 - w) T_Q) / rho` for a property law, the module at the centre of the pair's face of lower index with `w T_P + (1 - w) T_Q` and `w abs(u_P) + (1 - w) abs(u_Q)` for a user function | derived | ADR 0205 D1, ADR 0212 D1, ADR 0227 D1 (the face of lower index is a choice of ADR 0227, question 1) |
| The property viscosity is already one value per pair by default, up to rounding (the periodic face temperature is the same interpolation) | derived | ADR 0227 item 2 (code read), checked by F2p, G2p and T4p |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
