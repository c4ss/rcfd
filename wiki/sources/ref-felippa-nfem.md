---
type: source
tags: [reference, foundation, structural, fem, nonlinear, stability, cite-only]
sources: []
origin: https://www.colorado.edu/engineering/CAS/courses.d/NFEM.d/Home.html
licence: none stated on the course page or in the chapters read. Unknown, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Felippa, Nonlinear Finite Element Methods (NFEM, ASEN 6107 lecture notes, University of Colorado Boulder)

**Origin:** C. A. Felippa, *Nonlinear Finite Element Methods*, lecture notes for ASEN 6107, University of Colorado at Boulder. Read from the Internet Archive copies of the author's course site (`web.archive.org/web/2019id_/https://www.colorado.edu/engineering/CAS/courses.d/NFEM.d/NFEM.ChNN.d/NFEM.ChNN.pdf`). The chapter numbers printed inside some PDFs differ from the file numbers; locators below give the printed chapter and the file.
**Kind:** open lecture notes (PDF chapters).
**Licence:** none stated; cite only.
**Read:** 2026-11-22 by job p6-structural: printed ch. 23 "Newton Method: General Control and Variants" (file `NFEM.Ch26.pdf`), ch. 24 "Bifurcation: Linearized Prebuckling I" (`NFEM.Ch24.pdf`), ch. 25 "Bifurcation: Linearized Prebuckling II" (`NFEM.Ch25.pdf`, sec. 25.3-25.4 on assumptions and limitations), ch. 28 "Structural Stability: Basic Concepts" (`NFEM.Ch28.pdf`). Ch. 21 (load control) was skimmed for its introduction only.

## Summary

Residual equations of a structure under a load parameter $\lambda$, their solution by Newton-like methods with increment control, the static stability test by the tangent stiffness, the classification of critical points (bifurcation and limit points) and linearized prebuckling as an eigenvalue problem with its assumptions.

## Key points

- **Ch. 23 (file 26), sec. 23.1:** the conventional Newton method (CNM) has two drawbacks: high cost (the tangent stiffness $\mathbf K_k = \mathbf K(\mathbf u_k, \lambda_k)$ is formed and factored every iteration) and low reliability (convergence needs a close start, and near bifurcation points it may converge to an unwanted solution). Variants: relaxed and damped Newton for reliability, modified (chord) and quasi-Newton for efficiency; "no modification can be said to be uniformly superior". With $\lambda$ varying, the iteration runs on a scalar increment-control constraint $c(\mathbf u, \lambda) = 0$ (sec. 23.2.2).
- **Ch. 21, introduction:** load control (stage-parameter control) is the simplest increment control; robust increment control and automatic traversal of limit points are treated in later chapters (not read).
- **Ch. 24, sec. 24.3, tangent stiffness test:** for conservative systems $\mathbf K$ is the Hessian of the total potential energy, real symmetric; with eigenvalues $\kappa_i$ of $\mathbf K\mathbf z_i = \kappa_i\mathbf z_i$ (24.1): all $\kappa_i > 0$ strongly stable, all $\kappa_i \ge 0$ neutrally stable, some $\kappa_i < 0$ unstable. The transition from stable to unstable passes through a zero eigenvalue, so $\det\mathbf K(\lambda_{cr}) = 0$ (24.3), $\mathbf K(\mathbf u_{cr}, \lambda_{cr})\mathbf z = 0$ (24.4) with $\mathbf z$ the buckling mode. The condition is necessary, not sufficient (stable-symmetric bifurcation, Euler column, Remark 24.3). Remark 24.2: factoring $\mathbf K = \mathbf L\mathbf D\mathbf L^T$ is enough to assess stability, since the number of negative eigenvalues equals the number of negative pivots of $\mathbf D$.
- **Ch. 24, secs. 24.4-24.5, linearized prebuckling (LPB):** neglect geometry change before the first critical state; split $\mathbf K = \mathbf K_M + \mathbf K_G$; under LPB $\mathbf K_M = \mathbf K_0$ at the reference configuration and $\mathbf K_G = \lambda\mathbf K_1$ linear in the load parameter; the stability eigenproblem $(\mathbf K_0 + \lambda\mathbf K_1)\mathbf z = 0$ (24.9), a generalised symmetric eigenproblem with real eigenvalues when $\mathbf K_0$ is positive definite; under the assumptions, the critical states found are bifurcation points, not limit points.
- **Ch. 25, LPB assumptions and limitations:** conservative loading, prebuckling deformations small, imperfections negligible; LPB fits structures like columns and frames and fits poorly arches, shells, very thin members and imperfection-sensitive structures (the axially compressed circular cylinder is the standard example) (sec. 25.4).
- **Ch. 28, stability concepts:** a structure is stable at an equilibrium position if it returns to it after an admissible perturbation (28.1, sec. 28.3.1): stable, unstable or neutral, with the critical value $\lambda_{cr}$ at the transition. Static instability comes as bifurcation (buckling: two or more equilibrium paths intersect; post-buckling behaviour) or as a limit point (snapping, snap-through: the load parameter reaches a maximum or minimum; post-snapping) (sec. 28.4.1). Both are critical points; "Transition from stability to instability can only occur at critical points". For design, the critical load is the first critical point met along the primary path (sec. 28.4.3, Fig. 28.4). For FEM models the energy method is practical: the Hessian of the potential energy is the tangent stiffness matrix (sec. 28.4.5).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[stability-of-equilibrium]], [[linear-buckling-and-stress-stiffening]], [[nonlinear-fem-newton]], [[arc-length-continuation]]
