---
type: source
tags: [adr-thread, turbulence, sst, spalart-allmaras, k-epsilon, les, des, rsm, wall-functions, flat-plate, channel]
sources: [doc-reference-data-sst-wall, doc-reference-data-turbulence-finishing, doc-reference-data-spalart-allmaras]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the turbulence models of the early waves and the SST near-wall diagnosis

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with results, diagnoses), 9 ADRs, from the pre-wave ADR 0014 to wave 34C.

The first half adds models: Spalart-Allmaras and the turbulence-model interface (ADR 0014), the k-epsilon family on the backward-facing step, SST and BSL and a near-wall catalogue (ADR 0015), the Realizable constant A0 (ADR 0017), Smagorinsky and WALE subgrid models and a mixing-length closure (ADR 0026), a full viscous stress option, SA-based DES / DDES and a Reynolds stress model (ADR 0032). The second half is a chain of diagnoses of the wall-resolved SST flat plate after the wave 30C validation (ADR 0061): ADR 0063 finds no coding defect, ADR 0070 audits `sst.rs` term by term against NASA TMR and finds none, and ADR 0074 shows that the low observed order is "a property of rcfd's own mesh family/set-up, not of the SST implementation". See [[k-omega-sst]], [[flat-plate]], [[backward-facing-step]].

## Current state

- **Models in `rcfd-turb`:** SA-noft2 with a plugin registry and coefficients in the settings tree, and runtime model switching (ADR 0014); k-epsilon Standard, RNG, Realizable, SST, BSL, and wall treatments Scalable, roughness, Standard, Enhanced (ADR 0015); Smagorinsky, WALE and mixing length (ADR 0026); `full_viscous_stress`, DES / DDES and RSM (ssg, lrr_ip) for the transient algorithms only (ADR 0032). See [[rcfd-turb]].
- **F-041, F-042, F-043, F-203, F-045, F-206, F-046, F-207 stay partial** in these ADRs. F-041: "S2 misses at x = 1.0 (+5.29 % against 5 %)". F-042: on the corrected step BFS-C "only RNG meets the registered reattachment window, and that pass is marginal" (ADR 0015); the default A0 stays 4.04 ("the decision to change it is left to the user", ADR 0017).
- **The SST flat-plate deficit is explained, not fixed.** ADR 0070: "finds no defect in the SST implementation and changes no solver behaviour. The published model explains the slope and the Cf deficit, with TMR data as evidence." ADR 0074: on the NASA TMR nested grids (H6c) "rcfd converges with order 1.07 (TMR codes about 1.0 to 1.3)", but "rcfd lies 0.82 to 0.87 percent below the mean of the three incompressible codes on every grid"; that offset "stay[s] open".
- **k-epsilon wall-function mesh dependence** in the Re_tau 178 channel is "the validity limit of the log-law function with a viscous fall-back branch and a positive feedback of y* on k"; the equilibrium forms tried as an opt-in missed E1 and E2 "and were removed again" (ADR 0063).
- **Side finding that moved to numerics:** "a `Symmetry` top acts like a no-slip wall in a body-forced half channel" (ADR 0070); ADR 0074 traced it to "the averaged momentum diagonal of ADR 0010, not the `Symmetry` condition" and fixed one wiring defect (the case key never reached the transient kernel). See [[component-diagonal]].

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0014](../../docs/adr/0014-sa-and-turbulence-interface.md) | | registration + results | F-041, F-044 | Spalart-Allmaras and the turbulence-model interface | F-041 partial (S2 +5.29 % against 5 %); S1 +2.97 % against the TMR 2.70565e-3 | |
| [0015](../../docs/adr/0015-turbulence-finishing.md) | | registration + results | F-042, F-043, F-203 | k-epsilon on the BFS, SST / BSL, near-wall catalogue | B1, B3 met; B2, B4, B5 missed; B7 NACA 0012 not attempted; all partial | 0016 (C5 row), 0017 |
| [0017](../../docs/adr/0017-realizable-a0.md) | | registration + results | F-042 | Realizable A0 = 4.04 against the printed 4.0 | P4 hypothesis refuted (6.413 -> 6.349, marginal, not grid-converged); default stays 4.04 | |
| [0026](../../docs/adr/0026-wave18a-les.md) | | registration + results | F-045, F-206 | Smagorinsky, WALE, mixing length | TG1 (0.5920 against 0.85 to 1.15) and TG3 ordering missed (missing transposed stress); channel within 3.99 % of DNS | 0032 |
| [0032](../../docs/adr/0032-wave20a-turb2.md) | | registration + results | F-045, F-046, F-207 | Full viscous stress, DES / DDES, RSM | A1 to A8 met; C2(d) missed; C7 NOT DELIVERED (no wall treatment) | |
| [0061](../../docs/adr/0061-wave30c-thermal-turbulence-validation.md) | | validation | F-043, F-050, F-125 | Thermal, compressible and turbulence validation against read data | cavity, Sod, Graetz met; SST misses D1, D3 and the D2 log-law fit; k-epsilon D4 met | 0062, 0063 |
| [0063](../../docs/adr/0063-wave31b-turbulence-wall-diagnosis.md) | | diagnosis | F-042, F-043 | SST wall-resolved Cf deficit; k-epsilon wall-function mesh dependence | no defect isolated, no solver change; HA10 unresolved | 0070 |
| [0070](../../docs/adr/0070-wave33c-sst-near-wall-foundation.md) | | audit + diagnosis | | SST term audit, flat-plate and channel checks, grid convergence | no SST defect; "+3 percent" only partly explained; observed order 0.66 unresolved; P2f missed (expected) | 0074 |
| [0074](../../docs/adr/0074-wave34c-symmetry-and-sst-convergence.md) | | audit + diagnosis | F-712 | `Symmetry` terms, SST convergence order, the corrected diagonal as default | half-channel defect is the averaged diagonal; order 1.07 on TMR grids; default stays `averaged` | |

## Key numbers and limits

- ADR 0015 catalogue: "SST at y+~1, Standard at y+~30 and SST roughness (by 17 to 22 percent) miss; Enhanced did not converge and is unvalidated; non-equilibrium wall functions were not attempted."
- ADR 0017: A0 = 4.0 moves the medium-mesh reattachment "from a miss (6.413) to a pass (6.349)"; "P3 misses at 0.105, and on the fine mesh x_r = 6.453 is outside the window".
- ADR 0026: channel profile against DNS at `Re_tau` = 395 "worst 3.99 % for the linear profile, 3.12 % for the Nikuradse analogy, limit 10 %"; "In this solver WALE dissipates more than Smagorinsky, the opposite of the registered ordering."
- ADR 0032: C2(d) `k(40)/k0` = 1.83e13 (ssg) and 4.58e16 (lrr_ip) against `k(t_end) < 1e3 k0`, "not a property of the model".
- ADR 0063: the residual is "a slowly (first order) decaying sensitivity to the published wall-face omega factor 10, not a defect"; still unexplained then: "the log-law slope 2.78, the +3 % fixed-x difference against CFL3D after extrapolation".
- ADR 0070: "the finest rcfd value is -0.67 percent from the incompressible TMR mean, but rcfd's own Richardson value is still +2.3 percent above it"; observed order "0.66 against about 1.1 to 1.2 for the TMR codes".
- ADR 0074: the flipped `corrected` default "moves exactly one registered baseline hash (`packed_bed`), but the two lumped heat-exchanger tests do NOT converge with it".

## Open questions

- ADR 0074: the 0.82 to 0.87 percent offset below the TMR mean, "and which part of the TMR family restores the order, stay open".
- ADR 0017: changing A0 to 4.0 is "left to the user".
- ADR 0074: `component_diagonal = "corrected"` as default is "the user's" decision (later held; see [[component-diagonal]]).
- ADR 0026, 0032: model constants and DNS values "written from memory"; LRR-IP constants unverified.

## Related

- [[k-omega-sst]], [[rcfd-turb]], [[flat-plate]], [[backward-facing-step]], [[component-diagonal]], [[verification-vs-validation]]
- [[adr-thread-transient-rans]] (later turbulence work), [[adr-thread-early-validation-benchmarks]]
- [[roadmap-turbulence]]
