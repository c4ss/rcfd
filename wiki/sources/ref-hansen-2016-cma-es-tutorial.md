---
type: source
tags: [reference, foundation, optimisation, cma-es, evolution-strategy, cite-only]
sources: []
origin: https://arxiv.org/abs/1604.00772
licence: arXiv non-exclusive distribution licence 1.0 (http://arxiv.org/licenses/nonexclusive-distrib/1.0/, the "view license" link on the abstract page)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Hansen, "The CMA Evolution Strategy: A Tutorial" (arXiv:1604.00772v2)

**Citation:** N. Hansen, "The CMA Evolution Strategy: A Tutorial", arXiv:1604.00772 [cs.LG], v2 of 10 Mar 2023 (v1 4 Apr 2016); PDF compiled 13 March 2023. Inria Saclay.
**URL:** https://arxiv.org/abs/1604.00772 (PDF https://arxiv.org/pdf/1604.00772).
**Licence:** arXiv non-exclusive distribution licence 1.0. Not an open-content licence, so cite only; no copy in `raw/`.
**Read:** 2026-11-22, by job p6-optimisation (sections 0.2-0.4, 1-5, Appendix A with Table 1 and Fig. 6, B.3; Appendix C MATLAB code not opened).
**Also read by the repo:** for ADR 0222 (F-260), see [[doc-reference-data-f260-cma-es]].

## Summary

A tutorial on the covariance matrix adaptation evolution strategy (CMA-ES) for single-objective, non-elitist ("comma") black-box minimisation in $\mathbb{R}^n$. It derives the sampling equation, the weighted mean update, the rank-$\mu$ and rank-one covariance updates with an evolution path, and cumulative step-size adaptation, then lists the default parameters.

## Key points by section

### 0.2 "The Multivariate Normal Distribution"
- $\mathcal{N}(m, C)$ is fixed by its mean $m$ and symmetric positive definite covariance $C$; surfaces of equal density are the ellipsoids $x^T C^{-1} x = 1$, whose principal axes are the eigenvectors of $C$ and whose squared axis lengths are its eigenvalues.
- Eq. (4): $\mathcal{N}(m, C) \sim m + B D\, \mathcal{N}(0, I)$ with $C = B D^2 B^T$; this is how samples are generated from independent standard normal numbers.

### 0.3 "Randomized Black Box Optimization"
- Minimise $f: \mathbb{R}^n \to \mathbb{R}$ where only function values are accessible; search costs are counted in function evaluations. Fig. 2: sample $\lambda$ points from a distribution, evaluate, update the distribution parameters, repeat until a termination criterion holds.
- The tutorial does "not state the objective of searching for a global optimum, as this is often neither feasible nor relevant in practice".

### 0.4 "Hessian and Covariance Matrices"
- On a convex quadratic $f_H(x) = \frac{1}{2} x^T H x$, setting $C = H^{-1}$ is equivalent to optimising the sphere function with $C = I$; covariance adaptation approximates the inverse Hessian, "similar to a quasi-Newton method".

### 1 "Basic Equation: Sampling"
- Eq. (5): $x_k^{(g+1)} \sim m^{(g)} + \sigma^{(g)} \mathcal{N}(0, C^{(g)})$ for $k = 1, \dots, \lambda$, with step size $\sigma$ and population size $\lambda \ge 2$.

### 2 "Selection and Recombination: Moving the Mean"
- Eq. (6)-(7): the new mean is a weighted average of the $\mu$ best of the $\lambda$ samples, weights positive, decreasing and summing to 1 (truncation selection plus weighted recombination).
- Eq. (8): variance effective selection mass $\mu_\text{eff} = (\sum w_i^2)^{-1}$, $1 \le \mu_\text{eff} \le \mu$. Eq. (9): $m \leftarrow m + c_m \sum w_i (x_{i:\lambda} - m)$ with learning rate $c_m$ usually 1.

### 3 "Adapting the Covariance Matrix"
- 3.1: estimating $C$ from the selected steps $x_{i:\lambda} - m^{(g)}$ (eq. 12), measured from the old mean, enlarges variance in the direction of progress; the EMNA estimate (eq. 13), measured from the new mean, shrinks it and is "highly susceptible to premature convergence".
- 3.2 rank-$\mu$ update, eq. (15)-(16): exponential smoothing $C \leftarrow (1 - c_\mu) C + c_\mu \sum w_i y_{i:\lambda} y_{i:\lambda}^T$, $y = (x - m)/\sigma$; $1/c_\mu$ is the backward time horizon (about 63 % of the weight, eq. 18-20); a first-order choice is $c_\mu \approx \mu_\text{eff}/n^2$.
- 3.3 rank-one update and evolution path, eq. (22), (24), (28): $p_c \leftarrow (1 - c_c) p_c + \sqrt{c_c(2 - c_c)\mu_\text{eff}}\,(m^{(g+1)} - m^{(g)})/(c_m \sigma^{(g)})$, $C \leftarrow (1 - c_1) C + c_1 p_c p_c^T$, with $c_1 \approx 2/n^2$; the path restores the sign information that $y y^T$ loses.
- 3.4 eq. (30): the final update combines rank-one and rank-$\mu$ terms; the rank-$\mu$ part matters in large populations, the rank-one part in small ones.

### 4 "Step-Size Control"
- Cumulative step-size adaptation (CSA): a long evolution path (correlated steps) means $\sigma$ should grow, a short one (steps cancelling) means it should shrink.
- Eq. (31): conjugate path $p_\sigma \leftarrow (1 - c_\sigma) p_\sigma + \sqrt{c_\sigma(2 - c_\sigma)\mu_\text{eff}}\, C^{-1/2} (m^{(g+1)} - m^{(g)})/\sigma^{(g)}$. Eq. (32): $\ln \sigma \leftarrow \ln \sigma + \frac{c_\sigma}{d_\sigma}\left(\frac{\lVert p_\sigma \rVert}{E\lVert \mathcal{N}(0, I) \rVert} - 1\right)$.

### 5 "Discussion"
- CMA-ES is "an attractive option" when quasi-Newton or conjugate-gradient methods fail on non-convex or rugged landscapes (sharp bends, discontinuities, outliers, noise, local optima).
- Step-size control prevents premature convergence of the population but "does not prevent the search to end up in a local optimum"; small populations converge faster, large populations help avoid local optima.
- Invariances: to strictly monotonic transformations of $f$, to rotations and reflections of the search space, and (with matching initial state) to any invertible linear transformation.
- Footnote 22: the downhill simplex method shows population degeneration into a subspace "in dimension, say, larger than ten".

### Appendix A, Fig. 6 and Table 1 (default parameters, 2016)
- Algorithm eq. (38)-(47); eq. (48) $\lambda = 4 + \lfloor 3 \ln n \rfloor$ ("can be increased"); eq. (49) preliminary weights $w'_i = \ln\frac{\lambda + 1}{2} - \ln i$; $\mu = \lfloor \lambda/2 \rfloor$; negative weights in the covariance update (active CMA).
- Text after Table 1: the defaults are robust; changing them is not recommended "apart from increasing the population size"; increasing $\lambda$ improves global search at the cost of convergence speed, and "Independent restarts with increasing population size [3], automated or manually conducted, are a useful policy" ([3] = Auger and Hansen 2005).

### B.3 "Termination criteria"
- Stop when continuing wastes CPU time, "and it would be better to restart (eventually with increased population size [3])"; criteria NoEffectAxis, NoEffectCoord and others (listed in [[doc-reference-data-f260-cma-es]]).

## Contradictions with the wiki

None. The defaults agree with the rcfd reading recorded in [[doc-reference-data-f260-cma-es]].

## Pages this source informs

[[evolutionary-algorithms]], [[multivariate-normal-and-kernels]], [[nelder-mead-simplex]], [[design-loop]]
