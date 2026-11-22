---
type: source
tags: [fan, milestone, fan-01, validation]
sources: []
origin: docs/milestone-fan.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Milestone: assistant-driven fan design iteration

**Origin:** [docs/milestone-fan.md](../../docs/milestone-fan.md) at origin/main 2e94e5ef
**Kind:** repo doc: milestone plan with goal, decisions, limits, staged definition, wave order and a capability-chain table (dated 2026-10-18, with later rows appended).

## Summary

"Status: planned (set by the user on 2026-10-18). This document is the plan, not a claim of capability." Goal: "Claude, through the assistant tool layer, iterates on fan shape and size, sets up and runs the test cases, reads the results, and converges on designs that keep noise low while flow is high across a range of operating conditions (not at a single point)." The fan solve is not validated: the comparison with the measured reference fan (FAN-01) still misses and F-126 is not met. "Before any fan design iteration is trusted, the rotating-frame discretisation on bladed sectors needs a formulation that conserves angular momentum (for example the absolute-velocity form) or a demonstrated mesh/scheme convergence of the torque."

## Key points

- Open decisions for the user: structural/FSI coupling for the material dimension; which published fan case to target if several exist.
- Structural solver "started (2026-11-12)": maintainer approved ADR 0270 (basic), ADR 0290 (intermediate), ADR 0310 (advanced), crate `rcfd-fem`, items F-370 to F-436.
- Limits: broadband noise needs scale-resolving transient flow (steady RANS gives proxies only); the CFD solver does not see blade material; FAN-01 is "the only open low-speed case found whose blade the generator can represent", and its measured pressure rise is an installation value (bellmouth, finite hub, struts, diffuser).
- ADR 0078 (wave 35C): the rotor is underloaded "mainly because the steady relative-frame solver with first-order upwind manufactures swirl that the walls do not supply" (angular-momentum balance open by 168 % of the wall torque). Minmod raises torque by 27 %, in-plane refinement by 37 %, both by 53 %, "which leaves the torque 5.9 % below the measured shaft value and Delta p_ts at 76 Pa against 126 Pa"; a diagnostic, nothing claimed.
- ADR 0075 first comparison: rotor-only sector MISSES (torque -38 % at the design point, total-to-static rise negative against 126 Pa measured).
- ADR 0086 (wave 37C), first solution-verification study on three C-grid meshes (4 920 / 13 627 / 38 184 cells, ratio 1.41): error bands "NOT shown credible" because the empty-passage check failed (spurious outlet angular momentum grows with refinement to 0.106 N m). Torque 1.850 / 1.967 / 2.038 N m, observed order 1.51, band +-0.217 N m (+-10.6 %; Fs 1.25, order capped at 1); measured 2.142 N m inside; "The Richardson value 2.143 N m comes within 0.05 % of it by coincidence inside a +-10.6 % band; this is not agreement." Delta p_ts 46.8 / 64.2 / 74.0 Pa, order 1.74, band [44, 104] Pa; measured 126.5 Pa outside. See [[gci-richardson]].
- Later rows record ADR 0112 (rotor-stator cylindrical interface), ADR 0113 (MRF with cyclic seams) and the bladed limit cycle; the follow-up experiment is [[doc-designs-exp-rot]].
- Staged definition: 1 foundation; 2 design loop on a simple part (duct or heat-sink pressure-drop minimisation); 3 fan (parametric axial fan, steady MRF/sector solve per operating point, characteristic curve, noise estimate, robust multi-point objective).
- Wave order 32 (geometry, aeroacoustics, design-loop infrastructure), 33 (planned MRF fan solve), 34 (planned noise and robust objective).

## Contradictions with the wiki

> [!warning] Contradiction
> The capability table (2026-10-18) and the summary list "the operating map (F-254 candidate), the optimiser (F-095 partial, F-257 candidate) and the design-loop tools" as none. [[doc-design-loop]] documents operating maps, DOE, Nelder-Mead, differential evolution, NSGA-II, CMA-ES and the `design_*` MCP tools (from wave 32C, ADR 0067 on), and [[doc-fan-project-breakdown]] H2 says these optimisers exist. The "F-254" and "F-257" ids are parity-matrix proposals ([[doc-parity-matrix]]), not the real F-254 (particle wall interaction). Within the docs (stale table).

## Pages this source informs

- [[fan-01]], [[simulation-milestones]], [[computer-fan-target]], [[design-loop]], [[gci-richardson]], [[mrf]]
