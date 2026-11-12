# ADR 0339: Demonstration on the 120 mm fan: a polymer blade (orthotropy, temperature, creep, fatigue at a Campbell crossing) and the radiated power of the frame

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-29 of the structural solver, advanced tier. Size M.

## Goal

Bring the tier to what fan breakdown item E5 asks of a material choice: the blade of ADR 0309 with an orthotropic,
temperature-dependent polymer (assumed constants, named as assumed), its Campbell table with the Coriolis term, the creep growth
of the tip towards the housing at speed, the fatigue safety factor at a Campbell crossing under a unit blade-passing pressure
harmonic on the centrifugal mean stress, and a simplified frame (a square plate with the rotor opening on four mounts) driven by
the rotor's unbalance and blade-passing forces, with its equivalent radiated power. Consistency and mesh change only: nothing is
validated.

## Dependencies

ADR 0338; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-2, SA-4, SA-5, SA-6, SA-8, SA-11, SA-28, F-406.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Blade from solid case files. With isotropic constants and no
temperature the Campbell table has the bits of F-406. A uniform temperature at which the table gives 0.8 E scales every frequency
at rest by sqrt(0.8) within 1e-8. For the orthotropic blade the change of each squared frequency at rest for a 1 percent change of
E1 equals the Rayleigh-quotient sensitivity within 1 percent. With the Coriolis term the first four frequencies at the maximum
speed differ from those without by less than the registered bound, both recorded. Creep at the maximum speed over the registered
time: tip growth monotone, its initial rate within the registered factor of the one-dimensional estimate, change between two
meshes below 5 percent. Fatigue at a registered crossing: the safety-factor field is written, the largest alternating stress
changes by less than 10 percent between two meshes and lies in the same region, and doubling the pressure harmonic changes the
Goodman factor as the formula says within 1e-8. Frame: the equivalent radiated power spectrum at the rotation and blade-passing
frequencies is written and equals (rho c/2) sum A |v_n|^2 recomputed from the output within 1e-10. The page
docs/structural-solver.md lists every assumed input and states that no material datum, life, creep growth or sound level is
validated against a measurement.

## Reference to check against

Consistency only: the earlier demonstration (bits), the scaling of frequencies with the modulus, first-order perturbation, mesh
change. No reference value exists for the blade or the frame (docs/fan-project-breakdown.md, part G; decision A1).

## Files

`crates/rcfd-api/tests/sa_fan_polymer.rs`, `docs/structural-solver.md`, the E5 status text of `docs/fan-project-breakdown.md`.

## What could make it miss

It inherits the risks of ADR 0288 and ADR 0309 (the blade mesh). The repository has no frame geometry (fan breakdown A1 is
partial), so the frame is a stand-in. Every engineering number scales with assumed material data.
