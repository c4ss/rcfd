# ADR 0332: Fracture parameters: J-integral by the domain form, stress intensity factor, crack-growth life

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-22 of the structural solver, advanced tier. Size M.

## Goal

Given a crack in the mesh (a slit of doubled nodes with a straight front), the energy release rate along the front by the
equivalent domain integral over rings of elements, the mode-I stress intensity factor from it and from the opening displacement
behind the front, and the number of cycles for a crack to grow between two lengths by the Paris law. A builder makes a plate with
a slit. Stationary cracks in linear elasticity only.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-374, F-376, F-389.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Centre crack of half-length a in a plate of half-width W = 10
a under tension, plane strain (one layer, held in z), Hex20: K_I from J within 2 percent of sigma sqrt(pi a) sqrt(sec(pi a/(2
W))), the values of rings 2 to 4 within 1 percent of each other, and J equal to the change of potential energy between two models
with half-lengths a +/- 0.02 a within 2 percent. K_I from the opening displacement within 5 percent. Penny-shaped crack of radius
a in a cylinder of radius 10 a under tension (heavy record): K_I = 2 sigma sqrt(a/pi) within 3 percent, uniform along the front
within 2 percent. Under pure shear loading of the centre crack the mode-I factor from the opening is zero within 1e-8 of the
tension value. Growth: the cycles from a0 to a1 by the Paris law with K = d sigma sqrt(pi a) equal the closed-form integral for m
= 3 and the logarithm for m = 2 within 1e-6. A front that is not straight within the registered tolerance and a ring that reaches
the boundary are refused.

## Reference to check against

Closed forms: the centre crack in an infinite plate with the secant width correction (Feddersen's form, an approximation whose
error at this ratio is below the limit: stated and bounded in the registration), the penny-shaped crack (Sneddon, 1946), the
identity J = -dPi/da. Method: Li, Shih and Needleman, Eng. Fract. Mech. 21 (1985) 405; Shih, Moran and Nakamura, Int. J. Fract. 30
(1986) 79 (from memory; checked by path independence and the energy difference).

## Files

`crates/rcfd-solid/src/fracture.rs` (with the slit builder), `crates/rcfd-solid/tests/sa_fracture.rs`.

## What could make it miss

The mesh validation of the basic tier may refuse coincident nodes, which a slit needs. Curved fronts, mixed-mode separation, crack
growth with remeshing and elastic-plastic fracture are not planned. Toughness and growth constants are inputs.
