# ADR 0317: Damped and complex modes: a dense eigensolver for general real matrices and the quadratic eigenproblem on a modal subspace

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-7 of the structural solver, advanced tier. Size M.

## Goal

Everything with a skew or damping matrix needs eigenvalues that the symmetric solver of ADR 0283 cannot give. Add to `rcfd-linalg`
the eigenvalues and vectors of a dense real general matrix (balancing, Hessenberg reduction, shifted QR), and on it the quadratic
problem (lambda^2 M + lambda (C + G) + K) x = 0 projected on a set of undamped modes (2m by 2m). In `rcfd-fem`: damped modes
with Rayleigh damping, discrete dampers and loss factors, giving decay rates, damped frequencies, damping ratios and complex
shapes. The gyroscopic packages (ADR 0318, 0319) use the same routine.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-382, F-383.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. rcfd-linalg: the companion matrix of a polynomial with
registered real and complex roots returns them within 1e-10; a random matrix of order 50 has residuals |A v - lambda v| <= 1e-10
|A|; a symmetric matrix reproduces the Jacobi values of F-382 within 1e-10; a Jordan block is returned with its eigenvalue within
1e-6 and a report, not a failure; no convergence within the registered sweeps is an error. One oscillator: lambda = -zeta w +/- i
w sqrt(1 - zeta^2) within 1e-12. Rayleigh damping a M + b K on a solid model: every mode has the ratio (a/w_j + b w_j)/2 within
1e-8. Skew coupling: a two-unknown system with G = g [[0, 1], [-1, 0]] has the closed-form split frequencies within 1e-12.
Fixed-free bar with a tip dashpot c below A sqrt(E rho): lambda_j = (c0/L) (-atanh(c/(A sqrt(E rho))) + i (2 j - 1) pi/2), the
first three within 2 percent on 40 modes; at c above that value no oscillating root is returned and the result says so. Damping
ratios are -Re(lambda)/|lambda|; conjugate pairs are reported once.

## Reference to check against

Closed forms: the damped oscillator; proportional damping; the bar with a boundary dashpot (wave equation with the condition E A
u_x = -c u_t, giving tanh(lambda L/c0) = -A sqrt(E rho)/c). Method: Francis's QR iteration on a Hessenberg matrix (Golub and Van
Loan, Matrix Computations, chapter 7; from memory; checked by the residual test, not taken on trust).

## Files

`crates/rcfd-linalg/src/eigen_general.rs`, `crates/rcfd-fem/src/damped.rs`, `crates/rcfd-linalg/tests/sa_eigen_general.rs`,
`crates/rcfd-fem/tests/sa_damped.rs`.

## What could make it miss

A boundary dashpot converges slowly in undamped modes (the 2 percent is a draft; residual vectors may be needed). The QR iteration
has known hard cases; the test matrices are registered, not chosen after a run. Large models are reached only through the modal
subspace, never directly.
