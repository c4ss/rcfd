---
type: concept
tags: [foundation, multiphase, particles]
sources: [ref-marzouk-huckaby-2009-particle-laden-swirl, ref-jaganathan-2023-basset-history-force, ref-balasubramanyam-2006-nasa-droplet-evaporation]
builds_on: [particle-equation-of-motion, conservation-law]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Two-way coupling (particle-to-fluid exchange of momentum, mass and heat)

## In short

In one-way coupling the fluid moves the particles but the particles do not affect the fluid. In two-way coupling every force the fluid puts on a particle is put back on the fluid with the opposite sign, as a source term in the cell the particle is in; evaporating droplets also give the gas mass and take or give heat ([[ref-marzouk-huckaby-2009-particle-laden-swirl]] sec. 2-3; [[ref-jaganathan-2023-basset-history-force]] sec. 1).

## Explanation

**One-way coupling and when it holds.** The Maxey-Riley description assumes a dilute suspension, small enough that particle-particle interaction can be neglected and the coupling is one-way, "i.e., neglect the effect of particle on fluid flow" ([[ref-jaganathan-2023-basset-history-force]] sec. 1).

**Momentum source.** The carrier momentum equation gets a source $S_p$ "accounting for the momentum from the particle-phase" ([[ref-marzouk-huckaby-2009-particle-laden-swirl]] Eq. 3). It is the reaction to the drag of the [[particle-equation-of-motion]]: when "calculating the resultant force due to the particle on the fluid phase, the algorithm takes into account the particle residence time in each cell" (sec. 3). By Newton's third law and the [[conservation-law]] for momentum, what the particles gain the fluid loses, so the sum over particles and fluid is conserved. In that study (mass loading 0.034 kg particles per kg air) the gas and particles were "coupled through two-way momentum exchange" (abstract, sec. 1).

**Parcels.** For many particles, "multiple similar particles are grouped in a single parcel" and the equations of motion are integrated per parcel; the results were not sensitive to particles versus parcels at that loading ([[ref-marzouk-huckaby-2009-particle-laden-swirl]] abstract, sec. 6).

**Mass and heat.** Evaporating droplets add vapour mass to the gas at the droplet's evaporation rate and exchange heat with it; in the spray study both one-way and two-way coupled calculations were run, with the two-way coupled one compared against measured spray penetration ([[ref-balasubramanyam-2006-nasa-droplet-evaporation]] abstract, results). See [[droplet-evaporation-and-breakup]].

## Builds on

- [[particle-equation-of-motion]]: the forces whose reactions are the sources.
- [[conservation-law]]: the exchange terms must cancel between the phases.

## Leads to

- [[lagrangian-particles]] (two-way coupling F-128, droplet feedback F-326), [[droplet-evaporation-and-breakup]]

## How rcfd uses it

- One-way by default: "The particles never act on the flow, so existing flow results and the baseline hashes are unchanged" (F-056, ADR 0028) ([[lagrangian-particles]]).
- The exchange record (F-128, ADR 0057) is an observation only; the droplet path (F-326, ADR 0348) feeds vapour mass, momentum and sensible heat into the carrier in the transient two-way path, conserved to 1.8e-15 / 5.8e-16 / 6.7e-13 of the largest term ([[lagrangian-particles]], [[adr-thread-particle-wall-and-droplets]]).
- The steady path with the droplet tables stays refused (ADR 0348 Q1); the vapour is a dilute source at constant density until F-252 (Q2) ([[adr-thread-particle-wall-and-droplets]]).
