# ADR 0297: Cyclic symmetry: one sector of a rotor for statics and for modal analysis by harmonic index

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-7 of the structural solver, intermediate tier. Size M.

## Goal

A bladed disc of N identical sectors is solved on one sector. Statics under a load that repeats in every sector: the high boundary
equals the low boundary rotated by 2 pi/N, a real linear constraint (ADR 0296). Modal analysis for each harmonic index k = 0 ..
N/2: the boundaries differ by the phase k 2 pi/N, solved as a real problem of twice the size, giving single modes for k = 0 and
N/2 and pairs otherwise, expanded to the full rotor for output. The two boundary surface sets must match node for node under the
rotation.

## Dependencies

ADR 0296; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-6, F-375, F-383.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Equivalence: for a ring and for a disc with N = 12 sectors,
the frequencies of the sector for every harmonic index 0 to 6 equal the frequencies of the full 360 degree model built from the
same sector mesh within 1e-8 relative, with the same count below a registered cut-off (single for k = 0 and 6, double otherwise);
the expanded mode shapes satisfy the full model's eigen-equation within 1e-7. Closed forms: in-plane bending of a thin ring
(thickness/radius 0.05, nu = 0), omega_n^2 = E I n^2 (n^2 - 1)^2/(rho A R^4 (n^2 + 1)) for n = 2 and 3 found at harmonic index 2
and 3 within 2 percent; a thin annular disc b/a = 4 spinning about its axis (statics, k = 0): rim displacement rho omega^2 b ((3 +
nu) a^2 + (1 - nu) b^2)/(4 E) within 1 percent. Boundaries that do not match under the rotation within the registered tolerance
are refused with the first unmatched node; N < 2 and a harmonic index above N/2 are refused.

## Reference to check against

The full model is the reference for the equivalence (Bloch-Floquet decomposition of a rotationally periodic structure: Thomas,
Int. J. Numer. Meth. Engng 14 (1979) 81, from memory; derivable from the block-circulant structure of K and M). Closed forms: the
inextensional ring frequencies (Hoppe; Timoshenko, Vibration Problems in Engineering; re-derived by Rayleigh's method in the
registration) and the rotating annular disc in plane stress (Timoshenko and Goodier; re-derived).

## Files

`crates/rcfd-fem/src/cyclic.rs`, `crates/rcfd-fem/tests/si_cyclic.rs`.

## What could make it miss

The doubled real problem has every eigenvalue twice: the subspace must be large enough and the pairs must be recognised, not
reported as two modes. Nodes on the axis belong to both boundaries (refused in this package; the disc check is an annulus). The
full model of the equivalence check must stay within the direct solver's reach. Pre-stress and spin softening on a sector (a
rotating bladed disc) combine this package with ADR 0299 only in the demonstration (ADR 0309).
