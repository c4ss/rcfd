---
type: source
tags: [reference, foundation, structural, fem, cite-only]
sources: []
origin: https://www.colorado.edu/engineering/CAS/courses.d/IFEM.d/Home.html
licence: none stated on the course page or in the chapters read. Unknown, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Felippa, Introduction to Finite Element Methods (IFEM, ASEN 5007 lecture notes, University of Colorado Boulder)

**Origin:** C. A. Felippa, *Introduction to Finite Element Methods*, lecture notes for ASEN 5007, Department of Aerospace Engineering Sciences, University of Colorado at Boulder (course site last updated 2017-12-13). The author's site is no longer served at the URL above; the chapters were read from the Internet Archive copies of that site (`web.archive.org/web/2017id_/https://www.colorado.edu/engineering/CAS/courses.d/IFEM.d/IFEM.ChNN.d/IFEM.ChNN.pdf`).
**Kind:** open lecture notes (PDF chapters). Locators are chapter, section and equation numbers.
**Licence:** none stated; cite only.
**Read:** 2026-11-22 by job p6-structural: chapters 8 (MultiFreedom Constraints I), 9 (MultiFreedom Constraints II), 10 (Superelements and Global-Local Analysis, static condensation sections), 17 (Isoparametric Quadrilaterals), 28 (Stress Recovery).

## Summary

The direct stiffness method and its extensions: imposing multifreedom constraints by master-slave elimination, penalty elements or Lagrange multipliers; superelements and static condensation; the isoparametric quadrilateral with Gauss quadrature, its shear locking and the modified-integration cures; and the recovery of stresses from computed displacements.

## Key points

- **Ch. 8, sec. 8.1:** a multifreedom constraint (MFC) is a functional equation linking two or more displacement components, canonical form "F(nodal displacement components) = prescribed value" (8.3); multipoint if several nodes are involved, linear or nonlinear, homogeneous if the right side is zero. Inequality constraints arise mainly with contact (Remark 8.1). Three methods: master-slave elimination, penalty augmentation, Lagrange multiplier adjunction (sec. 8.1.2); all MFCs collected as $\mathbf A\mathbf u = \mathbf g$ (8.10).
- **Ch. 8, sec. 8.3, master-slave:** pick a slave freedom per constraint, write $\mathbf u = \mathbf T\hat{\mathbf u}$ and apply the congruent transformation $\hat{\mathbf K} = \mathbf T^T\mathbf K\mathbf T$, $\hat{\mathbf f} = \mathbf T^T\mathbf f$ (8.17); non-homogeneous constraints $\mathbf u = \mathbf T\hat{\mathbf u} + \mathbf g$ give $\hat{\mathbf f} = \mathbf T^T(\mathbf f - \mathbf K\mathbf g)$ (8.27). Drawback in computer work: the equations must be rearranged because $\hat{\mathbf u}$ is a subset of $\mathbf u$ (sec. 8.5). The same congruent transformation is a model reduction by kinematic constraints (sec. 8.4, eq. 8.35).
- **Ch. 9, sec. 9.2, penalty:** each MFC becomes a fictitious penalty element of weight $w$; for constraints $\mathbf A\mathbf u = \mathbf b$, $(\mathbf K + \mathbf A^T\mathbf W\mathbf A)\mathbf u = \mathbf f + \mathbf W\mathbf A^T\mathbf b$ (9.14). Large $w$ makes the matrix ill-conditioned; the "square root rule": with stiffness coefficients of order $10^k$ and $p$ digits of precision, take $w \approx 10^{k + p/2}$, which leaves a constraint violation and solution error of order $10^{-p/2}$ (about $10^{-8}$ in double precision) (sec. 9.2.3). Advantages: no rearrangement, assembly as ordinary elements, insensitive to linearly dependent constraints, positive definiteness kept; disadvantages: the choice of weight, and a worse condition number that hurts eigenvalue extraction and iterative solvers (sec. 9.2.6).
- **Ch. 9, sec. 9.3, Lagrange multipliers:** replace the rigid link by an unknown constraint-force pair; the Lagrangian $L = \tfrac12\mathbf u^T\mathbf K\mathbf u - \mathbf u^T\mathbf f + \boldsymbol\lambda^T(\mathbf A\mathbf u - \mathbf b)$ (9.21) gives the bordered system $\begin{bmatrix}\mathbf K & \mathbf A^T\\ \mathbf A & \mathbf 0\end{bmatrix}\begin{bmatrix}\mathbf u\\ \boldsymbol\lambda\end{bmatrix} = \begin{bmatrix}\mathbf f\\ \mathbf b\end{bmatrix}$ (9.22); the constraint forces are $-\mathbf A^T\boldsymbol\lambda$. Exact and needing no weights, but it adds unknowns and makes the matrix indefinite, which "may cause grief" with some solvers (sec. 9.3.4). An augmented Lagrangian method is sec. 9.4 (not read).
- **Ch. 10, superelements and static condensation:** a superelement is a group of elements treated as one; its internal freedoms are not connected to other superelements, its boundary freedoms are. Partitioning, $\mathbf u_i = \mathbf K_{ii}^{-1}(\mathbf f_i - \mathbf K_{ib}\mathbf u_b)$ (10.3), and the condensed equations $\tilde{\mathbf K}_{bb} = \mathbf K_{bb} - \mathbf K_{bi}\mathbf K_{ii}^{-1}\mathbf K_{ib}$, $\tilde{\mathbf f}_b = \mathbf f_b - \mathbf K_{bi}\mathbf K_{ii}^{-1}\mathbf f_i$ (10.5); this needs $\mathbf K_{ii}$ non-singular: the superelement must be rank-sufficient (only rigid-body zero-energy modes) and fixing the boundary must suppress all rigid motions, else it is "floating" (Remark 10.1). Internal freedoms are recovered from (10.3) after the global solve (sec. 10.2.3).
- **Ch. 17, isoparametric quadrilaterals:** shape functions in natural coordinates $(\xi, \eta)$; Jacobian $\mathbf J$ of $(x, y)$ with respect to $(\xi, \eta)$ computed from nodal coordinates (17.7)-(17.8), inverted numerically to get $x$- and $y$-derivatives (17.5); strain-displacement matrix (17.9). Gauss rules on $[-1, 1]$ with $p$ points integrate polynomials up to order $2p - 1$ exactly (sec. 17.3.1, Table 17.1); 2D product rules $\int\int F\,d\xi\,d\eta \approx \sum_i\sum_j w_i w_j F(\xi_i, \eta_j)$ (17.15); useful rules must be fully symmetric, have positive weights and interior points (sec. 17.3.3). Quad4 needs at least 2x2 Gauss for rank sufficiency; 1x1 leaves it rank deficient by two (sec. 17.5.1).
- **Ch. 17, secs. 17.6-17.7, shear locking:** low-order plane elements in in-plane bending absorb "parasitic shear energy" and are overstiff, worse at large aspect ratio; the bending energy ratio $r = U^e_{FEM}/U^e_{exact}$ (sec. 17.6.1) shows locking when $r \gg 1$; for $a = 10b$ and $\nu = 0$, $r \approx 50$, so the Quad4 gives about 2 percent of the correct deflection (sec. 17.6.2); even infinitely many elements along the beam keep $r \to 1/(1-\nu^2) > 1$ with one layer through the height. Cures listed historically: hybrid stress models (1964), Hellinger-Reissner mixed models (1965), modified Gauss integration (1971), incompatible displacement modes (1973), assumed strains (1978); for rectangles all give the same pure-bending-exact Quad4 (sec. 17.6.3). Definitions: reduced integration (rank-deficient rule), weighted integration $\mathbf K^e = (1-\gamma)\mathbf K^e_{1\times1} + \gamma\mathbf K^e_{2\times2}$ (17.38), selective integration (split $\mathbf E$ into parts integrated by different rules, 17.41), selective reduced integration (one of them rank-deficient alone); the same tools treat "pressure locking" of 3D elements with near-incompressible material (sec. 17.7).
- **Ch. 28, stress recovery:** stresses are derived quantities, less accurate than displacements ("if the accuracy level of displacements is 1% that of the stresses may be typically 10% to 20%") (sec. 28.1); element strains from $\mathbf e = \mathbf B\mathbf u^{(e)}$ and stresses from the constitutive law (sec. 28.2); stresses at a node differ between adjacent elements because displacement-assumed elements do not make stresses continuous, so they are averaged ("nodal point stresses", sec. 28.5); evaluating at the Gauss points and extrapolating to the nodes (bilinear extrapolation through a "Gauss element") usually gives better values for distorted quadrilaterals, backed by superconvergence results; no difference for rectangles (secs. 28.2-28.4).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[multi-point-constraints]], [[substructuring-and-craig-bampton]], [[isoparametric-elements-and-quadrature]], [[element-locking]], [[stress-recovery]], [[contact-mechanics]]
