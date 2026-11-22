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

# Reference data: F-309 second-order immersed forcing: the face flux at the surface crossing (ADR 0234)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-309 second-order immersed forcing: the face flux at the surface crossing (ADR 0234)" (lines 1447-1456) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-309 second-order immersed forcing (ADR 0234).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Opt-in (`ImmersedForcing::build_second_order`); the forcing of ADR 0218 (`build`) is unchanged and stays what a caller gets.

| Item | Status | What was read (source) |
|---|---|---|
| The ghost-cell (ghost-fluid) closure of a first fluid cell with the extension of the fluid profile through the no-slip value at the crossing (Fedkiw, Aslam, Merriman, Osher 1999; Gibou, Fedkiw, Cheng, Kang 2002; Tseng and Ferziger 2003; Mittal et al. 2008) | from memory | NOT read; the face flux `(HbyA_c + (t_f/theta)(u_b(x_G) - HbyA_c)) . S_f` of ADR 0234 D1 is derived on rcfd's pressure equation |
| A mass source in the cells of the body so that the flow inside the body does not constrain the fluid (Kim, Kim, Choi, J. Comput. Phys. 171 (2001) 132-150) | from memory | NOT read; the source of ADR 0234 D2 (the solid cells' net outflow, passed as the volume source of ADR 0052) and the net-flux correction of D3 are derived there |
| Circular Couette flow as the measure of the order (the set-up CC of ADR 0218) | derived | as the F-219 row above; measured `e` 6.12e-3, 1.61e-3, 4.24e-4, 1.03e-4 at N 24, 48, 96, 192 (orders 1.93, 1.93, 2.04) against 8.29e-3, 3.20e-3, 1.58e-3 of the forcing of ADR 0218 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[dfg-cylinder-benchmark]]
