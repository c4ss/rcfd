---
type: source
tags: [reference, foundation, multiphase, condensation, nucleation, wet-steam, cite-only]
sources: []
origin: https://orbit.dtu.dk/en/publications/91528a07-36d2-40fc-b15a-20d7eb396dc3
licence: DTU Orbit "General rights": copyright retained by the authors and/or other owners; one copy may be downloaded and printed for private study or research; no further distribution. Peer-reviewed accepted version. Cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Yang, Walther, Yan and Wen (2017), CFD modelling of water vapour condensation in supersonic flows

**Origin:** Y. Yang, J. H. Walther, Y. Yan, C. Wen, "CFD modeling of condensation process of water vapor in supersonic flows", *Applied Thermal Engineering* 115 (2017) 1357-1362, doi:10.1016/j.applthermaleng.2017.01.047. Peer-reviewed version on DTU Orbit: https://backend.orbit.dtu.dk/ws/files/163020895/CFD_modeling_of_condensation_process_of_water_vapor_in_supersonic_flows.pdf
**Kind:** journal paper (accepted manuscript).
**Licence:** all rights reserved apart from private study; cite only.
**Read:** 2026-11-22 by job p6-multiphase-heat: abstract, sec. 1, sec. 2 (Eqs. 1-13), the results on supercooling, nucleation rate and droplet number (sec. 3), conclusions. The paper ran its model as user code inside a commercial solver; only the published model equations are used here.

## Summary

A single-fluid model of spontaneous (homogeneous) condensation of water vapour in a Laval nozzle: vapour mass, momentum and energy equations with condensation source terms, plus transport equations for the liquid mass fraction and the droplet number, closed by classical nucleation theory and Gyarmathy's droplet growth law.

## Key points by section

### Sec. 2.1 "Governing equations"
- Vapour conservation equations with sources $S_m$, $S_{u_i}$, $S_h$ (Eqs. 1-3); liquid mass fraction $Y$ and droplet number $N$ transported with sources $S_Y$ and the nucleation rate $J$ (Eqs. 4, 5); all sources are written through the condensation rate $\dot m$: $S_m$ and $S_Y$ with $\dot m$, $S_{u_i}$ with $\dot m u_i$, $S_h$ with $\dot m h$ (Eqs. 6-8; the signs were lost in the text extraction).
- Condensed mass per unit volume and time: new droplets of critical size plus growth of existing ones, $\dot m = \frac43\pi r_*^3\rho_l J + 4\pi r^2\rho_l N\,dr/dt$ (Eq. 9).
- Kelvin-Helmholtz critical radius $r_* = 2\sigma/(\rho_l R_v T\ln S)$ (Eq. 10), with the supersaturation ratio $S$ = vapour pressure over equilibrium saturation pressure.
- Nucleation rate from the internally consistent classical nucleation theory (Eq. 11): a prefactor in $\rho_v^2/(S\rho_l)$, the surface tension and the molecular mass $m_v$, times an exponential whose argument is proportional to $-\sigma^3/(k_B^3T_v^3\rho_l^2(\ln S)^2)$, times a correction $\exp(\Theta)$ with the dimensionless surface tension $\Theta = \sigma a_0/(k_BT_v)$ (Eq. 12), $a_0$ the molecular surface area. (Constant factors such as $\pi$ were lost in the text extraction and are not reproduced.)
- Droplet growth by Gyarmathy's model (Eq. 13): $dr/dt$ driven by $\lambda_v(T_s - T_v)$ (vapour conductivity, saturation minus vapour temperature) and divided by $\rho_l$, the enthalpy $h$ and $r$, with a factor $(1 - r_*/r)$ and a correction containing the Prandtl number and the adiabatic exponent; the exact arrangement is not legible in the extraction.

### Sec. 3 results
- Condensation does not start when the steam reaches saturation but further downstream of the throat, in the supersaturated (supercooled) state; then nucleation is sharp, the droplet number rises "from 0 to $1.12\times10^{17}$ in a very short distance", the steam returns almost to equilibrium, and the droplets keep growing at about 2 K of supercooling.

## Contradictions with the wiki

None.

## Pages this source informs

[[nucleation-and-condensation]]
