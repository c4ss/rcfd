# ADR 0277: Stress and strain recovery: integration-point, element and nodal values; von Mises and principal stresses

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-7 of the structural solver, basic tier. Size S.

## Goal

From a displacement field: strain and stress at the integration points, element means, nodal values by extrapolation and
volume-weighted averaging; von Mises stress, principal stresses with their directions, and the strain energy. The six components
are stored in the order xx, yy, zz, xy, yz, zx with engineering shear strains (fixed by ADR 0270 section 2.2).

## Dependencies

ADR 0275; existing roadmap items: none.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Strain and stress at integration points, element means and
nodal averages; von Mises stress, principal stresses with directions, strain energy. For 1000 random symmetric tensors
(registered seed) the principal values from the routine agree with an independent method (Jacobi rotations against the closed
trigonometric form) within 1e-12 of the tensor norm, are ordered, have orthonormal directions within 1e-12 and are unchanged by
a random rotation of the tensor within 1e-12; von Mises equals |s| in uniaxial stress, sqrt(3) tau in pure shear and 0 for a
hydrostatic state within 1e-14 of the norm; a tensor with repeated roots gives finite values. On the patch test of the static
solve every recovered stress equals D eps within 1e-10; the strain energy equals half the work of the loads within 1e-10 for a
uniaxial and a bending load.

## Reference to check against

Closed forms: invariants of a symmetric 3 x 3 tensor; Clapeyron's theorem (strain energy = half the work of the loads in linear
elasticity).

## Files

`crates/rcfd-fem/src/recover.rs`, `crates/rcfd-fem/tests/sb_recover.rs`.

## What could make it miss

Loss of accuracy of the trigonometric form near repeated roots; nodal averaging across a material interface (basic tier: one
material per averaging region, otherwise refused). Superconvergent patch recovery and error estimation are intermediate.
