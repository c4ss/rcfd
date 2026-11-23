---
type: source
tags: [reference, foundation, structural, fem, nonlinear, non-commercial]
sources: []
origin: https://ocw.mit.edu/courses/2-094-finite-element-analysis-of-solids-and-fluids-ii-spring-2011/pages/lecture-notes/
licence: CC BY-NC-SA 4.0 (MIT OCW course page footer link, verified 2026-11-22). Non-commercial, so cite only (maintainer decision 6).
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Bathe, Finite Element Analysis of Solids and Fluids II, lecture notes (MIT OCW 2.094, Spring 2011)

**Origin:** K.-J. Bathe, MIT 2.094, Spring 2011, MIT OpenCourseWare (lecture notes page above; one PDF per lecture).
**Kind:** open course notes (PDF). Locators are lecture numbers and the notes' equation numbers (e.g. 7.11). The notes refer to sections of *Finite Element Procedures*, which was not read.
**Licence:** CC BY-NC-SA 4.0; cite only.
**Read:** 2026-11-22 by job p6-structural: lectures 7, 9, 11, 12, 13, 19, 20, 21. Downloaded but not read: 8, 14, 18.

## Summary

Isoparametric elements and Gauss integration, convergence of the displacement method, the mixed u/p formulation for (nearly) incompressible material and the inf-sup condition, finite-strain kinematics and the total Lagrangian formulation with its linear and nonlinear (geometric) stiffness, and structural elements: Timoshenko beams, Reissner-Mindlin plates and MITC shells, with shear locking and its cure by assumed strains.

## Key points

- **Lecture 7, isoparametric elements:** natural coordinates $(r, s)$ with a unique correspondence to $(x, y)$; geometry and displacement use the same $h_i$; the Jacobian $\mathbf J$ must be non-singular; $\mathbf K = \int\int\mathbf B^T\mathbf C\mathbf B\,t\det\mathbf J\,dr\,ds$ (7.11); Gauss integration $\mathbf K \approx t\sum_{i,j}\mathbf B_{ij}^T\mathbf C\mathbf B_{ij}\det\mathbf J_{ij}\,w_{ij}$ (7.13); 2x2 Gauss for the 4-node element (7.14), 3x3 for the 9-node element (7.16); for rectangular elements $\mathbf J$ is constant and 3x3 gives the exact $\mathbf K$; a distorted element is still accurate enough with high enough integration. Convergence: find $u_h \in V_h$ with $a(u_h, v_h) = (f, v_h)$ for all $v_h$; the error $e_h = u - u_h$ satisfies $a(e_h, v_h) = 0$ (7.31), and $a(u_h, u_h) \le a(u, u)$: the strain energy of the FE solution is never larger than the exact one.
- **Lecture 9, u/p formulation:** as $\nu \to 0.5$ the volumetric strain $\epsilon_V \to 0$ but the pressure $p = -\kappa\epsilon_V$ must still be computed accurately; stresses $\tau_{ij} = -p\delta_{ij} + 2G\epsilon'_{ij}$; a separate pressure interpolation with the constraint $\int\delta p\,(p/\kappa + \epsilon_V)\,dV = 0$ (9.16); element-wise discontinuous pressure is condensed statically, $(\mathbf K_{uu} - \mathbf K_{up}\mathbf K_{pp}^{-1}\mathbf K_{pu})\hat{\mathbf u} = \mathbf R$ (9.25), possible unless $\nu = 0.5$ exactly (in practice $\nu = 0.499999$ works); the 9/3 element (9 displacement nodes, 3 pressure unknowns, $p = p_0 + p_1 x + p_2 y$, 9.26) is called the best element; the inf-sup condition (9.27) must hold with a constant bounded away from zero as $h \to 0$; violations show as spurious pressures or as locking with large pressures when the bulk modulus increases.
- **Lecture 11, deformation and strain:** deformation gradient $d\,{}^t\mathbf x = {}^t_0\mathbf X\,d\,{}^0\mathbf x$ (11.5), usually called $\mathbf F$; right Cauchy-Green tensor $\mathbf C = \mathbf X^T\mathbf X$ (11.6); stretch of a fibre with direction $\mathbf n$: $\lambda = (\mathbf n^T\mathbf C\mathbf n)^{1/2}$ (11.14); Green-Lagrange strain $\boldsymbol\epsilon = \tfrac12(\mathbf X^T\mathbf X - \mathbf I) = \tfrac12(\mathbf C - \mathbf I)$, components $\tfrac12(u_{i,j} + u_{j,i} + u_{k,i}u_{k,j})$; polar decomposition $\mathbf X = \mathbf R\mathbf U$ (11.29) with $\mathbf R$ a rotation and $\mathbf U$ a symmetric stretch; the Green-Lagrange components are independent of a rigid-body rotation.
- **Lecture 12, total Lagrangian formulation:** second Piola-Kirchhoff stress ${}^t_0\mathbf S = \frac{{}^0\rho}{{}^t\rho}\,{}^t_0\mathbf X^{-1}\,{}^t\boldsymbol\tau\,{}^t_0\mathbf X^{-T}$ (12.11), its components also independent of a rigid-body rotation; virtual work on the original configuration $\int_{{}^0V} S_{ij}\,\delta\epsilon_{ij}\,d\,{}^0V = {}^t\mathcal R$; incremental decomposition $S = {}^tS + \Delta S$, $\epsilon = {}^t\epsilon + \Delta\epsilon$, $\Delta\epsilon = e + \eta$ with the linear increment $e_{ij}$ (12.21, including the initial-displacement effect) and the nonlinear increment $\eta_{ij} = \tfrac12 u_{k,i}u_{k,j}$ (12.25); linearising gives $({}^t_0\mathbf K_L + {}^t_0\mathbf K_{NL})\Delta\mathbf U = {}^{t+\Delta t}\mathbf R - {}^t_0\mathbf F$, and full Newton-Raphson updates both at every iteration (12.30).
- **Lecture 13:** $\mathbf K_L = \int\mathbf B_L^T\mathbf C\mathbf B_L\,d\,{}^0V$ (13.10), $\mathbf K_{NL} = \int\mathbf B_{NL}^T\,{}^t\mathbf S\,\mathbf B_{NL}\,d\,{}^0V$ (13.13), $\mathbf F = \int\mathbf B_L^T\hat{\mathbf S}\,d\,{}^0V$; for a truss bar along $x$ under axial stress, $\mathbf K_L$ has no stiffness for the transverse displacement but $\mathbf K_{NL}$, proportional to the current stress, does.
- **Lecture 19, slender structures:** a plane 4-node element in pure bending develops a spurious shear strain (19.18); Timoshenko (Reissner-Mindlin) kinematics: a fibre normal to the mid-line moves and rotates by $\beta$ without changing length; virtual work $EI\int\beta'\,\delta\beta'\,dx + (Ak)G\int(w' - \beta)(\delta w' - \delta\beta)\,dx = \int p\,\delta w\,dx$ (19.25, 20.1).
- **Lecture 20:** for the 2-node displacement-based Timoshenko element the transverse shear strain cannot vanish along the whole element, so about 200 elements are needed for 10 percent error; an assumed constant shear strain equal to the displacement-based value at the element middle (20.26) gives "a powerful element"; the analogy with incompressibility: interpolations must be chosen to satisfy an inf-sup condition (20.18).
- **Lecture 21, plates and shells:** Reissner-Mindlin plate: $w(x, y)$ and the rotations $\beta_x, \beta_y$ are independent; $u = -z\beta_x$, $v = -z\beta_y$ (21.8); transverse shear $\gamma_{xz} = \partial w/\partial x - \beta_x$ (21.11); bending plus shear stiffness with a shear correction factor $k$ (21.21); the displacement-based element locks in shear; MITC (mixed interpolation of tensorial components) interpolates the transverse shear strains from edge values (21.22)-(21.24) and works. Kirchhoff theory sets the transverse shear to zero, $\beta_x = \partial w/\partial x$, which needs continuity of slopes and makes good elements hard to build. Flat shell elements superpose plate bending and plane-stress stiffness; curved shells need curved elements (references: Dvorkin and Bathe 1984; Bathe and Dvorkin; Bathe, Iosilevich and Chapelle 2000; Chapelle and Bathe, *The Finite Element Analysis of Shells*).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[isoparametric-elements-and-quadrature]], [[element-locking]], [[finite-strain-kinematics]], [[nonlinear-fem-newton]], [[linear-buckling-and-stress-stiffening]], [[beam-theory]], [[plate-and-shell-theory]], [[principle-of-virtual-work]]
