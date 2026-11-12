# ADR 0312: Orthotropic elasticity with material axes, orthotropic expansion, and temperature-dependent properties from tables

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-2 of the structural solver, advanced tier. Size M.

## Goal

The material of a moulded, fibre-filled blade is not isotropic and softens with temperature. Add linear orthotropic elasticity
from nine constants with material axes per element set (fixed axes, cylindrical about an axis, or one triad per element as data),
orthotropic thermal expansion, transversely isotropic input as a shorthand, and properties tabulated against temperature
(piecewise linear E, nu, G, alpha, evaluated at the integration point from the nodal temperature field of ADR 0281, the thermal
strain being the integral of alpha from the reference temperature).

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-372, F-374, F-376, F-380.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Orthotropy: with isotropic constants the elasticity matrix
equals the isotropic one within 1e-13 relative; constants that are not positive definite are refused naming the constant. Off-axis
tension of a block with the material axes turned by 15, 30, 45 and 60 degrees about z: axial strain sigma/E_x with 1/E_x = c^4/E1
+ s^4/E2 + (1/G12 - 2 nu12/E1) s^2 c^2 and the shear-extension ratio from the rotated compliance within 1e-10; rotating the model
and the axes together leaves the strain energy unchanged within 1e-12. Cantilever of slenderness 20 with E1 along the span and E2
= E1/10: tip deflection P L^3/(3 E1 I) + P L/(kappa G13 A) within 1 percent. Free heating gives the strains alpha_i dT in material
axes with zero stress, full restraint the stress -D alpha dT, both within 1e-10. Cylindrical axes: a tube b/a = 2 with E_theta/E_r
= 4 under internal pressure in plane stress has the bore hoop stress of the polar-orthotropic closed form (powers r^(k-1) and
r^(-k-1), k = sqrt(E_theta/E_r), derived in the registration) within 1 percent. Tables: a bar whose temperature and so its modulus
vary linearly from E0 to E1 along it stretches under an end force by F L ln(E1/E0)/(A (E1 - E0)) within 0.1 percent with 20
elements; with alpha = a0 + a1 T the free expansion is a0 (T - Tref) + a1 (T^2 - Tref^2)/2 within 1e-10; a constant table gives
the bits of the constant material; a temperature outside a table is refused with the element and the temperature.

## Reference to check against

Closed forms: the rotated compliance of an orthotropic sheet (tensor rotation; Jones, Mechanics of Composite Materials, from
memory); the polar-orthotropic tube (Lekhnitskii, Theory of Elasticity of an Anisotropic Body; re-derived from equilibrium and
compatibility in the registration); the bar with a varying modulus (one integral).

## Files

`crates/rcfd-solid/src/mat_ortho.rs`, `crates/rcfd-solid/src/mat_table.rs`, `crates/rcfd-solid/tests/sa_ortho.rs`,
`crates/rcfd-solid/tests/sa_mat_table.rs`.

## What could make it miss

If the element routines of the basic tier evaluate the material once per element, properties that vary inside an element need the
routine opened (the scaffold's point context is meant to prevent that). The constants of a real moulded blade and its fibre
directions are not in the repository: they are inputs (decision A1). Split if needed: orthotropy first, tables second.
