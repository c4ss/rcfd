---
type: source
tags: [adr-thread, numerics, piso, pimple, consistent-correctors, diffusion-number, component-diagonal]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: PISO/PIMPLE stability at large diffusion numbers and the consistent correctors

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (diagnoses, opt-in registrations, maintainer answers), 11 ADRs, 2026-11-07 to 2026-11-15.

The falling block of F-244 (ADR 0211) diverged in the thin lubrication gap. F-299 (ADR 0214) located the cause in the default PISO correctors: above a cell diffusion number `nu dt/dx^2` of a few, the non-iterated step has a pressure mode that alternates in sign every step and grows. The remedy built is the opt-in **consistent (SIMPLEC-type) correctors** (`PimpleSettings::consistent_correctors`, `rAtU = V/(a_P - H1)`). The thread then extends the opt-in (case key, moving meshes, periodic faces, one settings value), confirms the same mechanism in a plain channel (F-530), and adds outer-iteration control (F-536, F-550), an opt-in refusal at the onset (F-535), and two side findings on the `Symmetry` condition and the averaged component diagonal (F-531, F-538). See [[piso]] and [[component-diagonal]].

## Current state

- **The default step is unchanged and still unstable at large diffusion numbers.** F-299 and F-530 are partial: "The default path is still unstable ... the removal is only the library opt-in consistent_correctors" (F-299 gap); F-530: "removal is only by the opt-in consistent correctors with 8 outer iterations (a count taken from a diagnosis, matched only after amendment 1)".
- **The default flip is held.** Maintainer answer of 2026-11-18 on ADR 0223 Q1 (first asked in ADR 0214 Q1): "Held. The default change is not approved now: it would move existing results and waits until the fan path needs it." The same answer holds `component_diagonal = corrected` as the transient default (ADR 0613 Q1, first asked as ADR 0455 Q1).
- **Case key:** `[solver.consistent_correctors] enabled` (ADR 0223, default `false`, bit-identical when absent; see [[doc-case-schema-consistent-correctors]]). The five further boolean keys of ADR 0258 Q2 were approved on 2026-11-18 under the class rule for additive golden edits; the item is F-366 (todo). The outer-criterion keys `solver.outer_tolerance` and `solver.outer_scale_window` (ADR 0617 Q1) were approved the same way; the item is F-692.
- **Library opt-ins built:** `ConsistentCorrectors` value with `conflicts` and three extended combinations X1 to X3 kept (F-322, ADR 0258); periodic combinations PN, PD, PJ accepted under `consistent_periodic_extensions` (F-367, ADR 0360); `outer_pressure_tolerance` (F-536, ADR 0458) with `outer_scale_window` (F-550, ADR 0617: every step meets the criterion before the cap, 19 to 26 percent fewer outer iterations); `lag_onset_refusal` (F-535, ADR 0471).
- **Still refused or missing** (F-322 gap): new flags with periodic faces or a moving mesh (F-321), non-orthogonal correctors with pressure gradient options, the fractional step; `sim.rs` still writes its own errors instead of calling `conflicts`; `overset.rs` reads the older field. F-301: case-level wiring of the moving-mesh lag estimate (Q2, needs a test edit) stays open.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0214](../../docs/adr/0214-f299-registration-piso-instability-at-large-cell-diffusion-number-across-a-thin-gap.md) | 2026-11-07 | diagnosis + opt-in | F-299 | Why the PISO step diverges across a thin gap | partial; every check met (two set-ups corrected by amendment); opt-in `consistent_correctors` built | 0223 |
| [0223](../../docs/adr/0223-f301-registration-consistent-piso-correctors-moving-mesh-lag-estimate-case-key-and-benchmarks.md) | 2026-11-08 | registration + benchmark | F-301 | Moving-mesh lag estimate, case key, benchmark set for the default | partial; DC1 to DC3, DC6, DC7 met; DC4, DC5 known misses; DC8 defective set-up; default held 2026-11-18 | 0258 |
| [0258](../../docs/adr/0258-f322-registration-consistent-correctors-one-settings-value-and-extended-combinations.md) | 2026-11-12 | registration | F-322 | One settings value, located refusals, three extended combinations | partial; X1 to X3 kept, S1X met; case half of clause A not delivered | 0360 |
| [0360](../../docs/adr/0360-f367-registration-consistent-correctors-new-combinations-with-periodic-faces.md) | 2026-11-12 | registration | F-367 | The new combinations with periodic faces | partial (Rule I); all three accepted, PD and PX met only after amendment 1 | |
| [0451](../../docs/adr/0451-f530-registration-piso-stability-at-large-diffusion-numbers.md) | 2026-11-14 | diagnosis | F-530 | Stability of the non-iterated PISO step at large `D` in a channel | partial; located (same mechanism as ADR 0214); removed by the option (A1); DG1, DG3, QF2, A2, A3, AP missed; c2b met only after amendment 1 | 0458, 0471 |
| [0455](../../docs/adr/0455-f531-registration-symmetry-plane-tangential-viscous-flux.md) | 2026-11-14 | diagnosis | F-531 | Viscous flux of `Symmetry` for tangential flow | partial; Q4c missed by `7.9e-2` against `5e-2`; removal only through `component_diagonal corrected` | 0613 |
| [0458](../../docs/adr/0458-f536-registration-outer-iteration-criterion.md) | 2026-11-14 | registration | F-536 | A criterion for the outer-iteration count | partial; K1 missed (10 steps each at `N = 25` and `50` reached the cap) | 0617 |
| [0471](../../docs/adr/0471-f535-registration-opt-in-refusal-at-the-pressure-lag-onset.md) | 2026-11-14 | registration | F-535 | Opt-in refusal at the lag onset naming the diffusion number | partial; every limit met, MU6 caught only by L3b added by amendment, N2 not completed | |
| [0613](../../docs/adr/0613-f538-registration-averaged-component-diagonal-in-the-relaxed-pimple-loop.md) | 2026-11-15 | diagnosis | F-538 | The averaged component diagonal in the relaxed PIMPLE loop | partial; registered model omitted `c_x`, C1 missed by `1.07e-2` (limit `5e-3`), met after amendment 1 (model X) | |
| [0617](../../docs/adr/0617-f550-registration-non-vanishing-outer-criterion-scale.md) | 2026-11-15 | registration | F-550 | A non-vanishing scale for the outer criterion | partial (MW6 an equivalent mutant); every acceptance check met | |

## Key numbers and limits

- ADR 0214 D1 to D4 (step-map radius of the gap mode, alternating sign `-1.000`): `3.15959` at `tau/10` (model `3.165`), `1.73092` at `tau/20`, `4.98354` at `tau/5`, `1.66032` with 3 correctors; `0.97040` at `tau/40`. With the option (F1): `0.95364`, `0.91984`, `0.94214`. L1: the B4 lag estimate in Raise mode stops the default at step 1 with lag amplification `3.30207` in the gap column. F2: block force with the option `0.75184038` against `0.75184108` (`9.4e-7`).
- ADR 0451: onset by B4 "between `D = 2.5` and `3.0`" (B4 `0.962` at `D = 2.51`, `2.036` at `5.03`); step-map radius `9.13` against B4's `9.40` at `D = 25`; the option bounded over 3 periods at `D = 25` and `100`; "The step with the option alone does not converge the pressure within a step at such `D`".
- ADR 0223 benchmark set with the option: DC1 cavity 88 of 88 stable (default 84 of 88); DC6 ADR 0154 K1, K2 `M_hf` `5.738e-3` (limit `0.0114`; default K1 diverges at step 30); DC7 St `0.16467`, `C_D` `1.34578`; known misses DC4 (MMS pressure ratio `1.119` at dt `0.025`, limit `1.05`) and DC5 (O1 coarse order `2.305`, band top `2.3`).
- ADR 0458: within 1 percent of the discrete implicit-Euler impedance at every level (at most `5.66e-3` real, `2.77e-3` imaginary); `rcfd-fv` suite 1047 tests pass.
- ADR 0617: window `W = 18` steps.
- ADR 0471 Q3: the lag estimate depends on its iteration count (`0.962` at 20, `1.0003` at 200, `D = 2.51`); recommendation keep 20.
- ADR 0613: model X leaves `2.4e-3` of the relaxed 20 h `Delta Z` (and `1.2e-3` of the PISO one) to the 2-D end flow.

## Open questions

- ADR 0451 Q1: a default for large diffusion numbers (consistent correctors with outer iterations above an estimate, keep and document, or refuse); recommendation (b) keep and document.
- ADR 0455 Q1 / ADR 0613 Q1: `component_diagonal = corrected` as the transient default with `packed_bed` re-registered: held 2026-11-18.
- ADR 0458 Q2: the outer criterion as the default (recommendation no). ADR 0617 Q3: the status (lead decides).
- ADR 0471 Q1, Q2: a case key `solver.lag_onset_refusal` and the `lag_amplification` divergence event (need the golden approval).
- ADR 0223 Q2: case-level wiring for moving meshes (lift ADR 0156's refusal of `corrector_control = "adaptive"` with `dynamic_mesh`; needs the edit of test A9b).
- ADR 0258 Q1 / ADR 0360 Q1: fold the older fields and `consistent_periodic_extensions` into the one value.

## Related

- [[piso]], [[simple]], [[component-diagonal]], [[dynamic-mesh]], [[divergence-guard]], [[setup-rules]]
- [[doc-case-schema-consistent-correctors]], [[doc-dynamic-mesh]], [[doc-component-diagonal-default]]
- [[roadmap-numerics-and-solvers]]
