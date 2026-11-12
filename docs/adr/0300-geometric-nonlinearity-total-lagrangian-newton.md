# ADR 0300: Geometric nonlinearity: total Lagrangian solid elements, Newton iteration with load steps

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-10 of the structural solver, intermediate tier. Size M.

## Goal

Large displacement and rotation with small strain: Green-Lagrange strain and second Piola-Kirchhoff stress with the linear
material (St. Venant-Kirchhoff) for Tet4, Tet10, Hex8 (full integration) and Hex20, internal force and consistent tangent
(material part plus the geometric part of ADR 0298), and a Newton solve with load steps, a residual and an energy criterion, step
halving on divergence and a located refusal when halving is exhausted. The material is called through `stress(strain, state)` with
commit and roll-back of the state per step, so that a material with history (advanced tier) fits without changing the driver. Dead
loads only.

## Dependencies

ADR 0298; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-8, F-374, F-375.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences of the internal
force within 1e-6 relative on distorted elements at displacements of 20 percent of the element size (registered seed). A rigid
rotation by 90 degrees with a translation gives an internal force below 1e-10 E L^2. Uniform stretch lambda = 1.2 of a bar with nu
= 0: nominal stress E lambda (lambda^2 - 1)/2 within 1e-9; under load control the last three residuals of each step show an order
>= 1.8. A load factor of 1e-6 reproduces the linear solution within 1e-5 relative. Elastica: a cantilever of slenderness 50, nu =
0, with tip force P L^2/(E I) = 1 and 2 has the tip deflection and shortening within 2 percent of the elliptic-integral solution
(recomputed in the registration by integrating the elastica equation; recalled 0.302 L, 0.056 L and 0.494 L, 0.161 L). An element
inverted during the iteration and a step that does not converge after the registered halvings are errors with the element or the
step.

## Reference to check against

Closed forms: the uniaxial St. Venant-Kirchhoff bar (one line from S = E E_GL and P = F S); invariance of the Green-Lagrange
strain under rotation; the elastica (Bisshopp and Drucker, Q. Appl. Math. 3 (1945) 272, from memory; the reference numbers are
computed in the registration from the ordinary differential equation, not taken from the paper). Method: Bathe, Finite Element
Procedures, chapter 6 (total Lagrangian formulation), from memory, derived in the registration.

## Files

`crates/rcfd-fem/src/nonlinear.rs`, `crates/rcfd-fem/tests/si_nonlinear.rs`.

## What could make it miss

The elastica compares a 3-D solid with small but finite strain (1 percent at this slenderness) against an inextensible rod: the 2
percent is a draft. Tet4 locks and is only checked on the uniform states. Follower pressure, arc-length control and nonlinear
dynamics are not included (advanced). J2 plasticity is not in this tier (decision N1).
