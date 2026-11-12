# ADR 0273: Element library: linear tetrahedron, trilinear hexahedron, quadratic tetrahedron; isotropic elasticity matrix

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-3 of the structural solver, basic tier. Size M.

## Goal

Shape functions and their derivatives, quadrature rules, the strain-displacement matrix and the element stiffness `K_e = sum_q
w_q B^T D B det J` for Tet4 (one point), Hex8 (2 x 2 x 2 Gauss, full integration) and Tet10 (a rule exact to degree 2 for
stiffness; a degree-4 rule is added here for the mass matrix of ADR 0284), and the isotropic matrix `D` from `E` and `nu`.
Behind an `Element` trait with the entry points later tiers need (section 2.4 of ADR 0270).

## Dependencies

ADR 0271; existing roadmap items: none.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. For Tet4, Hex8 (2 x 2 x 2 Gauss) and Tet10, on the
reference element and on randomly distorted ones (registered seed): the shape functions sum to 1 and their derivatives to 0
within 1e-14 at 100 random points; the analytic derivatives agree with central differences within 1e-7; each quadrature rule
integrates every monomial up to its stated degree within 1e-13; the element stiffness is symmetric within 1e-13 of its norm,
gives no force for the six rigid-body motions within 1e-11 relative and has rank n - 6; each of the six unit strain states gives
the strain energy V/2 eps:D:eps within 1e-12 relative; D gives stress E eps in uniaxial stress and G gamma in shear within
1e-14.

## Reference to check against

First principles: Hooke's law; exact integrals of monomials over the reference tetrahedron, `a! b! c! / (a + b + c + 3)!`, and
over the cube. Method texts (cited from memory, to be confirmed by the implementer): Hughes, The Finite Element Method (1987);
Zienkiewicz and Taylor, The Finite Element Method, vol. 1. Quadrature points and weights are derived or checked by the monomial
test, never taken on trust.

## Files

`crates/rcfd-solid/src/{element.rs, quadrature.rs, material.rs}`, `crates/rcfd-solid/tests/sb_elements.rs`.

## What could make it miss

A tetrahedral rule recalled with wrong weights (the monomial check catches it); the rank check on a badly shaped element;
node-order convention of Tet10 differing from the mid-edge upgrade of ADR 0271.
