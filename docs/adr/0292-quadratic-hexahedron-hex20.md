# ADR 0292: Quadratic hexahedron (Hex20) with its face loads, mass and the quadratic upgrade of a hexahedron box

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-2 of the structural solver, intermediate tier. Size S.

## Goal

The 20-node serendipity hexahedron: shape functions, 3 x 3 x 3 Gauss rule, stiffness, consistent mass, the consistent load of its
8-node faces, strain recovery, and the builder that upgrades a box of Hex8 to Hex20 by mid-edge nodes (with a coordinate map for
curved edges). It is the element for structured meshes of thin parts: one element through the thickness bends correctly.

## Dependencies

ADR 0291; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-1, F-372, F-374, F-375, F-376, F-383.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Hex20 on the reference element and on randomly distorted ones
(registered seed): shape functions sum to 1 and derivatives to 0 within 1e-14, analytic derivatives agree with central differences
within 1e-7, the 3 x 3 x 3 rule integrates every monomial of degree <= 5 per direction within 1e-13, the stiffness is symmetric
within 1e-13, has rank 54 and gives no force for the six rigid motions within 1e-11. Patch test with all six strains on a box with
displaced interior nodes: 1e-10. A uniform pressure on an 8-node face gives the nodal loads -1/12 (corners) and 1/3 (mid-edge) of
p A within 1e-13, and uniaxial tension the closed form within 1e-10. Cantilever L/h = 20, nu = 0.3, ONE element through the
thickness and 10 along: tip deflection within 1 percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A L^2)). Thick cylinder b/a = 2 in
plane strain on a curved mesh: bore displacement within 0.2 percent and bore hoop stress within 1 percent of Lame. Manufactured
solution of F-378: observed L2 order >= 2.7. Mass: total mass and inertia tensor of a box within 1e-12; first axial frequency of a
fixed-free bar (nu = 0) within 0.1 percent at 8 elements. The upgrade has (nodes + edges) nodes.

## Reference to check against

Closed forms of ADRs 0275, 0280 and 0284 (Timoshenko cantilever, Lame, bar frequency); exact monomial integrals over the cube; the
consistent load of a quadratic serendipity face (derivable: integral of the corner and mid-side functions over the square).
Method: Zienkiewicz and Taylor, The Finite Element Method, vol. 1 (serendipity family; from memory, the functions are checked by
the partition and derivative tests).

## Files

`crates/rcfd-solid/src/hex20.rs` (the element, its rule, its face functions and the upgrade builder),
`crates/rcfd-solid/tests/si_hex20.rs`.

## What could make it miss

The negative corner load of a quadratic face surprises a reader and any code that assumes positive nodal weights. Node order
against VTK type 25 (the mid-edge order; to be confirmed from the public VTK format description). Nodal stress extrapolation from
27 points is not unique: the package registers its rule.
