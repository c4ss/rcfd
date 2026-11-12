# ADR 0328: Hyperelastic materials (neo-Hookean, Mooney-Rivlin) with a mean-dilatation element for near incompressibility

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-18 of the structural solver, advanced tier. Size M.

## Goal

Rubber mounts and soft gaskets: finite-strain elasticity from a strain-energy function. Compressible neo-Hookean and Mooney-Rivlin
forms with a volumetric penalty, second Piola-Kirchhoff stress and material tangent on the total Lagrangian driver, and a
mean-dilatation formulation of Hex8 (constant pressure and volume ratio per element) so that a bulk modulus thousands of times the
shear modulus does not lock.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-390, F-397.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences within 1e-6; a
rotation of the deformed state leaves the energy unchanged within 1e-10; at a strain of 1e-6 the stress equals linear elasticity
with the same shear and bulk modulus within 1e-6 relative. With bulk/shear = 1e4: uniaxial nominal stress mu (lambda - lambda^-2)
(neo-Hookean) and 2 (C1 + C2/lambda)(lambda - lambda^-2) (Mooney-Rivlin) for stretches 0.7 to 3, equibiaxial mu (lambda -
lambda^-5), simple shear tau = mu gamma with the normal stress difference mu gamma^2, each within 0.1 percent after the registered
compressibility correction. Thick spherical shell B/A = 2, neo-Hookean, inflated to the inner stretch 1.5: pressure (mu/2)
((1/l_b^4 + 4/l_b) - (1/l_a^4 + 4/l_a)) with l_b^3 = 1 + (A/B)^3 (l_a^3 - 1) (recomputed by quadrature in the registration) within
1 percent with the mean-dilatation element, the standard Hex8 on the same mesh being the registered locking control. A
non-positive modulus is refused; an inverted element is an error with the element.

## Reference to check against

Closed forms of incompressible rubber elasticity: homogeneous deformations and the inflated sphere (Rivlin; Green and Zerna,
Theoretical Elasticity; Ogden, Non-linear Elastic Deformations; from memory, derivable from the strain energy). Method: the
three-field mean-dilatation element (Simo, Taylor and Pister, Comput. Meth. Appl. Mech. Engng 51 (1985) 177; from memory).

## Files

`crates/rcfd-fem/src/mat_hyper.rs`, `crates/rcfd-fem/src/nonlinear_md.rs`, `crates/rcfd-fem/tests/sa_hyper.rs`.

## What could make it miss

If the driver of ADR 0300 fixes the element's internal force to the standard displacement form, the mean-dilatation element needs
its own element routine (the scaffold's entry points allow it). Tetrahedra are not given a locking-free form. A true mixed
displacement-pressure element is not planned (ADR 0310).
