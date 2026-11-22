---
type: concept
tags: [foundation, structural, stability, nonlinear]
sources: [ref-felippa-nfem, adr-thread-structural-modal-buckling-cyclic, adr-thread-structural-beams-shells]
builds_on: [nonlinear-fem-newton]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Stability of equilibrium (bifurcation and limit points)

## In short

An equilibrium state is stable if the structure returns to it after a small disturbance. As load grows, stability can be lost in two ways: at a bifurcation point, where a second equilibrium path branches off (buckling), or at a limit point, where the load reaches a maximum and the structure snaps. Both show up as a singular tangent stiffness ([[ref-felippa-nfem]] ch. 24, 28).

## Explanation

**Definition.** A structure is stable at an equilibrium position if it returns to it after an admissible perturbation; otherwise unstable, or neutral in between; the load value at the transition is critical, $\lambda_{cr}$ (Felippa, sec. 28.3.1) ([[ref-felippa-nfem]]).

**Two kinds of critical point** (sec. 28.4.1):

- *bifurcation* (buckling): two or more equilibrium paths cross; what follows is post-buckling behaviour;
- *limit point* (snapping, snap-through): the load parameter reaches a local maximum or minimum along the path; what follows is post-snapping.

"Transition from stability to instability can only occur at critical points." For design the critical load is the first critical point met along the primary path (sec. 28.4.3) ([[ref-felippa-nfem]]).

**Energy test.** For conservative systems the tangent stiffness $\mathbf K$ is the Hessian of the total potential energy (sec. 28.4.5). With eigenvalues $\kappa_i$ of $\mathbf K\mathbf z_i = \kappa_i\mathbf z_i$ (24.1): all positive is strongly stable, all non-negative neutrally stable, any negative unstable. Loss of stability passes through $\det\mathbf K(\lambda_{cr}) = 0$ (24.3), with the null vector $\mathbf z$ (24.4) the buckling mode. The test is necessary, not sufficient: the Euler column has a stable-symmetric bifurcation (Remark 24.3) ([[ref-felippa-nfem]]).

**Pivots count negative eigenvalues.** Factoring $\mathbf K = \mathbf L\mathbf D\mathbf L^T$ is enough: the number of negative eigenvalues equals the number of negative pivots of $\mathbf D$ (Remark 24.2) ([[ref-felippa-nfem]]). This is the Sylvester inertia count also used by shifted eigensolvers ([[eigenvalue-solvers]]).

**Following a path past a limit point.** Under load control the Newton iteration fails at a limit point, because the load cannot increase further; increment control with the load factor as an unknown (arc length) is needed ([[ref-felippa-nfem]], ch. 21, 23; [[arc-length-continuation]]). The arc-length chapters of NFEM were not read in this job.

**Linear approximation.** Ignoring prebuckling geometry change turns the test into the eigenproblem of [[linear-buckling-and-stress-stiffening]], which finds bifurcations only ([[ref-felippa-nfem]], ch. 24).

## Builds on

- [[nonlinear-fem-newton]]: equilibrium paths and the tangent stiffness.

## Leads to

- [[arc-length-continuation]], [[linear-buckling-and-stress-stiffening]]

## How rcfd uses it

- Arc-length continuation through limit points (F-424, ADR 0480) ([[arc-length-continuation]]).
- Buckling on the transformation path refuses mechanisms by a zero-shift pivot ratio (F-496) ([[adr-thread-structural-modal-buckling-cyclic]]).
- The hinged cylindrical roof limit load with corotated shells (F-610) ([[structural-solver]]; [[adr-thread-structural-beams-shells]]).
