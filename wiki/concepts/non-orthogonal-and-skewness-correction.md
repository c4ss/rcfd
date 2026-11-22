---
type: concept
tags: [foundation, discretisation, mesh]
sources: [ref-jasak-1996-phd-thesis]
builds_on: [finite-volume-method, gradient-reconstruction, deferred-correction]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Non-orthogonal and skewness correction

## In short

The simple two-point diffusion flux `(phi_N - phi_P)/|d|` is exact only when the line between the two cell centres is parallel to the face normal and passes through the face centre. Real meshes break both: the angle between the centre-to-centre line and the face normal is the non-orthogonality, the miss distance at the face is the skewness. Non-orthogonal correction adds an explicit gradient term for the first; skewness lowers the accuracy of face interpolation unless it is corrected too ([[ref-jasak-1996-phd-thesis]] 3.3.1.3, 3.6).

## Explanation

**Orthogonal case.** When `d` (from P to N) is parallel to the face area vector `S`, the face-normal gradient is computed from the two cell values alone, `S . (grad phi)_f = |S| (phi_N - phi_P)/|d|`, which is more accurate than interpolating cell gradients ([[ref-jasak-1996-phd-thesis]] Eqn. 3.25).

**Non-orthogonal split.** Otherwise write `S = Delta + k` with `Delta` parallel to `d`. The orthogonal part `Delta . (grad phi)_f = |Delta| (phi_N - phi_P)/|d|` stays implicit; the correction `k . (grad phi)_f` uses the interpolated cell gradients from [[gradient-reconstruction]] and is treated explicitly ([[ref-jasak-1996-phd-thesis]] Eqns. 3.28-3.29). Three choices of `Delta` ([[ref-jasak-1996-phd-thesis]] Eqns. 3.30-3.34):
- **minimum correction:** `Delta` and `k` orthogonal, so the correction is as small as possible;
- **orthogonal correction:** the P-N contribution is kept as on an orthogonal mesh, `|Delta| = |S|`;
- **over-relaxed:** `Delta` grows with the non-orthogonality, so the implicit P-N part dominates as the angle increases.

The explicit correction "potentially creates unboundedness", and is updated by repeated solves ([[deferred-correction]]) ([[ref-jasak-1996-phd-thesis]] 3.3.1.3, 3.5).

**Which works best.** On a Laplace test on increasingly non-orthogonal meshes, the minimum correction approach diverges at 40 deg, the orthogonal correction stops converging at 45 deg, and the over-relaxed approach still converges at 65 deg with enough correctors ([[ref-jasak-1996-phd-thesis]] 3.7.4). Jasak concludes the over-relaxed form has the best stability and convergence "although it causes the largest discretisation error if the nonorthogonal correction is discarded" ([[ref-jasak-1996-phd-thesis]] 3.9).

**Discarding the correction.** If boundedness is essential the explicit correction is dropped, which gives a scheme obeying the sufficient boundedness criterion but creates a non-orthogonality error that violates the order of discretisation; of the three decompositions it is smallest for minimum correction ([[ref-jasak-1996-phd-thesis]] 3.5, 3.6). The effort to converge the correctors "can increase as much as an order of magnitude if mesh nonorthogonality is severe" ([[ref-jasak-1996-phd-thesis]] 3.7.4).

**Skewness.** If the P-N line meets the face at a point offset by a vector `m` from the face centre, linear interpolation gives the value at the wrong point, and the face integrals fall to first order; the skewness error is expressed through `m` and the gradient at the face ([[ref-jasak-1996-phd-thesis]] 3.6, Eqn. 3.129). Fixed-gradient boundary faces are first order for the same reason: `d` does not in general hit the face centre ([[ref-jasak-1996-phd-thesis]] 3.3.3.1).

## Builds on

- [[finite-volume-method]]: the face flux and the mesh vectors `d` and `S`.
- [[gradient-reconstruction]]: the cell gradients used in the explicit correction.
- [[deferred-correction]]: the explicit treatment of the correction.

## Leads to

- rcfd pages: [[mesh-quality]], [[simple]], [[mesh-adaptation]]

## How rcfd uses it

- rcfd reports non-orthogonality as `atan2(|d x n|, d . n)` and skewness among its pass/fail mesh measures; fan-passage gates are typically 70 deg non-orthogonality and skewness 2.0, later 85 deg / 4.0 ([[mesh-quality]]).
- **Implicit non-orthogonal pressure:** the opt-in `SimpleSolver::implicit_non_orth_pressure` (F-285) solves the non-orthogonal part of the pressure equation implicitly to converge the warped mesh W, where the explicit (deferred) step diverges ([[simple]]; [[mesh-quality]]).
- `GradientOptions` change the explicit non-orthogonal (secondary) part of `-div(gamma grad phi)` with a face correction, a secondary limiter and a boundary rule; "no benefit is claimed" ([[doc-case-schema-gradient-options]]).
- Setup rule SV302 combines aspect ratio and tan(non-orthogonality) for wall cells with unlimited `linear_upwind` ([[setup-rules]]).
