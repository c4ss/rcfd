---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 28A VOF wiring: AMG, mass transfer, wave inlet and beach (F-055, F-226, F-227, ADR 0052)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 28A VOF wiring: AMG, mass transfer, wave inlet and beach (F-055, F-226, F-227, ADR 0052)" (lines 1133-1150) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 28A VOF wiring references (ADR 0052).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read in this wave. Everything below is first-principles or recalled. The earlier entries for the same references (waves 23B, 25B) keep their status.

| Item | Status | Note |
|------|--------|------|
| Smoothed-aggregation AMG (Vanek, Mandel, Brezina 1996; Stuben 2001 survey): strength filter, weak couplings lumped to the diagonal, damped-Jacobi smoothed prolongator | unverified | structure recalled, not read; the defect found (zero filtered diagonal) is proven by a reproducer in `rcfd-linalg/tests/amg_contrast.rs`, and the fix is validated by convergence, not by a citation |
| Variable-density pressure equation `div(gamma grad p) = div(phiHbyA)` with `gamma = V/a_p`, 1000:1 jump | derived | |
| Lee model `mdot = c alpha rho (T - T_sat)/T_sat`, its exponential-integrator form for a frozen temperature | derived / unverified | the closed form `alpha(t) = alpha_0 exp(-c theta t)` is derived and checked to 1e-12; the attribution and the model form remain as listed in wave 25B (still UNVERIFIED) |
| Volume source `mdot (1/rho_v - 1/rho_l)` and the mass balance `rho_l S_alpha = -mdot` | derived | continuity residual checked to 1e-6 |
| Hertz-Knudsen flux, Schrage factor, Clausius-Clapeyron `p_sat(T)` with constant `h_fg` | derived / unverified | one-way flux derived from kinetic theory (checked 1e-9 in the M tests); the Schrage factor and the Clausius-Clapeyron constant-`h_fg` form are recalled, not read |
| Airy / second-order Stokes inlet values, ramp, exact liquid-fraction helper | as in wave 23B | the library formulas are those of `rcfd-fv/src/waves.rs`; the third-order residual scaling was checked there |
| Relaxation zone `w computed + (1 - w) target` with the exponential weights (Jacobsen, Fuhrman, Fredsoe 2012) | unverified | the form is the one of wave 23B; its attribution is DOUBTFUL (wave 25B); the PDF was not read |
| Goda and Suzuki (1976) two-gauge incident / reflected separation | unverified | the 2 x 2 complex system is DERIVED here (`B_i = A_I e^{ikx_i} + A_R e^{-ikx_i}`) and solved directly; the paper was not read; the gauge spacing is 0.25 wavelength, away from the singularities at multiples of half a wavelength |
| Damping estimates: implicit Euler `omega^2 dt/2`, first-order upwind `2 (|u| dx/2) k^2` | derived | used in the ADR to state the wave-height tolerances before the runs |
| Wave-tank results (heights, periods, reflection) | measured | numerical results of this code on a coarse mesh (lambda/32, 10 and 20 periods), no comparison with published tank data |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
