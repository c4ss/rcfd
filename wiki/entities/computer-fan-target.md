---
type: entity
tags: [fan, computer-fan, target, plan]
sources: [doc-fan-project-breakdown, doc-fan-measurement-protocol, doc-validation-data, doc-design-loop, doc-changelog, doc-milestone-fan]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Computer-fan target (small axial fan, e.g. 120 mm)

**What it is:** the project goal of a parametric computer fan that Claude optimises over time, extending the FAN-01 milestone to "a fan model that predicts real-world behaviour" ([[doc-fan-project-breakdown]]).
**Where:** plan [docs/fan-project-breakdown.md](../../docs/fan-project-breakdown.md); measurement protocol [docs/fan-measurement-protocol.md](../../docs/fan-measurement-protocol.md); library `rcfd_mesh::fan`, `rcfd_api::design`.

## Details

- Scope: "axial or centrifugal, about 40 to 140 mm, 1 000 to 5 000 rpm, tip speed about 5 to 25 m/s (Mach below 0.1), chord Reynolds number about 2e4 to 2e5 ... working against a high-impedance system (heat sink, filter, grille). This is not FAN-01" ([[doc-fan-project-breakdown]]).
- 120 mm appears as the worked example: the protocol's pressure scale is "about 190 Pa for 120 mm at 2 000 rpm (U_tip = 12.6 m/s)" and a 120 mm fan at 2 000 rpm needs "about 9 mN m of torque" ([[doc-fan-measurement-protocol]]); the breakdown lists "Which fan to target first (axial 120 mm, centrifugal blower, other)" among what the user could decide ([[doc-fan-project-breakdown]]).
- Outputs per variant and condition: flow and pressure rise (and the operating point against a system impedance), shaft power and efficiency, noise, vibration, and their spread under manufacturing inaccuracy and ageing ([[doc-fan-project-breakdown]]).
- Validation data: "no open source is adequate as the first computer-fan reference case", so G2 needs a fan measured by the user ([[doc-validation-data]]). The protocol assumes a 5 % tolerance, giving about 1.7 % per measurement, until a registered limit replaces it ([[doc-fan-measurement-protocol]]).
- In 0.1.0, the parts not needing a converged fan solve exist: parametric blade geometry and sector meshes, tolerance tools, a reduced-order fan model, rotor-dynamics checks and a design ledger ([[doc-changelog]]).
- The fan objective of the design loop runs on the reduced-order fan only: "Every number is a model proxy: no measured fan data exists" ([[doc-design-loop]]).
- Status of the plan parts (2026-10-26 with later edits): B2, C6, D2 done; G3 exists; H3 done (F-698); everything else partial or missing ([[doc-fan-project-breakdown]]).

## Related

[[fan-01]], [[design-loop]], [[design-ledger]], [[simulation-milestones]], [[verification-vs-validation]], [[fan-passage-mesh]]

## Open questions

- Which fan to target first, whether the user supplies a measured fan, and whether to start structural/FSI coupling (E5) ([[doc-fan-project-breakdown]]).
- The comparison tolerance is "not set yet" ([[doc-fan-measurement-protocol]]).
