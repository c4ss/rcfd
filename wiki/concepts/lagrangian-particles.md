---
type: concept
tags: [particles, lagrangian, multiphase]
sources: [doc-particles, adr-thread-particle-wall-and-droplets]
created: 2026-11-22
builds_on: [particle-equation-of-motion, explicit-time-integration-and-runge-kutta, turbulent-dispersion-random-walk, two-way-coupling]
updated: 2026-11-22
status: draft
---

# Lagrangian particle tracking

Following point particles (parcels) through a computed flow by integrating their equations of motion, cell to cell. rcfd tracks them with one-way coupling by default: "The particles never act on the flow, so existing flow results and the baseline hashes are unchanged" (F-056, ADR 0028; F-056 `partial`, F-128 done) ([[doc-particles]]).

## How rcfd uses it

- **Equation:** $dv/dt = f(Re)\,(u - v)/\tau_p + (1 - \rho_f/\rho_p)\, g$, $\tau_p = \rho_p d^2/(18 \mu)$; drag `stokes`, `schiller_naumann` (switch at Re 1000 UNVERIFIED), `none` ([[doc-particles]]).
- **Not modelled:** added mass, pressure-gradient force, Basset history, lift, Brownian motion, rotation, particle-particle interaction ([[doc-particles]]).
- **Integrators:** `analytic` (exact for Stokes drag in a uniform field), `euler`, `rk2`, `rk4` ([[doc-particles]]).
- **Tracking:** monotone face-crossing walk on fan-triangulated faces; bounding-box brute-force fallback for non-convex domains; periodic patches rejected; non-conformal interfaces only after `join_non_conformal` (not tested) ([[doc-particles]]).
- **Walls:** `reflect`, `stick`, `escape`; `max_bounces` 1000 then `stuck` ([[doc-particles]]).
- **Reproducibility:** SplitMix64 seeded from `particles.seed`, bit for bit ([[doc-particles]]).
- **Run loop:** since ADR 0037 `Simulation::new` builds the tracker and transient steps advance it; steady runs track once on the final field; transient runs with particles are not restartable ([[doc-particles]]).
- **Two-way:** the exchange record (F-128, ADR 0057) is an observation only; the droplet path (F-326, ADR 0348) feeds vapour mass, momentum and sensible heat into the carrier in the transient two-way path, conserved to 1.8e-15 / 5.8e-16 / 6.7e-13 of the largest term ([[doc-particles]]).
- **Dispersion:** discrete random walk; $k$, $\varepsilon$ from k-epsilon or SST ($\varepsilon = \beta^* k \omega$, $\beta^* = 0.09$) since F-255 (ADR 0208); other turbulence models refused ([[doc-particles]]).
- **Extensions:** surface reaction (library), evaporation and TAB breakup (library F-245, case tables F-326), wall film (F-253, F-292), erosion (DNV RP O501) and rough walls (Sommerfeld virtual wall) (F-254, ADR 0194) ([[doc-particles]]).
- **Wall models and droplet feedback (ADR thread):** wall film with gas buoyancy and shear (F-253, F-292), erosion with the `dnv_o501_2007_steel` preset and rough-wall rebound (F-254), dispersion from turbulence-model fields (F-255), d-squared evaporation and TAB breakup (F-245) with feedback in the exchange record (`Tracker::enable_droplet_exchange()`, F-323) and on the transient two-way path (F-326) ([[adr-thread-particle-wall-and-droplets]]).

## Builds on

- [[particle-equation-of-motion]]: drag, response time and the forces rcfd keeps or omits.
- [[explicit-time-integration-and-runge-kutta]]: the `euler`, `rk2`, `rk4` integrators.
- [[turbulent-dispersion-random-walk]]: the discrete random walk (`dispersion = "drw"`).
- [[two-way-coupling]]: exchange of momentum, mass and heat with the carrier.
- Also: [[particle-wall-interaction]], [[droplet-evaporation-and-breakup]].

## Related

- [[rcfd-particles]]
- [[rcfd-api]]
- [[unverified]]
- [[provenance]]

## Open questions

- No cyclone separation case (named by the roadmap acceptance) ([[doc-particles]]).
- The DNV steel preset comes from an unchecked third-party copy of RP O501 Rev 4.2-2007 ([[doc-particles]]).
