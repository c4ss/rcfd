---
type: entity
tags: [process, script]
sources: [doc-process-documentation-review, doc-process-lead-process, doc-adr-briefs-readme, doc-process-agent-rules, adr-thread-process-measures-and-roadmap-form, adr-thread-simulation-milestones-validation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Process report (scripts/process_report.py)

**What it is:** the single script that checks and reports the state of the process: roadmap checks, the ADR index, registration lint, open questions, provenance counts, overview numbers, dispatch, the lead's state, area pages, the weekly list, merge hygiene, test headers and process measures ([[doc-process-documentation-review]], I1).
**Where:** [scripts/process_report.py](../../scripts/process_report.py), tests `scripts/test_process_report.py`, exceptions lists in `docs/process/exceptions/`.

## Details

- **Sections a-m** (as proposed in I1 and put in place on 2026-11-18): (a) roadmap checks, including `gap` rules; (b) writes `docs/adr/index.md`; (c) registration lint (needs history, so it runs in the daily gate); (d) open questions: every ADR with an "Open questions for the maintainer" heading and no answers heading after it; (e) provenance: "from memory" and UNVERIFIED citations; (f) overview numbers "N done by the registered rule"; (g) dispatch per line and per week; (h) the lead's state: briefs issued and not merged, questions not answered; (i) area pages missing done items; (j) the weekly list; (k) merge hygiene (no conflict markers, roadmap SVGs equal their generator's output); (l) test headers naming an ADR; (m) process measures. A later section r lints registrations that cite agent-rules.md ([[doc-process-documentation-review]]).
- **Modes:** `--write` (run by the push script; regenerates the index and generated sections), `--check` (exits non-zero on stale generated files or failed lints; runs in the scripts job of ci.yml), `--weekly` (prints blocked priority items, lines over budget, unanswered questions and the process measures over 28 days), `--sections e` (counts ADRs citing from memory without a "Sources read" addendum) ([[doc-process-lead-process]], [[doc-process-documentation-review]]).
- Agents run `python scripts/process_report.py --check` among their checks (rule 7) ([[doc-process-agent-rules]]).
- Reads brief header lines (`Branch:`, `Items:`, `ADR:`, `Rules version:`, `Model:`, `Issued:`, `Size (lead):`) by their prefix ([[doc-adr-briefs-readme]]).
- "Only the lead regenerates docs/adr/index.md, on main; branches never do, so it cannot conflict" ([[doc-process-documentation-review]]).
- The unanswered-question count went from 88 to none on 2026-11-18; "That count reads markers and headings; it does not show that each question was answered well" ([[doc-process-documentation-review]]).

- From version 4 of the agent rules (2026-11-21), rule 7 asks agents to run `python scripts/process_report.py --check` before their report. This follows the maintainer's answer to ADR 0752 question 5 ("Yes, add it") ([[adr-thread-process-measures-and-roadmap-form]]).
- `--weekly` prints the four process measures of F-863: queue age, the lead's review minutes, proposed against written status, and escaped defects (ADR 0754) ([[adr-thread-process-measures-and-roadmap-form]]).
- A typical agent result on 2026-11-22: the only failures are "generated file is not current" (`docs/adr/index.md`, `docs/reference-data.md`), which rule 9 leaves to the lead ([[adr-thread-simulation-milestones-validation]]).

## Related

- [[process-ledger]]
- [[push-main]]
- [[maintainer-questions]]
- [[sources-read-addendum]]
- [[adr]]
