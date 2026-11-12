# ADR 0284: Mass matrix and linear modal analysis of a solid model

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-14 of the structural solver, basic tier. Size M.

## Goal

Consistent mass matrix of the three elements from the density, assembled like the stiffness; the lowest natural frequencies and
mass-normalised mode shapes of a constrained or free model with the eigensolver of ADR 0283. This is the modal analysis of fan
breakdown item E5 for a blade at rest (no rotation effects).

## Dependencies

ADR 0275, ADR 0283; existing roadmap items: none.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Consistent mass matrix for Tet4, Hex8 and Tet10: for a box,
the total mass, the centre of mass and the inertia tensor obtained from rigid motions r^T M r equal the closed forms within
1e-12. Modal analysis: a free box has six frequencies below 1e-6 of the seventh; a fixed-free bar with nu = 0 has f_n = (2n - 1)
sqrt(E/rho)/(4 L), n = 1, 2, 3, within 0.5 percent on a registered mesh, and the error of f_1 falls at an observed order >= 1.8
with Hex8; a cantilever of slenderness 20 and square section has its first bending frequency within 1 percent of (1.8751)^2
sqrt(E I/(rho A L^4))/(2 pi) and its second (4.6941) within 2.5 percent with Tet10, the Euler-Bernoulli value less the shear and
rotary-inertia correction estimated in the registration (0.15 and 0.9 percent); on a symmetric Hex8 mesh the two bending planes
give the same frequency within 1e-6 relative; mode shapes are M-orthonormal within 1e-9.

## Reference to check against

Closed forms: the wave equation of a bar (`f_n = (2n - 1) c/(4L)`, exact for a 3-D bar with nu = 0); Euler-Bernoulli cantilever,
roots of `cos x cosh x + 1 = 0` (1.8751, 4.6941, 7.8548: to be recomputed in the registration, one line of arithmetic); the
correction for shear and rotary inertia from Timoshenko beam theory (Timoshenko, Phil. Mag. 41 (1921) 744; from memory).
Rigid-body inertia of a box.

## Files

`crates/rcfd-solid/src/{mass.rs, modal.rs}`, `crates/rcfd-solid/tests/sb_modal.rs`.

## What could make it miss

Size: 3-D solid elements on a slender beam need many unknowns for the second mode within the direct solver's reach. Torsional
and bending modes interleave, so modes must be identified by shape, not by index. Rotating-blade effects (spin softening, stress
stiffening, Campbell diagram) are intermediate.
