---
type: concept
tags: [foundation, fsi, coupling]
sources: [ref-spenke-2020-interface-quasi-newton, ref-yao-2026-ale-chalmers, ref-banks-2016-amp-rigid-body]
builds_on: [added-mass, fixed-point-iteration-and-acceleration, ale-formulation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Partitioned fluid-structure interaction

## In short

Fluid-structure interaction can be solved with one big coupled system (monolithic) or with two existing solvers, one for the fluid and one for the structure, that exchange interface data (partitioned). A partitioned step either exchanges once (loose coupling) or repeats the exchange within the step until the interface agrees (strong coupling).

## Explanation

- **Monolithic vs partitioned.** "Monolithic schemes couple the governing equations for all domains into a single large system. Partitioned schemes make use of separate solvers for the fluid and solid domains with the separate solvers being coupled at fluid-solid interfaces" ([[ref-banks-2016-amp-rigid-body]], sec. 1). Partitioned coupling keeps the solvers as black boxes and is flexible and modular ([[ref-spenke-2020-interface-quasi-newton]], sec. 1). A monolithic solve avoids the lagged coupling mechanism but not physical added mass or nonlinear convergence difficulties ([[ref-yao-2026-ale-chalmers]], sec. 7.7).
- **Interface conditions.** Kinematic: continuity of displacement, velocity and acceleration across the interface; dynamic: equality of tractions (Newton's third law) ([[ref-spenke-2020-interface-quasi-newton]], sec. 2.3). With no slip and no mass transfer, velocity continuity and traction balance; the net mechanical interface power then vanishes ([[ref-yao-2026-ale-chalmers]], sec. 7.6).
- **Dirichlet-Neumann scheme.** The fluid solver takes the interface position and velocity (Dirichlet data), its stresses go to the structural solver as loads (Neumann data), and the new interface deformation goes back ([[ref-spenke-2020-interface-quasi-newton]], sec. 3.3). For rigid bodies the traditional coupling "uses the velocity of the body as a boundary condition on the fluid, and the fluid stress as a boundary condition on the rigid body" ([[ref-banks-2016-amp-rigid-body]], sec. 1, footnote 6).
- **Loose vs strong.** A one-pass update uses predicted or lagged interface information and generally keeps an interface mismatch; a strongly coupled method iterates within the step until interface and subproblem tolerances are met ([[ref-yao-2026-ale-chalmers]], sec. 7.7, table 3). The within-step iteration is a fixed-point iteration on the interface deformation ([[ref-spenke-2020-interface-quasi-newton]], sec. 3.3), so it is stabilised and accelerated by relaxation, Aitken or interface quasi-Newton updates ([[fixed-point-iteration-and-acceleration]]).
- **Interface mapping.** Fluid and structural meshes generally do not match at the interface, so data are transferred by a conservative projection ([[ref-spenke-2020-interface-quasi-newton]], sec. 3.3); load and velocity transfer should preserve the interface work ([[ref-yao-2026-ale-chalmers]], sec. 7.6). The "consistent vs conservative" mapping distinction in the map's scope was not spelled out further in the sources read.
- **A step in sequence** (Yao table 3): predict the interface displacement; move the fluid mesh; build the mesh trajectory and swept volumes (check the GCL); solve the fluid; transfer traction; solve the structure; evaluate the interface residual and relax or accelerate; repeat until converged ([[ref-yao-2026-ale-chalmers]], sec. 7.7).
- **Stability.** Added mass makes loose coupling unstable for light structures in dense incompressible fluids; see [[added-mass]].

## Builds on

- [[added-mass]]: why the plain exchange fails for light structures.
- [[fixed-point-iteration-and-acceleration]]: the within-step iteration and its acceleration.
- [[ale-formulation]]: the fluid side on a mesh that follows the interface.

## Leads to

- [[strong-fsi-coupling]], [[cosimulation-protocol]]

## How rcfd uses it

- rcfd's loose and strong six-DOF coupling (with its added-mass limit, the strong step and its case key) is on [[strong-fsi-coupling]]; the external-solver coupling and its convergence test are on [[cosimulation-protocol]]; both build on the ideas above. (Trimmed 2026-11-22 to a cross-reference by the maintainer's page-structure decision; the facts live on those pages.)
