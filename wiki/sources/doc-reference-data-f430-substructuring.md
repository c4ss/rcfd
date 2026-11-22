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

# Reference data: F-430 substructuring: static condensation and the Craig-Bampton reduction (ADR 0469)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-430 substructuring: static condensation and the Craig-Bampton reduction (ADR 0469)" (lines 1773-1784) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-430 substructuring and Craig-Bampton reduction (ADR 0469).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-430; the two papers are cited from memory for the names and the published form. Every formula used is derived in ADR 0469 (block elimination and the Rayleigh-Ritz method); the reference of every check is the full model computed by the existing solver paths (`solve_static`, `ModalBasis::all_modes`, `modal_analysis`, `DirectHarmonic`).

| Item | Status | Source |
|---|---|---|
| Static condensation `Psi = -K_ii^{-1} K_ib`, `K_hat = K_bb + K_bi Psi`, `f_hat = f_b + Psi^T f_i`, exact recovery `u_i = Psi u_b + K_ii^{-1} f_i` | derived | block Gaussian elimination (Schur complement), ADR 0469 design; R. J. Guyan, AIAA J. 3 (1965) 380: NOT read, from memory |
| Fixed-interface reduction `T_cb = [[I, 0], [Psi, Phi]]`, `K_r = diag(K_hat, Lambda)`, `M_r` with `M_hat = M_bb + M_bi Psi + Psi^T M_ib + Psi^T M_ii Psi`, `M_bq = (M_bi + Psi^T M_ii) Phi` | derived | Rayleigh-Ritz projection on `T_cb`, ADR 0469 design; R. R. Craig and M. C. C. Bampton, AIAA J. 6 (1968) 1313: NOT read, from memory |
| Reduced frequencies bound the full ones from above; nested bases give non-increasing eigenvalues; every fixed-interface mode kept gives every frequency exactly; rigid motions of a free part carried by the constraint modes | derived | min-max principle; congruence by a square nonsingular `T_cb`; `K_q r = 0` gives `r_i = Psi r_b` (ADR 0469 design) |
| Truncation estimate of the bar (16 x 2 x 2 Hex8, two halves, five modes each): first three frequency errors 2.8e-6, 2.8e-6, 3.4e-4, axial 0.10; harmonic error above 1e-2 from 1894 rad/s, 0.85 at the axial resonance | computed | numpy 1-D family chains fitted to the full model's first frequency per family, `f430-scratch/est/estimate.py`, `band.py` (ADR 0469 arithmetic 2, 3); measured 4.5e-6, 5.5e-6, 5.6e-4, 0.0999; 1978 rad/s and 0.94 |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
