---
type: source
tags: [reference, foundation, fsi, coupling, quasi-newton, aitken]
sources: []
origin: https://arxiv.org/abs/2001.07947
licence: arXiv non-exclusive licence to distribute (licenses/nonexclusive-distrib/1.0 on the abstract page, read 2026-11-22). Not an open licence, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Spenke, Hosters and Behr (2020), A multi-vector interface quasi-Newton method with linear complexity for partitioned FSI

**Origin:** T. Spenke, N. Hosters and M. Behr, "A Multi-Vector Interface Quasi-Newton Method with Linear Complexity for Partitioned Fluid-Structure Interaction", arXiv:2001.07947v1 [cs.CE], 22 January 2020 (preprint submitted to *Computer Methods in Applied Mechanics and Engineering*). PDF: https://arxiv.org/pdf/2001.07947.
**Kind:** research paper (preprint).
**Licence:** arXiv non-exclusive distribution licence; cite only.
**Read:** 2026-11-22 by job p6-linear-rotating: secs. 1-4.2 in full, sec. 5 and the conclusion skimmed.

## Summary

Partitioned FSI with a Dirichlet-Neumann scheme is a fixed-point iteration on the interface deformation. The paper reviews constant and Aitken relaxation and the interface quasi-Newton family (IQN-ILS, IQN-IMVJ) and adds a multi-vector variant whose cost is linear in the number of interface unknowns.

## Key points

- **Sec. 1:** partitioned schemes couple two black-box solvers by exchanging interface data; their "major downside" is an instability from the added-mass effect; the simplest remedy is relaxation, constant or dynamic by Aitken's method, often at a high cost in coupling iterations; interface quasi-Newton methods both stabilise and accelerate; IQN-ILS (Degroote et al.) is the reference variant.
- **Sec. 2.3, coupling conditions:** kinematic (continuity of displacement, velocity, acceleration across the interface) and dynamic (equality of stresses, Newton's third law).
- **Sec. 3.3, coupling approach:** non-matching interface meshes need a conservative projection between the solvers. A consistent solution needs iteration within the step, "strong (temporal) coupling". Dirichlet-Neumann scheme: the fluid solver takes the interface deformation, its stresses go to the structural solver as a Neumann condition, the new deformation goes back as a Dirichlet condition. Mathematically a fixed-point iteration $x_d = H(x_d)$ with $H = S \circ F$, residual $R(x_d) = H(x_d) - x_d$ (eqs. 12-14).
- **Sec. 3.3, added mass:** the forces are computed from a defective deformation during the iteration; through kinematic continuity an overestimated deformation gives exaggerated fluid acceleration and inertia, which acts like extra mass on the structure and can grow over the iterations; high density ratios $\rho_f / \rho_s$ are a major factor, with viscous and time-discretisation dependences too (refs. Foerster; Causin et al.).
- **Sec. 3.3, relaxation, eqs. (15)-(16):** constant under-relaxation $x^{(k+1)} = \omega \tilde{x}^{(k)} + (1 - \omega)\, x^{(k)}$ with $\omega < 1$, small enough for every step and therefore expensive; Aitken's dynamic relaxation adapts $\omega$ each coupling iteration from the two most recent residuals, often giving "significant speed-ups".
- **Sec. 4.1, IQN-ILS, eqs. (17)-(24):** a Newton update of the interface needs the inverse Jacobian of the residual, which is not accessible; it is approximated from input-output pairs collected during the step's iterations (differences of residuals $V_k$ and of deformations $W_k$), a multi-dimensional secant; the underdetermined system is closed by a minimum-norm (least-squares) condition and solved by QR; the first iteration of a step is a relaxation step.
- **Sec. 4.2:** reusing pairs from earlier time steps helps but risks rank deficiency and contradictory information; filtering is the remedy used in the literature.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[fixed-point-iteration-and-acceleration]], [[partitioned-fluid-structure-interaction]], [[added-mass]]
