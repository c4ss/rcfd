---
type: concept
tags: [foundation, fsi, added-mass]
sources: [ref-banks-2016-amp-rigid-body, ref-yao-2026-ale-chalmers, ref-spenke-2020-interface-quasi-newton]
builds_on: [rigid-body-dynamics-six-dof, incompressible-flow]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Added mass

## In short

A body accelerating through a fluid has to push the surrounding fluid out of the way and accelerate it too, so it behaves as if it were heavier. That extra apparent mass is the added mass. For a light body in a dense fluid (a thin plate in water, a valve leaflet in blood) it can exceed the body's own mass, and then simple ways of coupling a flow solver to a structure solver become unstable.

## Explanation

- **Physical added mass.** When a force is applied suddenly to a rigid body in an incompressible fluid, the acceleration depends on the body's mass and on "the added-mass of a portion of the surrounding fluid that is accelerated due to the instantaneous pressure field created by the motion of the body"; the effective mass is $m_b + M_a$ ([[ref-banks-2016-amp-rigid-body]], sec. 1, fig. 1).
- **A worked model.** A piston of mass $m_s$ on a spring closes a tube of incompressible fluid (density $\rho_0$, area $A$, length $L_0$). With plug flow, integrating the fluid momentum equation from the reservoir to the piston gives a pressure force proportional to the piston acceleration; linearising, the fluid adds the constant mass $m_a = \rho_0 A L_0$. For $m_a / m_s = 1$ the natural frequency drops to $1/\sqrt{2}$ of the dry piston's ([[ref-yao-2026-ale-chalmers]], sec. 7.9).
- **Added damping.** Viscous shear on a rotating body builds a boundary layer of width about $\sqrt{\nu\, \Delta t}$; its drag damps the motion over time rather than instantly ([[ref-banks-2016-amp-rigid-body]], sec. 1).
- **Why it destabilises loose coupling.** In a partitioned scheme the fluid forces are computed from a lagged or predicted body motion; with incompressible flow the pressure reacts instantly to the interface acceleration, so an overestimated deformation gives exaggerated fluid acceleration and inertia that "acts like an additional fluid mass on the structural degrees of freedom" and can grow from iteration to iteration ([[ref-spenke-2020-interface-quasi-newton]], sec. 3.3). Traditional partitioned rigid-body schemes "usually become unstable when the ratio of the density of the solid to that of the fluid is approximately one or less" ([[ref-banks-2016-amp-rigid-body]], sec. 1).
- **Smaller time steps do not help.** Added-damping instabilities can usually be removed by a smaller time step, but "added-mass instabilities generally cannot be overcome by taking a smaller time-step if the mass of the body is too small relative to the added-mass" ([[ref-banks-2016-amp-rigid-body]], sec. 1); loosely coupled schemes "can be unstable regardless of how small the time step is" (Causin et al. 2005, as summarised in [[ref-yao-2026-ale-chalmers]], sec. 7.7).
- **Density ratio is not the whole story.** The controlling inertia is the fluid added mass relative to the structural mass, with geometry and structural thickness also important; "a density ratio alone is not a complete criterion" ([[ref-yao-2026-ale-chalmers]], sec. 7.7).
- **Remedies.** Under-relaxed sub-iterations, often with Aitken acceleration (expensive for light bodies); interface quasi-Newton methods; interface conditions of Robin type that include the body acceleration in the fluid pressure condition (the AMP schemes, stable without sub-iterations even for zero mass) ([[ref-banks-2016-amp-rigid-body]], sec. 1; [[ref-yao-2026-ale-chalmers]], sec. 7.7).

## Builds on

- [[rigid-body-dynamics-six-dof]]: the body's equation of motion, whose mass the fluid effectively increases.
- [[incompressible-flow]]: the pressure as a constraint force that responds instantly to boundary acceleration.

## Leads to

- [[partitioned-fluid-structure-interaction]], [[strong-fsi-coupling]], [[cosimulation-protocol]]

## How rcfd uses it

- **A derived threshold:** rcfd's loose six-DOF coupling is stable up to an added-mass to body-mass ratio of 2 (ADR 0250) ([[strong-fsi-coupling]], [[dynamic-mesh]]).
- **Strong step:** Aitken relaxation with the safe start $2M/(2M + m_a)$, checked on a spring-mounted piston at a ratio of 4 (F-251) ([[strong-fsi-coupling]]).
- **Wall pressure:** the default extrapolation of `patch_force_moment` was first order on a fixed-flux moving wall (1.25 percent of the added mass on 20 cells, F-351); `wall_pressure = "prescribed_gradient"` corrects it (F-358) ([[strong-fsi-coupling]]).
