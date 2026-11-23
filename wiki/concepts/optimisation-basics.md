---
type: concept
tags: [foundation, optimisation]
sources: [ref-martins-ning-2021-engineering-design-optimization, ref-hansen-2016-cma-es-tutorial, ref-frazier-2018-bayesian-optimization]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Optimisation basics (objective, design variables, constraints)

## In short

An optimisation problem asks for the values of some design variables that make one number, the objective, as small as possible while a set of requirements, the constraints, still hold. Algorithms differ in what they need to know about the functions (only values, or also derivatives) and in whether they search near one point or over the whole design space ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 1.2, 1.4).

## Explanation

**The parts.** Martins and Ning name three ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 1.2.1-1.2.3):

- *Design variables* $x = [x_1, \dots, x_{n_x}]$: independent inputs the optimiser may set freely, usually with bounds $\underline{x}_i \le x_i \le \overline{x}_i$ (eq. 1.2). The choice of parameterisation changes the shape of the objective, not the optimum (their Ex. 1.1-1.2, a wing described by span and chord or by area and aspect ratio).
- *Objective* $f(x)$: one scalar that says whether one design is better than another. A maximisation becomes a minimisation of $-f$ (eq. 1.3). "A bad choice for the objective function is a common mistake in design optimization."
- *Constraints*: equalities $h(x) = 0$ and inequalities $g(x) \le 0$. The feasible region is the set of $x$ that satisfies all of them. An inequality is *active* at the optimum when $g(x^*) = 0$ and inactive when $g(x^*) < 0$; an inactive constraint could be dropped without changing the answer. A requirement such as a stress limit is usually a constraint, not an objective.

**Standard form** ([[ref-martins-ning-2021-engineering-design-optimization]] eq. 1.4):

$$\min_{x} f(x) \quad \text{s.t.} \quad \underline{x}_i \le x_i \le \overline{x}_i,\; g_j(x) \le 0,\; h_l(x) = 0$$ (Martins and Ning, eq. 1.4)

with $i = 1, \dots, n_x$, $j = 1, \dots, n_g$, $l = 1, \dots, n_h$. The objective and constraints come from an *analysis* (one or more numerical models) that must run without a human; models may fail for some $x$, and a good optimiser handles that (sec. 1.2.4). There can be no more independent equality constraints than design variables (sec. 1.2.3).

**Local and global optima.** A function with one minimum is unimodal, with several multimodal. A minimum found without knowing which is only a *local* minimum: better than all points near it. Proving unimodality of a complicated model is usually impossible; proving multimodality only takes two distinct local minima. Convex functions are unimodal ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 1.3.3). Hansen does not even aim at the global optimum, "as this is often neither feasible nor relevant in practice" ([[ref-hansen-2016-cma-es-tutorial]] sec. 0.3).

**Classes of algorithm** ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 1.4):

- *Gradient-free* (zeroth-order) algorithms use only function values; *gradient-based* (first-order) ones also use derivatives and scale much better with $n_x$. In the book's Fig. 1.23 the gradient-based count grows from 67 to 206 function calls while the gradient-free one grows from 103 to over 32,000 (sec. 1.4.1). Gradient-free methods are typically useful for "usually ≤ 30 variables" (sec. 7.1).
- *Local* search follows a sequence of points from one start; *global* search tries to span the whole space, which raises the chance of the global optimum "but this is by no means guaranteed" (sec. 1.4.2).
- *Mathematical* versus *heuristic* rules for the iteration and for stopping; *deterministic* versus *stochastic* algorithms; *direct* evaluations versus *surrogate* models of the analysis (sec. 1.4.3-1.4.5).
- *Black-box* optimisation: only function values of evaluated points are available, and cost is counted in evaluations ([[ref-hansen-2016-cma-es-tutorial]] sec. 0.3; [[ref-frazier-2018-bayesian-optimization]] sec. 1, where an evaluation may take hours and the budget is "typically to a few hundred").

**What it adds.** Calculus finds a minimum by setting a derivative to zero. Engineering problems add bounds, constraints, many variables, expensive or noisy models without derivatives, and several local minima. The vocabulary above sorts those problems so that a method can be picked for each.

## Builds on

- Floor: functions of several variables, partial derivatives, the idea of a minimum.

## Leads to

- [[nelder-mead-simplex]], [[evolutionary-algorithms]]: gradient-free local and global methods.
- [[multi-objective-optimisation]]: more than one objective.
- [[maximum-likelihood-estimation]]: fitting model parameters is itself an optimisation.
- [[design-of-experiments]], [[gaussian-process-surrogate]]: sampling and surrogate-based optimisation for expensive analyses.
- [[newton-raphson-method]] is the root-finding step behind gradient-based minimisation; it is a shared root, not part of this branch.

## How rcfd uses it

- rcfd's design-loop studies hold parameters with targets, objectives with robust modes and constraints ranked by Deb's rules, and offer DOE and optimisation methods (`factorial`, `lhs`, `sobol`, `nelder_mead`, `differential_evolution`, `nsga2`, `cma_es`) ([[design-loop]]).
- The ledger verdicts `diverged` and `constraint_rejected` record evaluations that failed or broke a constraint ([[design-ledger]]).
- The fan objective of the design loop runs on the reduced-order fan, and "Every number is a model proxy" ([[computer-fan-target]]).
- All of the rcfd optimisers are gradient-free; the adjoint shape sensitivities (ADR 0091) are library only ([[design-loop]]).
