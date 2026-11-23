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

# Reference data: F-410 linear viscoelasticity: Prony series, complex modulus, time-temperature shift (ADR 0485)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-410 linear viscoelasticity: Prony series, complex modulus, time-temperature shift (ADR 0485)" (lines 1978-1992) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-410 linear viscoelasticity (ADR 0485).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-410; the books and papers below are cited from memory for the names and the published forms. Every formula used is derived in ADR 0485 (arithmetic 1 to 8) and checked by a plain numpy prototype of the uniaxial law (`f410-scratch/probe.py`, log `probe.log`), no rcfd code. No polymer data is used: every modulus, relaxation time and shift constant of the checks is an input (decision A1 of ADR 0310).

| Item | Status | Source |
|---|---|---|
| Generalised Maxwell solid on an isotropic base, relative shear and bulk weights per branch; strain step gives $G(t) = G_0 (g_{\infty} + \sum g_i \exp(-t/\tau_i))$, `K(t)` alike, and for equal weights $E(t) = E_{\infty} + \sum E_i \exp(-t/\tau_i)$ | derived | hereditary integral of the branch stresses, ADR 0485 arithmetic 1; J. D. Ferry, Viscoelastic Properties of Polymers, 3rd ed., Wiley, 1980; N. W. Tschoegl, The Phenomenological Theory of Linear Viscoelastic Behavior, Springer, 1989: NOT read, from memory |
| Recursive update $h^{n+1} = e h^n + \gamma P (\sigma_0^{n+1} - \sigma_0^n)$, $e = \exp(-d\xi /\tau)$, $\gamma = (1 - e) \tau/d\xi$, exact for a strain linear in the step; tangent $2 G_t I_{\mathrm{dev}} + K_t 1 \otimes 1$, $G_t = G_0 (1 - \sum g_i (1 - \gamma_i))$ | derived | ADR 0485 arithmetic 1; R. L. Taylor, K. S. Pister and G. L. Goudreau, Int. J. Numer. Meth. Engng 2 (1970) 45; J. C. Simo and T. J. R. Hughes, Computational Inelasticity, Springer, 1998, chapter 10 (the chapter number not checked): NOT read, from memory |
| Creep of the standard solid under constant stress, $\varepsilon(t) = \sigma_{0} (1/E_{\infty} - (1/E_{\infty} - 1/E_0) \exp(-t/\tau_c))$, $\tau_c = \tau E_0/E_{\infty}$ | derived | the ordinary differential equation of the standard solid; probe: the recursion `1.208e-5` at 20 steps per $\tau$, order `2.0` |
| Complex modulus $G^{*}(\omega) = G_0 (g_{\infty} + \sum g_i i \omega \tau_i/(1 + i \omega \tau_i))$, $E^{*} = 9 K^{*} G^{*}/(3 K^{*} + G^{*})$; dissipation per cycle and volume $\pi \varepsilon_{0}^2 E''$ | derived | steady state of the hereditary law, ADR 0485 arithmetic 2 |
| Logarithmic decrement of a mode of the standard solid, $\delta = \pi \eta/(1 - (\omega/2)(dE'/d \omega)/E')$ to first order in the loss ($\pi E''/E'$ without the dispersion term); at $\omega \tau = 1$, $w = 2/21$: $\eta = 0.05$, $\delta/(\pi \eta) - 1 = 2.56 \times 10^{-2}$ (first order), `2.525e-2` (exact root) | derived, computed | ADR 0485 arithmetic 3; root of $s^2 + \lambda (w_{\infty} + w s \tau/(1 + s \tau)) = 0$ by Newton (probe and test) |
| Time-temperature shift: WLF $\log_{10} a_T = -c_{1} (T - T_{\mathrm{ref}})/(c_{2} + T - T_{\mathrm{ref}})$, Arrhenius $\ln a_T = (E_a/R)(1/T - 1/T_{\mathrm{ref}})$, $R = 8.314462618\ \mathrm{J/(mol\,K)}$ (CODATA 2018, exact); the "universal" WLF constants $c_{1} = 17.44$, $c_2 = 51.6\ \mathrm{K}$ used as inputs only | cited | M. L. Williams, R. F. Landel and J. D. Ferry, J. Am. Chem. Soc. 77 (1955) 3701; Ferry (1980), chapter 11 (the chapter number not checked): NOT read, from memory; the CODATA 2018 value of $R$ from memory |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
