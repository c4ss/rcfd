---
type: source
tags: [fan, fan-01, design, mrf, experiment]
sources: []
origin: docs/designs/exp-rot.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# EXP-ROT: how the FAN-01 rotor is modelled (design)

**Origin:** [docs/designs/exp-rot.md](../../docs/designs/exp-rot.md) at origin/main 2e94e5ef
**Kind:** repo design doc, dated 2026-11-04. "Status: Proposed. Design only. No code has been written and nothing has been run." Limits registered in ADR 0183. Roadmap F-276, milestone EXP-ROT (requires M-0).

## Summary

EXP-ROT is the experiment that decides how M-FAN-A1 models the FAN-01 rotor. Arm 1 rotates the ADR 0086 meshes rigidly and time-marches laminar PISO in the absolute frame, asking whether rigid rotation removes the spurious outlet angular momentum that the rotating-frame solve leaves in the empty passage. Arm 2 runs the bladed passage on the coupled solver with the rotation implicit (ADR 0085 arm C), asking whether the limiter limit cycle stays. Check H runs B2 time-accurately with SST to classify the hub trailing-edge pocket. The doc departs from its parent design: three small prerequisites (P-A, P-B, P-C) are needed and arm 2 needs a rebuild of about 1 000 to 1 800 lines; "The code ADR 0085 wrote for arm C was removed, was never committed, and has not been found on disk." Runs estimated at about 10.5 machine-hours.

## Key points

- Known state (ADR 0086 C-grid family): empty passages E3 / E2 / E1 with 5 040 / 13 882 / 38 688 cells, bladed B3 / B2 / B1 with 4 920 / 13 627 / 38 184. The exact outlet angular-momentum flux L of the empty passage is zero; the rotating-frame solve gives -0.0095, 0.0596 and 0.1060 N m, growing with refinement, and the passive duct gains 4.5 to 9.2 Pa of total pressure.
- The bladed steady iteration cycles at the blade-root leading edge on B3 and at the hub trailing edge (z 0.1716) on B2. ADR 0110 falsified SST, the seam gradient, the pressure-velocity coupling and the explicit Coriolis source as causes; jobs 49K to 49R (ADRs 0174, 0175, 0177 to 0181, "unmerged, not yet accepted by the maintainer") did not converge B2. ADR 0181: the B2 site is a near-stagnant, partly reversed corner pocket.
- Goals EG1 to EG5 with verdict classes (Q-ROT YES / NO / NOT REPRODUCED / NOT EVALUABLE; Q-POCKET STEADY / PHYSICAL / NUMERICAL / NOT EVALUABLE; Q-LC STAYS / GONE). EG5: every limit, prediction and stop rule is written in ADR 0183 before any code or run.
- Mapping (ADR 0183 D9): Q-ROT YES means A1 on rigid rotation, time-marched (F-277 moves into A1); NO means A1 stays rotating-frame; Q-LC STAYS with H STEADY or NUMERICAL means A1 starts from the M-FAN-B mesh.
- Arm 1 time step $\Omega\,\Delta t = 1^\circ$ (dt = 1.12156e-4 s, 360 steps per revolution); at most 15 revolutions. Check H: dt = T_c / 100 with T_c = 4.605 ms, STEADY threshold 1 % of $\Omega\, r_{hub}$ (0.193 m/s).
- Cost estimates: arm 1 about 7 h, check H about 2.5 h, arm 2 about 1.1 h.
- Non-goals include grading against the measured fan and "Any change of a default"; `baseline_hashes.rs` is not re-recorded.
- Seven open questions, each with a recommendation; "These are recommendations, not decisions."
- References recalled, not re-read (UNVERIFIED), including Celik et al. 2008 (as cited in ADR 0086).

## Contradictions with the wiki

None found.

## Pages this source informs

- [[fan-01]], [[mrf]], [[registration]], [[gci-richardson]], [[doc-designs-m-fan-b]]
