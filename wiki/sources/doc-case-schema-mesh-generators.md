---
type: source
tags: [interface, case, mesh]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: mesh generators and boundary-layer inflation

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Mesh generators and boundary-layer inflation", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Parameters of the built-in mesh generators, the rotating-zone annulus blocks, the sliding interface, the axial-fan sector generator, wall-layer inflation, the `[mesh.transform]` rotation and the solution-adaptive loop `[mesh.adapt]`.

## Key points

- Generators named: `cavity_2d` (square only when `nx = ny` and `lx = ly`), `box_2d`, `channel_2d`, `box_3d` (F-259, ADR 0191; six wall patches; `[mesh.box_3d] section` `box`/`rectangles`/`triangles`, ADR 0257), `channel_2d_interface` (non-conformal, F-114), `annulus_3d` and its `_interface`, `_stage`, `_cylindrical_interface`, `_sector_cylindrical_interface`, `_sector` variants, `pipe_sweep` (F-235), `axial_fan_sector` (F-133 partial, ADR 0065), `wedge_2d`, `nozzle_2d`, `cylinder_ogrid`.
- Options a generator does not apply (`wall_layers`, `periodic_x/y`, `grading != 1`) are rejected with a located diagnostic.
- Cylindrical interface pre-flight: worst interface non-orthogonality above 70 deg is rejected before the run (ADR 0109 runs diverged at 72.7 deg and above; the largest angle with a converged run is 48.75 deg).
- `[mesh.sliding]` (F-303, ADR 0637): the rotor block turns every step; transient laminar `piso`/`pimple`, `euler`, without energy, turbulence, sources and more; a step with interface faces over 70 deg stops the run. Sector form (F-711, ADR 0646) stays within 1.2e-2 of the full-annulus run over two pitches; `[mesh.sliding.coincidences] coupled = true` (F-730, ADR 0654).
- `axial_fan_sector`: `mesh.fan` keys (geometry, station lists ADR 0075, `topology` `h_slit`/`c_grid` ADR 0068, `c_grid_construction` `pursuit`/`smoothed`/`camber_seam`/`camber_seam_h`). It is "NOT validated for fan performance prediction"; the absolute `mrf` source is refused on it, use `rotating_frame`. `c_grid_grading` is "experimental, ADR 0181, not accepted".
- `mesh.wall_layers` (F-108): `box_2d`/`channel_2d` only; `layer_growth` 1.2; `target_y_plus` gives $h_1 = 2\, y^+ \nu / u_\tau$. Inflation on unstructured wall patches is not implemented (see `rcfd inflate`, [[doc-case-schema-decomposition-postprocessing]]).
- `[mesh.transform]` (ADR 0155): `rotate_z_deg` in [-360, 360], a plain number (a unit string is refused: `"-4 deg"` would be read in radians); only points move; settings that place things stay in world coordinates and trigger the warning rule `MT101`.
- `[mesh.adapt]` (F-342, ADR 0262): `cycles` 2, `max_level` 2, `refine_fraction` 0.4, `indicator` `velocity_gradient`; uniform lattice of `cavity_2d`/`box_2d`/`box_3d`, laminar with constant properties only; the run "cannot be checkpointed".

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-mesh]], [[mrf]], [[fan-01]], [[setup-rules]], [[case-file]]
