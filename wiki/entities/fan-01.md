---
type: entity
tags: [fan, validation, rotating]
sources: [doc-glossary, doc-readme, doc-roadmap-overview, doc-meshing, doc-validation-data, doc-milestone-fan, doc-reference-data-f870-fan01-design-point, doc-designs-exp-rot, doc-designs-m-fan-b, doc-changelog, doc-milestones-json, adr-thread-fan01-rotating-solve, adr-thread-sliding-mesh-rotor-stator]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# FAN-01 (reference fan case)

**What it is:** the reference fan case, a 500 mm low-pressure axial fan from the FAU / EAA open data set (CAD and measured characteristic, CC BY 4.0) ([[doc-glossary]]).
**Where:** `crates/rcfd-api/tests/fan01_*.rs`, `cases/axial_fan_sector`; detail in `docs/milestone-fan.md` (P1).

## Details

- Its steady rotating-frame ([[mrf]]) solve does not reach the measured operating point; several suspected causes were each ruled out as the single cause; diagnosis open ([[doc-roadmap-overview]]).
- Reference: Zenodo doi:10.5281/zenodo.20037409 (v5), `characteristic_n1ug.mat`, design point 1.4032 m^3/s, 126.46 Pa, 333.30 W; the primary rig paper SAE 2016-01-1805 is recorded as not read ([[doc-roadmap-overview]]).
- F-870 (design point) partial: pressure rise 73.99 Pa vs 126.46 Pa (band 29.75 Pa; rotor-only passage vs installation value); mesh gates of B1 missed (83.37 deg, skewness 3.765); limit cycle, res_u 1.2e-3; shaft power within 2.0 % ([[doc-roadmap-overview]]).
- Option on the table: close the diagnosis or choose a different reference case for the rotating machinery path ([[doc-roadmap-overview]]).
- **Passage mesh:** at the FAN-01 hub (chord 99 mm, stagger 44 deg, pitch 87 mm) the blade spans 68 mm of the pitch, so the `pursuit` and `smoothed` C-grid constructions refuse it; `camber_seam` (ADR 0079) and `camber_seam_h` (ADR 0083) build it (11 536 cells) at 83.26-83.3 deg / 3.98 / 227 against the 70 / 2.0 / 300 target (missed). The laminar smoke solve on the H mesh oscillates without converging (continuity up to 7.46e-3), away from the poor faces ([[doc-meshing]]). See [[fan-passage-mesh]].

### Data, diagnosis and plans (P1 evidence and fan ingest)

- Geometry and data (wave 34D survey, ADR 0075): 9 blades, tip radius 247.5 mm, duct 250 mm (gap 2.5 mm), hub 124 mm, "NACA 4510" sections, chord 99 to 55 mm, stagger 43.6 to 69.6 deg; 1486 rpm, design 1.4 m^3/s, 20 measured points 0.20 to 2.08 m^3/s, stall about 1.1 m^3/s. Zenodo v6 (10.5281/zenodo.23943494, 2026-11-04) changes no value ([[doc-validation-data]]).
- F-870 design point as read on 2026-11-22: 1.40324 m^3/s, 126.457 Pa total-to-static, 333.301 W shaft power, efficiency 0.532 (used as 1.4032 / 126.46 / 333.30); speed 1486 per minute (1486.08 from the rotor trigger); bearing friction measured and subtracted ([[doc-reference-data-f870-fan01-design-point]]).
- The measurement uncertainty is "not stated in any of" the sources read. The Kromer thesis could not be fetched. The round-robin scatter of +-3 Pa and 1.5 % of flow is "**not** used as the uncertainty". Air density is "not stated" and the tests assume 1.2 kg/m^3. The numerical half of the band is 0.21190 N m (32.98 W) and 29.75 Pa ([[doc-reference-data-f870-fan01-design-point]]). See [[gci-richardson]].
- Diagnosis history:
  - ADR 0075: rotor-only torque -38 % at the design point.
  - ADR 0078: the rotor is underloaded "mainly because the steady relative-frame solver with first-order upwind manufactures swirl that the walls do not supply". Minmod plus refinement left the torque 5.9 % below measured and Delta p_ts at 76 Pa.
  - ADR 0086: GCI bands "NOT shown credible", and the Richardson torque matching the measured one is "not agreement".

  Sources: [[doc-milestone-fan]].
- The bladed steady iteration cycles at the hub trailing edge (z 0.1716) on mesh B2. Jobs 49K to 49R did not converge it. EXP-ROT (F-276, ADR 0183, proposed, nothing run) is the experiment that decides the rotor model of M-FAN-A1, rigid rotation or rotating frame ([[doc-designs-exp-rot]]).
- The mesh in use "is a NACA-section fit to the published CAD, not the CAD itself". M-FAN-B (ADR 0184, proposed) plans OBJ/glTF import so the real blade can be meshed ([[doc-designs-m-fan-b]]).
- Release status: "The FAN-01 reference fan does not reach its operating point ... the cause has not been found" ([[doc-changelog]]). It is the reference of the `fan` milestone ([[doc-milestones-json]]).
- FAN-01 is not the computer-fan target, which has different physics ([[computer-fan-target]]).

### Rotating-solve thread (P3, moving meshes)

- The ADR chain of the rotating solve is collected on [[adr-thread-fan01-rotating-solve]] (F-126 partial; the wave ADRs are added by the records job).
- The transient rotor-stator path exists from a case file only on bladeless cylindrical-interface annuli and sectors, laminar, Euler, no restart (F-303, F-711); no ADR of the moving-meshes list runs it on the FAN-01 passage ([[adr-thread-sliding-mesh-rotor-stator]]).
- On 2026-11-18 the maintainer held several solver default changes "until the fan path needs it" (F-721, F-724, F-364, F-365, F-775) ([[adr-thread-fan01-rotating-solve]]).

## Related

- [[simulation-milestones]]
- [[mrf]]
- [[known-miss]]
- [[computer-fan-target]], [[gci-richardson]], [[design-loop]], [[validation-data-pack]]
- [[doc-milestone-fan]], [[doc-designs-exp-rot]], [[doc-designs-m-fan-b]]
- [[adr-thread-fan01-rotating-solve]], [[sliding-mesh]]

## Open questions

- Measurement uncertainty and density of the FAN-01 data are not traced, so no full band can be registered ([[doc-roadmap-overview]]).
- No band of F-870 uses a measurement uncertainty (ADR 0762 open question 1) ([[doc-reference-data-f870-fan01-design-point]]).
- The seven EXP-ROT open questions carry recommendations, which the doc says are "not decisions" ([[doc-designs-exp-rot]]).
