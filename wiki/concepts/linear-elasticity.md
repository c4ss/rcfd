---
type: concept
tags: [foundation, structural]
sources: [ref-roylance-mit-3-11-mechanics-of-materials, ref-bathe-mit-2-092-fea-notes, adr-thread-structural-hyperelastic-and-follower, adr-thread-structural-materials-nonlinear, adr-thread-structural-basic-tier]
builds_on: [stress, strain-and-strain-rate]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Linear elasticity (Hooke's law, small-strain tensor, elastic moduli)

## In short

For small deformations most solids behave like springs: stress is proportional to strain, and the material returns to its original shape when unloaded. In three dimensions this proportionality links the six stress components to the six strain components; an isotropic material needs only two constants, such as Young's modulus $E$ and Poisson's ratio $\nu$ ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Constitutive Equations").

## Explanation

**Small-strain tensor.** With displacement $\mathbf u = (u, v, w)$, strain is a displacement gradient. The symmetric (tensor) strain is $$\epsilon_{ij} = \tfrac12\,(u_{i,j} + u_{j,i})$$ (Roylance, "The Kinematic Equations", eq. 4). It has six independent components; the normal parts are the stretches $\partial u/\partial x$ (eq. 1), the shear parts are half the engineering shear strain $\gamma_{xy} = \partial v/\partial x + \partial u/\partial y$ (eq. 3). In the vector ("pseudovector") form used by finite element codes, $\boldsymbol\epsilon = \mathbf L\mathbf u$ with a matrix of derivative operators (eq. 8). The trace $\epsilon_{kk}$ is the volume change per unit volume (eq. 10). These formulas drop products of displacement gradients; they hold only while the gradients are small, and the Lagrangian strain keeps the quadratic terms (eqs. 11-13; see [[finite-strain-kinematics]]) ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Generalised Hooke's law.** A linear law $\epsilon_{ij} = S_{ijkl}\sigma_{kl}$ has 81 coefficients; the symmetry of stress and strain leaves 36, the symmetry of the compliance matrix 21 (Roylance, "Constitutive Equations", eq. 1). For an isotropic material only two remain: $$\epsilon_{ij} = \frac{1+\nu}{E}\,\sigma_{ij} - \frac{\nu}{E}\,\delta_{ij}\,\sigma_{kk}$$ (eq. 4), with shear modulus $G = E/2(1+\nu)$. Inverting gives the stiffness form $\boldsymbol\sigma = \mathbf D\boldsymbol\epsilon$, $\mathbf D = \mathbf S^{-1}$ (eqs. 5-6). A temperature change adds a free thermal strain $\alpha\,\Delta T\,\delta_{ij}$ (Problem 1) ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Volumetric and deviatoric parts.** Splitting stress into its mean (hydrostatic) part and the deviator $\sigma'_{ij}$ (eq. 7) decouples the law: $$\tfrac13\sigma_{kk} = K\,\epsilon_{kk}, \qquad \sigma'_{ij} = 2G\,e_{ij}$$ (eqs. 8-9), with bulk modulus $K = E/3(1-2\nu)$. Steel: $E = 207$ GPa, $\nu = 0.3$, $K = 173$ GPa, $G = 79.6$ GPa (Example 2). Plastic and viscous flow are driven mainly by the deviatoric part; the hydrostatic part produces only elastic volume change. As $\nu \to 0.5$, $K \to \infty$: the nearly incompressible limit behind volumetric locking ([[element-locking]]) ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Anisotropy.** A fibre composite or wood is stiffer along the fibres. In principal material axes the transversely isotropic (or orthotropic) compliance has no coupling between normal and shear terms, with $\nu_{21}/E_2 = \nu_{12}/E_1$ (eq. 11). Off-axis, the transformed compliance $\bar{\mathbf S}$ (eq. 12) is fully populated and a normal stress produces shear strain; a Kevlar-epoxy ply at 30 degrees has an apparent $E_x = 11.33$ GPa against $E_1 = 82$ GPa (Example 3) ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**In a finite element code.** The law enters as the matrix $\mathbf C$ (or $\mathbf D$) in $\int\mathbf B^T\mathbf C\mathbf B\,dV$; because $\mathbf C$ is symmetric the stiffness matrix is symmetric ([[ref-bathe-mit-2-092-fea-notes]], lecture 7).

**What it adds.** [[stress]] and [[strain-and-strain-rate]] are kinematic and static descriptions; linear elasticity is the material law that closes the system: three equilibrium equations and six kinematic equations in fifteen unknowns need six constitutive equations ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Constitutive Equations", introduction).

## Builds on

- [[stress]]: the stress tensor and its symmetry.
- [[strain-and-strain-rate]]: strain as a measure of deformation; the small-strain tensor above is the solid-mechanics form.

## Leads to

- [[equilibrium-equations]], [[principle-of-virtual-work]], [[finite-element-method-basics]], [[beam-theory]], [[element-locking]], [[plasticity-j2]], [[viscoelasticity-and-creep]], [[fracture-mechanics]]
- [[structural-solver]], [[hyperelastic-materials]] (linearised moduli)

## How rcfd uses it

- The basic tier of the [[structural-solver]] is linear elastostatics and modal analysis on Tet4, Hex8, Tet10 and Hex20, with centrifugal and thermal strain loads ([[structural-solver]]; [[adr-thread-structural-basic-tier]]).
- Orthotropic elasticity from nine constants with fixed, cylindrical or per-element axes and temperature tables (F-409, ADR 0452) ([[adr-thread-structural-materials-nonlinear]]).
- On the linear paths, the hyperelastic material is used through its small-strain moduli, with $E = 9KG/(3K+G)$ and $\nu = (3K-2G)/(2(3K+G))$ on Beam2, Shell4 and Shell3 ([[hyperelastic-materials]]; [[adr-thread-structural-hyperelastic-and-follower]]).
