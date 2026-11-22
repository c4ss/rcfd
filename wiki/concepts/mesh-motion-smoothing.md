---
type: concept
tags: [foundation, moving-mesh, ale]
sources: [ref-yao-2026-ale-chalmers]
builds_on: [ale-formulation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Mesh motion: smoothing, layering and remeshing

## In short

The boundary of a moving-mesh problem is told how to move (a wall, a piston, a flap); the interior points are not. Mesh-motion methods spread the boundary motion into the interior so the cells stay valid and reasonably shaped, by solving a Laplace-type or elasticity-type equation for the point displacements. When the motion is too large for any smooth spreading, cells must be added, removed or regenerated.

## Explanation

- **The task.** Extend prescribed boundary motion into the interior while keeping useful element shapes and resolution; it does not change the continuum solution if the equations are transformed consistently, but affects approximation, conditioning and geometry handling. At a moving boundary with velocity `v_b`, `(w - v_b) . n = 0`; tangential mesh motion may redistribute boundary nodes. The map must stay one-to-one with positive element Jacobians ([[ref-yao-2026-ale-chalmers]], sec. 7.1).
- **Spring analogy.** Edges are springs of stiffness `K_ij`; interior displacement increments solve the spring equilibrium (Batina 1990); with uniform stiffness each node's increment is the average of its neighbours', a graph Laplacian (on a Cartesian grid the five- or seven-point stencil) ([[ref-yao-2026-ale-chalmers]], sec. 7.2, eq. 44).
- **Laplacian smoothing.** Solve `div(gamma grad w) = 0` with the boundary mesh velocity prescribed; constant `gamma` gives componentwise harmonic extension. A diffusivity that depends on distance to the moving boundary or on cell size protects small or sensitive cells by making them stiffer; if `gamma` is frozen from known geometry the problem is linear, otherwise it needs iteration. The result must still be checked for tangling near boundary layers, in non-convex domains or under large motion. The unknown may be coordinates, displacement increments or velocity, giving different update procedures ([[ref-yao-2026-ale-chalmers]], sec. 7.2, eq. 45).
- **Elastic mesh motion.** Treat the displacement increment as that of a fictitious elastic solid with chosen Lame parameters (Johnson and Tezduyar 1994); it resists shear distortion but "does not guarantee valid elements under large deformation or finite rotation" ([[ref-yao-2026-ale-chalmers]], sec. 7.3).
- **Quality and remeshing.** Diagnostics include the Jacobian, its singular values, shape measures, non-orthogonality and skewness, with no universal thresholds (table 2). Remeshing regenerates the mesh before distortion makes it unusable and needs solution transfer; "simple interpolation need not conserve mass, momentum, or energy" ([[ref-yao-2026-ale-chalmers]], sec. 7.4).
- **Topology changes.** A smooth one-to-one map preserves topology; breakup, coalescence or contact need other treatment ([[ref-yao-2026-ale-chalmers]], sec. 8.1). Layering (adding and removing cell layers, as for a piston) is in the map's scope for this node; the source read does not describe it.

## Builds on

- [[ale-formulation]]: the mesh velocity whose interior values these methods choose.
- Floor: the Laplace equation.

## Leads to

- [[dynamic-mesh]], [[geometric-conservation-law]] (the chosen motion must give consistent swept volumes), [[mesh-quality]]

## How rcfd uses it

- **Interior motion:** `rigid` or `smoothing` (Laplacian, inverse distance, inverse volume `1/V^k`, F-290, ADR 0203); every moved mesh is validated ([[dynamic-mesh]]).
- **Topology changes:** layering on a `box_3d` piston stack (F-230, ADR 0090; tree F-259, ADR 0191/0257; prisms F-257, ADR 0213; thresholds 0.4 / 1.6 in the tree); local tet remeshing (F-247, ADR 0215, library, compressible only) ([[dynamic-mesh]], [[adr-thread-layering-remeshing-ale]]).
