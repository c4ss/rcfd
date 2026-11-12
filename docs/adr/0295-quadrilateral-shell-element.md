# ADR 0295: Four-node shell element (flat facet, assumed shear strain): plates, housings and thin blades as surfaces

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-5 of the structural solver, intermediate tier. Size M.

## Goal

A four-node quadrilateral shell with six unknowns per node: bilinear membrane, Reissner-Mindlin bending with the transverse shear
strain interpolated from the edge mid-points (free of shear locking in the thin limit), a small registered stiffness on the
drilling rotation, a rigid offset for warped facets, consistent mass with rotary inertia, pressure and body loads, and results as
stress resultants and surface stresses. For housings and for blades meshed as mid-surfaces.

## Dependencies

ADR 0294; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-4.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Membrane patch test and constant-curvature bending patch test
on a distorted flat mesh within 1e-8. Simply supported square plate under uniform load, 16 x 16 elements, nu = 0.3: centre
deflection within 1 percent of the Navier series (0.004062 q a^4/D, recomputed in the registration) at thickness/side 0.01 and
within 1.5 percent at 0.001 (no shear locking), the fully integrated shear control being below 20 percent at 0.001. Cantilever
strip, nu = 0: tip deflection P L^3/(3 E I) within 0.5 percent and surface stress 6 M/t^2 at mid-span within 2 percent. Curved: a
ring strip of radius R pinched by two opposite forces, nu = 0, 32 elements per quarter: change of the loaded diameter (pi/4 -
2/pi) P R^3/(E I) within 2 percent; a cylinder under internal pressure with free ends: radial displacement p R^2/(E t) within 0.5
percent. Frequencies of the simply supported plate: omega_mn = pi^2 (m^2 + n^2)/a^2 sqrt(D/(rho t)) within 1 percent for (1,1) and
3 percent for (1,2) and (2,2). A free plate has six zero frequencies; a facet warped beyond the registered limit is refused with
the element.

## Reference to check against

Closed forms: Navier's double series for the simply supported Kirchhoff plate (summed in the registration), Castigliano for the
pinched ring, membrane equilibrium of a pressurised cylinder, the plate frequencies (all in Timoshenko and Woinowsky-Krieger,
Theory of Plates and Shells; derivable). Method, from memory: Dvorkin and Bathe, Eng. Comput. 1 (1984) 77 and Bathe and Dvorkin,
Int. J. Numer. Meth. Engng 21 (1985) 367 (the assumed shear strain). The published shell obstacle course (MacNeal and Harder,
Finite Elem. Anal. Des. 1 (1985) 3) is public but its values are not in the repository; it is not used (decision N7).

## Files

`crates/rcfd-fem/src/shell.rs`, `crates/rcfd-fem/tests/si_shell.rs`.

## What could make it miss

At the upper end of M; if it must be split: statics first, then mass and warped facets. The drilling stiffness is a fiction with a
tuning constant (registered from an estimate, not searched). Membrane locking of a flat-facet element on a curved surface is mild
but real: the ring limit is a draft. No geometric stiffness for shells in this tier, so a shell blade gets no Campbell diagram
(ADR 0290 section 1.3). No triangle.
