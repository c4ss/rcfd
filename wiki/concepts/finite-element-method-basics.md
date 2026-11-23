---
type: concept
tags: [foundation, structural, fem]
sources: [ref-bathe-mit-2-092-fea-notes, ref-bathe-mit-2-094-fea-notes, ref-roylance-mit-3-11-mechanics-of-materials, adr-thread-structural-basic-tier, adr-thread-structural-linear-solvers]
builds_on: [principle-of-virtual-work, sparse-linear-systems]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Finite element method basics (shape functions, element stiffness, assembly)

## In short

Cut the body into small elements joined at nodes, describe the displacement inside each element by simple polynomials of its nodal values, and apply the principle of virtual work element by element. The result is a large, sparse, symmetric system $\mathbf K\mathbf U = \mathbf R$ for the nodal displacements; strains and stresses follow from them ([[ref-bathe-mit-2-092-fea-notes]], lectures 5-7; [[ref-roylance-mit-3-11-mechanics-of-materials]], "Finite Element Analysis").

## Explanation

**Interpolation.** In element $m$, $\mathbf u^{(m)} = \mathbf H^{(m)}\hat{\mathbf U}$ (Bathe, lecture 5, eq. 1) with interpolation (shape) functions $h_i$ equal to 1 at node $i$ and 0 at the other nodes; for the 4-node plane element they are bilinear, e.g. $h_1 = \tfrac14(1+x)(1+y)$, and $\sum h_i = 1$. Admissible functions must represent constant strain and rigid-body motion and keep the displacement compatible between elements (lecture 7). In one dimension the linear functions are $N_0 = 1 - x$, $N_1 = x$ ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Strain-displacement matrix.** Differentiating the interpolation gives $\boldsymbol\epsilon^{(m)} = \mathbf B^{(m)}\hat{\mathbf U}$ (lecture 5, eq. 2), with $\mathbf B_j = \mathbf L N_j$ in Roylance's notation ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Finite Element Analysis"). The same interpolation is used for the virtual displacements (Galerkin choice).

**Element matrices and assembly.** Substituting into virtual work and taking unit virtual displacements one at a time gives $\mathbf K\mathbf U = \mathbf R$ with $$\mathbf K = \sum_m \int_{V^{(m)}} \mathbf B^{(m)T}\mathbf C^{(m)}\mathbf B^{(m)}\,dV^{(m)}, \qquad \mathbf R_B = \sum_m\int_{V^{(m)}}\mathbf H^{(m)T}\mathbf f^{B(m)}\,dV^{(m)}$$ and a surface-load vector $\mathbf R_S$ of the same form (Bathe, lecture 5). The sum is the assembly: each element matrix, computed in local numbering, is added into the global positions given by a local-to-global table; only nodes joined by an element couple, so $\mathbf K$ is mostly zeros and solvers exploit the sparsity ([[ref-roylance-mit-3-11-mechanics-of-materials]], matrix truss analysis). The integrals are evaluated by Gauss quadrature on mapped elements ([[isoparametric-elements-and-quadrature]]).

**Properties of $\mathbf K$.** Symmetric (because $\mathbf C$ is), positive diagonal, singular while rigid-body motions are possible; after enough displacement boundary conditions it is positive definite, so the strain energy is positive for any non-zero $\mathbf U$ ([[ref-bathe-mit-2-092-fea-notes]], lectures 6-7). Each nodal value is either a prescribed displacement or a prescribed force, never both ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**What the solution satisfies.** Element nodal forces $\mathbf F^{(m)} = \int\mathbf B^{(m)T}\boldsymbol\tau^{(m)}dV$ balance the applied loads at every node, and each element is in equilibrium under its own nodal forces; compatibility and the material law hold, but differential equilibrium is not satisfied pointwise. The error falls as the mesh is refined or the interpolation order raised ([[ref-bathe-mit-2-092-fea-notes]], lecture 7). In energy terms the error is orthogonal to the finite element space, $a(u - u_h, v_h) = 0$, and the computed strain energy never exceeds the exact one, $a(u_h, u_h) \le a(u, u)$ ([[ref-bathe-mit-2-094-fea-notes]], lecture 7, eqs. 7.30-7.31).

**Dynamics.** Interpolating accelerations like displacements adds the consistent mass $\mathbf M = \sum\int\rho\,\mathbf H^T\mathbf H\,dV$ ([[ref-bathe-mit-2-092-fea-notes]], lecture 7, eq. 5; see [[structural-dynamics]]).

## Builds on

- [[principle-of-virtual-work]]: the weak form that is discretised.
- [[sparse-linear-systems]]: $\mathbf K\mathbf U = \mathbf R$ is a large sparse symmetric positive definite system.

## Leads to

- [[isoparametric-elements-and-quadrature]], [[element-locking]], [[structural-dynamics]], [[multi-point-constraints]], [[nonlinear-fem-newton]], [[stress-recovery]], [[substructuring-and-craig-bampton]]
- [[structural-solver]], [[rcfd-fem]]

## How rcfd uses it

- The crate [[rcfd-fem]] fixed its data model in the basic tier: `DofMap` as the only place that knows the unknowns per node, the `Element` trait, `assemble_with` as the generic assembly hook (ADRs 0271, 0273, 0275) ([[adr-thread-structural-basic-tier]]).
- $\mathbf K\mathbf U = \mathbf R$ is solved by an envelope $\mathbf L\mathbf D\mathbf L^T$ direct solver (ADR 0274), preconditioned CG, elasticity multigrid, and a supernodal sparse $\mathbf L\mathbf D\mathbf L^T$ ([[adr-thread-structural-basic-tier]]; [[adr-thread-structural-linear-solvers]]).
- Element library verified against closed forms and observed orders, not measurements ([[adr-thread-structural-verification-elements]]).
