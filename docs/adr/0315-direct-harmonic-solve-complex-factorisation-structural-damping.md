# ADR 0315: Direct harmonic solve: complex symmetric factorisation, structural (hysteretic) damping, discrete dampers, base excitation

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-5 of the structural solver, advanced tier. Size M.

## Goal

Mode superposition (ADR 0302) cannot take damping that differs between materials or depends on frequency. Add the direct solve of
(K (1 + i eta) - w^2 M + i w C) u = f per frequency: a complex symmetric L D L^T on the envelope in `rcfd-linalg`, loss factors
per material (constant, or a table of storage modulus and loss factor against frequency, which is how the complex modulus of ADR
0313 enters), Rayleigh and discrete viscous dampers, pre-stress as an added stiffness, harmonic base motion, a sweep with
amplitude and phase of displacement and stress.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-373, F-374, F-383, F-399.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. rcfd-linalg: a random complex symmetric diagonally dominant
matrix of order 200 is solved to a residual of 1e-12 relative; a real matrix gives the real factorisation's solution within 1e-13;
a pivot below the registered threshold is refused with the equation. Fixed-free bar, nu = 0, loss factor 0.02, tip force: tip
response F tan(k L)/(E* A k) with E* = E (1 + i eta) and k = w sqrt(rho/E*), amplitude and phase within 0.5 percent at 0.5, 1.0
and 1.5 times the first frequency. With Rayleigh damping the direct solve equals the mode superposition of F-399 with all modes
within 1e-8 on a small model. Tip dashpot c on the same bar: response F/(E A k cot(k L) + i w c) within 0.5 percent, and with c =
A sqrt(E rho) the amplitude is F |sin(k L)|/(E A k) (no resonance) within 0.5 percent at five frequencies. Base motion of the
clamped end, undamped: tip amplitude 1/cos(k L) of the base within 0.5 percent at half the first frequency. A table of modulus
against frequency reproduces the constant-modulus result when constant (1e-12). An undamped solve at a resonance is refused with
the frequency; results are bit-identical for 1 and 8 threads.

## Reference to check against

Closed forms: the wave equation of a bar with a complex modulus, with an end dashpot and with a moving base (one line each). The
factorisation is Gaussian elimination in symmetric form with complex numbers (no pivoting; stable here because the damping makes
the imaginary part definite: argued in the registration; Higham, Accuracy and Stability of Numerical Algorithms, on complex
symmetric systems, from memory).

## Files

`crates/rcfd-linalg/src/cskyline.rs`, `crates/rcfd-fem/src/harmonic_direct.rs`, `crates/rcfd-linalg/tests/sa_cskyline.rs`,
`crates/rcfd-fem/tests/sa_harmonic_direct.rs`.

## What could make it miss

Without pivoting an undamped system close to a resonance loses digits: the refusal threshold is a registered estimate. One
factorisation per frequency: a sweep of a large model is slow (recorded). Constraints by the transformation of ADR 0296 must carry
complex right-hand sides.
