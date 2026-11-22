---
type: source
tags: [interface, case, turbulence]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: turbulence

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`turbulence`", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Keys of the `turbulence` table: model choice, wall treatment, coefficients, live switching, per-boundary turbulence keys, the turbulence outputs and the force/moment report components. The longest section of the schema (lines 73-472).

## Key points

- `turbulence.model`: `laminar` (default), `k_epsilon`, `k_omega_sst` (Menter 2003, TMR form), `spalart_allmaras` (SA-noft2), `smagorinsky`, `wale`, `mixing_length`, `rsm`, `plugin`. `variant` `standard`/`rng`/`realizable` is k-epsilon only.
- Defaults: `wall_treatment = smooth`, `wall_function = standard`, `kappa` 0.41, `e_wall` 9.8, `convection = upwind`, `relax` 0.7, `relax_nut` 0.5, `initial_intensity` 0.05, `initial_viscosity_ratio` 10.
- RANS models need a wall patch, a positive `physics.nu`, no `energy.enabled`. k-epsilon walls must be at rest; SST allows a moving wall (wall law relative to the wall velocity, ADR 0075).
- Transient RANS: with `pimple` on a static mesh, k-epsilon (ADR 0210), SST (ADR 0199) and SA with `sa.des = "off"` get the time derivative of `solver.time_scheme` and one sweep per outer iteration; `piso`, a dynamic mesh, `solver.error_tolerance` and the fractional step are located errors.
- `turbulence.periodic_seam.diffusivity = "pair"` (ADR 0205, 0212, 0227) gives both faces of a periodic pair one diffusivity; `"owner"` is the default and keeps every earlier result.
- SST: wall omega `10 * 6 nu / (beta1 y^2)`; least accurate range y+ about 3 to 20; `linearUpwind` for k/omega "diverged in tests".
- SA: `nutilde_ratio` 3; `sa.wall_mode` `auto` / `resolved` / `auto_sa_law` (opt-in, ADR 0168); `sa.des` `off`/`des`/`ddes` (transient only; "not validated on a separated flow"); `sa.c_des` 0.65. Flat plate Cf(x = 0.97) +3.0 percent from the TMR CFL3D SA value (ADR 0014).
- Far fields default to the `freestream` turbulence role with SA (ADR 0166), SST and k-epsilon (ADR 0169); before, `zero_gradient`. `turbulence = "inlet"` on a far field accumulates (nuTilda grew to the cap `1e5 nu` within about 1500 iterations, ADR 0164 am. 1).
- `rsm`: transient only, **no wall patch**; the wall-bounded channel check (C7) "was **not delivered**".
- `k_omega_sst` coefficients are "shown, not editable"; `realizable_a0` 0 keeps 4.04 (the original report prints 4.0).
- Live switching (F-044): in a `ready`/`paused` service session, `settings.set` on `turbulence.*` rebuilds only the turbulence state (velocity, pressure, flux, counter, monitors kept); switching from or to `laminar` needs a rebuild.
- Report `component` (ADR 0164, 0166, 0170): `total`/`viscous` use the face viscosity the momentum equation used (on wall faces the wall-function value); `total_molecular` reproduces pre-ADR-0166 forces and pre-ADR-0170 moments. "The FAN-01 torques of ADRs 0075 to 0136 are such molecular moments." The molecular value made ADR 0159's NACA 0012 drag at 0 degrees "about half the measured value".
- Wave 26C report types (ADR 0048, F-238): `volume_std`, `area_std`, `area_min`, `area_max`, `mass_weighted_average`, `vector_flux`.

## Contradictions with the wiki

See [[doc-case-schema-caveats]] (the Caveats section vs. this section).

## Pages this source informs

- [[report]], [[fan-01]], [[rcfd-turb]], [[case-file]]
