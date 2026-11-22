---
type: concept
tags: [process, studio]
sources: [doc-process-documentation-review, doc-process-lead-process, doc-studio-guardrails, doc-process-framework-reconciliation, doc-ui-restart-plan, doc-adr-briefs-readme]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# The two work streams (campaign and Studio)

rcfd's work ran in two streams with their own written rules: the **campaign stream**, registration ADRs under `AGENT_RULES.md` from 2026-11-12 (509 ADRs, 421 registrations when reviewed), and the **Studio stream**, items F-800 to F-838 and F-848 under `docs/studio-guardrails.md`, ADR 0800 and ADR 0801 ([[doc-process-documentation-review]], S2).

## How rcfd uses it

- On 2026-11-18 the maintainer answered Q1 "yes": one set of process rules for both streams; the Studio keeps its product guardrails in `docs/studio-guardrails.md` ([[doc-process-documentation-review]]). The agent rules now apply "in both streams", briefs are one per dispatched branch "in both streams", and the roadmap files are written by the lead on main only, "both streams" ([[doc-process-lead-process]], [[doc-adr-briefs-readme]]).
- What differed before unification (scores of 2026-11-18): the Studio stream had its rules in the repo and versioned (8a yes) and items that named their target (1c yes), while the campaign stream had neither; the campaign stream kept misses as `*_known_miss` tests while the Studio stream had "no mechanism" (4b); the Studio stream had no per-item record for 30 of 40 items (8c) ([[doc-process-documentation-review]]).
- The Studio stream's items were merged one by one into an integration branch (34 item merges) and reached main in one merge of 101 commits; 69 roadmap commits were made on its own branch, for which no rule named a writer. The rule now: an integration branch gets a new or changed item by merging main, never by editing the roadmap pair ([[doc-process-documentation-review]]).
- Studio work additionally follows [[studio-guardrails]] and the [[view-and-report]] step where its acceptance names one ([[doc-studio-guardrails]]).
- The Studio stream started from the plan of 2026-10-26 ([[doc-ui-restart-plan]]) and moved to the Electron app on 2026-11-16 (ADR 0800).
- The framework proposal of 2026-11-17 is not a third stream: it is a proposal, partly adopted ([[doc-process-framework-reconciliation]]).

## Related

- [[lead]]
- [[implementation-agent]]
- [[studio]]
- [[registration]]
- [[roadmap-studio-desktop-app]]
