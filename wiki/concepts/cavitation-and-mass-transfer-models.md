---
type: concept
tags: [foundation, multiphase, cavitation]
sources: [ref-folden-aschmoneit-2023-cavitation-models, ref-lienhard-ahtt-6e]
builds_on: [two-phase-flow-basics, ideal-gas-and-thermodynamics-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Cavitation and mass-transfer models (Rayleigh-Plesset, Schnerr-Sauer, mixture)

## In short

Where the pressure in a liquid drops below its vapour pressure, vapour bubbles grow from tiny nuclei; when they reach higher pressure again they collapse violently. Cavitation models add a source term to the vapour volume-fraction equation that turns liquid into vapour (or back), with the rate taken from the growth of a single bubble ([[ref-folden-aschmoneit-2023-cavitation-models]] sec. I, II.1).

## Explanation

**The phenomenon.** Cavitation is a local liquid-to-vapour phase change "as the ambient pressure drops below the fluid's vapor pressure"; the cavities can be sheets attached to a surface, vortex cavities or clouds of bubbles, and their collapse sends shock waves that can damage the confining material ([[ref-folden-aschmoneit-2023-cavitation-models]] sec. I).

**Homogeneous mixture with a transport equation.** Both phases share velocity and pressure; the mixture properties follow from the vapour fraction $\alpha_v$ as in [[two-phase-flow-basics]] (Folden and Aschmoneit Eq. 2). Transport equation models (TEMs) give each phase a source:

$$\frac{\partial(\alpha_i\rho_i)}{\partial t} + \nabla\cdot(\alpha_i\rho_i\vec u) = \dot m_i$$ (Folden and Aschmoneit, Eq. 3)

split into an evaporation term $\dot m^+$ active for $p < p_v$ and a condensation term $\dot m^-$ for $p > p_v$ (Eq. 4).

**Rayleigh-Plesset equation.** The growth of one spherical bubble of radius $R$ with inside pressure $p_b$ in an infinite liquid at pressure $p$:

$$R\frac{d^2R}{dt^2} + \frac32\left(\frac{dR}{dt}\right)^2 = \frac{p_b - p}{\rho_l} - \frac{4\mu_l}{\rho_l R}\frac{dR}{dt} - \frac{2\sigma}{\rho_l R}$$ (Folden and Aschmoneit, Eq. 5)

The last term is the [[surface-tension-and-curvature]] pressure. The full equation is rarely used; models drop the surface tension, viscous or inertial terms. For a cluster of equal bubbles with number density $n_0$, $\alpha_v = n_0\frac43\pi R^3$ (Eq. 6) ([[ref-folden-aschmoneit-2023-cavitation-models]] sec. II.1).

**Schnerr-Sauer.** The vapour fraction couples the bubble density to the liquid volume (Eqs. 42, 43):

$$\alpha_v = \frac{\frac43\pi n_0R^3}{1 + \frac43\pi n_0R^3}$$ (Folden and Aschmoneit, Eq. 43)

and the bubble radius grows at the inertial (Rayleigh) rate obtained from Eq. 5 with second-order terms, viscosity and surface tension dropped, $p_b = p_v$ and the reference pressure equal to the cell pressure:

$$\frac{dR}{dt} = \sqrt{\frac{2|p_v - p|}{3\rho_l}}$$ (Folden and Aschmoneit, Eq. 45)

Together they give the evaporation and condensation sources (Eqs. 46, 47), with "no empirical constants besides the uniform bubble number $n_0$" ([[ref-folden-aschmoneit-2023-cavitation-models]] sec. III.9).

**Thermal growth for comparison.** A vapour bubble in superheated liquid (boiling rather than cavitation) grows at the rate heat can be conducted to it: Jakob's estimate gives $R \propto \sqrt t$ (eq. 5.58), and the more exact result is $\sqrt3$ times larger (eq. 5.59) ([[ref-lienhard-ahtt-6e]] sec. 5.6).

**Limits.** TEMs that assume no bubble-bubble interaction can fail for cloud cavities; Lagrangian and multiscale models add non-condensable gas, sub-grid turbulence and bubble interactions at higher cost ([[ref-folden-aschmoneit-2023-cavitation-models]] sec. II.1). TEM plus Rayleigh-Plesset is "the current standard" (sec. IV).

## Builds on

- [[two-phase-flow-basics]]: the homogeneous mixture and the volume-fraction equation, now with a source.
- [[ideal-gas-and-thermodynamics-basics]]: vapour pressure and phase change.

## Leads to

- [[vof]] (rcfd's cavitation and mass-transfer options), [[nucleation-and-condensation]] (the reverse phase change in a vapour)

## How rcfd uses it

- `multiphase.cavitation`: Schnerr-Sauer on the VOF path with `saturation_pressure`, `bubble_number_density`, `nucleus_radius`; the source is coupled implicitly to the pressure; isothermal, no vapour compressibility, no non-condensable gas, no turbulence correction (F-127, ADR 0056) ([[doc-case-schema-multiphase-mixture-cavitation]]).
- The reference-data page lists the Schnerr-Sauer form $\alpha_v = x/(1+x)$, $x = n\frac43\pi R^3$, and the Rayleigh rate as "recalled from memory" and UNVERIFIED ([[doc-reference-data-wave29b-cavitation]]); Folden and Aschmoneit Eqs. 43 and 45 state the same forms (a secondary source, not the 2001 original).
- Thermal mass transfer (`lee`, `hertz_knudsen`) runs with a prescribed temperature: "There is NO energy equation in the multiphase path" ([[doc-case-schema-multiphase-mass-transfer-waves]]).
- See [[vof]], [[adr-thread-vof-multiphase-particles]].
