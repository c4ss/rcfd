---
type: source
tags: [reference, foundation, optimisation, gaussian-process, bayesian-optimisation, expected-improvement, cite-only]
sources: []
origin: https://arxiv.org/abs/1807.02811
licence: arXiv non-exclusive distribution licence 1.0 (http://arxiv.org/licenses/nonexclusive-distrib/1.0/, the "view license" link on the abstract page)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Frazier, "A Tutorial on Bayesian Optimization" (arXiv:1807.02811v1)

**Citation:** P. I. Frazier, "A Tutorial on Bayesian Optimization", arXiv:1807.02811 [stat.ML], v1 of 8 Jul 2018 (the only version).
**URL:** https://arxiv.org/abs/1807.02811 (PDF https://arxiv.org/pdf/1807.02811).
**Licence:** arXiv non-exclusive distribution licence 1.0. Cite only; no copy in `raw/`.
**Read:** 2026-11-22, by job p6-optimisation (sections 1, 2, 3, 3.1, 3.2, 4.1, 5; sections 4.2-4.3, 6, 7 skimmed for headings only).

## Summary

Bayesian optimisation (BayesOpt) for expensive, derivative-free, black-box global optimisation: a Gaussian-process (GP) prior on the objective, the posterior after each evaluation, and an acquisition function, mainly expected improvement (EI), that picks the next point. Section 5 covers noisy, parallel, constrained and multi-fidelity variants.

## Key points by section

### 1 "Introduction"
- Problem (1): $\max_{x \in A} f(x)$ with $x \in \mathbb{R}^d$, "Typically $d \le 20$"; $A$ a simple set (often a hyper-rectangle); $f$ continuous, expensive ("typically to a few hundred" evaluations), a black box, no derivatives observed, noise-free for most of the article; the aim is a global optimum.
- BayesOpt has been used "since the 1960s" for engineering design (Mockus 1989; Jones et al. 1998; Forrester et al. 2008).

### 2 "Overview of BayesOpt", Algorithm 1
- Two parts: a Bayesian statistical model of $f$ (a GP) and an acquisition function. Start with $n_0$ points from "an initial space-filling experimental design, often consisting of points chosen uniformly at random"; then repeat: update the posterior, evaluate at the maximiser of the acquisition function, until the budget $N$ is spent.
- The posterior at each $x$ is normal with mean $\mu_n(x)$ and variance $\sigma_n^2(x)$; a 95 % credible interval is $\mu_n(x) \pm 1.96\,\sigma_n(x)$; with noise-free data the mean interpolates and the interval has zero width at evaluated points.

### 3 "Gaussian Process (GP) Regression"
- Eq. (2): the prior on $f(x_{1:k})$ is multivariate normal with mean vector $\mu_0(x_{1:k})$ and covariance matrix $\Sigma_0(x_{1:k}, x_{1:k})$ built from a kernel; the kernel must give a positive semi-definite matrix for any set of points, and nearby points get larger correlation.
- Eq. (3): posterior $\mu_n(x) = \Sigma_0(x, x_{1:n}) \Sigma_0(x_{1:n}, x_{1:n})^{-1} (f(x_{1:n}) - \mu_0(x_{1:n})) + \mu_0(x)$ and $\sigma_n^2(x) = \Sigma_0(x, x) - \Sigma_0(x, x_{1:n}) \Sigma_0(x_{1:n}, x_{1:n})^{-1} \Sigma_0(x_{1:n}, x)$.
- Numerics: a Cholesky decomposition and a linear solve are faster and more stable than the inverse (Rasmussen and Williams Algorithm 2.1); adding a small number "like $10^{-6}$" to the diagonal helps when points are close.

### 3.1 "Choosing a Mean Function and Kernel"
- Power exponential (Gaussian) kernel $\Sigma_0(x, x') = \alpha_0 \exp(-\lVert x - x' \rVert^2)$ with $\lVert x - x' \rVert^2 = \sum_i \alpha_i (x_i - x'_i)^2$; Matérn kernel with an extra smoothness parameter $\nu$ and the modified Bessel function $K_\nu$. Mean: usually a constant $\mu$, or a constant plus parametric trend functions (eq. 4).

### 3.2 "Choosing Hyperparameters"
- Maximum likelihood estimate $\hat\eta = \arg\max_\eta P(f(x_{1:n}) \mid \eta)$, where the likelihood is a multivariate normal density; MAP adds a prior $P(\eta)$ and is useful when MLE gives functions that vary too fast or too slowly; the fully Bayesian approach (eq. 5-6) averages over hyperparameters by MCMC samples. MLE is MAP with a flat prior.

### 4.1 "Expected Improvement"
- Eq. (7): $\mathrm{EI}_n(x) = E_n[\,[f(x) - f_n^*]^+\,]$ with $f_n^*$ the best value observed (maximisation).
- Eq. (8): $\mathrm{EI}_n(x) = [\Delta_n(x)]^+ + \sigma_n(x)\,\varphi\!\left(\frac{\Delta_n(x)}{\sigma_n(x)}\right) - |\Delta_n(x)|\,\Phi\!\left(-\frac{|\Delta_n(x)|}{\sigma_n(x)}\right)$, $\Delta_n(x) = \mu_n(x) - f_n^*$. Eq. (9): evaluate at $\arg\max \mathrm{EI}_n$. First proposed by Mockus (1975), popularised by Jones et al. (1998) as EGO.
- EI increases in both $\Delta_n$ and $\sigma_n$ (Fig. 3): it trades high expected quality against high uncertainty; it is zero at evaluated points.

### 5 "Exotic Bayesian Optimization"
- Noisy evaluations: add the noise variance on the diagonal, often as a hyperparameter; EI becomes conceptually awkward with noise, KG or the Scott et al. (2011) variant are preferred.
- Parallel EI, eq. (14), and the Constant Liar approximation: choose points one after another, pretending pending points returned a constant, "usually the expected value".
- Constraints: with noise-free $f$ and $g_i$, improvement counts only at feasible points better than the best feasible point (Schonlau et al. 1998).
- Multi-fidelity: sources $f(x, s)$, lower $s$ more accurate and more costly; accuracy modelled either as noise growing with $s$, or as deterministic $f(x, s+1) - f(x, s)$ given a zero-mean GP in $x$; both fit as one GP over $(x, s)$; goal $\max_x f(x, 0)$ within a total cost budget. Multi-information-source optimisation drops the ordering of sources by accuracy and cost. Direct EI gives zero value to low-fidelity evaluations, so EI-based methods choose $x$ and $s$ separately.

## Contradictions with the wiki

None found. Frazier suggests a diagonal term "like $10^{-6}$"; rcfd's surrogate uses a 1e-8 jitter ([[gaussian-process-surrogate]]). This is a different choice, not a contradiction.

## Pages this source informs

[[multivariate-normal-and-kernels]], [[maximum-likelihood-estimation]], [[gaussian-process-surrogate]], [[multi-fidelity-modelling]], [[design-of-experiments]]
