---
type: concept
tags: [verification, numerics, mesh-convergence]
sources: [doc-milestone-fan, doc-reference-data-f870-fan01-design-point, doc-reference-data-wave31b-turbulence-wall, doc-reference-data-wave31a-solver-defects, doc-designs-exp-rot, adr-thread-simulation-milestones-validation, adr-thread-fan01-design-point]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Grid convergence index (GCI) and Richardson extrapolation

Richardson extrapolation estimates the mesh-converged value of a quantity from solutions on systematically refined meshes and their observed order of convergence. The grid convergence index turns the difference between meshes into an error band by multiplying it by a factor of safety. rcfd uses the three-mesh form "after Roache and Celik et al. (2008), from memory as in ADR 0086, no source could be read" ([[doc-reference-data-f870-fan01-design-point]]).

## How rcfd uses it

- **FAN-01 (ADR 0086, wave 37C).** Three C-grid meshes with 4 920 / 13 627 / 38 184 cells and a ratio of 1.41. The first cell and the wall stacks were not refined, so the wall-normal error is outside the band. Torque was 1.850 / 1.967 / 2.038 N m, with observed order 1.51 and a band of +-0.217 N m (+-10.6 %; Fs 1.25, order capped at 1). The measured 2.142 N m lies inside the band. "The Richardson value 2.143 N m comes within 0.05 % of it by coincidence inside a +-10.6 % band; this is not agreement." Delta p_ts was 46.8 / 64.2 / 74.0 Pa with order 1.74 and a band of [44, 104] Pa, and the measured 126.5 Pa lies outside it. The bands are "NOT shown credible" because the empty-passage check failed ([[doc-milestone-fan]]).
- **F-870 band (ADR 0762).** The numerical half of the FAN-01 design-point band is 0.21190 N m (32.98 W) and 29.75 Pa, from the ADR 0086 meshes with the ADR 0170 torque ([[doc-reference-data-f870-fan01-design-point]]).
- **Flat plate.** The NASA TMR reference has GCI 0.28 % and p = 1.21. rcfd's Richardson-extrapolated Cf(0.97) is +2.6 to +3.6 % above it, which is "not resolved" ([[doc-reference-data-wave31b-turbulence-wall]]).
- **Cylinder.** A fitted model with first-order time and second-order space terms extrapolates St to 0.1669 and C_D to 1.357 at zero time step and mesh size ([[doc-reference-data-wave31a-solver-defects]]).
- EXP-ROT reuses the ADR 0086 meshes and cites Celik et al. 2008 "(as cited in ADR 0086)" as recalled and UNVERIFIED ([[doc-designs-exp-rot]]).

- **Validity condition on the observed order (ADR 0763):** for the cylinder the extrapolation counts only if the observed order is in [1.5, 2.5]. The sequence gave 1.387, so "the extrapolation is therefore not valid and the mesh-converged drag is not shown", although every order from 1 to 2.5 puts the value in the band. The coarser sequence of ADR 0761 gave the same order (1.38) ([[adr-thread-simulation-milestones-validation]]).
- **F-870 band form (ADR 0762):** `U_num = 1.25 abs(f_2 - f_1) / (r_21^p_band - 1)`, `p_band = min(p, 1)`, `r_21 = 1.40981`. It is added, not combined in quadrature, to the measurement uncertainty, which is not registered ([[adr-thread-fan01-design-point]]).

## Related

[[verification-vs-validation]], [[fan-01]], [[flat-plate]], [[cylinder-re100]]

## Open questions

- The Roache and Celik sources have not been read. The GCI formula in use is recalled from memory ([[doc-reference-data-f870-fan01-design-point]]).
