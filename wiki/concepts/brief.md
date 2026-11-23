---
type: concept
tags: [process, briefs]
sources: [doc-adr-briefs-readme, doc-process-lead-process, doc-process-documentation-review]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Brief

What the [[lead]] tells an [[implementation-agent]] when it issues a piece of work, kept as a file `docs/adr/briefs/<branch>.md`, committed on main before the agent starts. It is "the record that the work was issued, of what the agent was told to read, and of what produced the result: the rules version and the model" ([[doc-adr-briefs-readme]]).

## How rcfd uses it

- Header lines: `Branch`, `Items`, `ADR`, `Rules version`, `Model`, `Issued`, `Size (lead)` (five values: outcome; further registrations; heavy runs; questions for the maintainer; bytes to read); then `## Reading list` (paths with sizes in bytes) and `## Task` ([[doc-adr-briefs-readme]]).
- If the reading exceeds the reading limit, the lead splits the brief or points it at an area page's reading list ([[doc-process-lead-process]]).
- The registration cites its brief by path (rule 1) ([[doc-adr-briefs-readme]]).
- Abandoned: `Closed without merge (<date>): <why>` appended ([[doc-adr-briefs-readme]]).
- Why it exists: before 2026-11-18, 248 registrations referred to "the brief" and no brief was in the repository (G21); premise P2: "what an agent was told (its brief) is part of the record" ([[doc-process-documentation-review]]).
- The lead's size counts against a line's weekly budget; the agent's own size (in its registration) is scored and nothing depends on it (I12) ([[doc-process-documentation-review]]).

## Related

- [[process-report]] (section h: issued and not merged)
- [[process-ledger]]
- [[registration]]
