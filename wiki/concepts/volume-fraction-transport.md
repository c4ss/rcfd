---
type: concept
tags: [foundation, multiphase, vof]
sources: [ref-barreiro-villaverde-2021-jet-wiping-vof, ref-sharifi-2024-mules-interface-advection, ref-wikipedia-multiphase-heat]
builds_on: [two-phase-flow-basics, convection-diffusion-equation, tvd-schemes-and-flux-limiters]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Volume fraction transport (advecting $\alpha$, boundedness, algebraic vs geometric VOF)

## In short

In VOF the volume fraction $\alpha$ is carried along by the flow like a dye without diffusion. The equation is simple; the hard part is moving a field that jumps from 0 to 1 over one cell without smearing it and without creating values below 0 or above 1 ([[ref-wikipedia-multiphase-heat]] "Volume of fluid method"; [[ref-barreiro-villaverde-2021-jet-wiping-vof]] sec. II.B).

## Explanation

**The equation.** With the cell fraction $\alpha$ (0 empty, 1 full, between for interface cells) and the velocity $\vec U$:

$$\frac{\partial\alpha}{\partial t} + \nabla\cdot(\alpha\vec U) = 0$$ (Barreiro-Villaverde et al., Eq. 3)

For incompressible flow ($\nabla\cdot\vec U = 0$) this is the pure advection equation $\partial C/\partial t + \vec v\cdot\nabla C = 0$, the same equation the level-set function satisfies ([[ref-wikipedia-multiphase-heat]] "Volume of fluid method", Overview). It is a [[convection-diffusion-equation]] with zero diffusion, so every bit of smearing comes from the discretisation.

**Why it is hard.** "Lower-order upwind schemes smear the interface, while higher-order schemes can be unstable and oscillatory" ([[ref-wikipedia-multiphase-heat]] "Volume of fluid method", Discretization). This is the same dilemma that [[tvd-schemes-and-flux-limiters]] solve for smooth fields; for $\alpha$ the scheme must in addition keep $0 \le \alpha \le 1$ (boundedness), or the mixture density of [[two-phase-flow-basics]] goes outside the physical range.

**Two families.** Geometric VOF reconstructs the interface in each cell from the volume fractions and moves volumes geometrically: SLIC (interface segments parallel to the axes, Noh and Woodward 1976) or PLIC (a plane in each cell, Youngs 1982) ([[ref-wikipedia-multiphase-heat]] "Volume of fluid method", History; [[ref-barreiro-villaverde-2021-jet-wiping-vof]] sec. II.B). Algebraic VOF computes the face fluxes from $\alpha$ by a differencing formula without reconstruction: compressive differencing schemes (CICSAM, HRIC, built on Leonard's normalised variable diagram), THINC schemes (a hyperbolic-tangent profile inside the cell) and schemes with an explicit compression term ([[interface-compression]]) ([[ref-wikipedia-multiphase-heat]] "Volume of fluid method", Discretization; [[ref-sharifi-2024-mules-interface-advection]] sec. 1). Algebraic schemes are simpler, faster and adapt to unstructured meshes; geometric schemes are more accurate at the price of "a more cumbersome implementation process and slower execution" ([[ref-sharifi-2024-mules-interface-advection]] sec. 1).

**Boundedness in practice.** In the OpenFOAM `interFoam` solver the MULES limiter (Multidimensional Universal Limiter with Explicit Solution) adjusts the face fluxes "to guarantee boundedness and stability", and the time step is limited by an interface Courant number (0.2 in that study) as well as the flow Courant number ([[ref-barreiro-villaverde-2021-jet-wiping-vof]] sec. II.B).

## Builds on

- [[two-phase-flow-basics]]: what $\alpha$ is and how properties follow from it.
- [[convection-diffusion-equation]]: the transport equation, here with zero diffusion.
- [[tvd-schemes-and-flux-limiters]]: limiting fluxes to avoid new extrema, the basis of bounded schemes.

## Leads to

- [[interface-compression]], [[surface-tension-and-curvature]] (curvature from $\alpha$), [[cavitation-and-mass-transfer-models]] (the same equation with a source)
- [[vof]]

## How rcfd uses it

- rcfd's VOF is algebraic with interface compression; the case keys include `compression` (default 1) and `alpha_courant` (default 0.25) ([[doc-case-schema-multiphase]]).
- Verified on the Zalesak slotted disk among other cases (F-055, ADR 0049) ([[vof]], [[adr-thread-vof-multiphase-particles]]).
