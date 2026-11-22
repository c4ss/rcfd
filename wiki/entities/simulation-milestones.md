---
type: entity
tags: [roadmap, validation]
sources: [doc-roadmap-overview]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Simulation milestones

**What it is:** named simulations with a stated result; the per cent counts landed prerequisite items, and only the 'headline result' says whether the result agrees ([[doc-roadmap-overview]]).
**Where:** defined in `docs/milestones.json`; items name their milestones in `docs/roadmap.json`.

## Milestones (2026-11-18)

- **Vortex shedding behind a cylinder** — Re 100, O-grid, Strouhal and mean drag vs St 0.164-0.167, C_D 1.314-1.370: 19 items, 89%, headline not met. F-869 partial: fine-mesh C_D 1.33900 in band, but observed order 1.387 outside [1.5, 2.5] (known miss W3b, ADR 0763) ([[doc-roadmap-overview]]).
- **Assistant-driven fan design** — Claude iterating fan shape via the tool layer: 92 items, 70%, not met. Headlines F-870 (partial), F-872, F-873, F-702, F-874 (not started), F-260 (partial). No measured fan noise is recorded. See [[fan-01]] ([[doc-roadmap-overview]]).
- **Aerofoil lift (NACA 0012)** — Re 6e6 steady RANS vs Ladson (NASA TM 4074): 22 items, 89%, not met. F-868 met 0-16 deg and max lift 1.542; 17 deg does not converge with steady SA (known miss); F-043 (SST) partial ([[doc-roadmap-overview]]).

## Related

- [[fan-01]]
- [[known-miss]]
- [[benchmark-descriptor]]
- [[roadmap]]

## Open questions

- ADR 0763 Q1: does the cylinder order miss hold F-869? ADR 0760 Q1: does the NACA headline end at 16 deg? Both are the maintainer's ([[doc-roadmap-overview]]).
