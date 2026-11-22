---
type: entity
tags: [validation, benchmark, vortex-shedding, milestone]
sources: [doc-reference-data-cylinder-wake, doc-reference-data-f869-cylinder-mean-drag, doc-reference-data-wave30b-incompressible-validation, doc-reference-data-wave31c-reference-reading, doc-reference-data-wave31a-solver-defects, doc-milestones-json]
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

## Related

[[simulation-milestones]], [[dfg-cylinder-benchmark]], [[verification-vs-validation]], [[doc-reference-data]]

## Open questions

- No measured mean drag at Re 100 was obtained; the originals of the nine quoted values were not read ([[doc-reference-data-f869-cylinder-mean-drag]]).
