---
type: concept
tags: [foundation, turbulence, boundary-layer]
sources: [ref-davidson-turbulence-notes, ref-mit-ocw-2-29-turbulence, ref-nasa-tmr]
builds_on: [friction-velocity-and-y-plus, turbulence]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Law of the wall

## In short

In wall units the mean velocity near a smooth wall follows a universal profile: `u+ = y+` in the viscous sublayer (`y+` below about 5), the logarithmic law `u+ = (1/kappa) ln y+ + B` in the log layer (`y+` above about 30, with `kappa` about 0.41 and `B` about 5.2), and a buffer layer in between where neither holds. It is the basis of wall functions and the main check of a turbulence model near a wall ([[ref-davidson-turbulence-notes]], [[ref-mit-ocw-2-29-turbulence]]).

## Explanation

**Layers** ([[ref-davidson-turbulence-notes]] sec. 6.2, pp. 90-93, Fig. 6.2):
- *Viscous sublayer*, `y+ <= 5`: viscous diffusion dominates; the total shear stress equals `tau_w`, so `mu dU/dy = tau_w` integrates to `U = u_tau^2 y/nu`, i.e. `u+ = y+` (eq. 6.24).
- *Buffer layer*, `5 < y+ < 30`: viscous diffusion of momentum is gradually replaced by turbulent diffusion; the viscous and Reynolds shear stresses are equal at about `y+ = 11`.
- *Log layer*, from `y+ = 30` to about `y/delta = 0.3` in channels (about 0.1 in boundary layers): viscosity is negligible and the Reynolds stress is about `tau_w`.
- *Outer (wake) region*: scaled with the boundary-layer thickness.

In the whole inner region the total shear stress is approximately constant and equal to the wall shear stress.

**Deriving the log law.** In the log layer the velocity scale is `u_tau` and the only length is the wall distance, `l = kappa y` ("an eddy cannot be larger than the distance to the wall"). So `dU/dy = u_tau/(kappa y)` (eq. 6.28), equivalently an eddy viscosity `nu_t = kappa u_tau y` with `-mean(u'v') = u_tau^2` (eqs. 6.29-6.31). Integrating ([[ref-davidson-turbulence-notes]] eq. 6.33):

`u+ = (1/kappa) ln y+ + B`,  usually `kappa = 0.41`, `B = 5.2`

the logarithmic law due to von Karman. The MIT notes derive it the same way from Prandtl's mixing length `L = kappa y` and quote `kappa = 0.41` ([[ref-mit-ocw-2-29-turbulence]] L25 s.15, s.23). Its upper limit in `y+` grows with Reynolds number ([[ref-davidson-turbulence-notes]] p. 93). A common alternative writing is `u+ = (1/kappa) ln(E y+)` with `B = (1/kappa) ln E` (Davidson uses `E = 9.0`, eq. 11.125).

**Outer region and wake.** Coles' law of the wall adds a wake function: `u+ = (1/kappa) ln(y+) + C + (2 Pi/kappa) sin^2(pi y/(2 delta))`, used by the TMR "along with a van Driest type damping near the wall" for its flat-plate profile at `Re_theta = 10 000` ([[ref-nasa-tmr]] `flatplate_val.html`); Davidson quotes `kappa = 0.38`, `B = 4.1`, `Pi = 0.5` for that form from boundary-layer data (eq. 6.35).

**What it adds to its prerequisites.** [[friction-velocity-and-y-plus]] defines the wall units; [[turbulence]] supplies eddies whose size is limited by the wall. Together they give a universal near-wall profile, so the velocity at a known `y+` fixes `u_tau` and the wall shear, which is exactly what [[wall-functions]] exploit, and what a wall-resolved model must reproduce.

## Builds on

- [[friction-velocity-and-y-plus]]: the wall units `u+`, `y+`.
- [[turbulence]]: the eddy transport that dominates the log layer.

## Leads to

- [[wall-functions]], [[mixing-length-model]]
- [[k-epsilon]] (`WallConstants`), [[spalart-allmaras]] (Spalding law), [[flat-plate]]

## How rcfd uses it

- `WallConstants`: kappa `0.41`, E `9.8`, Cmu `0.09`, sublayer switch at `y+` `11.5` (ADR 0707) ([[adr-thread-transient-rans]], [[k-epsilon]]); case defaults `turbulence.kappa` 0.41 and `e_wall` 9.8 ([[doc-case-schema-turbulence]]).
- rcfd records log-law `kappa = 0.41`, `B = 5.2` (Pope 2000) as verified, the wall-function pair `kappa = 0.4187`, `E = 9.793` as UNVERIFIED (rounded to 0.41 / 9.8), and Reichardt's profile as verified against DNS ([[doc-reference-data-turbulence-constants]]).
- Spalding's single formula for the whole inner region, `y+ = u+ + exp(-kappa B)[exp(kappa u+) - 1 - kappa u+ - (kappa u+)^2/2 - (kappa u+)^3/6]`, is in `wall.rs`; the repo marks it "confirmed (secondary)", original paper not read ([[doc-reference-data-sst-wall]], [[rcfd-turb]]). No open source for it was read in this job.
- Wave 30C's SST flat plate missed the D2 log-law fit; ADR 0063 left "the log-law slope 2.78" unexplained ([[adr-thread-early-turbulence]]).
