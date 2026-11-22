---
type: concept
tags: [foundation, shared-roots, fluid-mechanics]
sources: [ref-bar-meir-fluid-mechanics, ref-wikipedia-shared-roots, ref-nasa-glenn-beginners-guide]
builds_on: [conservation-law, viscosity, stress]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Navier-Stokes equations

## In short

The Navier-Stokes equations are Newton's second law for a viscous fluid, written as conservation laws: mass is conserved, and the momentum of each fluid element changes by pressure forces, viscous stresses and body forces such as gravity. With an energy equation they describe laminar and turbulent flow of air and water; they are too hard to solve analytically in general, which is why CFD exists ([[ref-nasa-glenn-beginners-guide]] "Navier-Stokes Equations"; [[ref-bar-meir-fluid-mechanics]] sec. 8.5).

## Explanation

**Ingredients.** Mass balance (continuity) `d rho/dt + div(rho u) = 0` from [[conservation-law]]; momentum balance with the surface force `div sigma` from [[stress]] and a body force; a constitutive law for `sigma` from [[viscosity]].

**Momentum in stress form (Cauchy).** Forces on a small cube, Taylor-expanded, give ([[ref-bar-meir-fluid-mechanics]] sec. 8.5, Eq. 6, 9, 10; [[ref-wikipedia-shared-roots]] "Navier-Stokes equations"):

`rho (du/dt + (u . grad) u) = -grad p + div tau + rho a`

or in conservation form `d(rho u)/dt + div(rho u (x) u) = -grad p + div tau + rho a`, with `tau` the viscous (deviatoric) stress and `a` a body acceleration such as gravity. The left side is the acceleration of a fluid particle: local change plus convection, transport "by the ordered motion of the flow" ([[ref-nasa-glenn-beginners-guide]]).

**Newtonian closure.** Inserting `sigma = -p I + lambda (div u) I + mu (grad u + grad u^T)` ([[viscosity]]) gives the compressible equations; with constant viscosities Bar-Meir writes ([[ref-bar-meir-fluid-mechanics]] sec. 8.5, Eq. 55 form):

`rho Du/Dt = -grad P + (mu/3 + lambda) grad(div u) + mu laplacian u + f_B`

**Incompressible form.** With `div u = 0` ([[incompressible-flow]]) (Eq. 56 form; [[ref-wikipedia-shared-roots]] "Navier-Stokes equations"):

`div u = 0`, `rho (du/dt + (u . grad) u) = -grad p + mu laplacian u + f`

Dividing by `rho` puts `nu = mu / rho` in front of the Laplacian. The terms are: unsteady, convection (nonlinear in `u`), pressure gradient, diffusion of momentum by viscosity, body force.

**The full set.** The NASA page shows the 3D unsteady compressible set as five coupled equations: continuity, three momentum components, energy; the unknowns are pressure, density, temperature and three velocity components ([[ref-nasa-glenn-beginners-guide]]). In nondimensional form the viscous stress terms carry `1/Re` ([[ref-nasa-glenn-beginners-guide]]; Reynolds number in [[dimensionless-numbers]]).

**Simplifications.** Dropping the viscous terms gives the [[euler-equations]], which cannot model boundary layers; a boundary-layer simplification exists; in CFD the stress terms are often supplemented by a turbulence model ([[ref-nasa-glenn-beginners-guide]]).

**What it adds.** The [[conservation-law]] template plus a material law turns into a closed system: as many equations as unknowns. What it does not give is an equation for pressure in incompressible flow; that is the subject of [[incompressible-flow]] and [[pressure-velocity-coupling]].

## Builds on

- [[conservation-law]]: mass and momentum balances over a control volume.
- [[viscosity]]: the Newtonian stress law that closes the momentum balance.
- [[stress]]: the surface force `div sigma` and its symmetry.

## Leads to

- [[incompressible-flow]], [[pressure-velocity-coupling]], [[turbulence]], [[reynolds-averaging]], [[boundary-layer]].
- [[relative-and-absolute-velocity-formulations]] (rotating frames), [[immersed-boundary-method]], [[two-phase-flow-basics]], [[lighthill-acoustic-analogy]].
- rcfd: [[simple]], [[piso]], [[mrf]], [[immersed-solid]].

## How rcfd uses it

- rcfd's segregated pressure-velocity solvers [[simple]] (steady) and [[piso]] (transient) solve the incompressible equations; the co-simulation fluid side is "the incompressible PISO/PIMPLE step with implicit Euler on a moving mesh" ([[piso]]).
- [[mrf]] computes a rotating zone "in its own rotating frame" (ADR 0008) ([[mrf]]); [[immersed-solid]] gives the cells inside and next to a body that does not fit the mesh "a modified forcing that imposes the wall" (F-219, ADR 0218) ([[immersed-solid]]).
- Turbulence enters through RANS models ([[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]]) on top of these equations ([[turbulence]]).
