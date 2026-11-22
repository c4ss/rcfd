---
type: concept
tags: [process, provenance, citations]
sources: [doc-process-lead-process, doc-process-handover-2026-11-19, doc-process-documentation-review]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# "Sources read" addendum (closing a citation from memory)

Because an ADR is append-only, a citation made from memory is not edited away; it is closed by an addendum at the end of the ADR headed `## Sources read (YYYY-MM-DD)`, one row per citation from memory ([[doc-process-lead-process]]).

## How rcfd uses it

- Columns: citation (what the ADR takes from it), status (`read`, `not obtainable`, `not a source`), where (the place in the source and where it was read), finding (`agrees`, or both values) ([[doc-process-lead-process]]).
- `read` means the agent opened the source itself and found the place; a second-hand statement is `read` of that second source. `not obtainable` says what was tried. Vendor documentation and material derived from it are not sources (rule 4). A differing finding is not fixed in the addendum; it goes to the lead as a defect report ([[doc-process-lead-process]]).
- `process_report.py --sections e` counts ADRs that cite from memory without such an addendum; "that count may fall and never rise" ([[doc-process-lead-process]]). It went from 311 to 305 with batch 1 ([[doc-process-documentation-review]]).
- Batch 1 (PR 28): six ADRs, 70 citations, 51 read, 11 differences in the record, none in a number the code uses; about 1.1 million tokens ([[doc-process-handover-2026-11-19]], [[doc-process-documentation-review]]).
- Scope decided 2026-11-21: a weekly citation item re-reads five citations drawn by seed; batches only for an ADR whose numbers the code uses and that marks a value UNVERIFIED; "The other citations from memory are not a backlog" ([[doc-process-lead-process]], [[doc-process-handover-2026-11-19]]).

## Related

- [[provenance]]
- [[unverified]]
- [[clean-room]]
- [[adr]]
