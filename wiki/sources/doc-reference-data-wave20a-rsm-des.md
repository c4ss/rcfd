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

# Reference data: Wave 20A transposed stress, SA-based DES and Reynolds stress model references (F-045, F-207, F-046, ADR 0032)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 20A transposed stress, SA-based DES and Reynolds stress model references (F-045, F-207, F-046, ADR 0032)" (lines 828-845) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 20A transposed stress, SA-based DES and Reynolds-stress model references (ADR 0032).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Web pages were consulted for the entries marked "secondary" or "TMR"; none of the original papers was read. Vendor documentation was not consulted. The checks of ADR 0032 test the implementation against the formulas as written here.

| Quantity | Source | Status |
| --- | --- | --- |
| Full viscous stress `div(nu_eff (grad u + grad u^T))` of the incompressible momentum equation; the transposed part `div(nu_eff grad u^T)` equals `grad u . grad nu_eff` for divergence-free velocity (zero for constant viscosity) | continuum mechanics | derived; the operator is verified exactly on linear fields (A1) |
| SSG pressure-strain constants C1 1.7, C1* 0.9, C2 1.05, C3 0.8, C3* 0.65, C4 0.625, C5 0.2, and the GGDH diffusion constant 0.22 | Speziale, Sarkar and Gatski (1991); values read on the NASA Turbulence Modeling Resource page https://tmbwg.github.io/turbmodels/rsm-ssglrr.html (outer-region set) | verified against the TMR page; the 1991 paper itself was not read. The sign convention of the C5 (rotation) term follows the TMR definition of the rotation tensor `W_ij = (dU_i/dx_j - dU_j/dx_i)/2` and was not tested against a physical case |
| LRR-IP constant C1 = 1.8 | Launder, Reece and Rodi (1975) via the same TMR page | consistent with the TMR page; the 1975 paper was not read |
| LRR-IP constant C2 = 0.6 | Launder, Reece and Rodi (1975) | UNVERIFIED, from memory |
| Epsilon-equation constants Ce1 = 1.44, Ce2 = 1.83 (SSG) / 1.92 (LRR-IP), dissipation diffusion constant C_eps = 0.18, eddy viscosity `C_mu k^2/eps` with C_mu = 0.09 | common practice for stress-transport models | UNVERIFIED: values from memory (Ce2 1.83 is the value associated with SSG in the literature, 1.92 with LRR-IP) |
| Homogeneous-shear growth and isotropic decay exponent `1/(Ce2 - 1)` | model ODE | derived from the model; no comparison with experiment (the decay exponent of grid turbulence, the equilibrium shear parameter `S k/eps`) was made: no claim |
| DES length scale `d~ = min(d, C_DES Delta)`, `Delta = max(dx, dy, dz)` | Spalart et al. (1997) | verified secondary: statement on https://wmles.umd.edu/hybrid-les-rans-models/des/; the 1997 paper was not read |
| C_DES = 0.65 | Shur et al. (1999) / Spalart (2001) calibration | secondary only: stated in an arXiv summary (2512.18609) and an encyclopedia entry; the calibration papers were not read. Treated as an adjustable default (range 0.1 to 2) |
| DDES `f_d = 1 - tanh[(8 r_d)^3]`, `r_d = (nu + nu_t)/(max(sqrt(U_ij U_ij), 1e-10) kappa^2 d^2)`, `d~ = d - f_d max(0, d - C_DES Delta)` | Spalart et al. (2006) | verified secondary: wmles.umd.edu and arXiv:1606.05114; the 2006 paper was not read |
| SA equation with `d~` in place of `d` in `S~`, `r` and the destruction term | Spalart et al. (1997) | consistent with the secondary pages above; the equations are those of the SA-noft2 model of ADR 0014 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
