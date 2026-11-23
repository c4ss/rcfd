---
type: entity
tags: [process, role]
sources: [doc-process-agent-rules, doc-process-lead-process, doc-process-documentation-review, doc-adr-briefs-readme]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Implementation agent (role)

**What it is:** an agent issued one piece of work (one or more roadmap items) by the [[lead]] through a [[brief]], working in its own worktree under the agent rules; it registers, implements, measures and hands back a report, and does not merge, push or write the roadmap ([[doc-process-agent-rules]], [[doc-process-lead-process]]).
**Where:** rules in [docs/process/agent-rules.md](../../docs/process/agent-rules.md); entry point `AGENTS.md`.

## Details

- First commit is the [[registration]] ADR, alone, citing its brief and the agent rules by version ([[doc-process-agent-rules]], rule 1; [[doc-adr-briefs-readme]]).
- Writes its own ADR "above its answers"; the topic page under `docs/` whose subject it is; its fragment in `docs/reference-data.d/`; a roadmap proposal file `docs/roadmap/proposals/<ADR>-<item>.json` ([[doc-process-lead-process]]).
- Never edits a golden file or a hash table (rule 3) ([[doc-process-lead-process]]).
- Puts open questions in its ADR under "Open questions for the maintainer"; takes the recommended option under the standing instruction of 2026-11-11 except where rule 6 forbids it ([[maintainer-questions]]).
- The swarm premises behind the role: an agent "starts each item knowing only what it is given and what it reads, and agents do not talk to each other", so the record is the only channel; the agent proposes its follow-up items and sets its own numeric limits, which is why limits now cite a [[tolerance-class]] row or a source (P2, P6) ([[doc-process-documentation-review]]).
- Concurrency: at most 4 agents at work at once (stop condition, in force 2026-11-21) ([[doc-process-lead-process]]).
- The model is recorded in the brief (`Model:`); commits carry no agent marker (I7, decided 2026-11-18) ([[doc-adr-briefs-readme]], [[doc-process-documentation-review]]).

## Related

- [[lead]]
- [[maintainer]]
- [[work-streams]]
