---
type: concept
tags: [foundation, optimisation, doe, sampling]
sources: [ref-martins-ning-2021-engineering-design-optimization, ref-frazier-2018-bayesian-optimization]
builds_on: [probability-and-statistics-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Design of experiments (full factorial, Latin hypercube, low-discrepancy sequences)

## In short

A design of experiments, or sampling plan, decides at which input points an expensive model is evaluated before anything is known about it. A full grid is affordable only for a few variables, so practical plans spread a modest number of points evenly: a Latin hypercube puts exactly one point in each row and column of equal-probability bins, and low-discrepancy sequences such as Halton and Sobol add evenly spread points one after another ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.2).

## Explanation

**Full factorial.** Discretise every variable and evaluate all combinations. The cost grows exponentially: 10 variables at 12 intervals each need $12^{10}$, "approximately 62 billion", evaluations ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.2, Ex. 10.1). This is the curse of dimensionality.

**Random sampling** scales better but clusters, and needs many points before the sample looks like the target distribution (sec. 10.2.1). Frazier's basic Bayesian-optimisation loop starts from "an initial space-filling experimental design, often consisting of points chosen uniformly at random" ([[ref-frazier-2018-bayesian-optimization]] sec. 2, Algorithm 1).

**Latin hypercube sampling (LHS)** ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.2.1):

- Divide each of the $n_x$ dimensions into $n_s$ intervals of equal probability. Place the $n_s$ points so that each interval of each dimension holds exactly one point: the projection of the sample on every axis is then uniform (or follows the chosen marginal distribution). In two dimensions this is a Latin square.
- Many plans satisfy the rule, and some fill the space badly (the book's Fig. 10.6, all points on a diagonal). LHS can be posed as an optimisation that maximises the distance between points under the projection constraint, but usually one random point per interval with random pairing between dimensions is used (Alg. 10.1).
- Non-uniform marginals come from *inversion sampling*: if $u$ is uniform on $[0, 1]$, then $x = F^{-1}(u)$ follows the distribution with CDF $F$ (sec. 10.2.1, probability integral transform; see [[probability-and-statistics-basics]] for distributions).
- What LHS adds over random sampling: the even coverage of each marginal is enforced as a constraint rather than left to the law of large numbers, so far fewer samples are usually needed.

**Low-discrepancy (quasi-random) sequences** ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.2.2):

- *Discrepancy* is the variation of point density over the domain; a low-discrepancy set is close to even density. The sequences are deterministic, not random.
- Their advantage over LHS: points need not be chosen all at once, so more evenly spread points can be added later, which suits iterative procedures.
- Construction: the van der Corput sequence writes the integer $i$ in a base $b$, $i = a_0 + a_1 b + a_2 b^2 + \dots$ with digits $a_k \in [0, b-1]$, and mirrors the digits about the radix point (the radical inverse, Alg. 10.2), $\phi_b(i) = a_0/b + a_1/b^2 + \dots$. Halton uses a different prime base per dimension; Hammersley and Faure are related. "Sobol sequences use base 2 sequences but with a reordering based on 'direction numbers'" (p. 384).

**Uses beyond surrogates.** The same plans initialise genetic algorithms and particle swarms, give starting points for multistart gradient-based runs, and choose Monte Carlo points (sec. 10.2).

## Builds on

- [[probability-and-statistics-basics]]: uniform and normal distributions, the CDF, sample size and the law of large numbers.

## Leads to

- [[gaussian-process-surrogate]]: the initial sample before expected improvement.
- [[evolutionary-algorithms]]: initial populations.
- [[parametric-study]] and [[design-loop]]: the grid, list, LHS and Sobol methods.

## How rcfd uses it

- Study methods `factorial`, `lhs` and `sobol` in the design loop ([[design-loop]]).
- Measured Sobol L2-star discrepancy 0.0051 against 0.0131 for random points ([[doc-reference-data-wave32c-design-loop]]).
- The Latin hypercube's stratification property is "tested exactly"; McKay, Beckman and Conover (1979) were not read ([[doc-reference-data-wave32c-design-loop]]).
- The Sobol direction numbers for dimensions 2 to 10 "were recalled from memory" and are UNVERIFIED as a transcription, checked only by net properties ([[doc-reference-data-wave32c-design-loop]]). This page does not supply them.
- The Gaussian-process loop starts from a Latin hypercube ([[gaussian-process-surrogate]]).
- The case-file `study` section sweeps `list`, `linspace` and `logspace` values combined as `grid` (a full factorial) or `zip` ([[parametric-study]]).
- The tolerance driver of the ledger samples by Monte Carlo or Latin hypercube (ADR 0121) ([[adr-thread-design-optimisers-and-ledger]]).
