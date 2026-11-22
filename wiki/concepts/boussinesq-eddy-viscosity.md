---
type: concept
tags: [foundation, turbulence, rans]
sources: [ref-nasa-tmr, ref-mit-ocw-2-29-turbulence, ref-davidson-turbulence-notes, ref-menter-1992-nasa-tm-103975]
builds_on: [reynolds-stress, viscosity, strain-and-strain-rate]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Eddy viscosity and the Boussinesq hypothesis

## In short

The Boussinesq hypothesis models the unknown Reynolds stress like a viscous stress: proportional to the mean strain rate, with a coefficient called the eddy (or turbulent) viscosity $\nu_t$. Unlike the molecular viscosity, $\nu_t$ is not a property of the fluid; it is a property of the flow, and the turbulence model's job is to compute it ([[ref-nasa-tmr]], [[ref-mit-ocw-2-29-turbulence]]).

## Explanation

**The relation.** For incompressible flow ([[ref-mit-ocw-2-29-turbulence]] L25 s.14; [[ref-menter-1992-nasa-tm-103975]] eq. 11):

$$-\overline{u_i' u_j'} = \nu_t \left( \dfrac{\partial U_i}{\partial x_j} + \dfrac{\partial U_j}{\partial x_i} \right) - \tfrac{2}{3} k \delta_{ij} = 2 \nu_t S_{ij} - \tfrac{2}{3} k \delta_{ij}$$

with $S_{ij} = \tfrac{1}{2}(\partial U_i/\partial x_j + \partial U_j/\partial x_i)$ the mean strain-rate tensor, $k$ the turbulent kinetic energy and $\delta_{ij}$ the identity. The compressible form used on the TMR is $\tau_{ij} = 2 \mu_t \left(S_{ij} - \tfrac{1}{3} \dfrac{\partial U_k}{\partial x_k} \delta_{ij}\right) - \tfrac{2}{3} \rho k \delta_{ij}$, with $\mu_t = \rho \nu_t$ "the eddy viscosity obtained by the turbulence model" ([[ref-nasa-tmr]] `implementrans.html`). Models of this kind are called "linear eddy viscosity models" ([[ref-nasa-tmr]] `sst.html`).

**Why the $-\tfrac{2}{3} k \delta_{ij}$ term.** Taking the trace of the left side gives $-2k$; the strain part has zero trace in incompressible flow, so the term is needed "to ensure correct results for the sum of normal stresses" ([[ref-mit-ocw-2-29-turbulence]] L25 s.14). In practice it is often absorbed into the pressure or dropped in the momentum equation; for incompressible flow the production then reduces exactly to $P = \mu_t S^2$, $S = \sqrt{2 S_{ij} S_{ij}}$ ([[ref-nasa-tmr]] `sst.html`).

**The shear-flow case.** In a boundary layer it reduces to $-\overline{u'v'} = \nu_t\, dU/dy$, and the total shear stress is $(\mu + \mu_t)\, dU/dy$ ([[ref-davidson-turbulence-notes]] eq. 6.29). This is why the RANS momentum equation looks like the laminar one with $\nu$ replaced by $\nu + \nu_t$.

**Physical picture and dimensions.** Turbulence increases stirring, which "increase[s] effective viscosity" ([[ref-mit-ocw-2-29-turbulence]] L25 s.14). $\nu_t$ has the units of $\nu$ $[\mathrm{m^2/s}]$, so it is a turbulent velocity scale times a turbulent length scale ([[ref-davidson-turbulence-notes]] p. 93). Each model supplies the two scales differently: a mixing length and the mean velocity gradient ([[mixing-length-model]]), $\nu_t = C_\mu k^2/\varepsilon$ ([[k-epsilon]]), $\nu_t = k/\omega$ ([[k-omega-wilcox]]), or a transported $\nu_t$ itself ([[spalart-allmaras]]).

**Limits.** A single scalar $\nu_t$ makes the stress tensor line up with the strain-rate tensor; it amounts to an "assumption of isotropic turbulence, which is often inaccurate" ([[ref-mit-ocw-2-29-turbulence]] L25 s.14). In a two-equation model the shear stress also responds "instantaneously" to the local strain rate, so in adverse-pressure-gradient boundary layers, where production exceeds dissipation, the Boussinesq relation over-predicts the shear stress ([[ref-menter-1992-nasa-tm-103975]] pp. 6-7; [[ref-davidson-turbulence-notes]] sec. 16, eq. 16.15). SST's limiter ([[bradshaw-assumption-and-sst-limiter]]) and Reynolds-stress models ([[reynolds-stress-models]]) are two answers to that.

**What it adds to its prerequisites.** From [[viscosity]] it borrows the form of the Newtonian constitutive law (stress proportional to [[strain-and-strain-rate|strain rate]]); from [[reynolds-stress]] it takes the quantity to be modelled. The new idea is that six unknown stresses collapse to one unknown scalar field $\nu_t$, which closes the RANS equations once a model gives it.

## Builds on

- [[reynolds-stress]]: the tensor being modelled.
- [[viscosity]]: the Newtonian law whose form is copied.
- [[strain-and-strain-rate]]: the mean strain-rate tensor $S_{ij}$.

## Leads to

- [[mixing-length-model]], [[two-equation-turbulence-models]], [[bradshaw-assumption-and-sst-limiter]], [[les-and-des]] (subgrid eddy viscosity)
- rcfd models: [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]]

## How rcfd uses it

- k-epsilon, SST and SA all feed the momentum equation an effective viscosity $\nu + \nu_t$; with periodic seams the pair diffusivity applies to the momentum $\nu + \nu_t$ (F-296) ([[k-epsilon]], [[adr-thread-periodic-seam-diffusivity]]).
- Force reports with `total` or `viscous` use the momentum equation's face viscosity under a turbulence model (ADR 0166); before, the molecular viscosity only ([[spalart-allmaras]], [[doc-case-schema-turbulence]]).
- ADR 0032 added a `full_viscous_stress` option ([[adr-thread-early-turbulence]]).
