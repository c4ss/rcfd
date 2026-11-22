---
type: entity
tags: [structural, fem, solver]
sources: [doc-structural-solver, doc-cosimulation-protocol, doc-acoustics, doc-product, doc-roadmap-overview, adr-thread-structural-case-file-and-fan-blade, adr-thread-structural-incompatible-hex8-rule-l, adr-thread-structural-hyperelastic-and-follower, adr-thread-structural-plan]
builds_on: [finite-element-method-basics, linear-elasticity, modal-analysis, isoparametric-elements-and-quadrature, stress-recovery, substructuring-and-craig-bampton]
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
>
> **Settled as far as ADR 0354 to 0518 allow (p3-structural-a, 2026-11-22):** the solver exists as a library (`rcfd-fem` plus `rcfd_api::solid_case`). It has static, modal, buckling, transient, harmonic, rotating and nonlinear analyses, `.vtu` stress output and a solid case file ([[adr-thread-structural-case-file-and-fan-blade]]). It is not reachable from the CLI, MCP or the flow case (F-405 and F-404 todo; ADR 0438 Q3). So the PRODUCT.md sentence is wrong about the solver but right that the Studio cannot reach stress results today. The full callout is on [[roadmap-structural-solver]].
>
> **From ADR 0519 to 0746 (p3-structural-b, 2026-11-22):** confirmed. The second half adds finite-strain elements, hyperelastic materials, contact, fracture and rotor paths, and names "the solid case's output" as an approved target for the driver's inertia (F-842, ADR 0719) ([[adr-thread-structural-incompatible-hex8-rule-l]], [[adr-thread-structural-hyperelastic-and-follower]]). The part that is still true: the Studio has no stress or strain colouring; its capability inventory lists only flow-case keys and its sources have no stress view at the snapshot ([[roadmap-structural-solver]]).

## ADR threads (2026-11-12 to 2026-11-14)

- [[adr-thread-structural-verification-elements]], [[adr-thread-structural-linear-solvers]], [[adr-thread-structural-modal-buckling-cyclic]], [[adr-thread-structural-rotordynamics]], [[adr-thread-structural-dynamics-harmonic]], [[adr-thread-structural-fatigue-and-radiated-sound]], [[adr-thread-structural-beams-shells]], [[adr-thread-structural-materials-nonlinear]], [[adr-thread-structural-contact]], [[adr-thread-structural-fsi-coupling]], [[adr-thread-structural-case-file-and-fan-blade]].
- Concepts: [[cyclic-symmetry]], [[campbell-diagram]], [[mpc-transformation]], [[mutation-testing]].
- Most items in these threads are partial without a numeric miss. The usual cause is a mutation that was not caught, or a check met only after an amendment, and the gap reads "The maintainer decides done" ([[roadmap-structural-solver]]).
- **The plan and the basic tier** (job p3-records-b): [[adr-thread-structural-plan]] (ADRs 0270, 0290, 0310 and the 49 stubs that stayed PLANNED, each mapped to its item and registration) and [[adr-thread-structural-basic-tier]] (the 17 registrations under the plan's own numbers: crate, elements, direct solver, assembly, loads, recovery, output, orders, thermal strain, CG, eigensolver, modal, static participant, scaffold, Hex20, MPCs, elasticity AMG).

## Builds on

- [[finite-element-method-basics]]: shape functions, element stiffness, assembly.
- [[linear-elasticity]]: Hooke's law and the small-strain tensor.
- [[modal-analysis]]: natural frequencies and mode shapes.
- [[isoparametric-elements-and-quadrature]]: mapped elements and Gauss quadrature.
- [[stress-recovery]]: Gauss-point stresses, averaging, patch recovery.
- [[substructuring-and-craig-bampton]]: static condensation and component mode synthesis.

## Related

- [[rcfd-fem]]
- [[roadmap-structural-solver]]
- [[cosimulation-protocol]]
- [[aeroacoustics]]
- [[fan-passage-mesh]]

## Open questions

- ~~Case-file keys for the fan blade and centrifugal load are proposed in ADR 0505 and "wait for the maintainer" ([[doc-structural-solver]]).~~ Approved by the maintainer on 2026-11-18 under the additive-golden-edit class rule, "The item may be issued" (ADR 0505 answers). They are not built at snapshot 2e94e5ef ([[adr-thread-structural-case-file-and-fan-blade]]).
- CLI subcommand and MCP tool for the solid file (F-405; ADR 0438 Q3) are still the maintainer's step ([[adr-thread-structural-case-file-and-fan-blade]]).
- No fan-rotor demonstration (F-406, F-436) and no public coupled benchmark (F-434) ([[doc-structural-solver]]).
