---
type: concept
tags: [foundation, shared-roots, fluid-mechanics]
sources: [ref-wikipedia-shared-roots, ref-bar-meir-fluid-mechanics, ref-nasa-glenn-beginners-guide, ref-oberkampf-trucano-2002-sand2002-0529]
builds_on: [navier-stokes-equations, dimensionless-numbers]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Incompressible flow

## In short

A flow is incompressible when each fluid element keeps its density as it moves; then mass conservation reduces to `div u = 0`. Liquids almost always and gases at low speed (low Mach number) are treated this way. The price is that pressure loses its own equation: it becomes whatever field keeps the velocity divergence-free ([[ref-wikipedia-shared-roots]] "Incompressible flow", "Navier-Stokes equations").

## Explanation

**Definition and the constraint.** Incompressible flow means the material derivative of density vanishes, `D rho/Dt = 0`. Continuity written as `d rho/dt + u . grad rho + rho div u = 0` is `D rho/Dt = -rho div u`, so for nonzero density the condition is equivalent to `div u = 0` ([[ref-wikipedia-shared-roots]] "Incompressible flow", "Derivation"). `d rho/dt` at a fixed point need not vanish: layered (stratified) fluid can still flow incompressibly.

**Flow versus material.** A homogeneous incompressible material (constant `rho` everywhere) always gives incompressible flow, but a compressible fluid can also flow incompressibly if it is not compressed in that flow ([[ref-wikipedia-shared-roots]] "Incompressible flow", "Difference from material").

**When it is a good approximation.** If the compressibility `beta = (1/rho) d rho/dp` is acceptably small, the flow is treated as incompressible; the low-Mach (pseudo-incompressible) form assumes a Mach number "normally less than 0.3" ([[ref-wikipedia-shared-roots]] "Incompressible flow"). The NASA page states it without a Mach threshold: at low speeds, "roughly below 250 mph", air density stays essentially constant, and compressibility grows in importance with speed ([[ref-nasa-glenn-beginners-guide]] "Mach Number"). Oberkampf and Trucano list "assumption of incompressibility in low-speed flow" among the approximations that make up modelling error ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 4.1.2).

**The equations.** `div u = 0` and `rho (du/dt + (u . grad) u) = -grad p + mu laplacian u + f` ([[ref-wikipedia-shared-roots]] "Navier-Stokes equations"; [[ref-bar-meir-fluid-mechanics]] sec. 8.5, Eq. 56 form). The bulk-viscosity term multiplies `div u` and drops out.

**The role of pressure.** There is no equation that advances pressure in time; pressure is determined by the velocity field through the continuity constraint and acts to keep the volume of each fluid element constant ([[ref-wikipedia-shared-roots]] "Navier-Stokes equations", incompressible section). Taking the divergence of the momentum equation gives a Poisson equation for pressure, `laplacian p = -rho (du_i/dx_k)(du_k/dx_i)` (same article). Numerical methods that build such a pressure equation from the discrete momentum and continuity equations are [[pressure-correction-equation]] and [[projection-method]].

**What it adds.** The [[navier-stokes-equations]] become simpler (no equation of state, no bulk viscosity) but change type: velocity is constrained, and pressure is a constraint force instead of a thermodynamic variable. That is why incompressible solvers need a pressure-velocity coupling scheme.

## Builds on

- [[navier-stokes-equations]]: the incompressible case of the general equations.
- [[dimensionless-numbers]] (Mach number) decides when the approximation is acceptable.

## Leads to

- [[pressure-velocity-coupling]], [[pressure-correction-equation]], [[projection-method]], [[collocated-grid-and-checkerboarding]].
- [[added-mass]] (a light body in a dense incompressible fluid).
- rcfd: [[simple]], [[piso]], [[cosimulation-protocol]].

## How rcfd uses it

- A co-simulation case "must be incompressible piso/pimple with nothing else to advance" (F-444, ADR 0380) ([[cosimulation-protocol]]).
- [[simple]] and [[piso]] are rcfd's incompressible pressure-velocity solvers; the fluid side of co-simulation is the incompressible PISO/PIMPLE step ([[piso]]).
