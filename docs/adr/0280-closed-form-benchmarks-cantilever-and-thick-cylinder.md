# ADR 0280: Closed-form benchmarks: cantilever bending and the thick-walled cylinder under pressure

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-10 of the structural solver, basic tier. Size M.

## Goal

Two engineering checks with closed forms: the tip deflection and bending stress of a slender cantilever (which shows the locking
of the linear tetrahedron and that the quadratic one is free of it), and Lame's thick-walled cylinder under internal pressure in
plane strain on a quarter model with symmetry constraints (curved geometry, pressure load, rollers). Tests only.

## Dependencies

ADR 0272, ADR 0276, ADR 0277; existing roadmap items: none.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Cantilever of slenderness L/h = 20 (square section, nu =
0.3, clamped root, tip shear force): the tip deflection is within 1 percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A L^2)), kappa
= 5/6, with Tet10 on a registered mesh, and within 5 percent with Hex8 at four elements through the thickness; the bending
stress at the surface one section height from the clamp is within 3 percent of M c/I (Tet10); Tet4 on the same nodes is the
registered locking control (deflection below 90 percent of the closed form). Thick-walled cylinder, b/a = 2, plane strain,
internal pressure, quarter model: the radial displacement at the bore and at the outer wall is within 0.5 percent and the hoop
stress at the bore within 2 percent of Lame's solution (Hex8 and Tet10 with curved edges on registered meshes), and the observed
order of the radial displacement error with Hex8 is >= 1.8.

## Reference to check against

Closed forms. Cantilever: Euler-Bernoulli deflection with Timoshenko's shear term (the shear term is 0.2 percent at this
slenderness). Cylinder (Lame; Timoshenko and Goodier, Theory of Elasticity): `sigma_r = A - B/r^2`, `sigma_theta = A + B/r^2`,
`A = p a^2/(b^2 - a^2)`, `B = A b^2`, and in plane strain `u_r = (1 + nu)/E ((1 - 2 nu) A r + B/r)`; derivable in a few lines,
to be re-derived in the registration. No NAFEMS value is used (their target values are not in the repository).

## Files

`crates/rcfd-fem/tests/sb_closed_forms.rs` only.

## What could make it miss

The Hex8 limit of 5 percent is a draft: full integration locks in shear and the number depends on the element aspect ratio; the
implementer registers it from an estimate. The clamp is not the beam-theory support when nu > 0 (local effect, estimated below
0.5 percent at L/h = 20). Stress at the bore converges at first order for Hex8.
