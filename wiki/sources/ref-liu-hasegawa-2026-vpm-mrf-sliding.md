---
type: source
tags: [reference, foundation, rotating, mrf, sliding-mesh, immersed-boundary]
sources: []
origin: https://arxiv.org/abs/2601.10230
licence: CC BY 4.0 (arXiv abstract page licence link licenses/by/4.0, read 2026-11-22). Copyable; not copied to raw/ because the PDF is 14.6 MB (left for the lead).
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Liu and Hasegawa (2026), Volume penalization method for flows around a rotating solid with multiple reference frame and sliding mesh

**Origin:** M. Liu and Y. Hasegawa, "Volume penalization method for simulating flows around a rotating solid with multiple reference frame and sliding mesh", arXiv:2601.10230 (15 January 2026); *Computers and Fluids*, doi:10.1016/j.compfluid.2026.107235. Institute of Industrial Science, The University of Tokyo.
**Kind:** research paper (preprint).
**Licence:** CC BY 4.0.
**Read:** 2026-11-22 by job p6-linear-rotating: full text (equations partly lost in the text extraction; described in words).

## Summary

Combines the volume penalisation method (a diffuse-interface immersed-boundary method) with the multiple reference frame (MRF) and sliding mesh (SLM) approaches in OpenFOAM, and compares both with body-fitted runs on a rotating cuboid in a channel: pressure drop within about 5 percent and torque within 8-9 percent.

## Key points

- **Sec. 1, MRF vs SLM:** the domain is split into rotating and stationary regions. MRF "simplifies the simulation by fixing a rotating object inside the rotating region, while only inertia terms arising from the rotation of the frame of reference are taken into account in the momentum equation"; it neglects the unsteady interaction with stationary boundaries and "often fails to capture detailed transient characteristics". SLM rotates the grids of the rotating region at the prescribed speed and reproduces the transient flow.
- **Sec. 1, immersed-boundary methods:** body forcing (actuator disk) models a rotor by its thrust; the direct-forcing immersed boundary method adds a term to the momentum equation to enforce the boundary condition on the solid surface; IBMs are "sharp" or "diffused" depending on whether the interface is kept sharp or spread over a few cells; volume penalisation, a diffused IBM, models the solid as a porous medium of nearly zero permeability and drives the velocity inside the solid to the target velocity with one set of equations for fluid and solid.
- **Sec. 2.2, grid systems (fig. 2):** in MRF the grid is stationary everywhere and the absolute velocity in the rotating region is split as $u_a = u_\Omega + u_r$ with $u_\Omega = \Omega \times r$; in SLM a sliding interface separates rotating and stationary grids, nodes are remapped each step and the regions are coupled "through the continuity of the local velocity and stresses".
- **Sec. 2.3, MRF equations (2)-(5):** the relative-velocity momentum equation in the rotating frame carries Coriolis $-2 \epsilon_{ijk} \Omega_j u_k^r$ and centrifugal terms; solving for the relative velocity "causes the jump of the relative velocity across the interface between rotating and stationary regions", which "results in numerical instabilities", so the authors substitute $u_r = u_a - \Omega \times r$ and solve for the absolute velocity (eqs. 4-5), avoiding the jump.
- **Sec. 2.4-2.5, SLM equations (6)-(7):** absolute-velocity momentum in the absolute frame with the radius vector updated as the grid rotates; interface conditions are continuity of absolute velocity and of stress (eq. 7); a pressure jump can appear across the sliding interface from inconsistency inside a PISO loop, treated by an extra pressure-correction Poisson solve.
- **Sec. 2.6:** for SLM the regions are coupled by OpenFOAM's arbitrary mesh interface (AMI), "projecting grid nodes of the rotating region into the stationary region across the sliding interface".
- **Sec. 3.4, results:** at low rotation speed MRF predicts a slightly higher pressure drop than SLM (its inertia sources "assume a fully developed rotating flow"); at higher speed it predicts a lower one because "the transient interaction between rotating and stationary parts is not considered in MRF". A non-negligible pressure discontinuity appears at the rotating-stationary interface for both methods (fig. 8). VPM leaves a small non-zero wall velocity, delaying separation.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[relative-and-absolute-velocity-formulations]], [[frozen-rotor-and-mixing-plane]], [[non-conformal-interfaces]], [[immersed-boundary-method]], [[mrf]], [[sliding-mesh]]
