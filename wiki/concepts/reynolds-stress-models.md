---
type: concept
tags: [foundation, turbulence, rans]
sources: [ref-nasa-tmr, ref-mit-ocw-2-29-turbulence]
builds_on: [reynolds-stress, two-equation-turbulence-models]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reynolds-stress models (second-moment closures)

## In short

A Reynolds-stress model (RSM) does not use an eddy viscosity. It solves a transport equation for each of the six independent Reynolds stresses, plus one equation for a length-scale variable (`epsilon` or `omega`): seven extra equations. Production is then exact; the pressure-strain, dissipation and turbulent-diffusion terms still need models ([[ref-nasa-tmr]], [[ref-mit-ocw-2-29-turbulence]]).

## Explanation

**Why.** Eddy-viscosity models, including k-epsilon, assume the turbulence is isotropic, "which fails in many flows" ([[ref-mit-ocw-2-29-turbulence]] L25 s.25). Simpler models give the stresses "in terms of other tensors via some assumed relation (such as Boussinesq's hypothesis)", whereas full second-moment models "compute each of the 6 Reynolds stresses directly" ([[ref-nasa-tmr]] `rsm-ssglrr.html`).

**The equations** (SSG/LRR-omega, TMR notation `R_ij = mean(u_i'' u_j'') = -tau_ij/rho`) ([[ref-nasa-tmr]] `rsm-ssglrr.html`):

`d(rho R_ij)/dt + d(rho U_k R_ij)/dx_k = rho P_ij + rho Pi_ij - rho eps_ij + rho D_ij (+ rho M_ij)`

- `P_ij = -R_ik dU_j/dx_k - R_jk dU_i/dx_k`: production, "exact", no model needed, because it contains only the stresses themselves and mean velocity gradients.
- `Pi_ij`: pressure-strain correlation, which redistributes energy between components; modelled (LRR near walls blended to SSG away from walls, using an `F1` function of the [[wall-distance]] like SST's) with the anisotropy tensor `a_ij = R_ij/k - (2/3) delta_ij`.
- `eps_ij = (2/3) eps delta_ij`: dissipation, modelled as isotropic, `eps = C_mu k omega`, `C_mu = 0.09`, `k = R_ii/2`.
- `D_ij`: diffusion, modelled by a generalized gradient-diffusion model (Daly and Harlow).
- `M_ij`: fluctuating mass flux, neglected.
- A seventh equation for `omega` closes the length scale.

The MIT notes list the same structure: production, pressure-strain redistribution, a dissipation tensor and third-order turbulent diffusion; the common closures are isotropic dissipation, several pressure-strain models and an eddy-flux model for the diffusion ([[ref-mit-ocw-2-29-turbulence]] L25 s.26).

**Cost and status.** "Extra 6 + 1 = 7 PDEs to be solved increase cost"; the MIT notes call RSMs "among the most complex RANS used today" and "Mostly used for academic research" ([[ref-mit-ocw-2-29-turbulence]] L25 s.25).

**What it adds to its prerequisites.** [[reynolds-stress]] defines the six stresses; [[two-equation-turbulence-models]] provide the length-scale equation and the modelling style (production, destruction, diffusion). RSMs drop the [[boussinesq-eddy-viscosity]] link between stress and strain and transport the stresses directly, so stress anisotropy and stress history are represented. The [[bradshaw-assumption-and-sst-limiter|SST limiter]] imitates one consequence of this (the transport of the principal shear stress) inside an eddy-viscosity model.

## Builds on

- [[reynolds-stress]]: the transported tensor.
- [[two-equation-turbulence-models]]: the length-scale equation and the source/diffusion modelling.

## Leads to

- [[bradshaw-assumption-and-sst-limiter]] (the eddy-viscosity shortcut to shear-stress transport)

## How rcfd uses it

- `turbulence.model = "rsm"` (SSG and LRR-IP, ADR 0032): transient only, with **no wall patch**; the wall-bounded channel check C7 "was **not delivered**" ([[doc-case-schema-turbulence]], [[adr-thread-early-turbulence]]).
- ADR 0032 C2(d) homogeneous-shear growth missed (`k(40)/k0` = 1.83e13 for ssg); LRR-IP constants recorded as unverified, "written from memory" ([[adr-thread-early-turbulence]]).
- F-046 (RSM and transition) is partial: "RSM walls and wall reflection, steady and energy-coupled RSM, omega-based pressure-strain and stress inlets are not delivered" ([[roadmap-turbulence]]).
