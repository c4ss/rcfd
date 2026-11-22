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

# Reference data: F-415 the Coriolis term on the modal subspace: split Campbell table and stability (ADR 0472)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-415 the Coriolis term on the modal subspace: split Campbell table and stability (ADR 0472)" (lines 1785-1801) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-415 Coriolis term on the modal subspace, split Campbell table (ADR 0472).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-415; every item is cited from memory and derived in ADR 0472 (set-ups and arithmetic 1 to 6; numpy checks `f415-scratch/pm_probe.py`, `lag_estimate.py`, `flap_estimate.py`). The references of the checks are closed forms and the existing F-396 solver path (`RotatingPart::modes`, `campbell`).

| Item | Status | Source |
|---|---|---|
| Free vibration in the rotating frame `M u_tt + 2 Omega C u_t + K_eff u = 0`, `C` skew | derived | the relative acceleration `2 Omega a x u_t` in the kinetic energy (ADR 0411 D1, ADR 0472 D1) |
| Spectrum of a gyroscopic problem symmetric about both axes; a root off the imaginary axis implies one with positive real part | derived | `det(lambda^2 M + lambda G + K)` invariant under `lambda -> -lambda` for `G^T = -G`; L. Meirovitch, Methods of Analytical Dynamics (1970): NOT read |
| Point mass on isotropic springs in the rotating frame: `|w0 - Omega|`, `w0 + Omega` | derived | `z = x + i y`: `s^2 + 2 Omega s - (w0^2 - Omega^2) = 0` (ADR 0411 set-up PM) |
| Point mass on unequal springs: unstable exactly for `(w_x^2 - Omega^2)(w_y^2 - Omega^2) < 0`, growth `sqrt(mu+)` of `mu^2 + (w_x^2 + w_y^2 + 2 Omega^2) mu + (w_x^2 - Omega^2)(w_y^2 - Omega^2) = 0` | derived | the characteristic polynomial (ADR 0472 set-up PMA); G. Genta, Dynamics of Rotating Systems (2005), for the name only: NOT read |
| Thin ring spinning about its axis: travelling waves of in-plane mode `n` split by `4 n Omega/(n^2 + 1)` in the rotating frame (whirl numbers `+/- 2 n/(n^2 + 1)`) | derived | inextensional kinetic energy in the rotating frame, Lagrange's equations (ADR 0472 set-up R); G. H. Bryan, Proc. Camb. Phil. Soc. 7 (1890) 101: NOT read |
| Inextensional ring frequency `w_n^2 = (E I/(rho A R^4)) n^2 (n^2 - 1)^2/(n^2 + 1)` | from memory | classical thin-ring result, used only for the speed scale and the foundation (set-up check `w3/w2` within 5 percent) |
| Second-order shift of a gyroscopically coupled mode `omega ~ w_i (1 - 1/2 sum_j G_ij^2/(w_j^2 - w_i^2))` | derived | perturbation of the quadratic problem in modal coordinates (ADR 0472 arithmetic 4) |
| Lag shift of the radial beam by the axial modes `Delta omega = -2 Omega^2 S omega_lag`, `S = 1.4772e-8 s^2` (L = 1 m, 0.05 m square steel); flap shift by torsion `-9.5e-8` at `0.2 w_0` | computed | Euler-Bernoulli cantilever (`beta_1 L = 1.875104068711961`, root of `cos x cosh x = -1`) and fixed-free rod modes, quadrature (ADR 0472 arithmetic 4) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
