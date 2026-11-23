---
type: concept
tags: [foundation, optimisation, statistics, gaussian-process]
sources: [ref-martins-ning-2021-engineering-design-optimization, ref-frazier-2018-bayesian-optimization, ref-rasmussen-williams-2006-gpml]
builds_on: [probability-and-statistics-basics, optimisation-basics, multivariate-normal-and-kernels]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Maximum likelihood estimation

## In short

Maximum likelihood picks the model parameters under which the observed data would have been most probable. With independent normal errors it reduces to least squares; for a Gaussian-process surrogate it is how the length scales, variance and noise level are fitted ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.3.2, 10.4; [[ref-frazier-2018-bayesian-optimization]] sec. 3.2).

## Explanation

**Definition.** Given data $y$ and a model with parameters $\theta$, the likelihood $L(\theta) = p(y \mid \theta)$ is the probability density of the data read as a function of $\theta$. The estimate is $\hat\theta = \arg\max_\theta p(y \mid \theta)$ ([[ref-frazier-2018-bayesian-optimization]] sec. 3.2). For independent data points $L$ is a product, so one maximises the log-likelihood $\ell = \ln L$ instead, which has the same maximiser and is easier to handle ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.3.2). Finding $\hat\theta$ is an optimisation problem ([[optimisation-basics]]).

**Normal errors give least squares.** Model each observation as the model value plus an error $\varepsilon_i \sim \mathcal{N}(0, \sigma^2)$, independent; the central limit theorem motivates the normal shape. The log-likelihood is then a constant minus $\frac{1}{2\sigma^2}\sum_i (y_i - \hat f(x_i; \theta))^2$, so "maximizing the log likelihood function ... is equivalent to minimizing the sum of squared errors (the least-squares formulation)" ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.3.2, eq. 10.28-10.39). This is the statistical reason behind [[linear-regression-least-squares]].

**Gaussian-process (kriging) hyperparameters.** The data vector is jointly normal with covariance $K_y$ built from a kernel ([[multivariate-normal-and-kernels]]). Its log density is the log marginal likelihood:

$$\log p(y \mid X, \theta) = -\tfrac{1}{2}\, y^T K_y^{-1} y - \tfrac{1}{2} \log |K_y| - \tfrac{n}{2} \log 2\pi$$ (Rasmussen and Williams, eq. 5.8)

with $K_y = K_f + \sigma_n^2 I$ when there is observation noise. The first term rewards fitting the data, the second penalises complexity, the third is a constant; the balance between fit and complexity is automatic ([[ref-rasmussen-williams-2006-gpml]] sec. 5.4.1, p. 113). Analytic gradients with respect to $\theta$ cost $O(n^2)$ each once $K^{-1}$ is known, after an $O(n^3)$ factorisation (eq. 5.9). In kriging the mean $\mu^*$ and variance $\sigma^{*2}$ have closed-form maximisers (eq. 10.69-10.70); what remains, the concentrated likelihood (eq. 10.71), depends only on the kernel parameters and is maximised numerically ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.4). A positive term on the diagonal (a nugget) can be one more fitted parameter (sec. 10.4).

**Pitfalls and alternatives.**

- The marginal likelihood can have several local maxima, each a different reading of the data; the book's example with 7 points has a complex low-noise and a simple noisy optimum, and "care should be taken that one doesn't end up in a bad local optimum" ([[ref-rasmussen-williams-2006-gpml]] sec. 5.4.1, Fig. 5.5).
- MAP estimation adds a prior $P(\theta)$ and maximises $P(y \mid \theta) P(\theta)$; it helps when MLE gives functions that vary too fast or too slowly. MLE is MAP with a flat prior. The fully Bayesian approach averages over $\theta$ by MCMC samples ([[ref-frazier-2018-bayesian-optimization]] sec. 3.2, eq. 5-6).
- [[cross-validation]] is the other common way to choose hyperparameters ([[ref-rasmussen-williams-2006-gpml]] sec. 5.4.2).

## Builds on

- [[probability-and-statistics-basics]]: probability densities, the normal distribution, independence.
- [[optimisation-basics]]: the estimate is the solution of a maximisation.
- [[multivariate-normal-and-kernels]]: the joint normal density of correlated data.

## Leads to

- [[gaussian-process-surrogate]] (length scales and nugget), [[linear-regression-least-squares]] (the normal-error case), [[cross-validation]].

## How rcfd uses it

- The rcfd surrogate fits its length scales and an optional nugget "by maximum likelihood from ten starts with the analytic gradient"; the multiple starts address the local-maximum pitfall above ([[gaussian-process-surrogate]]).
