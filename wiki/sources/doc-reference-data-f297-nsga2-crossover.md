---
type: source
tags: [reference-data, f-item-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-297 NSGA-II crossover: the operator read against the papers (ADR 0220, docs/design-loop.md)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-297 NSGA-II crossover: the operator read against the papers (ADR 0220, docs/design-loop.md)" (lines 1498-1513) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-297 NSGA-II crossover read against the papers (ADR 0220).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Read 2026-11-07 for the registration of ADR 0220, before any code. No source listing of any optimisation code was opened (the web searches also listed implementation source pages of other NSGA-II and SBX codes; none was opened). The rows of F-249 above stay as they were written; this section says what the reading found.

| Item | Status | What was read (source) |
|---|---|---|
| NSGA-II main loop, crowded binary tournament, survivor selection; real-coded constants: crossover probability 0.9, mutation probability 1/n, distribution indices 20 and 20; population 100, 25 000 evaluations (250 generations); "the variablewise SBX operator used here" | **read**: equal to ADR 0209 D1 | Deb, Pratap, Agarwal and Meyarivan, IEEE Trans. Evol. Comput. 6 (2002) 182-197, sections III-C and IV-A (copy at cse.unr.edu/~sushil/class/gas/papers/nsga2.pdf). Its Table II gives the convergence metric of real-coded NSGA-II on ZDT1 as 0.033482 (mean of 10 runs), variance 0.004750, and on FON 0.001931 (a mean distance of the final population to 500 front points, not ADR 0209's C) |
| ZDT1 and FON definitions (n, bounds, objectives, Pareto-optimal solutions) | **read**: equal to ADR 0209 | the same paper, Table I |
| SBX: spread factor $\beta = \left\lvert \frac{c_1 - c_2}{p_1 - p_2}\right\rvert$, children equidistant from the parents' mid-point; densities $0.5\,(n + 1)\,\beta^n$ ($\beta \le 1$) and $0.5\,(n + 1) / \beta^{n + 2}$ ($\beta > 1$); multi-variable problems: "we choose to perform SBX in each variable with probability 0.5" (1.0 for a single variable), by analogy with single-point crossover (which swaps the variables beyond the cross site between the parents) and uniform crossover; children unbounded, a bounded distribution suggested as an extension | **read** | Deb and Agrawal, Complex Systems 9 (1995) 115-148, eqs. (6), (19), (20), sections 2, 3.1 and 6 (the publisher's PDF). ADR 0209's $\beta(u)$ is this distribution sampled by inversion. Differences from rcfd: rcfd crosses every variable of a crossed pair (ADR 0220 question 2) and clamps into the box |
| SBX procedure with bounds ($\beta_q$ from $u$ and $\alpha = 2 - \beta^{-(\eta+1)}$), children $c_1 = 0.5\,[(y_1 + y_2) - \beta_q \lvert y_2 - y_1\rvert]$, $c_2 = 0.5\,[(y_1 + y_2) + \beta_q \lvert y_2 - y_1\rvert]$ for $y_1 < y_2$ ("a simple modification ... for y1 > y2"); "each variable is chosen with a probability 0.5"; polynomial mutation $\delta_q$ with the bound term $(1 - \delta)^{\eta_m + 1}$, $c = y + \delta_q (y_u - y_l)$, citing Deb and Goyal (1996) | **read** | Deb and Agrawal (S.), "A niched-penalty approach for constraint handling in genetic algorithms", ICANNGA 1999, Appendix A (copy at homes.cs.washington.edu/~sagarwal/icannga.pdf). ADR 0209's SBX and mutation are these formulas without the bound terms ($\alpha = 2$, $\delta = 1$). The text does not state which child receives which value when $y_1 > y_2$, and states no exchange between the children |
| Polynomial mutation $p' = p + \delta_L (p - x_L)$ ($u \le 0.5$) or $p + \delta_R (x_U - p)$, $\delta_L = (2u)^{1/(1 + \eta_m)} - 1$, $\delta_R = 1 - (2(1 - u))^{1/(1 + \eta_m)}$ | **read** (another bounded form) | Deb and Deb, KanGAL report 2012016 (Int. J. Artif. Intell. Soft Comput. 4 (2014) 1-28), section 2 (copy at egr.msu.edu/~kdeb/papers/k2012016.pdf); it credits the operator to Deb and Agrawal (1999) |
| The per-variable exchange of the children's values (`Crossover::Exchange`) | **not in the papers read** | the lead's operator from ADR 0209 diagnostic DG1; its link to the papers is the analogy of Deb and Agrawal (1995) section 3.1 |
| Deb and Goyal, Computer Science and Informatics 26 (1996) 30-45 (the original polynomial mutation) | **not obtained** | no open copy found; the mutation was read in the 1999 and 2012 texts above (ADR 0220 question 1) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[design-loop]]
