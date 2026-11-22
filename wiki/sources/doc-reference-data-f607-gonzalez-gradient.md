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

# Reference data: F-607 the discrete gradient of Gonzalez in the energy-momentum step (ADR 0523)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-607 the discrete gradient of Gonzalez in the energy-momentum step (ADR 0523)" (lines 2143-2154) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-607 discrete gradient of Gonzalez in the energy-momentum step (ADR 0523).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-607. The papers below are cited from memory for the method and the energy (volumes, pages and years as recalled, not checked; ADR 0523 open question 4). Every formula used is derived in ADR 0523 (the discrete gradient, D2, D3) and checked by a plain numpy probe (`f607-scratch/probe_dg.py`, `probe_branch.py` and their logs), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Discrete gradient of a stored energy of `C`: `S_alg = S(E_m) + (nu/(dE:dE)) dE`, `nu = W(E_{n+1}) - W(E_n) - S(E_m):dE`, `E_m = (E_n + E_{n+1})/2`; `S_alg : dE = W(E_{n+1}) - W(E_n)` exactly, `S_alg = S(E_m) + O(|dE|^2)`; for St. Venant-Kirchhoff `nu = 0` and `S_alg = (S_n + S_{n+1})/2` | derived, cited | ADR 0523 (the discrete gradient); O. Gonzalez, J. Nonlinear Sci. 6 (1996) 449-467; O. Gonzalez, Comput. Methods Appl. Mech. Engrg. 190 (2000) 1763-1783: NOT read, from memory |
| Compressible neo-Hookean `W = (mu/2)(J^{-2/3} I_1 - 3) + (kappa/2)(J - 1)^2`, `S = mu J^{-2/3} (2 dev E) C^{-1} + kappa (J - 1) J C^{-1}`, the tangent `2 dS/dC`; small-strain moduli `mu`, `kappa`; the cancellation-free form `I_1 - 3 J^{2/3} = 2q - 2t^2 - 8 det E + 3g^2 + g^3`, `g = J^{2/3} - 1` | derived, cited | ADR 0523 D2; J. C. Simo and R. L. Taylor, Comput. Methods Appl. Mech. Engrg. 85 (1991) 273-310; G. A. Holzapfel, Nonlinear Solid Mechanics, Wiley, 2000, chapter 6: NOT read, from memory; the form is F-607's own until F-425 (ADR 0328) chooses its energy |
| Measured: neo-Hookean spinning bar over ten turns `dP = 4.1e-13`, `dJ = 7.5e-13`, `dE = 2.1e-13`; large-strain neo-Hookean cube (500 steps) `dE = 2.8e-9` with the discrete gradient against `4.7e-3` with the average; the discrete gradient of the isotropic material equals the F-570 step within `9.4e-13` (bar) and `9.7e-14` (cube) per step | computed | ADR 0523 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
