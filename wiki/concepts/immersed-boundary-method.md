---
type: concept
tags: [foundation, immersed-boundary, forcing]
sources: [ref-liu-hasegawa-2026-vpm-mrf-sliding, ref-yao-2026-ale-chalmers, ref-banks-2016-amp-rigid-body]
builds_on: [navier-stokes-equations, finite-volume-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Immersed boundary method

## In short

Instead of fitting the mesh to a body, keep a simple (often Cartesian) mesh and tell the flow equations where the body is by adding a force in the cells inside or next to it. The force is chosen so that the fluid velocity there takes the body's velocity, which imposes the wall without a body-fitted grid.

## Explanation

- **Idea.** An immersed-boundary method (IBM) "embeds an arbitrary solid geometry on Cartesian grids", avoiding the difficulty of generating high-quality body-fitted meshes ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], sec. 1). Peskin's immersed boundary method couples an Eulerian fluid discretisation with a Lagrangian representation of the boundary, exchanging forces and motion; its regularised-force formulation spreads the interaction over a finite grid width ([[ref-yao-2026-ale-chalmers]], sec. 8.2).
- **Direct forcing.** "An additional forcing term is added to the momentum equation so as to enforce the boundary condition on the solid surface"; for a moving body the object must be mapped onto the stationary grid every step ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], sec. 1).
- **Sharp vs diffused interface.** IBMs are "sharp" or "diffused" depending on "whether the fluid-solid interface is kept sharp or diffused within a few cells" ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], sec. 1).
- **Volume penalisation** (a diffused IBM). The solid is modelled as a porous medium of nearly zero permeability: an artificial body-force term proportional to $\chi (u - u_s)$, with $\chi$ a phase indicator (1 in the solid, 0 in the fluid, smooth in a thin interfacial band from a level-set function), drives the velocity inside the solid to the solid velocity, so fluid and solid share one set of equations ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], secs. 1, 2.2-2.4). The authors' band half-width is the local diagonal grid spacing times 1.5, so it narrows as the mesh is refined (sec. 2.2).
- **Accuracy cost.** A diffused interface leaves a small non-zero wall velocity and smears the wall shear stress over the band; the authors see delayed separation and torque about 8-9 percent above body-fitted results ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], sec. 3.4). Wall resolution and force accuracy depend on the boundary treatment and local refinement ([[ref-yao-2026-ale-chalmers]], sec. 8.2).
- **Moving and light bodies.** IBMs are one of several approaches to rigid-body FSI (with ALE, level sets, fictitious domain, embedded boundary methods); with light bodies they share the added-mass instability of partitioned coupling, which several IBM variants were designed to relax ([[ref-banks-2016-amp-rigid-body]], sec. 1).

## Builds on

- [[navier-stokes-equations]]: the momentum equation that receives the forcing term.
- [[finite-volume-method]]: the non-fitting background discretisation.

## Leads to

- [[immersed-solid]]

## How rcfd uses it

- **F-219 forcing:** cells inside the body and next to its surface get a modified forcing that imposes the wall on a non-fitting mesh (ADR 0218, library only); it converges at `1.023` on circular Couette flow against the registered `1.5` (known miss), while the DFG numbers are within 1 percent ([[immersed-solid]], [[dfg-cylinder-benchmark]]).
- **Second-order and moving variants:** `build_second_order_bounded`; opt-in `MovingImmersed` (F-308) checked against body-fitted moving-mesh runs ([[immersed-solid]]).
- **Not available:** a case-file key, rotating or deforming shapes, free motion ([[immersed-solid]]).
