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

# Reference data: Wave 23B interfacial mass transfer and wave references (F-226, F-227, ADR 0040)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 23B interfacial mass transfer and wave references (F-226, F-227, ADR 0040)" (lines 930-949) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 23B interfacial mass transfer and wave references (ADR 0040).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No web page and no paper was consulted in this wave. Every formula below is reproduced from knowledge and is checked in the tests only against closed forms, invariants and internal consistency, NOT against the original publication. Entries are therefore UNVERIFIED as citations unless marked "derived".

| Quantity | Source | Status |
| --- | --- | --- |
| Lee temperature-driven evaporation-condensation `mdot = r alpha rho (T - Tsat)/Tsat` | attributed to W. H. Lee (1980); title not recalled with confidence | UNVERIFIED citation; the relaxation frequencies are inputs, no default is claimed; hand values and the 0-D analytic relaxation checked (M1, M8) |
| Hertz-Knudsen flux and Schrage correction `2 sigma/(2 - sigma)` | kinetic theory of evaporation (Hertz, Knudsen, Schrage) | UNVERIFIED citation; the one-way flux `p sqrt(M/(2 pi R T))` is DERIVED here from `n m vbar/4`, `vbar = sqrt(8RT/(pi M))`, and matched to 1e-13 (M3); the Schrage factor is not independently checked |
| Pressure-driven mass transfer `3 alpha rho_v/R_B sqrt((2/3)abs(dp)/rho_l)` with `F_vap = 50`, `F_cond = 0.01`, `alpha_nuc = 5e-4`, `R_B = 1e-6 m` | attributed to Zwart, Gerber, Belamri (2004); the structure is DERIVED here from the Rayleigh-Plesset growth rate (ADR 0040 D1) | structure derived and checked (M4); the numerical factor values are UNVERIFIED and only the `Default` |
| Raoult's law, bubble and dew pressures; Henry's law with van 't Hoff dependence `H(T) = H0 exp(-C(1/T - 1/T0))` | standard thermodynamics | derived/standard; checked by sum-to-one and round-trip identities (M5, M6); no constants shipped |
| Nusselt laminar film: `delta = [4 mu k dT x/(g rho (rho - rho_v) h_fg)]^(1/4)`, mean `h = 0.943[...]^(1/4)`, `Gamma = rho (rho - rho_v) g delta^3/(3 mu)` | Nusselt (1916) film condensation theory | derived here from the film momentum and energy balance; the coefficient `(4/3)(1/4)^(1/4) = 0.9428` is computed; the Rohsenow correction `h_fg + 0.68 c_p dT` is UNVERIFIED; the water property values in the test are round test inputs, not data claims |
| Dispersion `omega^2 = g k tanh(kh)`, phase and group velocity, Airy kinematics, energy density `rho g a^2/2`, mean flux `E/c` | linear wave theory | derived/standard; checked against deep and shallow limits, kinetic-energy equipartition, continuity, Laplace (W1, W2) |
| Second-order Stokes: `eta2 = (k a^2/4) cosh kh (2 + cosh 2kh)/sinh^3 kh`, `u2`, `w2`, `phi2 = (3/8) a^2 omega cosh 2k(z+h)/sinh^4 kh sin 2 theta` | textbook Stokes expansion (Dean and Dalrymple type) | UNVERIFIED citation; the formulas are VALIDATED here by satisfying the kinematic and dynamic free-surface conditions through second order (the residual is third order, W3b ratios 8.00) and by the deep and shallow limits |
| Pierson-Moskowitz in `Hs`-`wp` form `S = (5/16) Hs^2 wp^4 w^-5 exp(-1.25 (wp/w)^4)`; moments `m_n = (5/64) Hs^2 wp^n 1.25^((n-4)/4) Gamma(1 - n/4)` | form attributed to Bretschneider / Pierson-Moskowitz | `m0 = Hs^2/16` and the moments are DERIVED here (substitution `s = 1.25 (wp/w)^4`) and checked numerically (W5); the attribution is UNVERIFIED |
| JONSWAP shape `gamma^r`, `r = exp(-(w - wp)^2/(2 s^2 wp^2))`, `s = 0.07/0.09`, `gamma = 3.3` | Hasselmann et al. (1973) | UNVERIFIED citation; the constant `A` is replaced by a numerical normalisation to `Hs` |
| Relaxation-zone exponential blending `1 - (exp(chi^3.5) - 1)/(e - 1)` | attributed to Jacobsen, Fuhrman, Fredsoe (2012) | UNVERIFIED citation; endpoint exactness and monotonicity checked (W6); the reflection result is a toy-model measurement (W7), not a validation |
| Linear shallow-water leapfrog tank | first principles | derived; the incident amplitude and the rigid-wall reflection `R = 1.00000` are internal consistency checks (W7) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
