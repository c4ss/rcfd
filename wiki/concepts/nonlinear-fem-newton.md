---
type: concept
tags: [foundation, structural, nonlinear, fem]
sources: [ref-bathe-mit-2-092-fea-notes, ref-bathe-mit-2-094-fea-notes, ref-felippa-nfem, adr-thread-structural-materials-nonlinear]
builds_on: [finite-strain-kinematics, newton-raphson-method, finite-element-method-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Nonlinear finite element analysis with Newton iterations

## In short

When the material yields, the geometry changes a lot or contact opens and closes, internal forces are no longer proportional to displacements. The load is then applied in increments, and in each increment Newton-Raphson iterations correct the displacement until the internal forces balance the applied loads: solve with the tangent stiffness, update, re-evaluate, repeat ([[ref-bathe-mit-2-092-fea-notes]], lecture 10).

## Explanation

**Kinds of nonlinearity.** Bathe lists five analysis types: I linear (strains below about 0.04, constant modulus); II materially nonlinear only; III large displacements, small strains (e.g. shell buckling); IV large displacements and large strains; V contact, i.e. changing boundary conditions ([[ref-bathe-mit-2-092-fea-notes]], lecture 10; see [[contact-mechanics]]).

**Incremental equilibrium.** Seek ${}^{t+\Delta t}\mathbf F = {}^{t+\Delta t}\mathbf R$: the internal nodal forces equal the external loads at the end of the step (lecture 10, eq. 1). Newton-Raphson iterates $$ {}^{t+\Delta t}\mathbf K^{(i-1)}\,\Delta\mathbf U^{(i)} = {}^{t+\Delta t}\mathbf R - {}^{t+\Delta t}\mathbf F^{(i-1)}, \qquad {}^{t+\Delta t}\mathbf U^{(i)} = {}^{t+\Delta t}\mathbf U^{(i-1)} + \Delta\mathbf U^{(i)}$$ (eqs. A, B), starting from the converged state at $t$ (C). Iterations stop when the displacement correction or the out-of-balance load is small; a too large step may not converge ([[ref-bathe-mit-2-092-fea-notes]]). The scalar algorithm is on [[newton-raphson-method]].

**Total Lagrangian form.** Virtual work is written on the original configuration with the second Piola-Kirchhoff stress and Green-Lagrange strain, $\int_{{}^0V} S_{ij}\,\delta\epsilon_{ij}\,d\,{}^0V = {}^t\mathcal R$; linearising gives $$({}^t_0\mathbf K_L + {}^t_0\mathbf K_{NL})\,\Delta\mathbf U = {}^{t+\Delta t}\mathbf R - {}^t_0\mathbf F$$ with material stiffness $\mathbf K_L = \int\mathbf B_L^T\mathbf C\,\mathbf B_L\,d\,{}^0V$ (13.10), geometric stiffness $\mathbf K_{NL}$ (13.13) and internal force $\mathbf F = \int\mathbf B_L^T\hat{\mathbf S}\,d\,{}^0V$; full Newton updates both every iteration (12.30) ([[ref-bathe-mit-2-094-fea-notes]], lectures 12-13; [[finite-strain-kinematics]]).

**Cost against reliability.** The conventional Newton method forms and factors the tangent every iteration (costly) and converges only from a close start, possibly to an unwanted branch near bifurcations. Variants: relaxed or damped Newton for reliability; modified Newton (keep an old tangent, more but cheaper iterations) and quasi-Newton for efficiency; "no modification can be said to be uniformly superior" (Felippa, NFEM sec. 23.1). Modified Newton still needs the internal forces computed correctly at every iteration (Bathe, lecture 10) ([[ref-felippa-nfem]]; [[ref-bathe-mit-2-092-fea-notes]]).

**Increment control.** Load control fixes the load parameter in each step and is the simplest; when the load factor itself is an unknown the iteration adds a scalar constraint $c(\mathbf u, \lambda) = 0$ (sec. 23.2.2). Passing limit points needs such a constraint ([[ref-felippa-nfem]]; [[stability-of-equilibrium]], [[arc-length-continuation]]).

## Builds on

- [[finite-strain-kinematics]]: strain and stress measures and their increments.
- [[newton-raphson-method]]: the iteration and its quadratic convergence.
- [[finite-element-method-basics]]: assembly of internal forces and stiffness.

## Leads to

- [[stability-of-equilibrium]], [[arc-length-continuation]], [[plasticity-j2]] (the consistent tangent), [[contact-mechanics]], [[hyperelastic-materials]]

## How rcfd uses it

- Newton driver: total Lagrangian, tangent `K_mat(F) + K_sigma(S)`, step halving (F-397, ADR 0404, partial) ([[adr-thread-structural-materials-nonlinear]]).
- Newton inside Newmark and generalised-alpha for nonlinear transients (F-423, ADR 0479) ([[adr-thread-structural-materials-nonlinear]]; [[newmark-and-generalised-alpha]]).
- J2 plasticity in the driver measured Newton orders 1.27 to 1.44 against 1.8 (F-422, partial) ([[adr-thread-structural-materials-nonlinear]]; [[plasticity-j2]]).
