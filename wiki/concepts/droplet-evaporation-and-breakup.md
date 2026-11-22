---
type: concept
tags: [foundation, multiphase, particles, droplets, evaporation]
sources: [ref-salimnezhad-2025-droplet-evaporation, ref-balasubramanyam-2006-nasa-droplet-evaporation, ref-lienhard-ahtt-6e]
builds_on: [particle-equation-of-motion, convective-heat-transfer]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Droplet evaporation and breakup ($d^2$ law, Spalding transfer number, TAB)

## In short

A liquid droplet in a hotter gas heats up and evaporates; in still air its diameter squared falls linearly in time (the $d^2$ law). In a spray, droplets are treated as Lagrangian particles whose mass, temperature and size change by low-order models; fast droplets also break up into smaller ones ([[ref-salimnezhad-2025-droplet-evaporation]] sec. 4.3, sec. 5; [[ref-balasubramanyam-2006-nasa-droplet-evaporation]]).

## Explanation

**The $d^2$ law.** A stationary droplet of initial diameter $d_0$ evaporating in stagnant air, with natural convection neglected:

$$d^2 = d_0^2 - 8\frac{\rho_g}{\rho_l}D\ln(1+B_M)\,t$$ (Salimnezhad et al., Eq. 32)

with gas density $\rho_g$, liquid density $\rho_l$, vapour diffusion coefficient $D$ and the Spalding mass transfer number

$$B_M = \frac{Y_{v,s} - Y_{v,\infty}}{1 - Y_{v,s}}$$ (Salimnezhad et al., sec. 4.3)

from the vapour mass fraction at saturation on the surface, $Y_{v,s}$, and far away, $Y_{v,\infty}$. The law assumes an unbounded domain ([[ref-salimnezhad-2025-droplet-evaporation]] sec. 4.3).

**Point-particle evaporation in a flow.** In sprays, interface-resolved simulation is not feasible; droplets are Lagrangian point particles and evaporation models give the mass and energy exchange ([[ref-salimnezhad-2025-droplet-evaporation]] sec. 5). A classical (Spalding) form of the rate is

$$\dot m_d = 2\pi r_d(\rho_g D)\,Sh_g\ln(1+B_M)$$ (Balasubramanyam et al., Eq. 10)

with the Sherwood number from the correlation $Sh_g = 2 + 0.6\,Re_p^{0.5}Sc^{0.333}$ (Eq. 12) and a matching Nusselt correlation for heat (Eq. 11). The Sherwood number is the mass-transfer twin of the Nusselt number of [[convective-heat-transfer]]: the "2" is the stagnant-gas limit, the $Re_p$ term the enhancement by the relative flow. The droplet temperature follows from a surface energy balance (no heat accumulation at the surface, latent heat at the surface temperature); the vapour concentration at the surface comes from Clausius-Clapeyron and Raoult's law; infinite-conductivity models assume a uniform droplet temperature, two-temperature models keep a core and a surface temperature ([[ref-balasubramanyam-2006-nasa-droplet-evaporation]] model development). Heat conducted into a droplet obeys the same laws as in [[heat-conduction]] ([[ref-lienhard-ahtt-6e]] sec. 2.1).

**Breakup.** In the spray model studied, primary and secondary breakup are modelled by the T-blob/T-TAB hybrid model, with the transition from primary to secondary breakup by an energy balance; the model's own equations are given in other papers ([[ref-balasubramanyam-2006-nasa-droplet-evaporation]] model development). The TAB equations themselves were not found in a source readable in this job (see Open questions).

## Builds on

- [[particle-equation-of-motion]]: the droplet is a particle whose mass and diameter now change.
- [[convective-heat-transfer]]: heat transfer coefficients and the Nusselt number, and by analogy the Sherwood number.

## Leads to

- [[two-way-coupling]] (vapour mass and heat into the carrier), [[lagrangian-particles]], [[adr-thread-particle-wall-and-droplets]]

## How rcfd uses it

- F-245 (ADR 0224): $d^2$ evaporation and TAB breakup in the library; F-323 (ADR 0230) the droplet exchange record; F-326 (ADR 0348) the case tables `[particles.evaporation]`, `[particles.breakup]` that feed the carrier in the transient two-way path ([[doc-particles]], [[adr-thread-particle-wall-and-droplets]]).
- rcfd's evaporation is the Bird-corrected film model of arXiv:2201.02818 with Ranz-Marshall $Nu$ and $Sh$; its $d^2$ law $d^2 = d_0^2 - Kt$ with $K = 8\rho_mD_m\ln(1+B_M)/\rho_l$ is derived in ADR 0224 ([[doc-reference-data-f245-tab-breakup]]), the same form as Salimnezhad et al. Eq. 32.
- TAB constants and equations were read by rcfd from the LA-UR-87-2105 report of O'Rourke and Amsden ([[doc-reference-data-f245-tab-breakup]]).

## Open questions

- No open source describing the TAB oscillator equation was read in this job; the TAB part of this node rests on [[doc-reference-data-f245-tab-breakup]] only.
