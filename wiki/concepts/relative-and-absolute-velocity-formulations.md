---
type: concept
tags: [foundation, rotating, mrf]
sources: [ref-liu-hasegawa-2026-vpm-mrf-sliding, ref-cline-variational-mechanics]
builds_on: [rotating-reference-frame, navier-stokes-equations]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Relative and absolute velocity formulations

## In short

Inside a rotating zone the flow equations can be solved for the velocity seen by the rotating blade (relative velocity) or for the velocity seen by the stationary casing (absolute velocity). Both describe the same flow; they differ in which source terms appear and in what happens at the boundary with a stationary zone.

## Explanation

- **The split.** In a zone rotating with $\Omega$, the absolute velocity is $u_a = u_r + \Omega \times r$: relative velocity plus the frame velocity at radius $r$ ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], sec. 2.2; [[ref-cline-variational-mechanics]], eq. 12.9).
- **Relative formulation.** Writing Navier-Stokes for $u_r$ in the rotating frame adds the fictitious accelerations of [[rotating-reference-frame]]: a Coriolis term $-2\, \Omega \times u_r$ and a centrifugal term $-\Omega \times (\Omega \times r)$ on the right-hand side of the momentum equation ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], eq. 2; [[ref-cline-variational-mechanics]], eq. 12.28).
- **Why the choice matters at an interface.** A stationary zone next to the rotating one holds absolute velocity. If the rotating zone holds relative velocity, the stored velocity jumps by $\Omega \times r$ across the interface; "solving the governing equation of the relative velocity ... causes the jump of the relative velocity across the interface between rotating and stationary regions. This results in numerical instabilities" unless the jump is treated ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], sec. 2.3).
- **Absolute formulation.** Substituting $u_r = u_a - \Omega \times r$ into the relative equation gives a momentum equation for $u_a$ (with the convecting flux still relative to the rotating frame) whose solution is continuous across the zone boundary; continuity is rewritten for $u_a$ too ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], eqs. 4-5). The authors solve this form "in a unified manner without introducing the velocity jump".
- **On a sliding mesh.** When the grid itself rotates, the momentum equation for the absolute velocity in the absolute frame is used, with the radius vector of each moving cell updated every step ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], sec. 2.4, eq. 6).

## Builds on

- [[rotating-reference-frame]]: the velocity split and the Coriolis and centrifugal accelerations.
- [[navier-stokes-equations]]: the momentum and mass balances being transformed.

## Leads to

- [[mrf]], [[frozen-rotor-and-mixing-plane]], [[sliding-mesh]]
- [[k-omega-sst]] (wall velocity seen in the rotating frame)

## How rcfd uses it

- **Both forms exist in rcfd:** the `mrf` source is the absolute formulation, "solves for the absolute velocity $u$, with the relative flux $\phi_\text{rel}$ in momentum convection only and a lagged $\Omega \times u$ source" (ADR 0008); the `rotating_frame` source is the relative-frame formulation used for smoke solves on the bladed sector, because the absolute-formulation `mrf` source is refused there (blade and seam faces are not surfaces of revolution) ([[mrf]]).
- **Second order** in both the `rotating_frame` and `mrf` formulations (ADR 0071) ([[mrf]]).
- **Relative wall velocity in SST:** ADR 0075 fixed a Galilean-invariance error in the SST wall treatment on the rotating passage ([[k-omega-sst]]).
