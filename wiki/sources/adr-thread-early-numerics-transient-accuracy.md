---
type: source
tags: [adr-thread, numerics, component-diagonal, schemes, bc-catalogue, convergence, adaptive-stepping, bdf2, piso, pimple, periodic-seams, fractional-step, coupled-solver]
sources: [doc-component-diagonal-default, doc-case-schema-adaptive-time-stepping, doc-case-schema-bc-catalogue, doc-case-schema-schemes-wall-motion-limits, doc-reference-data-wave13-numerics, doc-reference-data-taylor-green]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: early numerics, transient accuracy and the periodic-seam floors (component diagonal to wave 42A)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations, diagnoses, partials close-outs), 18 ADRs, ADR 0010 to wave 42A.

The thread starts with the convention that shaped many later results: the component-consistent momentum diagonal is an option, not the default, because flipping it moves the registered `packed_bed` hash (ADR 0010). It then collects the numerics items of the wave records: schemes, poor-mesh options and wall motion (ADR 0016), initialisation, zone sources and user equations (ADR 0019), the boundary-condition catalogue (ADR 0022), the residual-floor diagnosis with the pseudo-transient controller and the first adaptive stepping (ADR 0023), the block-coupled solver (ADR 0029), the fractional step (ADRs 0037, 0043), and the transient accuracy series on the Taylor-Green vortex and periodic seams (ADRs 0046, 0069, 0073, 0076, 0080). Partials close-outs end F-204, F-205, F-208 and keep F-209 partial (ADRs 0094, 0098, 0101, 0105). The later PISO-stability work is on [[adr-thread-piso-stability-consistent-correctors]] and the BDF2 step remainder on [[adr-thread-bdf2-step-consistent-face-flux]].

## Current state

- **Component diagonal:** `solver.component_diagonal = "corrected"` is an opt-in, `averaged` the default; enabling it for every case "fails `baseline_hashes::shipped_cases_match_registered_hashes`, because `packed_bed` (symmetry side walls) changes" (ADR 0010). MRF cases always use the correction (ADR 0008). ADR 0074 found that the half-channel error (with the averaged default the set-ups "are wrong by O(1)") is this averaged diagonal and that the case key never reached the transient kernel (fixed, opt-in). The default question went to the user repeatedly (ADRs 0010, 0042, 0074) and was later held ([[component-diagonal]], [[doc-component-diagonal-default]]).
- **Residual floors are a normalisation artefact:** "the residual stalls of the incompressible known misses are a normalisation artefact, the option `residual_norm` (default `reference`, bit-identical) is the delivered remedy, and no solver defect was found" (ADR 0023). The pseudo-transient controller `pseudo_control` "is NOT validated".
- **Adaptive stepping (F-209) stays partial:** `rcfd_fv::stepctl` with additive case keys (ADR 0050); a uniform step count check missed (ADR 0094, 74 steps against N_min = 64); two-timescale error control missed "as predicted" (ADR 0101); error control without a chosen step met on the library controller (ADR 0105), but "the case-key path still takes its first, largest and smallest step from `solver.dt`". See [[doc-case-schema-adaptive-time-stepping]].
- **BDF2 order floors on the iterative path:** the Taylor-Green order misses of ADR 0069 have a located cause, "the commutator $C(G p - b)$ of the spatially variable `rAU` with the face interpolation" (ADR 0073). The consistent interpolation is kept only as the opt-in `PimpleSettings::rhie_chow_consistent` "although it is NOT accepted by the registered rule". The combined fix K of ADR 0076 was removed by the per-fix rule; "the default solver is unchanged and keeps the 34B floors". See [[piso]].
- **Periodic seams:** the lagged partner value at translational periodic patches was a defect, made implicit in `pimple.rs` (ADR 0046); "Rotationally periodic patches keep the lagged partner (named limitation)". The transient body-force seam source is removed by the opt-in `PimpleSettings::coupled_body_pair_mean` (ADR 0080), "library-level with no schema key". Later seam work: [[adr-thread-coupled-periodic-faces]], [[adr-thread-periodic-seam-diffusivity]].
- **Closed partials:** F-205 done (coarse start 220 against 253 and 258 fine iterations, ADR 0094); F-204 and F-208 done (ADR 0098); F-063 done (ADR 0101).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0010](../../docs/adr/0010-component-diagonal.md) | | registration + results | F-031, F-050, F-204, F-712 | Component-consistent diagonal in the pressure-based kernels | option, not default (`packed_bed` hash moves); T3c BDF2 order 2.08 with it; T5 in/out and M3(a) still miss | 0074 |
| [0016](../../docs/adr/0016-wave13-numerics.md) | | registration + results | F-203, F-204, F-208, F-211, F-246, F-256, F-291 | Scheme additions, poor-mesh options, wall motion and shear | all partial; compressive(1.5) and (2) do not converge; D5b missed at n_r = 32 | 0093, 0098 |
| [0019](../../docs/adr/0019-wave15a-fv-sources.md) | | registration + results | F-201, F-202, F-205 | Initialisation methods, zone sources and fixed values, user transport equations | F-205 I2 missed (258 against 253 and 258); F-201, F-202 met | 0094 |
| [0022](../../docs/adr/0022-bc-catalogue.md) | | registration + results | F-200 | Mass-flow, total-pressure, vent, opening, radial-equilibrium outlets via `bc_kind` | F1 `secant` diverges; C3 reversed with K = 0 does not settle in 6000; residual convergence missed on five | 0055 (guarded secant) |
| [0023](../../docs/adr/0023-wave17a-convergence.md) | | diagnosis + registration | F-106, F-209 | Residual floors; pseudo-transient controller; error-estimator step | floors a normalisation artefact (`residual_norm = scaled`); controller NOT validated; E1, E1b missed | 0050 |
| [0029](../../docs/adr/0029-wave19a-coupled.md) | | registration + results | F-032, F-051 | Block-coupled solver with open boundaries, body force, energy, solid zones | A2, A3 and D4 (b) missed; coupled solver not shown faster | |
| [0037](../../docs/adr/0037-wave21c-wiring-fractional.md) | | registration + results | F-054, F-056, F-210 | Non-iterative fractional-step advance (and wiring, on [[adr-thread-vof-multiphase-particles]]) | F2 BDF2 order 1.040, F3 ratio 2.479, F8 Kim-Moin 1.130 missed; F1, F6 meet the acceptance | 0043 |
| [0043](../../docs/adr/0043-wave25a-temporal-accuracy.md) | | diagnosis + fixes | F-210, F-212, F-215 | Fractional-step periodic defect, solidification Newton, interface drift | periodic partner defect fixed (F3 3.954); F2 order 1.513 still a miss, the smoothing term "is not a defect" | |
| [0046](../../docs/adr/0046-wave26a-transient-defects.md) | | diagnosis + fix | F-214, F-743 | Lagged periodic partner and large-$d$ instability of PISO / PIMPLE | periodic coupling implicit (D1); wall-bounded remainder "a property of the scheme" beyond about d = 20 to 30 | [[adr-thread-piso-stability-consistent-correctors]] |
| [0050](../../docs/adr/0050-wave27b-robustness-adaptive-stepping.md) | | diagnosis + registration | F-209, F-235, F-322 | SIMPLEC non-orthogonal-corrector divergence; adaptive time stepping | compensation used the orthogonal part only, opt-in `simplec_non_orthogonal_compensation`; "No registered check missed" | |
| [0069](../../docs/adr/0069-wave33b-transient-accuracy-coupling.md) | | registration + results | F-301 | BDF2 order of PISO / PIMPLE and fractional step; stability on the impulsive cavity | O1-O4, O7 missed; F3 `component_diagonal` kept (fixes `Bdf2Bounded`); PIMPLE-2 28 of 32 | 0073 |
| [0073](../../docs/adr/0073-wave34b-transient-floor-pimple-lag.md) | | diagnosis + opt-in | F-301, F-344, F-530, F-535 | Taylor-Green BDF2 floor; PIMPLE one-corrector lag instability | H-COMM stands; `rhie_chow_consistent` kept opt-in; `pressure_lag_check` added; B4b, B4a misses | 0076 |
| [0076](../../docs/adr/0076-wave35a-transient-seams-and-relaxation.md) | | registration + results | F-344, F-443 | Seam convection lag, consistent interpolation, PIMPLE-C relaxation anchor | candidate K not accepted; all three fixes removed; seam body-force coefficient (Y5) left for the fan milestone | 0080 |
| [0080](../../docs/adr/0080-wave36a-transient-seam-body-force-pair-mean.md) | | registration + results | | Pair mean of `rAU` for the transient body-force seam flux | F-BP kept (seam pair sum from 1.6e-4 to below 7e-11); B-R2, B-R2b, B-R3 known misses | |
| [0094](../../docs/adr/0094-wave39c-coarse-step-start-plate-y-plus-uniform-step-count.md) | | partials close-out | F-108, F-205, F-209 | Coarse start on the step; solved flat-plate y+; uniform step count | F-205 done; Y1 met (+8.8 %); T1 missed (74 against 64) | |
| [0098](../../docs/adr/0098-wave40b-taylor-couette-converged-stretched-third-level-fast-piston.md) | | partials close-out | F-204, F-208, F-246, F-247 | Converged Taylor-Couette torque; third stretched level; Mach 0.2 piston | all met; F-204, F-208 done | |
| [0101](../../docs/adr/0101-wave41a-oscillating-cylinder-two-timescale-decay-vortex-bracket-milne.md) | | partials close-out | F-052, F-063, F-143, F-209 | Oscillating cylinder; two-timescale error control; vortex bracket; Milne slab | C1, G6, P1, R7 met (F-063, F-143, F-052 done); E1 missed as predicted | 0105 |
| [0105](../../docs/adr/0105-wave42a-error-control-without-a-chosen-step-simplec-divergence-hint.md) | | partials close-out | F-209, F-248 | Error control without a chosen step; SIMPLEC divergence hint | every limit met (E / (tol T) 0.792, 0.832 in [0.3, 3]); F-209 partial; F-248 hint on [[adr-thread-steady-solver-robustness]] | |

## Key numbers and limits

- ADR 0010: with the correction "`t3c_order_bdf2` passes at order 2.08 (registered [1.80, 2.20]; 1.34 before)"; misses remain: "T5 in/out equality (1.56e-5 against 1e-6), the M3(a) spurious speed (1.41e-3 V_i against 1e-3)".
- ADR 0016 C5: "the tree equals the library run (max abs difference 0.000e0, limit 1e-12), which does not validate the options physically"; G3 mesh deviation "13.96 degrees > 12".
- ADR 0022 / 0055: the `bernoulli` option meets F1 and F2; `guarded_secant` later meets G1 (4.2e-16 after 1880 iterations).
- ADR 0029 A3: coupled against SIMPLEC "4.6e-3 U_max against 1e-4", pressure drop "7.9 percent against 2 percent"; D4 (b) "2.55e-5 q L_x against 1e-9".
- ADR 0046: "in a closed box the 2-corrector PISO step diverges beyond about d = 20 to 30, a threshold read from forced runs only".
- ADR 0069: O1 fine pair 1.624, O2 1.223 and 1.587, O3 1.031, O4 0.965 and 1.460, O7 1.832 against [1.9, 2.3]; F2 rejected because "its error against the exact solution at dt = 0.05 rises 25.9 times"; BDF2 cylinder St 0.16480, C_D 1.35247.
- ADR 0073: F-K "gives order 2 on the fractional path but a 17 to 21 % larger error"; "The pressure-lag model's per-step factor matches the solver's measured growth within 4 % and predicted seven new runs correctly."
- ADR 0076 with K: C-S1 0.995 -> 2.232, C-O2 1.237 -> 1.332; N1 TG-S velocity 1.0504 -> 1.0077.
- ADR 0098: D5d "converged after 6206 iterations with a relative torque error of -4.307e-3", M5 L2 order 1.908, piston A1 / A2 2.935e-4 / 4.345e-4 against 2e-2.
- ADR 0101: E1 "1913 controlled steps against a smallest uniform count of 64 at tol 1e-2, and 3149 against 240 at tol 2.5e-3".

## Open questions

- ADR 0094: "the proposal to reword its [F-209] acceptance is with the user and not applied"; ADR 0101 derives that local error control cannot beat uniform implicit Euler on the final error "unless the slow mode has decayed by about e^-(rate ratio)".
- ADR 0073, 0076, 0080: wiring the transient keys (`rhie_chow_consistent`, the 35A keys, `coupled_body_pair_mean`) into the schema "is the user's decision".
- ADR 0076: the SECT-M seam body-force coefficient Y5 is "an open item for the fan milestone".
- ADR 0062: `PimpleSolver::step` "has no divergence guard" (later answered by the transient guard, [[doc-case-schema-transient-divergence-guard]]).

## Related

- [[component-diagonal]], [[piso]], [[simple]], [[divergence-guard]], [[bit-identical]], [[known-miss]]
- [[adr-thread-piso-stability-consistent-correctors]], [[adr-thread-bdf2-step-consistent-face-flux]], [[adr-thread-coupled-periodic-faces]], [[adr-thread-steady-solver-robustness]], [[adr-thread-fan01-rotating-solve]]
- [[roadmap-numerics-and-solvers]]
