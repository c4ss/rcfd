---
type: concept
tags: [foundation, turbulence, rans]
sources: [ref-davidson-turbulence-notes, ref-mit-ocw-2-29-turbulence, ref-nasa-tmr, ref-menter-1992-nasa-tm-103975]
builds_on: [reynolds-averaging, stress]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reynolds stress

## In short

The Reynolds stress $-\rho\, \overline{u_i' u_j'}$ is the extra stress that the averaged-out turbulent fluctuations exert on the mean flow: the mean rate at which fluctuating motion carries momentum across a surface. It is a symmetric tensor with six components; half its trace (per unit mass) is the turbulent kinetic energy $k$ ([[ref-davidson-turbulence-notes]], [[ref-mit-ocw-2-29-turbulence]]).

## Explanation

**Definition.** Averaging the momentum equation leaves the term $-\rho\, \overline{u_i' u_j'}$ next to the viscous stress ([[reynolds-averaging]]). The total stress acting on the mean flow is therefore ([[ref-mit-ocw-2-29-turbulence]] L25 s.13):

$$\sigma_{ij,\text{total}} = -\bar{p}\, \delta_{ij} + \tau_{ij,\text{viscous}} - \rho\, \overline{u_i' u_j'}$$

with the matrix of fluctuation correlations

$$\overline{u_i' u_j'} = \begin{bmatrix} \overline{u'^2} & \overline{u'v'} & \overline{u'w'} \ \overline{u'v'} & \overline{v'^2} & \overline{v'w'} \ \overline{u'w'} & \overline{v'w'} & \overline{w'^2} \end{bmatrix}.$$

It is symmetric ($\overline{u'v'} = \overline{v'u'}$), so it has six independent components ([[ref-davidson-turbulence-notes]] p. 88). Sign and density conventions vary: the TMR writes $\tau_{ij} = -\overline{\rho u_i'' u_j''}$, and notes that the literature also uses the opposite sign and leaves out the density ([[ref-nasa-tmr]] `implementrans.html`).

**Normal and shear parts.** As for any [[stress]], the diagonal entries are normal stresses and the off-diagonal entries are shear stresses. Half the trace is the turbulent kinetic energy, $k = \tfrac{1}{2} \overline{u_i' u_i'}$ ([[ref-davidson-turbulence-notes]] eq. 5.11; [[ref-nasa-tmr]] `implementrans.html`). In isotropic turbulence the off-diagonal terms are zero and the three normal stresses are equal ([[ref-mit-ocw-2-29-turbulence]] L25 s.13; [[ref-davidson-turbulence-notes]] p. 82).

**Why the shear part is not zero in a shear flow.** In a mean flow $u(y)$ with $du/dy > 0$, a parcel moving up ($v' > 0$) comes from slower fluid and slows its new neighbours ($u' < 0$), and the reverse for $v' < 0$; so $\overline{u'v'} < 0$ and $-\rho\, \overline{u'v'} > 0$: the fluctuations transport momentum down the gradient, like an enhanced viscosity ([[ref-mit-ocw-2-29-turbulence]] L25 s.13). In a boundary layer the total shear stress is $\tau_{12} = \mu\, dU/dy - \rho\, \overline{u'v'}$ ([[ref-davidson-turbulence-notes]] eq. 6.15); the Reynolds part is zero at the wall and outside the thin viscous sublayer it is much larger than the viscous part ([[ref-davidson-turbulence-notes]] p. 91, [[law-of-the-wall]]). The principal turbulent shear stress $\tau = -\overline{u'v'}$ of a boundary layer is the quantity SST's limiter is built around ([[ref-menter-1992-nasa-tm-103975]] eq. 14).

**What it adds to its prerequisites.** [[reynolds-averaging]] shows that a new unknown appears; this page says what it is: a genuine stress tensor, with the same normal/shear structure as the molecular [[stress]], but caused by momentum transport by eddies rather than by molecular friction, and set by the flow, not by the fluid. It brings the turbulent kinetic energy $k$ into the picture as its trace. Models either relate it to the mean strain ([[boussinesq-eddy-viscosity]]) or transport each component ([[reynolds-stress-models]]).

## Builds on

- [[reynolds-averaging]]: where the term comes from.
- [[stress]]: the tensor, its normal and shear components and its symmetry.

## Leads to

- [[boussinesq-eddy-viscosity]], [[reynolds-stress-models]], [[bradshaw-assumption-and-sst-limiter]]
- rcfd models: [[k-omega-sst]], [[k-epsilon]]

## How rcfd uses it

- The Reynolds stress model `rsm` (SSG and LRR-IP variants, ADR 0032) transports the stresses; it is transient only and runs with no wall patch ([[doc-case-schema-turbulence]], [[adr-thread-early-turbulence]]).
- The eddy-viscosity models (k-epsilon, SST, SA) model the stresses through $\nu_t$ ([[rcfd-turb]]).
