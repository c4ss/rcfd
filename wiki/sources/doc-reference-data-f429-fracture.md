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

# Reference data: F-429 fracture parameters: domain integral, stress intensity factor, growth life (ADR 0547)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-429 fracture parameters: domain integral, stress intensity factor, growth life (ADR 0547)" (lines 2283-2299) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-429 fracture parameters (ADR 0547).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-429. The books and papers below are cited from memory (volumes, pages and years as recalled, not checked; ADR 0547 open question 3). Every formula used is derived in ADR 0547 (D3 to D7, arithmetic 1 to 9) from first principles; the reference values of the checks are closed forms evaluated in the tests.

| Item | Status | Source |
|---|---|---|
| J-integral $J = \int_{\Gamma} (W n_1 - \sigma_{ij} n_j u_{i,1}) ds$, path independence | derived, cited | ADR 0547 D3; J. R. Rice, J. Appl. Mech. 35 (1968) 379-386: NOT read, from memory |
| Equivalent domain integral $\int_V (\sigma_{ij} u_{i,k} - W \delta_{jk}) q_{k,j} dV$ with a q-function over rings of elements | derived, cited | ADR 0547 D3; F. Z. Li, C. F. Shih and A. Needleman, Eng. Fract. Mech. 21 (1985) 405-421; C. F. Shih, B. Moran and T. Nakamura, Int. J. Fract. 30 (1986) 79-102; G. G. deLorenzi, Int. J. Fract. 19 (1982) 183-193: NOT read, from memory (the deLorenzi citation is the least certain) |
| Irwin's relation $G = K_{\mathrm{I}}^2 (1 - \nu^2)/E$ in plane strain; the near-tip opening $\delta = 8 K_{\mathrm{I}} (1 - \nu^2)/E \sqrt{r/(2 \pi)}$ | derived, cited | ADR 0547 D5, D6; T. L. Anderson, Fracture Mechanics: Fundamentals and Applications, 3rd ed., CRC/Taylor and Francis, 2005, chapter 2: NOT read, from memory |
| Centre crack in a strip, secant form $K = \sigma \sqrt{\pi a} \sqrt{\sec(\pi a/(2 W))}$ (Feddersen) and the refinement $(1 - 0.025 l^2 + 0.06 l^4) \sqrt{\sec(\pi l/2)}$; at $a/W = 0.1$ the two differ by `0.024 %` | cited | ADR 0547 arithmetic 1; C. E. Feddersen, ASTM STP 410 (1966) 77-79; H. Tada, P. C. Paris and G. R. Irwin, The Stress Analysis of Cracks Handbook, 3rd ed., ASME, 2000: NOT read, from memory (the refinement's coefficients and its stated accuracy of 0.1 % are recalled, not checked) |
| Westergaard crack-face opening $4 \sigma (1 - \nu^2)/E \sqrt{a^2 - x^2}$ (plane strain, infinite plate) | derived, cited | ADR 0547 arithmetic 3; H. M. Westergaard, J. Appl. Mech. 6 (1939) A49-A53: NOT read, from memory |
| Penny-shaped crack $K_{\mathrm{I}} = 2 \sigma \sqrt{a/\pi}$ in an infinite body | cited | ADR 0547 arithmetic 7; I. N. Sneddon, Proc. R. Soc. Lond. A 187 (1946) 229-260; Tada, Paris and Irwin (above): NOT read, from memory |
| Paris law $da/dN = C (\Delta K)^m$ and its closed-form life for $\Delta K = d \Delta \sigma \sqrt{\pi a}$ | derived, cited | ADR 0547 D7; P. C. Paris and F. Erdogan, J. Basic Eng. 85 (1963) 528-534: NOT read, from memory |
| Finite-body effects not in the references: the height $H = 2 W$ of the plate (estimated `< 0.1 %`), the cylinder $R = H = 10 a$ of the penny crack (estimated `~ 1e-3`) | estimate | ADR 0547 arithmetic 1 and 7: estimates, not references |
| Measured: plate $K$ from J of rings 2 to 4 within `2.1e-3` of the secant form, ring spread `1.2e-3`, energy difference `7.4e-5`, opening `1.2e-2`, shear opening `1.3e-13`; penny ring 3 within `2.2e-3`, along the front `2.7e-7`; Paris $\le 6.7 \times 10^{-16}$ | computed | ADR 0547 addendum 1 |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
