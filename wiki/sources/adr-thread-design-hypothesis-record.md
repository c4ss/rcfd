---
type: source
tags: [adr-thread, design-loop, hypothesis, statistics, ledger, long-horizon]
sources: [doc-design-loop, doc-fan-project-breakdown]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the hypothesis record of the long-horizon design loop (F-701 and its follow-ups)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (a proposal registered after the maintainer's decisions of 2026-11-15, result addenda, follow-ups), 7 ADRs, 2026-11-15 to 2026-11-17.

Part H6 of `docs/fan-project-breakdown.md`: claims about a design are registered in the [[design-ledger]] before their evidence, tested at a family-wise level, and their verdicts recomputed from the evidence rather than trusted from the file (F-701, ADR 0624). The follow-ups add a seeded re-run sample and an evidence-parameter guard (F-727), coupled held metrics and the held-tolerance floor (F-728, F-749), a verifier-drawn re-run sample (F-745), lessons that read the drawn checks (F-776) and a rule that drawn checks must be newer than the evidence (F-781). See [[hypothesis-record]].

## Current state

- **F-701 partial** (ADR 0624): "Every check is met and M1 to M14 are caught; the gap is that R2 was missed only through the process-id dependence of an f699 test, so done is the maintainer's call. M15 is now caught (F-727)". MCP exposure is F-702 (todo). "Every confirmation is about the evaluator, never a real fan."
- **Done follow-ups:** F-727 (ADR 0662, seeded re-run sample, evidence-parameter guard), F-728 (ADR 0665, two held metrics solved together, `held_floor`), F-749 (ADR 0670, floor over a sample of the design), F-745 (ADR 0693, `verify_drawn` with a verifier's seed not in the file), F-781 (ADR 0698, a drawn record counts only when no evidence of the row is newer).
- **F-776 partial** (ADR 0695): "No clause is stated as unmet. Mutation M40 (the row order) survived the registered run and was caught only by check A0b added under amendment 1, so the item waits for the maintainer to call it done."
- **Limits stated by the ADRs themselves:** the seeded sample of ADR 0662 "is derivable from the file, so it guards against mistakes and version drift, not a deliberate forger"; ADR 0693 does not cover "a forger who can also append to the ledger (records are not signed)".

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0624](../../docs/adr/0624-f701-hypothesis-record-and-registered-tests-for-the-long-horizon-loop.md) | 2026-11-15 | proposal then registration (status word DRAFT; hub) | F-701 | Durable hypothesis record, tests registered before runs, scoped verdicts (H6) | partial; R2 missed through an f699 test's process-id dependence; M15 recorded | 0662 (M15) |
| [0662](../../docs/adr/0662-f727-registration-hypothesis-rerun-sample-and-evidence-parameter-guard.md) | 2026-11-16 | registration | F-727 | Seeded re-run sample; evidence-parameter guard | done; impostor caught in 394 of 400 (limit 380) | 0693 |
| [0665](../../docs/adr/0665-f728-registration-coupled-held-metrics-and-the-held-tolerance-floor.md) | 2026-11-16 | registration | F-728 | Coupled held metrics; held-tolerance resolution floor | done | 0670 |
| [0670](../../docs/adr/0670-f749-registration-held-tolerance-floor-over-a-sample-of-the-design.md) | 2026-11-16 | registration | F-749 | The held-tolerance floor over a sample of the design | done | |
| [0693](../../docs/adr/0693-f745-registration-verifier-drawn-rerun-sample.md) | 2026-11-17 | registration | F-745 | A verifier-drawn re-run sample for evaluators outside the maintainer's control | done; 399 and 394 of 400 caught; the F-727 sample catches 0 of 800 | 0695 |
| [0695](../../docs/adr/0695-f776-registration-lessons-read-the-verifier-drawn-checks.md) | 2026-11-17 | registration | F-776 | Lessons read the verifier-drawn checks | partial; M40 caught only after amendment 1 | 0698 |
| [0698](../../docs/adr/0698-f781-registration-drawn-checks-newer-than-the-evidence.md) | 2026-11-17 | registration | F-781 | Drawn checks must be newer than the evidence they check | done | |

## Key numbers and limits

- **ADR 0624:** hypothesis `i` gets level `0.05 * 6 / (pi^2 i^2)` (Holm inside a registered batch; maintainer decision B2 of 2026-11-15); `min_effect` at least 10 times the resolution; pairs at least 16. "400 synthetic worlds with 1 false confirmation (limit 37) and 0.998 of large effects found, while the naive agent confirms falsely in 400, the unadjusted one in 61 and the peeking one in 75"; "4000 single tests at a_1 give 118 (79 to 165)"; on the reduced-order fan "+1 mm tip gap lowers eta_s by 7.39e-3 (confirmed, 'Not tested at cfd') and sweep does not respond".
- **ADR 0662:** `rerun_sample_size` from the hypergeometric miss probability `C(n-b,k)/C(n,k)`; "for 64 pairs and an evaluator wrong on 16 of them, k = 13 (miss 0.0147)"; without a sample at tier cfd the impostor "is caught in 0 and with k = 1 in 116 (66 to 134)".
- **ADR 0693:** an impostor honest on m of n pairs "is missed with probability C(m,k)/C(n,k)"; "a 'verifier' seed derived from the file catches 0 of 400; k = 1 catches 305 (287 to 350)". The seed comes from `DrawSeed::Given` or `DrawSeed::Entropy` ("not cryptographic").
- **ADR 0698:** on 40 synthetic worlds "an honest ledger keeps its row with 1 check and 0 stale in 40 of 40".
- **ADR 0665:** two held metrics "solved together in each arm by a damped, projected Newton iteration ... (Dennis and Schnabel 1983, cited from memory)".

## Open questions

- F-701 and F-776: "done is the maintainer's call" / "waits for the maintainer to call it done" (roadmap gaps).
- ADR 0624 remaining question 2 (print the sign of the claimed direction) is recommended "with the MCP tools of F-702".
- Records are not signed: a forger who can append to the ledger is not covered (ADR 0693).

## Related

- [[hypothesis-record]], [[design-ledger]], [[design-loop]], [[fidelity-management]], [[registered-limit]]
- [[adr-thread-design-surrogate-and-fidelity]], [[adr-thread-design-optimisers-and-ledger]]
- [[roadmap-design-studies-and-optimisation]]
