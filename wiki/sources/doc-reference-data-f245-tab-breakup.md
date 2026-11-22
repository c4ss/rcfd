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

# Reference data: F-245 droplet evaporation and TAB breakup (ADR 0224)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-245 droplet evaporation and TAB breakup (ADR 0224)" (lines 1558-1579) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-245 droplet evaporation and TAB breakup constants (ADR 0224).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Read 2026-11-08 for the registration of ADR 0224, before any code. No source listing of any spray or CFD code was opened (web searches listed source pages of open-source and commercial implementations of the TAB model and theory pages of a commercial code; none was opened). Nothing in F-245 is compared with measured data; the liquid and gas values of the checks are round test inputs. SHA-256 of the files read: LA-UR-87-2105-Rev. scan `ccc109b4b1f663149641924b4f67fa78ec43dac5b5af5cb4050ab965af4f9bef`, arXiv:2201.02818v1 `52c7b9dd554ccd34d2150a68fbdb5717710a5bf25ada3ed1495283993c1a86c4`, arXiv:2510.13033v2 `7f4dc3615150dd83a1e8a1d99435f7173c619ebe08a89732fc4b04b655d64cd3`.

| Item | Status | What was read (source) |
|---|---|---|
| TAB equation $\ddot y = \dfrac{C_F}{C_b}\dfrac{\rho_g}{\rho_l}\dfrac{u^2}{r^2} - \dfrac{C_k \sigma}{\rho_l r^3}\, y - \dfrac{C_d \mu_l}{\rho_l r^2}\, \dot y$, breakup at $y = 1$; $We = \rho_g u^2 r/\sigma$, $1/t_d = (C_d/2)\, \mu_l/(\rho_l r^2)$, $\omega^2 = C_k \sigma/(\rho_l r^3) - 1/t_d^2$; constants $C_k = 8$ (Lamb, fundamental frequency), $C_d = 5$ (Lamb, damping), $C_b = 1/2$, $C_F = 1/3$, $We_{crit} = 6$ (shock experiments), $C_k C_b/C_F = 12$; the numerical implementation (eqs. 25a, 25b, the amplitude test $We/12 + A \le 1$, the breakup time of eq. 26 "assuming that the drop oscillation is undamped for its first period", $N^{n+1} = N^n (r^n/r^{n+1})^3$, $y = \dot y = 0$ after breakup) | read | P. J. O'Rourke, A. A. Amsden, "The TAB method for numerical calculation of spray droplet breakup", Los Alamos report LA-UR-87-2105-Rev. (SAE paper 872089, 1987), eqs. 1 to 10, 25, 26 and the section "Numerical implementation"; scan at the UNT Digital Library (`digital.library.unt.edu/ark:/67531/metadc1106123/m2/1/high_res_d/6118786.pdf`). The first line of the printed eq. 4 is too faint in the scan; it was derived from eq. 3 and agrees with eqs. 25a, 25b |
| Product drops: $r/r_{32} = 1 + 8K/20 + (\rho_l r^3/\sigma)\, \dot y^2 (6K - 5)/120$ (eq. 22), $K = 10/3$, bag regime $r/r_{32} = 7/3$, large We $\rho_g u^2 r_{32}/\sigma = 6$ (eq. 23); normal velocity $V_\perp = C_v C_b r \dot y$ (eq. 14), "C_v is approximately unity", taken as 1; $\tan(\theta/2) = C_v (\sqrt{3}/3) \sqrt{\rho_g/\rho_l}$ (eq. 16); breakup times $\sqrt{3}\, \sqrt{\rho_l/\rho_g}\, r/u$ for large We (eq. 12a) and $\pi \sqrt{\rho_l r^3/(8 \sigma)}$ near critical (eq. 13); product radii drawn from a chi-squared distribution | read | the same report, eqs. 12a to 23 and the text after eq. 22 (rcfd gives every product drop the radius $r_{32}$: ADR 0224 Q4) |
| TAB constants $C_k = 8$, $C_f = 1/3$, $C_d = 5$, $C_b = 1/2$, $K = 10/3$ and eq. 22 | verified (secondary) | P. John, H. Chandrasekhar, S. Saha, O. Owoyele, arXiv:2510.13033v2, section 2.2.1 (equal to the report; its breakup test reads "A + We_g > 1.0" in the text extraction where the report has $We/12 + A > 1$) |
| Breakup time $\arccos(1 - 1/W)/\omega$ from rest, $r/r_{32} = 4/3 + We/6$ without viscosity, $We_{child} = 6\,We/(8 + We) < 6$ | derived | ADR 0224 DB from eqs. 22 and 26 |
| Quasi-steady droplet evaporation with Bird's correction (model B): $dm_p/dt = -\pi d_p \rho_m D_m Sh_{m,0} \ln(1 + B_M)$, $dT_p/dt = \dfrac{\pi d_p \lambda_m Nu_{m,0}}{m_p c_{p,p}} (T_s - T_p) \dfrac{\ln(1 + B_T)}{B_T} + \dfrac{L_v}{m_p c_{p,p}} \dfrac{dm_p}{dt}$, $1 + B_T = (1 + B_M)^{\phi_m}$, $\phi_m = (c_{p,vap,m}/c_{p,m})(1/Le_m)(Sh_{m,0}/Nu_{m,0})$, $B_M = (Y_{f,i} - Y_{f,s})/(1 - Y_{f,i})$, $Nu_{m,0} = 2 + 0.6\, Re^{1/2} Pr^{1/3}$ (Ranz-Marshall), $Sh_{m,0} = 2 + 0.6\, Re^{1/2} Sc^{1/3}$, Raoult's law and the frozen-chemistry surface mass fraction; the psychrometric wet bulb $c_{p,vap,m} (T_s - T_p) = B_T L_v$; the boiling point of n-heptane 371.5 K | read | A. Both, D. Mira, O. Lehmkuhl, "Evaporation of volatile droplets subjected to flame-like conditions", arXiv:2201.02818v1 (2022), sections 2.1, 2.2, eqs. 14 to 25, Tables 3 and 4. The constant properties of rcfd (no 1/3 law, no Daubert-Danner functions inside the library) are a simplification of ADR 0224 |
| The d-squared law $d^2 = d_0^2 - K t$, $K = 8 \rho_m D_m \ln(1 + B_M(T_{wb}))/\rho_l = 8 \lambda_m \ln(1 + B_T)/(\rho_l c_{p,vap})$, independent of $d$ at $Re = 0$ | derived | ADR 0224 DK from the model above; arXiv:2510.13033 section 3.1 names "the classical D2-law" (its source not read) |
| Clausius-Clapeyron $dP/dT = P L/(R T^2)$ for an ideal-gas vapour with negligible liquid volume, integrated for a constant $L$: $\ln(P_2/P_1) = -(L/R)(1/T_2 - 1/T_1)$ | verified (secondary) | Wikipedia, "Clausius-Clapeyron relation", read 2026-11-08; used through the boiling point, $X = (p_b/p)\, \exp\!\left((L_v W_f/R)(1/T_b - 1/T)\right)$ |
| Molar gas constant $R = 8.314\,462\,618\ldots\ \mathrm{J/(mol\,K)}$ (exact), Avogadro constant $6.022\,140\,76 \times 10^{23}\ \mathrm{mol^{-1}}$ (exact) | read | NIST CODATA 2022, physics.nist.gov/cgi-bin/cuu/Value?r and ?na, read 2026-11-08; rcfd uses $R = N_A k = 8.31446261815324$ with $k$ of the F-221 row |
| Abramzon-Sirignano film correction, Langmuir-Knudsen non-equilibrium, Miller, Harstad and Bellan (1998), Godsave and Spalding (1953), Taylor (1963), Reitz and Diwakar (1987) | not read | named in the sources above; not used (ADR 0224 Q6) |
| Measured droplet lifetimes, product drop sizes or breakup times | not used | no comparison attempted (F-252) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
