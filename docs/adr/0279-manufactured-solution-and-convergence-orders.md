# ADR 0279: Verification by a manufactured solution: observed order of displacement and stress for the three elements

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-9 of the structural solver, basic tier. Size M.

## Goal

A smooth displacement field on the unit cube, its body force from the analytic divergence of the stress, and the observed order
of convergence of the displacement error (L2) and of the stress error (energy norm) under uniform refinement, for each element.
Tests only: no library file is edited; a defect found in a library file is reported, not patched here.

## Dependencies

ADR 0276, ADR 0277; existing roadmap items: none.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Manufactured solution: a smooth trigonometric displacement
field on the unit cube with nu = 0.3, the body force from the analytic divergence of the stress (checked against finite
differences within 1e-6), displacements prescribed on all faces, and a second case with tractions on three faces. On at least
three meshes, each halving h, the observed order of the L2 displacement error over the finest pair is >= 1.8 for Tet4 and Hex8
and >= 2.7 for Tet10, the observed order of the energy-norm error is >= 0.9 and >= 1.8, and both errors fall monotonically.
Negative control: a body force with the sign of the lambda term reversed gives an order below 0.5.

## Reference to check against

The method of manufactured solutions (Roache, Verification and Validation in Computational Science and Engineering, 1998; from
memory); a-priori estimates of the finite element method: order p + 1 in L2 and p in the energy norm for complete polynomials of
degree p (Strang and Fix, An Analysis of the Finite Element Method, 1973).

## Files

`crates/rcfd-solid/tests/sb_mms.rs` only.

## What could make it miss

Meshes small enough for the direct solver (about 1.5e4 unknowns per run) may still be pre-asymptotic for Tet10; the error norms
need a quadrature of higher degree than the stiffness, or the measured order is polluted.
