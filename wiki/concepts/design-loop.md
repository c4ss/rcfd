---
type: concept
tags: [design-loop, optimisation, doe, mcp, fan]
sources: [doc-design-loop, doc-reference-data-wave32c-design-loop, doc-reference-data-f249-nsga2-nelder-mead, doc-reference-data-f297-nsga2-crossover, doc-reference-data-f260-cma-es, doc-reference-data-f698-gaussian-process, doc-fan-project-breakdown, doc-milestone-fan, doc-parity-matrix, adr-thread-design-optimisers-and-ledger, adr-thread-design-surrogate-and-fidelity, adr-thread-design-hypothesis-record, adr-thread-rom-reference-fan-and-c-grid, ref-martins-ning-2021-engineering-design-optimization, ref-hansen-2016-cma-es-tutorial]
builds_on: [design-of-experiments, nelder-mead-simplex, evolutionary-algorithms, multi-objective-optimisation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Design loop (operating maps, DOE, optimisation)

The design loop is rcfd's machinery for running a case over many operating points or design candidates without hand work, and for asking "what next". It was built in wave 32C (ADR 0067) for the fan milestone and extended by later ADRs. It is exercised on small laminar cases and analytic surrogates, not on the real fan ([[doc-design-loop]]).

## How rcfd uses it

- **Operating maps.** `rcfd map` and `design_run_operating_map` run a grid or a list of settings overrides. Each point warm-starts from the previous converged one, and a failed warm point is retried cold. The CSV/JSON tables are byte-identical between runs. Measured: 577 warm iterations against 746 cold, ratio 0.773 against a limit of 0.85 ([[doc-design-loop]]).
- **Studies (DOE and optimisation).** A study is `study.json` plus a resumable `evaluations.jsonl` store. It holds parameters with targets, chained operating conditions, objectives with robust modes and constraints ranked by Deb's rules ([[doc-design-loop]]).
  - Methods: `factorial`, `lhs`, `sobol`, `nelder_mead` (opt-in Gao-Han adaptive coefficients and `fminsearch` simplex, ADR 0209), `differential_evolution`, `nsga2` (ADR 0209, exchange crossover ADR 0220) and `cma_es` (ADR 0222).
  - Determinism: the same definition and seed give bit-identical histories for any worker count.
- **Measured results.** Sobol L2-star discrepancy 0.0051 against 0.0131 random ([[doc-reference-data-wave32c-design-loop]]). The E1 contraction channel's pressure drop falls from 9.94 to 5.07 (49 %, limit 25 %) ([[doc-design-loop]]). CMA-ES with restarts solves shifted Rastrigin d = 4 on 20 of 20 seeds, where Nelder-Mead stops at f = 35.8 ([[doc-design-loop]]).
- **Known misses.** Nelder-Mead T3 in the tool loop, the NSGA-II ZDT1 miss with SBX (fixed by the opt-in exchange crossover) and the ADR 0209 N3 seed ([[doc-design-loop]]).
- **Sources behind the algorithms.**
  - NSGA-II constants were checked against Deb et al. (2002), with Deb 2002 ZDT1 0.033482 ([[doc-reference-data-f297-nsga2-crossover]]).
  - CMA-ES follows Hansen's tutorial arXiv:1604.00772 and the Auger-Hansen 2005 restarts ([[doc-reference-data-f260-cma-es]]).
  - The Nelder-Mead options are cited from memory ([[doc-reference-data-f249-nsga2-nelder-mead]]).
  - GP test-function minima (Branin 0.397887, Hartmann3 -3.86278, Hartmann6 -3.32237, Shekel10 -10.5364) are from memory ([[doc-reference-data-f698-gaussian-process]]).
- **Assistant tools.** 14 `design_*` MCP tools: create study or variant, run map or study, evaluate, record, get metrics, compare, propose, list, plus four [[design-ledger]] tools. Write tools need approval. With `rcfd-mcp --project` the evaluations run on the project's runner (ADR 0189), and the runner's records equal the in-process ones bit for bit ([[doc-design-loop]]).
- **Library-only parts.** All of the following are library only. Their MCP tools wait for F-702 ([[doc-design-loop]]):
  - adjoint shape sensitivities (ADR 0091)
  - UQ (ADR 0121)
  - Sobol indices (ADR 0240)
  - the fan objective over impedance points (ADR 0620)
  - the Gaussian-process surrogate with expected improvement (ADR 0621)
  - fidelity management (ADR 0622)
  - registered hypotheses (ADR 0624)
- **Plan status.** In the computer-fan plan, H2 optimisers are partial (F-095, F-249, F-260; "a study on the real fan is todo") and H3 surrogate is done (F-698, library only) ([[doc-fan-project-breakdown]]).

> [!warning] Contradiction
> [[doc-milestone-fan]] (capability table dated 2026-10-18) lists the operating map, the optimiser and the design-loop tools as "none", citing "F-254 candidate" and "F-257 candidate". Those ids are proposals of [[doc-parity-matrix]]. [[doc-design-loop]] documents all three as built. Within the docs (stale table).

### ADR threads (P3)

- Optimisers, Sobol indices and the ledger: [[adr-thread-design-optimisers-and-ledger]]. The maintainer's shared decisions of 2026-11-15 (D1 to D6, B1 to B4) set the exposure (library only until F-702), the privacy split and the build order "F-268 and F-700, then F-697, then F-698 and F-699 in parallel, then F-701" ([[adr-thread-design-optimisers-and-ledger]]).
- Objective, [[gaussian-process-surrogate]] and [[fidelity-management]]: [[adr-thread-design-surrogate-and-fidelity]]. The basin restart's repetition trigger (ADR 0680) reaches the Hartmann 6 target on 8 of 10 seeds and the Shekel 10 target on 19 of 20. The stall pre-check (ADR 0645) cuts failed fan evaluations from 34 to 39 of 50 to 0 ([[adr-thread-design-surrogate-and-fidelity]]).
- [[hypothesis-record]]: [[adr-thread-design-hypothesis-record]].
- The reduced-order reference fan and its C-grid: [[adr-thread-rom-reference-fan-and-c-grid]].
- F-260 (CMA-ES and a real-fan study) is a headline of the `fan` milestone (ADR 0757). Its FAN-01 study is not attempted ([[adr-thread-design-optimisers-and-ledger]]).

## Builds on

- [[optimisation-basics]]: objectives, design variables, constraints, local and global search ([[ref-martins-ning-2021-engineering-design-optimization]] sec. 1.2-1.4).
- [[design-of-experiments]]: `factorial`, `lhs`, `sobol`.
- [[nelder-mead-simplex]]: `nelder_mead`.
- [[evolutionary-algorithms]]: `differential_evolution`, `cma_es`, and the Deb-style constraint ranking ([[ref-hansen-2016-cma-es-tutorial]], [[ref-martins-ning-2021-engineering-design-optimization]] sec. 7.6.3).
- [[multi-objective-optimisation]]: `nsga2` and Pareto sets.

## Related

[[design-ledger]], [[computer-fan-target]], [[fan-01]], [[mcp]], [[bit-identical]], [[roadmap-design-studies-and-optimisation]], [[doc-tutorial-mcp-design-loop]], [[doc-case-schema-design-loop-files]]

## Open questions

- Wiring the extended warm start into `rcfd map`, studies and the MCP tools is ADR 0209 question 7 ([[doc-design-loop]]).
- The real-fan study of F-260 is not delivered ([[doc-design-loop]]).
