---
type: concept
tags: [foundation, shared-roots, mechanics]
sources: [ref-openstax-university-physics-1, ref-bar-meir-fluid-mechanics, ref-wikipedia-shared-roots]
builds_on: [stress, viscosity]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Shear stress

## In short

Shear stress is the part of stress that acts along a surface rather than across it, `tau = F_parallel / A`. In a Newtonian fluid it is set by the velocity gradient, `tau = mu du/dy`; at a wall this gives the wall shear stress that drives skin friction and that wall-bounded turbulence models are built around ([[ref-openstax-university-physics-1]] sec. 12.3; [[ref-bar-meir-fluid-mechanics]] sec. 1.4).

## Explanation

**Definition.** Shear stress = `F_par / A`, the force parallel to a surface over its area (Eq. 12.42); in a solid it produces a shear strain `dx / L0` and the ratio is the shear modulus `S` (Eq. 12.41, 12.43). The book notes the shear modulus applies to solids, not fluids ([[ref-openstax-university-physics-1]] sec. 12.3): a fluid does not hold a shear strain, it keeps deforming, so its shear stress depends on the *rate* of shear instead.

**In a fluid.** `tau_xy = F/A`, first index the direction of the stress, second the direction of the area normal (`intro:eq:tauDef`); for a Newtonian fluid `tau_xy = mu dU/dy` (`intro:eq:tau_xy`) ([[ref-bar-meir-fluid-mechanics]] sec. 1.4). In general flow the shear component is `tau_xy = mu (dU_y/dx + dU_x/dy)` (sec. 8.5, Eq. 15).

**As part of the stress tensor.** The off-diagonal entries of the stress tensor are shear stresses; on a surface with unit normal `n` the shear stress magnitude is `tau_n = sqrt(T . T - sigma_n^2)`, the traction minus its normal part, and the largest shear at a point is `(sigma_1 - sigma_3)/2` from the principal stresses ([[ref-wikipedia-shared-roots]] "Cauchy stress tensor"). Shear stress is what the deviatoric part of stress carries; the hydrostatic part (pressure) carries none ([[stress]]).

**At a wall.** At a no-slip wall the fluid velocity matches the wall, so the wall shear stress is `tau_w = mu (du/dy)` evaluated at the wall (the Newtonian law applied at `y = 0`); [[ref-wikipedia-shared-roots]] "Strain-rate tensor" notes the no-slip condition in pipe flow and the gradient taken between layers. The skin-friction coefficient and wall units built on `tau_w` are in [[skin-friction-coefficient]] and [[friction-velocity-and-y-plus]].

**What it adds.** [[stress]] gives the whole tensor and [[viscosity]] the law; shear stress is the one component that every branch uses on its own: wall friction in flow, the sliding component in solids, the Reynolds shear stress in turbulence.

## Builds on

- [[stress]]: shear stress is the tangential part of the traction.
- [[viscosity]]: in a fluid it equals viscosity times the shear rate.

## Leads to

- [[skin-friction-coefficient]], [[friction-velocity-and-y-plus]], [[boundary-layer]], [[law-of-the-wall]].
- [[reynolds-stress]] and [[bradshaw-assumption-and-sst-limiter]] (turbulent shear stress), [[linear-elasticity]] (shear modulus).

## How rcfd uses it

- [[k-omega-sst]] is Menter's shear-stress-transport model; its eddy-viscosity limiter is the subject of [[bradshaw-assumption-and-sst-limiter]].
- [[flat-plate]] is compared by skin friction `Cf` against the NASA TMR references ([[flat-plate]]).
- rcfd has a `shear` boundary condition that acts on one velocity component; the [[component-diagonal]] defect concerns such conditions ([[component-diagonal]]).
