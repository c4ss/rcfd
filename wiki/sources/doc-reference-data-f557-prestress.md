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

# Reference data: F-557 the pre-stress of a static result on a model with solid and structural elements (ADR 0486)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-557 the pre-stress of a static result on a model with solid and structural elements (ADR 0486)" (lines 2026-2038) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-557 pre-stress of a static result (ADR 0486).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-557. The references of the checks are the closed forms of the F-420 section above (the taut-beam frequencies) and solid models of the existing solver (Hex20 for T3 and H1 to H3); the corrections that set the estimates are derived in ADR 0486 from first principles.

| Item | Status | Source |
|---|---|---|
| Taut-beam frequencies `w_n^2 = ((n pi/L)^4 E I + (n pi/L)^2 T)/(rho A)`, `T = pi^2 E I/L^2` | derived | the F-420 section above (ADR 0473 arithmetic 3) |
| End spring `T c` of a block of length `c` in tension rotating rigidly about its hinge (`int sigma_xx dV = T c`, the virial of the block), and its Rayleigh increase `2 c T/(L (E I k^2 + T))` of `omega_n^2` | derived | ADR 0486 arithmetic 1 |
| Overall against local buckling of a square plate with a central eccentric rib by the Rayleigh quotient of `sin(pi x/a) sin(pi y/a)` against the half-panel load `4 pi^2 D/b'^2` | derived | ADR 0486 arithmetic 2 |
| Joints of structural to solid elements: the consistent face coupling of ADR 0460, the node-to-surface tie of ADR 0456 | shared | ADR 0456, ADR 0460 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
