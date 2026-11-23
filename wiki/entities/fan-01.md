---
type: entity
tags: [fan, validation, rotating]
sources: [doc-glossary, doc-readme, doc-roadmap-overview, doc-meshing, doc-validation-data, doc-milestone-fan, doc-reference-data-f870-fan01-design-point, doc-designs-exp-rot, doc-designs-m-fan-b, doc-changelog, doc-milestones-json, adr-thread-fan01-rotating-solve, adr-thread-sliding-mesh-rotor-stator, adr-thread-fan01-design-point, doc-example-projects, adr-thread-design-surrogate-and-fidelity, doc-reference-data-f878-fan01-lda-profiles, doc-reference-data-f883-fan01-wall-pressure]
created: 2026-11-22
updated: 2026-11-23
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
- **Wave ADRs 0065 to 0117 (added by p3-records-a):** geometry and the first passage meshes (ADRs 0065, 0068, 0072, 0079, 0083, 0087), the F-126 attempt (ADR 0075, every limit missed), the deficit diagnosis (ADRs 0078, 0082, 0085), the first solution verification (ADR 0086), the stall diagnosis (ADRs 0110, 0111, 0115: a limit cycle of the Barth-Jespersen-limited `linear_upwind`, "The bladed stall has no attributed mechanism"), and the geometry an MRF zone can take (ADRs 0109, 0112, 0113, 0116, 0117: "the blade blocks MRF whatever the hub and shroud are") ([[adr-thread-fan01-rotating-solve]]). The hub item is [[f-126]].
- Off-design point of ADR 0075 attempt 3 (Q = 1.8039 m^3/s): Delta p_ts -84.4 Pa against 63.9 Pa measured, torque 1.010 N m against 1.882 N m ([[adr-thread-fan01-rotating-solve]]).
- The reduced-order tier of ADR 0119 gives FAN-01 dp_s 45.7 Pa (-64 %) at the design point with 33 of 40 strips stalled, and "does not bound FAN-01 below 1.6 m^3/s" ([[adr-thread-design-surrogate-and-fidelity]]).
- The transient rotor-stator path exists from a case file only on bladeless cylindrical-interface annuli and sectors, laminar, Euler, no restart (F-303, F-711); no ADR of the moving-meshes list runs it on the FAN-01 passage ([[adr-thread-sliding-mesh-rotor-stator]]).
- On 2026-11-18 the maintainer held several solver default changes "until the fan path needs it" (F-721, F-724, F-364, F-365, F-775) ([[adr-thread-fan01-rotating-solve]]).

- **F-870 in the ADR (ADR 0762):** by its own rule the work is "stopped and reported" on the band, and the item is written partial. B1 gives 73.988 Pa, where a measurement uncertainty of 22.72 Pa would be needed to close the gap, and 326.50 W (torque 2.098164 N m). `crates/rcfd-api/tests/f870_fan01_design_point.rs` holds the state ([[adr-thread-fan01-design-point]]).
- The ranked gap puts the compared quantity first: a rotor-only passage is compared with an installation value. Next come the missing converged second-order solution and then the mesh gates. By the agent's arithmetic, "The miss of 52.5 Pa is 63 % of the 83.3 Pa that the rotor-only definition discards"; the lead notes that no run shows this ([[adr-thread-fan01-design-point]]).
- The maintainer's answers of 2026-11-22 entered four follow-ups. F-877: a person reads the thesis for the uncertainty and the density. F-878: the LDA profiles, ADR 0765. F-879: check H, ADR 0766. F-880: the outlet as on the rig, not issued ([[adr-thread-fan01-design-point]]). F-880 now waits on F-882 and F-883 (maintainer's answer to ADR 0765 Q2, 2026-11-22) ([[adr-thread-fan01-design-point]]). Both have reported at 09647baa (F-883 done, F-882 partial); whether F-880 is issued is question 1 of ADR 0768, open with the maintainer ([[adr-thread-fan01-design-point]]).
- **The measured LDA profiles (ADR 0765, F-878 partial, 2026-11-22):** the record (Zenodo v6, CC BY 4.0) holds two-component LDA at the design point on two radial lines, x = 20 and x = 90 mm from the upstream face of the hub, 26 radii from 133 to 246 mm; no LDA uncertainty is stated ([[doc-reference-data-f878-fan01-lda-profiles]]). On B1 at x = 90 mm $c_x$ and $c_t$ each miss their band at 12 of 26 radii (worst at the hub and the tip, where the solve has no through-flow outside $r/250\,\text{mm}$ = 0.97). The Euler head over the measured span is 226.86 Pa simulated against 220.02 Pa measured and the outflow kinetic energy 102.28 against 99.20 Pa, so "the simulated rotor does the work of the measured one and leaves the same kinetic energy behind"; the missing 52.47 Pa cannot be divided between rotor and installation by the registered rule (rotor share between -34.4 and +59.1 Pa) ([[adr-thread-fan01-design-point]]).
- **Check H, the hub pocket in time (ADR 0766, F-879 partial, 2026-11-22):** two time-accurate SST runs on B2 (dt 4.605e-5 s and half of it, 6.27 revolutions each) do not decide by the registered rule (the outer-iteration substitute met at 43.76 % of steps against 99 %). The site oscillation decays below 1e-4 m/s in both; the time-marched pressure rise on B2 (69.62 and 70.07 Pa) is 8 to 9 % above the SIMPLEC sample of the same mesh ([[adr-thread-fan01-design-point]]).
- **Maintainer's answers of 2026-11-22 (ADRs 0765, 0766):** the Euler head, the outflow kinetic energy and the $c_x$, $c_t$ profiles become compared quantities of F-870 beside the pressure rise and the shaft power; loss budget F-882 and wall-pressure reading F-883 entered; the outer-iteration record and a rerun of check H entered as F-885; the convergence rule of F-870 waits on F-885 ([[adr-thread-fan01-design-point]]).
- The FAN-01 version-6 record says that the bearing friction "was measured with the fan removed and subtracted" ([[adr-thread-fan01-design-point]]). See also [[adr-thread-fan01-rotating-solve]].
- A Studio example shows one FAN-01 passage in 3D (4 920-cell F3 mesh, first-order `upwind`, torque 1.40 N m, 35 % below the measured 2.142 N m): [[example-project-axial-fan]], "not a validated fan prediction" ([[doc-example-projects]]).
- **Wave 49 stall diagnosis** (ADRs 0174 to 0181, 2026-10-30 to 2026-10-31, [[adr-thread-fan01-rotating-solve]]): the Barth-Jespersen limit cycle on B3 is not the NACA 0012 trailing-edge mechanism (ADR 0174); it is driven by the convection deferred correction at two sites, the blade-root leading edge and the hub trailing edge (ADRs 0175, 0177, 0178; the hub site is "a near-stagnant corner pocket", ADR 0181). No automatic first-order fallback (ADR 0179), bounded second-order scheme (ADR 0180) or mesh change at the hub (ADR 0181) converged it; torque and Delta p_ts stay the ADR 0086 cycle means with a band "until a converged second-order state exists".
- **A measured static pressure behind the rotor (ADR 0769, F-883 done, 2026-11-22):** the record's casing transducers give a time-mean wall pressure against the chamber of 113.90 Pa at the LDA plane x = 90 mm, with no stated uncertainty; over the measured span, by the radial equilibrium of the LDA profiles, $\Pi_m$ = 93.96 Pa +- 7.50 Pa ([[doc-reference-data-f883-fan01-wall-pressure]]). With it the rotor's share of the missing 52.47 Pa is 32.26 Pa with a band of 29.19 Pa and the installation's 20.21 Pa: it still "cannot be told" which is larger by the rule of ADR 0765. The rig's own terms are now measured: rotor and inlet lose $L_m$ = 26.85 Pa where the solve loses 62.87 Pa, and 32.50 Pa is recovered behind the plane against 12.28 Pa in the solve ([[adr-thread-fan01-design-point]]). By the maintainer's answer ("Yes, add it") the casing pressure at x = 90 mm is a compared quantity of F-870 ([[adr-thread-fan01-design-point]]).
- **Where the simulated loss arises (ADR 0768, F-882 partial, 2026-11-22):** a rothalpy budget of the loss of total pressure up to x = 90 mm closes on B3, B2, B1 and a finer mesh B0c (106 692 cells). The passage interior carries most of it and most of its fall with the mesh (61.8, 45.2, 33.5, 30.4 Pa); the blade surfaces 20.1, 19.5, 19.5, 16.9 Pa; hub and casing 2.8 to 3.5 Pa; the tip gap 1.6 to 1.9 Pa. The sampled loss $L_s$ is 104.34, 78.75, 62.87, 56.78 Pa. The observed order over B2, B1, B0c is 2.786, outside the registered [1.5, 2.5], so the extrapolation is not valid and no loss at mesh convergence is shown (a known miss). The registered mesh B0 diverged at iteration 106; B0c keeps the first cell at $y^+$ = 30 and misses all three mesh gates ([[adr-thread-fan01-design-point]]).
- **Check H by ADR 0183's own rule (ADR 0767, F-885 partial, 2026-11-22):** with the new opt-in record of the velocity change of each outer iteration, no step of the window of either run meets the rule (0 % against 99 %), so the class is again NOT EVALUABLE and the run does not decide. The reason is not the fan: in every step the unrelaxed 5th outer iteration moves the velocity about 1 m/s away from the relaxed ones ($d_5/d_1$ 0.93 and 0.96), and a cavity shows the same pattern ([[adr-thread-fan01-design-point]], [[piso]]).

## Related

- [[simulation-milestones]]
- [[mrf]]
- [[known-miss]]
- [[computer-fan-target]], [[gci-richardson]], [[design-loop]], [[validation-data-pack]]
- [[doc-milestone-fan]], [[doc-designs-exp-rot]], [[doc-designs-m-fan-b]]
- [[adr-thread-fan01-rotating-solve]], [[sliding-mesh]]
- [[example-project-axial-fan]]

## Open questions

- Measurement uncertainty and density of the FAN-01 data are not traced, so no full band can be registered ([[doc-roadmap-overview]]).
- No band of F-870 uses a measurement uncertainty (ADR 0762 open question 1) ([[doc-reference-data-f870-fan01-design-point]]).
- The seven EXP-ROT open questions carry recommendations, which the doc says are "not decisions" ([[doc-designs-exp-rot]]).
- ~~The convergence rule of F-870 is decided after F-885 reports; whether the missing pressure rise is the rotor's or the installation's waits on F-882 and F-883 ([[adr-thread-fan01-design-point]]).~~ All three reported by 2026-11-23 and none decides: F-885 gives no class (ADR 0767), F-882's extrapolation is not valid (ADR 0768), and with the measured casing pressure of F-883 the rotor's share is 32.26 +- 29.19 Pa of 52.47 Pa (ADR 0769) ([[adr-thread-fan01-design-point]]).
- Open with the maintainer at 09647baa: whether F-880 is issued now (ADR 0768 Q1), the outer-iteration rule on a relaxed PIMPLE loop (ADR 0767 Q1) and the convergence rule of F-870 (ADR 0767 Q2) ([[adr-thread-fan01-design-point]]).
