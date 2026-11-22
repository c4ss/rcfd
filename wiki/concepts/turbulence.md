---
type: concept
tags: [foundation, turbulence]
sources: [ref-davidson-turbulence-notes, ref-mit-ocw-2-29-turbulence, ref-menter-1992-nasa-tm-103975]
builds_on: [navier-stokes-equations, dimensionless-numbers]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Turbulence

## In short

Turbulence is the irregular, unsteady, three-dimensional motion that most real flows show at high Reynolds number: eddies of many sizes that mix momentum and heat far faster than molecular viscosity does. It still obeys the Navier-Stokes equations; it is just too fine-grained to compute directly in engineering flows, which is why turbulence models exist ([[ref-davidson-turbulence-notes]], [[ref-mit-ocw-2-29-turbulence]]).

## Explanation

**No definition, but recognisable features.** There is no formal definition of turbulent flow; it is recognised by a set of features ([[ref-davidson-turbulence-notes]] sec. 5.1, p. 77):
- *Irregularity:* the motion is chaotic and looks random, yet it is "deterministic and is described by the Navier-Stokes equations". It consists of eddies of many sizes; the largest are about the size of the flow geometry (the boundary-layer thickness, a jet width), the smallest are destroyed by viscosity.
- *Diffusivity:* turbulence increases the exchange of momentum; this delays separation on bluff bodies and raises wall friction and heat transfer in pipes and channels.
- *Large Reynolds numbers:* transition to turbulence happens in pipes at about $Re_D = 2300$ and in boundary layers at about $Re_x = 500\,000$.
- *Three-dimensional and unsteady* always; only the time-averaged field can be two-dimensional.
- *Dissipative:* the kinetic energy of the smallest eddies becomes heat; energy flows from the mean flow to the largest eddies and down to the smallest (the cascade, see [[energy-cascade-and-kolmogorov-scales]]).
- *Continuum:* even the smallest eddies are far larger than molecules.

The MIT lectures add that vortex stretching "is one of the main mechanisms to maintain or increase the intensity of turbulence", and that turbulence raises "stirring", which in turn raises molecular mixing; the averaged effect of stirring is what models represent as a "turbulent diffusion" ([[ref-mit-ocw-2-29-turbulence]] L24 s.18-19).

**What this adds to the Navier-Stokes equations.** The [[navier-stokes-equations]] already describe turbulent flow; nothing new is added to the physics. What turbulence adds is a problem of *scale*: the ratio of the largest to the smallest eddy grows like $Re_L^{3/4}$ per direction, so a direct simulation (DNS) of a 3D flow costs about $Re_L^3$ ([[ref-mit-ocw-2-29-turbulence]] L24 s.21, L25 s.4). The Reynolds number from [[dimensionless-numbers]] is what decides whether a flow is turbulent and how wide that range of scales is.

**Ways to compute it.** In order of fewer approximations and more cost: correlations, integral methods, averaged equations (RANS, [[reynolds-averaging]]), large-eddy simulation (LES, resolving the large eddies and modelling the small ones, [[les-and-des]]) and DNS (resolving all scales) ([[ref-mit-ocw-2-29-turbulence]] L24 s.23-24). Engineering CFD mostly uses RANS with an eddy-viscosity model. Among those, the algebraic models need a length scale that is hard to define in complex flows, which is why transport-equation models (one- and two-equation) are used ([[ref-menter-1992-nasa-tm-103975]] p. 1).

## Builds on

- [[navier-stokes-equations]]: turbulence is a solution of these equations; nothing is added to them.
- [[dimensionless-numbers]]: the Reynolds number sets transition and the range of eddy sizes.

## Leads to

- [[energy-cascade-and-kolmogorov-scales]], [[reynolds-averaging]], [[les-and-des]], [[law-of-the-wall]]
- rcfd models: [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]]

## How rcfd uses it

- rcfd's turbulence models live in [[rcfd-turb]]: k-epsilon, SST, Spalart-Allmaras, wall functions and wall distance, plus Smagorinsky, WALE, mixing length, DES/DDES and an RSM from the early waves ([[rcfd-turb]], [[adr-thread-early-turbulence]]).
- `turbulence.model` defaults to `laminar`; the other values are `k_epsilon`, `k_omega_sst`, `spalart_allmaras`, `smagorinsky`, `wale`, `mixing_length`, `rsm`, `plugin` ([[doc-case-schema-turbulence]]).
- The state of the whole turbulence line of work (16 items, 7 done, 9 partial at the snapshot) is on [[roadmap-turbulence]].
