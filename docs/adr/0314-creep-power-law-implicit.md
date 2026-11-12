# ADR 0314: Creep: power law with time hardening, implicit integration with a consistent tangent

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-4 of the structural solver, advanced tier. Size S.

## Goal

A plastic blade under centrifugal load grows slowly towards the housing. Add deviatoric creep with the rate A sigma_e^n t^m
(Norton for m = 0; an Arrhenius factor in temperature as input), integrated implicitly at the integration point with its
consistent tangent, driven by the Newton driver of ADR 0300 over time steps limited by a registered creep-strain increment.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-397.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences within 1e-6.
Uniaxial constant stress: creep strain A sigma^n t within 1e-8 for m = 0 and A sigma^n t^(m+1)/(m+1) within 0.1 percent for m =
-0.5. Relaxation at fixed strain, n = 3 and 5: sigma(t) = (sigma0^(1-n) + (n - 1) E A t)^(1/(1-n)) within 0.5 percent at the
registered step count, observed order >= 0.9. Thick cylinder b/a = 2 in plane strain under internal pressure, n = 4: the stress
starts at Lame within 1 percent and tends to the stationary field sigma_r = -p ((b/r)^(2/n) - 1)/((b/a)^(2/n) - 1), sigma_theta =
p ((2 - n)/n (b/r)^(2/n) + 1)/((b/a)^(2/n) - 1) within 2 percent after the registered redistribution time. Radial bar clamped on
the axis of rotation, n = 3: the tip creeps at the rate A times the integral of (rho Omega^2 (L^2 - r^2)/2)^n over the length
within 0.5 percent (the stress is statically determinate). A step whose creep-strain increment exceeds the registered limit is
halved; a non-converged local iteration is an error with the element.

## Reference to check against

Closed forms: uniaxial integrals of the creep law; stationary creep of a cylinder, which is nonlinear elasticity with exponent 1/n
(Bailey; Finnie and Heller, Creep of Engineering Materials; from memory, re-derived in the registration and equal to Lame at n =
1); the rotating bar by one integral.

## Files

`crates/rcfd-fem/src/mat_creep.rs`, `crates/rcfd-fem/tests/sa_creep.rs`.

## What could make it miss

The redistribution time of the cylinder is an estimate (ratio of elastic strain to creep rate). Primary creep of polymers is often
better fitted by other laws; only this one is offered. No creep constant of any polymer is in the repository.
