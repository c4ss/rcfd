---
type: source
tags: [particles, lagrangian, multiphase, how-and-why]
sources: []
origin: docs/particles.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Lagrangian particle tracking (F-056, ADR 0028)

**Origin:** [docs/particles.md](../../docs/particles.md) at `origin/main` 2e94e5ef
**Kind:** how-and-why page (434 lines)

## Summary

rcfd tracks point particles through a computed flow with one-way coupling (particles never act on the flow, so flow results and baseline hashes are unchanged), plus several opt-in library and case extensions. F-056 stays `partial`; F-128 done. Crate `rcfd-particles` (depends on `rcfd-core` and `rcfd-mesh` only), settings section `particles` (`rcfd-api::particles`).

## Key points

- **Not delivered** (long list): cyclone case, droplet/combusting/multicomponent types, two-way feedback inside `Simulation::step` (except the droplet path of F-326), dispersion from models other than k-epsilon and SST, surface reactions / evaporation / breakup in the case tree (library models), film splash and heat transfer, periodic patches, checkpointing, compressible flows.
- **Model:** drag + buoyant gravity; `particles.drag` = `stokes`, `schiller_naumann` (switch at Re 1000 UNVERIFIED; original 1933 paper not accessed), `none`. Not modelled: added mass, pressure gradient, Basset, lift, Brownian, rotation.
- **Integration:** `analytic` (exact for Stokes drag in a uniform field), `euler`, `rk2`, `rk4`; sub-steps at most half a cell and, non-analytic, a tenth of the relaxation time.
- **Tracking:** fan-triangulated faces, monotone terminating walk, bin-grid start, brute-force fallback. Periodic patches rejected.
- **Boundaries:** `reflect` (restitution e), `stick`, `escape`; more than `max_bounces` (1000) per step marks `stuck` (Zeno, ADR 0028 A2).
- **Injection:** point or surface, rate, parcel count, Rosin-Rammler; SplitMix64 seeded from `particles.seed` (bit-for-bit reproducible).
- **Run hooks:** since ADR 0037 (wave 21C) the run loop drives the tracker; a transient run with particles is not restartable. Outputs `particles.vtp`, `particle_report.json`, optional `particle_tracks.csv`.
- **F-128 (ADR 0057):** two-way exchange record (observation only), discrete random walk, cone/equal-mass injection.
- **F-255 (ADR 0208):** DRW from the turbulence model (k-epsilon, SST with `beta* = 0.09`); Taylor check within 1 percent (limit 5 percent); library surface reaction.
- **F-245 (ADR 0224):** evaporation (Bird-corrected film model, arXiv:2201.02818) and TAB breakup (O'Rourke and Amsden 1987), library; F-323 (ADR 0230) droplet exchange record; **F-326 (ADR 0348)** case tables `[particles.evaporation]`, `[particles.breakup]` feed the carrier in the transient two-way path.
- **Wall film** (F-253, ADR 0192): Nusselt film; gas buoyancy and shear opt-ins (F-292, ADR 0206).
- **Wall interaction** (F-254, ADR 0194): DNV RP O501 erosion (steel angle function), steel preset `dnv_o501_2007_steel` (ADR 0197) from "a third-party copy ... that has not been checked against the maintainer's edition"; Sommerfeld virtual-wall roughness.

## Contradictions with the wiki

- This page calls `rcfd-particles` "layer 2: depends on `rcfd-core` and `rcfd-mesh` only". [[doc-architecture]] places it in layer 3 (io / fv / particles). The dependency list agrees; the layer number differs (probably an older numbering). Between repo docs.

## Pages this source informs

- [[lagrangian-particles]]
- [[rcfd-particles]]
- [[rcfd-api]]
- [[provenance]]
- [[unverified]]
