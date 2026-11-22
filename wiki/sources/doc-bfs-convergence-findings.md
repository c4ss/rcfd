---
type: source
tags: [numerics, simple, convergence, analysis-record]
sources: []
origin: docs/bfs-convergence-findings.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Backward-facing step: why second-order SIMPLE did not converge (wave 6, F-106 / F-042)

**Origin:** [docs/bfs-convergence-findings.md](../../docs/bfs-convergence-findings.md) at `origin/main` 2e94e5ef
**Kind:** analysis record (143 lines)

## Summary

The steady SIMPLE solver stalled on the backward-facing step with second-order convection because the H operator was evaluated from the previous iterate (marginally stable lagged iteration), not because the flow was unsteady. The default now evaluates H from the solved momentum predictor. Status: fixed in wave 6; robustness limits at Re_h = 400 recorded.

## Key points

- Root cause: `SimpleSettings.h_from_solution` defaulted to `false`. With deferred-correction second-order schemes at cell Peclet ~50 (Re_h = 200, dx = 0.25 h) the iteration is unstable; first-order upwind hides it.
- Evidence: old default no convergence in 3000 iterations (residual 0.27-0.30); H from solved predictor converges in 646 (l_out 20); outlet, inlet profile, pressure reference, non-orthogonality, residual normalisation and the limiter ruled out.
- Fix: `h_from_solution` defaults to true ("classic SIMPLE / simpleFoam behaviour"); `false` still available. While a `MeanVelocityDriver` is active the lagged H is still used (the driver goes NaN with solved-predictor H; F-107 unchanged).
- Side effects: cavity Re 1000 plain SIMPLEC alpha_u 0.95 converges in 442 (was no convergence in 4154); SIMPLE 0.7/0.3 660 (was 1900). "The wave-5 premise ... came from this defect."
- Remaining limits at Re_h = 400 (120 x 16): vanLeer + SIMPLE 0.7/0.3 limit cycle; "the working recipe is vanLeer + SIMPLEC alpha_u 0.9" (696 iterations). No automatic setting added.
- k-epsilon wall-function oscillation (F-042, fixed): hard switch at y* = 11.225; `WallTreatment::Smooth` (default) and `Hysteresis` converge in 800 to 920 iterations; `Switch` never converges. Reattachment unchanged (< 0.003).
- Laminar reattachment x_r/h at Re_h 400: 11.24 / 11.67 / 11.70 / 11.75 with refinement, not mesh independent. Literature (Gartling 1990 about 12.2; Armaly et al. 1983 about 12) "NOT verified ... from memory".

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[simple]]
- [[rcfd-fv]]
- [[rcfd-turb]]
- [[unverified]]
