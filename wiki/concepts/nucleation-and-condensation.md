---
type: concept
tags: [foundation, multiphase, condensation, wet-steam]
sources: [ref-yang-2017-condensation-supersonic, ref-lienhard-ahtt-6e]
builds_on: [ideal-gas-and-thermodynamics-basics, two-phase-flow-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Nucleation and condensation (supersaturated vapour, wet steam)

## In short

When steam expands fast, for example through a supersonic nozzle, it cools below its saturation temperature without condensing at once. Only when it is far enough supersaturated do clusters of molecules large enough to survive form, suddenly and in huge numbers; these droplets then grow and the steam returns close to equilibrium ([[ref-yang-2017-condensation-supersonic]] abstract, sec. 3).

## Explanation

**Supersaturation.** The supersaturation ratio $S$ is the vapour pressure over the equilibrium saturation pressure at the vapour temperature ([[ref-yang-2017-condensation-supersonic]] sec. 2.1). In a Laval nozzle condensation "will not appear immediately when the steam reaches the saturation state"; it starts downstream of the throat, in the supersaturated state (abstract).

**Critical radius.** A droplet smaller than a critical size evaporates again because surface tension raises its vapour pressure; the Kelvin-Helmholtz critical radius is

$$r_* = \frac{2\sigma}{\rho_l R_v T\ln S}$$ (Yang et al., Eq. 10)

with surface tension $\sigma$, liquid density $\rho_l$ and the vapour gas constant $R_v$. This is the same surface-tension balance as the equilibrium bubble radius in boiling, $R_b = 2\sigma/(p_{sat} - p_{ambient})$ ([[ref-lienhard-ahtt-6e]] sec. 9.2, eq. 9.1), seen from the vapour side; see [[surface-tension-and-curvature]].

**Nucleation rate.** Classical nucleation theory gives the number of critical clusters formed per unit volume and time, $J$; in its internally consistent form it is a prefactor in $\rho_v^2/(S\rho_l)$ times an exponential whose argument is proportional to $-\sigma^3/\big(T_v^3\rho_l^2(\ln S)^2\big)$, with a correction factor (Eqs. 11, 12) ([[ref-yang-2017-condensation-supersonic]] sec. 2.1). Nucleation is abrupt: in the nozzle the droplet number rose "from 0 to $1.12\times10^{17}$ in a very short distance" (sec. 3).

**Growth and the single-fluid model.** After nucleation, droplets grow at a rate set by the vapour conductivity times the difference between the saturation and the vapour temperature, with a factor $(1 - r_*/r)$ that makes it vanish at the critical radius (Gyarmathy's model, Eq. 13). A single-fluid model transports the liquid mass fraction $Y$ and the droplet number $N$ beside the vapour's mass, momentum and energy (Eqs. 1-5); the condensed mass per volume and time is

$$\dot m = \frac43\pi r_*^3\rho_l J + 4\pi r^2\rho_l N\frac{dr}{dt}$$ (Yang et al., Eq. 9)

new droplets plus growth of existing ones, and it enters every vapour equation as a source (Eqs. 6-8) ([[ref-yang-2017-condensation-supersonic]] sec. 2.1).

## Builds on

- [[ideal-gas-and-thermodynamics-basics]]: saturation, supersaturation, latent heat, the vapour gas constant.
- [[two-phase-flow-basics]]: a mixture of vapour and dispersed liquid in one set of equations.

## Leads to

- [[adr-thread-wet-steam]] (rcfd's condensing nozzle), [[cavitation-and-mass-transfer-models]] (the liquid-side counterpart)

## How rcfd uses it

- F-221 (ADR 0217) built non-equilibrium condensing steam in a transonic nozzle in the library, inviscid with a perfect-gas vapour, compared with nozzle data; F-320 (ADR 0226) added checks for the nucleated-mass term (MU3) and the sound speed (MU6) ([[adr-thread-wet-steam]]).
- rcfd's references were read from other papers than the one used here: the nucleation rate, Young's growth law and the Moses and Stein nozzle data from Starzmann et al. (GT2016-57598), the critical radius $r_* = 2\sigma/(\rho_d R T_v\ln S)$ from Afzalifar et al., the Kantrowitz correction from Dykas et al. ([[doc-reference-data-f221-condensing-steam]]). The critical radius agrees with Yang et al. Eq. 10.
