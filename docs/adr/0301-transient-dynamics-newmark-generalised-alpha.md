# ADR 0301: Linear transient dynamics: Newmark and generalised-alpha integration, Rayleigh damping, load histories

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-11 of the structural solver, intermediate tier. Size M.

## Goal

Time integration of `M a + C v + K u = f(t)` for a linear model: the average-acceleration Newmark method and the generalised-alpha
method with the high-frequency spectral radius as its one parameter, Rayleigh damping `C = a M + b K`, loads scaled by tabulated
histories, initial displacement and velocity, one factorisation reused over the steps, and a state that can be saved and restored
bit for bit (the co-simulation participant of ADR 0303 needs that).

## Dependencies

ADR 0291; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-1, F-373, F-374, F-383.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Free vibration started in the first axial mode of a bar (the
discrete eigenvector): with Newmark the amplitude after 100 periods is within 1e-9 and the total energy constant within 1e-10; the
measured period is long by (omega dt)^2/12 within 5 percent of that amount at omega dt = 0.2 and 0.1, observed order >= 1.9. A
step load proportional to M phi_1 gives the peak displacement twice the static one within 0.5 percent at 40 steps per period.
Rayleigh damping with a registered ratio 0.02 on that mode: logarithmic decrement 2 pi zeta/sqrt(1 - zeta^2) within 1 percent.
Generalised-alpha: with spectral radius 1 the result equals Newmark within 1e-12; with 0.5 and 0.8 a mode at omega dt = 1e3 decays
per step by that factor within 1e-3 while the period error of the first mode keeps order >= 1.9. A run split in two by save and
restore has the bits of the unsplit run; results are bit-identical for 1 and 8 threads; a non-positive step, a table that does not
cover the run and a singular effective matrix are refused.

## Reference to check against

Closed forms: the single oscillator (free, damped, step-loaded); the period error of the average-acceleration method, from
tan(omega_h dt/2) = omega dt/2 (derivable). Methods, from memory: Newmark, J. Eng. Mech. Div. ASCE 85 (1959) 67; Chung and
Hulbert, J. Appl. Mech. 60 (1993) 371 (the parameters in terms of the spectral radius are re-derived or checked by the decay test,
not taken on trust).

## Files

`crates/rcfd-solid/src/transient.rs`, `crates/rcfd-solid/tests/si_transient.rs`.

## What could make it miss

Starting accelerations need a solve with M; a constrained model must do it on the reduced system. The decay test at omega dt = 1e3
needs a mode that is isolated in the discrete model (a two-unknown model is used). Nonlinear transient (Newton inside the step) is
not included.
