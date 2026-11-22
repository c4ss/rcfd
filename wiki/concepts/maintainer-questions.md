---
type: concept
tags: [process]
sources: [doc-process-lead-process, doc-process-documentation-review, doc-process-handover-2026-11-19, doc-process-agent-rules, doc-ui-restart-plan]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Questions to the maintainer (the question-and-answer flow)

How an open decision reaches the [[maintainer]] and how the answer is recorded. A question reaches the maintainer "only when it is about scope or priority, as options with a recommendation" ([[doc-process-lead-process]]).

## How rcfd uses it

1. **The agent writes it** in its own ADR under the fixed heading "Open questions for the maintainer", each with options and a recommendation (rule 8). Questions that rule 6 allows are taken on the recommendation under the standing instruction of 2026-11-11, except ones that would change a default result, a golden or a hash table, or need a source the repo lacks ([[doc-process-agent-rules]], [[doc-process-documentation-review]] I3).
2. **The process report lists it**: section (d) lists every ADR with such a heading and no "Answers of the maintainer" heading after it; the weekly list (`process_report.py --weekly`) puts blocked priority items first ([[process-report]], [[doc-process-lead-process]]).
3. **The lead filters**: puts a question only if it is about scope or priority, or if rule 6 forbids taking it; decides the rest and records the decision in the ADR ([[doc-process-lead-process]]).
4. **The answer is recorded** by the lead, with its date, under "Answers of the maintainer (<date>)" in the same ADR, on main ([[doc-process-lead-process]]). No shared file is written by many agents; "the one place is the list I1(d) generates" ([[doc-process-documentation-review]]).

- Questions listed before 2026-11-11 are closed by the lead on their written recommendation, with the same exceptions (the maintainer, 2026-11-18) ([[doc-process-lead-process]]).
- Before the flow existed: markers such as "NEEDS THE USER" (21 notes), "Open:" (79), "not asked" (10) were scattered in roadmap notes; the Studio stream already wrote questions with options, a recommendation and dated answers in 12 ADRs ([[doc-process-documentation-review]], G11, G17). The count of unanswered questions went from 88 to none on 2026-11-18 ([[doc-process-documentation-review]]).
- Example outside an ADR: the handover note's two scope questions, answered 2026-11-21 "Yes to both as recommended, go ahead", were written under "Answers of the maintainer" in that note ([[doc-process-handover-2026-11-19]]).
- Earlier practice in the Studio plan: decisions the research forced were listed "for the user when wave 47 starts" and the answers recorded in a later section ([[doc-ui-restart-plan]]).

## Related

- [[lead]]
- [[adr]]
- [[implementation-agent]]

## Open questions

- A budget in maintainer questions per line of work was proposed once the headings are in use; the sources do not say it was set ([[doc-process-documentation-review]], I6).
