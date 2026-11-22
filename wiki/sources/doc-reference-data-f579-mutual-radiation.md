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

# Reference data: F-579 mutual radiation impedance and light-fluid modes of a baffled plane (ADR 0538)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-579 mutual radiation impedance and light-fluid modes of a baffled plane (ADR 0538)" (lines 1758-1772) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-579 mutual radiation impedance and light-fluid modes (ADR 0538).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-579; every item is cited from memory and derived in ADR 0538 (D1 to D5; numpy checks `f579-scratch/estimate.py`). The time convention is F-551's, $x(t) = \operatorname{Re}(X e^{+i \omega t})$, outgoing $e^{i(\omega t - k r)}$. No measured data.

| Item | Status | Source |
|---|---|---|
| Mutual impedance of two small baffled pistons $z_{12} = F_2/U_1 = (i \omega \rho/(2 \pi)) S_{1} S_{2} e^{-i k d}/d$ in the $e^{+i \omega t}$ convention; with $Z_{mn} = (1/2) \int P[V_m] \operatorname{conj}(V_n) dS$, $z_{12} = 2 Z_{12}/(U_{1} \operatorname{conj} U_{2})$; no conjugation against F-551 | derived | the baffled simple source $P = (i \omega \rho/(2 \pi)) U S e^{-i k r}/r$ (Rayleigh integral of F-418/F-551) integrated over the second piston; R. L. Pritchard, J. Acoust. Soc. Am. 32 (1960) 730 (journal page uncertain): NOT read, from memory |
| The same in the $e^{-i \omega t}$ convention: $(-i \omega \rho/(2 \pi)) S_{1} S_{2} e^{+i k d}/d$ (complex conjugate) | derived | conjugation of the time factor; attributed from memory to A. D. Pierce, *Acoustics*, and P. M. Morse and K. U. Ingard, *Theoretical Acoustics* ($e^{-i \omega t}$), and to L. E. Kinsler et al., *Fundamentals of Acoustics* ($e^{j \omega t}$): NOT read, open question ADR 0538 OQ2 |
| Reciprocity of the discrete matrix: $Z_{mn} = Z_{nm}$ for real fields, $Z_{nm}[V] = Z_{mn}[\operatorname{conj} V]$ for complex ones | derived | the symmetric kernel $e^{-i k R}/R$ (ADR 0538 D1) |
| Modal radiation impedance $Zr_{nm} = \int P[\phi_m] \phi_n dS = C + i \omega M_a$ (unit modal velocity), added mass $M_a = \operatorname{Im} Zr/\omega$, damping $C = \operatorname{Re} Zr$ | derived | modal projection of the fluid force (ADR 0538 D3); M. C. Junger and D. Feit, *Sound, Structures, and Their Interaction*, and F. Fahy and P. Gardonio, *Sound and Structural Vibration*: NOT read, from memory |
| Light-fluid first-order shift $\omega \approx \omega_1 (1 - M_{a,11}/(2 \mu_1))$; one mode exactly $\omega_1/\sqrt{1 + \varepsilon}$, so $\Delta_c/\Delta_{\mathrm{first}} = 1 - 3 \varepsilon/4 + O(\varepsilon^2)$ | derived | Rayleigh quotient with the added mass (ADR 0538 D4) |
| Spectral form $Zr_{mn} = (\omega \rho/(4 \pi^2)) [\int_{\kappa<k} \hat{V}_m \operatorname{conj} \hat{V}_n/\sqrt{k^2 - \kappa^2} + i \int_{\kappa>k} \hat{V}_m \operatorname{conj} \hat{V}_n/\sqrt{\kappa^2 - k^2}] d^2 \kappa$ | derived | plane-wave expansion of the half-space field, Euler equation at the plane, Parseval (ADR 0538 D5) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
