---
type: concept
tags: [foundation, structural]
sources: [ref-roylance-mit-3-11-mechanics-of-materials, ref-bathe-mit-2-092-fea-notes]
builds_on: [stress, divergence-theorem]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Equilibrium equations (strong form of elastostatics)

## In short

A body at rest has every piece of it in force balance. Applied to an arbitrarily small piece, that balance says the stress may vary from point to point only in a way that its divergence cancels the body force; at the surface, the stress must match the applied tractions ([[ref-roylance-mit-3-11-mechanics-of-materials]], "The Equilibrium Equations").

## Explanation

**Traction and Cauchy's relation.** The traction $\mathbf T$ is the force per unit area on a plane. The stress matrix turns the plane's unit normal into the traction on it: $$[\boldsymbol\sigma]\,\hat{\mathbf n} = \mathbf T$$ (Roylance, eq. 3). Moment equilibrium makes $\boldsymbol\sigma$ symmetric ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Differential equilibrium.** Requiring the traction integrated over the surface of any volume to vanish, and converting the surface integral by the divergence theorem, gives (without body forces) $\nabla\cdot\boldsymbol\sigma = 0$, or $\sigma_{ij,j} = 0$ (Roylance, eqs. 5-7). In the vector form used with finite elements it is $\mathbf L^T\boldsymbol\sigma = 0$, the transpose of the kinematic operator of $\boldsymbol\epsilon = \mathbf L\mathbf u$ (eq. 9). Any constant stress satisfies it; the difficulty is satisfying the boundary conditions too ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**The boundary value problem.** With body force $f^B$ and the boundary split into $S_u$ (displacements prescribed) and $S_f$ (loads prescribed), the exact solution must satisfy ([[ref-bathe-mit-2-092-fea-notes]], lecture 4):

- equilibrium $\tau_{ij,j} + f_i^B = 0$ in $V$, and $\tau_{ij}n_j = f_i^{S_f}$ on $S_f$;
- compatibility: $u_i = u_i^{S_u}$ on $S_u$, displacements continuous;
- the stress-strain law ([[linear-elasticity]]).

Bathe calls this the differential formulation. A point may belong to both $S_u$ and $S_f$ in different directions. Equilibrium and kinematics are independent of the material; the constitutive law supplies the missing six equations ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Constitutive Equations").

**What it adds.** [[stress]] describes the internal force state at a point; the equilibrium equations constrain how it may vary in space. They are Newton's law without acceleration; with inertia the body force gains the term $-\rho\ddot{\mathbf u}$ ([[ref-bathe-mit-2-092-fea-notes]], lecture 7; see [[structural-dynamics]]).

## Builds on

- [[stress]]: the stress tensor, traction, symmetry.
- [[divergence-theorem]]: turns the surface integral of traction into a volume integral of $\nabla\cdot\boldsymbol\sigma$.

## Leads to

- [[principle-of-virtual-work]] (the weak form of these equations), [[finite-element-method-basics]], [[structural-dynamics]]

## How rcfd uses it

- The [[structural-solver]] solves the discrete form of these equations; its fan-blade demonstration reports reactions that balance the applied loads "to about 1e-11" ([[structural-solver]]).
