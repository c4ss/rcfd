---
type: concept
tags: [foundation, optimisation, gradient-free]
sources: [ref-martins-ning-2021-engineering-design-optimization, ref-hansen-2016-cma-es-tutorial]
builds_on: [optimisation-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Nelder-Mead simplex (downhill simplex)

## In short

The Nelder-Mead method minimises a function using only its values. It keeps $n + 1$ points in $n$ dimensions (a triangle in 2D) and, at each step, replaces the worst point by reflecting it through the centre of the others, stretching or shrinking that move depending on how good the new point is ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 7.3).

## Explanation

**The simplex.** A set of $n + 1$ points $x^{(0)}, \dots, x^{(n)}$ in the space of $n$ design variables. The start is usually an equal-edge simplex around the initial guess (eq. 7.1-7.2). Each iteration sorts the points from best $x^{(0)}$ to worst $x^{(n)}$ by $f$ ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 7.3, pp. 287-288). Martins and Ning class it as a deterministic, local, heuristic direct-search method; it has nothing to do with the simplex algorithm of linear programming (sec. 7.2-7.3).

**The moves.** All but shrinking create one trial point on the line through the worst point and the centroid of the others:

$$x = x_c + \alpha\,(x_c - x^{(n)}), \qquad x_c = \frac{1}{n}\sum_{i=0}^{n-1} x^{(i)}$$ (Martins and Ning, eq. 7.3-7.4)

- reflection $\alpha = 1$, always tried first;
- expansion $\alpha = 2$, if the reflected point beats the best point;
- outside contraction $\alpha = 1/2$, if the reflected point lies between the second-worst and the worst;
- inside contraction $\alpha = -1/2$, if it is worse than the worst;
- shrink, as a last resort, moves every point toward the best: $x^{(i)} = x^{(0)} + \gamma\,(x^{(i)} - x^{(0)})$ with $\gamma = 0.5$ (eq. 7.5).

([[ref-martins-ning-2021-engineering-design-optimization]] sec. 7.3, Fig. 7.4, Alg. 7.1.) The focus is on replacing the worst point, not improving the best.

**Cost and stopping.** One evaluation per iteration when the reflection is accepted, two for an expansion or contraction, $n + 2$ for a shrink. Convergence is judged by the simplex size (eq. 7.6) and by the standard deviation of the function values at its vertices (eq. 7.7). On the book's two-variable bean function it took 68 evaluations to a tolerance of $10^{-6}$ (sec. 7.3).

**Limits.** It cannot handle constraints directly; a penalty term turns the problem into an unconstrained one, and the penalty need not be differentiable (sec. 7.3). Like other gradient-free methods its cost grows steeply with dimension (sec. 7.1). Hansen notes that the downhill simplex shows degeneration of its points into a subspace "in dimension, say, larger than ten" ([[ref-hansen-2016-cma-es-tutorial]] sec. 5, footnote 22).

**What it adds to the basics.** A local search that needs no gradient, no line search and no model of the function: only comparisons of function values.

## Builds on

- [[optimisation-basics]]: objective, design variables, local search, gradient-free methods.

## Leads to

- [[design-loop]] (`nelder_mead` study method).
- [[evolutionary-algorithms]]: population methods that keep more than $n + 1$ points and search globally.

## How rcfd uses it

- `nelder_mead` is a design-loop study method, with opt-in Gao-Han adaptive coefficients and an `fminsearch`-style simplex (ADR 0209) ([[design-loop]]).
- The adaptive coefficients $\alpha = 1$, $\beta = 1 + 2/n$, $\gamma = 0.75 - 1/(2n)$, $\delta = 1 - 1/n$ are "verified (secondary)" from the Optim.jl documentation; Gao and Han (2012) was not read ([[doc-reference-data-f249-nsga2-nelder-mead]]). None of the sources read for this page covers them.
- The standard coefficients "1, 2, 0.5, 0.5" and the 5 % initial simplex were taken "from general knowledge"; Nelder and Mead (1965) was not read ([[doc-reference-data-wave32c-design-loop]]). They agree with the reflection, expansion, contraction and shrink factors in Martins and Ning (sec. 7.3), where an inside contraction is written as $\alpha = -1/2$.
- Known misses: Nelder-Mead T3 in the tool loop, and N3 (19 of 20 seeds on a constrained quadratic) ([[design-loop]], [[adr-thread-design-optimisers-and-ledger]]).
- On shifted Rastrigin $d = 4$ every Nelder-Mead run stops at $f = 35.82$, where CMA-ES with restarts reaches the target on 20 of 20 seeds ([[adr-thread-design-optimisers-and-ledger]]).
