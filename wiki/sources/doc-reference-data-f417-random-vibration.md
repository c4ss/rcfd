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

# Reference data: F-417 random vibration by modal superposition (ADR 0477)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-417 random vibration by modal superposition (ADR 0477)" (lines 1904-1918) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-417 random vibration by modal superposition (ADR 0477).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-417; the books and papers below are cited from memory for the names and the published forms. Every formula is derived in ADR 0477 (closed forms; arithmetic 1 to 6) and the references of the checks are closed forms or integrals computed by plain-Python quadrature in `f417-scratch` (`refs.py`, `refs2.py`).

| Item | Status | Source |
|---|---|---|
| Output density $\lvert Y \rvert^2 G$ of a linear system under a stationary input, cross-densities $S_{ij} = \sum_{ab} G_{ab} \operatorname{Re}(Y_{ia} \operatorname{conj} Y_{jb})$ | cited | S. H. Crandall and W. D. Mark, Random Vibration in Mechanical Systems (Academic Press 1963); D. E. Newland, An Introduction to Random Vibrations, Spectral and Wavelet Analysis (3rd ed., Longman 1993), ch. 7: NOT read, from memory |
| One mode under a flat force spectrum: $m_0 = (\pi/2) f_n Q G/k^2$, $Q = 1/(2 \zeta)$; $\int_0^\infty r^2/D dr = \int_0^\infty dr/D = \pi/(4 \zeta)$, $\nu_0 = f_n$ | derived | residues of $D = (r^2 + 2 s r + 1)(r^2 - 2 s r + 1)$, $s = \sqrt{1 - \zeta^2}$, and the substitution $r = 1/s$ (ADR 0477); J. W. Miles, J. Aeronaut. Sci. 21 (1954) 753, for the name only: NOT read |
| One mode under a flat base acceleration: absolute acceleration $m_0 = (\pi/2) f_n G (Q + 1/Q)$ | derived | the transmissibility $(1 + 2 i \zeta r)/(1 - r^2 + 2 i \zeta r)$ (ADR 0477) |
| Finite-band integrals at $\zeta = 0.02$: $I_0(10) = 39.26957079652609$, $I_1 = 38.77258066850795$, $I_2 = 39.16923599186199$, $I_4 = 49.006230409610886$, $I_0 + 4 \zeta^2 I_2 = 39.33224157411308$ | computed | 4-point Gauss-Legendre on split intervals, `f417-scratch/refs.py`, `refs2.py` |
| Two modes: CQC cross term $\rho = 8 \zeta^2 (1 + b) b^{1.5}/((1 - b^2)^2 + 4 \zeta^2 b (1 + b)^2)$ | derived | the same residues (ADR 0477); P. H. Wirsching, T. L. Paez and K. Ortiz, Random Vibrations: Theory and Practice (Wiley 1995): NOT read, from memory |
| Zero up-crossing rate $\sqrt{m_2/m_0}$, peak rate $\sqrt{m_4/m_2}$ | cited | S. O. Rice, Bell Syst. Tech. J. 23 (1944) 282 and 24 (1945) 46: NOT read, from memory (as F-413) |
| Root-mean-square von Mises stress $\sqrt{\operatorname{tr}(A C)}$, $A$ the quadratic form of $\sigma_{\mathrm{vm}}^2$, $C$ the stress covariance | derived | first principles (ADR 0477 D5); D. J. Segalman, C. W. G. Fulcher, G. M. Reese and R. V. Field, J. Vib. Acoust. 122 (2000) 42, for the definition: NOT read, from memory |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
