# ADR 0330: Frictionless contact: node-to-surface with an augmented Lagrangian, rigid and deformable targets, interference fits

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-20 of the structural solver, advanced tier. Size M.

## Goal

Unilateral contact as an extra contribution to the Newton driver: the search of ADR 0329 repeated as the state moves, a gap per
node of the contactor against a deformable surface or a rigid plane, cylinder or sphere, a penalty with augmentation of the
multipliers until the registered penetration is met, an initial gap or interference, and contact pressure as a result.

## Dependencies

ADR 0329; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-19, F-397.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A bar pushed against a rigid stop across a gap g: the
piecewise-linear force-displacement closed form within 1e-8, the penetration after augmentation below 1e-8 L. Two stacked blocks
with matching meshes under pressure p: uniform stress and contact pressure p within 1e-8; pulled apart they separate with no
tensile contact force (every multiplier >= 0). Elastic sphere on a rigid plane: contact radius (3 F R/(4 E*))^(1/3), peak pressure
3 F/(2 pi a^2) and approach a^2/R within 5, 5 and 3 percent at a/R = 0.1 with at least ten nodes across the contact radius.
Interference fit of a ring (outer radius b) on a solid shaft (radius a) of the same material with radial interference d, plane
stress: contact pressure E d (b^2 - a^2)/(2 a b^2) within 2 percent, and the hoop stress of the ring that of Lame under that
pressure within 2 percent. The contact tangent agrees with central differences within 1e-6 in a fixed active set; the last
residuals of a step with a settled active set show an order >= 1.8; an active set that cycles beyond the registered count is an
error with the nodes.

## Reference to check against

Closed forms: springs with a gap; Hertz's sphere on a plane (Johnson, Contact Mechanics, chapter 4; from memory; derivable); the
shrink fit from Lame's cylinder (one line, re-derived in the registration). Method: penalty with augmentation of the multipliers
(Simo and Laursen, Comput. Struct. 42 (1992) 97; Wriggers, Computational Contact Mechanics; from memory).

## Files

`crates/rcfd-fem/src/contact.rs`, `crates/rcfd-fem/tests/sa_contact.rs`.

## What could make it miss

Hertz with node-to-surface contact converges slowly and quadratic elements give uneven nodal contact forces: the percentages are
drafts and the check uses linear hexahedra. Friction, self-contact and contact in dynamics are not planned.
