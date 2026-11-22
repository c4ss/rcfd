---
type: entity
tags: [validation, benchmark, turbulence, laminar]
sources: [doc-reference-data-backward-step, doc-reference-data-laminar-step, doc-reference-data-known-misses, doc-reference-data-turbulence-constants, doc-reference-data-turbulence-finishing, doc-reference-data-wave30b-incompressible-validation, doc-reference-data-wave31c-reference-reading, doc-verification-report, doc-validation-data]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Backward-facing step (turbulent and laminar)

**What it is:** two step cases: the turbulent Driver and Seegmiller (1985) step for RANS models, and a laminar 2D step of expansion ratio 2 ([[doc-reference-data-backward-step]], [[doc-reference-data-laminar-step]]).
**Where:** `cases/backward_step/case.json`, `crates/rcfd-fv/tests/step_laminar.rs`; dataset `backward-step-tmr` (NASA TMR 2DBFS) ([[doc-validation-data]]).

## Details

Turbulent step (Driver and Seegmiller, AIAA J. 23 (1985) 163):

- Reattachment x/H = 6.26 +/- 0.10 and Re_H about 36000, verified on the NASA TMR and OpenFOAM pages; expansion ratio 1.125 with inlet channel 8h confirmed from secondary pages in wave 13; the "5.5 to 7 step heights for RANS" scatter is "unverifiable" ([[doc-reference-data-backward-step]]).
- Known misses against 6.26 +/- 0.10 (F-042): Realizable 6.402, RNG 6.373, Standard 5.359 ([[doc-reference-data-known-misses]]). Later turbulence finishing (ADR 0015): BFS-C Standard 5.350 and Realizable 6.413 against a window 6.16 to 6.36 ([[doc-reference-data-turbulence-finishing]]).
- The Realizable A0 constant (4.04 in code vs 4.0 in Shih et al.) moves BFS-C x_r/h from 6.349 (4.0) to 6.413 (4.04) ([[doc-reference-data-turbulence-constants]]); the verification report noted the 6.402 miss "could flip to a pass or stay a miss; this was NOT run" ([[doc-verification-report]]).

Laminar step, expansion ratio 2 (tests use Re_h = U_mean h / nu; literature Re = 2 Re_h):

- Re 800 (Re_h 400): Erturk 11.834 and Gartling 12.20 (arXiv:2507.16509 Table 2, secondary); measured 11.672 (-1.4 % / -4.3 %), pass within 5 % windows ([[doc-reference-data-laminar-step]]).
- Re_h 100 and 200: 4.95 and 8.10 (the arXiv paper's own computation, consistency check only); measured 4.949 and 8.182 ([[doc-reference-data-laminar-step]]).
- Wave 30B read ten published values 11.48 to 12.20 at Re_D 800 ([[doc-reference-data-wave30b-incompressible-validation]]). Armaly et al. (1983) was read in full in wave 31C but its reattachment lengths exist only as figures: "data not obtained" ([[doc-reference-data-wave31c-reference-reading]]).

> [!warning] Contradiction
> Gartling (1990) at Re 800: 12.20 / 9.70 / 20.96 step heights (MFIX-Exa quoting Gartling; arXiv:2507.16509) against 11.9 / 9.5 / 20.6 in a row labelled Gartling by Teruel and Fogliatto (AMCA 2013): "unresolved". The in-repo 12.20 agrees with two of three secondary sources ([[doc-reference-data-wave31c-reference-reading]]).

## Related

[[known-miss]], [[flat-plate]], [[doc-reference-data]], [[validation-data-pack]]

## Open questions

- Whether setting Realizable A0 to 4.0 turns the F-042 Realizable miss into a pass was not run ([[doc-verification-report]]).
