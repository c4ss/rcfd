---
type: concept
tags: [foundation, optimisation, statistics, gaussian-process]
sources: [ref-rasmussen-williams-2006-gpml, ref-frazier-2018-bayesian-optimization, ref-hansen-2016-cma-es-tutorial, ref-martins-ning-2021-engineering-design-optimization]
builds_on: [probability-and-statistics-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Multivariate normal distribution and covariance kernels

## In short

The multivariate normal distribution is the bell curve for several correlated quantities at once: a mean vector says where it is centred and a covariance matrix says how spread and how correlated the components are. A covariance function (kernel) builds such a matrix for function values at any set of points, from the distances between them, which is what turns the normal distribution into a model of an unknown function ([[ref-rasmussen-williams-2006-gpml]] A.2, ch. 4; [[ref-frazier-2018-bayesian-optimization]] sec. 3).

## Explanation

**Density.** For a vector $x$ of length $D$:

$$p(x \mid m, \Sigma) = (2\pi)^{-D/2}\, |\Sigma|^{-1/2} \exp\!\left(-\tfrac{1}{2}(x - m)^T \Sigma^{-1} (x - m)\right)$$ (Rasmussen and Williams, eq. A.4)

with mean $m$ and symmetric positive definite covariance $\Sigma$; written $x \sim \mathcal{N}(m, \Sigma)$. For $D = 1$ this is the normal density of [[probability-and-statistics-basics]].

**Geometry.** Surfaces of equal density are ellipsoids $x^T \Sigma^{-1} x = \text{const}$; their principal axes are the eigenvectors of $\Sigma$ and the squared axis lengths its eigenvalues. With $\Sigma = B D^2 B^T$, a sample is $m + B D z$ with $z \sim \mathcal{N}(0, I)$ ([[ref-hansen-2016-cma-es-tutorial]] sec. 0.2, eq. 4). Equivalently $x = m + L u$ with the Cholesky factor $\Sigma = L L^T$; a small multiple of $I$ may have to be added because the eigenvalues can decay so fast that the factorisation fails ([[ref-rasmussen-williams-2006-gpml]] A.2, p. 201).

**Conditioning.** If $x$ and $y$ are jointly normal with covariance blocks $A$ (of $x$), $B$ (of $y$) and $C$ (between them), then

$$x \mid y \sim \mathcal{N}\!\left(\mu_x + C B^{-1}(y - \mu_y),\; A - C B^{-1} C^T\right)$$ (Rasmussen and Williams, eq. A.6)

Observing $y$ moves the mean of $x$ and always reduces its covariance. This one formula is the whole of Gaussian-process prediction: Frazier's posterior mean and variance at a new point (eq. 3) are eq. (A.6) with $y$ the observed values ([[ref-frazier-2018-bayesian-optimization]] sec. 3).

**Kernels.** A Gaussian process prior puts $f(x_{1:k}) \sim \mathcal{N}(\mu_0(x_{1:k}), \Sigma_0(x_{1:k}, x_{1:k}))$ for any finite set of points, with the matrix built entry by entry from a kernel $k(x_i, x_j)$ ([[ref-frazier-2018-bayesian-optimization]] eq. 2). Requirements: nearby points should be more strongly correlated, and the kernel must give a positive semi-definite matrix for every choice of points ([[ref-frazier-2018-bayesian-optimization]] sec. 3.1; [[ref-rasmussen-williams-2006-gpml]] eq. 4.2). A *stationary* kernel depends only on $x - x'$; an *isotropic* one only on $r = |x - x'|$ ([[ref-rasmussen-williams-2006-gpml]] sec. 4.1). Two common choices, normalised to $k(0) = 1$ (multiply by a variance $\sigma_f^2$):

- Squared exponential, infinitely differentiable, so very smooth sample functions:

$$k_\text{SE}(r) = \exp\!\left(-\frac{r^2}{2\ell^2}\right)$$ (Rasmussen and Williams, eq. 4.9)

- Matérn class with smoothness $\nu$ (eq. 4.14, with a modified Bessel function $K_\nu$); for $\nu = 5/2$:

$$k_{\nu=5/2}(r) = \left(1 + \frac{\sqrt{5}\, r}{\ell} + \frac{5 r^2}{3 \ell^2}\right) \exp\!\left(-\frac{\sqrt{5}\, r}{\ell}\right)$$ (Rasmussen and Williams, eq. 4.17)

$\nu = 3/2$ and $5/2$ are called "the most interesting cases for machine learning"; $\nu = 1/2$ gives a very rough process, and for $\nu \ge 7/2$ finite noisy data can hardly tell the kernel from the squared exponential ([[ref-rasmussen-williams-2006-gpml]] sec. 4.2.1, pp. 84-85).

The length-scale $\ell$ sets how fast the function is believed to change; with one $\ell$ per input direction (Frazier's $\alpha_{1:d}$ weights, [[ref-frazier-2018-bayesian-optimization]] sec. 3.1) each variable gets its own scale. Martins and Ning's kriging kernel is a power exponential in each coordinate with exponents $0 \le p_l \le 2$, Gaussian when every $p_l = 2$ ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.4, eq. 10.64).

## Builds on

- [[probability-and-statistics-basics]]: the one-dimensional normal distribution, mean, variance, covariance.
- Floor: matrices, eigenvalues, the matrix inverse.

## Leads to

- [[gaussian-process-surrogate]]: kriging prediction is conditioning with a kernel.
- [[maximum-likelihood-estimation]]: the length scales are fitted by maximising the multivariate normal density of the data.
- [[evolutionary-algorithms]]: CMA-ES samples from $\mathcal{N}(m, \sigma^2 C)$.
- [[direct-solvers-lu-and-cholesky]]: the Cholesky factor used for sampling and for solves with $\Sigma$.

## How rcfd uses it

- The rcfd Gaussian-process surrogate uses a Matern 5/2 correlation (squared exponential opt-in) with a 1e-8 jitter ([[gaussian-process-surrogate]]).
- Rung R3 of the fidelity ladder is a zero-mean Gaussian process on the R2 residual ([[fidelity-management]]).
- CMA-ES in the design loop samples a multivariate normal ([[design-loop]], [[evolutionary-algorithms]]).
