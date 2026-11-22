---
type: concept
tags: [foundation, structural, contact, nonlinear]
sources: [ref-kim-ufl-contact-notes, ref-bathe-mit-2-092-fea-notes, ref-felippa-ifem, adr-thread-structural-contact]
builds_on: [multi-point-constraints, nonlinear-fem-newton]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Contact mechanics in finite elements

## In short

Two bodies may touch, press on each other and separate, but never interpenetrate. Contact is a boundary nonlinearity: where the bodies touch and how hard they press are both unknown. It is imposed as an inequality constraint (gap closed or open, pressure only compressive) by Lagrange multipliers, penalty springs or a combination, usually with one surface as "master" and the other's nodes as "slaves" ([[ref-kim-ufl-contact-notes]]; [[ref-bathe-mit-2-092-fea-notes]], lecture 10).

## Explanation

**Why it is hard.** The contact boundary is part of the solution; the force-displacement relation turns vertical once in contact, so the contact force changes abruptly, which is difficult for Newton-Raphson; in contact the force cannot be found from the displacement (an analogy with incompressibility); and the discrete contact boundary changes node by node ([[ref-kim-ufl-contact-notes]], "Why are contact problems difficult?"). Bathe counts contact as its own analysis type, changing boundary conditions ([[ref-bathe-mit-2-092-fea-notes]], lecture 10).

**Conditions.** For a cantilever tip above a rigid block with gap $G$: $$g = v_{tip} - G \le 0, \qquad \lambda \ge 0, \qquad \lambda\,g = 0$$ no penetration, compressive force only, and force only where the gap is closed ([[ref-kim-ufl-contact-notes]], cantilever example). These are inequality constraints, the case Felippa sets aside from multifreedom constraints (Remark 8.1) ([[ref-felippa-ifem]]).

**Enforcement.**

- *Lagrange multiplier:* the multiplier is the contact force, enforced exactly (in the example $\lambda = 75$ N) ([[ref-kim-ufl-contact-notes]]); the bordered, indefinite system of [[multi-point-constraints]] applies to the active set ([[ref-felippa-ifem]], ch. 9).
- *Penalty:* $\lambda = K_N\langle g\rangle$, force proportional to penetration; a small penetration is allowed and decreases as the penalty grows, at the cost of conditioning ([[ref-kim-ufl-contact-notes]]; [[ref-felippa-ifem]], sec. 9.2).
- *Augmented Lagrangian:* a combination of both (Felippa, sec. 9.4, not read in this job).

**General geometry.** A point $\mathbf x$ of the slave body is projected to the closest point $\mathbf x_c(\xi_c)$ of the master surface, and the normal gap $$g_n = \big(\mathbf x - \mathbf x_c(\xi_c)\big)^T\mathbf e_n(\xi_c) \ge 0$$ must stay non-negative (impenetrability) on the possible contact boundary. Slave nodes cannot penetrate master surface elements ([[ref-kim-ufl-contact-notes]], "General Contact Formulation", "Slave-master concept"). The sign convention of the gap differs from the cantilever example.

**Friction.** Coulomb friction adds stick ($|t| < \mu\lambda$, no slip) and slip ($|t| = \mu\lambda$) conditions; the result depends on the load sequence ([[ref-kim-ufl-contact-notes]]).

## Builds on

- [[multi-point-constraints]]: penalty and Lagrange enforcement of constraints.
- [[nonlinear-fem-newton]]: contact is solved inside the incremental Newton loop.

## Leads to

- [[structural-solver]] (interference fits, punches), [[adr-thread-structural-contact]]

## How rcfd uses it

- Frictionless node-to-surface contact on bilinear Hex8 target faces or a rigid plane: closest-point projection with the exact gap gradient, a per-node augmented Lagrangian force with Uzawa updates, a linear-body Newton driver and a total Lagrangian body (F-427, ADR 0507, partial: the interference-fit shaft stress missed, `7.0e-3` against `1e-6`) ([[adr-thread-structural-contact]]).
