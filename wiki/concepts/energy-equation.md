---
type: concept
tags: [foundation, heat-transfer]
sources: [ref-lienhard-ahtt-6e]
builds_on: [conservation-law, heat-conduction, convection-diffusion-equation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Energy equation (temperature transport: convection, conduction, sources)

## In short

In a moving fluid, heat is carried along with the flow as well as conducted. The energy equation for the temperature is the heat conduction equation plus a convection term $\vec u\cdot\nabla T$: a [[convection-diffusion-equation]] for $T$ with diffusivity $\alpha = k/(\rho c_p)$ ([[ref-lienhard-ahtt-6e]] sec. 6.3).

## Explanation

**Assumptions of the simple form.** Pressure variations too small to change the properties (most liquids, and gases below about 1/3 of the speed of sound); density changes small enough that the flow behaves as incompressible, $\nabla\cdot\vec u = 0$; constant $k$; kinetic and potential energy changes negligible against thermal ones; viscous dissipation too small to heat the fluid ([[ref-lienhard-ahtt-6e]] sec. 6.3).

**Derivation.** Conservation of energy for a fixed control volume through which fluid flows: the rate of increase of internal energy equals the enthalpy carried in by the flow, the heat conducted in and the heat generated (Lienhard eq. 6.36); Gauss's theorem and $dh \approx c_p\,dT$ give

$$\rho c_p\left(\frac{\partial T}{\partial t} + \vec u\cdot\nabla T\right) = k\nabla^2T + \dot q$$ (Lienhard eq. 6.37)

whose terms are energy storage, enthalpy convection, heat conduction and heat generation. It is "the same as the corresponding equation for a solid body, (2.11), except for the enthalpy transport--or convection--term" ([[ref-lienhard-ahtt-6e]] sec. 6.3); see [[heat-conduction]]. The bracket is the material derivative $DT/Dt$, the rate of change of temperature of a fluid particle (eq. 6.38). It is a [[conservation-law]] for energy written for $T$.

**Boundary-layer form.** Steady, 2-D, without sources, and with $\partial^2T/\partial x^2 \ll \partial^2T/\partial y^2$ in a boundary layer: $u\,\partial T/\partial x + v\,\partial T/\partial y = \alpha\,\partial^2T/\partial y^2$ (eq. 6.40). It has the same form as the boundary-layer momentum equation with $\nu$ replaced by $\alpha$, so for $\nu = \alpha$ the temperature and velocity profiles coincide (eqs. 6.41-6.43) ([[ref-lienhard-ahtt-6e]] sec. 6.3); the ratio $\nu/\alpha$ is the Prandtl number of [[convective-heat-transfer]].

**What it adds.** [[heat-conduction]] needs a medium at rest; the energy equation lets the flow carry heat, which couples temperature to the velocity field, and through density to the momentum equation when buoyancy matters ([[boussinesq-buoyancy-approximation]]).

## Builds on

- [[conservation-law]]: energy balance on a control volume.
- [[heat-conduction]]: Fourier's law and the conduction equation it extends.
- [[convection-diffusion-equation]]: the mathematical form of the result.

## Leads to

- [[convective-heat-transfer]], [[boussinesq-buoyancy-approximation]], [[conjugate-heat-transfer]], [[thermal-radiation]] (as a source or boundary flux)
- [[differentially-heated-cavity]]

## How rcfd uses it

- F-050: energy equation (T / enthalpy), Boussinesq buoyancy and ideal-gas density, done ([[roadmap-heat-materials-and-chemistry]]); `energy.enabled` also works with transient `piso`/`pimple`, first-order coupling in time ([[doc-case-schema-other-solver-physics]]).
- The multiphase (VOF) path has no energy equation ([[vof]], [[doc-case-schema-multiphase-mass-transfer-waves]]).
- Validated on natural convection in the [[differentially-heated-cavity]].
