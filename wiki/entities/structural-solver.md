---
type: entity
tags: [structural, fem, solver]
sources: [doc-structural-solver, doc-cosimulation-protocol, doc-acoustics, doc-product, doc-roadmap-overview]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Structural solver (FEM)

**What it is:** rcfd's finite-element structural solver for solid parts, the library crate [[rcfd-fem]]. "Early-stage software. Nothing in it is validated against a measured blade or a measured fan" ([[doc-structural-solver]]).
**Where:** `crates/rcfd-fem/`; glue modules in `rcfd-post` (`solid_coupling`, `solid_dynamic`, `solid_acoustic`); user page `docs/structural-solver.md`; the solid case file `docs/solid-case-file.md`.

## Details

- **Entry points:** Rust or the solid case file (a TOML file of its own run by a library function). "There is no CLI command and no MCP tool for the structural solver yet" (F-405); no `[solid]` table in the flow case (F-404) ([[doc-structural-solver]]).
- **Basic tier** (ADR 0270) in the code: linear elastostatics and modal analysis, Tet4 / Hex8 / Tet10 (and Hex20), meshes from box builders, FV meshes, the tet mesher and Gmsh; loads incl. centrifugal and thermal strain; direct and PCG solves; stresses; consistent mass; `.vtu` output. Five basic items partial by a missed check: F-378, F-381, F-383, F-384, F-386 ([[doc-structural-solver]]).
- **Intermediate and advanced tiers** (ADR 0290, 0310) added item by item: beams (Beam2, warping), shells (Shell3, Shell4, laminated), orthotropic / viscoelastic / creep / hyperelastic / J2 materials, cyclic symmetry, superelements and Craig-Bampton, buckling, harmonic and random response, rotor dynamics (Coriolis, whirl, Campbell, bearings), Newton and arc-length, contact, fatigue, fracture, multigrid and smoothed-aggregation CG, `rcfd-dist` and CUDA solves (F-433) ([[doc-structural-solver]]).
- **Counts:** hand-written summary "307 roadmap items ... as of 2026-11-18: 152 done, 129 partial, 26 todo"; the generated list on the same page has 154 done items ([[doc-structural-solver]]).
- **Fan-blade demonstration** (F-387, ADR 0500): one blade of `ComputerFan::default()` clamped at the hub, Tet10, nominal PBT-like material (E = 2.5 GPa, nu = 0.38, 1300 kg/m^3, "round numbers, not data"), 2000 rpm and 50 Pa; first frequency 188.8 -> 186.1 Hz coarse to fine; reactions balance to about 1e-11. "They do not show that a real fan blade has these frequencies or deflections" ([[doc-structural-solver]]).
- **Validated against publications:** hinged cylindrical roof limit load 2222.4 N vs 2226.3 N of Sze, Liu and Lo (2004), 1.8e-3 below (F-610); other checks are closed forms ([[doc-structural-solver]]).
- **Coupling with the flow:** one-way face forces (F-385); `StaticSolid` (massless, mock fluids only) and `DynamicSolid` as co-simulation participants; the elastic bar connected to the flow (F-401, ADR 0434) ([[doc-structural-solver]], [[doc-cosimulation-protocol]]). See [[cosimulation-protocol]].
- **Radiated sound** of a vibrating structural surface (ERP, baffled plane) lives in `rcfd_post::solid_acoustic` ([[doc-acoustics]]).
- **Plan approval:** ADR 0270 approved 2026-11-12 with ADR 0290 and 0310 ([[doc-roadmap-overview]]).

> [!warning] Contradiction — structural solver exists?
> [[doc-product]] says a structural solver "does not exist"; [[doc-structural-solver]] documents a large library with 150+ done items (already flagged on [[overview]]).

## Related

- [[rcfd-fem]]
- [[roadmap-structural-solver]]
- [[cosimulation-protocol]]
- [[aeroacoustics]]
- [[fan-passage-mesh]]

## Open questions

- Case-file keys for the fan blade and centrifugal load are proposed in ADR 0505 and "wait for the maintainer" ([[doc-structural-solver]]).
- No fan-rotor demonstration (F-406, F-436) and no public coupled benchmark (F-434) ([[doc-structural-solver]]).
