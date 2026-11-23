---
type: concept
tags: [foundation, optimisation, statistics, model-selection]
sources: [ref-martins-ning-2021-engineering-design-optimization, ref-rasmussen-williams-2006-gpml]
builds_on: [probability-and-statistics-basics, linear-regression-least-squares]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Cross-validation (hold-out, k-fold, leave-one-out)

## In short

A fitted model always looks good on the data it was fitted to. Cross-validation measures how well it predicts instead: fit on part of the data, test on the rest, and rotate. The error on held-out points is used to choose between models or settings, and the simplest model with a near-best error is usually preferred ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.3.4; [[ref-rasmussen-williams-2006-gpml]] sec. 5.3).

## Explanation

**Why.** In the book's example, fitting polynomials of rising order to the same data drives the training error toward zero (a 20th-order polynomial almost to zero), yet a 19th-order fit passes through every point and predicts badly between them: *overfitting*. Too few degrees of freedom is *underfitting* ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.3.4, Ex. 10.5). Least squares ([[linear-regression-least-squares]]) gives the best fit for a fixed model; cross-validation chooses the model.

**Variants.**

- *Hold-out (simple) validation*: split the data into a training set and a validation set; the validation error estimates the generalisation error. Drawbacks: only part of the data trains the model, and a small validation set gives a high-variance estimate ([[ref-rasmussen-williams-2006-gpml]] sec. 5.3, pp. 111-112; [[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.3.4).
- *$k$-fold*: split into $k$ equal parts, train on $k - 1$, validate on the remaining one, repeat $k$ times and average. Typical $k$ is 3 to 10; the price is $k$ fits instead of one ([[ref-rasmussen-williams-2006-gpml]] sec. 5.3).
- *Leave-one-out (LOO)*: $k = n$, each validation set is a single point; the extreme case for very small data sets ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.3.4; [[ref-rasmussen-williams-2006-gpml]] sec. 5.3).
- Any loss can be used: squared error is the most common for regression; for probabilistic models the negative log predictive probability is natural ([[ref-rasmussen-williams-2006-gpml]] sec. 5.3).

**Choosing.** Choose the model with the lowest average validation error, optionally retrain on all data; and "Among the similar solutions, we generally prefer the simplest model" (the book picks a fourth-order polynomial from a flat range of similar errors) ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.3.4, continuing Ex. 10.5). The formal "one-standard-error rule" named in the foundations map is not stated in either source read; the preference for the simplest of similar models is.

**LOO for Gaussian processes is cheap.** Leaving out one point of a Gaussian-process model only removes one row and column of the covariance matrix, so all $n$ LOO predictions follow from one inverse:

$$\mu_i = y_i - \frac{[K^{-1} y]_i}{[K^{-1}]_{ii}}, \qquad \sigma_i^2 = \frac{1}{[K^{-1}]_{ii}}$$ (Rasmussen and Williams, eq. 5.12)

The cost is $O(n^3)$ once plus $O(n^2)$ for all of LOO. The LOO log predictive probability $L_\text{LOO} = \sum_i \log p(y_i \mid X, y_{-i}, \theta)$ (eq. 5.10-5.11) can itself be maximised over the hyperparameters, an alternative to [[maximum-likelihood-estimation]] ([[ref-rasmussen-williams-2006-gpml]] sec. 5.4.2, pp. 116-117).

## Builds on

- [[probability-and-statistics-basics]]: sample mean and variance of errors, the idea of an estimate's variance.
- [[linear-regression-least-squares]]: fitting a model to data, residuals.

## Leads to

- [[multi-fidelity-modelling]] (choosing a discrepancy correction), [[gaussian-process-surrogate]] (leave-one-out diagnostics), [[fidelity-management]].

## How rcfd uses it

- The rcfd surrogate computes leave-one-out "from one factorisation" ([[gaussian-process-surrogate]]), the shortcut of eq. (5.12) above.
- The fidelity discrepancy ladder R0, R1, R2 is "chosen by leave-one-out error with the one-standard-error rule", and a correction is "trusted only when it helps on held-out points and exceeds twice the stated uncertainty" ([[fidelity-management]]).
- Rung R3's leave-one-out standard deviation through the Cholesky factor agrees with a double-double brute force within 1.3e-10 and 8.1e-10 to 9.2e-10 ([[fidelity-management]]).
