---
type: source
tags: [reference, foundation, optimisation, gaussian-process, statistics, cite-only]
sources: []
origin: https://gaussianprocess.org/gpml/chapters/
licence: "(c) Copyright 2006 by Massachusetts Institute of Technology"; front matter "All rights reserved. No part of this book may be reproduced ... without permission in writing from the publisher."; web copy by permission of MIT Press
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Rasmussen and Williams, *Gaussian Processes for Machine Learning* (chapters read)

**Citation:** C. E. Rasmussen and C. K. I. Williams, *Gaussian Processes for Machine Learning*, MIT Press, 2006, ISBN 026218253X. Chapters online at https://gaussianprocess.org/gpml/chapters/ (RW0, RW4, RW5, RWA read).
**Licence:** all rights reserved (front matter, RW0.pdf); the chapters page says MIT Press "kindly agreed to allow us to make the book available on the web". Cite only.
**Read:** 2026-11-22, by job p6-optimisation: Appendix A.2 and A.4; sections 4.1-4.2.1; sections 5.1, 5.3, 5.4.1-5.4.2.

> [!warning] Contradiction
> [[foundations-sources]] reports that a library record claims a Creative Commons licence for this book. The book's own front matter (RW0.pdf, read 2026-11-22) says "All rights reserved", and the chapters page states MIT Press copyright with permission to post. The book's own statement is taken; cite only. Within the wiki's survey notes, not the repo docs.

## Key points by section

### A.2 "Gaussian Identities" (p. 200)
- Eq. (A.4): multivariate normal density $p(x \mid m, \Sigma) = (2\pi)^{-D/2} |\Sigma|^{-1/2} \exp\!\left(-\tfrac{1}{2}(x - m)^T \Sigma^{-1} (x - m)\right)$, $\Sigma$ symmetric positive definite.
- Eq. (A.5)-(A.6): for jointly Gaussian $x, y$ with blocks $A$, $C$, $B$, the marginal is $x \sim \mathcal{N}(\mu_x, A)$ and the conditional is $x \mid y \sim \mathcal{N}(\mu_x + C B^{-1}(y - \mu_y),\; A - C B^{-1} C^T)$.
- Sampling: $x = m + L u$ with $K = L L^T$ (Cholesky) and $u \sim \mathcal{N}(0, I)$; a small multiple of $I$ may be needed because the eigenvalues of $K$ can decay rapidly and the Cholesky factorisation would fail.

### A.4 "Cholesky Decomposition" (p. 202)
- $A = L L^T$ for symmetric positive definite $A$; solve by forward then back substitution, $n^2/2$ operations each; factorisation "numerically extremely stable", $n^3/6$ operations; $\log|A| = 2 \sum \log L_{ii}$.

### 4.1-4.2.1 Covariance functions (pp. 79-85)
- A valid covariance function gives positive semi-definite Gram matrices (eq. 4.2). Stationary: a function of $x - x'$; isotropic: a function of $r = |x - x'|$ only.
- Eq. (4.9) squared exponential $k_\text{SE}(r) = \exp\!\left(-\frac{r^2}{2\ell^2}\right)$, characteristic length-scale $\ell$; infinitely differentiable, so the process is very smooth; multiply by $\sigma_f^2$ for any process variance.
- Eq. (4.14) Matérn class $k(r) = \frac{2^{1-\nu}}{\Gamma(\nu)} \left(\frac{\sqrt{2\nu}\, r}{\ell}\right)^{\nu} K_\nu\!\left(\frac{\sqrt{2\nu}\, r}{\ell}\right)$, $K_\nu$ a modified Bessel function. Eq. (4.17): $k_{\nu=5/2}(r) = \left(1 + \frac{\sqrt{5} r}{\ell} + \frac{5 r^2}{3 \ell^2}\right) \exp\!\left(-\frac{\sqrt{5} r}{\ell}\right)$; $\nu = 3/2$ and $5/2$ are "the most interesting cases for machine learning"; $\nu = 1/2$ is the rough exponential kernel; for $\nu \ge 7/2$ the data can hardly tell it from SE.

### 5.1 and 5.3 Model selection and cross-validation (pp. 105-112)
- Hold-out validation wastes data and has a high-variance estimate when the validation set is small; $k$-fold cross-validation with typical $k$ from 3 to 10; $k = n$ is leave-one-out (LOO-CV), usually costly but with shortcuts for GP regression; any loss function, including the negative log predictive probability.

### 5.4.1 "Marginal Likelihood" (pp. 112-116)
- Eq. (5.8): $\log p(y \mid X, \theta) = -\tfrac{1}{2} y^T K_y^{-1} y - \tfrac{1}{2} \log |K_y| - \tfrac{n}{2} \log 2\pi$, $K_y = K_f + \sigma_n^2 I$: data fit, complexity penalty, normalisation. The trade-off between fit and complexity is automatic (Occam's razor).
- Eq. (5.9): analytic gradients with respect to hyperparameters, $O(n^2)$ each once $K^{-1}$ is known; inversion $O(n^3)$.
- The marginal likelihood can have multiple local optima, each a different interpretation of the data (Fig. 5.5, 7 points: a complex low-noise and a simple noisy model).

### 5.4.2 "Cross-validation" (pp. 116-117)
- Eq. (5.10)-(5.11): LOO log predictive probability $L_\text{LOO} = \sum_i \log p(y_i \mid X, y_{-i}, \theta)$ (the "log pseudo-likelihood").
- Eq. (5.12): $\mu_i = y_i - [K^{-1} y]_i / [K^{-1}]_{ii}$ and $\sigma_i^2 = 1/[K^{-1}]_{ii}$: all $n$ LOO predictions from one inverse at $O(n^3)$ plus $O(n^2)$.

## Contradictions with the wiki

The licence conflict above (within the wiki's survey notes).

## Pages this source informs

[[multivariate-normal-and-kernels]], [[maximum-likelihood-estimation]], [[cross-validation]], [[gaussian-process-surrogate]]
