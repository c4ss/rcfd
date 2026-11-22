---
type: source
tags: [adr-thread, numerics, robustness, simplec, divergence-guard, linear-upwind, limiter, vent, f-248]
sources: [doc-case-schema-steady-divergence-guard, doc-case-schema-simplec-compensation, doc-case-schema-linear-upwind-limiter]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: steady segregated-solver robustness (duct diagnosis, divergence guard, limited linear upwind, SIMPLEC modes, F-248)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (diagnoses with opt-in fixes), 8 ADRs, wave 24 to wave 42A.

Each ADR answers a steady SIMPLE / SIMPLEC run that diverged or stalled, usually found by another wave: the symmetry-walled duct and the pressure-pressure duct (ADR 0042), the cooler cavity and the cylinder bisection from the wave 30 validation (ADR 0062), the 2 h FAN-01 run with no divergence stop (ADRs 0075, 0077), the follow-ups of that (ADR 0081), the SIMPLEC instability on uniformly sheared meshes (ADR 0084), and SIMPLEC with a velocity inlet and a forward vent (F-248: ADRs 0095, 0102, 0105). The pattern is always the same: rank mechanisms, register a decision rule, keep a fix only as an opt-in when its mechanism stands. See [[divergence-guard]], [[simple]].

## Current state

- **A diverging steady run now stops.** "a steady SIMPLE/SIMPLEC/coupled run that diverges is now stopped, and the 34D FAN-01 run that ran for 2 h stops at iteration 7 to 8 in about a second"; the guard "only reads and does not stop direct `Simulation::step` callers, so every baseline hash is unchanged" (ADR 0077). The steady `max_du`, `continuity` and `res_u` reports keep NaN and `SimpleSolver::run` "no longer reports a NaN state as converged" (ADR 0081). See [[doc-case-schema-steady-divergence-guard]].
- **Limited linear upwind:** the FAN-01 divergence is "the unlimited linear-upwind extrapolation in high-aspect, strongly non-orthogonal wall cells, and a limited gradient removes it" (ADR 0077); the opt-in `solver.linear_upwind_limiter = "barth_jespersen"` (default `none`) "met every limit and is kept" (ADR 0081). Its limiter later cycles on the bladed passage ([[adr-thread-fan01-rotating-solve]]). See [[doc-case-schema-linear-upwind-limiter]].
- **Sheared-mesh SIMPLEC:** "a period-two pressure mode of the outer iteration"; fix R, the opt-in `solver.velocity_correction = "face_flux"`, "met every acceptance limit and is kept" (ADR 0084). Finding S, "the SIMPLEC compensation missing on periodic seam faces, is reported, not fixed".
- **F-248 done with a key:** SIMPLEC "compensates `(rAtU - rAU) grad(p_old)` on internal faces only, and the vent law feeds the resulting kick of the vent-cell velocity back with the gain G = 0.5 (rAtU - rAU) (A/V) K |u|. The run diverges when G > 1, whatever the Courant number." The fix is the opt-in `solver.simplec_boundary_compensation` (ADR 0102); a located hint names the key when such a run diverges (ADR 0105). "H-R3 is falsified: the FAN-01 stall is not caused by the missing boundary compensation." See [[doc-case-schema-simplec-compensation]].
- **Duct fix in the default path:** the committed F2 `convection.rs::lag_boundary_inflow` lags the inflow at a fixed-pressure inlet in the steady constant-density kernel, "and all 12 registered hash checks stay identical under it" (ADR 0042).
- **Wave 30 defects:** the cooler-cavity stall is linear instability of SIMPLEC with alpha_u 0.8, "no solver defect is proven"; the missed cylinder Strouhal number is "a time-discretisation error of the first-order-in-time PISO step combined with a dt refined together with the mesh" (ADR 0062).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0042](../../docs/adr/0042-wave24-duct-diagnosis.md) | | diagnosis | | Plug-flow deviation between symmetry walls; X4 pressure-pressure duct to NaN | symptom 1 the averaged diagonal; X4 fixed by `lag_boundary_inflow` (F2); D missed (4.96e-6 against 1e-6) | |
| [0062](../../docs/adr/0062-wave31a-solver-defects.md) | | diagnosis + patch | F-006, F-029, F-060 | Van Albada overflow; cavity Ra 1e6 stall; cylinder Re 100 bisection | limiter patch; `a_ra_1e6_alpha05` passes; K1 met with a time-error allowance | |
| [0077](../../docs/adr/0077-wave35b-solver-robustness-under-automation.md) | | registration + diagnosis | F-248, F-744 | Steady divergence guard; linear-upwind divergence on FAN-01; API lag check; corrected diagonal on the heat-exchanger channel | guard added; H2 and H0 stand (no linear-upwind fix by the rule); C4 missed | 0081 |
| [0081](../../docs/adr/0081-wave36b-solver-robustness-follow-ups.md) | | registration + diagnosis | | Limited linear upwind; sheared-mesh divergence; NaN reports; coupled guard trip; lag warning | `barth_jespersen` limiter kept; sheared mesh "start-up only" (H-X6); NaN reports fixed | 0084 |
| [0084](../../docs/adr/0084-wave37a-sheared-mesh-simplec-linearisation.md) | | diagnosis + opt-in | F-248 | Linearise the SIMPLEC outer iteration on sheared meshes | period-two pressure mode; `velocity_correction = "face_flux"` kept; M1 met in scaled variables only | |
| [0095](../../docs/adr/0095-wave39d-field-registers-monitor-frames-fan-vent-curves.md) | | partials close-out | F-124, F-144, F-217, F-248 | Field-threshold registers, monitor plots, fan and vent curves | all three done; X7 at pseudo_dt 0.02 known miss; finding: SIMPLEC with a forward vent diverges when K > 0 | 0102 |
| [0102](../../docs/adr/0102-wave41b-simplec-boundary-compensation-forward-vent.md) | | diagnosis + opt-in | F-248 | Why SIMPLEC with a velocity inlet and a vent diverges | boundary compensation missing; `simplec_boundary_compensation` added; F-248 done with the key | 0105 |
| [0105](../../docs/adr/0105-wave42a-error-control-without-a-chosen-step-simplec-divergence-hint.md) | | partials close-out | F-209, F-248 | A located hint naming the key when SIMPLEC diverges | D1, D1n, D2 met; F-248 stays done | |

## Key numbers and limits

- ADR 0042: SIMPLEC plug-flow deviation 0.159 between symmetry side walls; "A lossless pressure-pressure duct with `dp != 0` is ill-posed and must not be asked to converge"; at the fixed-pressure inlet "the convective terms cancel and leave a first-cell diagonal that is only viscous".
- ADR 0077: the pressure-lag check "warned on 10 runs of 3 existing tests (C4 MISSED), all with two correctors, so it was restricted to one corrector"; the `corrected` diagonal "reaches the exact plug flow to 2.5e-13 on the heat-exchanger channel".
- ADR 0084: "raw K40 60: 4.26e-4 against 1e-4"; final gate "1879 passed, 0 failed, 525 ignored"; on FAN-01 "P10 (L / c10 0.330 against 8.36)" met, "`res_u` and `res_p` are never both below 1e-4 within 1500 iterations".
- ADR 0102: D1 "1.553e-4 against 1e-4" at plain u = 1; with the key, "the F-200 C3 forward vent is non-finite at iteration 2767, and the X7 duct shows a second, transverse mode (not registered, not diagnosed further)".
- ADR 0095: X7 at fixed pseudo_dt 0.02 "passed at u = 0.5 and u = 1, gave NaN at u = 2"; corrected at Courant 1 for all six flows.

## Open questions

- ADR 0077: making the guard an error "is the user's decision (recorded)".
- ADR 0081: the mechanism of the sheared-mesh start-up growth "is not identified" (answered by ADR 0084 for SIMPLEC).
- ADR 0084 finding S and ADR 0102 transverse mode: reported, not fixed.

## Related

- [[divergence-guard]], [[simple]], [[component-diagonal]], [[fan-01]]
- [[adr-thread-fan01-rotating-solve]], [[adr-thread-early-numerics-transient-accuracy]], [[adr-thread-steady-kernel-warped-mesh]]
- [[roadmap-numerics-and-solvers]]
