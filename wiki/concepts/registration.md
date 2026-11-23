---
type: concept
tags: [process]
sources: [doc-process-agent-rules, doc-contributing, doc-roadmap-overview, doc-process-documentation-review, doc-process-lead-process, doc-process-tolerances, doc-adr-briefs-readme, doc-audit-report]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Registration (registered-before-run)

The first commit of every roadmap item: an ADR that fixes the checks, their numeric limits and the decision rule before any code or run ([[doc-process-agent-rules]], rule 1; [[doc-contributing]]).

## How rcfd uses it

- Status line 'REGISTERED <date>, written before any code of <item> and before any run of its checks'; cites its brief and the agent rules by version ([[doc-process-agent-rules]]).
- Contents: what was read, design from first principles or a cited source, checks table (acceptance clauses first), negative controls and mutations, a 'Size (agent):' line, the decision rule, what is not covered ([[doc-process-agent-rules]]).
- Follow-ups that diagnose or remedy a miss register under the parent item (`NNNN-fMMM-registration-...`) ([[doc-process-agent-rules]], rule 13).
- This is why 'done' needs no per-result acceptance by the maintainer ([[doc-roadmap-overview]]).
- The registration cites its [[brief]] by path; the brief carries the lead's size, the registration the agent's own ([[doc-adr-briefs-readme]], [[doc-process-documentation-review]] I12).
- Limits in registrations written from 2026-11-18 cite a [[tolerance-class]] row, a published reference, or the parent's acceptance; the lead sends back a registration with an unsourced limit before any code ([[doc-process-tolerances]], [[doc-process-lead-process]]).
- Compliance measured from git on 2026-11-18: of 421 campaign registrations, 414 had single-file creating commits and 406 preceded the item's first code commit; "No registration in either group was found with code first", and no numeric limit changed in any ([[doc-process-documentation-review]], G5).
- Before registration existed: the audit of 2026-10-14 found that git history "cannot prove 'no criterion was changed'" and that earlier tolerance edits were known only from self-disclosures ([[doc-audit-report]]; unreviewed model output).
- Practised in the campaign stream first, then made the rule for both [[work-streams]] on 2026-11-18 ([[doc-process-documentation-review]]).

## Related

- [[adr]]
- [[registered-limit]]
- [[known-miss]]
- [[roadmap-status]]
- [[brief]]
- [[tolerance-class]]
- [[lead]]
- [[implementation-agent]]
