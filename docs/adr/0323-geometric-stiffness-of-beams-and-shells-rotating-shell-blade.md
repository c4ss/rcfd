# ADR 0323: Stress stiffening of beams and shells: buckling, pre-stressed modes and the Campbell table of a shell blade

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-13 of the structural solver, advanced tier. Size M.

## Goal

ADR 0298 gives geometric stiffness to solid elements only, so a blade meshed as a shell has no Campbell table. Add the geometric
stiffness of the beam (from its axial force) and of both shells (from the membrane resultants), their centrifugal load and
spin-softening mass, and pass them through the existing pre-stressed modal analysis, buckling and rotating-part analysis.

## Dependencies

ADR 0322; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-12, F-391, F-392, F-395, F-396.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Beam: Euler loads pi^2 E I/L^2 (pinned) and pi^2 E I/(4 L^2)
(clamped-free) with the registered shear correction within 0.1 percent at 8 elements; under tension w_n^2 = (n pi/L)^4 E I/(rho A)
+ (n pi/L)^2 T/(rho A) within 0.5 percent for n = 1, 2. Rotating radial beam of square section: w_flap^2 - w_lag^2 = Omega^2
within 1e-6 Omega^2 and the stiffening slope equal to the Rayleigh quotient of F-396 within 1 percent. Shell: a simply supported
square plate buckles at 4 pi^2 D/a^2 per width in one direction and at 2 pi^2 D/a^2 under equal compression in both, within 2
percent at 16 x 16 Shell4 and 4 percent at 32 x 32 x 2 Shell3; a stretched strip has the tension frequencies above within 1
percent. Equivalence: a flat plate strip twisted by 30 degrees, thickness/chord 0.02, clamped at the root and rotating: the
Campbell table of the first four modes as a shell equals that of the solid model of F-396 (Hex20) within 2 percent at the
registered speeds. Models of solid elements only keep their bits.

## Reference to check against

Closed forms: Euler and Engesser; the beam and the strip under tension; buckling of the simply supported plate (Timoshenko and
Gere, Theory of Elastic Stability; derivable from the Navier series with in-plane forces); the flap-lag identity and the
perturbation slope of ADR 0299. The solid model is the reference of the equivalence.

## Files

`crates/rcfd-fem/src/geometric_struct.rs`, `crates/rcfd-fem/src/rotating.rs` (only if its softening mass assumes solid
elements), `crates/rcfd-fem/tests/sa_geometric_struct.rs`.

## What could make it miss

At the upper end of M; split: beams first, shells second. The equivalence compares two discretisations of a thin plate, both with
their own error: the 2 percent is a draft. Large untwist is not covered (ADR 0327 gives modes about a nonlinear state for solids
only).
