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

# Reference data: F-249 parametric studies: NSGA-II, Nelder-Mead options, extended warm start (ADR 0209, docs/design-loop.md)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-249 parametric studies: NSGA-II, Nelder-Mead options, extended warm start (ADR 0209, docs/design-loop.md)" (lines 1422-1436) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-249 parametric study references: NSGA-II, Nelder-Mead options, extended warm start (ADR 0209).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Nothing in F-249 is compared with measured data. Every check is against a closed form, an analytic test function or a cold run of the same case.

| Item | Status | What was read (source) |
|---|---|---|
| NSGA-II: fast non-dominated sorting, crowding distance, crowded binary tournament, elitist survivor selection, constrained domination; crossover probability 0.9, distribution indices 20, mutation probability 1/d | from memory | Deb, Pratap, Agarwal and Meyarivan, IEEE Trans. Evol. Comput. 6 (2002) 182-197, NOT read for ADR 0209; the constants are those recalled from its experiments (ADR 0209 question 5) |
| Simulated binary crossover `beta = (2u)^(1/(eta+1))` or `(1/(2(1-u)))^(1/(eta+1))`, children `0.5((1 +- beta) p1 + (1 -+ beta) p2)` | from memory | Deb and Agrawal (1995), NOT read; applied to every variable of a crossed pair, without exchanging the children's values (ADR 0209 D1; the MO1z miss and its diagnosis, amendment 1) |
| Polynomial mutation `delta = (2u)^(1/(eta+1)) - 1` or `1 - (2(1-u))^(1/(eta+1))`, `x += delta (hi - lo)` | from memory | Deb and Goyal (1996), the simple form, NOT read |
| ZDT1 (`f1 = x1`, `g = 1 + 9 sum x_i/(d-1)`, `f2 = g (1 - sqrt(f1/g))`, front `f2 = 1 - sqrt(f1)`) and FON (`f_{1,2} = 1 - exp(-sum (x_i -+ 1/sqrt d)^2)`, x in [-4, 4]^3) | from memory | Zitzler, Deb and Thiele (2000) and Fonseca and Fleming (1995), NOT read; the definitions are checked by their own closed forms (ADR 0209 MO3c) |
| Gao-Han adaptive Nelder-Mead coefficients `alpha = 1, beta = 1 + 2/n, gamma = 0.75 - 1/2n, delta = 1 - 1/n` | verified (secondary) | the Optim.jl documentation, "Nelder-Mead" page (julianlsolvers.github.io/Optim.jl/stable/algo/nelder_mead/), read 2026-11-07; Gao and Han, Comput. Optim. Appl. 51 (2012) 259-277, NOT read (the University of Idaho copy was not reachable) |
| `fminsearch` initial simplex: "adding 5% of each component x0(i) to x0 ... uses 0.00025 as component i if x0(i) = 0" | read | MathWorks documentation, "Optimizing Nonlinear Functions" (the fminsearch algorithm), read 2026-11-07; no MathWorks code was opened. The `fminsearch` reference page's Rosenbrock example (start (-1.2, 1), final value 8.17766e-10 in its figure title) was read for context only; it gives no value at 120 evaluations |
| Warm start of temperature and k-epsilon / k-omega SST fields | derived | no source; the checks compare a warm and a cold run of the same discrete equations (ADR 0209 W1 to W4) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[design-loop]]
