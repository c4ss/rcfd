# ADR 0320: Random vibration: response to force and base-acceleration spectra by modal superposition, root-mean-square stress

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-10 of the structural solver, advanced tier. Size S.

## Goal

Broadband aerodynamic loading is a spectrum, not a harmonic. From the modes and the modal transfer functions of ADR 0302: the
response spectral density at chosen nodes for a load pattern with a force spectrum (or a few patterns with a cross-spectral
matrix) and for a base acceleration spectrum; mean squares by integration on a grid refined at the peaks; root-mean-square
displacement, stress components and von Mises stress; spectral moments and the zero-crossing rate, which the narrow-band fatigue
estimate of ADR 0316 takes.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-399.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. One mode under a flat one-sided force spectrum G: mean-square
displacement (pi/2) f_n Q G/k^2 with Q = 1/(2 zeta), and under a flat base acceleration spectrum the mean-square absolute
acceleration (pi/2) f_n Q G (for Q well above 1; the exact integral is computed in the registration), both within 0.5 percent at
zeta = 0.02 on the registered grid, observed convergence under grid refinement; zero-crossing rate f_n within 1 percent. Two well
separated modes: the mean square is the sum within 1 percent. A band-limited spectrum equals the numerical integral registered
beforehand within 1e-6. For a uniaxial stress state the root-mean-square von Mises stress equals the root-mean-square stress
within 1e-10. Two fully correlated patterns equal one summed pattern within 1e-12; uncorrelated ones add in mean square. A
spectrum with a negative value or a non-increasing frequency list is refused with the entry.

## Reference to check against

Closed form: the integral of |H|^2 of a single oscillator over frequency (residues; Crandall and Mark, Random Vibration in
Mechanical Systems; Miles, J. Aeronaut. Sci. 21 (1954) 753; from memory, derived). The root-mean-square von Mises stress as a
quadratic form of the stress covariance (first principles).

## Files

`crates/rcfd-fem/src/random.rs`, `crates/rcfd-fem/tests/sa_random.rs`.

## What could make it miss

No force spectrum of the fan exists (broadband loads need a scale-resolving flow run); the package is machinery. Response spectra
(shock) are not included (ADR 0310, not planned).
