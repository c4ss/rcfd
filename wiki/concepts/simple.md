---
type: concept
tags: [numerics, pressure-velocity, steady]
sources: [doc-bfs-convergence-findings, doc-component-diagonal-default, doc-gpu, doc-meshing, adr-thread-steady-kernel-warped-mesh, adr-thread-gradient-options]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# SIMPLE / SIMPLEC

The steady segregated pressure-velocity coupling: solve a momentum predictor, then a pressure correction, with under-relaxation (SIMPLEC uses a consistent correction that allows weaker relaxation). In rcfd: `rcfd-fv` `SimpleSolver`, collocated with Rhie-Chow, case values `simple`, `simplec` ([[doc-bfs-convergence-findings]]).

## How rcfd uses it

- **H operator from the solved predictor.** `SimpleSettings.h_from_solution` defaults to true since wave 6 ("classic SIMPLE / simpleFoam behaviour"); the old lagged H (from the previous iterate) made the iteration "only marginally stable" and stalled the backward-facing step with second-order convection. `false` is still available; while a `MeanVelocityDriver` is active the lagged H is still used ([[doc-bfs-convergence-findings]]).
- **Effect:** lid-driven cavity Re 1000 with plain SIMPLEC alpha_u 0.95 converges in 442 iterations (was no convergence in 4154); SIMPLE 0.7/0.3 in 660 (was 1900) ([[doc-bfs-convergence-findings]]).
- **Remaining limit:** at Re_h = 400 a TVD limiter (vanLeer) with SIMPLE 0.7/0.3 limit-cycles; "the working recipe is vanLeer
  + SIMPLEC alpha_u 0.9" ([[doc-bfs-convergence-findings]]).
- **Component diagonal:** default `averaged`; with `corrected`, the correction is applied only in cells whose component diagonals are not bit-equal, so walls-only cases stay bit-identical. See [[component-diagonal]] ([[doc-component-diagonal-default]]).
- **GPU:** the pressure solve of `simple`, `piso`, `pimple` can run CG/BiCGStab on a CUDA device via `linear_solver.device` ([[doc-gpu]]). See [[rcfd-gpu]].
- **Adaptive SIMPLE loops** on directional lattices: `run_adaptive_simple_directional*` in `rcfd_fv::adaptive` ([[doc-meshing]]).
- **Warped mesh W (F-285):** the default steady kernel diverges on F-208's warped mesh W from `n = 32` because the deferred-correction pressure step has an eigenvalue of about `-1.18`; the opt-in `SimpleSolver::implicit_non_orth_pressure` (fix I-R, the non-orthogonal part solved implicitly) converges W at `n = 16, 32, 64` and leaves converged results elsewhere unchanged to `3e-12` ([[adr-thread-steady-kernel-warped-mesh]]).
- **Pressure-interpolation variants (F-246, partial):** `solver.pressure_interpolation` = `linear` (default), `skew_corrected`, `body_force_weighted`, steady kernel only ([[adr-thread-steady-kernel-warped-mesh]]).
- **Gradient options** on the energy, scalar, momentum, pressure, turbulence and electric-potential diffusion terms, opt-in ([[adr-thread-gradient-options]]). The VOF and coupled pressure solves use [[amg]].

## Related

- [[piso]]
- [[component-diagonal]]
- [[rcfd-fv]]
- [[mesh-adaptation]]

## Open questions

- Laminar BFS reattachment at Re_h 400 is not mesh independent (11.24 to 11.75), and the literature values compared with (Gartling 1990 about 12.2) are "from memory" ([[doc-bfs-convergence-findings]]).
