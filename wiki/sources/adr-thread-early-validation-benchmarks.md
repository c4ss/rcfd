---
type: source
tags: [adr-thread, validation, benchmarks, gci, lid-driven-cavity, backward-facing-step, cylinder, channel, heated-cavity, sod, references]
sources: [doc-reference-data-wave30b-incompressible-validation, doc-reference-data-wave31c-reference-reading, doc-validation-data]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the wave 30 validation against read benchmark data and the reference reading of wave 31C

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (validation registrations, a reading wave), 3 ADRs, wave 30B to wave 31C; their follow-up diagnoses are ADR 0062 ([[adr-thread-steady-solver-robustness]]) and ADR 0063 ([[adr-thread-early-turbulence]]).

Wave 30 is the first validation of the solvers against benchmark data read in the same wave (F-006, F-125), with mesh sequences, an observed order and a grid-convergence index for every benchmark, and no solver change ("a proven defect is recorded and reported, not fixed"). Wave 30B covers the incompressible laminar and turbulent cases (cavity, backward step, cylinder, Blasius, channel); wave 30C the thermal, compressible and turbulence cases (heated cavity, Sod, Graetz, pipe, flat plate). Wave 31C then read public sources to turn secondary and unverified citations into read ones. See [[gci-richardson]], [[verification-vs-validation]], [[unverified]].

## Current state

- **Incompressible (ADR 0060):** "The backward step met all its limits"; Blasius only on a corrected set-up; misses kept as ignored known misses: C5 at Re 1000, D4, D5, K1 and K3. "The addendum records no solver defect for C5 and no defect proven for D4, D5, K1 and K3." The limits "are not blind for the cavity, the backward step and the cylinder, because results of the existing tests were known to the author; the channel Re_tau = 178 and Blasius runs are blind." K1 was later met with a time-error allowance (ADR 0062). See [[lid-driven-cavity]], [[backward-facing-step]], [[cylinder-re100]].
- **Thermal, compressible, turbulence (ADR 0061):** the heated cavity meets A0 to A6 at Ra 1e3, 1e4, 1e5, and at Ra 1e6 two convergence checks miss (later met with alpha_u 0.5, ADR 0062); all Sod limits met by 19 tests, with a Van Albada limiter overflow recorded; Graetz C1 to C6 met; the constant-flux pipe met only C7'; SST misses D1, D3 and the D2 log-law fit; k-epsilon wall functions meet D4; "NACA 0012 was not attempted". See [[differentially-heated-cavity]], [[flat-plate]].
- **References (ADR 0064):** "The Williamson 1989 fit and Armaly's definitions moved to verified from primary sources"; the Ghia u values, Gartling lengths and Dean's relation moved "on secondary sources only"; "The reading found seven discrepancies and changed no existing value or test".

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0060](../../docs/adr/0060-wave30b-incompressible-validation.md) | | validation | F-006, F-125, F-786 | Incompressible laminar and turbulent solvers against read data, with GCI | backward step met; C5, D4, D5, K1, K3 known misses; no solver defect proven | 0062 (K1) |
| [0061](../../docs/adr/0061-wave30c-thermal-turbulence-validation.md) | | validation | F-043, F-050, F-125 | Thermal, compressible and turbulence validation | cavity, Sod, Graetz met; Ra 1e6 convergence, constant-flux pipe C7 and SST D1, D3 missed | 0062, 0063 |
| [0064](../../docs/adr/0064-wave31c-reference-reading.md) | | reading | F-006, F-125 | Turn secondary and unverified citations into read ones | some verified from primaries; seven discrepancies; no value or test changed | |

## Key numbers and limits

- ADR 0060 misses: "C5 at Re 1000 (GCI_fine 1.41 % against 1.0 %), D4 (observed order 0.63 against >= 1.0, GCI_fine 2.43 % against 1 %), D5 (ny = 12 U_b+ 0.164 > 0.10, spread 18.8 % > 5 %), K1 (St -3.77 % of 0.165 against 3 %) and K3 (GCI_fine St 4.05 %, C_D 3.67 % against 3 %)".
- ADR 0060: not obtained: "Armaly (1983), the Gartling and Ghia originals, Ghia v at Re 400/1000, Moody/Colebrook/Dean and the Williamson original".
- ADR 0064: "The Gartling Re 800 lengths remain in conflict, 12.20 / 9.70 / 20.96 against 11.9 / 9.5 / 20.6, unresolved without the original"; "The Schiller-Naumann switch at Re 1000 stays unverified".

## Open questions

- ADR 0064: the Gartling Re 800 conflict, Armaly's numeric reattachment lengths and a numeric Moody table remain unread.
- ADR 0061: the suspected "first-cell-height dependent omega wall treatment of `KOmegaSst` is not proven" (followed by ADRs 0063, 0070, 0074 on [[adr-thread-early-turbulence]]).

## Related

- [[gci-richardson]], [[verification-vs-validation]], [[unverified]], [[validation-data-pack]]
- [[lid-driven-cavity]], [[backward-facing-step]], [[cylinder-re100]], [[differentially-heated-cavity]], [[flat-plate]]
- [[adr-thread-simulation-milestones-validation]] (the later validation headlines)
- [[roadmap-quality-docs-and-validation]]
