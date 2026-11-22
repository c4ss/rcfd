---
type: source
tags: [fan, plan, computer-fan]
sources: []
origin: docs/fan-project-breakdown.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Computer-fan project: breakdown into parts

**Origin:** [docs/fan-project-breakdown.md](../../docs/fan-project-breakdown.md) at origin/main 2e94e5ef
**Kind:** repo doc: plan (2026-10-26) written at the user's request. "This is a plan, not a claim of capability."

## Summary

Splits the computer-fan goal into parts A to H with a status each, extending [[doc-milestone-fan]] from "a fan the solver can run" to "a fan model that predicts real-world behaviour and that Claude optimises over time for the computer-fan use case". The target is a parametric fan that returns, per variant and operating condition, flow and pressure rise (including the operating point against a system impedance such as a heat sink), shaft power and efficiency, noise, vibration, and the spread of all of these under manufacturing inaccuracy and ageing, plus a loop in which Claude keeps "a durable record so that the design improves over weeks, not over one session".

## Key points

- Scope: "axial or centrifugal, about 40 to 140 mm, 1 000 to 5 000 rpm, tip speed about 5 to 25 m/s (Mach below 0.1), chord Reynolds number about 2e4 to 2e5 (transitional, laminar separation and tip-leakage dominated), working against a high-impedance system (heat sink, filter, grille). This is not FAN-01".
- Five limits, stated first: prediction needs calibration data; broadband noise needs scale-resolving transient flow; vibration needs forces plus a rotor-dynamics/structural model; dust build-up is stochastic and slow (proxy only); "The CFD route is not yet reliable" (FAN-01 stall open after waves 43 to 45).
- As of 2026-10-26: B2 (exact-sector generator, ADR 0116), C6 (convergence and failure reporting, ADR 0067, 0077) and D2 (FW-H, as post-processing) done; G3 (registered limits, misses recorded) exists; all else partial or missing.
- Selected parts: A1 partial (`rcfd_mesh::fan`, ADR 0065); B3 partial (cylindrical interface ADR 0112; sliding AMI F-250 library only, ADR 0216); C1 partial (FAN-01 stall open); C2 transient sliding mesh missing; C7 reduced-order tier partial; D5 noise validation "missing (needs data)"; E5 partial (one blade as a clamped solid, F-387); F2 dust model missing (F-254 particle wall interaction "todo" at the time).
- G. Validation data: G1 survey "missing", G2 reference computer fan "missing", G3 "exists".
- H. Optimisation and the assistant loop: H1 partial (F-697, ADR 0620, reduced-order tier only); H2 partial (F-095, F-249, F-260; NSGA-II ADR 0209/0220 and CMA-ES ADR 0222 exist, "a study on the real fan is todo"); H3 done (F-698, ADR 0621, library only); H4 design ledger partial (F-268; ADR 0121, 0240, 0623; F-700); H5 assistant tools partial; H6 long-horizon hypotheses partial (F-701, ADR 0624; no MCP tool until F-702); H7 fidelity management partial (F-699, ADR 0622; synthetic high sources only).
- Order of work: 1 foundations needing no converged CFD fan (wave 46); 2 close the CFD route; 3 noise and vibration from CFD, then H3/H7; 4 H6 long-horizon loop.
- What the user could add: measured data of one real computer fan and its geometry; "Which fan to target first (axial 120 mm, centrifugal blower, other)"; whether to start structural/FSI coupling (E5).

## Contradictions with the wiki

> [!warning] Contradiction
> G1 (small-fan data survey) is marked "missing", but [[doc-validation-data]] records the survey as "Searched and read on 2026-10-26 (wave 46E, part G1 ...)". The status line also says the H rows were updated later (H3 done, ADR 0621) while the "As of the plan's date" sentence still lists only B2, C6, D2 as done. Within the docs.

## Pages this source informs

- [[computer-fan-target]], [[fan-01]], [[design-loop]], [[design-ledger]], [[doc-fan-measurement-protocol]]
