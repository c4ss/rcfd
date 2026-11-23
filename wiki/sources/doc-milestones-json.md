---
type: source
tags: [milestone, validation, data]
sources: []
origin: docs/milestones.json
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# milestones.json (simulation milestones)

**Origin:** [docs/milestones.json](../../docs/milestones.json) at origin/main 2e94e5ef
**Kind:** repo data file: the three simulation milestones, each with result statement, cases, reference and headline roadmap items.

## Summary

Machine-readable definition of three milestones: `vortex-shedding`, `fan` and `aerofoil-lift`. Each names the test files or cases that carry it, a reference string that points into [[doc-reference-data]] or [[doc-validation-data]], and the headline F-items.

## Key points

- `vortex-shedding` - "Vortex shedding behind a cylinder": "Laminar flow past a circular cylinder at Re 100 on a body-fitted O-grid: the Strouhal number and the mean drag coefficient against the published values." Reference: "St 0.164 to 0.167 and C_D 1.314 to 1.370 at Re 100 (arXiv:2303.09262 Table 1 and arXiv:1401.6513 Table 3, read as quotations) and Williamson, J. Fluid Mech. 206 (1989), eq. (6), St(100) = 0.164335". Cases: `crates/rcfd-fv/tests/cylinder.rs`, `validation_cylinder_re100*.rs` (three files), `w48t_cylinder_spokes.rs`. Headline: F-029, F-869.
- `fan` - "Assistant-driven fan design" (page `docs/milestone-fan.md`). Reference for the fan solve: the FAN 01 measured characteristic, Zenodo doi:10.5281/zenodo.20037409 (v5), file `characteristic_n1ug.mat`, "design point 1.4032 m^3/s, 126.46 Pa, 333.30 W" (the primary rig paper SAE 2016-01-1805 recorded as not read); "for the noise, no measured fan noise is recorded (docs/fan-project-breakdown.md, part D5: missing, needs data)". Cases: 11, from `fan01_validation.rs` to `f697_fan_objective.rs` and `cases/axial_fan_sector`. Headline: F-870, F-872, F-873, F-260, F-702, F-874.
- `aerofoil-lift` - "Aerofoil lift (NACA 0012)": "The NACA 0012 at Re 6e6 in incompressible steady RANS on an O-grid: the lift curve up to maximum lift against the measured lift." Reference: Ladson, NASA TM 4074 (1988), tripped, M 0.15, Re 6e6, as the TMR file `CLCD_Ladson_expdata.dat` ("the report itself is not recorded as read"). Cases: `cases/naca0012/case.json`, two AoA-sweep scripts and tests. Headline: F-043, F-868.

## Contradictions with the wiki

None found; [[simulation-milestones]] lists the same three milestones.

## Pages this source informs

- [[simulation-milestones]], [[cylinder-re100]], [[naca0012]], [[fan-01]]
