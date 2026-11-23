---
type: concept
tags: [design-loop, hypothesis, statistics, ledger, long-horizon]
sources: [adr-thread-design-hypothesis-record, doc-design-loop]
builds_on: [probability-and-statistics-basics, design-of-experiments]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Hypothesis record (registered design claims)

A hypothesis record applies the registration rule of rcfd's own process to design claims: the claim, its test and its significance level are written in the [[design-ledger]] before any evidence, and the verdict is recomputed from the evidence, never read from a written line. It is `design::hypothesis`, part H6 of the fan plan (F-701, ADR 0624), library only and opt-in ([[adr-thread-design-hypothesis-record]]).

## How rcfd uses it

- **Registration:** claim, factor at two levels, response with resolution, direction, predicted size, `min_effect` at least 10 times the resolution, held metric, domain, at least 16 pairs, seed, evaluator scope, statistic; level `0.05 * 6 / (pi^2 i^2)` for hypothesis `i` (Holm inside a batch) ([[adr-thread-design-hypothesis-record]]).
- **Judgement:** a one-sided t (or sign) test against `min_effect`; `verify` and `lessons` recompute verdicts and scope every row to its tier, model error and staleness. In 400 synthetic worlds: 1 false confirmation (limit 37), against 400 for a naive agent, 61 unadjusted and 75 peeking ([[adr-thread-design-hypothesis-record]]).
- **Guards added later:** a seeded re-run sample and an evidence-parameter guard (F-727), coupled held metrics and the held-tolerance floor (F-728, F-749), a verifier-drawn sample from a seed not in the file (F-745), lessons that read the drawn checks (F-776), and drawn checks valid only when newer than the evidence (F-781) ([[adr-thread-design-hypothesis-record]]).
- "Every confirmation is about the evaluator that produced it, never about a real fan" (ADR 0624). Example on the reduced-order fan: "+1 mm tip gap lowers eta_s by 7.39e-3 (confirmed, 'Not tested at cfd')" ([[adr-thread-design-hypothesis-record]]).

## Builds on

- [[probability-and-statistics-basics]]: the one-sided t test, significance level and sample size behind each verdict.
- [[design-of-experiments]]: the factor at two levels and the paired evaluations are a small designed experiment.

## Related

[[design-ledger]], [[design-loop]], [[registration]], [[registered-limit]], [[fidelity-management]], [[adr-thread-design-hypothesis-record]]

## Open questions

- Records are not signed; a forger who can append to the ledger is not covered (ADR 0693) ([[adr-thread-design-hypothesis-record]]).
- MCP exposure waits for F-702; F-701 and F-776 wait for the maintainer to call them done ([[adr-thread-design-hypothesis-record]]).
