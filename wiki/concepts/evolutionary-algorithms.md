---
type: concept
tags: [foundation, optimisation, gradient-free, evolutionary, cma-es]
sources: [ref-hansen-2016-cma-es-tutorial, ref-martins-ning-2021-engineering-design-optimization]
builds_on: [optimisation-basics, probability-and-statistics-basics, multivariate-normal-and-kernels]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Evolutionary algorithms (genetic algorithms, CMA-ES)

## In short

Evolutionary algorithms keep a population of candidate designs, evaluate them, keep the better ones and generate the next population from them with random variation. Genetic algorithms do this with crossover and mutation; the covariance matrix adaptation evolution strategy (CMA-ES) samples each new population from a multivariate normal distribution and learns that distribution's shape and size from the selected points ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 7.2, 7.6; [[ref-hansen-2016-cma-es-tutorial]] sec. 1-4).

## Explanation

**The family.** Most evolutionary algorithms are population based and stochastic; because the population is spread out they perform a global search, which raises the chance of getting near the global minimum "but by no means guarantee[s] it", and they lack a first-order optimality criterion ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 7.2). Martins and Ning add that "What constitutes an evolutionary algorithm is not well defined" and list dozens of nature-inspired variants. Hansen's general loop (Fig. 2) is: sample $\lambda$ points from a distribution $P(x \mid \theta)$, evaluate them, update $\theta$ from the evaluated points, repeat ([[ref-hansen-2016-cma-es-tutorial]] sec. 0.3).

**Genetic algorithms (GA)** ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 7.6):

- A population of $n_p$ designs, each a chromosome whose genes are design variables; $n_p$ about "one order of magnitude larger than the number of design variables". Better initial spread comes from the sampling plans of [[design-of-experiments]].
- *Selection* builds a mating pool: tournament selection keeps the better of random pairs (the best point always enters twice, the worst never); roulette-wheel selection draws with probability proportional to a scaled fitness.
- *Crossover* makes two offspring from two parents; *mutation* changes each variable (or bit) with a small probability, e.g. $p = 0.005$ to $0.05$, to keep diversity. Real-encoded GAs work on the real numbers directly; many crossover strategies exist (sec. 7.6.2).
- *Constraints* without penalty parameters, by tournament rules: "1. Prefer a feasible solution. 2. Among two feasible solutions, choose the one with a better objective. 3. Among two infeasible solutions, choose the one with a smaller constraint violation." (sec. 7.6.3).
- No rigorous convergence test; usually a budget of generations (sec. 7.6.4).

**CMA-ES** ([[ref-hansen-2016-cma-es-tutorial]]):

- *Sampling*, generation $g$:

$$x_k^{(g+1)} \sim m^{(g)} + \sigma^{(g)}\,\mathcal{N}\!\left(0, C^{(g)}\right), \quad k = 1, \dots, \lambda$$ (Hansen, eq. 5)

with mean $m$, overall step size $\sigma$ and covariance matrix $C$ (see [[multivariate-normal-and-kernels]]).
- *Selection and recombination*: the new mean is a weighted average of the $\mu$ best samples, $m \leftarrow m + c_m \sum_{i=1}^{\mu} w_i (x_{i:\lambda} - m)$, with positive decreasing weights summing to one and $c_m$ usually 1 (eq. 6-9). The variance effective selection mass is $\mu_\text{eff} = 1/\sum w_i^2$ (eq. 8).
- *Covariance adaptation* (sec. 3): the selected steps $y_{i:\lambda} = (x_{i:\lambda} - m)/\sigma$ are added to $C$ by exponential smoothing. The rank-$\mu$ update uses all selected steps of one generation (eq. 16); the rank-one update uses an *evolution path* $p_c$, a smoothed sum of successive mean moves that keeps the sign information lost in $y y^T$ (eq. 24, 28). Combined (eq. 30):

$$C \leftarrow (1 - c_1 - c_\mu \textstyle\sum w_j)\, C + c_1\, p_c p_c^T + c_\mu \sum_{i=1}^{\lambda} w_i\, y_{i:\lambda} y_{i:\lambda}^T$$ (Hansen, eq. 30)

with $c_1 \approx 2/n^2$ and $c_\mu \approx \min(\mu_\text{eff}/n^2, 1 - c_1)$. On a convex quadratic, learning $C$ amounts to learning the inverse Hessian, "similar to a quasi-Newton method" (sec. 0.4).
- *Step-size control* (sec. 4, cumulative step-size adaptation): a conjugate evolution path $p_\sigma$ (eq. 31) is compared with its expected length under random selection; $\sigma$ grows when the path is longer (successive steps point the same way) and shrinks when it is shorter (steps cancel):

$$\ln \sigma \leftarrow \ln \sigma + \frac{c_\sigma}{d_\sigma}\left(\frac{\lVert p_\sigma \rVert}{E\lVert \mathcal{N}(0, I)\rVert} - 1\right)$$ (Hansen, eq. 32)

- *Defaults* (Table 1, eq. 48-49): $\lambda = 4 + \lfloor 3 \ln n \rfloor$, $\mu = \lfloor \lambda/2 \rfloor$, weights $w'_i = \ln\frac{\lambda + 1}{2} - \ln i$; the defaults should not be changed "apart from increasing the population size".
- *Restarts*: a larger $\lambda$ improves global search at the cost of speed, and "Independent restarts with increasing population size ... are a useful policy to perform well on most problems" (Appendix A, citing Auger and Hansen 2005; also sec. B.3).
- *What CMA-ES adds over a plain GA or evolution strategy*: it adapts to badly scaled and non-separable problems, it works with small populations without degenerating, and it is invariant to monotone transformations of $f$ and to rotations of the search space (sec. 5). Step-size control prevents premature convergence of the population but not convergence to a local optimum.

**Differential evolution** is listed in the map under this node, but none of the sources read in this job describes it (Martins and Ning cover GA and particle swarm; Storn and Price 1997 was not read). It is left out here.

## Builds on

- [[optimisation-basics]]: black-box objective, global versus local search, stochastic algorithms.
- [[probability-and-statistics-basics]]: random sampling, mean, variance.
- [[multivariate-normal-and-kernels]]: the multivariate normal distribution that CMA-ES samples from.

## Leads to

- [[multi-objective-optimisation]]: NSGA-II is a GA with a different selection step.
- [[design-loop]] (`differential_evolution`, `nsga2`, `cma_es` study methods).

## How rcfd uses it

- `cma_es` with restarts of increasing population (ADR 0222) and `differential_evolution` are design-loop study methods ([[design-loop]], [[adr-thread-design-optimisers-and-ledger]]).
- The CMA-ES follows Hansen's tutorial arXiv:1604.00772 and the Auger-Hansen 2005 restarts, read for ADR 0222 without opening any source code; the recorded defaults include $\lambda = 4 + \lfloor 3 \ln n \rfloor$ and $\mu = \lfloor \lambda/2 \rfloor$ ([[doc-reference-data-f260-cma-es]]), equal to Hansen eq. (48)-(49) as read here.
- With restarts it solves shifted Rastrigin $d = 4$ on 20 of 20 seeds within 40 000 evaluations; "Without restarts 3 of 20 seeds reach the target"; defaults `restarts` 0 and `sigma0` 0.3 ([[adr-thread-design-optimisers-and-ledger]]).
- Differential evolution is DE/rand/1/bin with F = 0.5 and CR = 0.9 "from general knowledge" (Storn and Price not read), checked on Rastrigin $d = 4$ ([[doc-reference-data-wave32c-design-loop]]).
- Constraints in studies are ranked by Deb's rules, "as commonly stated" (Deb 2000 not read) ([[design-loop]], [[doc-reference-data-wave32c-design-loop]]); the three tournament rules in Martins and Ning sec. 7.6.3 are the same three rules.
- The FAN-01 study of F-260 is not attempted ([[adr-thread-design-optimisers-and-ledger]]).
