---
type: concept
tags: [foundation, optimisation, multi-fidelity, surrogate]
sources: [ref-frazier-2018-bayesian-optimization, ref-martins-ning-2021-engineering-design-optimization]
builds_on: [gaussian-process-surrogate, linear-regression-least-squares, cross-validation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Multi-fidelity modelling

## In short

Often the same quantity can be computed several ways: a fast rough model, a slow accurate simulation, a scarce measurement. Multi-fidelity modelling uses the cheap source to explore widely and the dear source to check and correct it, so the answer approaches the accurate one for less total cost ([[ref-frazier-2018-bayesian-optimization]] sec. 5; [[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.1).

## Explanation

**Setting.** Instead of one objective $f$, there is a family $f(x, s)$ indexed by a fidelity $s$; Frazier lets lower $s$ mean higher fidelity, with $f(x, 0)$ the true objective and cost $c(s)$ rising as $s$ falls. Examples: $s$ the mesh size of a PDE solve, or the time horizon of a simulation. The goal is $\max_x f(x, 0)$ using observations $f(x_n, s_n)$ whose total cost $\sum_n c(s_n)$ stays within a budget $B$ ([[ref-frazier-2018-bayesian-optimization]] sec. 5, "Multi-Fidelity and Multi-Information Source Evaluations").

**Modelling the gap between fidelities.** Frazier gives two models, both expressible as one Gaussian process over $(x, s)$ ([[ref-frazier-2018-bayesian-optimization]] sec. 5):

- $f(x, s)$ equals $f(x, 0)$ observed with noise whose variance grows with $s$;
- or $f(x, s)$ is deterministic and the difference between successive fidelities, $f(x, s+1) - f(x, s)$, is a zero-mean Gaussian process in $x$. This is an *additive* discrepancy: the cheap model plus a learned correction.

A *multiplicative* (scaled) correction, $f_\text{high} \approx \rho\, f_\text{low} + \delta(x)$, is named in the foundations map but is not written out in the sources read in this job; rcfd's own R1 and R2 rungs use it ([[fidelity-management]]). A correction that is linear in a few parameters can be fitted by least squares ([[linear-regression-least-squares]]), and the choice between corrections is a model-selection problem for [[cross-validation]].

**Multi-information sources.** When the sources are not ordered by accuracy and cost, for example two approximate simulators each biased in different regions, the problem is multi-information-source optimisation: each $f(x, s)$, $s \ne 0$, has its own bias, noise and cost relative to $f(x, 0)$, and which is more accurate can depend on $x$ ([[ref-frazier-2018-bayesian-optimization]] sec. 5).

**Choosing what to evaluate.** Plain expected improvement fails here: an evaluation at $s \ne 0$ never improves the best observed $f(x, 0)$, so EI is zero for it and only the highest fidelity would ever be run. EI-based methods therefore choose $x$ by EI as if $f(x, 0)$ were observed and choose $s$ by a separate rule; knowledge-gradient and entropy-search acquisitions handle the choice directly ([[ref-frazier-2018-bayesian-optimization]] sec. 5).

**Data fusion with surrogates.** Surrogate models can fuse "numerical models with different levels of fidelity or experimental data", for example calibrating numerical data with experimental data, which is usually much scarcer ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 10.1). Martins and Ning also advise starting an optimisation with cheaper but representative models (sec. 1.2.4), and note that multi-objective fronts are useful when the models are low fidelity and higher-fidelity tools will follow (sec. 9.4).

## Builds on

- [[gaussian-process-surrogate]]: the Gaussian-process model of the objective and of the discrepancy.
- [[linear-regression-least-squares]]: fitting a low-order correction.
- [[cross-validation]]: deciding whether a correction helps on held-out points.

## Leads to

- [[fidelity-management]]: rcfd's screen, confirm and correct driver.

## How rcfd uses it

- `design::fidelity::run` screens with a low model, promotes exploit and explore picks to a `HighSource` (any model or a measured table), and records one `fidelity_pair` per promotion with the prediction made before the high result ([[fidelity-management]]).
- Discrepancy ladder: R0, R1 (additive or multiplicative), R2 ($\rho\, y_L + b_0 + c \cdot x$), chosen by leave-one-out error with the one-standard-error rule; R3 is a zero-mean Gaussian process on the R2 residual ([[fidelity-management]]).
- Ledger tiers `analytic`, `reduced_order`, `cfd` (and `measured`, private) label each evaluation's fidelity ([[design-ledger]]).
- Verified on synthetic sources only; no CFD or measured source yet ([[fidelity-management]]). Multi-fidelity joining in the surrogate is not built ([[gaussian-process-surrogate]]).
