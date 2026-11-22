---
type: concept
tags: [design-loop, ledger, provenance, fan]
sources: [doc-design-loop, doc-fan-project-breakdown, doc-fan-measurement-protocol, doc-changelog, doc-test-process, adr-thread-design-optimisers-and-ledger, adr-thread-design-hypothesis-record, doc-example-projects]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Design ledger

The design ledger (`design::ledger`, ADR 0121) is one JSON-lines file that keeps every evaluated variant of every study, evaluator and fidelity tier, "so the design history survives sessions". It is different from a study's evaluation store, which belongs to one optimiser run ([[doc-design-loop]]).

## How rcfd uses it

- **Entries.** An entry holds the parameters and metrics as `{name, value, unit}`, the evaluator id and version, the solver version, a geometry or mesh hash, the settings that re-run it, a verdict (`ok`, `diverged`, `constraint_rejected`, `not_evaluated`) with a located reason, the tier (`analytic`, `reduced_order`, `cfd`), a time stamp, a seed and free provenance ([[doc-design-loop]]).
- **Crash safety.** Each batch is one append-mode write followed by a sync, and torn last lines are skipped. The ledger assumes a single writer ([[doc-design-loop]]).
- **Queries and re-run.** Queries cover filters, the Pareto set, best so far and parameter history. Export is to CSV or JSON. `ledger::rerun_check` re-evaluates an entry and compares metrics, units, verdict and geometry hash ([[doc-design-loop]]).
- **Optional keys (ADR 0632).** `parents`, `source`, `fidelity_level`, `uncertainty`, `objective` and `fitted` are written only when set, so older ledgers stay byte-identical ([[doc-design-loop]]).
- **Privacy split.** Measured data (tier `measured`) goes only to a private ledger, which is refused inside a repository tree (ADR 0647). Measured fan files "stay outside the repository" ([[doc-design-loop]], [[doc-fan-measurement-protocol]]).
- **Users of the ledger.** The UQ and Sobol drivers, the fan objective sink, the GP/EI driver, the fidelity driver and the hypothesis record all write to it. MCP tools: `design_ledger_append`, `design_ledger_query`, `design_ledger_export`, `design_ledger_rerun_check` ([[doc-design-loop]]).
- **Hypotheses (ADR 0624).** Claims are registered before their evidence. Hypothesis `i` is tested at `0.05 * 6 / (pi^2 i^2)`, so the levels of all hypotheses sum to at most 0.05. Verdicts are recomputed from the record, never taken from a written line ([[doc-design-loop]]).
- **Plan status.** H4 is partial (F-268; ADR 0121, 0240, 0623; F-700) ([[doc-fan-project-breakdown]]). The 0.1.0 changelog lists "a design ledger that keeps every variant" ([[doc-changelog]]).
- **Mutation check.** The ledger re-run check was one of the 10 of 12 detected mutations (ADR 0121 L6) ([[doc-test-process]]).

- **Store header fix (F-705, ADR 0644):** a kill inside the first append left a store that could not be reopened. Now "the header is written when the opened file is empty", shown at every byte of the first append (1354 points) ([[adr-thread-design-optimisers-and-ledger]]).
- **Ledger sink (ADR 0623):** `Study::set_ledger_sink` writes one entry per evaluated pair, with "0 duplicate and 0 missing at all 55 kill points of a Nelder-Mead study" ([[adr-thread-design-optimisers-and-ledger]]).
- **Public ledger rule (decision D3, 2026-11-15):** "A public ledger gets rounded aggregates only (3 significant digits, at least 5 pairs)" ([[adr-thread-design-optimisers-and-ledger]]).
- **Hypothesis guards:** seeded and verifier-drawn re-run samples, and drawn checks that count only when newer than the evidence (ADRs 0662, 0693, 0698). Records are not signed ([[adr-thread-design-hypothesis-record]]). See [[hypothesis-record]].

## Related

[[design-loop]], [[computer-fan-target]], [[provenance]], [[bit-identical]]
- Example ledgers: `designs/variants.ledger.jsonl` in [[example-project-wind-tunnel]] (8 lines), [[example-project-quick-start]] (4), [[example-project-axial-fan]] (1)

## Open questions

- Several writer processes are not locked against each other (single-writer assumption) ([[doc-design-loop]]).
- MCP tools for hypotheses, the surrogate and the fidelity driver wait for F-702 ([[doc-design-loop]]).
