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

# Reference data: F-598 the warping degree of freedom of Beam2 and the I-section lateral-torsional buckling (ADR 0546)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-598 the warping degree of freedom of Beam2 and the I-section lateral-torsional buckling (ADR 0546)" (lines 2197-2213) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-598 warping DOF of Beam2 and lateral-torsional buckling (ADR 0546).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-598. The books below are cited from memory for the names and the published forms (editions, chapters and years as recalled, not checked). Every formula used is derived in ADR 0546 (D1, D2, arithmetic 1 to 4) and checked by plain numpy scripts (`f598-scratch/arith.py`, `arith2.py` and their logs), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Non-uniform torsion: warping displacement $-\omega \phi'$, energy $(1/2) \int (G J \phi'^2 + E C_w \phi''^2) dx$, $C_w = \int \omega^2 dA$ | derived, cited | ADR 0546 D1; V. Z. Vlasov, Thin-Walled Elastic Beams (1940; English translation 1961); S. P. Timoshenko and J. M. Gere, Theory of Elastic Stability, 2nd ed., McGraw-Hill 1961, ch. 5: NOT read, from memory |
| Warping constant of a thin-walled doubly symmetric I $C_w = t_f b_f^3 h_c^2/24 = I_f h_c^2/2$ ($h_c$ between the flange mid-planes; $I_z h^2/4$ with the web neglected) | derived, cited | ADR 0546 D1 (the flange sectorial coordinate $y h_c/2$); Timoshenko and Gere, op. cit., ch. 5; N. S. Trahair, Flexural-Torsional Buckling of Structures, E & FN Spon 1993, ch. 2: NOT read, from memory |
| Wagner coefficients $\beta_y = \int z (y^2 + z^2) dA/I_y$, $\beta_z = \int y (y^2 + z^2) dA/I_z$, zero for a section with two axes of symmetry | derived, cited | ADR 0508 D1, ADR 0546 D1; Trahair, op. cit., ch. 6 (the monosymmetry parameter $\beta_x$, about the shear centre): NOT read, from memory; the shear-centre form is not used (Limits of ADR 0546) |
| Lateral-torsional buckling under uniform moment, fork supports, warping free: $M_{\mathrm{cr}} = (\pi/L) \sqrt{E I_z G J (1 + \pi^2 E C_w/(G J L^2))}$ | derived, cited | ADR 0546 arithmetic 1 ($v = a \sin kx$, $\phi = b \sin kx$); Timoshenko and Gere, op. cit., ch. 6: NOT read, from memory |
| Lateral and warping both fixed: $M = (2 \pi/L) \sqrt{E I_z (G J + 4 \pi^2 E C_w/L^2)}$ (the upper bound of N3) | derived | ADR 0546 arithmetic 1 (mode $1 - \cos(2 \pi x/L)$) |
| Cantilever under end torque with the root warping held: $\phi(L) = (T/(G J))(L - \tanh(kL)/k)$, $k = \sqrt{G J/(E C_w)}$ | derived | solution of $G J \phi' - E C_w \phi''' = T$, $\phi(0) = \phi'(0) = 0$, $\phi''(L) = 0$ (ADR 0546 arithmetic 4) |
| Cubic Hermite twist element: `G J/(30 L)[36, 3L, -36, 3L; ...]`, $E C_w/L^3 [12, 6L, -12, 6L; \dots ]$ | derived | ADR 0546 D2, arithmetic 3 |
| Measured: I beam, $L = 3$, 16 elements `5.9e-4` of the closed form (8: `8.3e-4`; $L = 1.5$: `1.1e-3`; $L = 6$: `2.0e-4`); $C_w = 0$ `5.9e-4` of $M_0$; torque cantilever `1.1e-6` | computed | ADR 0546 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
