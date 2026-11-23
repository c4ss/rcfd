---
type: concept
tags: [foundation, structural, fem, constraints]
sources: [ref-felippa-ifem, adr-thread-structural-modal-buckling-cyclic, adr-thread-structural-beams-shells, adr-thread-structural-contact]
builds_on: [finite-element-method-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Multi-point constraints (elimination, penalty, Lagrange multipliers)

## In short

Sometimes nodal displacements must obey extra equations: two nodes move together (a rigid link), a node slides along a skewed support, a sector's edge copies its neighbour's motion. These multifreedom constraints are imposed on the assembled stiffness equations in one of three ways: eliminate dependent ("slave") unknowns by a transformation, add stiff "penalty" springs, or add Lagrange multipliers that are the constraint forces ([[ref-felippa-ifem]] ch. 8-9).

## Explanation

**Form.** A multifreedom constraint (MFC) links two or more displacement components, "F(nodal displacement components) = prescribed value" (Felippa, eq. 8.3); it is multipoint if it involves several nodes, linear or not, homogeneous if the right side is zero. Linear MFCs are collected as $\mathbf A\mathbf u = \mathbf g$ (8.10). Inequality constraints arise mainly in contact (Remark 8.1; [[contact-mechanics]]) ([[ref-felippa-ifem]]).

**Master-slave elimination.** Choose a slave freedom per constraint and express all freedoms through the retained ones, $\mathbf u = \mathbf T\hat{\mathbf u} + \mathbf g$; a congruent transformation gives $$\hat{\mathbf K}\hat{\mathbf u} = \hat{\mathbf f}, \qquad \hat{\mathbf K} = \mathbf T^T\mathbf K\,\mathbf T, \qquad \hat{\mathbf f} = \mathbf T^T(\mathbf f - \mathbf K\mathbf g)$$ (eqs. 8.17, 8.27). Exact and smaller, but the equations must be rearranged and the slaves chosen with care (sec. 8.5). The same transformation is a kinematic model reduction (sec. 8.4) ([[ref-felippa-ifem]]).

**Penalty augmentation.** Each constraint becomes a fictitious stiff element of weight $w$: $$(\mathbf K + \mathbf A^T\mathbf W\mathbf A)\,\mathbf u = \mathbf f + \mathbf W\mathbf A^T\mathbf b$$ (eq. 9.14). Easy to program (assembled like elements), keeps positive definiteness, tolerant of repeated constraints; but the constraint is only approximate and a large $w$ ruins the conditioning. The "square root rule" picks $w \approx 10^{k + p/2}$ for stiffness magnitudes $10^k$ and $p$ significant digits, leaving errors of order $10^{-p/2}$, about $10^{-8}$ in double precision; the worse conditioning also hurts eigenvalue extraction and iterative solvers (secs. 9.2.3, 9.2.6) ([[ref-felippa-ifem]]).

**Lagrange multipliers.** Adjoin one unknown per constraint. Making $L = \tfrac12\mathbf u^T\mathbf K\mathbf u - \mathbf u^T\mathbf f + \boldsymbol\lambda^T(\mathbf A\mathbf u - \mathbf b)$ stationary (9.21) gives the bordered system $$\begin{bmatrix}\mathbf K & \mathbf A^T\\ \mathbf A & \mathbf 0\end{bmatrix}\begin{bmatrix}\mathbf u\\ \boldsymbol\lambda\end{bmatrix} = \begin{bmatrix}\mathbf f\\ \mathbf b\end{bmatrix}$$ (9.22); the constraint forces are $-\mathbf A^T\boldsymbol\lambda$. Exact, no weights to choose, but more unknowns and an indefinite matrix, which some solvers handle badly (sec. 9.3.4). An augmented Lagrangian combines the two (sec. 9.4, not read) ([[ref-felippa-ifem]]).

## Builds on

- [[finite-element-method-basics]]: the assembled equations $\mathbf K\mathbf u = \mathbf f$ that the constraints modify.

## Leads to

- [[mpc-transformation]] (rcfd's elimination route), [[cyclic-symmetry]], [[substructuring-and-craig-bampton]], [[contact-mechanics]]

## How rcfd uses it

- Linear MPCs, rigid sets with a reference point and elastic supports by the transformation $x = T q + g$ in `rcfd_fem::mpc` (F-393, ADR 0296): the master-slave route; the ADRs call it "the transformation path" ([[mpc-transformation]]; [[adr-thread-structural-modal-buckling-cyclic]]).
- The $k = 0$ rows of [[cyclic-symmetry]], $u_H = R u_L$, are rows of that transformation (ADR 0386) ([[adr-thread-structural-modal-buckling-cyclic]]).
- Tied non-matching meshes and beam/shell joints to solids (F-426, F-543) ([[adr-thread-structural-beams-shells]]); frictionless contact by an augmented Lagrangian with Uzawa updates (F-427) ([[adr-thread-structural-contact]]).
