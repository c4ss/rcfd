---
type: concept
tags: [foundation, turbulence, rans]
sources: [ref-mit-ocw-2-29-turbulence, ref-menter-1992-nasa-tm-103975, ref-nasa-tmr, ref-davidson-turbulence-notes]
builds_on: [boussinesq-eddy-viscosity, energy-cascade-and-kolmogorov-scales, convection-diffusion-equation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Two-equation turbulence models

## In short

A two-equation model solves two extra transport equations, one for the turbulent kinetic energy `k` and one for a second quantity (`epsilon`, `omega`, ...) that together give a velocity scale and a length scale. From them it builds the eddy viscosity `nu_t`. This removes the prescribed length of the mixing-length model ([[ref-menter-1992-nasa-tm-103975]], [[ref-mit-ocw-2-29-turbulence]]).

## Explanation

**Why two.** An eddy viscosity needs a velocity scale and a length scale ([[boussinesq-eddy-viscosity]]). `k` gives the velocity scale `sqrt(k)`. A second variable gives the length (or time) scale: with `epsilon`, `L ~ k^(3/2)/epsilon`; with `omega ~ epsilon/k`, `L ~ sqrt(k)/omega` ([[ref-mit-ocw-2-29-turbulence]] L25 s.17, s.20; [[ref-menter-1992-nasa-tm-103975]] p. 5). Two-equation models "solve two transport equations, generally one for the turbulent kinetic energy and another one related to the turbulent length- (or time-) scale" ([[ref-menter-1992-nasa-tm-103975]] p. 2).

**The generic form.** Each equation is a [[convection-diffusion-equation]] with sources:

`d(rho phi)/dt + d(rho U_j phi)/dx_j = Production - Destruction + d/dx_j[(mu + mu_t/sigma_phi) d phi/dx_j] (+ extra terms)`

- *Convection* by the mean flow on the left.
- *Production* of `k`: `P_k = -rho mean(u_i' u_j') dU_i/dx_j`, the energy the mean flow loses to the fluctuations; with the Boussinesq relation it becomes `P_k = mu_t S^2` in incompressible flow ([[ref-mit-ocw-2-29-turbulence]] L25 s.18-19; [[ref-nasa-tmr]] `sst.html`).
- *Destruction*: `rho epsilon` in the k equation (written `beta* rho omega k` in k-omega form); a matching sink in the second equation.
- *Diffusion*: molecular plus "eddy diffusion", modelling the transport of `k` by pressure and velocity fluctuations as `mu_t/sigma_k dk/dx_j` ([[ref-mit-ocw-2-29-turbulence]] L25 s.19).

The source terms of the second equation are taken proportional to those of `k`, with the ratio `epsilon/k` (or `omega/k`) for dimensions ([[ref-mit-ocw-2-29-turbulence]] L25 s.20). The model constants (`C_mu`, `C1`, `C2`, `sigma_k`, `sigma_eps` for k-epsilon; `beta*`, `beta`, `gamma`, `sigma_k`, `sigma_w` for k-omega) are fixed numbers of each model; for k-omega the TMR gives `gamma = beta/beta* - sigma_w kappa^2/sqrt(beta*)`, chosen to give the Karman constant `kappa` ([[ref-nasa-tmr]] `wilcox.html`).

**Two standard members.**
- k-epsilon (Launder and Spalding 1974): `nu_t = C_mu k^2/epsilon`, constants `C_mu = 0.09`, `C1 = 1.44`, `C2 = 1.92`, `sigma_k = 1.0`, `sigma_eps = 1.3`; the epsilon equation rests on the "equilibrium turbulent flows" idea that dissipation balances production ([[ref-mit-ocw-2-29-turbulence]] L25 s.20-21). See [[k-epsilon]].
- k-omega (Wilcox): `nu_t = k/omega`, `omega` the "specific turbulent dissipation rate (or turbulence frequency)" ([[ref-menter-1992-nasa-tm-103975]] p. 2). See [[k-omega-wilcox]].

**Known weaknesses that later models address.** k-epsilon lacks sensitivity to adverse pressure gradients ("significantly too high shear-stress levels and thereby delays (or completely prevents) separation") and needs damping functions and a non-zero wall value of `epsilon` near walls; k-omega depends strongly on its free-stream `omega`; both use the Boussinesq relation, which over-predicts shear stress when production exceeds dissipation ([[ref-menter-1992-nasa-tm-103975]] pp. 2-3, 6-7). The k and epsilon (or omega) time scales are much shorter than the mean flow's, so they are usually solved separately from the mean flow with under-relaxation ([[ref-mit-ocw-2-29-turbulence]] L25 s.22; [[ref-menter-1992-nasa-tm-103975]] p. 9).

**What it adds to its prerequisites.** [[boussinesq-eddy-viscosity]] needs `nu_t`; the [[energy-cascade-and-kolmogorov-scales|cascade]] says that `k` and `epsilon` characterise the energetic eddies; the [[convection-diffusion-equation]] provides the form of the transport equations. Put together, `nu_t` is computed from quantities that are convected, diffused, produced and destroyed with the flow, so the length scale adapts to the flow instead of being prescribed as in the [[mixing-length-model]].

## Builds on

- [[boussinesq-eddy-viscosity]]: the stress relation and `nu_t`.
- [[energy-cascade-and-kolmogorov-scales]]: the meaning of `k` and `epsilon` and the length scale `k^(3/2)/epsilon`.
- [[convection-diffusion-equation]]: the form of each transport equation.

## Leads to

- [[k-epsilon]], [[k-omega-wilcox]], [[sst-blending-functions]], [[k-omega-sst]], [[reynolds-stress-models]]

## How rcfd uses it

- rcfd's two-equation models are the k-epsilon family (standard, RNG, realizable) and k-omega SST (plus BSL) ([[rcfd-turb]], [[adr-thread-early-turbulence]]); their constants are recorded on [[doc-reference-data-turbulence-constants]] and [[doc-reference-data-sst-wall]].
- A "generalised two-equation model with tunable coefficients" is in F-206 and not delivered ([[roadmap-turbulence]]).
