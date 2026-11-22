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

# Reference data: F-423 nonlinear transient dynamics, Newton inside the Newmark and generalised-alpha step (ADR 0479)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-423 nonlinear transient dynamics, Newton inside the Newmark and generalised-alpha step (ADR 0479)" (lines 1932-1945) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-423 nonlinear transient dynamics (ADR 0479).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-423; the books and papers below are cited from memory for the names and the published forms. Every formula used is derived in ADR 0479 (arithmetic 1 to 8, D1) and the reference values are closed forms evaluated by plain Python in `f423-scratch` (`reg_arith.py`, log `reg_arith.log`); the probes of the registration are two-node truss prototypes (`probe_truss.py`, `probe2.py` to `probe5.py`), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Step with the internal force averaged at its end values, $M a_m + C v_f + (1 - \alpha_f) f_{\mathrm{int}}(u_{n+1}) + \alpha_f f_{\mathrm{int},n} = (1 - \alpha_f) f_{n+1} + \alpha_f f_n$, unknown `u_{n+1}`, $a_{n+1} = (u_{n+1} - \tilde{u})/(\beta h^2)$; $\rho_{\infty} = 1$ is the trapezoidal rule | derived | ADR 0479 D1 and arithmetic 7; J. Chung and G. M. Hulbert, J. Appl. Mech. 60 (1993) 371, N. M. Newmark, J. Eng. Mech. Div. ASCE 85 (1959) 67 (as ADR 0389); D. Kuhl and M. A. Crisfield, IJNME 45 (1999) 569, S. Erlicher, L. Bonaventura and O. S. Bursi, Comput. Mech. 28 (2002) 83, for the averaged form: NOT read, from memory |
| Compound pendulum released from the horizontal: $T = 2 \pi \sqrt{2 L/(3 g)}\, (2/\pi)\, K(\sin 45^\circ)$, $K(1/\sqrt{2}) = 1.854074677301372$, factor `1.180340599016096`; $L = 1\ \mathrm{m}$, $g = 9.81$: $T = 1.933334854373\ \mathrm{s}$ | derived, computed | energy integral of $I = m L^2/3$, torque $m g (L/2) \sin \theta$; $K$ by the arithmetic-geometric mean and a 20000-point midpoint quadrature (`reg_arith.log`); L. D. Landau and E. M. Lifshitz, Mechanics, section 11, for the elliptic form: NOT read, from memory |
| Large axial oscillation of one St. Venant-Kirchhoff bar element, $\nu = 0$, from $\lambda = 1.2$: $T/T_{\mathrm{lin}} = (1/\pi) \int_{-\pi/2}^{\pi/2} d\phi /\sqrt{1 + 0.44 \sin \phi} = 1.0407443288903$, $T_{\mathrm{lin}} = 2 \pi \sqrt{\rho L^2/(3 E)}$, $\lambda_{\mathrm{min}} = \sqrt{0.56}$ | derived, computed | energy $(\rho A L/6) \dot{q}^2 + (E A L/8)(\lambda^2 - 1)^2$, the substitution $\lambda^2 - 1 = 0.44 \sin \phi$; midpoint rule (64 to 4096 points) and the binomial series agree to `3e-15` (`reg_arith.log`) |
| Conservation of $P = \sum p_a$ and $J = \sum x_a \times p_a$, $p = M v$, for the consistent mass and an objective internal force; the trapezoidal rule keeps $P$ and, in general, not $J$ | derived, cited | ADR 0479 arithmetic 3; J. C. Simo and N. Tarnow, ZAMP 43 (1992) 757, J. C. Simo, N. Tarnow and K. K. Wong, CMAME 100 (1992) 63, for the non-conservation and the instability of the trapezoidal rule in nonlinear elastodynamics: NOT read, from memory |
| Probes (truss): pendulum $dt = 0.005\ \mathrm{s}$, $E = 2 \times 10^{9}$: $T/T_0 = 1.180504$, energy `2.76e-4` of `m g L/2`; spinning truss from the steady spin, Newmark: $\lvert dJ \rvert/\lvert J \rvert = 7.2 \times 10^{-12}$, unstretched start, generalised-alpha 0.9: `1.6e-3` | computed | `probe4.log`, `probe5.log` (ADR 0479 arithmetic 2, 4) |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
