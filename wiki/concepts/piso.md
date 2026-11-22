---
type: concept
tags: [numerics, pressure-velocity, transient]
sources: [doc-dynamic-mesh, doc-component-diagonal-default, doc-cosimulation-protocol, adr-thread-piso-stability-consistent-correctors, adr-thread-bdf2-step-consistent-face-flux, adr-thread-transient-rans, doc-investigation-job48t-cylinder-spokes, ref-jasak-1996-phd-thesis]
created: 2026-11-22
builds_on: [simple, implicit-time-integration, stability-and-cfl]
updated: 2026-11-22
status: draft
---

# PISO / PIMPLE

The transient segregated pressure-velocity coupling: per time step a momentum predictor and several pressure correctors (PISO), optionally inside outer iterations with relaxation (PIMPLE). In rcfd: `PimpleSolver`, case values `piso`, `pimple` ([[doc-dynamic-mesh]]).

## How rcfd uses it

- **Time-step limit in narrow gaps** (F-244, ADR 0211; located by F-299, ADR 0214): the default PISO step "grows unstable above a diffusion number $\nu\,\Delta t/\Delta x^2$ of about 3 to 4 across the finest cells" (16 cells across the gap, two correctors); the mode alternates in sign every step. Same limit in a plane channel (F-530, ADR 0451) ([[doc-dynamic-mesh]]).
- **Remedy:** consistent (SIMPLEC-type) correctors, case key `[solver.consistent_correctors] enabled = true` (F-301, ADR 0223); at large diffusion numbers also outer iterations (8, two correctors, no relaxation: within 0.6 percent of the discrete model) ([[doc-dynamic-mesh]]).
- **Library opt-ins without case keys:** `outer_pressure_tolerance` makes `n_outer` a cap (F-536, ADR 0458; "keep a cap"); `outer_scale_window` (F-550, ADR 0617); `lag_onset_refusal` refuses the first step when the ADR 0073 lag estimate is at least one (F-535, ADR 0471); `moving_mesh_lag` (ADR 0223 E0) ([[doc-dynamic-mesh]]).
- **Moving meshes:** Euler only in the case tree; BDF2 opt-in in the library (F-345, ADR 0379) ([[doc-dynamic-mesh]]).
- **Periodic and sliding opt-ins:** `rotational_periodic_implicit`, `coupled_non_orth`, `coupled_ddt_correction`, `fractional_body_force` ([[doc-dynamic-mesh]]); see [[sliding-mesh]].
- **Component diagonal:** the key reaches PISO/PIMPLE since ADR 0074 W1 (before "silently ignored although documented"); the split is applied in every cell without the bit-equality guard ([[doc-component-diagonal-default]]). See [[component-diagonal]].
- **Co-simulation fluid side** is the incompressible PISO/PIMPLE step with implicit Euler on a moving mesh ([[doc-cosimulation-protocol]]).
- **ADR thread:** the full chain from F-299 to F-550, with the benchmark set of ADR 0223 (cavity 88 of 88 stable with the option, default 84 of 88) and the maintainer's "Held" of 2026-11-18 on the default flip, is in [[adr-thread-piso-stability-consistent-correctors]].
- **Step-consistent face flux:** the opt-in `PimpleSettings::rhie_chow_step_consistent` / `[solver.rhie_chow] step_consistent` removes the step-size dependence of the Rhie-Chow face flux on internal faces (F-344, F-443); BDF2 then meets its order at rest but not on the moving overset TG_2 (`2.454` against 4) ([[adr-thread-bdf2-step-consistent-face-flux]]).
- **Transient RANS:** SST, k-epsilon and SA without DES update once per outer iteration of `pimple`; `piso` is refused for them ([[adr-thread-transient-rans]]).
- Job 48T (ADR 0154): on a cylinder O-grid with thin wall cells the default PISO (one outer iteration, two correctors) amplifies its leftover pressure error from step to step once nu dt / dr1^2 passes an onset between 6.5 and 11.4, giving wall pressure spokes; three correctors, `fractional_step` or a smaller dt remove them ([[doc-investigation-job48t-cylinder-spokes]]).
- **Adaptive correctors (opt-in, ADR 0156):** `solver.corrector_control = "adaptive"` counts the PISO/PIMPLE correctors up until the pressure-lag estimate is at most 0.5, with a cap of 10, and re-estimates when dt changes by a factor 1.5 (ADR 0162); SV301 warns before a run when the estimate is at least 1.3. The library default stays `fixed`; `adaptive` is refused with `dynamic_mesh` ([[adr-thread-piso-stability-consistent-correctors]]).

## Builds on

- [[simple]]: the same predictor-corrector building blocks; in PISO the explicit velocity correction neglects the neighbour corrections, so $H(U)$ is updated and the pressure equation solved again with fixed $H$ coefficients ([[ref-jasak-1996-phd-thesis]] 3.8.2.1).
- [[implicit-time-integration]]: implicit Euler and the opt-in BDF2.
- [[stability-and-cfl]]: Courant and diffusion numbers behind the time-step limits above.
- Also relevant: [[projection-method]] (`fractional_step`), [[rhie-chow-interpolation]] (step-consistent face flux).

## Related

- [[simple]]
- [[dynamic-mesh]]
- [[sliding-mesh]]
- [[component-diagonal]]
- [[rcfd-fv]]
- [[doc-investigation-job48t-cylinder-spokes]]

## Open questions

- The adaptive corrector count and SV301 exclude dynamic meshes (ADR 0223 Q2) ([[doc-dynamic-mesh]]).
