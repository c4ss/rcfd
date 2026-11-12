# ADR 0322: Three-node shell element (flat facet, assumed shear strain) for triangulated surfaces

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-12 of the structural solver, advanced tier. Size M.

## Goal

The surface meshers of the repository produce triangles (decision N6 of ADR 0290). Add a three-node shell with six unknowns per
node on the conventions of ADR 0295: constant-strain membrane, Reissner-Mindlin bending with an assumed transverse shear strain
that is isotropic in the element and free of locking, the same drilling treatment, consistent mass, pressure and body loads,
resultants and surface stresses, and mixed meshes with the four-node shell.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-391, F-392.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Membrane patch test and constant-curvature bending patch test
on an irregular flat mesh, and on a mesh mixing Shell3 and Shell4, within 1e-8. Simply supported square plate under uniform load,
32 x 32 squares each cut in two, nu = 0.3: centre deflection within 2 percent of the Navier series at thickness/side 0.01 and
within 3 percent at 0.001, and within 1 percent of each other for the two diagonal directions. Cantilever strip, nu = 0: tip
deflection within 1 percent. Pinched ring strip, 32 elements per quarter: loaded diameter within 3 percent of (pi/4 - 2/pi) P
R^3/(E I). Cylinder under internal pressure: radial displacement p R^2/(E t) within 1 percent. First frequency of the simply
supported plate within 2 percent. A free plate has six zero frequencies below 1e-6 of the seventh; a single element has exactly
six zero eigenvalues; a triangle with an angle below the registered limit is refused with the element.

## Reference to check against

The closed forms of ADR 0295 (Navier series, Castigliano ring, membrane cylinder, plate frequencies). Method: the three-node
element with mixed interpolation of the shear strain (Lee and Bathe, Comput. Struct. 82 (2004) 945; from memory; the tying is
re-derived and checked by the patch and isotropy tests).

## Files

`crates/rcfd-solid/src/shell3.rs`, `crates/rcfd-solid/tests/sa_shell3.rs`.

## What could make it miss

Triangles converge more slowly than quadrilaterals: every percentage is a draft. The assumed-strain triangle is recalled, not
derived here; if it shows a spurious mode the fallback is the discrete-Kirchhoff triangle (thin plates only) by an amendment
before the run.
