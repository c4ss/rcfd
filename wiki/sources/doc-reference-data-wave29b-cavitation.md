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

# Reference data: Wave 29B mixture and cavitation (F-127, ADR 0056)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 29B mixture and cavitation (F-127, ADR 0056)" (lines 1219-1239) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 29B mixture and cavitation references (ADR 0056).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read in this wave. Every reference below is recalled from memory and is UNVERIFIED. No vendor code, binary or install was used. No published cavitation data (cavitating venturi, hydrofoil sheet length, nozzle) and no bubble-column void-fraction profile was compared: NOT ATTEMPTED. Full multi-fluid Euler-Euler (per-phase momentum, lift, virtual mass, turbulent dispersion) is NOT delivered.

| Item | Status | Note |
|------|--------|------|
| Mixture (algebraic-slip) model: slip flux $\alpha (1-\alpha)\, v_{12}$, drift mass flux and drift stress in the mixture momentum | unverified | the model class (Ishii drift flux, Manninen et al. 1996 mixture model) is recalled from memory; the equations used are DERIVED in ADR 0056 from the volume-averaged mixture balances and checked against analytic divergences (M3 order 1.99) |
| Manninen 1996 relative velocity with Stokes factor $f = 1$; Schiller-Naumann $f = 1 + 0.15\, Re^{0.687}$ (1935) | unverified | the closure form and the 0.15 and 0.687 constants are recalled from memory; Stokes terminal velocity itself is derived (D2 1.5e-16, D3 balance 6.4e-16) |
| Kynch batch-settling front (kinematic wave) | unverified | recalled (Kynch 1952); the front speed used in D4/D5 is the DERIVED flux $\alpha v$ characteristic; D4_corrected front 0.234 dx, D5_corrected 0.432 dx |
| Schnerr-Sauer cavitation model, $\alpha_v = x/(1+x)$, $x = n\, \tfrac{4}{3} \pi R^3$, $\dot{m} = \frac{\rho_v \rho_l}{\rho_m}\, \alpha(1-\alpha)\, \frac{3}{R}\, R'$ | unverified | the model is recalled from Schnerr and Sauer (2001); the algebra is checked here as an identity $D\alpha_v/Dt = \dot{m}\, \rho_m/(\rho_v \rho_l)$ (C1 exact); the bubble-number density `n = 1e11` and nucleus radius `1e-5 m` are set by the case, not taken from a source |
| Rayleigh bubble growth $R' = \sqrt{2 (p_{sat} - p)/(3 \rho_l)}$ (Rayleigh 1917; Plesset 1949 adds viscosity and surface tension) | unverified | recalled; the inertial limit is derived from the energy balance of the liquid shell; the Rayleigh-Plesset integral comparison C4c (first integral 3.95e-10; $R_{SS} - R_{RP}$ within `[0.56, 0.57] R_0`) is against the equation without viscosity or surface tension |
| Saturation pressure of water, $p_{sat}(293\ \mathrm{K}) = 2339\ \mathrm{Pa}$ | unverified | recalled from memory (steam-table value); used as a case input |
| Bernoulli pressure drop in the venturi throat, $p = p_{in} - 0.5\, \rho (u_t^2 - u_{in}^2)$ | derived | N1: throat pressure 99.3 Pa against Bernoulli-from-inlet 488.9 Pa (limit 1250 Pa); minimum throat-region pressure -1349.9 Pa < $p_{sat}$, so inception is expected |
| Mixture mass and volume conservation | derived | D1_corrected volume 1.7e-15, mass 2.4e-16; M4b per-cell mass balance 7.9e-15 |
| Cavitating venturi (2-D, 80 x 20): bounded vapour fraction | derived | N2: $\alpha_v$ in `[4.19e-4, 0.99993]` over 6000 steps; periodic cavity growth and collapse with a water-hammer-like pulse (ADR 0056 addendum) |
| Cavitating venturi N3 end of run, N4, N4', N5 | MISS | known-miss tests with the registered limits intact: end-of-run phase (cavity collapsed), advection into $p \ge p_{sat}$ cells (31763 events), end-of-step source sign (16 events), global balance 1.96e-5 against 1e-6 |
| Void-fraction profile of a bubble column, sheet-cavity length of a hydrofoil | not used | no comparison attempted |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
