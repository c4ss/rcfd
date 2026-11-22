---
type: source
tags: [reference, foundation, fsi, added-mass, rigid-body]
sources: []
origin: https://arxiv.org/abs/1611.05711
licence: arXiv non-exclusive licence to distribute (licenses/nonexclusive-distrib/1.0, read 2026-11-22). Cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Banks, Henshaw, Schwendeman and Tang (2016), A stable partitioned FSI algorithm for rigid bodies and incompressible flow, Part I

**Origin:** J. W. Banks, W. D. Henshaw, D. W. Schwendeman and Q. Tang, "A stable partitioned FSI algorithm for rigid bodies and incompressible flow. Part I: Model problem analysis", arXiv:1611.05711v1 [math.NA], 17 November 2016. Rensselaer Polytechnic Institute.
**Kind:** research paper (preprint).
**Licence:** arXiv non-exclusive distribution licence; cite only.
**Read:** 2026-11-22 by job p6-linear-rotating: abstract, sec. 1 and secs. 2-3 (model problem and traditional scheme); the stability analyses (secs. 5-6) only by their headings.

## Summary

An added-mass partitioned (AMP-RB) scheme for rigid bodies in incompressible viscous flow that stays stable without sub-iterations even for zero-mass bodies, by a Robin-type pressure condition including the body's accelerations and added-damping tensors for viscous shear; the traditional scheme (TP-RB) is analysed on model problems to show when it fails.

## Key points

- **Sec. 1, classification:** FSI methods for rigid bodies include ALE, level sets, fictitious domain, embedded boundary and immersed boundary methods; schemes are monolithic (one system) or partitioned (separate solvers coupled at the interface).
- **Sec. 1, added mass (fig. 1):** a force applied suddenly to a rigid body in an incompressible fluid accelerates the body and also "a portion of the surrounding fluid that is accelerated due to the instantaneous pressure field created by the motion of the body"; that added mass `M_a` adds to the effective mass. Heavy body: small effect, weak instability; light body: significant, unstable partitioned solvers.
- **Sec. 1, added damping:** a torque on a disk sets up a viscous boundary layer of width about `sqrt(nu dt)`; its drag damps the motion over time; added-damping instabilities can usually be removed by a smaller time step (except for zero mass), "while added-mass instabilities generally cannot be overcome by taking a smaller time-step if the mass of the body is too small relative to the added-mass".
- **Sec. 1, traditional coupling:** uses the body velocity as the fluid boundary condition and the fluid stress as the body load; implementations "usually become unstable when the ratio of the density of the solid to that of the fluid is approximately one or less"; the common remedy, under-relaxed sub-iterations often with Aitken acceleration, is expensive for light bodies.
- **Sec. 2, model problem:** a rectangular rigid body below a viscous incompressible fluid; vertical motion gives added mass, horizontal motion added damping; the pressure satisfies a Poisson equation from the divergence of momentum.
- **Sec. 3, TP-RB (Algorithm 1):** predictor-corrector fractional-step scheme, second order, unstable when the body-to-fluid mass ratio is too small.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[added-mass]], [[partitioned-fluid-structure-interaction]], [[immersed-boundary-method]]
