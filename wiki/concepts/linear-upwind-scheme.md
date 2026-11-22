---
type: concept
tags: [foundation, discretisation, convection-scheme]
sources: [ref-berger-aftosmis-murman-2005-slope-limiters, ref-venkatakrishnan-1995-icase-95-3, ref-jasak-1996-phd-thesis, ref-mit-ocw-2-29-numerics, ref-klaij-vuik-2013-simple-preconditioners]
builds_on: [upwind-and-central-differencing, gradient-reconstruction]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Linear-upwind scheme (second-order upwind by gradient extrapolation)

## In short

Instead of copying the upwind cell value to the face (first-order upwind), extrapolate it: start from the upwind cell's value and add its gradient times the distance from the cell centre to the face. That makes the face value exact for linear fields, so the scheme is second order, while still taking its information from upstream. It is not bounded by itself, so it is often combined with a limiter ([[ref-berger-aftosmis-murman-2005-slope-limiters]]; [[ref-jasak-1996-phd-thesis]]).

## Explanation

**Face value.** A piecewise-linear reconstruction in cell `i` is `U_i(x) = u_i + phi_i grad u_i . (x - x_i)` with a limiter factor `phi_i` (1 means unlimited) ([[ref-berger-aftosmis-murman-2005-slope-limiters]] II.B, Eqn. 4). Evaluating it at the face centre `x_f` of the upwind cell `C` gives the linear-upwind face value `phi_f = phi_C + (grad phi)_C . (x_f - x_C)` (own evaluation of Eqn. 4; the gradient comes from [[gradient-reconstruction]]). This is the reconstruction step of van Leer's MUSCL schemes on unstructured grids ([[ref-venkatakrishnan-1995-icase-95-3]] 3).

**Accuracy.** Exact for linear data when the gradient is (least squares is exact for linear data even on distorted meshes), so "first-order slope estimates ... are sufficient for second-order accuracy overall" ([[ref-berger-aftosmis-murman-2005-slope-limiters]] III.B).

**Boundedness.** On the normalised-variable diagram the linear upwind scheme of Warming and Beam, like central differencing and QUICK, lies outside the convection boundedness criterion; "the only basic differencing scheme that satisfies the boundedness criterion is UD" ([[ref-jasak-1996-phd-thesis]] 3.4.1, Fig. 3.11). Near steep gradients the extrapolated value can therefore overshoot the neighbours, which is what slope limiters prevent ([[slope-limiters-unstructured]]).

**Implementation by deferred correction.** Because the extrapolation reaches past the immediate neighbours, the scheme is usually written as first-order upwind in the matrix plus an explicit correction in the source: "higher-order advection schemes are implemented in defect correction form" ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.2); see [[deferred-correction]]. The OCW notes list the linear upwind scheme among the options next to QUICK, skewed upwind and hybrid schemes ([[ref-mit-ocw-2-29-numerics]] L16 s.18).

## Builds on

- [[upwind-and-central-differencing]]: the upwind direction and the first-order scheme being improved.
- [[gradient-reconstruction]]: supplies the cell gradient.

## Leads to

- [[tvd-schemes-and-flux-limiters]], [[slope-limiters-unstructured]]
- rcfd pages: [[k-omega-sst]], [[doc-case-schema-linear-upwind-limiter]]

## How rcfd uses it

- `solver.linear_upwind_limiter` (`none` default, `barth_jespersen`) limits `linear_upwind` for `simple`/`simplec` (ADR 0081) and `piso`/`pimple` including the moving-mesh step (ADR 0183); it applies a Barth-Jespersen factor per velocity component before the face extrapolation; turbulence, scalar and energy equations are not limited ([[doc-case-schema-linear-upwind-limiter]]).
- Measured (ADR 0081): with the limiter the FAN-01 sector mesh is stable where the unlimited scheme diverges at iteration 7; forced on in all 542 steady `linear_upwind` runs of the test suite with no new divergence-guard trip; Kovasznay L2 order 2.17 ([[doc-case-schema-linear-upwind-limiter]]).
- Setup rule SV302 warns about `linear_upwind` without limiter on wall cells whose aspect ratio times tan(non-orthogonality) is 3000 or more (ADR 0172) ([[setup-rules]]).
- For the SST model `linearUpwind` for k/omega "diverged in tests" ([[k-omega-sst]]).
