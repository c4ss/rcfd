---
type: concept
tags: [foundation, moving-mesh, ale]
sources: [ref-yao-2026-ale-chalmers]
builds_on: [conservation-law, finite-volume-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Arbitrary Lagrangian-Eulerian (ALE) formulation

## In short

On a fixed (Eulerian) mesh the fluid flows through the cells; on a Lagrangian mesh the cells move with the fluid. ALE lets the mesh move in any way you choose, for example following a moving wall at the boundary and staying put far away, and writes the balance laws so that only the fluid's motion relative to the moving cell faces carries material across them.

## Explanation

- **Three velocities.** The mesh velocity $w$ is the time derivative of the map from a fixed reference domain to the current domain, at a fixed reference point; $v$ is the fluid (material) velocity; the relative velocity is $c = v - w$. Only $c \cdot n$ enters the advective flux through a moving face; a face is impermeable to fluid when $(v - w) \cdot n = 0$ ([[ref-yao-2026-ale-chalmers]], sec. 2.3, fig. 3).
- **ALE time derivative.** The rate of change at a moving mesh point differs from both the fixed-point rate and the material rate: the material derivative equals the ALE derivative plus $c \cdot \nabla f$. A non-zero rate seen at a mesh point need not mean a change seen by a fluid particle ([[ref-yao-2026-ale-chalmers]], sec. 2.4).
- **Volume change.** The Jacobian $J$ of the map (the volume of a mapped element relative to its reference volume) evolves as $dJ/dt = J\, \nabla \cdot w$ at a fixed reference point (eq. 10); expanding and contracting meshes are both admissible, and $\nabla \cdot w$ need not vanish even for incompressible flow ([[ref-yao-2026-ale-chalmers]], sec. 2.5).
- **Balance law on a moving control volume.** By the Reynolds transport theorem (eqs. 13-14), for a quantity with density $\phi$, flux $j$ and source $s$: $\dfrac{d}{dt} \int_{V(t)} \phi \, dV + \oint \phi \, (v - w) \cdot n \, dA + \oint j \cdot n \, dA = \int s \, dV$. The surface moves with the mesh velocity, so the advective flux uses the relative velocity, while non-advective fluxes (viscous stress, diffusion) are evaluated on the current surface ([[ref-yao-2026-ale-chalmers]], sec. 3.3). $w = 0$ recovers the Eulerian form and $w = v$ the Lagrangian (sec. 3.2).
- **Two equivalent local forms.** In spatial-divergence form a mesh-dilatation term accompanies the ALE time derivative; in convective form it cancels by the product rule. The volume contribution "must not be added a second time" (sec. 3.3).
- **What it adds.** ALE keeps a body-fitted mesh on a moving or deforming boundary. Its limits: a smooth one-to-one map cannot represent breakup, coalescence or contact; heavy distortion needs remeshing, whose solution transfer must be counted in the error (sec. 8.1).

## Builds on

- [[conservation-law]]: the integral balance that ALE rewrites for a moving control volume.
- [[finite-volume-method]]: cell balances and face fluxes, now on cells whose faces move.

## Leads to

- [[geometric-conservation-law]], [[mesh-motion-smoothing]], [[overset-mesh]], [[dynamic-mesh]], [[partitioned-fluid-structure-interaction]], [[cosimulation-protocol]]

## How rcfd uses it

- **Dynamic mesh:** rcfd solves the flow on moving points in ALE form, with fluxes relative to the moving faces (F-063, F-229; ADR 0033, partial); incompressible laminar PISO/PIMPLE with implicit Euler, and the explicit compressible solver with prescribed motion or a layered piston (`CompressibleSolver::step_ale`) ([[dynamic-mesh]]).
- **Walls:** following walls take the mesh velocity, with pressure condition $\partial p/\partial n = -n \cdot a_\text{wall}$ ([[dynamic-mesh]]).
- **Topology changes:** layering and local tet remeshing for motions the map cannot absorb ([[dynamic-mesh]], [[adr-thread-layering-remeshing-ale]]).
