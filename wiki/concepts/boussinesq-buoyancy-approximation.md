---
type: concept
tags: [foundation, heat-transfer, natural-convection]
sources: [ref-lienhard-ahtt-6e, ref-openstax-college-physics-ch14]
builds_on: [energy-equation, navier-stokes-equations]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Boussinesq buoyancy approximation (natural convection, Grashof and Rayleigh numbers)

## In short

Warm fluid is lighter than cold fluid, so gravity makes it rise: natural convection. The Boussinesq approximation keeps all fluid properties constant except the density in the gravity term, which varies linearly with temperature through the thermal expansion coefficient; the strength of the resulting flow is measured by the Rayleigh number ([[ref-lienhard-ahtt-6e]] sec. 8.2-8.4; [[ref-openstax-college-physics-ch14]] 14.6).

## Explanation

**Buoyancy.** In natural convection the fluid moves because of density differences caused by temperature, as when air heated by a radiator rises ([[ref-openstax-college-physics-ch14]] 14.6). Adding gravity to the boundary-layer momentum equation and subtracting the hydrostatic pressure gradient of the undisturbed fluid leaves the driving term $(1 - \rho_\infty/\rho)g$ (Lienhard eq. 8.4).

**Thermal expansion coefficient.** For small density changes

$$\beta = -\frac1\rho\left(\frac{\partial\rho}{\partial T}\right)_p \approx -\frac1\rho\,\frac{\rho_\infty - \rho}{T_\infty - T}$$ (Lienhard eq. 8.7)

and for an ideal gas $\beta = 1/T$ ([[ref-lienhard-ahtt-6e]] sec. 8.2; see [[ideal-gas-and-thermodynamics-basics]]). The momentum equation of the vertical-plate boundary layer becomes

$$u\frac{\partial u}{\partial x} + v\frac{\partial u}{\partial y} = -g\beta(T - T_\infty) + \nu\frac{\partial^2u}{\partial y^2}$$ (Lienhard eq. 8.8, for the plate orientation of his Fig. 8.1a)

coupled to the [[energy-equation]]: temperature drives the velocity and the velocity carries the temperature. "The use of constant properties except for density in the buoyancy term is called the Boussinesq approximation" (Lienhard p. 417, footnote 2). The term $-g\beta(T - T_\infty)$ is a body force in the momentum equation of [[navier-stokes-equations]].

**Groups.** The Grashof number compares buoyancy with viscous forces:

$$Gr_L = \frac{g\beta\Delta T L^3}{\nu^2}$$ (Lienhard eq. 8.9)

and $Nu_L = fn(Gr_L, Pr)$ (eq. 8.10). Its product with the Prandtl number is the Rayleigh number

$$Ra_L = Gr_L\,Pr = \frac{g\beta\Delta T L^3}{\nu\alpha}$$ (Lienhard eq. 8.11)

and $Nu = fn(Ra, Pr)$ with the Prandtl number of secondary importance (eq. 8.12). On a vertical plate the boundary layer turns turbulent where $Ra_L/[1 + (0.492/Pr)^{9/16}]^{16/9}$ is about $10^8$; the Churchill-Chu correlation covers laminar flow up to $Ra_L \approx 10^9$ (eq. 8.13a) ([[ref-lienhard-ahtt-6e]] sec. 8.3-8.4).

## Builds on

- [[energy-equation]]: the temperature field that sets the density.
- [[navier-stokes-equations]]: the momentum equation that receives the buoyancy force.

## Leads to

- [[differentially-heated-cavity]] (natural convection in a square box), [[conjugate-heat-transfer]]

## How rcfd uses it

- F-050 delivered Boussinesq buoyancy together with the energy equation and ideal-gas density ([[roadmap-heat-materials-and-chemistry]]).
- The [[differentially-heated-cavity]] case checks it at $Ra = 10^3$ to $10^6$, $Pr = 0.71$, against de Vahl Davis's average Nusselt numbers ([[doc-reference-data-heated-cavity]]).

## Open questions

- Lienhard's chapter covers external natural convection (plates, cylinders); no source on enclosure natural convection was read in this job, so the cavity is linked only through its rcfd pages.
