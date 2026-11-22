---
type: concept
tags: [foundation, optimisation, multi-objective, pareto, nsga2]
sources: [ref-martins-ning-2021-engineering-design-optimization]
builds_on: [evolutionary-algorithms, optimisation-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Multi-objective optimisation (Pareto fronts, NSGA-II)

## In short

With several objectives that compete (drag and weight, power and noise) there is usually no single best design. A design is Pareto optimal when no other design is at least as good in every objective and better in one; the set of these designs traces a trade-off curve, the Pareto front. NSGA-II is a genetic algorithm that pushes a whole population toward that front ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 9.1-9.3).

## Explanation

**Is it really multi-objective?** Martins and Ning advise asking first whether one objective is more fundamental (range rather than drag and weight) or whether some "objectives" are really constraints, because solving a multi-objective problem costs much more ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 9.1). The problem is the standard form of [[optimisation-basics]] with a vector objective $f(x) = [f_1(x), \dots, f_{n_f}(x)]$.

**Dominance and the Pareto front** (sec. 9.2). For minimisation, design A *dominates* design B if A is better than B in all objectives (the book's wording; Fig. 9.1). A design that no other design in the domain dominates is *Pareto optimal*; it need not dominate the others. The *Pareto set* is the set of Pareto optimal $x^*$; the *Pareto front* is their objective values $f(x^*)$. The slope of the front shows trade-off sensitivity: how much of one objective must be given up for a gain in the other (Fig. 9.3).

**Methods with one objective at a time** (sec. 9.3.1-9.3.3):

- *Weighted sum* $f = w f_1 + (1 - w) f_2$, swept over $w$: easy, but even steps in $w$ give uneven points, and only the convex parts of the front can be found.
- *Epsilon-constraint*: minimise one objective with the others bounded, $f_j \le \varepsilon_j$, and sweep $\varepsilon$; reaches non-convex parts and $\varepsilon$ is in the objective's own units.
- *Normal boundary intersection*: find the anchor points (single-objective optima) and the utopia point, then search along normals to the plane through the anchors for evenly spaced front points.

**NSGA-II** (sec. 9.3.4; credited to Deb et al., the book's ref. 148). The reproduction and mutation of a [[evolutionary-algorithms]] GA stay; fitness and selection change:

- *Non-dominated sorting*: all non-dominated members get rank 1, are removed, the next non-dominated set gets rank 2, and so on (Alg. 9.2; the non-dominated set itself by the algorithm of Kung et al., Alg. 9.1).
- *Crowding distance*: for each member, the sum over objectives of the gap between its two neighbours in that objective, each normalised by the objective's range; for objective 1 sorted ascending, the contribution of member $j$ is

$$d_j \mathrel{+}= \frac{f_1^{(j+1)} - f_1^{(j-1)}}{f_1^{(n_p)} - f_1^{(1)}}$$ (Martins and Ning, sec. 9.3.4, Alg. 9.3)

and the anchor points get infinite distance so they are kept.
- *Crowded tournament selection*: lower rank wins; at equal rank the larger crowding distance wins.
- *Elitism*: parents and offspring are merged ($2 n_p$ members) and the next population is filled rank by rank; the last rank that does not fit is filled in order of largest crowding distance (Alg. 9.4, Ex. 9.4: 6 kept out of 12).
- "Many variations are possible, so although the algorithm is based on the concepts of NSGA-II, the details may differ somewhat." Simulated binary crossover (SBX) is not described in the book's chapters read here; its real-encoded GA uses a simple linear crossover (sec. 7.6.2).

**What it adds.** A single-objective optimiser returns one point; a Pareto front shows the whole trade-off, which helps when the final choice is made later, by a person or with a higher-fidelity tool (sec. 9.4). If a GA suits the single-objective problem, the multi-objective version costs little extra; for many variables, several gradient-based runs may still be cheaper (sec. 9.3.4).

## Builds on

- [[evolutionary-algorithms]]: population, selection, crossover, mutation, constraint tournament rules.
- [[optimisation-basics]]: objectives, constraints, feasible region.

## Leads to

- [[design-loop]] (`nsga2` study method) and the Pareto-set query of the [[design-ledger]].

## How rcfd uses it

- `nsga2` is a design-loop study method (ADR 0209), with an opt-in per-variable exchange crossover (ADR 0220) ([[design-loop]]).
- ADR 0220 read Deb, Pratap, Agarwal and Meyarivan (2002) sections III-C: crossover probability 0.9, mutation probability $1/n$, distribution indices 20 and 20, "equal to ADR 0209 D1"; ZDT1 and FON definitions from its Table I ([[doc-reference-data-f297-nsga2-crossover]]).
- The per-variable exchange of the children's values is "not in the papers read"; it is the lead's operator from diagnostic DG1 ([[doc-reference-data-f297-nsga2-crossover]]).
- Known miss MO1z: NSGA-II with the default SBX on the 30-parameter ZDT1 (C 0.35 to 0.54 against 1e-2), met only with the exchange crossover (C 1.3e-3 to 1.6e-3) ([[adr-thread-design-optimisers-and-ledger]]).
- Deb 2002 ZDT1 0.033482 is a recorded reference value ([[design-loop]]).
- The design ledger queries include the Pareto set ([[design-ledger]]).
- Multi-objective expected improvement is not built ([[gaussian-process-surrogate]]).
