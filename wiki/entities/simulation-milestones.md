---
type: entity
tags: [roadmap, validation]
sources: [doc-roadmap-overview, doc-milestones-json, doc-reference-data-f868-ladson-naca0012, doc-reference-data-f869-cylinder-mean-drag, doc-reference-data-f870-fan01-design-point, adr-thread-fan01-rotating-solve, adr-thread-simulation-milestones-validation, adr-thread-fan01-design-point]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Simulation milestones

**What it is:** named simulations with a stated result; the per cent counts landed prerequisite items, and only the 'headline result' says whether the result agrees ([[doc-roadmap-overview]]).
**Where:** defined in `docs/milestones.json`; items name their milestones in `docs/roadmap.json`.

## Milestones (2026-11-18)

- **Vortex shedding behind a cylinder** — Re 100, O-grid, Strouhal and mean drag vs St 0.164-0.167, C_D 1.314-1.370: ~~19 items, 89%, headline not met. F-869 partial: fine-mesh C_D 1.33900 in band, but observed order 1.387 outside [1.5, 2.5] (known miss W3b, ADR 0763) ([[doc-roadmap-overview]]).~~ At 5b96a1f8: 20 items, 16 done, 3 partly, 1 not started, 88 %, **headline met**; F-869 done by the maintainer's answer (c) to ADR 0763 Q1, the order miss kept as a known miss, F-881 entered ([[adr-thread-simulation-milestones-validation]]).
- **Assistant-driven fan design** — Claude iterating fan shape via the tool layer: 92 items, 70%, not met. Headlines F-870 (partial), F-872, F-873, F-702, F-874 (not started), F-260 (partial). No measured fan noise is recorded. See [[fan-01]] ([[doc-roadmap-overview]]).
- **Aerofoil lift (NACA 0012)** — Re 6e6 steady RANS vs Ladson (NASA TM 4074): ~~22 items, 89%, not met. F-868 met 0-16 deg and max lift 1.542; 17 deg does not converge with steady SA (known miss); F-043 (SST) partial ([[doc-roadmap-overview]]).~~ At 5b96a1f8: 23 items, 18 done, 4 partly, 1 not started, 87 %, not met; F-868 done (17 degrees by a time-accurate arm, $c_l$ 1.5516 against 1.6057, ADR 0764), F-043 still partly met; F-884 entered for the steady route at 17 degrees ([[adr-thread-simulation-milestones-validation]]).

## Definitions and reference data (from milestones.json and reference-data)

- `docs/milestones.json` defines the ids `vortex-shedding`, `fan` and `aerofoil-lift`. For each it gives the result statement, the case files, a reference string and the headline items: F-029 and F-869; F-870, F-872, F-873, F-260, F-702 and F-874; F-043 and F-868 ([[doc-milestones-json]]).
- Cylinder: the drag band [1.314, 1.370] (width 0.056, half-width 2.09 % of the middle 1.342) was registered in ADR 0761 before any run. "All are computations, none is a measurement" ([[doc-reference-data-f869-cylinder-mean-drag]]). See [[cylinder-re100]].
- NACA 0012: the Ladson 120-grit maximum is (17.24, 1.6347) and the 180-grit maximum (17.13, 1.6219). The scatter taken across the three grit zones is 0.018 (ADR 0760) ([[doc-reference-data-f868-ladson-naca0012]]). See [[naca0012]].
- Fan: the design point is 1.4032 m^3/s, 126.46 Pa, 333.30 W. The band's numerical half is 29.75 Pa, and the measurement uncertainty is not stated in any source read ([[doc-reference-data-f870-fan01-design-point]]). See [[fan-01]].
- Fan milestone, moving-meshes side: F-126 (rotating interfaces) and F-218 (actuator disk) carry the `fan` milestone in [[roadmap-moving-meshes-rotors-and-coupling]], both partial; the thread is [[adr-thread-fan01-rotating-solve]].

## From the ADR threads (P3)

- The milestone figures were built by F-865 (ADR 0756, done). At the merge the three per cents were 92, 89 and 90. The lead wrote that they are high because members are "what its run of record uses", so "The column 'Headline result' is the one to read" ([[adr-thread-simulation-milestones-validation]]).
- F-868, F-869 and F-870 exist because of the maintainer's answers of 2026-11-21 to ADR 0756 questions 1 to 3 (option (b) each). With F-869 entered, the vortex-shedding headline "reads 'partly met' from this day, where it read 'met' on the Strouhal number alone". The first six further milestones are approved as F-871 (todo) ([[adr-thread-simulation-milestones-validation]]).
- F-867 (ADR 0757) made the fan milestone the whole design goal: 89 members, 49 done, 30 partly done, 10 not started, 72 per cent at the merge. Structure and vibration get a milestone of their own (F-875, todo) ([[adr-thread-simulation-milestones-validation]]).
- F-870 is detailed on [[adr-thread-fan01-design-point]]. Its follow-ups F-877 to F-880 were entered on 2026-11-22 ([[adr-thread-fan01-design-point]]). F-878 (ADR 0765) and F-879 (ADR 0766) reported partial the same day; F-882, F-883 and F-885 followed from the maintainer's answers, and F-870 now also compares the rotor quantities the LDA record measures ([[adr-thread-fan01-design-point]]).

## Related

- [[cylinder-re100]], [[naca0012]], [[computer-fan-target]], [[doc-milestones-json]]

- [[fan-01]]
- [[known-miss]]
- [[benchmark-descriptor]]
- [[roadmap]]

## Open questions

- ~~ADR 0763 Q1: does the cylinder order miss hold F-869? ADR 0760 Q1: does the NACA headline end at 16 deg? Both are the maintainer's ([[doc-roadmap-overview]]).~~ Both answered on 2026-11-22 ([[adr-thread-simulation-milestones-validation]]).
- ~~ADR 0760 Q1: does the NACA headline end at 16 deg?~~ The maintainer answered on 2026-11-22 with option (a): F-868 stays partial and an unsteady arm at 17 degrees is registered under F-868, issued as ADR 0764 ([[adr-thread-simulation-milestones-validation]]). ~~ADR 0763 Q1 is still open; the agent and the lead recommend (c) ([[adr-thread-simulation-milestones-validation]]).~~ Answered (c): F-869 done, F-881 entered; ADR 0764 answered (a): F-868 done, F-884 entered ([[adr-thread-simulation-milestones-validation]]).
