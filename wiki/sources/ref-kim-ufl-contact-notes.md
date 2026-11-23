---
type: source
tags: [reference, foundation, structural, contact, cite-only]
sources: []
origin: https://web.mae.ufl.edu/nkim/egm6352/Chap5.pdf
licence: none stated in the notes. Unknown, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Kim, Finite Element Analysis of Contact Problem (EGM 6352 course notes, ch. 5, University of Florida)

**Origin:** N.-H. Kim, course notes for EGM 6352 (nonlinear finite element analysis), chapter 5 "Finite Element Analysis of Contact Problem", Department of Mechanical and Aerospace Engineering, University of Florida (slide-style PDF).
**Kind:** course lecture slides (PDF). Locators are the slide titles.
**Licence:** none stated; cite only.
**Read:** 2026-11-22 by job p6-structural: the opening slides, "Contact Problem - Boundary Nonlinearity", "Why are contact problems difficult?", the cantilever-beam contact example with and without friction, and the start of "General Contact Formulation". The later variational-inequality and implementation slides were not read.

## Summary

Contact as a boundary nonlinearity: both the contact region and the contact force are unknown; non-penetration is an inequality constraint enforced by Lagrange multipliers or a penalty, with a master-slave pairing of surfaces; friction adds stick and slip conditions.

## Key points

- **Opening slides:** contact is a boundary nonlinearity; the force-displacement graph becomes vertical; both displacement and contact force are unknown at the interface; the tasks are to find whether and where bodies touch, the contact pressure and any relative slip. Equilibrium is the minimum of potential energy subject to the contact constraint, turned into an unconstrained problem by the penalty or Lagrange multiplier method. "Slave-master concept": the slave nodes cannot penetrate the master surface elements.
- **"Why are contact problems difficult?":** the contact boundary is part of the solution; the force changes abruptly, which is difficult for Newton-Raphson; once in contact the force cannot be found from the displacement (an analogy with incompressibility); the discrete contact boundary changes node by node and is sensitive to discretisation.
- **Cantilever on a rigid block:** gap condition $g = v_{tip} - G \le 0$, contact force $\lambda \ge 0$, consistency $\lambda g = 0$; with a Lagrange multiplier the multiplier is the contact force (here 75 N); with a penalty, small penetration is allowed and $\lambda = K_N\langle g\rangle$, the force proportional to penetration, a larger penalty giving less penetration.
- **Friction:** stick when $|t| < \mu\lambda$ (no slip), slip when $|t| = \mu\lambda$ (tangential force at the Coulomb limit), consistency between slip and force; also treated by a tangential penalty; friction makes the result depend on the load sequence (path-dependent).
- **General formulation:** contact of a flexible body with a rigid surface by closest-point projection of a point onto the surface; normal gap $g_n = (\mathbf x - \mathbf x_c(\xi_c))^T\mathbf e_n(\xi_c)$ with the impenetrability condition $g_n \ge 0$ on the possible contact boundary; a tangential slip function for friction.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[contact-mechanics]]
