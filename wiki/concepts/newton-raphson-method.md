---
type: concept
tags: [foundation, shared-roots, numerics]
sources: [ref-openstax-calculus, ref-wikipedia-shared-roots]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Newton-Raphson method

## In short

Newton's method finds a root of `f(x) = 0` by replacing the curve with its tangent line, jumping to where the tangent crosses zero, and repeating. Close to a simple root it converges very fast (the number of correct digits roughly doubles per step); far from it, it can wander, oscillate or fail. For a system of equations the derivative becomes the Jacobian matrix and each step solves a linear system ([[ref-openstax-calculus]] Vol. 1 sec. 4.9; [[ref-wikipedia-shared-roots]] "Newton's method").

## Explanation

**One equation.** From the tangent `y = f(x_0) + f'(x_0)(x - x_0)` set to zero (Eq. 4.8) ([[ref-openstax-calculus]] Vol. 1 sec. 4.9):

`x_n = x_(n-1) - f(x_(n-1)) / f'(x_(n-1))`

Example 4.47: for `f(x) = x^2 - 2` from `x_0 = 2` the iterates are 1.5, 1.416666667, ..., `x_5 = 1.414213562` ([[ref-openstax-calculus]]).

**Convergence.** If `f'(root) != 0` and the start is close enough, convergence is at least quadratic, `|eps_(n+1)| <= M eps_n^2` (sec. 4.1); "The number of correct digits roughly doubles with each step." A multiple root (or `f'(root) = 0`) gives only linear convergence (sec. 3.3) ([[ref-wikipedia-shared-roots]] "Newton's method").

**Failures.** `f'(x_n) = 0` stops the method (horizontal tangent); a poor start can converge to another root; iterates can cycle, e.g. `f(x) = x^3 - 2x + 2` from `x_0 = 0` alternates between 0 and 1 (Example 4.48) ([[ref-openstax-calculus]]). Robust codes cap the iterations, keep the iterate inside a bracket known to hold the root, and fall back to a safer root finder ([[ref-wikipedia-shared-roots]] "Newton's method", sec. 3.2). Damped Newton and line searches were not covered in the part of the sources read.

**Systems.** For `F(x) = 0` with `k` equations and `k` unknowns, the step solves ([[ref-wikipedia-shared-roots]] "Newton's method", sec. 6.1.1):

`J_F(x_n) (x_(n+1) - x_n) = -F(x_n)`

with `J_F` the Jacobian (matrix of partial derivatives); one solves this linear system rather than forming the inverse. In mechanics `F` is a residual (out-of-balance force) and `J` the tangent stiffness. With more equations than unknowns the same idea becomes Gauss-Newton ([[linear-regression-least-squares]]).

**What it adds.** First-year calculus uses the tangent line to approximate a function; Newton's method uses it repeatedly to solve nonlinear equations, and for systems it turns each nonlinear step into a linear solve ([[sparse-linear-systems]]).

## Builds on

Floor only: derivatives, tangent lines, linear systems `A x = b` (for the system form).

## Leads to

- `nonlinear-fem-newton` (residual and tangent stiffness), [[fixed-point-iteration-and-acceleration]] (quasi-Newton), `stability-of-equilibrium`.
- rcfd: [[arc-length-continuation]], [[hyperelastic-materials]], [[incompatible-modes-hex8]].

## How rcfd uses it

- The hyperelastic materials run on the total Lagrangian Newton driver ([[hyperelastic-materials]]).
- The incompatible-modes Hex8 at finite strain solves its internal modes "by a local Newton iteration at every element evaluation" (F-596, ADR 0608) ([[incompatible-modes-hex8]]).
- The stability chain around [[arc-length-continuation]] includes an indefinite-tangent Newton and a driver retry that reports the inertia of each accepted state (ADR 0694, ADR 0719) ([[arc-length-continuation]]).
- The structural solver's advanced tier lists "Newton and arc-length" ([[structural-solver]]).
