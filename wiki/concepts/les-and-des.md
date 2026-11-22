---
type: concept
tags: [foundation, turbulence, les, des]
sources: [ref-mit-ocw-2-29-turbulence, ref-davidson-turbulence-notes, ref-nasa-tmr]
builds_on: [turbulence, boussinesq-eddy-viscosity]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# LES and DES

## In short

Large-eddy simulation (LES) filters the flow in space instead of averaging it in time: the large eddies are computed, unsteady and three-dimensional, and only the small ones are modelled, usually with a subgrid eddy viscosity such as Smagorinsky's. Detached-eddy simulation (DES) is a hybrid: RANS in the attached boundary layer, LES in the separated regions away from walls, with the switch decided by the grid size. Delayed DES (DDES) shields the boundary layer from switching too early ([[ref-mit-ocw-2-29-turbulence]], [[ref-davidson-turbulence-notes]]).

## Explanation

**LES: filtering.** The large scales carry most of the energy and transport; the small ones are more universal. So "Instead of time-averaging, LES uses spatial filtering to separate large and small eddies" ([[ref-mit-ocw-2-29-turbulence]] L25 s.27). The filtered velocity is $\bar{u}(x,t) = \int G(x, x'; \Delta)\, u(x', t)\, dV'$ with a filter kernel $G$ of width $\Delta$ (box, Gaussian, spectral cut-off). Filtering the Navier-Stokes equations leaves $\overline{u_i u_j} \ne \bar{u}_i \bar{u}_j$; the difference is the subgrid-scale stress $\tau_{ij}^{SG} = \rho(\overline{u_i u_j} - \bar{u}_i \bar{u}_j)$, "the large scale momentum flux caused by the action of the small or unresolved scales" ([[ref-mit-ocw-2-29-turbulence]] L25 s.28-29). LES is 3D, unsteady and expensive, but much cheaper than DNS (L25 s.27).

**Smagorinsky model.** An eddy-viscosity model for the subgrid stress ([[ref-davidson-turbulence-notes]] eqs. 18.16-18.17): $\tau_{ij} - \tfrac{1}{3} \delta_{ij} \tau_{kk} = -2 \nu_{sgs} \bar{s}_{ij}$,  $\nu_{sgs} = (C_S \Delta)^2 \lvert \bar{s} \rvert$,  $\lvert \bar{s} \rvert = (2 \bar{s}_{ij} \bar{s}_{ij})^{1/2}$,  $\Delta = (\Delta V)^{1/3}$. It is the [[mixing-length-model]] with the length $C_S \Delta$ set by the grid (sec. 18.7). Near walls it must be damped, e.g. by $1 - e^{-y^+/26}$ or by limiting the length with $\kappa n$, $n$ the wall distance; the "constant" $C_S$ is flow dependent, from 0.065 to 0.25 (p. 192).

**DES.** In DES "the boundary layer is treated with RANS and ... the outer detached eddies with LES"; it was developed for wings at very high angles of attack ([[ref-davidson-turbulence-notes]] sec. 20, p. 215). In S-A DES the wall distance $d$ of the Spalart-Allmaras destruction term is replaced by $\tilde{d} = \min(d,\, C_{des} \Delta)$, $\Delta = \max(\Delta x, \Delta y, \Delta z)$, $C_{des} = 0.65$ (eq. 20.3). Inside the boundary layer $d < C_{des} \Delta$ and the model is RANS; outside, the shorter length increases destruction, lowers $\nu_t$ and lets the flow resolve unsteady eddies (LES mode) (p. 216). In k-omega SST DES the $k$ dissipation is multiplied by $F_{DES} = \max(L_t/(C_{DES} \Delta),\, 1)$, $L_t = \sqrt{k}/(\beta^* \omega)$, $C_{DES} = 0.61$ (eq. 20.8). The grid decides where the switch happens, "Hence it is crucial to generate an appropriate grid" (p. 218).

**DDES.** If the grid is refined along the wall so that $\Delta$ becomes smaller than the boundary-layer thickness, DES switches to LES inside the boundary layer on a mesh too coarse for LES. DDES "protects the boundary layer from the LES mode": $\tilde{d}_{DDES} = d - f_d \max(0,\, d - C_{DES} \Delta)$ with a shielding function $f_d$ (eqs. 20.9-20.10); for SST $F_{DDES} = \max(L_t/(C_{DES} \Delta)(1 - F_S),\, 1)$ with $F_S$ one of the SST blending functions (eq. 20.11) ([[ref-davidson-turbulence-notes]] pp. 218-219). The TMR notes that with SA-based DES and DDES the $f_{t2}$ term "may cause an undesirable delay in transition to turbulence in the RANS region" ([[ref-nasa-tmr]] `spalart.html`).

**What it adds to its prerequisites.** [[turbulence]] makes clear that the large eddies are flow-specific and the small ones more universal; LES models only the latter. [[boussinesq-eddy-viscosity]] provides the form of the subgrid model, now with a length tied to the grid instead of the flow. DES adds the idea of letting one model act as RANS or as a subgrid model depending on the ratio of turbulent length to cell size, using the [[wall-distance]].

## Builds on

- [[turbulence]]: the range of scales and why the small ones can be modelled.
- [[boussinesq-eddy-viscosity]]: the eddy-viscosity form of the subgrid and RANS stresses.

## Leads to

- [[spalart-allmaras]] (`sa.des`), [[k-omega-sst]] (SST-DES/DDES as a variant)

## How rcfd uses it

- `turbulence.model` offers `smagorinsky` and `wale` (ADR 0026); in that ADR the channel profile was within 3.99 % of DNS at $Re_\tau = 395$, and "In this solver WALE dissipates more than Smagorinsky, the opposite of the registered ordering" ([[doc-case-schema-turbulence]], [[adr-thread-early-turbulence]]).
- `sa.des` `off`/`des`/`ddes` with `sa.c_des` 0.65, transient only and "not validated on a separated flow"; SA-DES stays on the once-per-step path of the transient algorithms ([[doc-case-schema-turbulence]], [[spalart-allmaras]], [[adr-thread-transient-rans]]).
- F-045 (LES and hybrids) and F-207 (LES and hybrid catalogue) are partial: no decaying isotropic turbulence spectrum, no channel LES sanity check, no separated-flow validation, no IDDES ([[roadmap-turbulence]]).
