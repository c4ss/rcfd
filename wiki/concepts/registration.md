---
type: concept
tags: [process]
sources: [doc-process-agent-rules, doc-contributing, doc-roadmap-overview]
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

## Related

- [[adr]]
- [[registered-limit]]
- [[known-miss]]
- [[roadmap-status]]
