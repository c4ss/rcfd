---
type: source
tags: [milestone, validation, data]
sources: []
origin: docs/milestones.json
origin_commit: 09647baa
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# milestones.json (simulation milestones)

**Origin:** [docs/milestones.json](../../docs/milestones.json) at origin/main 09647baa (first read at 2e94e5ef, three milestones; seven added since)
**Kind:** repo data file: the ~~three~~ ten simulation milestones, each with result statement, cases, reference and headline roadmap items.

## Summary

Machine-readable definition of ~~three milestones~~ ten milestones at 09647baa: `vortex-shedding`, `fan` and `aerofoil-lift` (unchanged, byte for byte), six added by ADR 0775 (F-871) and `fan-structure` added by ADR 0779 (F-875) ([[adr-thread-simulation-milestones-validation]]). Each names the test files or cases that carry it, a reference string that points into [[doc-reference-data]] or [[doc-validation-data]], and the headline F-items.

## Key points

- `vortex-shedding` - "Vortex shedding behind a cylinder": "Laminar flow past a circular cylinder at Re 100 on a body-fitted O-grid: the Strouhal number and the mean drag coefficient against the published values." Reference: "St 0.164 to 0.167 and C_D 1.314 to 1.370 at Re 100 (arXiv:2303.09262 Table 1 and arXiv:1401.6513 Table 3, read as quotations) and Williamson, J. Fluid Mech. 206 (1989), eq. (6), St(100) = 0.164335". Cases: `crates/rcfd-fv/tests/cylinder.rs`, `validation_cylinder_re100*.rs` (three files), `w48t_cylinder_spokes.rs`. Headline: F-029, F-869.
- `fan` - "Assistant-driven fan design" (page `docs/milestone-fan.md`). Reference for the fan solve: the FAN 01 measured characteristic, Zenodo doi:10.5281/zenodo.20037409 (v5), file `characteristic_n1ug.mat`, "design point 1.4032 m^3/s, 126.46 Pa, 333.30 W" (the primary rig paper SAE 2016-01-1805 recorded as not read); "for the noise, no measured fan noise is recorded (docs/fan-project-breakdown.md, part D5: missing, needs data)". Cases: 11, from `fan01_validation.rs` to `f697_fan_objective.rs` and `cases/axial_fan_sector`. Headline: F-870, F-872, F-873, F-260, F-702, F-874.
- `aerofoil-lift` - "Aerofoil lift (NACA 0012)": "The NACA 0012 at Re 6e6 in incompressible steady RANS on an O-grid: the lift curve up to maximum lift against the measured lift." Reference: Ladson, NASA TM 4074 (1988), tripped, M 0.15, Re 6e6, as the TMR file `CLCD_Ladson_expdata.dat` ("the report itself is not recorded as read"). Cases: `cases/naca0012/case.json`, two AoA-sweep scripts and tests. Headline: F-043, F-868.

- Added at 09647baa by ADR 0775 (F-871), written by `python scripts/roadmap_milestones.py --apply docs/adr/0775-f871-mapping.tsv --definitions docs/adr/0775-f871-milestones.json`:
  - `lid-driven-cavity` - "Lid-driven cavity": steady laminar flow in a square cavity at Re 100, 400 and 1000, SIMPLE or SIMPLEC on uniform meshes, "the centreline velocity profiles against the published tables". Reference: Ghia, Ghia and Shin (1982), "verified against secondary copies; the printed original is recorded as not read", and for Re 1000 Botella and Peyret (1998) as in arXiv:1807.04896 Table A-1. Cases: `crates/rcfd-fv/tests/cavity.rs`, `crates/rcfd-api/tests/validation_cavity_ghia.rs`, `cases/cavity/case.json`. Headline: F-028.
  - `backward-facing-step` - "Turbulent backward-facing step": expansion ratio 1.125 (Driver and Seegmiller, $Re_h$ about 36000), steady RANS with k-epsilon and k-omega SST, "the reattachment length against the measured value". Reference: $x_r/h = 6.26 \pm 0.10$ (from the NASA TMR backstep page, verified). Cases: four `crates/rcfd-turb/tests/step_*.rs` files and `cases/backward_step/case.json`. Headline: F-042, F-043.
  - `flat-plate` - "Turbulent flat plate skin friction": the zero-pressure-gradient plate of the NASA TMR (Re 5e6 per unit length) with Spalart-Allmaras, k-omega SST and k-epsilon, "the skin friction against the correlations and the TMR data". Reference: the power law $0.0576\,Re_x^{-0.2}$, White $0.455/\ln^2(0.06\,Re_x)$, the TMR CFL3D values $C_f(0.97)$ = 0.00268299 (SST) and 2.70565e-3 (SA), Karman-Schoenherr and the Coles law. Cases: six `crates/rcfd-turb/tests/` files and `cases/flat_plate/case.json`. Headline: F-041, F-043, F-203.
  - `heated-cavity` - "Differentially heated cavity": Boussinesq, Pr 0.71, Ra 1e3 to 1e6, "the average Nusselt number and the velocity maxima against the benchmark solution". Reference: de Vahl Davis (1983), Table V, "read as a page scan" ($Nu_{avg}$ 1.118, 2.243, 4.519, 8.800). Headline: F-050.
  - `shock-tube-and-wedge` - "Shock tube and wedge": the density-based solver on the Sod tube against the exact Riemann solution ($p^*$ 0.30313, $u^*$ 0.92745) and Mach 2 over a 10 degree wedge against the oblique-shock angle (weak $\beta$ 39.31 degrees); "Sod (1978) is recorded there as not obtained". Headline: F-060.
  - `packed-bed` - "Packed bed pressure drop": steady laminar air through a bed of 5 mm spheres (porosity 0.4) as a porous zone, "the pressure gradient against the Ergun equation" with the constants 150 and 1.75 ("checked only against a secondary source; Ergun (1952) recorded as not read"). Cases: `crates/rcfd-api/tests/sources.rs`, `cases/packed_bed/packed_bed.toml`. Headline: F-031.
- Added at 09647baa by ADR 0779 (F-875): `fan-structure` - "Structure and vibration of a fan rotor" (page `docs/structural-solver.md`): the rotor of the parametric 120 mm fan as an elastic solid (one blade under centrifugal and pressure load with its modes at rest, its Campbell table at speed, the bladed hub by cyclic symmetry against the full rotor, the response to blade-passing forcing, a polymer blade and the radiated power of the frame): "demonstrations checked against closed forms, identities and mesh change, not against a measured blade". Reference: "no measured or published datum of this blade, rotor or frame is recorded"; the comparisons are closed forms and identities, among them "the first bending frequency of the flat plate between the Euler-Bernoulli beam value 142.9 Hz and the wide-plate value 154.5 Hz" (derived in ADR 0500). Cases: `crates/rcfd-fem/tests/sb_fan_blade.rs`, `sb_fan_blade_builder.rs`. Headline: F-387, F-406, F-436.
- The memberships themselves are the `milestones` key of each item in `docs/roadmap.json` ([[doc-roadmap-json]]); the figures are in the generated block of [[doc-roadmap-overview]].

## Contradictions with the wiki

None found at the first ingest (2e94e5ef). At 09647baa [[simulation-milestones]] was brought up to the ten milestones in the same ingest.

## Pages this source informs

- [[simulation-milestones]], [[cylinder-re100]], [[naca0012]], [[fan-01]]
- [[lid-driven-cavity]], [[backward-facing-step]], [[flat-plate]], [[differentially-heated-cavity]], [[structural-solver]]
