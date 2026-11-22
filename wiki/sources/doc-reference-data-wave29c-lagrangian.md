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

# Reference data: Wave 29C Lagrangian extensions (F-128, F-056, ADR 0057)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 29C Lagrangian extensions (F-128, F-056, ADR 0057)" (lines 1183-1200) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 29C Lagrangian extension references (ADR 0057).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read in this wave. Every reference below is recalled from memory and is unverified unless marked derived. No vendor binary, install or text was used.

| Item | Status | Note |
|------|--------|------|
| Gosman and Ioannides (1983) eddy-interaction (discrete random walk) model | unverified | the structure (Gaussian velocity fluctuation held for min(eddy lifetime, crossing time)) is recalled; the paper was not read |
| Constants `C_L = 0.15` (integral time `T_L = C_L k/eps`) and `C_mu = 0.09` (eddy length `C_mu^(3/4) k^(3/2)/eps`) | unverified | values recalled from common practice; `C_L` is a case key (`dispersion_cl`), not a validated constant |
| Variance of a Stokes particle in a fluid velocity with autocorrelation `sigma^2 exp(-s/T_L)`: `sigma_p^2 = sigma^2 T_L/(T_L + tau_p)` | derived | double integral `int int exp(-(a+b)/tau) exp(-abs(a-b)/T) = tau^2 T/(T+tau)` worked out in the ADR; consistent with the Tchen/Hinze ratio `1/(1 + tau/T_L)` recalled from memory (not read) |
| Exponential eddy lifetimes with independent Gaussian redraws give autocorrelation exactly `exp(-s/T_L)` | derived | probability that no redraw occurred in the lag |
| Eddy crossing time `-tau ln(1 - L_e/(tau abs(u - v)))` | derived | from Stokes relative motion `tau w (1 - exp(-t/tau))` set equal to `L_e` (the recalled published form is the same) |
| Ranz and Marshall (1952) `Nu = 2 + 0.6 Re^(1/2) Pr^(1/3)` | unverified | coefficients recalled from memory (secondary sources earlier in this file for related correlations); lumped-sphere relaxation time is derived |
| Rosin-Rammler mass CDF `1 - exp(-(d/d_bar)^n)` | verified in two secondary sources in wave 18C (ADR 0028) | sampling by inversion and the equal-mass parcel weighting are derived |
| Kolmogorov-Smirnov critical value 1.95/sqrt(N) (99.9 percent) | unverified | recalled from memory; used only to set a statistical bound, the constant 1.949 is the usual two-sided tail value |
| Momentum and total-energy exchange definitions (drag impulse, `-(Delta KE - W_g + m c_p Delta T)`) | derived | conservative by construction; no source |
| Particle-laden jet, spray penetration, falling-film thickness (Nusselt), any published experiment | not used | no comparison attempted |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
