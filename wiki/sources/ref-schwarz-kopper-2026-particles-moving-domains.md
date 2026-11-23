---
type: source
tags: [reference, foundation, multiphase, particles, erosion, cite-only]
sources: []
origin: https://arxiv.org/abs/2606.17969
licence: arXiv non-exclusive distribution licence 1.0 (arXiv licence link, read 2026-11-22). Cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Schwarz and Kopper (2026), particle-laden flows in moving domains (ALE and sliding mesh)

**Origin:** A. Schwarz, P. Kopper, "High-Order Simulation of Particle-Laden Flows in Moving Domains Using Coupled ALE and Sliding Mesh Approaches", arXiv:2606.17969 [physics.comp-ph], 16 June 2026. PDF: https://arxiv.org/pdf/2606.17969
**Kind:** research preprint (University of Stuttgart group).
**Licence:** arXiv non-exclusive distribution licence; cite only.
**Read:** 2026-11-22 by job p6-multiphase-heat: abstract, sec. 1, sec. 2.2 "Dispersed Phase" (Eqs. 4-6, particle-wall collisions), the start of sec. 2.4 (particle-induced wall deformation), the erosion application (sec. 5.1) skimmed, sec. 6.

## Summary

Euler-Lagrange framework (discontinuous Galerkin carrier, Lagrangian point particles) for compressible particle-laden flow in moving meshes, applied to sand erosion of a compressor blade.

## Key points by section

### Sec. 2.2 "Dispersed Phase"
- Point particles of mass $m_p$ and diameter $d_p$: $dx_p/dt = v_p$, $m_p\,dv_p/dt = F_D = 3\pi\mu d_p f_D(u_f - v_p)$ (Eq. 4), following the Maxey-Riley-Gatignol formulation reduced to drag only, with an empirical drag factor $f_D$ for high particle Reynolds numbers.
- Particle-wall collisions by the hard-sphere approach, "not resolved in time but handled in an a posteriori manner"; momentum change at impact with the wall normal $n$ (Eq. 5). Energy dissipation is not inherent in Eq. 5; real collisions are inelastic.
- Rebound models relate pre- and post-impact kinematics through the coefficient of restitution $e_x = x^{ref}/x^{imp}$ (Eq. 6), usually for the normal ($e_n$) and tangential ($e_t$) velocities; $e_x$ generally depends on parameters of the collision physics (a neural-network rebound model is used here).

### Sec. 2.4 and 5.1
- Each impact can remove (erosion) or add (deposition) wall material; erosion models correlate the energy a particle dissipates at impact with an erosion rate, converted to a volume loss with the wall material density. The rebound model scatters velocities and reflection angles; the predicted erosion profile agreed with measured blade wear except near the leading and trailing edges.

## Contradictions with the wiki

None.

## Pages this source informs

[[particle-wall-interaction]], [[particle-equation-of-motion]]
