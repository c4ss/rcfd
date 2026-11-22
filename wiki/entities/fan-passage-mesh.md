---
type: entity
tags: [mesh, fan, rotating]
sources: [doc-meshing, doc-structural-solver, adr-thread-sliding-mesh-rotor-stator, adr-thread-rom-reference-fan-and-c-grid]
builds_on: [periodic-and-cyclic-boundaries, relative-and-absolute-velocity-formulations, rotating-reference-frame, mesh-generation-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Axial-fan passage meshes (H-slit and C-grid)

**What it is:** rcfd's generators for one blade passage of an axial fan as a periodic-sector hex mesh: the H-slit grid (ADR 0065) and the C-grid (ADR 0068) with its successive constructions. "NOT validated for accurate fan performance prediction" ([[doc-meshing]]).
**Where:** `rcfd_mesh::fan` (geometry), `rcfd_mesh::fan_mesh` (H-slit), `rcfd_mesh::fan_cgrid` (C-grid); case hook `mesh.generator = "axial_fan_sector"`, `mesh.fan.topology = "c_grid"`, `mesh.fan.c_grid_construction`; cases `cases/axial_fan_sector`, `axial_fan_cgrid`.

## Details

- **Blade geometry** (`AxialFan`, F-133 partial, ADR 0065): radial stations with chord, stagger, camber, thickness, sweep, lean; NACA 4-digit or double circular arc sections on cylinders; `validate()` returns every problem as located `FanError`. Blade surface watertight; area and volume converge at orders 1.96 and 2.00 ([[doc-meshing]]).
- **Patches:** `inlet`, `outlet`, `hub`, `shroud`, `blade`, rotational pair `seam_lo` / `seam_hi` (rotated by exactly `2 pi / N`, mismatch 1.1e-16 m); cell zone `rotor` ([[doc-meshing]]).
- **Boundary-layer stack:** `h1 = 2 y+ nu / u_tau` with `Cf = 0.0576 Re_c^(-1/5)` at mid-span ([[doc-meshing]]).
- **Solver use:** `FvGeometry::new` accepts the sector; the absolute-formulation `mrf` source is refused (blade and seam faces are not surfaces of revolution), so smoke solves use `rotating_frame`. Seam mass flows do not cancel exactly (relative 3.5e-6 to 2.7e-5) ([[doc-meshing]]).

### Measured quality (max non-orthogonality / skewness / aspect; gates as registered)

| Mesh | Set-up | Result | Gate |
|---|---|---|---|
| H-slit (ADR 0065) | A, y+ 30, 138 192 cells | 73.4 deg / 7.57 / 810 | 70 / 2.0 / 300, missed |
| H-slit | B, y+ 1, 239 472 cells | 89.8 deg / 66 / 19 123 | aspect gate 3000, missed |
| C-grid (ADR 0068) | A' | 81.6 / 0.86 / 177 | non-orth missed |
| C-grid | B' | 83.7 / 4.85 / 2651 | non-orth and skewness missed |
| `smoothed` (ADR 0072) | A' / B' | 61.4 / 0.86 / 395; 78.2 / 7.1 / 8532 | aspect missed; all three missed on B' |
| `camber_seam` (ADR 0079) | FAN-01-C, 11 536 cells | 83.3 / 3.98 / 227 | 70 / 2.0 / 300 missed |
| `camber_seam_h` (ADR 0083) | FAN-01-C | 83.26 / 3.98 / 227 | missed |
| S (ADR 0692, library) | reference, 12 841 cells | 86.97 / 7.43 / 525.6 | 85 / 4.0, missed |
| S' (ADR 0696) | reference / second blade | 84.69 / 3.24 / 525.6; 84.22 / 4.011 / 551.7 | 85 / 4.0 / 1000; second blade skewness missed |
| S'' (ADR 0699) | reference / second blade | 84.95 / 2.76; 84.47 / 2.83 | within |
| S''' (ADR 0702) | reference / second blade | 84.69; 84.22 deg | — |
| S'''' (ADR 0713) | reference / second blade | 84.12 / 2.54 / 525.6; 83.89 / 2.72 / 551.7 | within 84.5 / 4.0 / 1000; scan builds 24 of 24 |

(all [[doc-meshing]])

- **Causes located:** H-slit worst cells sit behind the blunt trailing face, sheared by the stagger angle; an O- or C-grid "is the likely remedy" ([[doc-meshing]]). The 33A facet-sagitta guess was falsified (ADR 0072) ([[doc-meshing]]).
- **Robustness:** C-grid built 44 of 144 control combinations; `smoothed` 107 of 117 (median 74.5 deg); `camber_seam` 20 of 39 in-range FAN-01 combinations (none meets target); H 22 of 39 ([[doc-meshing]]).
- **Seam-topology floor:** "a non-orthogonality floor of about the stagger, 77.6 deg at the reference tip" ([[doc-meshing]]).
- **Defaults:** "The default stays the 33A construction"; S to S'''' are library only with no case value ([[doc-meshing]]).
- **FAN-01:** at the hub (chord 99 mm, stagger 44 deg, pitch 87 mm) `pursuit` and `smoothed` refuse it; `camber_seam` builds it; the laminar smoke solve on H oscillates without converging (continuity up to 7.46e-3) and the oscillation sits away from the poor faces ([[doc-meshing]]).
- The S constructions target `ComputerFan::rom_reference()` (stagger 69.8 to 77.6 deg), the reduced-order reference of the 120 mm computer fan ([[doc-meshing]]). The structural solver's blade mesher is a separate structured Tet4/Tet10 split of the same `AxialFan` geometry ([[doc-structural-solver]]).
- The sliding-interface generators of the case tree are different meshes: `annulus_3d_cylindrical_interface` (full annulus) and `annulus_3d_sector_cylindrical_interface` (sector below 180 deg, rotor and stator of one pitch, cell zone `rotor`, the 70 deg pre-flight), with no blade (F-303, ADR 0637; F-711, ADR 0646) ([[adr-thread-sliding-mesh-rotor-stator]]).

- **Constructions for strongly staggered blades (ADRs 0692 to 0713):** S, S', S'', S''' and S'''' are opt-in `CGridOptions` constructions beside `camber_seam_h`. The last, `camber_seam_s_free_rows_construction()`, builds `ComputerFan::rom_reference()` blade 0 at 84.122 / 2.538 / 525.6 and a second blade at 83.887 / 2.720 / 551.7, within 84.5 deg / 4.0 / 1000 ([[adr-thread-rom-reference-fan-and-c-grid]]). On FAN-01, S gives 88.0 deg and 4.30, which is worse ([[adr-thread-rom-reference-fan-and-c-grid]]).

## Builds on

- [[periodic-and-cyclic-boundaries]]: one blade passage with rotationally periodic sides.
- [[relative-and-absolute-velocity-formulations]], [[rotating-reference-frame]]: the rotating-frame solves run on these meshes ([[mrf]]).
- [[mesh-generation-basics]]: structured hex blocks (H-slit, C-grid).

## Related

- [[sliding-mesh]], [[adr-thread-fan01-rotating-solve]]
- [[fan-01]]
- [[mesh-quality]]
- [[mesh-generators]]
- [[mrf]]
- [[known-miss]]

## Open questions

- No construction meets the registered gates on FAN-01-C; whether a different topology is pursued is not stated here.
