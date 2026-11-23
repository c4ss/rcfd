---
type: source
tags: [reference, foundation, shared-roots, fluid-mechanics, cite-only]
sources: []
origin: https://eng.libretexts.org/Bookshelves/Civil_Engineering/Fluid_Mechanics_(Bar-Meir)
licence: GNU Free Documentation License 1.3 (page footers); the contributor note on the same pages says "Version 1.2 or later or Potto license"
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Bar-Meir, *Fluid Mechanics* (LibreTexts edition, sections read)

**Origin:** Genick Bar-Meir, *Basics of Fluid Mechanics* (Potto Project, potto.org/fluidMech), as edited into *Fluid Mechanics (Bar-Meir)* on Engineering LibreTexts. Book root: https://eng.libretexts.org/Bookshelves/Civil_Engineering/Fluid_Mechanics_(Bar-Meir)
**Kind:** open textbook (web edition), undergraduate fluid mechanics.
**Licence:** GNU FDL 1.3 in the page footers; the contributors section states "GNU Free Documentation License, Version 1.2 or later or Potto license". The two statements differ. The GFDL is not one of the licences P6 allows to be copied (public domain, CC0, CC BY), so this source is cite only.
**Read:** 2026-11-22, by job p6-roots. Equation cross-references print as "(??)" on several pages; the internal equation labels from the page source are used as locators where no number shows.

## Summary

The engineering derivation chain from shear stress and viscosity to the Navier-Stokes equations: Couette two-plate experiment, Newtonian fluids, kinematic viscosity, the Reynolds transport theorem, the differential mass balance, the stress tensor and its symmetry, the Newtonian constitutive law and the compressible and incompressible Navier-Stokes equations, plus a table of dimensionless numbers.

## Key points by section

### 1.4 "Shear Stress"
- Shear stress $\tau_{xy} = F/A$ (label `intro:eq:tauDef`); first index the direction of the stress, second the direction of the area normal.
- Two-plate (Couette) experiment, Figs. 1.3-1.4: fluid between close plates, upper plate dragged; linear dependence gives $U/h$ proportional to $F/A$; for small angles and a linear profile $d\beta/dt = dU/dy$ (`intro:eq:velocityTime`).
- $\tau_{xy} = \mu\, dU/dy$ (`intro:eq:tau_xy`), $\mu$ the absolute (dynamic) viscosity, unit $\mathrm{N\,s/m^2}$; fluids with a constant ratio are Newtonian (air, water).
- Example 1.1: glycerin, 1 cm gap, $1\ \mathrm{m^2}$ plate at 0.5 m/s needs about 53.45 N.
- Viscosity arises from molecular cohesion and interaction that hamper momentum flux across the layers.

### 1.5.1 "General Discussion" (viscosity)
- Gas viscosity rises with temperature (kinetic theory: roughly with the square root of absolute temperature); liquid viscosity falls with temperature as cohesive forces weaken. Pressure raises viscosity slightly, usually negligible; oils show the largest effect.

### 1.5.3 "Kinematic Viscosity"
- $\nu = \mu / \rho$, unit $\mathrm{m^2/s}$; kinematic viscosity of gases generally increases with temperature.

### 5.4 "Reynolds Transport Theorem"
- A system is a fixed quantity of matter; a control volume is a region in space whose boundary (control surface) fluid can cross.
- Property per unit mass $f$; total $F = \int \rho f \, dV$ (`mass:eq:F`).
- RTT (`mass:eq:RT`), "a three dimensional generalization of Leibniz integral rule": rate of change of the system's $F$ = $\dfrac{d}{dt} \int_\text{c.v.} f \rho \, dV + \int_{S_\text{c.v.}} f \rho\, U_{rn} \, dA$, where $U_{rn}$ is the velocity normal to the surface relative to the moving boundary.

### 8.2 "Mass Conservation"
- From the system statement $\dfrac{D}{Dt} \int \rho \, dV = 0$ and the RTT (Eq. 3), Taylor-expanded face fluxes on an infinitesimal box give Eq. 8: $\partial \rho/\partial t + \partial(\rho U_x)/\partial x + \partial(\rho U_y)/\partial y + \partial(\rho U_z)/\partial z = 0$; vector form $\partial \rho/\partial t + \nabla \cdot (\rho U) = 0$; index form $\partial \rho/\partial t + \partial(\rho U_i)/\partial x_i = 0$. The conversion of the surface integral uses the divergence theorem (book Appendix A.1.2). Cylindrical (Eq. 14) and spherical forms follow.

### 8.4 "Momentum Conservation"
- Stress depends on the force and on the orientation of the area (`dif:eq:stressD`); at rest without shear $\tau = -P n$ (`dif:eq:noStress`), so pressure is the normal component with a minus sign.
- $\tau_{xy}$ acts on an $x$-face in the $y$ direction; the full stress is the $3 \times 3$ array $\tau_{ij}$ (`dif:eq:tauComplit`); the tetrahedron argument (`dif:eq:transformationTau`, `dif:eq:tetrahedron`) gives the stress on any face from the coordinate faces.
- Moment balance on a shrinking cube (`dif:eq:cubeM`, `dif:eq:tauTourqueF`): $G_T + \tau_{ij} = \tau_{ji}$ with $G_T$ an external body torque per volume; without body torque $\tau_{ij} = \tau_{ji}$ (`dif:eq:tauSymmetry`). Gravity's effect on symmetry is negligible; magnetic body forces are not.

### 8.5 "Derivations of the Momentum Equation"
- Surface stresses and body force on an infinitesimal cube, Taylor-expanded, give the momentum equation in stress form (Eq. 6; index Eq. 9; vector Eq. 10).
- Newtonian, isotropic: $\tau_{xy} = \mu (\partial U_y/\partial x + \partial U_x/\partial y)$ (Eq. 15); shear strain rate is the sum of the two velocity gradients (Eq. 16); normal stresses follow from a 45-degree rotation (Eq. 22, Eq. 30).
- General form (Eq. 48): $\tau_{ij} = -[P + (\tfrac{2}{3} \mu - \lambda)\, \nabla \cdot U]\, \delta_{ij} + \mu (\partial U_i/\partial x_j + \partial U_j/\partial x_i)$; the mechanical pressure is $P_m = -(\tau_{xx} + \tau_{yy} + \tau_{zz})/3$; the $\tfrac{2}{3} \mu$ coefficient is called experimental and $\lambda$ is kept as a bulk viscosity.
- Compressible Navier-Stokes (Eq. 55 form): $\rho\, DU/Dt = -\nabla P + (\mu/3 + \lambda)\, \nabla (\nabla \cdot U) + \mu \nabla^2 U + f_B$.
- Incompressible (Eq. 56 form, $\nabla \cdot U = 0$): $\rho\, DU/Dt = -\nabla P + \mu \nabla^2 U + f_B$, with x, y, z component forms.

### 9.4 "Summary of Dimensionless Numbers" (Table 9.8)
- Reynolds $Re = \rho U l / \mu$: inertia to viscous forces. Mach $M = U/c$: flow speed to speed of sound. Prandtl $Pr = \nu/\alpha$: viscous to thermal diffusion. Strouhal $St = \omega l / U$: unsteady effects to inertia. Froude $Fr = U / \sqrt{g l}$: inertia to gravity. Euler $Eu = (P_0 - P_\infty)/(\rho U^2 / 2)$: pressure to inertia. Weber $We = \rho U^2 l / \sigma$. Knudsen $Kn = \lambda / l$: mean free path to length.
- The table's Stokes-number row gives $\rho g l^3 / \mu^2$ with the meaning "particle relaxation time to Kolmogorov time"; that formula is the Galileo number, so the row looks like a copy error. The Weber row's wording says "viscous force" where the application column (and the formula) means surface tension. Neither row is used for a formula on wiki pages.
- No Peclet number on this page.

## Contradictions with the wiki

None with rcfd pages. Internal errors of the source (Stokes and Weber rows of Table 9.8) are noted above.

## Pages this source informs

[[shear-stress]], [[viscosity]], [[conservation-law]], [[stress]], [[strain-and-strain-rate]], [[navier-stokes-equations]], [[incompressible-flow]], [[dimensionless-numbers]]
