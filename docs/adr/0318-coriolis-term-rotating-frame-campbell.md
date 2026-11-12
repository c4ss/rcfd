# ADR 0318: Coriolis term of a rotating part: gyroscopic matrix in the rotating frame and the Campbell table with split modes

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-8 of the structural solver, advanced tier. Size M.

## Goal

ADR 0299 neglects the Coriolis coupling (decision N5 of ADR 0290). Add the skew matrix G = 2 Omega int rho N^T [e x] N dV of solid
elements and the modes of lambda^2 M + lambda G + K_eff(Omega) = 0 on the modal subspace (ADR 0317), opt-in beside the existing
table: frequencies over speed with the forward and backward members of each split pair, tracked, with the engine-order crossings.
The default table keeps its bits.

## Dependencies

ADR 0317; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-7, F-396.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. G is skew within 1e-13 of its norm and x^T G x = 0 within
1e-12. Without the option the table has the bits of F-396; with it at zero speed the frequencies equal those of F-396 within
1e-10. A mass on isotropic springs in the plane of rotation (two unknowns): frequencies |w0 - Omega| and w0 + Omega within 1e-10;
the same as a stiff block on elastic supports within 1e-3. Thin ring spinning about its axis (thickness/radius 0.05, nu = 0, full
model): the two travelling waves of in-plane mode n differ in frequency by 4 n Omega/(n^2 + 1) within 2 percent for n = 2 and 3 at
Omega = 0.1 w_2. Radial beam of square section: the flap frequency equals that of F-396 within 1e-6 relative and the lag frequency
moves by the registered second-order estimate within 20 percent of that estimate. Crossings of split modes are tracked without
exchange (modal assurance on the complex shapes above 0.9). A speed at which an eigenvalue has a positive real part is reported as
unstable with the speed.

## Reference to check against

Closed forms: the plane oscillator in a rotating frame; the split of the travelling waves of an inextensional ring (Bryan, Proc.
Camb. Phil. Soc. 7 (1890) 101; recalled as the factor 4 n/(n^2 + 1), re-derived in the registration from the ring's kinetic energy
in the rotating frame). The matrix follows from the relative-acceleration term 2 Omega x v (first principles).

## Files

`crates/rcfd-fem/src/gyro.rs`, `crates/rcfd-fem/tests/sa_gyro.rs`.

## What could make it miss

The ring needs the full 360 degree model: cyclic symmetry with the Coriolis term (complex sector problem with a skew part) is not
included, so a bladed disc gets the term only as a full rotor. Bryan's factor is recalled. The lag estimate of the beam is a
perturbation result with a wide draft limit.
