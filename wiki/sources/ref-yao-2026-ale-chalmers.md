---
type: source
tags: [reference, foundation, ale, moving-mesh, gcl, fsi]
sources: []
origin: https://research.chalmers.se/en/publication/551352
licence: none stated on the record page or found in the PDF text. Unknown, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Yao (2026), Arbitrary Lagrangian-Eulerian (ALE) Formulation: Theory, Numerical Methods, and Applications

**Origin:** H. Yao, "Arbitrary Lagrangian-Eulerian (ALE) Formulation: Theory, Numerical Methods, and Applications", technical report, Chalmers University of Technology, Mechanics and Maritime Sciences (M2), 2026, doi:10.63959/m2.techreport/2026.1. Full text: https://research.chalmers.se/publication/551355/file/551355_Fulltext.pdf.
**Kind:** technical report / course text, about 27 pages; locators are its section, equation and table numbers.
**Licence:** unknown; cite only.
**Read:** 2026-11-22 by job p6-linear-rotating: secs. 2.3-3.3, 5.1-5.5, 6.4, 7.1-7.4, 7.6-7.7, 7.9, 8.1-8.2.

## Summary

A systematic ALE text: the map from a fixed reference domain to the moving domain, mesh, material and relative velocities, the ALE time derivative, the Jacobian evolution, the Reynolds transport theorem and balance laws on moving domains, the continuous and discrete geometric conservation law (GCL) with worked checks, finite element and finite volume ALE discretisations, mesh-motion strategies and quality metrics, ALE-FSI interface conditions and partitioned/monolithic coupling, a fluid-loaded piston model of added mass, and a comparison with VOF, level-set, immersed-boundary and overset alternatives.

## Key points

- **Sec. 2.3 (fig. 3):** mesh velocity $w$ is the time derivative of the map at fixed reference point; relative velocity $c = v - w$; only $c \cdot n$ enters the advective flux through a moving face; a face is material in the normal direction when $(v - w) \cdot n = 0$.
- **Sec. 2.4:** the ALE derivative (at fixed reference coordinate) equals the spatial time derivative plus $w \cdot \nabla f$; the material derivative then follows with $c \cdot \nabla f$; a moving-interval example shows a non-zero rate at a mesh point need not mean a change for a fluid particle.
- **Sec. 2.5, eq. (10):** the Jacobian $J$ evolves as $dJ/dt = J\, \nabla \cdot w$ at fixed reference point; expanding and contracting meshes are both admissible and $\nabla \cdot w$ need not vanish for incompressible flow. Eq. (11): the Piola transformation of fluxes.
- **Sec. 3.1-3.3, eqs. (13)-(14) and following:** the Reynolds transport theorem for a moving control volume; ALE identifies the boundary velocity with the mesh velocity; with $w = 0$ the Eulerian form, with $w = v$ the Lagrangian; the integral balance has advective flux $\phi\, (v - w) \cdot n$ through the moving surface; in the spatial-divergence form a mesh-dilatation term must accompany the ALE derivative, in the convective form it cancels.
- **Sec. 5.1:** the continuous GCL: the rate of change of the mesh volume equals the divergence of the transformed mesh flux; a uniform flow is then preserved without requiring $\nabla \cdot w = 0$.
- **Sec. 5.2, eq. (28)-(29):** the swept volume of a face over a step; the discrete GCL (DGCL): the change of a cell's volume over the step equals the sum of its faces' swept volumes; a product $w_f \cdot n_f A_f$ represents the swept volume only if its time and space quadrature reproduce the swept-volume integral. Lesoinne and Farhat (1996), Farhat et al. (2001) on DGCL and stability.
- **Sec. 5.3:** an expanding square: old-time flux gives an area increment 2, new-time flux 4, the midpoint flux the exact 3 (because the face flux is linear in time).
- **Sec. 5.5:** constant-state preservation, global conservation, accuracy and stability are four separate properties; a method can conserve through shared fluxes yet fail to preserve a constant state if volumes and mesh fluxes disagree; Geuzaine et al. (2003): DGCL "is neither necessary nor sufficient for retaining temporal order" in the schemes they examined.
- **Sec. 6.4, eq. (41):** a first-order finite-volume update with exact geometric cancellation, upwinded by the sign of the relative face flux.
- **Sec. 7.1:** mesh motion extends boundary motion into the interior; geometric conformity $(w - v_b) \cdot n = 0$; the mesh must stay injective with positive Jacobians.
- **Sec. 7.2, eqs. (44)-(45):** spring analogy (Batina 1990): interior displacement increments from edge stiffnesses, equal to neighbour averages for uniform stiffness (a graph Laplacian); Laplacian extension $\nabla \cdot (\gamma \nabla w) = 0$ with a diffusivity that may depend on distance or cell size to protect small cells; linear when $\gamma$ is frozen.
- **Sec. 7.3, eqs. (46)-(47):** elastic mesh motion (Johnson and Tezduyar 1994), fictitious Lame parameters; it does not guarantee valid elements under large deformation.
- **Sec. 7.4, table 2:** mesh diagnostics; remeshing needs solution transfer, simple interpolation need not conserve.
- **Sec. 7.6:** FSI interface conditions: velocity continuity and traction balance; at an impermeable interface the relative advective flux is zero; for non-matching interface meshes the load- and velocity-transfer operators should preserve the interface power.
- **Sec. 7.7, table 3:** partitioned (one-pass or strongly coupled, iterating within the step) vs monolithic coupling; Causin et al. (2005): the controlling inertia is fluid added mass relative to structural mass, "a density ratio alone is not a complete criterion", and conventional loosely coupled schemes "can be unstable regardless of how small the time step is"; strong coupling uses relaxation, interface quasi-Newton or Robin conditions; "simply repeating an unrelaxed exchange need not converge"; a seven-step partitioned ALE-FSI sequence (table 3).
- **Sec. 7.9:** a piston in a fluid-filled tube: linearisation introduces the constant added mass $m_a = \rho_0 A L_0$; for $m_a / m_s = 1$ the natural frequency is $1/\sqrt{2}$ of the dry frequency.
- **Sec. 8.1-8.2:** a one-to-one map cannot represent breakup or contact; alternatives: VOF, level sets, the immersed boundary method (Peskin 2002, regularised forces between an Eulerian fluid and a Lagrangian boundary), overset grids (interpolation across overlaps needs separate accuracy and conservation checks).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[ale-formulation]], [[geometric-conservation-law]], [[mesh-motion-smoothing]], [[partitioned-fluid-structure-interaction]], [[added-mass]], [[immersed-boundary-method]], [[non-conformal-interfaces]], [[overset-interpolation]]
