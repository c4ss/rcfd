---
type: concept
tags: [foundation, shared-roots, statistics, numerics]
sources: [ref-nist-sematech-ehandbook, ref-wikipedia-shared-roots]
builds_on: [probability-and-statistics-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Linear regression and least squares

## In short

Least squares fits a model to data by choosing the parameters that make the sum of squared misfits as small as possible. When the model is linear in its parameters (a straight line, a polynomial, any weighted sum of known functions) the best parameters follow from one small linear system; otherwise an iterative method is needed ([[ref-nist-sematech-ehandbook]] sec. 4.4.3.1).

## Explanation

**Criterion.** For data `(x_i, y_i)` and a model `f(x; beta)`, minimise ([[ref-nist-sematech-ehandbook]] sec. 4.4.3.1):

`Q = sum_i [y_i - f(x_i; beta)]^2`

The differences `y_i - f(x_i; beta)` are the residuals (the handbook calls them deviations).

**Straight line.** For `y = beta0 + beta1 x + eps`, setting `dQ/dbeta0 = dQ/dbeta1 = 0` gives two linear equations whose solution is `beta1 = sum (x_i - xbar)(y_i - ybar) / sum (x_i - xbar)^2` and `beta0 = ybar - beta1 xbar` ([[ref-nist-sematech-ehandbook]] sec. 4.4.3.1). For any model linear in its parameters the handbook says the minimisation is "usually done analytically using calculus" in the same way; it does not write out the general matrix form (the normal equations), and no source read in this job does, so it is left to [[gradient-reconstruction]] and later sources.

**Fit quality.** The residual standard deviation is `sigma = sqrt(Q / (n - p))` for `n` points and `p` parameters ([[ref-nist-sematech-ehandbook]] sec. 4.4.3.1). The usual assumption behind least squares is normally distributed errors ([[ref-nist-sematech-ehandbook]] sec. 1.3.6.6.1; [[probability-and-statistics-basics]]).

**Nonlinear models.** If the model is not linear in the parameters, the minimisation needs iterative numerical algorithms ([[ref-nist-sematech-ehandbook]] sec. 4.4.3.1). Newton's method applied to an over-determined system becomes the Gauss-Newton method, whose step uses `(J^T J)^(-1) J^T`, the least-squares solution of the linearised problem ([[ref-wikipedia-shared-roots]] "Newton's method", sec. 6.1.2; [[newton-raphson-method]]).

**What it adds.** Solving `A x = b` needs as many equations as unknowns; least squares handles more equations than unknowns (more data points than parameters) by asking for the best compromise instead of an exact fit.

## Builds on

- [[probability-and-statistics-basics]]: mean, variance and the normal-error assumption behind the criterion.
- Floor: partial derivatives set to zero, linear systems, matrix transpose.

## Leads to

- [[gradient-reconstruction]] (least-squares cell gradients), `cross-validation`, `multi-fidelity-modelling` (discrepancy corrections fitted to data), [[fixed-point-iteration-and-acceleration]] (least-squares quasi-Newton).
- [[solution-verification]]: the least-squares version of the GCI for noisy grid convergence ([[ref-celik-2008-jfe-discretisation-uncertainty]]).

## How rcfd uses it

- The fidelity discrepancy ladder R0, R1 (additive or multiplicative), R2 fits corrections between model fidelities; R3 adds a Gaussian process on the R2 residual ([[fidelity-management]]).
