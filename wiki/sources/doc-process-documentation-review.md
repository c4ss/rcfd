---
type: source
tags: [process, review, model-output]
sources: []
origin: docs/process/documentation-process-review.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Documentation process review, adapted for an agent swarm (2026-11-18)

**Origin:** [docs/process/documentation-process-review.md](../../docs/process/documentation-process-review.md) at `origin/main` 2e94e5ef
**Kind:** review (model output) whose improvements the maintainer adopted on 2026-11-18

## Summary

A review of how rcfd records plans, decisions, evidence and status, measured against a framework drawn from published authors and adapted for one maintainer directing a lead agent and many implementation agents. "It is model output (Claude, 2026-11-18, repository at commit 205889bf) and has not been reviewed by a human." On 2026-11-18 the maintainer said to put it in place and answered its seven questions. It is the basis of the rule set in force ([[doc-process-agent-rules]], [[doc-process-lead-process]]). Not to be confused with the earlier review of the same name in `framework-2026-11-17/` ([[doc-process-framework-2026-11-17]]).

## Key points

- **Two streams** (S2): campaign stream, registration ADRs under `AGENT_RULES.md`, 2026-11-12 onward (509 ADRs, 421 registrations); Studio stream, items F-800 to F-838 and F-848 under `docs/studio-guardrails.md`, ADR 0800 and ADR 0801. See [[work-streams]].
- **Six swarm premises** (P1-P6): scarcity moves (maintainer attention, compute on the one shared machine, whole-system verification); the documents are the memory (what an agent was told is part of the record); trust comes from mechanism (agents of one model may share an error); coordination cost is contention on shared files (96 of 500 campaign merges conflict on replay); a written priority does not steer by itself; the worker writes the next work (435 of 490 new items took their acceptance from an agent's addendum).
- **Facts G1-G28**, e.g. G1: 736 files in docs/adr, 23.7 MB, about 3.9M words; G5: of 421 campaign registrations, 414 single-file creating commits, "No registration in either group was found with code first", no numeric limit changed; G8: last green hosted gate eef6c094 (2026-11-05); G9: the lead's checks left no record and the lead committed roadmap status a median 32 s after the merge, before those checks; G12: stale first-read statements, including "No structural solver" (overview, README, CHANGELOG; "docs/structural-solver.md exists since 2026-11-14; 307 structural items, 133 done"); G21: no brief in the repository.
- **Framework:** 8 dimensions (direction, bounded bets, record system, evidence, single source of truth, reader fit, flow and feedback, governance and provenance) built from Rumelt, Singer, Nygard, Nosek et al., Humble and Farley, Oberkampf and Roy, Hunt and Thomas, Martraire, Procida, Goldratt, Forsgren et al., Gawande. Scored as read: 34 of 80 (43 %); projected if adopted: 77 of 80 (96 %), a forecast, not a measurement.
- **Twelve improvements I1-I12:** I1 one process report ([[process-report]]); I2 push script and ledger ([[push-main]], [[process-ledger]]); I3 rules in the repo (agent-rules.md, lead-process.md; the fixed question headings, see [[maintainer-questions]]); I4 first-read pages and the revived roadmap generator; I5 area summaries and reading lists; I6 priority statement with weekly budgets; I7 authorship (no agent marker on commits); I8 keep the brief ([[brief]]); I9 a reader of another kind as a registered trial; I10 sources read, not remembered ([[sources-read-addendum]]); I11 diff-selected tests before the merge, within 30 minutes; I12 size as a registered prediction (five values). Plus gap-closing changes: append-only ADRs, kind words in names, a committed mutation patch per registration, cargo-mutants, a "kind" column with a validation rule, a "margin" column, the tolerance table ([[tolerance-class]]), held self-sourced limits, one-line note entries, a weekly audit of the lead.
- **Maintainer answers (2026-11-18), Q1-Q7:** one rule set for both streams; revive the roadmap generator; main may run ahead of a green full run by what one gate a day can handle, and testing is trimmed "to the least validation that Claude cannot do for itself"; acceptance is "the green light for work to be scoped or prioritised, not for detail work"; remove citations from memory as far as possible; any model is welcome; agents' reach is not restricted and what they fetch is not kept. Also: no stop of merging, no quarantining of failing tests, no agent marker on commits.
- **Declined (Not proposed):** stopping merges until green; quarantining a failing test ("the room for abuse is too large"); a standing daily read by a different model; one file per roadmap item written by the item's branch (later piloted in shadow mode, see [[doc-process-framework-reconciliation]]); an automatic revert.
- **What has been put in place (2026-11-18 and 11-19)**, five parts: tooling and pages (process report sections a-m, push script, ledger, agent-rules v1, lead-process, briefs folder, tolerance draft); the revived `render_roadmap.py` making ROADMAP.md generated (48 disagreements fixed, second F-847 gone); every partial item given a `gap` (319, drafted by a model from acceptance and note alone, unchecked); the maintainer's decisions on seventeen option sets (all recommendations taken; agent rules at version 3; unanswered-question count 88 to none); six PRs listed in [[doc-process-handover-2026-11-19]].

## Contradictions with the wiki

- G12 records "No structural solver" statements as false since 2026-11-14; [[doc-product]] and [[doc-ui-restart-plan]] still carry the claim. See [[studio]].
- I3 says the rules should not be a root CLAUDE.md or AGENTS.md; [[doc-process-framework-reconciliation]] later adopted a root `AGENTS.md` (2026-11-21). Within the repo docs; the newer decision stands.

## Pages this source informs

- [[work-streams]]
- [[lead]]
- [[maintainer]]
- [[implementation-agent]]
- [[process-report]]
- [[push-main]]
- [[process-ledger]]
- [[brief]]
- [[maintainer-questions]]
- [[tolerance-class]]
- [[sources-read-addendum]]
- [[registration]]
- [[gate]]
- [[roadmap]]
