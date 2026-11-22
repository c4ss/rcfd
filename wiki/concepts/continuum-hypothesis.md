---
type: concept
tags: [foundation, shared-roots, mechanics]
sources: [ref-wikipedia-shared-roots, ref-oberkampf-trucano-2002-sand2002-0529, ref-bar-meir-fluid-mechanics]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Continuum hypothesis

## In short

Fluids and solids are made of molecules, but for engineering we treat them as if they filled space continuously, so that density, velocity, temperature and stress have a value at every point and can be differentiated. The assumption holds when the lengths that matter are much larger than the molecular spacing or the gas's mean free path ([[ref-wikipedia-shared-roots]] "Continuum mechanics", "Knudsen number").

## Explanation

**The assumption.** Continuum mechanics models a substance as if it "completely fill[s] the space it occupies", ignoring its atomic structure; its properties then become smooth functions of position and time, and the body can be cut into infinitesimal elements ([[ref-wikipedia-shared-roots]] "Continuum mechanics"). The field variables used everywhere downstream are defined this way: density `rho(x, t)`, velocity `v(x, t)`, stress `sigma(x, t)`, heat flux `q(x, t)`, internal energy per mass `e(x, t)` ([[ref-wikipedia-shared-roots]] "Continuum mechanics").

**When it holds.** It is accurate at length scales much larger than inter-atomic distances; one checks it through a representative volume that is small compared with the problem but large compared with the microstructure (separation of scales) ([[ref-wikipedia-shared-roots]] "Continuum mechanics"). For gases the measure is the Knudsen number `Kn = lambda / L`, mean free path over a representative length; by an empirical classification `Kn < 0.01` is continuum flow, `0.01 to 0.1` slip flow, `0.1 to 10` transitional, above 10 free molecular. Near or above one, the continuum assumption "is no longer a good approximation" ([[ref-wikipedia-shared-roots]] "Knudsen number", sec. "Application"). The Knudsen number is also in Bar-Meir's table of dimensionless numbers as mean free path to characteristic length ([[ref-bar-meir-fluid-mechanics]] Table 9.8).

**Why it matters for checking results.** Oberkampf and Trucano list the "assumption of a continuum fluid" among the acknowledged approximations that make up modelling error when a computation is compared with an experiment ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 4.1.2, p. 60). It is a modelling choice, made before any discretisation.

**What it adds.** Newtonian mechanics of particles becomes mechanics of fields: with continuous fields one can write derivatives such as `div v` and balance laws at a point ([[conservation-law]]).

## Builds on

Floor only: Newton's laws, density and pressure (first-year physics).

## Leads to

- [[conservation-law]]: balances of mass, momentum and energy over a control volume of continuous matter.
- [[stress]] and [[strain-and-strain-rate]]: tensors defined at a point.
- [[navier-stokes-equations]], [[structural-solver]] (solids), and through them every flow and solid model in rcfd.

## How rcfd uses it

- No rcfd page discusses the assumption directly. The flow solvers ([[simple]], [[piso]]) and the [[structural-solver]] are continuum models; rarefied (high-Knudsen) flow does not appear on any rcfd page in the wiki.
- In validation terms it is part of the modelling error of [[validation-comparison]].
