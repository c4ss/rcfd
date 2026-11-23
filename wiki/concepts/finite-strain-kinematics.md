---
type: concept
tags: [foundation, structural, nonlinear]
sources: [ref-bathe-mit-2-094-fea-notes, ref-roylance-mit-3-11-mechanics-of-materials, adr-thread-structural-materials-nonlinear, adr-thread-structural-hyperelastic-and-follower]
builds_on: [strain-and-strain-rate, stress]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Finite-strain kinematics (deformation gradient, Green-Lagrange strain, Piola-Kirchhoff stress)

## In short

When displacements or rotations are large, the small-strain tensor gives wrong answers: a pure rotation would show up as strain. Finite-strain kinematics describes deformation by the deformation gradient, which maps material line elements from the original to the current shape, and measures strain with quantities that ignore rigid rotation, such as the Green-Lagrange strain. Stress is then measured by a matching quantity on the original configuration, the second Piola-Kirchhoff stress ([[ref-bathe-mit-2-094-fea-notes]], lectures 11-12).

## Explanation

**Deformation gradient.** A material line element maps as $$d\,{}^t\mathbf x = {}^t_0\mathbf X\;d\,{}^0\mathbf x$$ (Bathe, eq. 11.5), with ${}^t_0\mathbf X$ (usually called $\mathbf F$) collecting $\partial\,{}^tx_i/\partial\,{}^0x_j$ ([[ref-bathe-mit-2-094-fea-notes]]).

**Stretch and Cauchy-Green tensor.** The right Cauchy-Green tensor $\mathbf C = \mathbf X^T\mathbf X$ (11.6) gives the stretch of a fibre of direction $\mathbf n$: $\lambda = (\mathbf n^T\mathbf C\,\mathbf n)^{1/2}$ (11.14) ([[ref-bathe-mit-2-094-fea-notes]]).

**Polar decomposition.** $\mathbf X = \mathbf R\,\mathbf U$ (11.29): a rotation times a symmetric stretch. Strain measures built from $\mathbf U$ or $\mathbf C$ do not see $\mathbf R$ ([[ref-bathe-mit-2-094-fea-notes]]).

**Green-Lagrange strain.** $$\boldsymbol\epsilon = \tfrac12\,(\mathbf X^T\mathbf X - \mathbf I) = \tfrac12\,(\mathbf C - \mathbf I), \qquad \epsilon_{ij} = \tfrac12\,(u_{i,j} + u_{j,i} + u_{k,i}u_{k,j})$$ (lecture 11). Its components are unchanged by a rigid-body rotation. Dropping the quadratic term recovers the small-strain tensor of [[linear-elasticity]] ([[ref-bathe-mit-2-094-fea-notes]]). Roylance gives the same Lagrangian strain (eq. 11) and shows that at 100 percent conventional strain the two measures differ by 50 percent (Example 2) ([[ref-roylance-mit-3-11-mechanics-of-materials]], "The Kinematic Equations").

**Second Piola-Kirchhoff stress.** The energy-conjugate stress on the original configuration is $${}^t_0\mathbf S = \frac{{}^0\rho}{{}^t\rho}\;{}^t_0\mathbf X^{-1}\;{}^t\boldsymbol\tau\;{}^t_0\mathbf X^{-T}$$ (12.11), built from the Cauchy stress ${}^t\boldsymbol\tau$; its components are also independent of rigid rotation, so a material law can be written as a relation between $\mathbf S$ and $\boldsymbol\epsilon$ ([[ref-bathe-mit-2-094-fea-notes]]; [[ref-roylance-mit-3-11-mechanics-of-materials]], "Finite strain model").

**Increments.** For Newton iterations the strain increment from a known state splits into a part linear in the displacement increment, $e_{ij}$ (12.21, including the effect of the initial displacement), and a quadratic part $\eta_{ij} = \tfrac12 u_{k,i}u_{k,j}$ (12.25). The linear part gives the material stiffness, the quadratic part the geometric (initial-stress) stiffness ([[ref-bathe-mit-2-094-fea-notes]]; [[nonlinear-fem-newton]], [[linear-buckling-and-stress-stiffening]]).

## Builds on

- [[strain-and-strain-rate]]: strain as a measure of deformation.
- [[stress]]: the Cauchy stress, here transformed to the reference configuration.

## Leads to

- [[nonlinear-fem-newton]], [[linear-buckling-and-stress-stiffening]], [[hyperelastic-materials]], [[incompatible-modes-hex8]], [[element-locking]]

## How rcfd uses it

- The Newton driver is total Lagrangian, with the tangent $K_{\mathrm{mat}}(F) + K_{\sigma}(S)$ from the deformation gradient and the second Piola-Kirchhoff stress (F-397, ADR 0404) ([[adr-thread-structural-materials-nonlinear]]).
- Hyperelastic laws (neo-Hookean and successors) are written on these kinematics ([[hyperelastic-materials]]; [[adr-thread-structural-hyperelastic-and-follower]]).
