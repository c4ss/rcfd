---
type: concept
tags: [foundation, turbulence, rans]
sources: [ref-davidson-turbulence-notes, ref-mit-ocw-2-29-turbulence, ref-nasa-tmr]
builds_on: [turbulence, navier-stokes-equations]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reynolds averaging and the RANS equations

## In short

Split every flow variable into a mean and a fluctuation, `u = U + u'`, and average the Navier-Stokes equations. The result, the Reynolds-averaged Navier-Stokes (RANS) equations, looks like the original equations for the mean flow plus one new term, the Reynolds stress, which is unknown. Supplying it is the job of a turbulence model ([[ref-davidson-turbulence-notes]], [[ref-mit-ocw-2-29-turbulence]]).

## Explanation

**Decomposition and averages.** Write `u_i = u_bar_i + u_i'` and `p = p_bar + p'`. The average can be a time average over an interval `T` long compared with the turbulent fluctuations (for a statistically steady flow) or an ensemble average over many repetitions of the experiment (for an unsteady mean flow); for a stationary process the two agree ([[ref-mit-ocw-2-29-turbulence]] L24 s.18, L25 s.9). The TMR states the time scale requirement as "long compared to turbulent fluctuations, and short compared to unsteadiness in the mean flow" ([[ref-nasa-tmr]] `implementrans.html`). By construction `mean(u_i') = 0` ([[ref-davidson-turbulence-notes]] eq. 6.4).

**Averaging rules.** The average of a linear term is the same term of the mean, and derivatives commute with averaging; only products of fluctuations survive: `mean(u_i u_j) = u_bar_i u_bar_j + mean(u_i' u_j')` ([[ref-mit-ocw-2-29-turbulence]] L25 s.11; [[ref-davidson-turbulence-notes]] sec. 6.1, p. 88).

**The RANS equations** (incompressible, constant viscosity, steady mean flow) ([[ref-davidson-turbulence-notes]] eqs. 6.9-6.10):
- continuity `d u_bar_i / dx_i = 0`;
- momentum `d(u_bar_i u_bar_j)/dx_j = -(1/rho) d p_bar/dx_i + d/dx_j [ nu d u_bar_i/dx_j - mean(u_i' u_j') ]`.

Here `u_bar` is the mean velocity, `p_bar` the mean pressure, `nu` the kinematic viscosity. The new term `-rho mean(u_i' u_j')` acts on the mean flow like an extra stress: the [[reynolds-stress]]. For compressible flow the same is done with a density-weighted (Favre) average `f_hat = mean(rho f)/mean(rho)` ([[ref-nasa-tmr]] `implementrans.html`).

**The closure problem.** The RANS system has ten unknowns (three mean velocities, the pressure and six independent Reynolds stresses) but only four equations, so it "is not closed" ([[ref-davidson-turbulence-notes]] p. 88; [[ref-mit-ocw-2-29-turbulence]] L25 s.12). Every RANS turbulence model is a way to express the Reynolds stresses through mean-flow quantities, either by a constitutive relation (eddy viscosity, [[boussinesq-eddy-viscosity]]) or by transport equations for the stresses themselves ([[reynolds-stress-models]]) ([[ref-nasa-tmr]] `implementrans.html`).

**What it adds to its prerequisites.** The [[navier-stokes-equations]] describe every eddy of a [[turbulence|turbulent]] flow; Reynolds averaging trades that detail for equations of the mean flow only, which can be solved on a much coarser grid and in steady form. The price is the unknown Reynolds stress. The decomposition is not unique: "some arbitrariness in the decomposition" and in calling the fluctuations turbulence ([[ref-mit-ocw-2-29-turbulence]] L25 s.10). When the mean flow itself is unsteady (vortex shedding, a rotor passing) the averaged equations keep a time derivative; this is unsteady RANS (URANS).

## Builds on

- [[turbulence]]: the fluctuations that are averaged out.
- [[navier-stokes-equations]]: the equations that are averaged.

## Leads to

- [[reynolds-stress]], [[boussinesq-eddy-viscosity]], [[reynolds-stress-models]], [[les-and-des]] (spatial filtering instead of averaging)
- rcfd models: [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]]

## How rcfd uses it

- All of rcfd's RANS models solve the averaged equations; they need a wall patch and a positive `physics.nu` ([[doc-case-schema-turbulence]]).
- Unsteady RANS: SST (F-286, done), k-epsilon and Spalart-Allmaras without DES (F-294) run on the outer iterations of `pimple` on a static mesh, with the momentum's time scheme; `piso` is refused for the transported models ([[adr-thread-transient-rans]]).
