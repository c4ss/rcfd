---
type: entity
tags: [process, role]
sources: [doc-process-lead-process, doc-process-documentation-review, doc-process-framework-reconciliation, doc-process-handover-2026-11-19, doc-adr-briefs-readme, doc-process-agent-rules, adr-thread-process-measures-and-roadmap-form, adr-thread-daily-gate-and-edit-lock]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# The lead (role)

**What it is:** the lead agent that works "between the maintainer and the implementation agents": it issues briefs, gives ADR numbers, merges, runs the push checks and writes the roadmap ([[doc-process-lead-process]]). "The lead is the one writer of main" ([[doc-process-framework-reconciliation]]).
**Where:** rules in [docs/process/lead-process.md](../../docs/process/lead-process.md) (version 1, 2026-11-18).

## Details

- **State outside the session:** issued = the brief files ([[brief]]), merged = [[process-ledger]], asked = the open-questions list of [[process-report]]. "Another session can read all three" ([[doc-process-lead-process]]). Before 2026-11-18, briefs, ADR numbering, item selection and roadmap edits were "written nowhere", and the lead's state lived only in its session ([[doc-process-documentation-review]], G9, Roles).
- **Issues work** on the lines of the priority statement in order, within each line's weekly budget; gives the next free ADR number on main ("Numbers are given by the lead only"); sends back a registration whose limit has no source ([[doc-process-lead-process]]).
- **Merges and pushes** with [[push-main]]; writes the item status in `docs/roadmap.json` in a commit of its own after the script's commit; applies agents' roadmap proposals with `scripts/roadmap_proposals.py apply` and adds its own entry after the agent's, without rewording it (the maintainer, 2026-11-21) ([[doc-process-lead-process]]).
- **Decides "done"** by the rule the registration fixed, including items that passed only after an amendment made under rule 2 (the maintainer, 2026-11-18) ([[doc-process-lead-process]], [[roadmap-status]]).
- **Questions:** puts a question to the maintainer only if it is about scope or priority, or rule 6 forbids taking it; decides the rest and records it in the ADR; writes "Answers of the maintainer" on main ([[maintainer-questions]]).
- **Sole writer of** `docs/roadmap.json`, the head of `docs/ROADMAP.md`, `docs/roadmap-overview.md`, `scripts/roadmap_progress.py` categories, briefs, the answers sections, and (on the maintainer's word) agent-rules.md, lead-process.md and tolerances.md ([[doc-process-lead-process]]).
- **Weekly:** prints `process_report.py --weekly`; issues one audit item and one citation item; runs `push_main.py --mutants --run` on one merge; issues distillation items ([[doc-process-lead-process]]).
- **Stop conditions** (in force 2026-11-21, ADR 0754): no further agent while 3+ reports wait unread, while escaped defects exceed 2 commits and 1 in 10 merges over 28 days, while the newest gate line is red (except the fix), or beyond 4 agents at once ([[doc-process-lead-process]]).
- **Golden files:** re-records an "additive golden edit" on main after checking the diff only adds; any other golden change and every `baseline_hashes.rs` change needs the maintainer's word ([[doc-process-lead-process]]).
- Historical signs that motivated the written process: F-571 merged twice (2026-11-15); F-287 and F-288 waiting on the lead's approval since 2026-11-06; the lead committed roadmap status a median 32 s after a merge, before its checks ([[doc-process-documentation-review]]).
- No reviewer of the lead exists other than the maintainer; a weekly seeded audit item and the weekly list make its state readable "and stop there" ([[doc-process-documentation-review]]).

- **Stop conditions in force** are recorded with their basis in [[adr-thread-process-measures-and-roadmap-form]]. Condition 3 applied on the day of the answer: "until a gate line is green the lead starts no agent except on F-866" (ADR 0754) ([[adr-thread-process-measures-and-roadmap-form]]).
- **Proposal files:** the lead applies [[roadmap-proposal]] files. Follow-ups are entered only after the maintainer answers questions of scope (ADRs 0757, 0762) ([[adr-thread-process-measures-and-roadmap-form]]).
- **Lead decisions recorded in ADRs:** the lead fixed a broken link inside ADR 0759 and entered the ADR in `docs/process/exceptions/adr-rewritten.txt`. The lead also wrote F-855 partial, against the agent's "done" by the registered rule (ADR 0758) ([[adr-thread-daily-gate-and-edit-lock]]).

## Related

- [[maintainer]]
- [[implementation-agent]]
- [[work-streams]]
- [[doc-process-handover-2026-11-19]]

## Open questions

- The lead's load "was not measured" and may be a constraint ([[doc-process-documentation-review]]).
- The lead still edits `roadmap.json` with one-off scripts kept outside the repository and re-enters every status ([[doc-process-framework-reconciliation]]).
