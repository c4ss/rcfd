---
type: entity
tags: [fan, validation, rotating]
sources: [doc-glossary, doc-readme, doc-roadmap-overview]
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

## Related

- [[simulation-milestones]]
- [[mrf]]
- [[known-miss]]

## Open questions

- Measurement uncertainty and density of the FAN-01 data are not traced, so no full band can be registered ([[doc-roadmap-overview]]).
