---
type: concept
tags: [foundation, moving-mesh, ale, gcl]
sources: [ref-yao-2026-ale-chalmers]
builds_on: [ale-formulation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Geometric conservation law (GCL)

## In short

When cells move, their volumes change, and the scheme must account for that change exactly through the volumes swept by their faces. If the computed volume change and the computed face motion disagree, a perfectly uniform flow on a moving mesh is no longer preserved: the mismatch acts as a spurious source.

## Explanation

- **Continuous GCL.** From the Jacobian evolution $dJ/dt = J\, \nabla \cdot w$ and the Piola identity, the rate of change of the mesh volume equals the divergence of the transformed mesh flux. For a uniform flow (constant density, velocity and pressure) the geometric storage term and the flux term then cancel, so the uniform state is preserved without needing $\nabla \cdot w = 0$ ([[ref-yao-2026-ale-chalmers]], sec. 5.1).
- **Discrete GCL (DGCL).** Define the signed volume swept by each face over a time step (eq. 28). The DGCL requires that the change of each cell's volume over the step equals the sum of the swept volumes of its faces (eq. 29). Neighbouring cells use opposite signs for the same swept volume, so the geometry is consistent between them. A mesh flux written as $w_f \cdot n_f A_f$ represents the swept volume only if its time and space quadrature reproduce the swept-volume integral; face-centre velocity and geometry at one time level generally do not. Computing the new volume from the new vertex coordinates is fine, provided the fluxes agree with it ([[ref-yao-2026-ale-chalmers]], sec. 5.2).
- **A worked check.** A unit square expanding as $(1 + t)$ has area $(1 + t)^2$; over $t = 0 \ldots 1$ its area grows by 3. Using the old-time face flux gives 2, the new-time flux 4, the midpoint flux the exact 3, because here the face flux is linear in time ([[ref-yao-2026-ale-chalmers]], sec. 5.3). For nonlinear motion or 3-D cells midpoint quadrature must be checked.
- **Why violating it matters.** "A finite volume method may conserve the sum of cell contents through shared face fluxes yet fail to preserve a constant state if its volumes and mesh fluxes disagree" ([[ref-yao-2026-ale-chalmers]], sec. 5.5). Lesoinne and Farhat (1996) and Farhat et al. (2001) related the DGCL to aeroelastic computations and to nonlinear stability (sec. 5.2).
- **What it does not give.** Constant-state preservation, global conservation, accuracy and stability are separate properties. Geuzaine et al. (2003) found the DGCL "neither necessary nor sufficient for retaining temporal order" in the schemes examined ([[ref-yao-2026-ale-chalmers]], sec. 5.5). A first-order update with exact geometric cancellation is given as eq. (41) (sec. 6.4).

## Builds on

- [[ale-formulation]]: mesh velocity, Jacobian evolution and the moving-control-volume balance.

## Leads to

- [[dynamic-mesh]], [[sliding-mesh]] (GCL closure), [[mesh-motion-smoothing]]

## How rcfd uses it

- **Dynamic mesh:** the face swept volume divided by $\Delta t$ is the mesh flux; cell volume change equals the sum of swept volumes to round-off (checks G1 to G3) ([[dynamic-mesh]]).
- **Sliding interface:** when rotor and stator nodes coincide the interface graph falls into parts; since F-710 (ADR 0638) the GCL closure and the old-flux remap are solved jointly, uniform flow kept to $10^{-14}$ ([[sliding-mesh]]).
- **BDF2 on moving meshes:** opt-in in the library (F-345, ADR 0379), uniform flow kept to $7 \times 10^{-15}$ ([[dynamic-mesh]]).
