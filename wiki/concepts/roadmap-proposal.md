---
type: concept
tags: [process, roadmap, lead, agent-rules]
sources: [adr-thread-process-measures-and-roadmap-form, adr-thread-simulation-milestones-validation, adr-thread-fan01-design-point]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Roadmap proposal file (form 1)

A roadmap proposal is a JSON file `docs/roadmap/proposals/<ADR>-<item>.json` ("form 1") in which an implementing agent writes the status, note, gap and follow-ups it proposes for its item, beside the quoted paragraphs of its addendum. The lead checks and applies it with `scripts/roadmap_proposals.py` and still writes `docs/roadmap.json` alone (F-864, ADR 0755; rule 9 of the agent rules from version 5) ([[adr-thread-process-measures-and-roadmap-form]]).

## How rcfd uses it

- **Rule for whose words stand** (maintainer, 2026-11-21): "the agent's entry stands as written, and the lead adds what he knows as an entry of his own after it". In the replay of six merged ADRs before that rule, "the lead rewrote 6 of 6 notes and 4 of 4 gaps" ([[adr-thread-process-measures-and-roadmap-form]]).
- **Commands:** `python scripts/roadmap_proposals.py check <file>` (agent, prints e.g. "F-869: todo -> partial; gap set; note + 592 characters"), `apply` (lead) ([[adr-thread-simulation-milestones-validation]]).
- **First live use:** ADR 0757's proposal, applied "from a copy without the three follow-ups", because entering follow-ups is a question of scope; ADR 0762's proposal was likewise "applied without its four follow-ups" until the maintainer answered ([[adr-thread-simulation-milestones-validation]], [[adr-thread-fan01-design-point]]).
- **No cutover:** `docs/roadmap/items/` as the source is not adopted now (ADR 0755 Q2 (a)) ([[adr-thread-process-measures-and-roadmap-form]]).

## Related

[[lead]], [[roadmap]], [[roadmap-status]], [[f-item]], [[adr-thread-process-measures-and-roadmap-form]]

## Open questions

- Two defects of form 1 keep F-864 partial: "the stale check refuses an item to which the lead only added a note; a gap cannot name a follow-up that has no id"; until fixed, such a proposal "is applied by the lead by hand and noted" ([[adr-thread-process-measures-and-roadmap-form]]).
