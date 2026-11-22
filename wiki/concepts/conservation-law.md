---
type: concept
tags: [foundation, shared-roots, mechanics]
sources: [ref-bar-meir-fluid-mechanics, ref-wikipedia-shared-roots, ref-openstax-calculus]
builds_on: [divergence-theorem, continuum-hypothesis]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Conservation law (balance law)

## In short

A conservation law says that the amount of something inside a region changes only by what flows through its boundary plus what is created inside. Written for a finite region it is a flux balance; written at a point (through the divergence theorem) it is a partial differential equation like $\partial \rho/\partial t + \nabla \cdot (\rho u) = 0$ ([[ref-bar-meir-fluid-mechanics]] sec. 5.4, 8.2; [[ref-wikipedia-shared-roots]] "Conservation law").

## Explanation

**System and control volume.** Newton's laws and mass conservation are statements about a *system*, a fixed piece of matter. In fluids it is easier to watch a *control volume*, a fixed region that matter flows through; its boundary is the control surface ([[ref-bar-meir-fluid-mechanics]] sec. 5.4).

**Reynolds transport theorem.** For a property with amount $f$ per unit mass (total $F = \int \rho f \, dV$), the rate of change of the system's $F$ equals the rate of change inside the control volume plus the net outflow through the control surface ([[ref-bar-meir-fluid-mechanics]] sec. 5.4, label `mass:eq:RT`):

$$\dfrac{dF_\text{sys}}{dt} = \dfrac{d}{dt} \int_\text{cv} (\rho f)\, dV + \int_\text{cs} (\rho f)\, U_{rn}\, dA$$

with $U_{rn}$ the velocity normal to the surface relative to the (possibly moving) boundary. The book calls it "a three dimensional generalization of Leibniz integral rule".

**General integral balance.** With a volume source $h$ and a surface source (flux) $g$, a quantity $f$ per volume obeys ([[ref-wikipedia-shared-roots]] "Continuum mechanics"):

$$\dfrac{d}{dt} \int_\Omega f \, dV = \int_{\partial \Omega} f\, (u_n - v \cdot n)\, dA + \int_{\partial \Omega} g \, dA + \int_\Omega h \, dV$$

Here $u_n$ is the boundary's normal speed and $v$ the material velocity; for a fixed region $u_n = 0$ and the first boundary term is the convective inflow.

**Differential form.** Shrinking the region to a point, or applying the [[divergence-theorem]] to the surface integral, gives a PDE. For mass, Bar-Meir's Taylor expansion of the face fluxes of a small box gives $\partial \rho/\partial t + \partial(\rho U_x)/\partial x + \partial(\rho U_y)/\partial y + \partial(\rho U_z)/\partial z = 0$, i.e. $\partial \rho/\partial t + \nabla \cdot (\rho U) = 0$ (sec. 8.2, Eq. 8). In general a conserved density $y$ with flux $j(y)$ obeys $y_t + \nabla \cdot j(y) = 0$; with a source term it is a *balance* equation rather than a strict conservation law ([[ref-wikipedia-shared-roots]] "Conservation law"). The continuum balances of mass, momentum, angular momentum and energy are all of this type ([[ref-wikipedia-shared-roots]] "Continuum mechanics").

**Integral versus differential.** The integral form needs less smoothness and admits discontinuous (weak) solutions such as shocks; the differential form needs derivatives to exist ([[ref-wikipedia-shared-roots]] "Conservation law"). This is one reason finite-volume methods discretise the integral form.

**What it adds.** The [[continuum-hypothesis]] gives fields; the divergence theorem links volume and surface integrals; a conservation law combines them into the one template (accumulation + net flux = source) from which the mass, momentum and energy equations of CFD are built.

## Builds on

- [[divergence-theorem]]: converts the surface flux integral into a volume integral of a divergence.
- [[continuum-hypothesis]]: makes density, velocity and fluxes point fields.

## Leads to

- [[navier-stokes-equations]] (mass and momentum balances of a viscous fluid), `euler-equations`, `energy-equation`.
- [[finite-volume-method]], [[convection-diffusion-equation]].
- [[ale-formulation]] and [[geometric-conservation-law]] (balances over moving control volumes, the $u_n$ term above).
- `two-way-coupling` (sources exchanged between particles and fluid).

## How rcfd uses it

- **Moving meshes:** rcfd solves the flow in the arbitrary Lagrangian-Eulerian form, with "fluxes relative to the moving faces, and cell volume changes matched by the swept volumes"; the face swept volume per time step is the mesh flux ([[dynamic-mesh]]).
- **VOF:** one transported volume fraction ($\alpha$) per cell marks which fluid fills it ([[vof]]).
- **Particles:** in the transient two-way droplet path, vapour mass, momentum and sensible heat are fed into the carrier, "conserved to 1.8e-15 / 5.8e-16 / 6.7e-13 of the largest term" ([[lagrangian-particles]]); the earlier exchange record of F-128 is an observation only.
