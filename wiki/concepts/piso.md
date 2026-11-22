---
type: concept
tags: [numerics, pressure-velocity, transient]
sources: [doc-dynamic-mesh, doc-component-diagonal-default, doc-cosimulation-protocol]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# PISO / PIMPLE

The transient segregated pressure-velocity coupling: per time step a momentum predictor and several pressure correctors (PISO), optionally inside outer iterations with relaxation (PIMPLE). In rcfd: `PimpleSolver`, case values `piso`, `pimple` ([[doc-dynamic-mesh]]).

## How rcfd uses it

- **Time-step limit in narrow gaps** (F-244, ADR 0211; located by F-299, ADR 0214): the default PISO step "grows unstable above a diffusion number `nu dt/dx^2` of about 3 to 4 across the finest cells" (16 cells across the gap, two correctors); the mode alternates in sign every step. Same limit in a plane channel (F-530, ADR 0451) ([[doc-dynamic-mesh]]).
- **Remedy:** consistent (SIMPLEC-type) correctors, case key `[solver.consistent_correctors] enabled = true` (F-301, ADR 0223); at large diffusion numbers also outer iterations (8, two correctors, no relaxation: within 0.6 percent of the discrete model) ([[doc-dynamic-mesh]]).
- **Library opt-ins without case keys:** `outer_pressure_tolerance` makes `n_outer` a cap (F-536, ADR 0458; "keep a cap"); `outer_scale_window` (F-550, ADR 0617); `lag_onset_refusal` refuses the first step when the ADR 0073 lag estimate is at least one (F-535, ADR 0471); `moving_mesh_lag` (ADR 0223 E0) ([[doc-dynamic-mesh]]).
- **Moving meshes:** Euler only in the case tree; BDF2 opt-in in the library (F-345, ADR 0379) ([[doc-dynamic-mesh]]).
- **Periodic and sliding opt-ins:** `rotational_periodic_implicit`, `coupled_non_orth`, `coupled_ddt_correction`, `fractional_body_force` ([[doc-dynamic-mesh]]); see [[sliding-mesh]].
- **Component diagonal:** the key reaches PISO/PIMPLE since ADR 0074 W1 (before "silently ignored although documented"); the split is applied in every cell without the bit-equality guard ([[doc-component-diagonal-default]]). See [[component-diagonal]].
- **Co-simulation fluid side** is the incompressible PISO/PIMPLE step with implicit Euler on a moving mesh ([[doc-cosimulation-protocol]]).

## Related

- [[simple]]
- [[dynamic-mesh]]
- [[sliding-mesh]]
- [[component-diagonal]]
- [[rcfd-fv]]

## Open questions

- The adaptive corrector count and SV301 exclude dynamic meshes (ADR 0223 Q2) ([[doc-dynamic-mesh]]).
