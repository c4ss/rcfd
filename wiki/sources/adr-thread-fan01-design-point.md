---
type: source
tags: [adr-thread, fan, fan-01, validation, milestone]
sources: [doc-reference-data-f870-fan01-design-point, doc-milestone-fan, doc-validation-data, doc-milestones-json]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: FAN-01 at its design point (F-870)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread, 1 registration with a result addendum, the lead's reading and the maintainer's answers (2026-11-22), framed by ADRs 0756 and 0757.

F-870 is the headline item of the `fan` milestone that asks for the FAN-01 pressure rise and shaft power at the design point within a band built from "the measurement's stated uncertainty and a mesh-convergence estimate". It was created by the maintainer's answer to ADR 0756 question 3 (option (b), 2026-11-21) and made part of the whole fan design goal by ADR 0757. ADR 0762 is the one registration so far: it tabulates the state of every earlier FAN-01 diagnosis (ADRs 0075 to 0183), registers the band, runs mesh B1 once and ranks the gap. The rotating-frame solve and its stall diagnoses themselves are the subject of [[adr-thread-fan01-rotating-solve]] (job p3-moving-meshes), if that page exists; this page holds the design-point ADR.

## Current state

- **F-870 partial** (ADR 0762, the lead's reading). By the ADR's own rule the work is "stopped and reported" on the band: "the stated uncertainty of the FAN-01 measurement is in no source that could be read". The run, the test that holds the state and the ranked gap were delivered all the same.
- **Result on B1** (38 184 cells, 8000 iterations, 49.9 minutes): total-to-static pressure rise 73.988 Pa against 126.46 Pa (-41.5 %, a miss against the numerical band 29.75 Pa; "a measurement uncertainty of 22.72 Pa (18 % of the measured value) would be needed to close it"); shaft power 326.50 W (torque 2.098164 N m) against 333.30 W (-2.0 %, inside the numerical part 32.98 W, "not a pass of the clause"). Mesh gates missed (83.37 deg against 70, skewness 3.765 against 2.0; aspect 252.4 against 300 met). Convergence missed (`res_u` 1.2e-3 against 1e-5; a limit-cycle sample by ADR 0110).
- **State held by a test:** `crates/rcfd-api/tests/f870_fan01_design_point.rs`, three gate tests (a B3 short run on the heavy record's path, 1 % either way) and a heavy record of B1; five mutations caught. Before this, "No test on main asserts a design-point pressure rise or torque of the second-order set-up" (ADR 0762, task 1).
- **Maintainer's answers (2026-11-22, "Yes, all seven" across ADRs 0760 to 0762):** Q1 and Q2 option (a): a person reads Kromer's thesis and the SAE paper for the uncertainty and the density, item F-877, "not issued to an agent: the site of the thesis refuses automated access". Q3: four follow-ups entered in order F-877, F-878 (the measured LDA profiles behind the rotor against the solve, issued as ADR 0765), F-879 (check H of ADR 0183, issued as ADR 0766), F-880 (the outlet as on the rig, entered and not issued; issued "only if F-878 shows that the missing pressure rise is the installation's"). Q4: neither the compared quantity nor the convergence rule is decided now.
- ADRs 0765 and 0766 are not in this snapshot.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0762](../../docs/adr/0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md) | 2026-11-22 | registration (hub: state table of ADRs 0075 to 0183) | F-870 | FAN-01 pressure rise and shaft power at 1.4032 m^3/s against the measured 126.46 Pa and 333.30 W | stopped on the band, proposed and written partial; A1, A3a, A3b, A4 missed; A2 inside the numerical part | follow-ups F-877 to F-880 (ADRs 0765, 0766 issued) |

Framing ADRs (on [[adr-thread-simulation-milestones-validation]]): [ADR 0756](../../docs/adr/0756-f865-registration-per-cent-complete-for-the-roadmap-and-for-each-simulation-milestone.md) question 3 (an item whose acceptance is the FAN-01 design point, answered (b)); [ADR 0757](../../docs/adr/0757-f867-registration-the-fan-milestone-as-the-whole-design-goal.md) (F-870 heads the fan milestone with F-872, F-873, F-260, F-702 and F-874).

## Key numbers and limits

- **Measurement** (ADR 0762): row 13 of `w34d_fan01_characteristic_n1ug.csv`, 1.40324 m^3/s, 126.45658 Pa, 333.30090 W; speed 1486 rpm; shaft torque compared 2.14184 N m. The v6 record (doi:10.5281/zenodo.23943494) says "The bearing friction was measured with the fan removed and subtracted", which removes bearing loss from the list of unknowns of ADRs 0075, 0078 and 0086.
- **Band form:** met iff $\lvert f_{B1} - f_{measured}\rvert \le U_{num} + u_{meas}$, the two added, not combined in quadrature. $U_{num} = 1.25\,\lvert f_2 - f_1\rvert / (r_{21}^{p_{band}} - 1)$, $p_{band} = \min(p, 1)$, $r_{21} = (38184 / 13627)^{1/3} = 1.40981$. Torque triplet 2.098164 / 2.028692 / 1.918189 N m gives p 1.3928 and U_num 0.21190 N m (32.98 W); pressure triplet 73.988 / 64.233 / 46.778 Pa gives p 1.7378 and U_num 29.75 Pa. $u_{meas}$ "not registered". See [[gci-richardson]].
- **Mesh gates** (from `docs/meshing.md`, set-up A, and the gap of F-133): non-orthogonality <= 70 deg, skewness <= 2.0, aspect ratio <= 300, by `rcfd_mesh::quality::check_mesh`.
- **Convergence rule:** `Simulation::is_converged()` within 8000 iterations with `tolerance_u = tolerance_p = 1e-5`; "The cap rule of ADR 0075 and ADR 0086 is reported beside it and does **not** count as converged for F-870".
- **Best recorded values before the run** (task 1): torque B1 2.098164 N m effective (326.50 W, 2.0 % below), pressure 73.99 Pa; best pressure ever, arm KN of ADR 0078, 76.2 Pa on the H-slit with minmod (molecular torque 2.0163 N m). The only converged bladed state above first order is ADR 0178's B3 (1.9144 N m, 45.75 Pa) with first order in 322 hand-picked hub cells.
- **Shape of the miss** (the agent's inference, "no run of this work shows it", per the lead): the rotor raises total pressure by 157.25 Pa on B1 (ADR 0086), total-to-total efficiency 0.68; 157.25 - 73.99 = 83.3 Pa leaves as velocity (53.9 Pa axial dynamic pressure, 23.5 Pa swirl); the miss of 52.5 Pa is 63 % of it. ADR 0086's extrapolated pressure rise 85.9 Pa is still 40.5 Pa (32 %) below the measurement.
- **Ranked gap** (ADR 0762, task 4): 1 the compared quantity (rotor-only passage against an installation value; follow-ups LDA comparison, then outlet model); 2 no converged second-order solution (check H of ADR 0183 first); 3 the mesh gates (F-133); 4 resolution and credibility of the estimate; 5 the rotating-frame discretisation (0.106 N m of spurious outlet angular momentum in the empty passage, F-126); 6 the unknowns of the measurement; 7 geometry and model (CAD chords 4 to 5 % shorter, no root fillet, fully turbulent SST at chord Re 1.25e5 to 1.5e5).

## Open questions

- ADR 0762 Q4 (deferred by the maintainer on 2026-11-22): whether the headline compares the installation value (needing the outlet model of F-880) or "becomes a rotor quantity", and whether "a rule on a time mean" is acceptable if check H classes the hub pocket as physical. "Both are put again when F-878 and F-879 have reported."
- F-877 waits for a person: the stated uncertainty and the air density are in no source read; the tests assume 1.2 kg/m^3, and one per cent of density is 0.74 Pa and 3.3 W.
- ADR 0762 records that ADR 0183 names an item F-276 "which is not in `docs/roadmap.json`", and ADR 0757 says of the stall diagnoses "no item has them as its subject"; F-879 now carries check H.

## Related

- [[fan-01]], [[simulation-milestones]], [[mrf]], [[gci-richardson]], [[fan-passage-mesh]], [[known-miss]], [[computer-fan-target]]
- [[adr-thread-fan01-rotating-solve]], [[adr-thread-simulation-milestones-validation]]
- [[roadmap-quality-docs-and-validation]]
