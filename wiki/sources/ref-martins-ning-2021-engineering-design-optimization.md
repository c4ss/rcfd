---
type: source
tags: [reference, foundation, optimisation, surrogate, doe, multi-objective, cite-only]
sources: []
origin: https://mdobook.github.io/
licence: "Copyright (c) 2025 Joaquim R. R. A. Martins and Andrew Ning. All rights reserved." (copyright page of the free electronic edition); the website shows "(c) Andrew Ning" and no licence
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Martins and Ning, *Engineering Design Optimization* (free electronic edition)

**Citation:** J. R. R. A. Martins and A. Ning, *Engineering Design Optimization*, Cambridge University Press, 2021, ISBN 9781108833417. Electronic edition from https://mdobook.github.io (PDF http://flowlab.groups.et.byu.net/mdobook.pdf); page numbers match the printed book; first electronic edition January 2020.
**Licence:** all rights reserved. Free to read; cite only, never copy into `raw/`.
**Read:** 2026-11-22, by job p6-optimisation: sections 1.2-1.4, 7.1-7.3, 7.6, 9.1-9.3, 10.1-10.4, 10.6. The PDF's math glyphs did not survive text extraction, so equations below are written from the equation's described structure and the surrounding text, with the book's equation numbers. Lead check 2026-11-22: eq. (1.4), (7.3)-(7.5) and (10.100)-(10.102) compared against the rendered PDF pages (book pp. 15, 288, 416); all match as written here.

## Summary

A graduate textbook on design optimisation. The parts read cover problem formulation, the classification of problems and algorithms, gradient-free methods (Nelder-Mead, genetic algorithms), multi-objective optimisation and NSGA-II, sampling plans, regression, cross-validation, kriging and efficient global optimisation.

## Key points by section

### 1.2 "Optimization Problem Formulation"
- 1.2.1 Design variables: the vector $x$ of $n_x$ independent inputs the optimiser may choose; bounds $\underline{x}_i \le x_i \le \overline{x}_i$ (eq. 1.2, "bound constraints or side constraints"); continuous versus discrete variables; the parameterisation changes the shape of the objective but not the optimum (Ex. 1.1-1.2).
- 1.2.2 Objective: a scalar $f(x)$ that ranks designs; maximisation becomes minimisation of $-f$ (eq. 1.3); "A bad choice for the objective function is a common mistake in design optimization."
- 1.2.3 Constraints: equality $h(x) = 0$ and inequality $g(x) \le 0$; feasible region; active ($g(x^*) = 0$) versus inactive constraints; the number of independent equality constraints must not exceed $n_x$; requirements such as stress limits are often constraints rather than objectives.
- 1.2.4 eq. (1.4) standard form: minimise $f(x)$ by varying $\underline{x}_i \le x_i \le \overline{x}_i$, $i = 1, \dots, n_x$, subject to $g_j(x) \le 0$, $j = 1, \dots, n_g$, and $h_l(x) = 0$, $l = 1, \dots, n_h$. The analysis must be fully automatic; models may fail for some $x$ and "A good optimizer can handle such eventualities gracefully"; start with a simple subproblem and cheaper models.

### 1.3-1.4 Classification
- 1.3.3: unimodal versus multimodal; a found minimum is local unless unimodality is known; multimodality is proved by finding two distinct local minima; convex functions are unimodal.
- 1.3.4: deterministic versus stochastic models.
- 1.4.1: gradient-free (zeroth-order) versus gradient-based (first-order) algorithms; Fig. 1.23: on the example the gradient-based count grows from 67 to 206 function calls while the gradient-free one grows from 103 to over 32,000.
- 1.4.2: local versus global search; a global search raises the likelihood of the global optimum "but this is by no means guaranteed". 1.4.3: mathematical versus heuristic. 1.4.4: direct evaluations versus surrogate models ("interpolation-based or projection-based"). 1.4.5: deterministic versus stochastic algorithms.

### 7.1-7.2 Gradient-free algorithms
- Useful when gradients are unavailable or the function is noisy or discontinuous; much less efficient as dimension grows; "for small-size problems (usually ≤ 30 variables)" they can be useful (Fig. 7.1, $n$-dimensional Rosenbrock). Multimodality, multiple objectives and discrete variables are not automatic reasons to choose them. Global algorithms need bounds.
- Table 7.1 classes: local direct (Nelder-Mead, GPS, MADS), model-based local (trust region), deterministic global (DIRECT, MCS), surrogate-based global (EGO with kriging and expected improvement), stochastic (hit-and-run, evolutionary algorithms). Evolutionary algorithms are mostly population based and stochastic and give global exploration without guarantee.

### 7.3 "Nelder-Mead Algorithm" (pp. 287-289)
- A simplex of $n + 1$ points; initial equal-edge simplex (eq. 7.1-7.2); points sorted best $x^{(0)}$ to worst $x^{(n)}$.
- Eq. (7.3) $x = x_c + \alpha (x_c - x^{(n)})$ with centroid $x_c = \frac{1}{n}\sum_{i=0}^{n-1} x^{(i)}$ (eq. 7.4): reflection $\alpha = 1$, expansion $\alpha = 2$, outside contraction $\alpha = 1/2$, inside contraction $\alpha = -1/2$; shrink eq. (7.5) $x^{(i)} = x^{(0)} + \gamma (x^{(i)} - x^{(0)})$ with $\gamma = 0.5$.
- Cost per iteration: 1 evaluation (reflection accepted), 2 (expansion or contraction), $n + 2$ (shrink). Convergence by simplex size (eq. 7.6) and the standard deviation of the function values (eq. 7.7). It cannot handle constraints directly; a (non-differentiable) penalty is enough. Bean function: 68 evaluations to $10^{-6}$.

### 7.6 "Genetic Algorithms" (pp. 306-316)
- Population of $n_p$ designs; selection, crossover, mutation (Alg. 7.5); population size about "one order of magnitude larger than the number of design variables"; the sampling plans of 10.2 give better initial spread than random.
- Tournament and roulette-wheel selection; binary encoding with single-point crossover; real encoding with a linear crossover and random or normal mutation at probability "$p = 0.005 \sim 0.05$"; many other crossover strategies exist (ref. 149).
- 7.6.3 constraint handling in the tournament without penalty parameters: "1. Prefer a feasible solution. 2. Among two feasible solutions, choose the one with a better objective. 3. Among two infeasible solutions, choose the one with a smaller constraint violation."
- 7.6.4: no rigorous convergence criterion; usually a budget of generations.

### 9 "Multiobjective Optimization" (pp. 355-369)
- 9.1: before going multi-objective, ask whether one objective is fundamental or some "objectives" are constraints; multi-objective problems cost much more.
- 9.2: design A dominates B if it is better in all objectives; a point no other point dominates is Pareto optimal; the Pareto set is the set of such $x^*$, the Pareto front their objective values $f(x^*)$.
- 9.3.1 weighted sum: easy but uneven spacing and only the convex part of the front. 9.3.2 epsilon-constraint: minimise one objective with the others as constraints; reaches non-convex parts. 9.3.3 normal boundary intersection: anchor points, utopia point, more even spacing.
- 9.3.4 NSGA-II (Deb et al., ref. 148): non-dominated set (Kung et al. algorithm), non-dominated sorting into ranks, crowding distance (normalised neighbour gaps per objective, infinite for anchor points), crowded tournament selection (lower rank wins, then larger crowding distance), elitism by merging parents and offspring ($2 n_p$) and refilling by rank then crowding distance. "Many variations are possible". Example 9.4 fills 6 of 12 members.

### 10.1 "When to Use a Surrogate Model"
- Expensive or noisy models, experimental data, design-space understanding, and fusing several data sources: "numerical models with different levels of fidelity or experimental data", e.g. calibrating numerical data with scarcer experimental data. Curse of dimensionality. Surrogate-based optimisation (Fig. 10.2): sample, build, optimise, infill, repeat.

### 10.2 "Sampling"
- Full factorial scales exponentially: 10 variables at 12 intervals is $12^{10}$, about 62 billion evaluations (Ex. 10.1).
- 10.2.1 Latin hypercube: one point in each row and column of equiprobable bins (Latin square, Latin hypercube); can be posed as maximising spread under the projection constraint, but usually one random point per interval with random pairing (Alg. 10.1); inversion sampling for non-uniform marginals.
- 10.2.2 low-discrepancy sequences: deterministic, well spread ("quasi-random"), extendable without re-choosing earlier points; van der Corput radical inverse, Halton (prime bases), Hammersley, Faure; "Sobol sequences use base 2 sequences but with a reordering based on 'direction numbers'" (p. 384).

### 10.3 "Constructing a Surrogate"
- 10.3.2: with independent normal errors of zero mean (motivated by the central limit theorem), maximising the log likelihood is "equivalent to minimizing the sum of squared errors (the least-squares formulation)" (eq. 10.28-10.39).
- 10.3.4 cross-validation: training error falls with model complexity but prediction gets worse (overfitting, Ex. 10.5); simple (hold-out) and $k$-fold cross-validation; leave-one-out is the extreme case; "Among the similar solutions, we generally prefer the simplest model."

### 10.4 "Kriging"
- Model (eq. 10.62): a trend (usually a constant $\mu$) plus a normal random process $Z(x)$ of variance $\sigma^2$; correlation by a kernel (eq. 10.64, exponents $0 \le p_l \le 2$, $p_l = 2$ Gaussian); kernels must be symmetric positive definite.
- Parameters by maximum likelihood (eq. 10.65-10.71): $\mu^*$ and $\sigma^{*2}$ in closed form, the length-scale parameters by numerical maximisation of the concentrated likelihood. Predictor (eq. 10.75) interpolates the data; mean square error (eq. 10.76) vanishes at data points. A nugget on the diagonal gives regression, and a tiny one keeps the matrix invertible.

### 10.6 "Optimization and Infill"
- Exploitation infill: evaluate at the surrogate's optimum; fast but may converge to a local optimum (Alg. 10.4).
- 10.6.2 EGO: improvement $I = \max(f^* - \hat f(x), 0)$ (eq. 10.100), expected improvement in closed form with $\Phi$ and $\varphi$ of the standard normal and the kriging mean and standard error (eq. 10.101-10.102); infill at the maximum EI until it falls below a threshold (Alg. 10.5).

## Contradictions with the wiki

None.

## Pages this source informs

[[optimisation-basics]], [[design-of-experiments]], [[nelder-mead-simplex]], [[evolutionary-algorithms]], [[multi-objective-optimisation]], [[maximum-likelihood-estimation]], [[cross-validation]], [[multi-fidelity-modelling]], [[gaussian-process-surrogate]]
