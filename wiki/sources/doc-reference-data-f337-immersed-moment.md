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

# Reference data: F-337 immersed solid: second-order moment of the forcing (ADR 0239)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-337 immersed solid: second-order moment of the forcing (ADR 0239)" (lines 1457-1467) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-337 immersed solid second-order moment of the forcing (ADR 0239).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Opt-in (`ImmersedForcing::force_moment_second_order`); `force_moment` is unchanged and stays what a caller gets.

| Item | Status | What was read (source) |
|---|---|---|
| The force and moment on a body from the momentum and angular-momentum balance of a control volume (Batchelor, An Introduction to Fluid Dynamics, 1967, section 3.2), and the force on an immersed boundary from its forcing and the momentum of the forced cells (Lai and Peskin, J. Comput. Phys. 160 (2000) 705-719; Balaras, Comput. Fluids 33 (2004) 375-404) | from memory | NOT read; the statement of ADR 0239 D1 (the moment of the forcing with every term at the centre of its cell is the moment of the full traction and of the momentum flux on the regular fluid faces around the forced cells) is derived there on rcfd's discrete momentum equation |
| The identity `integral x x (grad u^T n) dS = -integral n x u dS` for a divergence-free field on a closed surface | derived | ADR 0239 D1 (Stokes' theorem on a closed surface) |
| Circular Couette torque `4 pi mu omega R1^2 R2^2/(R2^2 - R1^2) = 4.18879` as the measure of the order of the moment | derived | as the F-219 row above; measured error 0.396, 0.107, 0.027 percent at N 24, 48, 96 (orders 1.89, 1.98) against 3.46, 1.60, 0.74 percent of `force_moment` |
| DFG 2D-1 reference values | read | as the F-219 row above; the force is that of `force_moment` (M2, second-order forcing: `C_D` -0.306 percent, `C_L` +2.534 percent) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[dfg-cylinder-benchmark]]
