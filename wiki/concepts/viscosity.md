---
type: concept
tags: [foundation, shared-roots, fluid-mechanics]
sources: [ref-openstax-university-physics-1, ref-bar-meir-fluid-mechanics, ref-wikipedia-shared-roots]
builds_on: [stress, strain-and-strain-rate]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Viscosity (Newtonian fluid)

## In short

Viscosity is a fluid's internal friction: the resistance of fluid layers to sliding over each other. In a Newtonian fluid (air, water) the shear stress is proportional to the rate of shear, `tau = mu du/dy`; the constant `mu` is the dynamic viscosity, and `nu = mu / rho` the kinematic viscosity ([[ref-openstax-university-physics-1]] sec. 14.7; [[ref-bar-meir-fluid-mechanics]] sec. 1.4, 1.5.3).

## Explanation

**Two-plate experiment.** Fluid fills a gap `L` between two plates; dragging the upper plate of area `A` at speed `v` needs a force `F = eta v A / L`, which defines the viscosity `eta = F L / (v A)` (Eq. 14.17), unit Pa s ([[ref-openstax-university-physics-1]] sec. 14.7). With a linear velocity profile across the gap, `v / L` is the velocity gradient `dU/dy`, giving `tau_xy = mu dU/dy` ([[ref-bar-meir-fluid-mechanics]] sec. 1.4, label `intro:eq:tau_xy`). Example 1.1: glycerin in a 1 cm gap, a 1 m^2 plate at 0.5 m/s needs about 53.45 N.

**Newtonian fluid.** Fluids with a constant ratio of shear stress to velocity gradient are Newtonian ([[ref-bar-meir-fluid-mechanics]] sec. 1.4). Viscosity comes from molecular cohesion and interaction hampering momentum transfer across the layers (same section).

**Kinematic viscosity.** `nu = mu / rho`, unit m^2/s, a diffusivity of momentum ([[ref-bar-meir-fluid-mechanics]] sec. 1.5.3).

**Values and temperature.** In units of `10^-3 Pa s`: air 0.0181 at 20 C, 0.0218 at 100 C; water 1.002 at 20 C, 0.282 at 100 C (Table 14.4) ([[ref-openstax-university-physics-1]] sec. 14.7). Gas viscosity rises with temperature, roughly with the square root of absolute temperature; liquid viscosity falls as cohesion weakens; pressure effects are usually negligible ([[ref-bar-meir-fluid-mechanics]] sec. 1.5.1).

**Tensor form (constitutive law).** In 3D the Newtonian stress is linear in the strain-rate tensor: `sigma = -p I + lambda (div u) I + mu (grad u + grad u^T)` ([[ref-wikipedia-shared-roots]] "Navier-Stokes equations"); Bar-Meir writes the same law as `tau_ij = -[P + (2/3 mu - lambda) div U] delta_ij + mu (dU_i/dx_j + dU_j/dx_i)` (sec. 8.5, Eq. 48), with the shear component `tau_xy = mu (dU_y/dx + dU_x/dy)` (Eq. 15). Only the symmetric (strain-rate) part of the velocity gradient enters; rotation causes no stress ([[strain-and-strain-rate]]). The bulk viscosity `zeta = lambda + (2/3) mu` multiplies `div u`; the Stokes hypothesis `zeta = 0` holds for monatomic gases and is "generally incorrect" otherwise, and it drops out for incompressible flow ([[ref-wikipedia-shared-roots]] "Navier-Stokes equations").

**Laminar pipe flow.** Poiseuille's law `Q = (p2 - p1) pi r^4 / (8 eta l)` (Eq. 14.19) shows how viscosity sets the pressure drop of laminar flow ([[ref-openstax-university-physics-1]] sec. 14.7).

**What it adds.** [[stress]] says how forces act inside a fluid and [[strain-and-strain-rate]] how it deforms; viscosity is the material law that connects them, which closes the momentum balance.

## Builds on

- [[stress]]: the quantity the law gives.
- [[strain-and-strain-rate]]: the quantity it depends on (the symmetric velocity gradient).

## Leads to

- [[shear-stress]], [[navier-stokes-equations]], [[dimensionless-numbers]] (Reynolds number).
- [[boussinesq-eddy-viscosity]] (turbulent "viscosity" as a modelled, not a fluid, property), [[boundary-layer]], `particle-equation-of-motion` (Stokes drag).

## How rcfd uses it

- **PISO time-step limit:** the default PISO step "grows unstable above a diffusion number `nu dt/dx^2` of about 3 to 4 across the finest cells" in narrow gaps (F-244, ADR 0211) ([[piso]]); on a refined cylinder O-grid the wall diffusion number onset is between 6.5 and 11.4 (ADR 0154) ([[cylinder-re100]]).
- **Particles:** the particle response time is `tau_p = rho_p d^2/(18 mu)` ([[lagrangian-particles]]).
- **Turbulence models:** [[k-epsilon]], [[k-omega-sst]] and [[spalart-allmaras]] add an eddy viscosity to the molecular one (see [[boussinesq-eddy-viscosity]]).
