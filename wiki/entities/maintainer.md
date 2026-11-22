---
type: entity
tags: [process, role]
sources: [doc-process-lead-process, doc-process-documentation-review, doc-studio-guardrails, doc-process-handover-2026-11-19]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# The maintainer (role)

**What it is:** the one person directing rcfd: direction, acceptance and changes to rules ([[doc-process-documentation-review]], Roles). "The maintainer's attention comes first" among the constraints of the process ([[doc-process-lead-process]]).
**Where:** decisions are recorded in ADR "Answers of the maintainer" sections, the priority statement at the head of `docs/ROADMAP.md`, and the process pages.

## Details

- **Acceptance** is "the maintainer's go-ahead for a line of work to be scoped or prioritised", written in the dated priority statement with weekly budgets; "The maintainer does not accept results one by one" ([[doc-process-lead-process]]; Q4 answer of 2026-11-18 in [[doc-process-documentation-review]]).
- Receives questions only of scope or priority, as options with a recommendation, and a weekly list printed by `process_report.py --weekly` ([[maintainer-questions]], [[process-report]]).
- Approves changes to the agent rules, the lead process, the tolerance table and every Studio guardrail; a change that breaks a guardrail needs "the maintainer's word and an ADR" ([[doc-process-lead-process]], [[doc-studio-guardrails]]).
- Owns the word on `baseline_hashes.rs` and on any non-additive golden change ([[doc-process-lead-process]]).
- Notable decisions recorded in this phase's sources: 2026-11-16 guardrails approved, egui apps deleted (F-449), WCAG contrast requirement removed ([[doc-studio-guardrails]]); 2026-11-18 one rule set for both streams, no stop of merging, no quarantine, no agent marker on commits, agents' reach not restricted ([[doc-process-documentation-review]]); 2026-11-21 weekly citation item instead of full batches, a reader only for items changing a default or golden ([[doc-process-handover-2026-11-19]]).
- Does the [[view-and-report]] step for Studio items that name it ([[doc-studio-guardrails]], G13).

## Related

- [[lead]]
- [[implementation-agent]]
- [[maintainer-questions]]
