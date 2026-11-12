# ADR 0302: Harmonic response by mode superposition: modal damping, static correction, effective mass, force harmonics

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-12 of the structural solver, intermediate tier. Size M.

## Goal

The steady response to loads `Re(f e^{i omega t})` from the modes of ADR 0284: modal damping ratios, a static correction for the
truncated modes, a sweep over frequencies, amplitude and phase of displacement and stress at chosen nodes; modal participation
factors and effective masses; and the Fourier harmonics of a periodically sampled force history, which is how the unsteady blade
loads of fan breakdown item E3 (blade-passing forcing) become harmonic loads.

## Dependencies

ADR 0291; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-1, F-374, F-383.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A load proportional to M phi_1 at omega_1 gives the amplitude
of the static response divided by 2 zeta within 1e-8 and a phase lag of 90 degrees within 1e-6 rad; the half-power width of that
peak is 2 zeta omega_1 within 1 percent (zeta = 0.01). Fixed-free bar with nu = 0 under a harmonic tip force, undamped, at 0.5 and
1.5 times the first frequency: tip amplitude F tan(k L)/(E A k), k = omega/c, within 0.5 percent with 10 modes and the static
correction, the same without the correction being the registered control (error above 1 percent at 0.5). Effective mass: the first
axial mode of the bar carries 8/pi^2 of the mass within 0.5 percent, the first bending mode of a cantilever 0.613 (recomputed in
the registration) within 1 percent, and the sum over all modes of a small model equals the unconstrained mass within 1e-10. Force
harmonics of a sampled signal with three harmonics: amplitudes and phases within 1e-12; a record that is not a whole number of
periods is refused.

## Reference to check against

Closed forms: the damped oscillator at resonance and its half-power width; the bar under a harmonic end force (wave equation, one
line); modal expansions of the bar and the Euler-Bernoulli cantilever for the effective masses; the discrete Fourier sum. Method:
mode superposition with the mode-acceleration (static) correction (Craig and Kurdila, Fundamentals of Structural Dynamics; from
memory; derivable from the spectral expansion of the inverse of K).

## Files

`crates/rcfd-fem/src/harmonic.rs`, `crates/rcfd-fem/tests/si_harmonic.rs`.

## What could make it miss

Modal damping is an input: no number for a moulded plastic blade is in the repository, so every response level scales with an
assumed ratio. A direct (full) harmonic solve and damped or complex modes are not included (advanced). With pre-stress the modes
of ADR 0299 can be passed in; that pairing is checked only in the demonstration.
