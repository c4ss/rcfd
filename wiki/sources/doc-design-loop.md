---
type: source
tags: [design-loop, optimisation, mcp, fan]
sources: []
origin: docs/design-loop.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Design loop: operating maps, DOE, optimisation and the assistant tools

**Origin:** [docs/design-loop.md](../../docs/design-loop.md) at origin/main 2e94e5ef
**Kind:** repo doc: user and integrator reference for the design-loop library, CLI and MCP tools (wave 32C, ADR 0067 and later ADRs).

## Summary

Explains how rcfd runs a case over many operating points or design candidates without hand work, what each part guarantees (determinism, resume, failure handling) and what was measured. Built in wave 32C (ADR 0067) for the fan of [[doc-milestone-fan]]; "The real fan is not part of this wave; everything below is exercised on small laminar cases and on analytic surrogates." Code lives in `crates/rcfd-api/src/design/`.

## Key points

- Not delivered: the real-fan study of F-260; Halton sequences; adjoint gradients for turbulent, transient or energy runs and through studies or tools; RBF morphing; other surrogates; warm start for transient, coupled and most turbulence models; MCP tools for the fan objective, surrogate, fidelity driver and hypothesis record ("all library only; F-702").
- Operating maps (`rcfd map`): axes (`path=v1,v2` or `path=start:stop:count`), optional serpentine order; each point restarts from the previous converged solution (steady SIMPLE/SIMPLEC without energy or turbulence), a failed warm point is retried cold. Exit codes 0 / 5 / 2 / 3. `map.csv` and `map.json` are byte-identical between runs. Measured (ADR 0067 M3): warm chain 577 iterations against 746 cold (ratio 0.773, limit 0.85), pressure drops agree to 2e-8 relative.
- Extended warm start (library, ADR 0209) carries temperature and k-epsilon / SST fields: warm vs cold agree to 6.9e-9, 3.1e-11 and 2.6e-11 relative (limit 1e-4).
- Studies: `study.json` plus `evaluations.jsonl` store; named continuous parameters, operating conditions (chained), objectives with robust modes (`mean`, `worst`, `mean_plus_k_sigma`, `weighted`), constraints by Deb's rules; failed evaluations are the worst value. Methods: `factorial`, `lhs`, `sobol` (up to 10 parameters), `nelder_mead`, `differential_evolution`, `nsga2`, `cma_es`. "Everything is deterministic": same definition and seed give bit-identical histories for any `workers`. Store resume after a kill; the Joe-Kuo Sobol direction numbers "were typed from memory".
- Ask/tell: `design_propose` replays the method against the store; `design_record` stores external metrics; `design_evaluate` / `design_run_study` (bounded by `max_design_evaluations`, default 1000 per call).
- Measured (ADR 0067): Rosenbrock 20 of 20 seeds below 1e-18; Rastrigin d = 4 by DE 20 of 20; Sobol L2-star discrepancy 0.0051 vs random 0.0131 (ratio 0.39, limit 0.5); tool-loop Nelder-Mead T3 is a known miss (3.1e-3 vs 1e-3 after 120 candidates). E1 contraction channel: pressure drop "falls from 9.94 to 5.07 (49 %, limit 25 %)" at (4.46 degrees, 1.77).
- CMA-ES (ADR 0222, Hansen tutorial arXiv:1604.00772): on shifted Rastrigin d = 4 with 9 restarts, 20 of 20 seeds reach f <= 1e-8 within 40 000 evaluations, while Nelder-Mead stays at f = 35.8. NSGA-II (ADR 0209; exchange crossover ADR 0220): ZDT1 with 30 parameters is a known miss with SBX (0.35 to 0.54 from the front) and within 1.6e-3 with `crossover = "exchange"`.
- MCP tools: 14 `design_*` tools (8 `write_files`, 6 `read`), including four ledger tools. With `rcfd-mcp --project` studies run on the project's runner (ADR 0189), chained via `--continue-from` (ADR 0200), adopting matching runs after a restart (ADR 0201); runner records equal in-process ones bit for bit.
- Divergence guards for PIMPLE/PISO (ADR 0067) and steady solvers (ADR 0077) end a diverged candidate as `diverged`.
- Adjoint shape sensitivities (library, ADR 0091): matched central differences to 1.1e-6 relative (limit 5 %) on a 2D duct.
- Design ledger (ADR 0121, ADR 0632, ADR 0647), UQ (ADR 0121), Sobol indices (ADR 0240), fan objective (ADR 0620), Gaussian-process surrogate and expected improvement (ADR 0621, restarts ADR 0651/0680), fidelity management (ADR 0622, rung R3 ADR 0652), hypotheses registered before their runs (ADR 0624 and amendments) - all library only. See [[design-ledger]].
- Fan objective: "Every number is a model proxy: no measured fan data exists, so the objective ranks variants of the model, not real fans."

## Contradictions with the wiki

Contradicts the capability table of [[doc-milestone-fan]] (see the callout there).

## Pages this source informs

- [[design-loop]], [[design-ledger]], [[computer-fan-target]], [[mcp]], [[bit-identical]], [[known-miss]]
