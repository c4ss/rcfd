---
type: source
tags: [reference-data, f-item-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-221 non-equilibrium condensing steam (ADR 0217)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-221 non-equilibrium condensing steam (ADR 0217)" (lines 1535-1557) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-221 non-equilibrium condensing steam references (ADR 0217).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


The equations and the nozzle data were read from the sources below (page images or extracted text; the PDFs are kept outside the repository). SHA-256 of the files read: GT2016-57598 accepted manuscript `172644ad36a846a67a01d0dc1a94aae8f62ceabc05b40a9f242f2d4ff23c4ae3`, GT2011-45672 `f77c3faa910dac090e693eeda16cb63b89ad836c2a46ba6df7e07311ab61fa32`, Afzalifar et al. 2016 `bcbaa6e78b8f441fc62cb118138dd6c3eb45eed76303aa548687939ea67f597f`, Dykas et al. 2013 `c70aff5280c5a098184e9fb9a0e630593c1592497d312ba40c647f5a02e02c94`, IAPWS R1-76(2014) `77d07ae4c6d473806b73f982d4c031b927db02a5718bd3c25cd7a03c3321587d`.

| Item | Status | What was read (source) |
|---|---|---|
| Nucleation rate per unit mass of mixture `J = q_c/(1+phi) sqrt(2 sigma/(pi m^3)) rho_g^2/(rho rho_f) exp(-4 pi sigma r*^2/(3 k_B T_g))`, `q_c = 1`; Young's growth `dr/dt = lambda_g/(rho_f L r) (T_s - T_g)(1 - r*/r)/(1 + 3.78 (1 - nu) Kn/Pr)`, `nu = R T_s/L (alpha - 0.5 - (2 - q_c)/(2 q_c) ((gamma + 1)/(2 gamma)) (c_p T_s/L))`; `alpha = 11` | read | Starzmann, Hughes, White, Grübel, Vogt, ASME GT2016-57598, Eqs. 1-3 and text (accepted manuscript, University of Cambridge Apollo repository, handle 1810/256656) |
| Moses and Stein nozzle: depth 10 mm, throat height 10 mm, wall arcs R = 53 mm and R = 686 mm (Fig. 1); case 203 `p_t1 = 35.87 kPa`, `T_t1 = 368.3 K`; 41 centreline pressures and 17 Sauter radii (Fig. 2, data taken by the authors from Young 1982) | read (digitized) | the same paper, Figs. 1 and 2 digitized from their vector paths with the figures' own tick marks (ADR 0217); the values are tabulated in ADR 0217 and in `crates/rcfd-fv/tests/wetsteam.rs`. Moses and Stein (1978) and Young (1982) were not read |
| The Moses and Stein nozzle's transonic and supersonic sections are one circular arc; throat height 0.01 m | read | Afzalifar, Turunen-Saaresti, Grönman, ISROMAC 2016 (HAL hal-01891315), section 4 |
| Critical radius `r* = 2 sigma/(rho_d R T_v ln S)`, `S = p_v/p_s`; single-fluid mixture `rho_m = rho_v/(1 - Y)`, `E_m = Y E_d + (1 - Y) E_v` | read | the same paper, Eqs. 1, 5-8 |
| Kantrowitz correction `C = [1 + 2 (gamma-1)/(gamma+1) (h_v - h_l)/(R T_v) ((h_v - h_l)/(R T_v) - 1/2)]^-1`; `h_m = (1 - y) h_v + y h_l` | read | Dykas, Majkut, Smołka, Strozik, J. Power Technologies 93(5) (2013) 288-294, Eqs. 2 and 5 (open access) |
| Knudsen number `Kn = l/(2r)`, mean free path `l = 1.5 mu sqrt(R T_g)/p`, `Pr = mu c_p/lambda` | read | Starzmann, Schatz, Casey, Mayer, Sieverding, ASME GT2011-45672, Eqs. 6-9 |
| Surface tension `sigma = 235.8 mN/m tau^1.256 (1 - 0.625 tau)`, `tau = 1 - T/647.096 K`; Table 1 column 4; extrapolation to -25 °C | read | IAPWS R1-76(2014), pages 3-4 |
| Boltzmann constant `1.380 649 x 10^-23 J/K`, exact | read | NIST CODATA, physics.nist.gov/cgi-bin/cuu/Value?k, 2026-11-07 |
| Saturation line, liquid density, latent heat, `c_p`, vapour viscosity and conductivity | existing | IAPWS-IF97 and the IAPWS 2008/2011 transport correlations as implemented and verified in `rcfd-core::iapws` (F-111); the saturation equation and region 1 are extrapolated below 273.15 K only in transients (ADR 0217 M3) |
| Moore nozzle B geometry | not read | stated in GT2016-57598 to be "available in the IWSMP collaboration"; not printed in any source read |
| F-320 checks (ADR 0226): rates decomposition, dry nucleation box, frozen sound speed, HLLC upwinding | no new data | the reference values are ADR 0217's M1 table (the F-221 prototype) and first principles (isentropic derivative of the M2 equation of state, the upwind branch of HLLC, Toro ch. 10); no new source was read |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
