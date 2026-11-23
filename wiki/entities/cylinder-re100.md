---
type: entity
tags: [validation, benchmark, vortex-shedding, milestone]
sources: [doc-reference-data-cylinder-wake, doc-reference-data-f869-cylinder-mean-drag, doc-reference-data-wave30b-incompressible-validation, doc-reference-data-wave31c-reference-reading, doc-reference-data-wave31a-solver-defects, doc-milestones-json, adr-thread-simulation-milestones-validation, doc-investigation-job48t-cylinder-spokes, doc-example-projects, adr-thread-piso-stability-consistent-correctors]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Cylinder wake at Re 100 (vortex shedding)

**What it is:** laminar flow past a circular cylinder at Re 100 on a body-fitted O-grid, compared by Strouhal number and mean drag coefficient; the case behind the `vortex-shedding` milestone (headline F-029, F-869) ([[doc-milestones-json]]).
**Where:** `crates/rcfd-fv/tests/cylinder.rs`, `crates/rcfd-api/tests/validation_cylinder_re100*.rs`, `w48t_cylinder_spokes.rs`, `f869_cylinder_re100_mean_drag.rs`; literature rows in `validation-data/reference/cylinder_re100_literature.csv`.

## Details

- Strouhal number: St = 0.165 (Williamson 1996 review, as reported in arXiv:2303.09262), "verified within 1 percent"; the same table gives 0.164 (Kravchenko, Posdziech and Grundmann, Sivakumar) and 0.167 (Oliveira) ([[doc-reference-data-cylinder-wake]]).
- Williamson (1989) eq. (6), read in the primary paper in wave 31C: S = A/Re + B + C Re with A = -3.3265, B = 0.1816, C = 1.600e-4, giving St(100) = 0.164335 ([[doc-reference-data-wave31c-reference-reading]]). Henderson 1995 "still unverifiable".
- "The 5 percent tolerance is unchanged; the recorded run of 0.1588 is -3.8 percent from 0.165 (-3.2 percent from 0.164)" ([[doc-reference-data-cylinder-wake]]).
- Wave 31A (rcfd computation): St extrapolated to zero time step and mesh 0.1669, C_D 1.357 ([[doc-reference-data-wave31a-solver-defects]]).
- Mean drag band (F-869, ADR 0761): eleven values from arXiv:2303.09262v1 Table 1 and arXiv:1401.6513 Table 3 (Kravchenko 1.314, Oliveira 1.370, Sivakumar 1.325, Posdziech 1.325, Mossaz 1.328, Qu 1.319, Constant 1.37, Peng 1.361, Parvar et al. own 1.359 on 64 D x 32 D, Park et al. 1.328, Ding 1.325). "Band of the mean drag coefficient at Re 100: [1.314, 1.370] ... width 0.056, middle 1.342, half-width 2.09 % of the middle; no allowance for discretisation added", registered before any run. "Nine of the eleven values are quotations and two are primary; all are computations, none is a measurement" ([[doc-reference-data-f869-cylinder-mean-drag]]).

> [!warning] Contradiction
> The in-repo CSV row "Williamson (experiment) 0.165" (1996 review) and the 1989 fit 0.164335 differ by 0.4 %; both lie inside the 1 % window of the cylinder test ([[doc-reference-data-wave31c-reference-reading]] discrepancy 5, [[doc-reference-data-cylinder-wake]]). Within the docs, recorded.

- **F-869 first sequence (ADR 0761, partial; BDF2, far field 20 D):** the drag is 1.35247 on 128 x 100 at dt 0.05 (V1, in the band) and 1.34511 at dt 0.025. The Richardson value is 1.37479, 0.0048 above the band (known miss V2b). The default first-order step gives 1.30996, outside the band ([[adr-thread-simulation-milestones-validation]]).
- **F-869 second sequence (ADR 0763, partial; BDF2, dt 0.025, far field 30 D):** the drag is 1.32259, 1.33285 and 1.33900 on 90 x 70, 128 x 100 and 181 x 141 (St 0.16169, 0.16395, 0.16486; GCI_fine 0.935 %). The observed order is 1.387, outside the registered [1.5, 2.5] (known miss W3b), so the mesh-converged drag "is not shown". The extrapolation gives 1.343 to 1.354 for every order from 1 to 2.5 ([[adr-thread-simulation-milestones-validation]]).
- **F-869 done (2026-11-22):** the maintainer answered ADR 0763 Q1 "Yes, option (c)": done as it stands, the fine value 1.33900 in the band and the Richardson value in it for every order from 1 to 2.5; the order miss W3 (b) is kept as a known-miss test and a record. The `vortex-shedding` headline reads met again. F-881 (todo) diagnoses why the drag converges at an order near 1.4, testing the O-grid grading, the wall force integral and the far-field condition one at a time ([[adr-thread-simulation-milestones-validation]]).
- The comparison uses the option `time_scheme = Bdf2`. The default step stays unchanged, by the maintainer's answer to ADR 0761 Q2 (a) ([[adr-thread-simulation-milestones-validation]]).
- **Pressure spokes on a refined O-grid (job 48T, ADR 0154):** a 256 x 256 run with `grading = 100` shows grid-scale wall pressure spokes because the default PISO (two correctors) loses its pressure-velocity coupling when nu dt / dr1^2 is large (clean up to 6.5, spoky from 11.4; 22.7 on the user's run); `time_advance = "fractional_step"` is the registered workaround, clean to t = 10 only ([[doc-investigation-job48t-cylinder-spokes]]).
- The Studio example [[example-project-wind-tunnel]] runs Re 100 and Re 200 on a 64 x 48 O-grid (3072 cells) to 80 D/U: St 0.1517 and cd_mean 1.323 at Re 100, "not a validation" ([[doc-example-projects]]).
- **Refined to 256 x 256 in the Studio** (wave 48, [[adr-thread-piso-stability-consistent-correctors]]): radial pressure spokes at the wall, "a numerical artefact of the solver" once the wall diffusion number exceeds an onset between 6.5 and 11.4 (ADR 0154); the opt-in adaptive correctors (`solver.corrector_control = "adaptive"`, ADR 0156) keep every refined run clean, and the egui Studio's Run turned them on (ADR 0158). Through Run on the automatic step the 256 x 256 case ran 40 s within every limit: "10,742 steps in 1.31 h, Courant at most 0.4501, St 0.1663" (ADR 0153, [[adr-thread-setup-warnings-and-time-step]]).

## Related

[[simulation-milestones]], [[dfg-cylinder-benchmark]], [[verification-vs-validation]], [[doc-reference-data]]
- [[doc-investigation-job48t-cylinder-spokes]], [[example-project-wind-tunnel]]

## Open questions

- No measured mean drag at Re 100 was obtained; the originals of the nine quoted values were not read ([[doc-reference-data-f869-cylinder-mean-drag]]).
- ~~ADR 0763 question 1 (open): is F-869 done when the fine value and every extrapolated value are in the band but the observed order is 1.39? The recommendation is (c): done, with a new item to diagnose the order ([[adr-thread-simulation-milestones-validation]]).~~ Answered (c) on 2026-11-22; F-881 entered ([[adr-thread-simulation-milestones-validation]]).
- Why the order is near 1.4 is open (F-881, not issued) ([[adr-thread-simulation-milestones-validation]]).
